//! Build a complete tiny-map item library from the game's client packs.
//!
//! Modern block placements become thin CGameItemModel wrappers around the exact
//! prefab selected by the source map. The three legacy, prefab-less models used
//! by Summer 01 come from the public Nadeo converted-item archive; their Ident
//! strings are rewritten here, in Rust. An intentionally empty pillar mobil is
//! represented by the bundled empty item template so it remains an explicit
//! placement rather than a silent omission.

use crate::{container, embedded, names, rescale::{self, Rescale}, store::DataStore};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use tmmaps::{gbx::Gbx, map::MapFile};

pub const AUTHOR: &str = "KTaOsd-lTR2zkoskETSfPA";
pub const BLUEBAY_KEY: &str = "660C4C156B80337E296A1034B0AA05B8";
pub const STADIUM_KEY: &str = "B773D73047A4104857722366D78D28A6";

const LEGACY_PLATFORM: &str = "Walls/DecoWall/Platform/Platform/PlatformBase.Item.Gbx";
const LEGACY_SLOPE: &str = "Walls/DecoWall/Unnamed_1/SlopeStraight/DecoWallSlope2Straight.Item.Gbx";
const LEGACY_WALL: &str = "Walls/DecoWall/Platform/Platform/DecoWallBase.Item.Gbx";
/// The archive crystal every generated item is written around.
const CRYSTAL_TEMPLATE: &str = "RoadTech/Main/Main/RoadTechStraight.Item.Gbx";

/// The archive's waypoint crystal for a start/checkpoint/finish prefab.
fn waypoint_archive_item(prefab: &str) -> Option<&'static str> {
    let stem = prefab.rsplit('\\').next().unwrap_or(prefab);
    match stem {
        "Start_Air.Prefab.Gbx" => Some("RoadTech/Racing/StartFinish/RoadTechStart.Item.Gbx"),
        "Finish_Air.Prefab.Gbx" => Some("RoadTech/Racing/StartFinish/RoadTechFinish.Item.Gbx"),
        "Checkpoint_Air.Prefab.Gbx" => Some("RoadTech/Racing/Checkpoints/RoadTechCheckpoint.Item.Gbx"),
        _ => None,
    }
}

/// An archive item re-identified as `ident` (header and body; the body's
/// author reads as the ident, see `set_body_ident_nameless`).
pub fn nameless_ident(bytes: &[u8], ident: &str) -> Vec<u8> {
    let out = set_header_ident(bytes, ident, ident);
    set_body_ident_nameless(&out, ident)
}

