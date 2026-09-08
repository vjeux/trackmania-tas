//! Item-file helpers shared by the item builders: Ident rewriting (header
//! and body), the library archive (`zip`), the crystal-from-pack-model bake
//! of the `crystal-item` / `catalog-lib` commands, and the mesh-editor
//! material tables of the crystal era (2026-09-05; the static-item path of
//! `tiny_library` superseded the crystal map library that lived here).

use crate::{container, names, rescale::{self, Rescale}, store::DataStore};
use std::collections::BTreeMap;
use tmmaps::gbx::Gbx;

pub const AUTHOR: &str = "KTaOsd-lTR2zkoskETSfPA";
pub const BLUEBAY_KEY: &str = "660C4C156B80337E296A1034B0AA05B8";
pub const STADIUM_KEY: &str = "B773D73047A4104857722366D78D28A6";

/// A crystal item from a pack model's VISUAL geometry (finest detail level):
/// (item bytes, faces). The collision mesh was tried first and is a
/// simplification that lacks kerbs, trims and end pieces (one-block test
/// 2026-09-05). The map's collection decides the material family.
pub fn crystal_from_model_in(store: &mut DataStore, logical: &str, template: &[u8], ident: &str, collection: u32) -> Result<(Vec<u8>, usize), String> {
    let m = store.load_model(logical)?;
    let mut c = crate::geom::Collector::new(store);
    c.link_labels = true;
    c.finest_lod_only = true;
    c.model(&m, &crate::geom::IDENTITY, 0);
    let surface_links = c.surface_links.clone();
    let scene = c.scene;
    let mut mesh = crate::crystal::CrystalMesh::default();
    let mut materials = Vec::new();
    for (label, g) in &scene.groups {
        if g.tris.is_empty() || !label.contains('|') {
            continue;
        }
        // Terrain visuals shade through a shared id material; the look
        // material is the one the collision surface names.
        let label: &str = if label.starts_with("Techno3\\") && !surface_links.is_empty() { &surface_links[0] } else { label };
        let Some(spec) = visual_material_for(label, collection) else { continue };
        mesh.add_tris(&g.verts, &g.tris, materials.len() as u32, 32.0);
        materials.push(spec);
    }
    if collection != 26 {
        // The mesh-editor materials are checker/tint textures: sampling one
        // point of each gives clean flat colours instead of checkerboards.
        mesh.flatten_uvs();
    }
    if mesh.faces.is_empty() {
        return Err("no visual geometry the walker can read (procedural or unparsed model)".into());
    }
    let faces = mesh.faces.len();
    Ok((crate::crystal::build_item(template, ident, ident, &materials, &mesh), faces))
}

pub fn put_string(v: &mut Vec<u8>, s: &str) {
    v.extend_from_slice(&(s.len() as u32).to_le_bytes());
    v.extend_from_slice(s.as_bytes());
}

/// A reference table for a file that lives in `Items/`, naming files by
/// their logical pack path (`BlueBay\Media\Prefab\...`): ancestor level 1
/// (up to the root the collections hang off), a shared folder tree, and one
/// entry per (node index, path, useFile). Folder indices follow the game's
/// convention: 0 is the ancestor directory itself, the tree below it is
/// numbered depth-first from 1.
pub fn ref_table(entries: &[(u32, String, bool)]) -> Vec<u8> {
    #[derive(Default)]
    struct Folder {
        name: String,
        subs: Vec<Folder>,
    }
    fn insert(f: &mut Folder, parts: &[&str]) {
        if parts.is_empty() {
            return;
        }
        let pos = match f.subs.iter().position(|s| s.name == parts[0]) {
            Some(p) => p,
            None => {
                f.subs.push(Folder { name: parts[0].to_string(), subs: Vec::new() });
                f.subs.len() - 1
            }
        };
        insert(&mut f.subs[pos], &parts[1..]);
    }
    fn number(f: &Folder, prefix: &str, next: &mut u32, out: &mut BTreeMap<String, u32>) {
        for sub in &f.subs {
            let path = if prefix.is_empty() { sub.name.clone() } else { format!("{prefix}\\{}", sub.name) };
            out.insert(path.clone(), *next);
            *next += 1;
            number(sub, &path, next, out);
        }
    }
    fn write(f: &Folder, v: &mut Vec<u8>) {
        v.extend_from_slice(&(f.subs.len() as u32).to_le_bytes());
        for sub in &f.subs {
            put_string(v, &sub.name);
            write(sub, v);
        }
    }
    let mut root = Folder::default();
    let mut split: Vec<(u32, String, String, bool)> = Vec::new();
    for (node, path, use_file) in entries {
        let p = path.replace('/', "\\");
        let (dir, file) = match p.rfind('\\') {
            Some(i) => (p[..i].to_string(), p[i + 1..].to_string()),
            None => (String::new(), p.clone()),
        };
        if !dir.is_empty() {
            insert(&mut root, &dir.split('\\').collect::<Vec<_>>());
        }
        split.push((*node, dir, file, *use_file));
    }
    let mut index = BTreeMap::new();
    let mut next = 1u32;
    number(&root, "", &mut next, &mut index);
    let mut v = Vec::new();
    v.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    v.extend_from_slice(&1u32.to_le_bytes()); // ancestor: Items -> root
    write(&root, &mut v);
    for (node, dir, file, use_file) in split {
        v.extend_from_slice(&0u32.to_le_bytes()); // flags: named file
        put_string(&mut v, &file);
        v.extend_from_slice(&node.to_le_bytes());
        v.extend_from_slice(&(use_file as u32).to_le_bytes());
        let fi = if dir.is_empty() { 0 } else { index[&dir] };
        v.extend_from_slice(&fi.to_le_bytes());
    }
    v
}

