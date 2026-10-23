//! Hugo's Floor Finish v2 (2026-10-23): ONE finish item per vista map whose trigger is the union of all
//! open-water cells as thin slabs lying on the lake BED, in WORLD coordinates (the item is placed at the map
//! origin, yaw 0). One item instead of thousands: the game's finish indicator lags with thousands of finishes.
//!
//! Bed = the water tile's collision floor under the water: the zone prefabs put it at local +2.00 over the
//! cell floor (RedIsland `Water` / `WaterHill` WetDirtRoad quad, WhiteShore `Water` Sand, GreenCoast `Lake`
//! Grass) and +4.00 for BlueBay `Sea` (Sand) — measured 2026-10-23 (`surfhist` over the prefabs' collision).
//! The WaterHill rim climbs from 2.0 to 7.4 at the hill side, so its slab sits on the deep part's floor and
//! runs INTO the rim: the slab is still crossed by any car reaching the bed there.
//!
//! Slab thickness: bed −0.1 .. bed +0.4 (0.5 m). The physics step is 10 ms; a car sinking or diving at up to
//! ~30 m/s moves 0.3 m per step, so a slab thinner than ~0.3 m could be tunnelled between two steps; 0.5 m
//! leaves margin, and the car's contact with the floor collision (which IS at the bed) holds it inside the
//! slab's upper 0.4 m once it has settled — a car resting on the lake bed is inside the trigger.
//!
//! Mesh: water cells merged into ROW STRIPS (consecutive cells along x at one (cy, cz)) — one closed box
//! (8 vertices, 12 triangles) per strip; Fall 01's 3 212 cells become ~300 strips.
use std::collections::BTreeMap;

use tmmaps::map::MapFile;

/// The open-water tiles and the bed height above the cell floor, per collection.
pub fn water_tiles(collection: u32) -> (&'static [&'static str], f32) {
    match collection {
        0x10 => (&["Water", "WaterHill"], 2.0),
        0x1d => (&["Water"], 2.0),
        0x1c => (&["Sea"], 4.0),
        0xf => (&["Lake"], 2.0),
        _ => (&[], 0.0),
    }
}

pub struct FloorMesh {
    pub verts: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
    pub cells: usize,
    pub strips: usize,
    pub bbox: ([f32; 3], [f32; 3]),
}

/// The world-frame slab mesh of a map's open-water cells. `below`/`above`: the slab's span under / over the bed.
pub fn floor_mesh(m: &MapFile, collection: u32, below: f32, above: f32) -> Result<FloorMesh, String> {
    let (tiles, bed) = water_tiles(collection);
    if tiles.is_empty() {
        return Err(format!("collection {collection:#x}: no open-water tile table (vista collections only)"));
    }
    let ground = tmmaps::map::ground_y(collection);
    // (cy, cz) -> sorted cx list
    let mut rows: BTreeMap<(i32, i32), Vec<i32>> = BTreeMap::new();
    let mut cells = 0usize;
    for b in m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none()) {
        if !tiles.iter().any(|t| t.eq_ignore_ascii_case(&b.name)) {
            continue;
        }
        let c = b.coords();
        rows.entry((c.1, c.2)).or_default().push(c.0);
        cells += 1;
    }
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let quads: [[u32; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [1, 2, 6, 5], [0, 4, 7, 3]];
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    let mut strips = 0usize;
    for ((cy, cz), mut xs) in rows {
        xs.sort_unstable();
        xs.dedup();
        let y_bed = ground + 8.0 * cy as f32 + bed;
        let (y0, y1) = (y_bed - below, y_bed + above);
        let (z0, z1) = (32.0 * cz as f32, 32.0 * (cz + 1) as f32);
        let mut i = 0;
        while i < xs.len() {
            let start = xs[i];
            let mut end = start;
            while i + 1 < xs.len() && xs[i + 1] == end + 1 {
                i += 1;
                end = xs[i];
            }
            i += 1;
            let (x0, x1) = (32.0 * start as f32, 32.0 * (end + 1) as f32);
            let base = verts.len() as u32;
            verts.extend_from_slice(&[[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]);
            for q in quads {
                tris.push([base + q[0], base + q[1], base + q[2]]);
                tris.push([base + q[0], base + q[2], base + q[3]]);
            }
            for v in &verts[base as usize..] {
                for k in 0..3 {
                    lo[k] = lo[k].min(v[k]);
                    hi[k] = hi[k].max(v[k]);
                }
            }
            strips += 1;
        }
    }
    if cells == 0 {
        return Err("no open-water tile in the map".into());
    }
    Ok(FloorMesh { verts, tris, cells, strips, bbox: (lo, hi) })
}

/// The one-per-map carrier: an invisible Finish item (speck visual, no collision) whose trigger is the
/// floor mesh; ident = author = `<ident>`; collection = the map's. Returns (bytes, mesh).
pub fn build_floor_item(store: &mut crate::store::DataStore, m: &MapFile, collection: u32, ident: &str, below: f32, above: f32, lm_word: Option<u64>) -> Result<(Vec<u8>, FloorMesh), String> {
    let mesh = floor_mesh(m, collection, below, above)?;
    let donor = "Stadium\\Items\\LightCube2m.Item.Gbx";
    let mut mg = crate::static_item::build::pack_item_merged(store, donor, 1.0, collection, 0, None).map_err(|e| format!("donor {donor}: {e}"))?;
    crate::static_item::build::make_invisible_mesh_waypoint(&mut mg, crate::sttc::WP_FINISH, mesh.verts.clone(), mesh.tris.clone())?;
    if let Some(w) = lm_word {
        mg.file_write_time = w;
    }
    let (bytes, _mg) = crate::static_item::build::finish_item(mg, ident, ident, 1.0, collection)?;
    Ok((bytes, mesh))
}
