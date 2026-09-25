//! THE GAME'S CHART LAYOUT for a map, transcribed end to end (REPORT-5 §4-E.1/§4-E.2; `lmtool packtest` is the oracle —
//! eleven of eleven BlueBay editor bakes reproduce every chart in position and size):
//!
//! 1. the chart list = the zone tiles (one per cell of the 64×64 grid; object id k = the map file's k-th BAKED block
//!    record in file order, the uncovered cells after them in x-major order) then the items (object base + item index);
//!    an item whose Solid2 carries LIGHTS, or whose materials are all a RaceTriggerFX class, is packed but not bound
//!    (`Charted::Packed`) — tiny 16's holes; the others are `Charted::Bound`;
//! 2. the extents: a tile's = uvExt × (MeterByUv × q) from the zone tile solid's PreLightGen with q = its QUALITY
//!    (`tile_quality`: 1.0 on an item's cell, (√2)^−r for the first ring r = 1…8 holding an item cell at the tile's
//!    own level, (√2)^−9 otherwise — 0x140dcc290 → FUN_140dcc8e0); an item's = uvExt × (MeterByUv × ((√2)^e · scale))
//!    from its PreLightGen and its MapElemLightmapQuality e;
//! 3. the order: ascending (area, centre z, centre y, centre x, |h|², record index) over the block records (the stored
//!    visual boxes through the placement Iso4 — `lmtiles`; the cell-centre stand-in without a pak), floats compared
//!    sign-aware, placed from the end;
//! 4. `pack::allocate_ordered` — D = W·H / Σarea (an f32 running sum), the scale search with the bake quality's
//!    iteration count {1, 3, 6, 8, 10, 10}, TryPack with a = (ext.x·s)·(ext.y·s), the floor fit, the carry, the
//!    binary-tree packer with its 4·N node capacity.
//!
//! Layout units are the 2048 grid (`pos = packer + pad`, `size = packer − 2·pad`); the stored texel of a chart at layout
//! (X, Y, W, H) is ((X + 1)/2, (Y + 1)/2) with W/2 × H/2 texels.

use crate::pack::{ChartExt, Placed};

/// Whether a chart is bound into the visual mapping or only takes its place in the atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charted {
    Bound,
    /// Packed by TryPack, absent from the mapping (an item with lights / the FX material class).
    Packed,
}

#[derive(Clone, Debug)]
pub struct LayoutChart {
    /// The object id (tile k < base, item base + index).
    pub obj: u32,
    pub ext: [f32; 2],
    pub charted: Charted,
    /// Layout units (2048 grid): the mapping's pos/size.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Debug)]
pub struct GameLayout {
    pub charts: Vec<LayoutChart>,
    /// The allocated texel density (layout units per metre).
    pub s: f32,
    pub sum_area: f32,
    pub w_atlas: u16,
    /// (g, pad, m) of the layout side.
    pub params: (u16, u16, u16),
    pub max_iter: u32,
    /// Per object id (tiles): the cell.
    pub cell_of: Vec<(i32, i32)>,
    pub tile_quality: Vec<f32>,
}

/// The zone tile's PreLightGen constants: MeterByUv and the uv bounds (u0, v0, u1, v1). The BlueBay `Zone\Sea\Base.Prefab.Gbx`
/// entity 0 by default (`mapgeom zone-tile-plg`).
#[derive(Clone, Copy, Debug)]
pub struct TilePlg {
    pub meter_by_uv: f32,
    pub bounds: [f32; 4],
}

impl TilePlg {
    pub const BLUEBAY_SEA: TilePlg = TilePlg { meter_by_uv: f32::from_bits(0x420dc57e), bounds: [f32::from_bits(0x3d448f40), f32::from_bits(0x3d58373f), f32::from_bits(0x3f7272d8), f32::from_bits(0x3f759e83)] };