pub fn replace_lp(mut b: Vec<u8>, old: &str, new: &str) -> Vec<u8> {
    let oldb = old.as_bytes();
    let mut hits = Vec::new();
    for i in 4..=b.len().saturating_sub(oldb.len()) {
        if &b[i..i + oldb.len()] == oldb
            && u32::from_le_bytes(b[i - 4..i].try_into().unwrap()) as usize == oldb.len()
        {
            hits.push(i);
        }
    }
    assert!(!hits.is_empty(), "length-prefixed string {old:?} is absent");
    for i in hits.into_iter().rev() {
        b[i - 4..i].copy_from_slice(&(new.len() as u32).to_le_bytes());
        b.splice(i..i + oldb.len(), new.as_bytes().iter().copied());
    }
    b
}

pub fn rewrite_ident(bytes: &[u8], old_name: &str, alias: &str, old_author: &str) -> Vec<u8> {
    rename_item_ident(bytes, old_name, old_author, alias, AUTHOR)
}

/// A game item file under a new Ident and author: the header chunk
/// 0x2E001003 rebuilt (`set_header_ident`), every length-prefixed
/// occurrence of the old name and author in the body replaced (the body
/// ident's two lookback strings, the Name and Description chunks). The
/// reference table is untouched.
pub fn rename_item_ident(bytes: &[u8], old_name: &str, old_author: &str, name: &str, author: &str) -> Vec<u8> {
    let headed = set_header_ident(bytes, name, author);
    let mut g = Gbx::parse(&headed);
    if !old_name.is_empty() && old_name != name {
        g.body = replace_lp(g.body, old_name, name);
    }
    if !old_author.is_empty() && old_author != author && old_author != old_name {
        g.body = replace_lp(g.body, old_author, author);
    }
    let body = g.body.clone();
    g.write_body_recompressed(&body)
}