/// A crystal item from a pack model's visual geometry: (item bytes, faces).
pub fn crystal_from_model(store: &mut DataStore, logical: &str, template: &[u8], ident: &str) -> Result<(Vec<u8>, usize), String> {
    let m = store.load_model(logical)?;
    let mut c = crate::geom::Collector::new(store);
    c.link_labels = true;
    c.model(&m, &crate::geom::IDENTITY, 0);
    let surface_links = c.surface_links.clone();
    let scene = c.scene;
    let mut mesh = crate::crystal::CrystalMesh::default();
    let mut materials = Vec::new();
    for (label, g) in &scene.groups {
        if g.tris.is_empty() || !label.contains('|') {
            continue;
        }
        let label: &str = if label.starts_with("Techno3\\") && !surface_links.is_empty() { &surface_links[0] } else { label };
        let spec = crate::crystal::material_for_link_label(label);
        mesh.add_tris(&g.verts, &g.tris, materials.len() as u32, 8.0);
        materials.push(spec);
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
    let mut g = Gbx::parse(bytes);
    g.user_data = replace_lp(g.user_data, old_name, alias);
    g.body = replace_lp(g.body, old_name, alias);
    g.user_data = replace_lp(g.user_data, old_author, AUTHOR);
    g.body = replace_lp(g.body, old_author, AUTHOR);
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
            let mut lb = |r: &mut Reader, table: &mut Vec<String>| -> Option<String> {
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

/// Rewrite the collection id in both idents (header 0x2E001003 and body
/// 0x2E00100B). A BlueBay map places items in collection 0x1C; an item that
/// says Stadium (0x1A) inside is dropped there.
pub fn set_ident_collection(bytes: &[u8], collection: u32) -> Vec<u8> {
    use tmmaps::gbx::Reader;
    let mut g = Gbx::parse(bytes);
    // header
    let ud = g.user_data.clone();
    let n = u32::from_le_bytes(ud[0..4].try_into().unwrap()) as usize;
    let mut off = 4 + n * 8;
    let mut new_ud = ud.clone();
    for i in 0..n {
        let id = u32::from_le_bytes(ud[4 + i * 8..8 + i * 8].try_into().unwrap());
        let size = (u32::from_le_bytes(ud[8 + i * 8..12 + i * 8].try_into().unwrap()) & 0x7FFF_FFFF) as usize;
        if id == 0x2E001003 {
            let mut r = Reader::new(&ud[off..off + size]);
            r.u32(); // lbver
            let w = r.u32();
            if (w & 0x3FFF_FFFF) == 0 && w != 0xFFFF_FFFF { r.string(); }
            let at = off + r.o;
            new_ud[at..at + 4].copy_from_slice(&collection.to_le_bytes());
        }
        off += size;
    }
    g.user_data = new_ud;
    // body
    let mut body = g.body.clone();
    let pos = body.windows(4).position(|w| w == 0x2E00100Bu32.to_le_bytes()).expect("body ident chunk");
    let mut o = pos + 4;
    let w = u32::from_le_bytes(body[o..o + 4].try_into().unwrap());
    o += 4;
    if (w & 0x3FFF_FFFF) == 0 && w != 0xFFFF_FFFF {
        let l = u32::from_le_bytes(body[o..o + 4].try_into().unwrap()) as usize;
        o += 4 + l;
    }
    body[o..o + 4].copy_from_slice(&collection.to_le_bytes());
    g.body = body.clone();
    g.write_body_recompressed(&body)
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

/// Where the game keeps the item file for a placed model name.
fn find_item_file(store: &DataStore, model: &str) -> Option<String> {
    let want = format!("\\{}.ITEM.GBX", model.to_uppercase());
    let mut hits: Vec<String> = store
        .entries()
        .map(|e| e.path())
        .filter(|p| p.to_uppercase().ends_with(&want))
        .collect();
    hits.sort_by_key(|p| (!p.to_uppercase().contains("\\ITEMS\\"), p.len()));
    hits.into_iter().next()
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut c = !0u32;
    for &x in bytes {
        c ^= x as u32;
        for _ in 0..8 {
            c = (c >> 1) ^ ((0u32.wrapping_sub(c & 1)) & 0xEDB_88320);
        }
    }
    !c
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
    fn relative(name: &str) -> String {
        let n = name.replace('\\', "/");
        match n.find("/Documents/Trackmania/") {
            Some(i) => n[i + "/Documents/Trackmania/".len()..].to_string(),
            None => n.trim_start_matches('/').to_string(),
        }
    }
    fn deflate_stored(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() + data.len() / 65535 * 5 + 5);
        if data.is_empty() {
            out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
            return out;
        }
        let mut chunks = data.chunks(65535).peekable();
        while let Some(c) = chunks.next() {
            let last = chunks.peek().is_none();
            out.push(if last { 1 } else { 0 });
            out.extend_from_slice(&(c.len() as u16).to_le_bytes());
            out.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
            out.extend_from_slice(c);
        }
        out
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    let mut count = 0u16;
    for (name, data) in files {
        let name = relative(name);
        assert!(!name.ends_with('/'), "directory rows are not written: {name}");
        let off = out.len() as u32;
        let n = name.as_bytes();
        let crc = crc32(data);
        let comp = deflate_stored(data);
        out.extend_from_slice(b"PK\x03\x04");
        for x in [20u16, 0, 8, 0, 0] {
            out.extend_from_slice(&x.to_le_bytes());
        }
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(n.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(n);
        out.extend_from_slice(&comp);

        central.extend_from_slice(b"PK\x01\x02");
        for x in [20u16, 20, 0, 8, 0, 0] {
            central.extend_from_slice(&x.to_le_bytes());
        }
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(n.len() as u16).to_le_bytes());
        for x in [0u16, 0, 0, 0] {
            central.extend_from_slice(&x.to_le_bytes());
        }
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(n);
        count += 1;
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(b"PK\x05\x06");
    for x in [0u16, 0, count, count] {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn fallback_alias(name: &str, flags: u32) -> Option<&'static str> {
    match name {
        "PlatformBase" => Some("AC00000101"),
        "DecoWallSlope2Straight" => Some("AC00000102"),
        "DecoWallBasePillar" => Some("AC00000103"),
        "StructurePillar" if flags == 0x0000_4001 => Some("AC00000104"),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build(
    map: &Path,
    catalog: &Path,
    footprints: &Path,
    nadeo_zip: &Path,
    empty_template: &Path,
    blue_pak: &Path,
    stadium_pak: &Path,
    out_zip: &Path,
    out_map: &Path,
    scale: f32,
) {
    assert!(scale.is_finite() && scale > 0.0, "scale must be positive");
    let source = MapFile::load(map);
    let mut catalog_map = BTreeMap::new();
    for (line_no, line) in fs::read_to_string(catalog).unwrap().lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 3 {
            panic!(
                "{}:{}: expected NAME<TAB>FLAGS<TAB>PREFAB",
                catalog.display(),
                line_no + 1
            );
        }
        let flags = u32::from_str_radix(f[1], 16).expect("hex block flags");
        assert!(catalog_map
            .insert((f[0].to_string(), flags), f[2].to_string())
            .is_none());
    }

    let mut footprint_map: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for (line_no, line) in fs::read_to_string(footprints).unwrap().lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 3 {
            panic!(
                "{}:{}: expected NAME<TAB>SX<TAB>SZ",
                footprints.display(),
                line_no + 1
            );
        }
        let sx: u32 = f[1].parse().expect("SX cells");
        let sz: u32 = f[2].parse().expect("SZ cells");
        assert!(footprint_map
            .insert(f[0].to_string(), (sx, sz))
            .is_none());
    }
    for b in &source.blocks {
        assert!(
            footprint_map.contains_key(&b.name),
            "no footprint for block model {}",
            b.name
        );
    }

    let mut paths = BTreeSet::new();
    for b in &source.blocks {
        if fallback_alias(&b.name, b.flags).is_none() {
            let p = catalog_map
                .get(&(b.name.clone(), b.flags))
                .unwrap_or_else(|| {
                    panic!(
                        "no resolved prefab for block#{} {} flags {:08X}",
                        b.index, b.name, b.flags
                    )
                });
            paths.insert(p.clone());
        }
    }
    let aliases: BTreeMap<String, String> = paths
        .into_iter()
        .enumerate()
        .map(|(i, p)| (p, format!("AC{i:08}")))
        .collect();
    assert_eq!(aliases.len(), 101, "Summer 01 direct-prefab count changed");

    // One store over both client packs: BlueBay prefabs name Stadium files
    // (pillars, slope bases) and each pack has its own key.
    let mut store = DataStore::empty();
    store.add_pak(&blue_pak.display().to_string(), BLUEBAY_KEY).unwrap();
    store.add_pak(&stadium_pak.display().to_string(), STADIUM_KEY).unwrap();
    let legacy = embedded::unzip(&fs::read(nadeo_zip).unwrap()).expect("read Nadeo item archive");
    // Every generated item is a mesh-modeler crystal written around this
    // archive crystal (the only item kind the game embeds AND scales); its
    // placement carries the scale, so the geometry stays authored-size.
    let template = legacy
        .get(CRYSTAL_TEMPLATE)
        .unwrap_or_else(|| panic!("{CRYSTAL_TEMPLATE} absent from Nadeo archive"))
        .clone();
    let mut files = BTreeMap::new();
    let mut faces_total = 0usize;
    for (prefab, alias) in &aliases {
        let ident = format!("{alias}.Item.Gbx");
        // Start, checkpoint and finish keep their race function only as the
        // archive's waypoint crystals; a generated crystal would be decor.
        if let Some(src) = waypoint_archive_item(prefab) {
            let bytes = legacy.get(src).unwrap_or_else(|| panic!("{src} absent from Nadeo archive"));
            files.insert(format!("Items/{ident}"), nameless_ident(bytes, &ident));
            continue;
        }
        let (item, faces) = crystal_from_model(&mut store, prefab, &template, &ident)
            .unwrap_or_else(|e| panic!("{prefab}: {e}"));
        faces_total += faces;
        files.insert(format!("Items/{ident}"), item);
    }
    // The map's own items: a crystal from each model's entity geometry, or a
    // report of why not. Vegetation is procedural (no mesh to copy); a
    // waypoint gate must keep its function, so it stays the game's own item.
    let mut item_models: Vec<String> = source.items.iter().map(|it| it.model.clone()).collect();
    item_models.sort();
    item_models.dedup();
    let mut item_aliases: BTreeMap<String, String> = BTreeMap::new();
    for (i, model) in item_models.iter().enumerate() {
        let n = source.items.iter().filter(|it| &it.model == model).count();
        let logical = find_item_file(&store, model)
            .unwrap_or_else(|| panic!("item model {model} has no .Item.Gbx in the client packs"));
        if source.items.iter().any(|it| &it.model == model && it.waypoint_tag.is_some()) {
            println!("  KEPT AS IS ({n} placements): {model} -- a waypoint item; a generated crystal would lose the trigger");
            continue;
        }
        let alias = format!("AC{:08}", 200 + i);
        let ident = format!("{alias}.Item.Gbx");
        match crystal_from_model(&mut store, &logical, &template, &ident) {
            Ok((item, faces)) => {
                faces_total += faces;
                files.insert(format!("Items/{ident}"), item);
                item_aliases.insert(model.clone(), alias);
            }
            Err(e) => println!("  NOT rescaled ({n} placements): {model} -- {e}"),
        }
    }

    for (src, alias) in [
        (LEGACY_PLATFORM, "AC00000101"),
        (LEGACY_SLOPE, "AC00000102"),
        (LEGACY_WALL, "AC00000103"),
    ] {
        let bytes = legacy
            .get(src)
            .unwrap_or_else(|| panic!("{src} absent from Nadeo archive"));
        files.insert(format!("Items/{alias}.Item.Gbx"), nameless_ident(bytes, &format!("{alias}.Item.Gbx")));
    }
    // The "empty" item (an invisible structural pillar): a single tiny
    // triangle well below the ground, so the placement exists and draws nothing.
    let _ = empty_template;
    let mut tiny = crate::crystal::CrystalMesh::default();
    tiny.add_tris(&[[0.0, -50.0, 0.0], [0.01, -50.0, 0.0], [0.0, -50.0, 0.01]], &[[0, 1, 2]], 0, 8.0);
    let mat = vec![crate::crystal::MaterialSpec { link: "Stadium\\Media\\Material\\RoadTech".into(), physics: 16 }];
    files.insert(
        "Items/AC00000104.Item.Gbx".into(),
        crate::crystal::build_item(&template, "AC00000104.Item.Gbx", "AC00000104.Item.Gbx", &mat, &tiny),
    );
    // Every item must claim the map's own collection inside (header and body
    // idents), or a BlueBay map drops it without a word.
    let collection = source.items.first().map(|it| it.collection_raw).unwrap_or(26);
    for bytes in files.values_mut() {
        *bytes = set_ident_collection(bytes, collection);
    }
    println!("  {} crystal items, {} faces, collection {:#x}", files.len(), faces_total, collection);

    let archive = zip(&files);
    fs::write(out_zip, &archive).unwrap();
    let mut mapping = String::new();
    for b in &source.blocks {
        let alias = fallback_alias(&b.name, b.flags)
            .unwrap_or_else(|| aliases[&catalog_map[&(b.name.clone(), b.flags)]].as_str());
        let (sx, sz) = footprint_map[&b.name];
        // Every item is authored at size 1; the placement carries the scale.
        mapping.push_str(&format!(
            "@{}\t{}.Item.Gbx\t1\t{}\t{}\n",
            b.index, alias, sx, sz
        ));
    }
    for it in &source.items {
        if let Some(alias) = item_aliases.get(&it.model) {
            mapping.push_str(&format!("i@{}\t{}.Item.Gbx\t1\n", it.index, alias));
        }
    }
    let mapping_path = out_zip.with_extension("placements.tsv");
    fs::write(&mapping_path, mapping).unwrap();
    let args = vec![
        "tmmaps".into(),
        "tiny".into(),
        map.display().to_string(),
        "--out".into(),
        out_map.display().to_string(),
        "--mapping".into(),
        mapping_path.display().to_string(),
        "--library".into(),
        out_zip.display().to_string(),
        "--scale".into(),
        scale.to_string(),
    ];
    tmmaps::tiny::cmd(&args);
    println!(
        "  generated {} block items + {} item conversions",
        aliases.len() + 4,
        item_aliases.len()
    );
    println!("  library: {} ({} bytes)", out_zip.display(), archive.len());
}
