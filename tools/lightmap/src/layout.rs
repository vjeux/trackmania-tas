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
    /// The grouped allocation's entries: (placed outer rect x, y, w, h; nb, na; the member records' chart indices in
    /// ordinal order) — empty for the per-record allocation.
    pub entries: Vec<((i32, i32, i32, i32), (u32, u32), Vec<(usize, u32)>)>,
    /// Per entry: (group index, chunk index, the area whose bits the walk sorts by).
    pub entry_keys: Vec<(usize, u32, f32)>,
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
    // THE RING IS THREE-DIMENSIONAL (E, stpad's editor table, 2026-09-25): r = the Chebyshev distance max(|dx|, |dy|, |dz|) to the
    // nearest marked cell — the 180 WaterBase blocks one cell ABOVE the ground give their tiles q = (√2)^−1, not 1.0, and the
    // eight ground-level items give theirs 1.0; a marked cell farther than 8 in any axis counts for nothing. On the one-level
    // BlueBay maps this is the planar rule.
    let mut ys: Vec<i32> = item_cells.iter().map(|c| c.1).collect();
    ys.sort_unstable();
    ys.dedup();
    cells
        .iter()
        .map(|&(cx, cz)| {
            let mut best: i32 = i32::MAX;
            for &y in &ys {
                let dy = (y - tile_y).abs();
                if dy > 8 || dy >= best {
                    continue;
                }
                // the nearest marked cell at this level: planar Chebyshev radius p ascending, the 3-D distance max(dy, p)
                for p in 0..=8i32 {
                    let r = dy.max(p);
                    if r >= best {
                        break;
                    }
                    let mut hit = false;
                    'scan: for dx in -p..=p {
                        for dz in -p..=p {
                            if dx.abs().max(dz.abs()) != p {
                                continue;
                            }
                            if item_cells.contains(&(cx + dx, y, cz + dz)) {
                                hit = true;
                                break 'scan;
                            }
                        }
                    }
                    if hit {
                        best = r;
                        break;
                    }
                }
            }
            if best == 0 {
                1.0
            } else if best <= 8 {
                (0.5f32).powf(best as f32 * 0.5)
            } else {
                Q_FAR
            }
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
    Ok(GameLayout { charts: out, s, sum_area, w_atlas: input.w_atlas, params: (g, pad, m), max_iter, cell_of: Vec::new(), tile_quality: Vec::new(), entries: Vec::new(), entry_keys: Vec::new() })
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
/// A collection's ground: the decoration grid (cells of 32 m from the world origin), the ground row, the world-y offset
/// (`world_y = cy · 8 + yoff`) and the flat terrain zones a tiny map marks its ground with.
pub struct CollectionProfile {
    pub grid: i32,
    pub ground_row: i32,
    pub yoff: f32,
    pub flat_zones: &'static [&'static str],
}

impl CollectionProfile {
    pub fn of(collection: &str) -> CollectionProfile {
        match collection {
            // stpad's table: 96 × 96 Grass tiles at y 8 (row 9), the WaterBase blocks at cy 10 → 16
            "Stadium" => CollectionProfile { grid: 96, ground_row: 9, yoff: -64.0, flat_zones: &["Grass"] },
            // the tiny maps' items sit in row 16 at y 8..16 → yoff −120 (the one Water block at row 14)
            "WhiteShore" => CollectionProfile { grid: 64, ground_row: 14, yoff: -120.0, flat_zones: &["Land", "Water"] },
            // items in row 4 at y −8..0 → yoff −40 (the one Lake block at row 4)
            "GreenCoast" => CollectionProfile { grid: 64, ground_row: 4, yoff: -40.0, flat_zones: &["Grass", "Lake"] },
            // BlueBay: the Sea tiles' row 5 at y 0
            _ => CollectionProfile { grid: 64, ground_row: 5, yoff: -40.0, flat_zones: &["Sea", "Land"] },
        }
    }
}