    /// ext = uvExt × (MeterByUv × q) — FUN_1402917e0 / FUN_14028f480: f = MeterByUv × the record's scale, then the uv extents × f.
    pub fn ext(&self, q: f32) -> [f32; 2] {
        let f = self.meter_by_uv * q;
        [(self.bounds[2] - self.bounds[0]) * f, (self.bounds[3] - self.bounds[1]) * f]
    }
}

/// The far tiles' quality: (√2)^−9 = powf(0.5, 4.5) = f32 0x3d3504f3.
pub const Q_FAR: f32 = f32::from_bits(0x3d3504f3);

/// The scale search's iteration count per bake quality index (BlockSplit l.299–316).
pub fn max_iter_for_quality(q: u32) -> u32 {
    match q {
        0 => 1,
        1 => 3,
        2 => 6,
        3 => 8,
        _ => 10,
    }
}

/// The tile object → cell map: the file's BAKED records in order, then the uncovered cells x-major.
pub fn tile_cells(baked: &[(i32, i32)], sx: i32, sz: i32) -> Vec<(i32, i32)> {
    let mut covered = vec![false; (sx * sz) as usize];
    let mut cells: Vec<(i32, i32)> = Vec::with_capacity((sx * sz) as usize);
    for &(cx, cz) in baked {
        if cx >= 0 && cx < sx && cz >= 0 && cz < sz {
            covered[(cx * sz + cz) as usize] = true;
        }
        cells.push((cx, cz));
    }
    for cx in 0..sx {
        for cz in 0..sz {
            if !covered[(cx * sz + cz) as usize] {
                cells.push((cx, cz));
            }
        }
    }
    cells
}

/// The generated tile's quality per cell (FUN_140dcc8e0's ring search over the item-marked 3-D grid).
pub fn tile_quality(cells: &[(i32, i32)], tile_y: i32, item_cells: &std::collections::HashSet<(i32, i32, i32)>) -> Vec<f32> {
    cells
        .iter()
        .map(|&(cx, cz)| {
            if item_cells.contains(&(cx, tile_y, cz)) {
                return 1.0;
            }
            for r in 1..=8i32 {
                for dx in -r..=r {
                    for dz in -r..=r {
                        if dx.abs() != r && dz.abs() != r {
                            continue;
                        }
                        if item_cells.contains(&(cx + dx, tile_y, cz + dz)) {
                            return (0.5f32).powf(r as f32 * 0.5);
                        }
                    }
                }
            }
            Q_FAR
        })
        .collect()
}

/// An item's quality factor from its MapElemLightmapQuality byte (FUN_140dcc1c0: (√2)^e, e = {0: 0, 1: +1, 2: +2, 3: +3,
/// 4: −1, 5: −2, 6: −3, other: 0}).
pub fn item_quality(elem: u8) -> f32 {
    let e: i32 = match elem {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => -1,
        5 => -2,
        6 => -3,
        _ => 0,
    };
    (0.5f32).powf(-(e as f32) * 0.5)
}

/// Whether an item's model is bound into the visual mapping (tiny 16's oracle: no lights, not the RaceTriggerFX class).
pub fn item_bound(m: &crate::geometry::ModelGeom) -> bool {
    let fx_only = !m.mat_links.is_empty() && m.mat_links.iter().all(|l| l.contains("RaceTriggerFX"));
    m.lights.is_empty() && !fx_only
}

/// One chart's radix key: (area, centre z, y, x, |h|²).
#[derive(Clone, Copy, Debug)]
pub struct ChartKey {
    pub centre: [f32; 3],
    pub h2: f32,
}

/// The sign-aware float order of the radix sorter (−0.0 sorts below +0.0).
pub fn fcmp(x: f32, y: f32) -> std::cmp::Ordering {
    let k = |v: f32| -> u32 { if v.is_sign_negative() { !v.to_bits() } else { v.to_bits() | 0x8000_0000 } };
    k(x).cmp(&k(y))
}

/// The walk order: ascending (area, z, y, x, |h|², index) — TryPack places from the end.
pub fn walk_order(charts: &[ChartExt], keys: &[ChartKey]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..charts.len()).collect();
    idx.sort_by(|&p, &q| {
        let (ap, aq) = (charts[p].ext[0] * charts[p].ext[1], charts[q].ext[0] * charts[q].ext[1]);
        fcmp(ap, aq).then(fcmp(keys[p].centre[2], keys[q].centre[2])).then(fcmp(keys[p].centre[1], keys[q].centre[1])).then(fcmp(keys[p].centre[0], keys[q].centre[0])).then(fcmp(keys[p].h2, keys[q].h2)).then(p.cmp(&q))
    });
    idx
}

