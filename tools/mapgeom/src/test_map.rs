//! `mapgeom test-map --source MAP --out OUT.Map.Gbx --stack X=FILE.Item.Gbx[,X=FILE…] [--start CX,CZ --finish CX,CZ
//! --z CZ --height 3 --name NAME]` — Hugo's A/B map for the expandable checkpoint items (2026-10-27): a Stadium
//! source stripped of every authored block and item, then a RoadTechStart, one upright STACK of `height` items per
//! `--stack` entry (each item at its own cell's block frame: pos = cell corner, yaw 0, one per row from the grass
//! row up) and a RoadTechFinish beyond; the items embedded; lightmap stripped, password removed, unvalidated,
//! renamed, fresh uid. The stacks sit side by side across x so the driver can pick one.
use std::collections::HashSet;
use std::path::Path;
use tmmaps::map::{FreeBlockSpec, MapFile};

pub struct TestMapOpts {
    pub source: String,
    pub out: String,
    pub stacks: Vec<(i32, String)>,
    /// single placements: (file, cx, dy above the grass row, cz)
    pub items: Vec<(String, i32, i32, i32)>,
    /// Nadeo pak items by ident (not embedded), posed at the cell's road-deck centre: pos = corner + (16, 2, 16),
    /// yaw 0 (the gate items' origin = the arch foot centre on the deck; Fall 01/06/12: deck = cell floor + 2)
    pub native_items: Vec<(String, i32, i32, i32)>,
    /// extra grid blocks: (name, cx, dy above the grass row, cz, dir, additional variant index — flags bits 21–27)
    pub blocks: Vec<(String, i32, i32, i32, u8, u32)>,
    pub start: (i32, i32),
    pub finish: (i32, i32),
    /// rows above the grass row for the start / finish blocks
    pub course_dy: i32,
    pub z: i32,
    pub height: i32,
    pub name: String,
}