/// Give an item file the Ident the game matches placements against: the
/// header chunk 0x2E001003 rebuilt with `name` and `author` as fresh lookback
/// strings (the archive crystals ship with NO ident name at all, and a
/// game-made item carries whatever path it was saved under). The chunk's
/// other fields are copied through; the header size table is updated.
pub fn set_header_ident(bytes: &[u8], name: &str, author: &str) -> Vec<u8> {
    use tmmaps::gbx::Reader;
    let mut g = Gbx::parse(bytes);
    let ud = g.user_data.clone();
    let n = u32::from_le_bytes(ud[0..4].try_into().unwrap()) as usize;
    let mut off = 4 + n * 8;
    let mut out = Vec::new();
    out.extend_from_slice(&ud[0..4]);
    let mut chunks: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    for i in 0..n {
        let id = u32::from_le_bytes(ud[4 + i * 8..8 + i * 8].try_into().unwrap());
        let raw_size = u32::from_le_bytes(ud[8 + i * 8..12 + i * 8].try_into().unwrap());
        let size = (raw_size & 0x7FFF_FFFF) as usize;
        let d = ud[off..off + size].to_vec();
        off += size;
        let d = if id == 0x2E001003 {
            let mut r = Reader::new(&d);
            let mut table: Vec<String> = Vec::new();
            let lb = |r: &mut Reader, table: &mut Vec<String>| -> Option<String> {
                let w = r.u32();
                if w == 0xFFFF_FFFF { return None; }
                if (w & 0x3FFF_FFFF) == 0 { let s = r.string(); table.push(s.clone()); return Some(s); }
                Some(table[((w & 0x3FFF_FFFF) - 1) as usize].clone())
            };
            let lbver = r.u32();
            assert_eq!(lbver, 3, "header ident lookback version");
            let _old_name = lb(&mut r, &mut table);
            let coll = r.u32();
            let _old_author = lb(&mut r, &mut table);
            let v = r.u32();
            assert!(v >= 7, "collector header version {v} unsupported");
            let page = r.string();
            let parent = lb(&mut r, &mut table);
            let rest = d[r.o..].to_vec(); // flags, catalog position, name, prod state
            let mut w = Vec::new();
            let mut t2: Vec<String> = Vec::new();
            let mut put = |w: &mut Vec<u8>, s: Option<&str>| match s {
                None => w.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()),
                Some(s) => match t2.iter().position(|x| x == s) {
                    Some(i) => w.extend_from_slice(&(0x4000_0000u32 | (i as u32 + 1)).to_le_bytes()),
                    None => {
                        t2.push(s.to_string());
                        w.extend_from_slice(&0x4000_0000u32.to_le_bytes());
                        put_string(w, s);
                    }
                },
            };
            w.extend_from_slice(&3u32.to_le_bytes());
            put(&mut w, Some(name));
            w.extend_from_slice(&coll.to_le_bytes());
            put(&mut w, Some(author));
            w.extend_from_slice(&v.to_le_bytes());
            put_string(&mut w, &page);
            put(&mut w, parent.as_deref());
            w.extend_from_slice(&rest);
            w
        } else {
            d
        };
        chunks.push((id, raw_size & 0x8000_0000, d));
    }
    for (id, flag, d) in &chunks {
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&((d.len() as u32) | flag).to_le_bytes());
    }
    for (_, _, d) in &chunks {
        out.extend_from_slice(d);
    }
    g.user_data = out;
    let body = g.body.clone();
    g.write_body_recompressed(&body)
}

/// Body counterpart of `set_header_ident` for an item whose body ident has
/// NO name (the archive crystals, game-made block items): chunk 0x2E00100B
/// currently reads `FFFFFFFF, collection, NEW author`. It becomes
/// `NEW name, collection, REF 1` — the same number of string definitions, so
/// no later lookback index moves; the item's author reads as its own name.
/// Refuses anything else (a named body ident would need a full renumber).
pub fn set_body_ident_nameless(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut g = Gbx::parse(bytes);
    let b = &g.body;
    let pos = b.windows(4).position(|w| w == 0x2E00100Bu32.to_le_bytes()).expect("body ident chunk 0x2E00100B");
    let o = pos + 4;
    assert_eq!(&b[o..o + 4], &[0xFF, 0xFF, 0xFF, 0xFF], "body ident already has a name; renumbering needed");
    let coll = &b[o + 4..o + 8];
    assert_eq!(u32::from_le_bytes(b[o + 8..o + 12].try_into().unwrap()), 0x4000_0000, "author is not the first body string");
    let alen = u32::from_le_bytes(b[o + 12..o + 16].try_into().unwrap()) as usize;
    let end = o + 16 + alen;
    let mut nb = Vec::with_capacity(b.len() + name.len());
    nb.extend_from_slice(&b[..o]);
    nb.extend_from_slice(&0x4000_0000u32.to_le_bytes());
    put_string(&mut nb, name);
    nb.extend_from_slice(coll);
    nb.extend_from_slice(&0x4000_0001u32.to_le_bytes());
    nb.extend_from_slice(&b[end..]);
    g.body = nb.clone();
    g.write_body_recompressed(&nb)
}

/// Insert a name into a nameless body ident (`FFFFFFFF, collection, NEW
/// author` -> `NEW name, collection, NEW author`). This ADDS a lookback
/// string, so every later index in the body moves by one: only safe for
/// bodies that back-reference nothing, which is what a game-made block item
/// (author + archetype name, both fresh definitions) looks like. The caller
/// vouches for that; `set_body_ident_nameless` is the shift-free variant.
pub fn set_body_ident_insert(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut g = Gbx::parse(bytes);
    let b = &g.body;
    let pos = b.windows(4).position(|w| w == 0x2E00100Bu32.to_le_bytes()).expect("body ident chunk 0x2E00100B");
    let o = pos + 4;
    assert_eq!(&b[o..o + 4], &[0xFF, 0xFF, 0xFF, 0xFF], "body ident already has a name");
    let mut nb = Vec::with_capacity(b.len() + name.len() + 8);
    nb.extend_from_slice(&b[..o]);
    nb.extend_from_slice(&0x4000_0000u32.to_le_bytes());
    put_string(&mut nb, name);
    nb.extend_from_slice(&b[o + 4..]);
    g.body = nb.clone();
    g.write_body_recompressed(&nb)
}

