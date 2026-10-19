//! Hugo's vista terrain ITEMS (2026-10-19): a map's terrain-integrated variant BLOCKS
//! (`<Base>On<Terrain><Cliff|Hill>N…`) replaced by whole-variant items — deck + cliff/hill + the
//! collection's terrain materials in ONE `.Item.Gbx` each (`tiny_library::bake_named_block` at scale 1),
//! embedded in the map, placed in the block's own frame: position = `place::grid_block(cell, dir,
//! (W, D), ground_y)`, yaw = −dir·π/2, pivot 0 — the same rule as the FinishStand carriers and
//! Hugo's C# placement. The block records go; the terrain TILE under the variant stays (it is a
//! separate record). Report rows per block.
use std::collections::BTreeMap;
use std::path::Path;

use tmmaps::map::MapFile;

use crate::store::DataStore;

pub struct Replaced {
    pub block: String,
    pub cell: (i32, i32, i32),
    pub dir: u8,
    pub ident: String,
    pub pos: [f32; 3],
    pub yaw: f32,
}

pub struct Outcome {
    pub items: Vec<(String, Vec<u8>)>,
    pub replaced: Vec<Replaced>,
    pub notes: Vec<String>,
}

fn is_vista_variant(n: &str) -> bool {
    ["OnDirt", "OnGrass", "OnLand", "OnWater"].iter().any(|k| n.contains(k)) && (n.contains("Cliff") || n.contains("Hill"))
}