/// The inputs of one allocation.
pub struct LayoutInput {
    /// Tiles: (obj, ext, key) — obj = 0..n_tiles in chart-array order.
    pub tiles: Vec<([f32; 2], ChartKey)>,
    /// Items: (obj, ext, key, bound).
    pub items: Vec<(u32, [f32; 2], ChartKey, Charted)>,
    pub w_atlas: u16,
    pub quality_index: u32,
}

/// Run the allocation: the chart list in record order (tiles, then items), the walk order, the scale search.
pub fn allocate(input: &LayoutInput) -> Result<GameLayout, String> {
    let (g, pad, m) = crate::pack::layout_params(input.w_atlas, input.w_atlas);
    let max_iter = max_iter_for_quality(input.quality_index);
    let mut charts: Vec<ChartExt> = Vec::new();
    let mut keys: Vec<ChartKey> = Vec::new();
    let mut objs: Vec<(u32, Charted)> = Vec::new();
    for (i, (ext, key)) in input.tiles.iter().enumerate() {
        charts.push(ChartExt { ext: *ext, mins: [1, 1] });
        keys.push(*key);
        objs.push((i as u32, Charted::Bound));
    }
    for (obj, ext, key, ch) in &input.items {
        charts.push(ChartExt { ext: *ext, mins: [1, 1] });
        keys.push(*key);
        objs.push((*obj, *ch));
    }
    let order = walk_order(&charts, &keys);
    let (s, placed): (f32, Vec<Placed>) = crate::pack::allocate_ordered(&charts, &order, input.w_atlas, input.w_atlas, g, m, max_iter).ok_or("the allocation failed (no scale packs)")?;
    let sum_area = charts.iter().fold(0f32, |acc, c| acc + c.ext[0] * c.ext[1]);
    let out: Vec<LayoutChart> = placed
        .iter()
        .enumerate()
        .map(|(k, p)| LayoutChart { obj: objs[k].0, ext: charts[k].ext, charted: objs[k].1, x: p.x as i32 + pad as i32, y: p.y as i32 + pad as i32, w: p.w as i32 - 2 * pad as i32, h: p.h as i32 - 2 * pad as i32 })
        .collect();
    Ok(GameLayout { charts: out, s, sum_area, w_atlas: input.w_atlas, params: (g, pad, m), max_iter, cell_of: Vec::new(), tile_quality: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tile_extent_is_the_pwc_day_one() {
        // the far tiles of pwc-day: q = 0.5^4.5 → (1.4082849, 1.4201677) m, area 2.0000007 (REPORT-5 §4-E.1)
        let e = TilePlg::BLUEBAY_SEA.ext(Q_FAR);
        assert_eq!(e[0].to_bits(), 0x3fb442ae, "{e:?}");
        assert_eq!(e[1].to_bits(), 0x3fb5c80e, "{e:?}");
        assert!((e[0] * e[1] - 2.0).abs() < 2e-6);
    }

    #[test]
    fn the_ring_quality_ladder() {
        // one item at cell (10, 5, 10): its own cell 1.0, ring r (√2)^−r, far (√2)^−9
        let mut items = std::collections::HashSet::new();
        items.insert((10, 5, 10));
        let cells: Vec<(i32, i32)> = vec![(10, 10), (11, 10), (12, 12), (18, 10), (19, 10), (0, 0)];
        let q = tile_quality(&cells, 5, &items);
        assert_eq!(q[0], 1.0);
        assert!((q[1] - 0.70710677).abs() < 1e-6);
        assert!((q[2] - 0.5).abs() < 1e-6);
        assert!((q[3] - 0.0625).abs() < 1e-6, "{}", q[3]);
        assert_eq!(q[4], Q_FAR);
        assert_eq!(q[5], Q_FAR);
        // another level does not count
        let mut other = std::collections::HashSet::new();
        other.insert((10, 8, 10));
        assert_eq!(tile_quality(&cells, 5, &other)[0], Q_FAR);
    }

    #[test]
    fn tile_cells_are_the_baked_records_then_x_major() {
        let cells = tile_cells(&[(1, 1), (0, 0)], 2, 2);
        assert_eq!(cells, vec![(1, 1), (0, 0), (0, 1), (1, 0)]);
    }

    #[test]
    fn the_walk_order_is_area_then_z_desc_from_the_end() {
        let charts = vec![ChartExt { ext: [1.0, 1.0], mins: [1, 1] }, ChartExt { ext: [1.0, 1.0], mins: [1, 1] }, ChartExt { ext: [2.0, 2.0], mins: [1, 1] }];
        let keys = vec![ChartKey { centre: [0.0, 0.0, 368.0], h2: 1.0 }, ChartKey { centre: [0.0, 0.0, 336.0], h2: 1.0 }, ChartKey { centre: [0.0, 0.0, 0.0], h2: 1.0 }];
        let o = walk_order(&charts, &keys);
        // ascending: the small ones (z 336 then 368), then the big one; placed from the end → big, z 368, z 336
        assert_eq!(o, vec![1, 0, 2]);
        assert_eq!(fcmp(-0.0, 0.0), std::cmp::Ordering::Less);
    }
}

/// The bake quality index of a baked map (cache chunk 0x0602200F = (q, 0)), None when the map has no lightmap.
pub fn quality_index_of(own: &crate::mapio::MapLightmap) -> Option<u32> {
    let d = own.chunk.data.as_ref()?;
    for c in &d.cache.chunks {
        if c.id == 0x0602_200F {
            if let crate::format::ChunkBody::Raw(b) = &c.body {
                if b.len() >= 4 {
                    return Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                }
            }
        }
    }
    None
}

/// The whole allocation of a map: the tiles from its baked records + the 64×64 grid, the items from the scene, the keys
/// from the block records when a pak is given (`lmtiles`), else the cell/triangle centres. `quality_index` = the game's
/// enum (tinyctl quality − 1); `base` = the tile count (the item object base).
pub fn for_map(map_path: &str, scene: &crate::geometry::Scene, base: u32, quality_index: u32, tile_plg: TilePlg, pak: Option<(&str, &str)>, collection: &str, zone: &str) -> Result<GameLayout, String> {
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(map_path));
    let (sx, sz) = (64i32, 64i32);
    let baked: Vec<(i32, i32)> = mf.baked.iter().map(|b| { let (x, _, z) = b.coords(); (x, z) }).collect();
    let cell_of = tile_cells(&baked, sx, sz);
    if cell_of.len() as u32 != base {
        return Err(format!("{} tile cells but base {base}", cell_of.len()));
    }
    let tile_y: i32 = mf.baked.first().map(|b| b.coords().1).unwrap_or(5);
    let item_cells: std::collections::HashSet<(i32, i32, i32)> = mf.items.iter().map(|it| (it.file_cell[0] as i32, it.file_cell[1] as i32, it.file_cell[2] as i32)).collect();
    let tq = tile_quality(&cell_of, tile_y, &item_cells);
    // the keys
    let mut tile_keys: Vec<ChartKey> = cell_of.iter().map(|&(cx, cz)| ChartKey { centre: [cx as f32 * 32.0 + 16.0, 8.0, cz as f32 * 32.0 + 16.0], h2: 0.0 }).collect();
    let mut item_keys: std::collections::HashMap<u32, ChartKey> = Default::default();
    if let Some((pak_path, key)) = pak {
        let mut store = mapgeom::store::DataStore::empty();
        store.add_pak(pak_path, key).map_err(|e| format!("pak: {e}"))?;
        let chunks = tmmaps::gbx::all_skip_chunks(&mf.gbx.body);
        let gen: Vec<(String, u32)> = chunks.iter().find(|(c, ..)| *c == 0x0304_3043).and_then(|&(_, _, payload, size)| tmmaps::map::genealogy_full(&mf.gbx.body[payload..payload + size]).ok()).map(|recs| recs.into_iter().map(|r| (r.current, r.dir)).collect()).unwrap_or_default();
        let size = [mf.size[0].max(0) as usize, mf.size[1].max(0) as usize, mf.size[2].max(0) as usize];
        let tiles = crate::lmtiles::tile_records(&mut store, collection, size, &gen, zone, tile_y as f32, -40.0, 1.0)?;
        let mut by_cell: std::collections::HashMap<(i32, i32), ChartKey> = Default::default();
        for (cx, cz, _zone, _dir, rec) in &tiles {
            let h = rec.world.h;
            by_cell.insert((*cx as i32, *cz as i32), ChartKey { centre: rec.world.c, h2: (h[0] * h[0] + h[1] * h[1]) + h[2] * h[2] });
        }
        for (k, c) in cell_of.iter().enumerate() {
            if let Some(kk) = by_cell.get(c) {
                tile_keys[k] = *kk;
            }
        }
        for it in crate::lmtiles::item_records(scene, 1.0, false) {
            if let Some(r) = it.record {
                let h = r.world.h;
                item_keys.insert(base + it.item as u32, ChartKey { centre: r.world.c, h2: (h[0] * h[0] + h[1] * h[1]) + h[2] * h[2] });
            }
        }
    } else {
        for inst in &scene.instances {
            let mdl = &scene.models[inst.model];
            let mut lo = [f32::MAX; 3];
            let mut hi = [f32::MIN; 3];
            for t in &mdl.tris {
                for p in &t.p {
                    let w = crate::geometry::xf_point(&inst.xf, *p);
                    for k in 0..3 {
                        lo[k] = lo[k].min(w[k]);
                        hi[k] = hi[k].max(w[k]);
                    }
                }
            }
            item_keys.insert(base + inst.item as u32, ChartKey { centre: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0], h2: 0.0 });
        }
    }
    let tiles: Vec<([f32; 2], ChartKey)> = (0..base as usize).map(|k| (tile_plg.ext(tq[k]), tile_keys[k])).collect();
    let items: Vec<(u32, [f32; 2], ChartKey, Charted)> = scene
        .instances
        .iter()
        .map(|inst| {
            let m = &scene.models[inst.model];
            let sc = (inst.xf[0] * inst.xf[0] + inst.xf[1] * inst.xf[1] + inst.xf[2] * inst.xf[2]).sqrt();
            let q = item_quality(inst.lm_quality);
            let ext = match m.plg_bounds {
                Some(b) => {
                    let f = m.plg_u02 * (q * sc);
                    [(b[2] - b[0]) * f, (b[3] - b[1]) * f]
                }
                None => [0.0, 0.0],
            };
            let obj = base + inst.item as u32;
            (obj, ext, item_keys.get(&obj).copied().unwrap_or(ChartKey { centre: [0.0; 3], h2: 0.0 }), if item_bound(m) { Charted::Bound } else { Charted::Packed })
        })
        .collect();
    let mut out = allocate(&LayoutInput { tiles, items, w_atlas: 2048, quality_index })?;
    out.cell_of = cell_of;
    out.tile_quality = tq;
    Ok(out)
}