/// The mesh-editor material a Stadium item material stands in for, outside
/// Stadium (a flat tint each; see crystal::material_for_physics_name_in).
pub fn editors_link_for_stadium_material(name: &str) -> &'static str {
    match name {
        // BlueBay terrain (measured on the 46 Summer 01 block models)
        "Land" | "TransitionToSand" => "Editors\\MeshEditorMedia\\Materials\\Grass",
        "SeaFloor" | "TransitionToSeaFloor" | "Beach" | "Sand" => "Editors\\MeshEditorMedia\\Materials\\Sand",
        "HillPxz" | "CliffPxz" | "TransitionRocks" | "TransitionRocksToCliffPxz" | "Rock" => "Editors\\MeshEditorMedia\\Materials\\Rock",
        "Water" => "Editors\\MeshEditorMedia\\Materials\\Ice",
        // Stadium structure / screens
        "Structure" | "Pylon" | "ScreenBack" | "Deco" | "TechnicsStep" | "TechnicsSpecials" => "Editors\\MeshEditorMedia\\Materials\\Metal",
        "RaceAd6x1" | "Ad2x3Screen" | "Ad4x1Screen" | "Ad155Screen" | "Show4x1" | "CanopyGlass" => "Editors\\MeshEditorMedia\\Materials\\Ice",
        "TechnicsTrimsColorize" => "Editors\\MeshEditorMedia\\Materials\\Metal",
        "Speedometer" | "SpeedometerLight" | "ItemObstacleLightOn" | "SpecialSignTurbo" | "SpecialSignOff" | "SpecialFXTurbo" => "Editors\\MeshEditorMedia\\Materials\\Plastic",
        "RoadTech" | "RoadDirt" | "RoadIce" | "RoadBump" => "Editors\\MeshEditorMedia\\Materials\\Asphalt",
        "TrackBorders" | "TrackBordersOff" | "DecalPaint2Logo4x1" | "DecalPlatform" => "Editors\\MeshEditorMedia\\Materials\\Concrete",
        "Technics" | "TechnicsTrims" | "ItemPillar" | "ItemTrackBarrier" => "Editors\\MeshEditorMedia\\Materials\\Metal",
        "LightSpot" | "SpeedometerLight_Dyna" => "Editors\\MeshEditorMedia\\Materials\\Plastic",
        "PlatformTech" | "TrackWallClips" | "TrackWall" => "Editors\\MeshEditorMedia\\Materials\\Stone",
        "Grass" => "Editors\\MeshEditorMedia\\Materials\\Grass",
        _ => "Editors\\MeshEditorMedia\\Materials\\Concrete",
    }
}

/// Material for a VISUAL group label `LINK|PHYS` in the target collection.
/// `None` = drop the faces: decals (physics 28, overlays that would become
/// opaque plates) and lights (32). Stadium keeps the real links; elsewhere
/// the name is mapped onto the mesh-editor family, which is all the
/// environment accepts for embedded items.
pub fn visual_material_for(label: &str, collection: u32) -> Option<crate::crystal::MaterialSpec> {
    let (link, phys) = label.rsplit_once('|').unwrap_or((label, "16"));
    let physics: u8 = phys.parse().unwrap_or(16);
    if matches!(physics, 28 | 32) {
        return None;
    }
    let name = link.rsplit('\\').next().unwrap_or(link);
    if name.starts_with("Decal") || name.starts_with("LightSpot") || name == "SpeedometerLight_Dyna" {
        return None;
    }
    // Water surfaces: the game's water is a separate system; a flat blue
    // plate at sea level is what the tile draws and it is not the race.
    if name == "Water" {
        return None;
    }
    if collection == 26 {
        return Some(crate::crystal::material_for_link_label(label));
    }
    Some(crate::crystal::MaterialSpec { link: editors_link_for_stadium_material(name).to_string(), physics })
}