/// The tiles' cell row: LMTOOL_TILE_LEVEL, else the baked records' row, else a ground-flagged flat terrain block's (the tiny
/// maps' one Water / Lake block), else the collection's ground row.
pub fn tile_level(mf: &tmmaps::map::MapFile, collection: &str) -> i32 {
    let prof = CollectionProfile::of(collection);
    std::env::var("LMTOOL_TILE_LEVEL").ok().and_then(|v| v.parse().ok())
        .or_else(|| mf.baked.first().map(|b| b.coords().1))
        .or_else(|| mf.blocks.iter().find(|b| b.flags & 0x1000 != 0 && prof.flat_zones.contains(&b.name.as_str())).map(|b| b.coords().1))
        .unwrap_or(prof.ground_row)
}

pub fn for_map(map_path: &str, scene: &crate::geometry::Scene, base: u32, quality_index: u32, tile_plg: TilePlg, pak: Option<(&str, &str)>, collection: &str, zone: &str, kept: Option<&std::collections::HashSet<usize>>) -> Result<GameLayout, String> {
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(map_path));
    let prof = CollectionProfile::of(collection);
    // the ground grid: the map's own size when its cell count is the tile base (the 64 × 64 tiny maps), else the collection's
    // decoration grid (Stadium's 96 × 96 ground under a 48 × 48 map)
    let (sx, sz) = if (mf.size[0].max(0) * mf.size[2].max(0)) as u32 == base { (mf.size[0], mf.size[2]) } else { (prof.grid, prof.grid) };
    let baked: Vec<(i32, i32)> = mf.baked.iter().map(|b| { let (x, _, z) = b.coords(); (x, z) }).collect();
    let cell_of = tile_cells(&baked, sx, sz);
    if cell_of.len() as u32 != base {
        return Err(format!("{} tile cells but base {base}", cell_of.len()));
    }
    // the tile level: the baked records' row, else a ground-flagged flat terrain block's (the tiny maps' one Water / Lake block),
    // else the collection's ground row; LMTOOL_TILE_LEVEL / LMTOOL_YOFF override
    let tile_y: i32 = tile_level(&mf, collection);
    let yoff: f32 = std::env::var("LMTOOL_YOFF").ok().and_then(|v| v.parse().ok()).unwrap_or(prof.yoff);
    // the marked cells: the items' file cells and the blocks' (a ghost block, flag bit 28, marks nothing); the ring is 3-D
    // (tiny03 WhiteShore's table: an ITEM marks its cell only at the tiles' own level — rings 1–4 = 147 / 129 / 111 / 106 exactly
    // with the items of row 14 alone, the rows 15–21 items excluded; a BLOCK marks in 3-D — stpad's WaterBase blocks one row up give
    // ring 1. LMTOOL_ITEMS_3D=1 marks items in 3-D too.)
    let items_3d = std::env::var("LMTOOL_ITEMS_3D").is_ok();
    let mut item_cells: std::collections::HashSet<(i32, i32, i32)> = mf.items.iter().map(|it| (it.file_cell[0] as i32, it.file_cell[1] as i32, it.file_cell[2] as i32)).filter(|c| items_3d || c.1 == tile_y).collect();
    for b in &mf.blocks { if b.flags & 0x1000_0000 != 0 { continue; } if b.flags & 0x1000 != 0 && prof.flat_zones.contains(&b.name.as_str()) { continue; } let (x, y, z) = b.coords(); item_cells.insert((x, y, z)); }
    let tq = tile_quality(&cell_of, tile_y, &item_cells);
    if std::env::var("LMTOOL_LAYOUT_TRACE").is_ok() {
        let mut h: std::collections::BTreeMap<u32, usize> = Default::default();
        for q in &tq { *h.entry(q.to_bits()).or_default() += 1; }
        eprintln!("layout tiles: grid {sx} × {sz}, level {tile_y}, yoff {yoff}, {} marked cells; quality histogram {:?}", item_cells.len(), h.iter().map(|(b, n)| (f32::from_bits(*b), *n)).collect::<Vec<_>>());
    }
    // the keys
    let mut tile_keys: Vec<ChartKey> = cell_of.iter().map(|&(cx, cz)| ChartKey { centre: [cx as f32 * 32.0 + 16.0, 8.0, cz as f32 * 32.0 + 16.0], h2: 0.0 }).collect();
    let mut item_keys: std::collections::HashMap<u32, ChartKey> = Default::default();
    let mut tile_plg_from_pak: Option<TilePlg> = None;
    if let Some((pak_path, key)) = pak {
        let mut store = mapgeom::store::DataStore::empty();
        store.add_pak(pak_path, key).map_err(|e| format!("pak: {e}"))?;
        // the zone prefab's PreLightGen (MeterByUv, uv bounds) — the collection's tile chart extent
        if let Ok(zt) = crate::records::zone_tiles(&mut store, collection, zone, 1, 0.0, 0.0, &|_, _| 1.0) {
            if let Some(r) = zt.first() { tile_plg_from_pak = Some(TilePlg { meter_by_uv: r.meter_by_uv, bounds: r.uv }); }
        }
        let chunks = tmmaps::gbx::all_skip_chunks(&mf.gbx.body);
        let gen: Vec<(String, u32)> = chunks.iter().find(|(c, ..)| *c == 0x0304_3043).and_then(|&(_, _, payload, size)| tmmaps::map::genealogy_full(&mf.gbx.body[payload..payload + size]).ok()).map(|recs| recs.into_iter().map(|r| (r.current, r.dir)).collect()).unwrap_or_default();
        let size = [mf.size[0].max(0) as usize, mf.size[1].max(0) as usize, mf.size[2].max(0) as usize];
        let size = if (size[0] * size[2]) as u32 == base { size } else { [sx as usize, size[1], sz as usize] };
        let tiles = crate::lmtiles::tile_records(&mut store, collection, size, &gen, zone, tile_y as f32, yoff, 1.0)?;
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
    let tile_plg = tile_plg_from_pak.unwrap_or(tile_plg);
    let tiles: Vec<([f32; 2], ChartKey)> = (0..base as usize).map(|k| (tile_plg.ext(tq[k]), tile_keys[k])).collect();
    // the records among the items: the kept list when given (RE 7's reduction of the reference bakes), and never a model without
    // a non-degenerate uv-set-0 bound (the 1-uv-set items get no record)
    let is_record = |inst: &crate::geometry::Instance| -> bool {
        if let Some(k) = kept { if !k.contains(&inst.item) { return false; } }
        let m = &scene.models[inst.model];
        // the game's filter (itemrule::static_item_record): a PreLightGen with non-degenerate uv-set-0 bounds AND lightmap
        // geometry — a geom whose MATERIAL takes a lightmap set (the compiled material's texcoord index ≠ −1): the
        // RaceTriggerFX materials take none, so an item made only of them (tiny 16's two AC16497076) has no record; a
        // single-set material (the wall's TrackWallInWorld) lightmaps through its set 0 and counts
        let fx_only = !m.mat_links.is_empty() && m.mat_links.iter().all(|l| l.contains("RaceTriggerFX"));
        if fx_only { return false; }
        match m.plg_bounds { Some(b) => b[2] > b[0] && b[3] > b[1], None => false }
    };
    let record_instances: Vec<&crate::geometry::Instance> = scene.instances.iter().filter(|i| is_record(i)).collect();
    let items: Vec<(u32, [f32; 2], ChartKey, Charted)> = record_instances
        .iter()
        .map(|inst| {
            let m = &scene.models[inst.model];
            // the placement's own scale word (1.0 unless the item is scaled) — not the transform column's norm, whose f32
            // rounding gave rotated instances of one model extents an ulp apart (and the grid entries different area bits,
            // breaking the walk's ties: tiny 16's groups 18/19 pack in list order only when their areas tie exactly)
            let sc = if inst.pose.scale > 0.0 { inst.pose.scale } else { 1.0 };
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
    // the group keys: tiles = (the zone prefab, q); items = (the model, q) — LMTOOL_LAYOUT_PER_RECORD=1 keeps the per-record
    // allocation (the grouped one coincides with it where every record is solo)
    let input = LayoutInput { tiles, items, w_atlas: 2048, quality_index };
    let mut out = if std::env::var_os("LMTOOL_LAYOUT_PER_RECORD").is_some() {
        allocate(&input)?
    } else {
        let mut groups: Vec<u64> = Vec::with_capacity(input.tiles.len() + input.items.len());
        for k in 0..input.tiles.len() {
            groups.push(0xF000_0000_0000_0000u64 | tq[k].to_bits() as u64);
        }
        for inst in &record_instances {
            let q = item_quality(inst.lm_quality);
            groups.push(((inst.model as u64) << 32) | q.to_bits() as u64);
        }
        allocate_grouped(&input, &groups)?
    };
    out.cell_of = cell_of;
    out.tile_quality = tq;
    Ok(out)
}

/// THE GROUPED ALLOCATION (RE 7, 2026-09-25 14:30Z; `itemrule`): the game packs one chart per (PreLightGen, quality) GROUP
/// holding an nb × na grid of instance cells — unless the record is SOLO (ext·D₁ > 100 on either axis) — the groups
/// chunked to at most `c` records per entry, the entries packed by the same walk, every record given the z-order cell of its
/// Morton ordinal. Where every record is solo (the small maps: D₁ ≈ 110) this is `allocate` exactly.
///
/// `groups[k]`: the group key of record k (tiles first, then items, as `LayoutInput` lists them).
thread_local! {
    /// The last grouped allocation's walk position per entry (a study aid for `packtest --cell-study`).
    pub static WALK_POS: std::cell::RefCell<Vec<usize>> = std::cell::RefCell::new(Vec::new());
}

pub fn allocate_grouped(input: &LayoutInput, groups: &[u64]) -> Result<GameLayout, String> {
    use crate::itemrule as ir;
    let (g, pad, m) = crate::pack::layout_params(input.w_atlas, input.w_atlas);
    let max_iter = max_iter_for_quality(input.quality_index);
    // the records
    let mut exts: Vec<[f32; 2]> = Vec::new();
    let mut keys: Vec<ChartKey> = Vec::new();
    let mut objs: Vec<(u32, Charted)> = Vec::new();
    for (i, (ext, key)) in input.tiles.iter().enumerate() {
        exts.push(*ext);
        keys.push(*key);
        objs.push((i as u32, Charted::Bound));
    }
    for (obj, ext, key, ch) in &input.items {
        exts.push(*ext);
        keys.push(*key);
        objs.push((*obj, *ch));
    }
    let n = exts.len();
    if groups.len() != n {
        return Err(format!("{} group keys for {n} records", groups.len()));
    }
    // Σ₁ (record order, f32) → D₁
    let sum1 = exts.iter().fold(0f32, |acc, e| acc + e[0] * e[1]);
    let d1 = (input.w_atlas as f32 * input.w_atlas as f32) / sum1;
    // the model list: solo records each their own entry; the rest grouped by key in first-appearance order
    let mut members: Vec<Vec<usize>> = Vec::new();
    let mut key_of: std::collections::HashMap<u64, usize> = Default::default();
    let mut model_of_record: Vec<usize> = vec![0; n];
    for k in 0..n {
        if ir::is_solo(exts[k], d1) {
            model_of_record[k] = members.len();
            members.push(vec![k]);
            continue;
        }
        let gi = *key_of.entry(groups[k]).or_insert_with(|| { members.push(Vec::new()); members.len() - 1 });
        model_of_record[k] = gi;
        members[gi].push(k);
    }
    // ordinals: per group, the Morton order of the centres — record at group position i gets perm[i]
    let mut ordinal: Vec<u32> = vec![0; n];
    for mem in &members {
        let centres: Vec<[f32; 3]> = mem.iter().map(|&k| keys[k].centre).collect();
        let perm = ir::group_ordinals(&centres);
        for (i, &k) in mem.iter().enumerate() {
            ordinal[k] = perm[i];
        }
    }
    // chunks
    let counts: Vec<u32> = members.iter().map(|m| m.len() as u32).collect();
    // the chunk size: RE 7's dumps give c = 8 on tiny 16 (458 models) and on stpad (946 models) where the read formula gives
    // 8 and 6 — c = 8 whenever chunking applies (k = ceil(1000 / nModels) > 1) until a third map pins the divisor
    // RE 7's FUN_140292740 chunk size (tiny 16: 458 groups → 8; stpad: 735 → 8; tiny03 WhiteShore: 279 → 6); LMTOOL_CHUNK=N overrides
    let chunk = match std::env::var("LMTOOL_CHUNK").ok().and_then(|v| v.parse::<u32>().ok()) { Some(c) => Some(c), None => ir::chunk_size(n as u32, members.len() as u32) };
    let entries = ir::split_chunks(&counts, chunk);
    let c = chunk.unwrap_or(u32::MAX);
    // the entry of a record: its group's chunk `ordinal / c` — the chunk-0 entry sits at the group's place, the others where
    // split_chunks appended them
    let mut entry_of: std::collections::HashMap<(usize, u32), usize> = Default::default();
    for (ei, e) in entries.iter().enumerate() {
        entry_of.insert((e.group, e.chunk), ei);
    }
    // the entry charts: the grid dims, the grid extent, the key of the first member (chunk order)
    let mut charts: Vec<ChartExt> = Vec::with_capacity(entries.len());
    let mut ekeys: Vec<ChartKey> = Vec::with_capacity(entries.len());
    let mut dims: Vec<(u32, u32)> = Vec::with_capacity(entries.len());
    let mut first_member: Vec<Option<usize>> = vec![None; entries.len()];
    for k in 0..n {
        let ei = entry_of[&(model_of_record[k], if chunk.is_some() { ordinal[k] / c } else { 0 })];
        if first_member[ei].is_none() {
            first_member[ei] = Some(k);
        }
    }
    for (ei, e) in entries.iter().enumerate() {
        // THE ENTRY'S EXTENT IS THE GROUP'S FIRST RECORD'S (record order) — for every chunk of the group: the per-record
        // extents differ in the last bits (the placement scale's rounding), and tiny 16's editor table places group 59's
        // chunk 78 (whose own first member has the smaller area bits) with the group's area, right before chunk 77
        let k0 = members[e.group][0];
        first_member[ei] = Some(k0);
        let ext = exts[k0];
        let (nb, na) = ir::grid_dims(e.count, ext, d1, m as u32);
        let (gext, _area) = ir::grid_chart(ext, nb, na);
        charts.push(ChartExt { ext: gext, mins: [nb as u16, na as u16] });
        ekeys.push(keys[k0]);
        dims.push((nb, na));
    }
    // THE GROUPED WALK (RE 7, 15:10Z): the sorter sees the entry count differ from the record count and resets to the
    // identity before its single stable pass by the f32 bits of area → (area bits ascending, then model-list index); the
    // (z, y, x, |h|²) passes exist only when every record is its own entry — there `walk_order` (the same result up to the
    // tie rule, which the per-record maps need: LMTOOL_LAYOUT_PER_RECORD keeps that path)
    let areas: Vec<f32> = (0..charts.len()).map(|i| ((dims[i].0 * dims[i].1) as f32 * exts[first_member[i].unwrap()][1]) * exts[first_member[i].unwrap()][0]).collect();
    let order = if entries.len() == n { walk_order(&charts, &ekeys) } else {
        match std::env::var("LMTOOL_GROUP_TIE").ok().as_deref() {
            // the study: ties by the first member's record index / by the smallest member record index / by the Morton-first member
            Some("first") => { let mut idx: Vec<usize> = (0..charts.len()).collect(); idx.sort_by_key(|&i| (areas[i].to_bits(), first_member[i].unwrap_or(0))); idx }
            Some("minrec") => { let mut idx: Vec<usize> = (0..charts.len()).collect(); let minrec: Vec<usize> = (0..charts.len()).map(|ei| (0..n).filter(|&k| entry_of[&(model_of_record[k], if chunk.is_some() { ordinal[k] / c } else { 0 })] == ei).min().unwrap_or(0)).collect(); idx.sort_by_key(|&i| (areas[i].to_bits(), minrec[i])); idx }
            Some("revidx") => { let mut idx: Vec<usize> = (0..charts.len()).collect(); idx.sort_by_key(|&i| (areas[i].to_bits(), std::cmp::Reverse(i))); idx }
            _ => ir::grouped_pack_order(&areas),
        }
    };
    // TotalLmSurfaceMeter: the f32 sum of the entry areas along the ascending-area radix order (ties by index)
    let sum_area = {
        let mut idx: Vec<usize> = (0..charts.len()).collect();
        idx.sort_by(|&p, &q| fcmp(charts[p].ext[0] * charts[p].ext[1], charts[q].ext[0] * charts[q].ext[1]).then(p.cmp(&q)));
        idx.iter().fold(0f32, |acc, &i| acc + ((dims[i].0 * dims[i].1) as f32 * exts[first_member[i].unwrap()][1]) * exts[first_member[i].unwrap()][0])
    };
    crate::pack::SUM_AREA_OVERRIDE.store(sum_area.to_bits(), std::sync::atomic::Ordering::Relaxed);
    let res = crate::pack::allocate_ordered(&charts, &order, input.w_atlas, input.w_atlas, g, m, max_iter);
    crate::pack::SUM_AREA_OVERRIDE.store(0, std::sync::atomic::Ordering::Relaxed);
    let (s, placed) = res.ok_or("the allocation failed (no scale packs)")?;
    // the cells: each record's rect inside its entry's placed rect
    let mut out: Vec<LayoutChart> = Vec::with_capacity(n);
    for k in 0..n {
        let ei = entry_of[&(model_of_record[k], if chunk.is_some() { ordinal[k] / c } else { 0 })];
        let (nb, na) = dims[ei];
        let p = &placed[ei];
        let cells = ir::zorder_cells(nb, na);
        let o = if chunk.is_some() { ordinal[k] % c } else { ordinal[k] };
        let (cx, cy) = cells.get(o as usize).copied().unwrap_or((0, 0));
        let ex = ir::cell_edges(p.w as u32, nb, g as u32);
        let ey = ir::cell_edges(p.h as u32, na, g as u32);
        let (x0, x1) = (ex[cx as usize], ex[cx as usize + 1]);
        let (y0, y1) = (ey[cy as usize], ey[cy as usize + 1]);
        out.push(LayoutChart { obj: objs[k].0, ext: exts[k], charted: objs[k].1, x: p.x as i32 + x0 as i32 + pad as i32, y: p.y as i32 + y0 as i32 + pad as i32, w: (x1 - x0) as i32 - 2 * pad as i32, h: (y1 - y0) as i32 - 2 * pad as i32 });
    }
    if std::env::var_os("LMTOOL_LAYOUT_TRACE").is_some() {
        let solos = members.iter().filter(|m| m.len() == 1).count();
        eprintln!("layout grouped: {n} records, Σ₁ {sum1} D₁ {d1}, {} models ({solos} solo), chunk {chunk:?}, {} entries, Σ {sum_area}, s {s}", members.len(), entries.len());
    }
    let mut entry_out: Vec<((i32, i32, i32, i32), (u32, u32), Vec<(usize, u32)>)> = entries.iter().enumerate().map(|(ei, _)| { let p = &placed[ei]; ((p.x as i32, p.y as i32, p.w as i32, p.h as i32), dims[ei], Vec::new()) }).collect();
    for k in 0..n {
        let ei = entry_of[&(model_of_record[k], if chunk.is_some() { ordinal[k] / c } else { 0 })];
        let o = if chunk.is_some() { ordinal[k] % c } else { ordinal[k] };
        entry_out[ei].2.push((k, o));
    }
    // (the walk position: the packer places `order` from its end — position 0 = placed first)
    let mut walk_pos: Vec<usize> = vec![0; entries.len()];
    for (pos, &ei) in order.iter().rev().enumerate() { walk_pos[ei] = pos; }
    let entry_keys: Vec<(usize, u32, f32)> = entries.iter().enumerate().map(|(ei, e)| (e.group, e.chunk, areas[ei])).collect();
    let _ = &walk_pos;
    WALK_POS.with(|w| *w.borrow_mut() = walk_pos.clone());
    Ok(GameLayout { charts: out, s, sum_area, w_atlas: input.w_atlas, params: (g, pad, m), max_iter, cell_of: Vec::new(), tile_quality: Vec::new(), entries: entry_out, entry_keys })
}