pub fn build(opts: &TestMapOpts) -> Result<Vec<String>, String> {
    let mut report = Vec::new();
    let hdr = tmmaps::header::read(&opts.source)?;
    let m0 = MapFile::try_load(Path::new(&opts.source))?;
    let collection = m0.body_collections().map(|c| c[0].1).unwrap_or(26);
    let ground = tmmaps::map::ground_y(collection);
    let grass_row = m0.blocks.iter().chain(m0.baked.iter()).filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none() && b.name.eq_ignore_ascii_case("Grass")).map(|b| b.coords().1).min().ok_or("source map: no Grass row")?;
    if m0.items.is_empty() {
        return Err("source map has no item to clone as the placement donor".into());
    }
    report.push(format!("source {}: collection {collection:#x} ground {ground} grass row {grass_row}, {} blocks, {} items", opts.source, m0.blocks.len(), m0.items.len()));
    let out = Path::new(&opts.out);
    // 1. every item but the first (the donor) goes
    {
        let mut m = MapFile::try_load(Path::new(&opts.source))?;
        m.remove_items(|it| it.index != 0);
        m.write_to(out).map_err(|e| e.to_string())?;
    }
    // 2. every authored block goes; the start and the finish come in
    let mut specs = vec![
        FreeBlockSpec { name: "RoadTechStart".into(), author: None, flags: 0, pos: [0.0; 3], rot: [0.0; 3], grid: Some([opts.start.0, grass_row + opts.course_dy, opts.start.1]), dir: 0, waypoint: Some("Spawn".into()) },
        FreeBlockSpec { name: "RoadTechFinish".into(), author: None, flags: 0, pos: [0.0; 3], rot: [0.0; 3], grid: Some([opts.finish.0, grass_row + opts.course_dy, opts.finish.1]), dir: 0, waypoint: Some("Goal".into()) },
    ];
    for (name, cx, dy, cz, dir, add) in &opts.blocks {
        let wp = if name.ends_with("Finish") { Some("Goal".to_string()) } else if name.ends_with("Checkpoint") { Some("Checkpoint".to_string()) } else { None };
        // a block on the grass row is a GROUND placement (FLAG_GROUND 0x1000): the editor's own rule, and what makes the
        // ground variant (and its terrain void) apply — the PlatformBase / DecoCliffMid render test of 10-31
        let flags = (if *dy == 0 { crate::blockmap::FLAG_GROUND } else { 0 }) | ((*add & 0x7F) << 21);
        specs.push(FreeBlockSpec { name: name.clone(), author: None, flags, pos: [0.0; 3], rot: [0.0; 3], grid: Some([*cx, grass_row + dy, *cz]), dir: *dir, waypoint: wp });
        report.push(format!("block {name} at cell ({cx}, {}, {cz}) dir {dir} flags {flags:#x}", grass_row + dy));
    }
    {
        let mut m = MapFile::try_load(out)?;
        m.remove_and_add_blocks(|b| b.flags != 0xFFFF_FFFF, |_| false, &specs);
        m.write_to(out).map_err(|e| e.to_string())?;
    }
    // the placement list: every stack level, then the single items
    let mut places: Vec<(String, i32, i32, i32)> = Vec::new();
    for (cx, file) in &opts.stacks {
        for k in 0..opts.height {
            places.push((file.clone(), *cx, grass_row + k, opts.z));
        }
    }
    for (file, cx, dy, cz) in &opts.items {
        places.push((file.clone(), *cx, grass_row + dy, *cz));
    }
    let n_embedded = places.len();
    for (ident, cx, dy, cz) in &opts.native_items {
        places.push((ident.clone(), *cx, grass_row + dy, *cz));
    }
    // 3. the items embedded (merged with whatever the source embeds, in order)
    let mut idents: Vec<String> = Vec::new();
    {
        let mut m = MapFile::try_load(out)?;
        let (old_rows, old_zip) = m.embedded_manifest();
        let mut added: Vec<(String, Vec<u8>)> = Vec::new();
        let mut manifest = old_rows.clone();
        let mut seen: HashSet<String> = HashSet::new();
        for (pi, (file, _, _, _)) in places.iter().enumerate() {
            if pi >= n_embedded {
                idents.push(file.clone());
                continue;
            }
            let ident = Path::new(file).file_name().map(|s| s.to_string_lossy().to_string()).ok_or_else(|| format!("{file}: no file name"))?;
            idents.push(ident.clone());
            if !seen.insert(ident.clone()) {
                continue;
            }
            let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
            let b = crate::tiny_assets::set_ident_collection(&bytes, collection);
            added.push((format!("Items/{ident}"), b));
            manifest.push((ident.clone(), collection, ident.clone()));
        }
        let zip = tmmaps::header::append_stored_zip(&old_zip, &added);
        let refs: Vec<(&str, u32, &str)> = manifest.iter().map(|(a, c, b)| (a.as_str(), *c, b.as_str())).collect();
        m.replace_embedded_objects_rows(&refs, &zip);
        m.write_to(out).map_err(|e| e.to_string())?;
        report.push(format!("embedded {} item file(s) ({} kept from the source)", added.len(), old_rows.len()));
    }
    // 4. the placements: one clone per entry, posed at its cell's block frame (dir 0: the cell corner)
    let total = places.len();
    {
        let mut m = MapFile::try_load(out)?;
        m.append_item_clones(total);
        m.write_to(out).map_err(|e| e.to_string())?;
    }
    {
        let mut m = MapFile::try_load(out)?;
        for (i, _) in places.iter().enumerate() {
            m.set_item_model(i, &idents[i]);
            m.set_item_author(i, if i >= n_embedded { "Nadeo" } else { &idents[i] });
            m.set_item_collection(i, collection);
        }
        m.write_to(out).map_err(|e| e.to_string())?;
    }
    {
        let mut m = MapFile::try_load(out)?;
        for (i, (_, cx, cy, cz)) in places.iter().enumerate() {
            let native = i >= n_embedded;
            let off = if native { [16.0, 2.0, 16.0] } else { [0.0; 3] };
            let pos = [*cx as f32 * 32.0 + off[0], *cy as f32 * 8.0 + ground + off[1], *cz as f32 * 32.0 + off[2]];
            let it = m.items[i].clone();
            if it.skin_region.is_some() || it.flags & 4 != 0 {
                m.set_item_skin(i, None);
            }
            m.move_item_pos(i, pos);
            m.set_item_rotation(i, 0.0, 0.0, 0.0);
            m.set_item_pivot(i, [0.0, 0.0, 0.0]);
            m.set_item_flags(i, 0);
            m.set_item_scale(i, 1.0);
            m.set_item_cell(i, [(*cx).clamp(0, 255) as u8, (*cy).clamp(0, 255) as u8, (*cz).clamp(0, 255) as u8]);
            report.push(format!("{}: cell ({cx}, {cy}, {cz}) pos ({:.1}, {:.1}, {:.1}) yaw 0", idents[i], pos[0], pos[1], pos[2]));
        }
        m.write_to(out).map_err(|e| e.to_string())?;
    }
    // 5. lightmap, password, validation, name, uid
    {
        let mut m = MapFile::try_load(out)?;
        let stripped = m.strip_lightmap();
        m.strip_validation_ghost_to(tmmaps::map::GhostForm::Remove);
        m.remove_password();
        let (h, b) = m.set_map_name(&hdr.name, &opts.name);
        m.set_unvalidated(&hdr);
        m.write_to(out).map_err(|e| e.to_string())?;
        report.push(format!("lightmap stripped ({stripped}), password removed, unvalidated, name {:?} -> {:?} ({h} header + {b} body)", hdr.name, opts.name));
    }
    {
        let mut m = MapFile::try_load(out)?;
        let old_uid = hdr.uid.clone();
        // a prefix derived from the map name and the build time, so no two builds share a uid (the game caches by uid)
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as u32).unwrap_or(0);
        let h = opts.name.bytes().fold(5381u32 ^ seed, |h, b| h.wrapping_mul(33) ^ b as u32);
        let prefix: String = (0..4).map(|k| "ABCDEFGHJKLMNPQRSTUVWXYZ".as_bytes()[((h >> (k * 5)) % 24) as usize] as char).collect();
        let new_uid = if old_uid.len() > 4 { format!("{prefix}{}", &old_uid[..old_uid.len() - 4]) } else { old_uid.clone() };
        m.set_map_uid_any_len(&new_uid);
        m.write_to(out).map_err(|e| e.to_string())?;
        report.push(format!("uid {old_uid} -> {new_uid}"));
    }
    let m = MapFile::try_load(out)?;
    report.push(format!("result: {} authored blocks, {} items, {} bytes", m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF).count(), m.items.len(), std::fs::metadata(out).map(|x| x.len()).unwrap_or(0)));
    Ok(report)
}
