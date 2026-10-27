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
    pub start: (i32, i32),
    pub finish: (i32, i32),
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
    let specs = vec![
        FreeBlockSpec { name: "RoadTechStart".into(), author: None, flags: 0, pos: [0.0; 3], rot: [0.0; 3], grid: Some([opts.start.0, grass_row, opts.start.1]), dir: 0, waypoint: Some("Spawn".into()) },
        FreeBlockSpec { name: "RoadTechFinish".into(), author: None, flags: 0, pos: [0.0; 3], rot: [0.0; 3], grid: Some([opts.finish.0, grass_row, opts.finish.1]), dir: 0, waypoint: Some("Goal".into()) },
    ];
    {
        let mut m = MapFile::try_load(out)?;
        m.remove_and_add_blocks(|b| b.flags != 0xFFFF_FFFF, |_| false, &specs);
        m.write_to(out).map_err(|e| e.to_string())?;
    }
    // 3. the items embedded (merged with whatever the source embeds, in order)
    let mut idents: Vec<String> = Vec::new();
    {
        let mut m = MapFile::try_load(out)?;
        let (old_rows, old_zip) = m.embedded_manifest();
        let mut added: Vec<(String, Vec<u8>)> = Vec::new();
        let mut manifest = old_rows.clone();
        let mut seen: HashSet<String> = HashSet::new();
        for (_, file) in &opts.stacks {
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
    // 4. the placements: one clone per stack level, posed at the cell's block frame (dir 0: the cell corner)
    let total = opts.stacks.len() * opts.height as usize;
    {
        let mut m = MapFile::try_load(out)?;
        m.append_item_clones(total);
        m.write_to(out).map_err(|e| e.to_string())?;
    }
    {
        let mut m = MapFile::try_load(out)?;
        for (s, (_, _)) in opts.stacks.iter().enumerate() {
            for k in 0..opts.height as usize {
                let i = s * opts.height as usize + k;
                m.set_item_model(i, &idents[s]);
                m.set_item_author(i, &idents[s]);
                m.set_item_collection(i, collection);
            }
        }
        m.write_to(out).map_err(|e| e.to_string())?;
    }
    {
        let mut m = MapFile::try_load(out)?;
        for (s, (cx, _)) in opts.stacks.iter().enumerate() {
            for k in 0..opts.height {
                let i = s * opts.height as usize + k as usize;
                let cy = grass_row + k;
                let pos = [*cx as f32 * 32.0, cy as f32 * 8.0 + ground, opts.z as f32 * 32.0];
                let it = m.items[i].clone();
                if it.skin_region.is_some() || it.flags & 4 != 0 {
                    m.set_item_skin(i, None);
                }
                m.move_item_pos(i, pos);
                m.set_item_rotation(i, 0.0, 0.0, 0.0);
                m.set_item_pivot(i, [0.0, 0.0, 0.0]);
                m.set_item_flags(i, 0);
                m.set_item_scale(i, 1.0);
                m.set_item_cell(i, [(*cx).clamp(0, 255) as u8, cy.clamp(0, 255) as u8, opts.z.clamp(0, 255) as u8]);
                report.push(format!("{}: cell ({cx}, {cy}, {}) pos ({:.1}, {:.1}, {:.1}) yaw 0", idents[s], opts.z, pos[0], pos[1], pos[2]));
            }
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
        let new_uid = if old_uid.len() > 4 { format!("XPND{}", &old_uid[..old_uid.len() - 4]) } else { old_uid.clone() };
        m.set_map_uid_any_len(&new_uid);
        m.write_to(out).map_err(|e| e.to_string())?;
        report.push(format!("uid {old_uid} -> {new_uid}"));
    }
    let m = MapFile::try_load(out)?;
    report.push(format!("result: {} authored blocks, {} items, {} bytes", m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF).count(), m.items.len(), std::fs::metadata(out).map(|x| x.len()).unwrap_or(0)));
    Ok(report)
}