/// `src` → `out`: every vista variant block (or only `only`, when given) of the map as an embedded item.
pub fn vista_items_map(store: &mut DataStore, src: &Path, out: &Path, only: &[String], author: &str, items_dir: Option<&Path>) -> Result<Outcome, String> {
    let m = MapFile::try_load(src)?;
    let collection = m.body_collections().map(|c| c[0].1).unwrap_or(26);
    let ground = tmmaps::map::ground_y(collection);
    let mut notes = Vec::new();
    // the blocks to replace
    let targets: Vec<(usize, String, (i32, i32, i32), u8, u32)> = m
        .blocks
        .iter()
        .filter(|b| b.flags != 0xFFFF_FFFF)
        .filter(|b| if only.is_empty() { is_vista_variant(&b.name) } else { only.iter().any(|o| o.eq_ignore_ascii_case(&b.name)) })
        .map(|b| (b.index, b.name.clone(), b.coords(), b.dir, b.flags))
        .collect();
    if targets.is_empty() {
        return Err("no vista variant block in the map (or none of --only)".into());
    }
    // one item per (name, flags without the dir-independent bits): the bake depends on ground/variant bits
    let mut items: BTreeMap<String, (String, Vec<u8>, [i32; 3])> = BTreeMap::new();
    for (_, name, _, _, flags) in &targets {
        let key = format!("{name}@{:08X}", flags & !0u32);
        if items.contains_key(&key) {
            continue;
        }
        let ident = format!("{name}.Item.Gbx");
        let (bytes, mm, srcnote) = crate::tiny_library::bake_named_block(store, collection, name, *flags, &ident, author, 1.0)?;
        // footprint in cells from the baked plan's units (the Merged keeps none; read the block info again)
        let size = block_size(store, collection, name).unwrap_or([1, 1, 1]);
        notes.push(format!("{name}: {} visuals, {} materials, {} collision tris; {srcnote}; size {:?}", mm.visuals.len(), mm.materials.len(), mm.surf_triangles.len(), size));
        if let Some(d) = items_dir {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            std::fs::write(d.join(&ident), &bytes).map_err(|e| e.to_string())?;
        }
        items.insert(key, (ident, bytes, size));
    }
    // pass 1: remove the blocks
    let drop: Vec<usize> = targets.iter().map(|t| t.0).collect();
    {
        let mut mm = MapFile::try_load(src)?;
        mm.remove_blocks(|b| drop.contains(&b.index), |_| false);
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 2: embed (merged with what the map already embeds, base archive byte-for-byte)
    {
        let mut mm = MapFile::try_load(out)?;
        let (old_rows, old_zip) = mm.embedded_manifest();
        let mut added: Vec<(String, Vec<u8>)> = Vec::new();
        let mut manifest: Vec<(String, u32, String)> = old_rows.clone();
        for (ident, bytes, _) in items.values() {
            if manifest.iter().any(|(a, _, _)| a == ident) {
                continue;
            }
            let b = crate::tiny_assets::set_ident_collection(bytes, collection);
            added.push((format!("Items/{ident}"), b));
            manifest.push((ident.clone(), collection, author.to_string()));
        }
        let zip = tmmaps::header::append_stored_zip(&old_zip, &added);
        let refs: Vec<(&str, u32, &str)> = manifest.iter().map(|(a, c, b)| (a.as_str(), *c, b.as_str())).collect();
        mm.replace_embedded_objects_rows(&refs, &zip);
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    // pass 3: one placement per removed block
    let base_items = MapFile::try_load(out)?.items.len();
    {
        let mut mm = MapFile::try_load(out)?;
        mm.append_item_clones(base_items + targets.len());
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    let mut replaced = Vec::new();
    {
        let mut mm = MapFile::try_load(out)?;
        for (k, (_, name, cell, dir, flags)) in targets.iter().enumerate() {
            let i = base_items + k;
            let key = format!("{name}@{:08X}", flags & !0u32);
            let (ident, _, size) = &items[&key];
            mm.set_item_model(i, ident);
            mm.set_item_author(i, author);
            mm.set_item_collection(i, collection);
        }
        mm.write_to(out).map_err(|e| e.to_string())?;
        let mut mm = MapFile::try_load(out)?;
        for (k, (_, name, cell, dir, flags)) in targets.iter().enumerate() {
            let i = base_items + k;
            let key = format!("{name}@{:08X}", flags & !0u32);
            let (ident, _, size) = &items[&key];
            let x = crate::place::grid_block(*cell, *dir, (32.0 * size[0] as f32, 32.0 * size[2] as f32), ground);
            let pos = [x[9], x[10], x[11]];
            let yaw = -(*dir as f32) * std::f32::consts::FRAC_PI_2;
            let it = mm.items[i].clone();
            if it.skin_region.is_some() || it.flags & 4 != 0 {
                mm.set_item_skin(i, None);
            }
            mm.set_item_waypoint(i, None, 0);
            mm.move_item_pos(i, pos);
            mm.set_item_rotation(i, yaw, 0.0, 0.0);
            mm.set_item_pivot(i, [0.0, 0.0, 0.0]);
            mm.set_item_flags(i, 0);
            mm.set_item_scale(i, 1.0);
            let c = [(cell.0.clamp(0, 255)) as u8, (cell.1.clamp(0, 255)) as u8, (cell.2.clamp(0, 255)) as u8];
            mm.set_item_cell(i, c);
            replaced.push(Replaced { block: name.clone(), cell: *cell, dir: *dir, ident: ident.clone(), pos, yaw });
        }
        mm.write_to(out).map_err(|e| e.to_string())?;
    }
    // verify: every placement landed
    {
        let mm = MapFile::try_load(out)?;
        for (k, r) in replaced.iter().enumerate() {
            let it = &mm.items[base_items + k];
            if it.model != r.ident || (it.pos[0] - r.pos[0]).abs() > 0.01 || (it.pos[1] - r.pos[1]).abs() > 0.01 || (it.pos[2] - r.pos[2]).abs() > 0.01 {
                return Err(format!("placement {k} did not land: {} at {:?} (wanted {} at {:?})", it.model, it.pos, r.ident, r.pos));
            }
        }
    }
    Ok(Outcome { items: items.into_values().map(|(i, b, _)| (i, b)).collect(), replaced, notes })
}

fn block_size(store: &mut DataStore, collection: u32, name: &str) -> Option<[i32; 3]> {
    let path = crate::vista::find_block_info(store, name)?;
    let bi = crate::blockinfo::load(store, &path).ok()?;
    let v = bi.variant_base_ground.as_ref().filter(|v| !v.block_units.is_empty()).or(bi.variant_base_air.as_ref())?;
    let mut size = [0i32; 3];
    for u in &v.block_units {
        for k in 0..3 {
            size[k] = size[k].max(u.offset[k] + 1);
        }
    }
    let _ = collection;
    Some(size)
}