/// Replace every `Stadium\Media\Material\X` link string in an item body with
/// its mesh-editor stand-in. Body strings are length-prefixed and nothing in
/// a crystal item points at a body offset, so the body may grow or shrink.
pub fn remap_stadium_links(bytes: &[u8]) -> Vec<u8> {
    let mut g = Gbx::parse(bytes);
    let body = g.body.clone();
    let needle = b"Stadium\\Media\\Material\\";
    let mut out = Vec::with_capacity(body.len());
    let mut i = 0usize;
    while i < body.len() {
        if i >= 4 && body[i..].starts_with(needle) {
            let len = u32::from_le_bytes(body[i - 4..i].try_into().unwrap()) as usize;
            if len >= needle.len() && i + len <= body.len() && body[i..i + len].iter().all(|c| c.is_ascii_graphic() || *c == b' ') {
                let name = std::str::from_utf8(&body[i + needle.len()..i + len]).unwrap_or("");
                // The physics byte sits 2 bytes before the length prefix
                // (phys, gameplay, len, link). Decals (28, NotCollidable) and
                // lights (32) keep their link: culled here, they would
                // otherwise become opaque plates.
                let phys = body[i - 6];
                if matches!(phys, 28 | 32) {
                    out.push(body[i]);
                    i += 1;
                    continue;
                }
                let repl = editors_link_for_stadium_material(name);
                out.truncate(out.len() - 4);
                out.extend_from_slice(&(repl.len() as u32).to_le_bytes());
                out.extend_from_slice(repl.as_bytes());
                i += len;
                continue;
            }
        }
        out.push(body[i]);
        i += 1;
    }
    g.body = out.clone();
    g.write_body_recompressed(&out)
}

/// Rewrite the collection id in both idents (header 0x2E001003 and body
/// 0x2E00100B). A BlueBay map places items in collection 0x1C; an item that
/// says Stadium (0x1A) inside is dropped there.
pub fn set_ident_collection(bytes: &[u8], collection: u32) -> Vec<u8> {
    tmmaps::header::set_ident_collection(bytes, collection)
}

pub fn wrapper(template: &[u8], alias: &str, prefab: &str) -> Vec<u8> {
    let mut g = Gbx::parse(template);
    g.user_data = replace_lp(g.user_data, "GateSupport", alias);
    g.body = replace_lp(g.body, "GateSupport", alias);
    g.user_data = replace_lp(g.user_data, "Nadeo", AUTHOR);
    g.body = replace_lp(g.body, "Nadeo", AUTHOR);
    g.ref_table = ref_table(&[(1, prefab.to_string(), false)]);
    let body = g.body.clone();
    g.write_body_recompressed(&body)
}

/// An embedded copy of one of the game's own items, re-pointed at scaled
/// copies of its geometry: the item's own file (so waypoint type, placement
/// parameters and everything else survive) with its Ident renamed to `alias`
/// and its reference table rebuilt to name the scaled files from `Items/`.
/// `None` when nothing the item references is geometry this tool can scale
/// (vegetation is procedural `VegetTreeModel`s), so the item is left alone.
pub fn item_copy(
    store: &mut DataStore,
    rs: &mut Rescale,
    logical: &str,
    stem: &str,
    alias: &str,
) -> Result<Option<Vec<u8>>, String> {
    let bytes = store.read(logical)?;
    let g = container::Gbx::parse(&bytes)?;
    let folder = match logical.rfind('\\') {
        Some(i) => &logical[..i],
        None => "",
    };
    let mut entries = Vec::new();
    let mut scaled_any = false;
    for e in &g.refs {
        let path = names::join(folder, &g.ref_path(e));
        let path = if rescale::is_geometry(&path) {
            scaled_any = true;
            rs.file(store, &path)?
        } else {
            path
        };
        entries.push((e.node_index, path, e.use_file));
    }
    if !scaled_any {
        return Ok(None);
    }
    let mut out = Gbx::parse(&bytes);
    out.user_data = replace_lp(out.user_data, stem, alias);
    out.body = replace_lp(out.body, stem, alias);
    out.user_data = replace_lp(out.user_data, "Nadeo", AUTHOR);
    out.body = replace_lp(out.body, "Nadeo", AUTHOR);
    out.ref_table = ref_table(&entries);
    let body = out.body.clone();
    Ok(Some(out.write_body_recompressed(&body)))
}


/// Minimal stored ZIP with explicit directory rows for Trackmania's browser.
/// The archive the map carries its custom files in, laid out the way the
/// game writes it (read off downloaded maps that embed items): entries are
/// RELATIVE to the user folder (`Items/Foo.Item.Gbx`), deflated (method 8,
/// version 2.0), no directory rows, no extra fields. A file under the item
/// Ident is what the game matches the manifest against, so any
/// `C:/Users/<user>/Documents/Trackmania/` prefix on a key is dropped here.
///
/// "Deflate" is written as stored deflate blocks (BTYPE 00): a valid stream
/// for any inflater, needing no compressor.
pub fn zip(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    // The game requires embedded item entries to be DEFLATED: a stored zip
    // of Granady's own items crashed the loader, the same items deflated
    // loaded (bisected on U10S_01 [Tiny], 2026-09-05). Small crystal items
    // happened to survive stored, which hid this for a day.
    tmmaps::header::deflated_zip(files)
}
