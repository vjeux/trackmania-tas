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
/// The water SURFACE (the Water quad's local height over the cell floor) per vista collection — measured on the
/// zone prefabs with `static-item --collection 26`: RedIsland 7.70, WhiteShore 7.00, BlueBay 7.00, GreenCoast 7.20.
pub fn water_surface(collection: u32) -> f32 {
    match collection {
        0x10 => 7.70,
        0x1d => 7.00,
        0x1c => 7.00,
        0xf => 7.20,
        _ => 0.0,
    }
}

/// Hugo's round 5 (2026-10-24 00:12): the slab TOP sits a constant 2.5 m under the water surface on every vista
/// (so the finish depth reads the same everywhere), the BOTTOM 0.1 m under the bed (a car resting on the bed is
/// inside). Returns (below the bed, above the bed) for `floor_mesh`.
/// v4.3 (Hugo, via the coordinator 06:25: "all finishes on the same y on a map" — his friend saw the shore slabs
/// stepping up the bank in 25 cm quanta): every box of a water row sits on the row's OPEN BED — seabed − 0.1 ..
/// seabed + 1.15 — and the indicator at seabed + 0.02; the shore squares no longer carry their own bed (the
/// 1.15 m top still covers a car on a bank square whose bed is up to ~1 m above the seabed).
pub fn vista_slab(_collection: u32) -> (f32, f32) {
    (0.1, 1.15)
}

pub fn water_tiles(collection: u32) -> (&'static [&'static str], f32) {
    match collection {
        // RedIsland's `WaterHill` is the lake's HILL RIM (a 26-prefab shore family: its water quad covers 24–61 of
        // the cell's 64 four-metre squares and its bed climbs from the deep 2.0 to the surface), not open water —
        // Hugo (2026-10-23 09:51): "only full seabed blocks … only where the car can reach seafloor level" → out
        0x10 => (&["Water"], 2.0),
        0x1d => (&["Water"], 2.0),
        0x1c => (&["Sea"], 4.0),
        0xf => (&["Lake"], 2.0),
        _ => (&[], 0.0),
    }
}

/// The water footprint of ONE placement of a terrain tile: the prefab the placement's flags pick (Beach is a
/// 14-list shore family: straight / dead-end / corner pieces), its Water quad's x/z extent in the block frame
/// (dir 0) and the collision floor under it (min/max y: flat = a slab, sloping = a ramp).
#[derive(Clone, Debug)]
pub struct WaterRect {
    pub prefab: String,
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    /// the water quad's local y
    pub surface: f32,
    /// the collision floor directly under the water quad: (min y, max y) over the quad's footprint
    pub bed_lo: f32,
    pub bed_hi: f32,
    /// collision triangles under the quad
    pub bed_tris: usize,
    /// the water quad's own triangles (block frame; the slab footprint — a shore's water is a triangle / an L,
    /// not its bounding box)
    pub tris: Vec<[[f32; 3]; 3]>,
    /// the collision triangles under the quad (block frame) — the bed; a square is DEEP when the bed under its
    /// centre lies within ±0.3 m of the collection's open-water bed (Hugo 2026-10-24: "same depth")
    pub bed_tris_geo: Vec<[[f32; 3]; 3]>,
    /// every collision triangle of the tile (block frame): the dry land / sand top for the section drawings
    pub all_tris_geo: Vec<[[f32; 3]; 3]>,
    /// the block's footprint in cells at dir 0 (1×1 for the zone tiles; the Deco bays span 2×1 .. 2×4)
    pub size: (i32, i32),
}

/// Measure a tile placement's water footprint: bake the picked prefab (water quad kept), take the Water
/// material's visual triangles → rect + surface; the collision triangles whose centroid lies inside that rect
/// and below the surface → bed range.
pub fn water_rect(store: &mut crate::store::DataStore, collection: u32, tile: &str, flags: u32) -> Result<Option<WaterRect>, String> {
    let Some(bp) = crate::vista::find_block_info(store, tile) else { return Err(format!("{tile}: no block info")) };
    let bi = crate::blockinfo::load(store, &bp)?;
    let ground = flags & crate::blockmap::FLAG_GROUND != 0;
    let variant = (flags & 63) as usize;
    let sub = ((flags >> crate::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
    let addv = ((flags >> crate::blockmap::FLAG_ADDITIONAL_SHIFT) & 0x7F) as usize;
    let Some(picked) = bi.pick_placement_add(ground, variant, sub, addv) else { return Err(format!("{tile}: no variant for flags {flags:#x}")) };
    let size = {
        let units = &picked.variant.block_units;
        if units.is_empty() { (1, 1) } else { (units.iter().map(|u| u.offset[0]).max().unwrap_or(0) + 1, units.iter().map(|u| u.offset[2]).max().unwrap_or(0) + 1) }
    };
    let Some(mobil) = picked.mobils.first() else { return Ok(None) };
    let Some(prefab) = mobil.prefab.clone() else { return Ok(None) };
    let mut m = crate::static_item::build::Merged::default();
    m.keep_water = true;
    let mut at = crate::geom::IDENTITY;
    if let Some(t) = mobil.translation { at[9] = t[0]; at[10] = t[1]; at[11] = t[2]; }
    crate::static_item::build::add_prefab(store, &prefab, &at, 1.0, &mut m, 0)?;
    // the Water quad(s)
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    let mut n_water = 0usize;
    let mut wtris: Vec<[[f32; 3]; 3]> = Vec::new();
    for mv in &m.visuals {
        let link = m.materials.get(mv.material).and_then(|mat| mat.link()).unwrap_or("");
        if !link.ends_with("\\Material\\Water") { continue; }
        let Some(main) = mv.visual.main.as_ref() else { continue };
        let Some(crate::static_item::Node::VertexStream(stream)) = main.vertex_streams.first().and_then(|r| r.inline.as_deref()) else { continue };
        let compress = stream.compress_local3d.unwrap_or(false);
        let mut pos: &[[f32; 3]] = &[];
        for (d, e) in stream.decls.iter().zip(stream.elems.iter()) {
            if let (crate::static_item::vstream::N_POSITION, crate::static_item::vstream::T_FLOAT3, crate::static_item::vstream::Elem::Float3(p)) = (d.name(), d.stored_type(compress), e) {
                pos = p;
                for v in p { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } n_water += 1; }
            }
        }
        if let Some(ib) = mv.visual.index_buffer.as_ref() {
            for t in ib.indices.chunks(3) {
                if t.len() == 3 && t.iter().all(|&i| (i as usize) < pos.len()) {
                    wtris.push([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
                }
            }
        }
    }
    if n_water == 0 { return Ok(None); }
    // the prefab carries the quad once per LOD level: keep one copy of each triangle (by its xz corners, cm grid)
    {
        let mut seen: std::collections::HashSet<[(i64, i64); 3]> = Default::default();
        wtris.retain(|t| {
            let mut k: Vec<(i64, i64)> = t.iter().map(|v| ((v[0] * 100.0).round() as i64, (v[2] * 100.0).round() as i64)).collect();
            k.sort_unstable();
            seen.insert([k[0], k[1], k[2]])
        });
    }
    let surface = (lo[1] + hi[1]) / 2.0;
    // the bed: collision triangles under the quad's footprint
    let (mut blo, mut bhi, mut nb) = (f32::INFINITY, f32::NEG_INFINITY, 0usize);
    let mut bed_geo: Vec<[[f32; 3]; 3]> = Vec::new();
    let mut all_geo: Vec<[[f32; 3]; 3]> = Vec::new();
    for t in &m.surf_triangles {
        let pts: Vec<[f32; 3]> = t.indices.iter().filter_map(|&i| m.surf_vertices.get(i as usize).copied()).collect();
        if pts.len() < 3 { continue; }
        all_geo.push([pts[0], pts[1], pts[2]]);
        let c = [(pts[0][0] + pts[1][0] + pts[2][0]) / 3.0, (pts[0][1] + pts[1][1] + pts[2][1]) / 3.0, (pts[0][2] + pts[1][2] + pts[2][2]) / 3.0];
        if c[0] >= lo[0] - 0.01 && c[0] <= hi[0] + 0.01 && c[2] >= lo[2] - 0.01 && c[2] <= hi[2] + 0.01 && c[1] < surface - 0.05 {
            for p in &pts { blo = blo.min(p[1]); bhi = bhi.max(p[1]); }
            nb += 1;
            bed_geo.push([pts[0], pts[1], pts[2]]);
        }
    }
    Ok(Some(WaterRect { prefab: prefab.rsplit('\\').next().unwrap_or(&prefab).to_string(), x0: lo[0], x1: hi[0], z0: lo[2], z1: hi[2], surface, bed_lo: blo, bed_hi: bhi, bed_tris: nb, tris: wtris, bed_tris_geo: bed_geo, all_tris_geo: all_geo, size }))
}

/// The 4 m-grid water mask of a shore kind in the block frame: bit (zi·8 + xi) = the square at x 4·xi..4·xi+4,
/// z 4·zi..4·zi+4 (its centre inside a water triangle). The same rasterisation `floor_mesh` uses.
/// The height of the bed collision under a block-frame point (the highest collision triangle covering it,
/// below the water surface), or None when none covers it.
fn bed_height_at(w: &WaterRect, p: [f32; 2]) -> Option<f32> {
    let mut best: Option<f32> = None;
    for t in &w.bed_tris_geo {
        let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
        let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
        let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
        if (d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0) { continue; }
        // barycentric height
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        let y = if det.abs() < 1e-9 { (t[0][1] + t[1][1] + t[2][1]) / 3.0 } else {
            let l1 = ((b[0] - p[0]) * (c[1] - p[1]) - (c[0] - p[0]) * (b[1] - p[1])) / det;
            let l2 = ((c[0] - p[0]) * (a[1] - p[1]) - (a[0] - p[0]) * (c[1] - p[1])) / det;
            let l3 = 1.0 - l1 - l2;
            l1 * t[0][1] + l2 * t[1][1] + l3 * t[2][1]
        };
        if y < w.surface - 0.05 { best = Some(best.map_or(y, |b: f32| b.max(y))); }
    }
    best
}

/// A shore square counts as DEEP water when the water over its bed is at least this deep (Hugo 2026-10-24:
/// "same depth" — the Beach bank is a continuous ramp 6.6 → 4.5 → 4.0 over its last 12 m, so a one-cell channel
/// between two banks bottoms out at ~4.5 under a 7.0 surface = 2.5 m of water, never at the 4.0 open-sea bed;
/// the shallow upper ramp (< 2 m of water) stays out).
pub const DEEP_WATER_MIN: f32 = 2.0;

/// v4.1 (Hugo 00:57 "don't instantly finish on the slopes" + the coordinator's NEAR-FLOOR decision, 01:12): a
/// shore square counts when it has ≥ 2 m of water AND its LOWEST bed sample lies within this of the collection's
/// open-water floor (BlueBay 4.0, the others 2.0). The Beach bank is a uniform ramp (0.31 m/m, 1.24 m per 4 m
/// square) with no level part, so this keeps exactly the lowest square of each bank — an ~8 m lane along the
/// trough bottom of a two-bank channel — and drops the banks above; WaterHill keeps its deep part, loses the rim.
pub const NEAR_FLOOR_MAX: f32 = 0.75;

/// Per 4 m square (index zi·8+xi): the square's MINIMUM bed height when the square is deep (≥ 2 m of water at
/// its centre) and near the floor (`bed_min ≤ open floor + NEAR_FLOOR_MAX`). The slab sits on that minimum.
pub fn deep_squares_near(w: &WaterRect, open_bed: f32) -> Vec<Option<f32>> {
    cell_deep_squares(w, open_bed, 0, 0)
}

/// `bed_height_at` for the drawing tools.
pub fn bed_height_at_pub(w: &WaterRect, p: [f32; 2]) -> Option<f32> {
    bed_height_at(w, p)
}

/// The dry TOP of the tile under a block-frame point: the highest collision triangle covering it that is NOT the
/// water quad (the sand / land / bank), or None when nothing covers it.
pub fn top_height_at(w: &WaterRect, p: [f32; 2]) -> Option<f32> {
    let mut best: Option<f32> = None;
    for t in &w.all_tris_geo {
        let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
        let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
        let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
        if (d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0) { continue; }
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        let y = if det.abs() < 1e-9 { (t[0][1] + t[1][1] + t[2][1]) / 3.0 } else {
            let l1 = ((b[0] - p[0]) * (c[1] - p[1]) - (c[0] - p[0]) * (b[1] - p[1])) / det;
            let l2 = ((c[0] - p[0]) * (a[1] - p[1]) - (a[0] - p[0]) * (c[1] - p[1])) / det;
            l1 * t[0][1] + l2 * t[1][1] + (1.0 - l1 - l2) * t[2][1]
        };
        if (y - w.surface).abs() < 0.05 { continue; } // the water quad's own collision
        best = Some(best.map_or(y, |b: f32| b.max(y)));
    }
    best
}

/// The DEEP mask of a shore kind (bit zi·8+xi): the near-floor squares of `deep_squares_near`.
pub fn deep_mask(w: &WaterRect, bed: f32) -> u64 {
    deep_squares_near(w, bed).iter().enumerate().fold(0u64, |m, (i, d)| if d.is_some() { m | (1u64 << i) } else { m })
}

/// Kept for the callers that want the squares with their beds: the near-floor rule with the open bed.
pub fn deep_squares(w: &WaterRect) -> Vec<Option<f32>> {
    deep_squares_near(w, w.bed_lo)
}

/// Is the block-frame point under the water quad?
fn under_water(w: &WaterRect, p: [f32; 2]) -> bool {
    let inside = |p: [f32; 2], t: &[[f32; 3]; 3]| -> bool {
        let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
        let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
        let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    };
    w.tris.iter().any(|t| inside(p, t))
}

/// The water mask of ONE unit cell (ux, uz) of the block (bit zi·8+xi = the 4 m square at block-frame
/// x 32·ux + 4·xi.., z 32·uz + 4·zi..).
pub fn cell_water_mask(w: &WaterRect, ux: i32, uz: i32) -> u64 {
    let mut mask = 0u64;
    for zi in 0..8 {
        for xi in 0..8 {
            let p = [32.0 * ux as f32 + 4.0 * xi as f32 + 2.0, 32.0 * uz as f32 + 4.0 * zi as f32 + 2.0];
            if under_water(w, p) {
                mask |= 1u64 << (zi * 8 + xi);
            }
        }
    }
    mask
}

/// The water mask of the block's first cell (the 1×1 zone tiles' whole footprint).
pub fn footprint_mask(w: &WaterRect) -> u64 {
    cell_water_mask(w, 0, 0)
}

/// The near-floor (finish) squares of ONE unit cell: per square (index zi·8+xi) the square's minimum bed when it
/// has ≥ 2 m of water at its centre and that minimum lies within `NEAR_FLOOR_MAX` of `open_bed`.
pub fn cell_deep_squares(w: &WaterRect, open_bed: f32, ux: i32, uz: i32) -> Vec<Option<f32>> {
    let mut out = vec![None; 64];
    for zi in 0..8 {
        for xi in 0..8 {
            let (cx, cz) = (32.0 * ux as f32 + 4.0 * xi as f32 + 2.0, 32.0 * uz as f32 + 4.0 * zi as f32 + 2.0);
            if !under_water(w, [cx, cz]) { continue; }
            let Some(yc) = bed_height_at(w, [cx, cz]) else { continue };
            if w.surface - yc < DEEP_WATER_MIN { continue; }
            let mut ymin = yc;
            for p in [[cx - 1.9, cz - 1.9], [cx + 1.9, cz - 1.9], [cx - 1.9, cz + 1.9], [cx + 1.9, cz + 1.9], [cx, cz - 1.9], [cx, cz + 1.9], [cx - 1.9, cz], [cx + 1.9, cz]] {
                if let Some(h) = bed_height_at(w, p) { ymin = ymin.min(h); }
            }
            if ymin <= open_bed + NEAR_FLOOR_MAX {
                out[zi * 8 + xi] = Some(ymin);
            }
        }
    }
    out
}

/// Every unit cell of the block with its water mask and its near-floor squares: ((ux, uz), water, deep squares).
pub fn cell_footprints(w: &WaterRect, open_bed: f32) -> Vec<((i32, i32), u64, Vec<Option<f32>>)> {
    let mut v = Vec::new();
    for uz in 0..w.size.1 {
        for ux in 0..w.size.0 {
            let water = cell_water_mask(w, ux, uz);
            if water == 0 { continue; }
            v.push(((ux, uz), water, cell_deep_squares(w, open_bed, ux, uz)));
        }
    }
    v
}

pub struct FloorMesh {
    pub verts: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
    /// every box (x0, x1, z0, z1, anchor y = the ground it marks: bed / grass), for the indicator plane
    pub quads: Vec<(f32, f32, f32, f32, f32)>,
    /// open-water cells (authored + baked)
    pub cells: usize,
    /// row strips the open-water cells merged into
    pub strips: usize,
    /// shore tiles with a water part
    pub shore_boxes: usize,
    /// the 4 m-grid boxes their water footprints became (merged along x per 4 m row)
    pub shore_prisms: usize,
    pub notes: Vec<String>,
    pub bbox: ([f32; 3], [f32; 3]),
}

/// The shore tiles whose water PART counts too (Hugo: "inside the water everywhere"), per collection — their
/// water rect depends on the placement's flags (the prefab the flags pick), so it is measured per kind
/// (`water_rect`) rather than tabled.
pub fn shore_tiles(collection: u32) -> &'static [&'static str] {
    // v4.1 list. v4.2 (02:55) had added 29 Deco water families (DecoBeach*/DecoTreeBeach*/Mangrove, DecoLake*/
    // DecoRiver*, DecoTerrainHD/Rocky, PlatformTechBaseOnWaterHillStraight) found by baking every kind at the lowest
    // terrain row and keeping the ones with a Water quad — REVERTED (coordinator 03:06): those pieces are OVERLAYS
    // standing on authored terrain tiles that already carry the same water (Fall 14's DecoTreeBeach cells have Beach
    // tiles under them, Fall 07's DecoLake pieces LakeShore/Lake), so they only doubled the boxes. A water quad in a
    // kind's prefab is not evidence of new water: check what the kind sits on (`deco_water_overlay_check`).
    match collection {
        0x1c => &["Beach", "SeaCliff13", "Sea_Land0_Land12"],
        0x1d => &["WaterShore1"],
        0xf => &["LakeShore"],
        0x10 => &["WaterHill", "WaterShore1"],
        _ => &[],
    }
}

/// A water piece in the WORLD frame (bed = the collision floor under it): a full cell, or a shore's water
/// footprint as the water quad's own xz triangles (exact: a beach's water is a triangle / an L, not a box).
#[derive(Clone, Debug)]
pub struct WorldRect {
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    pub y_bed: f32,
    /// the world y of the cell floor (ground + 8·cy): the water surface is `floor + water_surface(collection)`
    pub floor_y: f32,
    pub full_cell: bool,
    /// shore water: the footprint triangles in world xz (empty for a full cell)
    pub tris: Vec<[[f32; 2]; 3]>,
}

/// A block-frame (dir 0) point → the world xz of a placement at `cell` with `dir`.
/// A block-frame point of a (sx, sz)-metre footprint → world (the grid_block rule: pivot = the file cell, offsets
/// East (+sz, 0), South (+sx, +sz), West (0, +sx), yaw −dir·90°).
pub fn turn_pt_sized(cell: (i32, i32, i32), dir: u8, size_m: (f32, f32), x: f32, z: f32) -> [f32; 2] {
    let (bx, bz) = (32.0 * cell.0 as f32, 32.0 * cell.2 as f32);
    let (sx, sz) = size_m;
    let (ox, oz) = match dir & 3 {
        0 => (x, z),
        1 => (sz - z, x),
        2 => (sx - x, sz - z),
        _ => (z, sx - x),
    };
    [bx + ox, bz + oz]
}

fn turn_pt(cell: (i32, i32, i32), dir: u8, x: f32, z: f32) -> [f32; 2] {
    let (bx, bz) = (32.0 * cell.0 as f32, 32.0 * cell.2 as f32);
    let (ox, oz) = match dir & 3 {
        0 => (x, z),
        1 => (32.0 - z, x),
        2 => (32.0 - x, 32.0 - z),
        _ => (z, 32.0 - x),
    };
    [bx + ox, bz + oz]
}

/// A rect in the block frame (dir 0) → the world frame of a placement at `cell` with `dir` (the cell corner
/// turned by dir: (0,0) (32,0) (32,32) (0,32) in x,z for dir 0..3, yaw −dir·90°).
fn turn_rect(cell: (i32, i32, i32), dir: u8, x0: f32, x1: f32, z0: f32, z1: f32) -> (f32, f32, f32, f32) {
    let (bx, bz) = (32.0 * cell.0 as f32, 32.0 * cell.2 as f32);
    // local (x, z) → world offsets within the cell for each dir
    let f = |x: f32, z: f32| -> (f32, f32) {
        match dir & 3 {
            0 => (x, z),
            1 => (32.0 - z, x),
            2 => (32.0 - x, 32.0 - z),
            _ => (z, 32.0 - x),
        }
    };
    let (ax, az) = f(x0, z0);
    let (bx2, bz2) = f(x1, z1);
    (bx + ax.min(bx2), bx + ax.max(bx2), bz + az.min(bz2), bz + az.max(bz2))
}

/// Every water rectangle of the map in the world frame: the OPEN-WATER tiles (authored AND baked — BlueBay bakes
/// its `Sea` under the editor's auto-terrain; RedIsland/WhiteShore/GreenCoast author theirs) as full cells on the
/// bed, plus the shore tiles' water parts (measured per flags kind, turned by the placement's dir).
pub fn water_rects(store: &mut crate::store::DataStore, m: &MapFile, collection: u32, shores: bool) -> Result<(Vec<WorldRect>, usize, usize, Vec<String>), String> {
    let (tiles, bed) = water_tiles(collection);
    if tiles.is_empty() {
        return Err(format!("collection {collection:#x}: no open-water tile table (vista collections only)"));
    }
    let ground = tmmaps::map::ground_y(collection);
    // the shore tiles' DEEP water squares (Hugo 2026-10-24 00:12: the same-depth channels between banks finish);
    // `shores = false` turns them off (the 10-23 09:51 open-water-only form)
    let shore: &[&str] = if shores { shore_tiles(collection) } else { &[] };
    let mut out: Vec<WorldRect> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut n_open = 0usize;
    let mut n_shore = 0usize;
    let mut shore_cache: BTreeMap<(String, u32), Option<WaterRect>> = BTreeMap::new();
    let mut seen_open: std::collections::HashSet<(i32, i32, i32)> = Default::default();
    for b in m.blocks.iter().chain(m.baked.iter()).filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none()) {
        let c = b.coords();
        if tiles.iter().any(|t| t.eq_ignore_ascii_case(&b.name)) {
            if !seen_open.insert(c) {
                continue;
            }
            out.push(WorldRect { x0: 32.0 * c.0 as f32, x1: 32.0 * (c.0 + 1) as f32, z0: 32.0 * c.2 as f32, z1: 32.0 * (c.2 + 1) as f32, y_bed: ground + 8.0 * c.1 as f32 + bed, floor_y: ground + 8.0 * c.1 as f32, full_cell: true, tris: Vec::new() });
            n_open += 1;
        } else if shore.iter().any(|t| t.eq_ignore_ascii_case(&b.name)) {
            let key = (b.name.clone(), b.flags);
            if !shore_cache.contains_key(&key) {
                let r = water_rect(store, collection, &b.name, b.flags).unwrap_or_else(|e| { notes.push(format!("{}: {e}", b.name)); None });
                shore_cache.insert(key.clone(), r);
            }
            if let Some(Some(w)) = shore_cache.get(&key) {
                // the near-floor squares of every unit cell of the block (a Deco bay spans several), each as its own 4 m
                // square with the LOCAL bed under it; the block frame turned with the footprint size (grid_block rule)
                let cells = cell_footprints(w, bed);
                if cells.iter().all(|(_, _, d)| d.iter().all(|s| s.is_none())) { continue; }
                let size_m = (32.0 * w.size.0 as f32, 32.0 * w.size.1 as f32);
                for ((ux, uz), _, deep) in &cells {
                    for zi in 0..8 {
                    for xi in 0..8 {
                        let Some(_y_local) = deep[zi * 8 + xi] else { continue };
                        // v4.3: the square's own bed is NOT used — every box of the water row sits on the open bed
                        let y_local = bed;
                        let (lx0, lz0) = (32.0 * *ux as f32 + 4.0 * xi as f32, 32.0 * *uz as f32 + 4.0 * zi as f32);
                        let (lx1, lz1) = (lx0 + 4.0, lz0 + 4.0);
                        let tp = |x: f32, z: f32| turn_pt_sized(c, b.dir, size_m, x, z);
                        let tris = vec![
                            [tp(lx0, lz0), tp(lx1, lz0), tp(lx1, lz1)],
                            [tp(lx0, lz0), tp(lx1, lz1), tp(lx0, lz1)],
                        ];
                        let xs = [tris[0][0][0], tris[0][1][0], tris[0][2][0], tris[1][2][0]];
                        let zs = [tris[0][0][1], tris[0][1][1], tris[0][2][1], tris[1][2][1]];
                        out.push(WorldRect { x0: xs.iter().cloned().fold(f32::INFINITY, f32::min), x1: xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max), z0: zs.iter().cloned().fold(f32::INFINITY, f32::min), z1: zs.iter().cloned().fold(f32::NEG_INFINITY, f32::max), y_bed: ground + 8.0 * c.1 as f32 + y_local, floor_y: ground + 8.0 * c.1 as f32, full_cell: false, tris });
                    }
                    }
                }
                n_shore += 1;
            }
        }
    }
    for ((name, flags), r) in &shore_cache {
        match r {
            Some(w) => {
                let cells = cell_footprints(w, bed);
                let (wsq, dsq) = cells.iter().fold((0u32, 0usize), |(a, b), (_, m, d)| (a + m.count_ones(), b + d.iter().filter(|s| s.is_some()).count()));
                notes.push(format!("{name} flags {flags:08x}: {} ({}×{} cells) water x {:.2}..{:.2} z {:.2}..{:.2} (block frame), surface {:.2}, bed {:.2}..{:.2}, water squares {wsq} of which DEEP {dsq}", w.prefab, w.size.0, w.size.1, w.x0, w.x1, w.z0, w.z1, w.surface, w.bed_lo, w.bed_hi))
            }
            None => notes.push(format!("{name} flags {flags:08x}: no water quad")),
        }
    }
    Ok((out, n_open, n_shore, notes))
}

/// Hugo's Floor Finish on STADIUM (2026-10-23, his final rule of 10:35 PT): the grass floor is the finishable
/// area; "if the block placement causes the grass to disappear, it should not have a finish on it" — the
/// occupied columns are the footprints of the AUTHORED blocks that remove the grass: every block at the BASE
/// ROW (IsGround set — all of them sit there — or an air-variant piece standing at ground level, e.g. the
/// DecoWall*Grass walls), every unit cell of the placed variant turned by the block's dir in our standard frame,
/// the Grass tiles themselves excepted by name. Air blocks above the base row leave the grass and do not
/// occupy; items do not ("items that touch the grass are fine to be ignored"); the baked FC/pillar pieces are
/// not authored and do not. The editor's own baked `Grass` list is NOT the free set: every Fall Stadium source
/// bakes exactly 48 × 48 = 2 304 Grass tiles with identical flags, under the blocks too — so the footprint
/// maths is needed. The grass plane of the base row: Stadium ground −64, Grass at cy 9, zone plane local +2 →
/// world y 10.00 (the sources' SupportTube items standing on the grass are at y 10.00).
pub struct StadiumFloor {
    /// grass cells whose tile the game HIDES (a ground-variant authored block's turned unit covers them)
    pub hidden: std::collections::HashSet<(i32, i32)>,
    /// the grass cells
    pub grass: std::collections::BTreeSet<(i32, i32)>,
    pub ground_blocks: usize,
    pub grass_y: f32,
    /// block name → (size x, size z) in cells at dir 0
    pub sizes: BTreeMap<String, (i32, i32)>,
    /// per cell the OCCUPIED 4 m squares (bit zi·8+xi, world cell frame): structure a car can clip into
    pub occupied: BTreeMap<(i32, i32), u64>,
    /// per cell the GREEN squares: a collidable grass surface a block or baked filler brings into a hidden cell
    pub green: BTreeMap<(i32, i32), u64>,
    pub notes: Vec<String>,
}

/// The unit cells of the variant a placement's flags pick (dir 0, cell units); `[0,0,0]` when the info has none.
pub fn block_units(store: &mut crate::store::DataStore, name: &str, flags: u32) -> Vec<[i32; 3]> {
    let mut units: Vec<[i32; 3]> = vec![[0, 0, 0]];
    if let Some(bp) = crate::vista::find_block_info(store, name) {
        if let Ok(bi) = crate::blockinfo::load(store, &bp) {
            let ground_v = flags & crate::blockmap::FLAG_GROUND != 0;
            let variant = (flags & 63) as usize;
            let sub = ((flags >> crate::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
            let addv = ((flags >> crate::blockmap::FLAG_ADDITIONAL_SHIFT) & 0x7F) as usize;
            if let Some(p) = bi.pick_placement_add(ground_v, variant, sub, addv) {
                if !p.variant.block_units.is_empty() {
                    units = p.variant.block_units.iter().map(|u| u.offset).collect();
                }
            }
        }
    }
    units
}

/// A unit cell at dir 0 → the world cell of a placement at `c` with `dir`: the footprint spans `size` cells
/// (x, z) and turns about the placement cell's corner — the (0,0) / (W,0) / (W,D) / (0,D) rule in cell units.
pub fn turn_unit(c: (i32, i32, i32), dir: u8, u: [i32; 3], size: (i32, i32)) -> (i32, i32) {
    let (sx, sz) = size;
    match dir & 3 {
        0 => (c.0 + u[0], c.2 + u[2]),
        1 => (c.0 + (sz - 1 - u[2]), c.2 + u[0]),
        2 => (c.0 + (sx - 1 - u[0]), c.2 + (sz - 1 - u[2])),
        _ => (c.0 + u[2], c.2 + (sx - 1 - u[0])),
    }
}

/// The 4 m sub-cell footprint of a block VARIANT (Hugo, 2026-10-23 12:10: the grass between a TrackWall
/// pillar's thin walls and inside a road loop must finish): per base-row unit cell a 64-bit mask (bit zi·8+xi =
/// the 4 m square at local x 4·xi.., z 4·zi.. of that cell) of the geometry WITHIN THE BASE UNIT ROW — every
/// collision and visual triangle of the variant's mobils whose lowest vertex is under `touch_y` (8 m, the unit
/// height: a road deck at 2–2.8 m replaces the grass under it just like a wall standing on it) — rasterised
/// conservatively: a square is occupied when any such triangle covers its centre OR any of its edges crosses the
/// square (thin walls have ~no area). Squares nothing touches are free grass. The whole-cell table stays for
/// variants that fail to bake (returned as full masks).
/// Per base cell `(occupied, standable)`: `occupied` = geometry within the base unit row other than the
/// grass-level floor (decks, borders, walls, pillars, decals — anything a car can clip into); `standable` =
/// upward-facing collision within ±0.3 m of local 0, any physics id — a ground fill the block itself (or a
/// baked filler) brings into a cell whose Grass tile the game hides: the car can stand there.
/// Round 3 (Hugo 2026-10-23 12:55: a car clipping through a road block and falling into the VOID must never
/// finish): the Grass tile is HIDDEN under every unit of a ground-variant block (read off the game's lightmaps,
/// tmmaps::tiny::tiles::HiddenTiles, 93 440 tile records), geometry notwithstanding — so inside such cells only
/// `green` squares are grass; everything else there is void or structure.
fn is_up_debug(upward: bool, ylo: f32, yhi: f32) -> bool { upward && yhi - ylo < 0.5 }

pub fn variant_masks(store: &mut crate::store::DataStore, variant: &crate::blockinfo::Variant, touch_y: f32) -> Result<BTreeMap<(i32, i32), (u64, u64)>, String> {
    let mut m = crate::static_item::build::Merged::default();
    m.keep_water = true;
    let mut n_mobils = 0usize;
    for list in &variant.mobils {
        for mobil in list {
            let Some(prefab) = mobil.prefab.as_deref() else { continue };
            let low = prefab.to_ascii_lowercase();
            if low.ends_with(".vegettreemodel.gbx") { continue; }
            let mut at = crate::geom::IDENTITY;
            if let Some(t) = mobil.translation { at[9] = t[0]; at[10] = t[1]; at[11] = t[2]; }
            let r = if low.ends_with(".prefab.gbx") { crate::static_item::build::add_prefab(store, prefab, &at, 1.0, &mut m, 0) } else { crate::static_item::build::add_static_object_file(store, prefab, &at, 1.0, &mut m) };
            if r.is_ok() { n_mobils += 1; }
        }
    }
    // the units' BOTTOM clips: the `*FCBGround` ground fills (TrackToGrassCurve2FCBGround: the grass lobes around a
    // curve, Green 76 collision at y 0 — Hugo 2026-10-23 13:56 "it stands on it"). The clip sits one unit under the
    // block's unit; its mobil carries translation +8, so baked at the unit's x/z offset and y −8 it lands at the base
    let debug = std::env::var_os("FLOOR_DEBUG").is_some();
    for u in &variant.block_units {
        for clip in &u.clips[5] {
            let ci = match crate::blockinfo::load(store, clip) {
                Ok(ci) => ci,
                Err(e) => { if debug { eprintln!("clip {clip}: load failed: {e}"); } continue }
            };
            if debug { eprintln!("clip {clip}: air {} ground {}", ci.variant_base_air.as_ref().map(|v| v.mobils.iter().map(|l| l.len()).sum::<usize>()).unwrap_or(0), ci.variant_base_ground.as_ref().map(|v| v.mobils.iter().map(|l| l.len()).sum::<usize>()).unwrap_or(0)); }
            let Some(cv) = ci.variant_base_air.as_ref().or(ci.variant_base_ground.as_ref()) else { continue };
            for list in &cv.mobils {
                for mobil in list {
                    let Some(prefab) = mobil.prefab.as_deref() else { continue };
                    let low = prefab.to_ascii_lowercase();
                    if !low.ends_with(".prefab.gbx") { continue; }
                    let mut at = crate::geom::IDENTITY;
                    at[9] = 32.0 * u.offset[0] as f32;
                    at[10] = 8.0 * (u.offset[1] - 1) as f32;
                    at[11] = 32.0 * u.offset[2] as f32;
                    if let Some(t) = mobil.translation { at[9] += t[0]; at[10] += t[1]; at[11] += t[2]; }
                    match crate::static_item::build::add_prefab(store, prefab, &at, 1.0, &mut m, 0) {
                        Ok(()) => { n_mobils += 1; if debug { eprintln!("  baked {prefab} at {:?}: surf tris now {}", [at[9], at[10], at[11]], m.surf_triangles.len()); } }
                        Err(e) => { if debug { eprintln!("  {prefab}: {e}"); } }
                    }
                }
            }
        }
    }
    // the base-row cells
    let mut masks: BTreeMap<(i32, i32), (u64, u64)> = BTreeMap::new();
    for u in &variant.block_units {
        if u.offset[1] == 0 { masks.insert((u.offset[0], u.offset[2]), (0, 0)); }
    }
    if masks.is_empty() { masks.insert((0, 0), (0, 0)); }
    if n_mobils == 0 {
        // nothing to bake (procedural terrain / pillar fillers): the whole footprint stands, no grass
        for v in masks.values_mut() { *v = (u64::MAX, 0); }
        return Ok(masks);
    }
    // triangles within the base unit row (block frame, dir 0): collision + visuals; grass-level grass-physics
    // collision goes to `green`, everything else to `occupied`
    let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
    let mut green_tris: Vec<[[f32; 3]; 3]> = Vec::new();
    for t in &m.surf_triangles {
        let pts: Vec<[f32; 3]> = t.indices.iter().filter_map(|&i| m.surf_vertices.get(i as usize).copied()).collect();
        if pts.len() != 3 { continue; }
        let (ylo, yhi) = (pts.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min), pts.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max));
        // round 4 (Hugo 13:51): a STANDABLE grass-level surface of ANY physics id — an upward-facing collision
        // triangle within ±0.3 m of local 0 (the block's own ground fill: grass, dirt, sand, asphalt aprons)
        let n = {
            let (a, b, c) = (pts[0], pts[1], pts[2]);
            let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
        };
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let upward = len > 1e-6 && n[1] / len > 0.7;
        if upward && ylo > -0.3 && yhi < 0.3 {
            green_tris.push([pts[0], pts[1], pts[2]]);
        } else if ylo < touch_y && yhi > 0.3 {
            tris.push([pts[0], pts[1], pts[2]]);
        }
        if std::env::var_os("FLOOR_DEBUG").is_some() && is_up_debug(upward, ylo, yhi) {
            eprintln!("  surf tri phys {} y {:.2}..{:.2} upward {upward}", t.material_id, ylo, yhi);
        }
    }
    // visuals: structure only when they rise above the grass-level floor (a ground fill's grass visual lies
    // flat at y ≈ 0 and is the floor itself, not something to clip into); decals a few cm up count as floor too
    for mv in &m.visuals {
        let Some(main) = mv.visual.main.as_ref() else { continue };
        let Some(crate::static_item::Node::VertexStream(stream)) = main.vertex_streams.first().and_then(|r| r.inline.as_deref()) else { continue };
        let compress = stream.compress_local3d.unwrap_or(false);
        let mut pos: &[[f32; 3]] = &[];
        for (d, e) in stream.decls.iter().zip(stream.elems.iter()) {
            if let (crate::static_item::vstream::N_POSITION, crate::static_item::vstream::T_FLOAT3, crate::static_item::vstream::Elem::Float3(p)) = (d.name(), d.stored_type(compress), e) { pos = p; }
        }
        if let Some(ib) = mv.visual.index_buffer.as_ref() {
            for t in ib.indices.chunks(3) {
                if t.len() == 3 && t.iter().all(|&i| (i as usize) < pos.len()) {
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let (ylo, yhi) = (p.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min), p.iter().map(|v| v[1]).fold(f32::NEG_INFINITY, f32::max));
                    if ylo < touch_y && yhi > 0.3 { tris.push(p); }
                }
            }
        }
    }
    let inside = |p: [f32; 2], t: &[[f32; 3]; 3]| -> bool {
        let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
        let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
        let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    };
    // segment–square overlap (Liang–Barsky style clip test)
    let seg_hits = |a: [f32; 2], b: [f32; 2], x0: f32, z0: f32, x1: f32, z1: f32| -> bool {
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        for (p, q) in [(-dx, a[0] - x0), (dx, x1 - a[0]), (-dz, a[1] - z0), (dz, z1 - a[1])] {
            if p == 0.0 { if q < 0.0 { return false; } continue; }
            let r = q / p;
            if p < 0.0 { if r > t1 { return false; } if r > t0 { t0 = r; } } else { if r < t0 { return false; } if r < t1 { t1 = r; } }
        }
        true
    };
    let covers = |list: &[[[f32; 3]; 3]], x0: f32, z0: f32, x1: f32, z1: f32, conservative: bool| -> bool {
        let centre = [x0 + 2.0, z0 + 2.0];
        list.iter().any(|t| {
            let (tx0, tx1) = (t.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max));
            let (tz0, tz1) = (t.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[2]).fold(f32::NEG_INFINITY, f32::max));
            if tx1 < x0 || tx0 > x1 || tz1 < z0 || tz0 > z1 { return false; }
            inside(centre, t) || (conservative && (0..3).any(|k| seg_hits([t[k][0], t[k][2]], [t[(k + 1) % 3][0], t[(k + 1) % 3][2]], x0, z0, x1, z1)))
        })
    };
    for ((ux, uz), (occ, green)) in masks.iter_mut() {
        let (cx0, cz0) = (32.0 * *ux as f32, 32.0 * *uz as f32);
        for zi in 0..8 {
            for xi in 0..8 {
                let (x0, z0) = (cx0 + 4.0 * xi as f32, cz0 + 4.0 * zi as f32);
                let (x1, z1) = (x0 + 4.0, z0 + 4.0);
                // occupied: conservative (centre or any edge); green: the square's centre must lie on grass
                if covers(&tris, x0, z0, x1, z1, true) { *occ |= 1u64 << (zi * 8 + xi); }
                if covers(&green_tris, x0, z0, x1, z1, false) { *green |= 1u64 << (zi * 8 + xi); }
            }
        }
    }
    if std::env::var_os("FLOOR_DEBUG").is_some() {
        eprintln!("  masks: {} structure tris, {} standable tris → {:?}", tris.len(), green_tris.len(), masks.iter().map(|(c, (o, g))| format!("{c:?}:{o:016x}:{g:016x}")).collect::<Vec<_>>());
    }
    Ok(masks)
}

pub fn stadium_floor(store: &mut crate::store::DataStore, m: &MapFile) -> Result<StadiumFloor, String> {
    let collection = 0x1a;
    let ground = tmmaps::map::ground_y(collection);
    let mut grass: std::collections::BTreeSet<(i32, i32)> = Default::default();
    let mut grass_rows: std::collections::BTreeSet<i32> = Default::default();
    for b in m.baked.iter().chain(m.blocks.iter()).filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none()) {
        if b.name.eq_ignore_ascii_case("Grass") {
            let c = b.coords();
            grass.insert((c.0, c.2));
            grass_rows.insert(c.1);
        }
    }
    if grass.is_empty() {
        return Err("no Grass tile in the map (authored or baked)".into());
    }
    let base_row = *grass_rows.iter().next().unwrap();
    // the Grass tile's collision quad is at local 0.00 (Zone\Grass\Base: 8 tris, 1 024 m², measured 2026-10-23 12:10;
    // the vista zone planes sit at +2 — Stadium's does not)
    let grass_y = ground + 8.0 * base_row as f32;
    let mut notes = vec![format!("{} grass cells on row {} (rows seen {:?}); grass plane world y {grass_y:.2}", grass.len(), base_row, grass_rows)];
    let mut hidden: std::collections::HashSet<(i32, i32)> = Default::default();
    let mut occupied: BTreeMap<(i32, i32), u64> = BTreeMap::new();
    let mut green: BTreeMap<(i32, i32), u64> = BTreeMap::new();
    let mut unit_cache: BTreeMap<(String, u32), Vec<[i32; 3]>> = BTreeMap::new();
    let mut mask_cache: BTreeMap<(String, u32), BTreeMap<(i32, i32), (u64, u64)>> = BTreeMap::new();
    let mut sizes: BTreeMap<String, (i32, i32)> = BTreeMap::new();
    let (mut ground_blocks, mut ghost_ground, mut air_at_base, mut baked_fillers, mut overlaps) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut base_claims: std::collections::HashSet<(i32, i32)> = Default::default();
    // authored blocks first (hiding + geometry), then the baked fillers at the base row (geometry only)
    for (authored, b) in m.blocks.iter().map(|b| (true, b)).chain(m.baked.iter().map(|b| (false, b))).filter(|(_, b)| b.flags != 0xFFFF_FFFF && b.free_pos.is_none()) {
        if b.name.eq_ignore_ascii_case("Grass") {
            continue;
        }
        let c = b.coords();
        let is_ground = b.flags & crate::blockmap::FLAG_GROUND != 0;
        let is_ghost = b.flags & 0x1000_0000 != 0;
        if !(is_ground || c.1 == base_row) {
            continue;
        }
        let key = (b.name.clone(), b.flags);
        let units = match unit_cache.get(&key) {
            Some(u) => u.clone(),
            None => {
                let u = block_units(store, &b.name, b.flags);
                unit_cache.insert(key.clone(), u.clone());
                u
            }
        };
        let size = (units.iter().map(|u| u[0]).max().unwrap_or(0) + 1, units.iter().map(|u| u[2]).max().unwrap_or(0) + 1);
        if authored {
            ground_blocks += 1;
            if is_ground && is_ghost { ghost_ground += 1; }
            if !is_ground { air_at_base += 1; }
            sizes.entry(b.name.clone()).and_modify(|s| { s.0 = s.0.max(size.0); s.1 = s.1.max(size.1); }).or_insert(size);
            // THE HIDING RULE (the game's lightmaps, tmmaps::tiny::tiles): a ground-variant authored block hides
            // the tile under every turned unit, ghost or not; an air variant hides nothing
            if is_ground {
                for u in &units {
                    // every unit hides the tile of its column (Summer 12's Dirt under a slope's TOP unit stays
                    // hidden — tmmaps::tiny::tiles); the turn check counts only the base-row units, which two
                    // ground blocks never share
                    let cell = turn_unit(c, b.dir, *u, size);
                    if grass.contains(&cell) {
                        hidden.insert(cell);
                        // the turn check: two ground blocks' BASE-ROW units never share a 3-D cell
                        if u[1] == 0 && c.1 == base_row && !base_claims.insert(cell) {
                            overlaps += 1;
                        }
                    }
                }
            }
        } else {
            baked_fillers += 1;
        }
        // geometry: the sub-cell masks of the placement kind, turned by dir
        let masks = match mask_cache.get(&key) {
            Some(mm) => mm.clone(),
            None => {
                let mm = placement_masks(store, &b.name, b.flags).unwrap_or_default();
                mask_cache.insert(key.clone(), mm.clone());
                mm
            }
        };
        for (&(ux, uz), &(occ, grn)) in &masks {
            let cell = turn_unit(c, b.dir, [ux, 0, uz], size);
            if !grass.contains(&cell) { continue; }
            *occupied.entry(cell).or_default() |= turn_mask(occ, b.dir);
            *green.entry(cell).or_default() |= turn_mask(grn, b.dir);
        }
    }
    notes.push(format!("{ground_blocks} authored blocks at the base row or IsGround ({ghost_ground} ghost ground blocks hide like the others, {air_at_base} air variants hide nothing), {} distinct names, {baked_fillers} baked fillers at the base row (geometry only); hidden grass cells {}, kept {}; {overlaps} unit cells claimed by two ground blocks (turn check: 0 expected)", sizes.len(), hidden.len(), grass.len() - hidden.len()));
    let full: usize = grass.iter().filter(|c| occupied.get(c).copied().unwrap_or(0) == u64::MAX).count();
    let green_cells: usize = hidden.iter().filter(|c| green.get(c).copied().unwrap_or(0) != 0).count();
    let green_sq: u32 = hidden.iter().map(|c| (green.get(c).copied().unwrap_or(0) & !occupied.get(c).copied().unwrap_or(0)).count_ones()).sum();
    let void_sq: u32 = hidden.iter().map(|c| (!(green.get(c).copied().unwrap_or(0) | occupied.get(c).copied().unwrap_or(0))).count_ones()).sum();
    let occ_hidden_sq: u32 = hidden.iter().map(|c| occupied.get(c).copied().unwrap_or(0).count_ones()).sum();
    notes.push(format!("hidden cells' 4 m squares: {occ_hidden_sq} structure (a), {green_sq} collidable grass fill (b) in {green_cells} cells, {void_sq} VOID (c); {full} cells fully structure"));
    Ok(StadiumFloor { hidden, grass, ground_blocks, grass_y, sizes, occupied, green, notes })
}

/// The sub-cell masks of a placement kind: the variant its flags pick, baked (`variant_masks`); full masks when
/// the info is unknown.
pub fn placement_masks(store: &mut crate::store::DataStore, name: &str, flags: u32) -> Result<BTreeMap<(i32, i32), (u64, u64)>, String> {
    let Some(bp) = crate::vista::find_block_info(store, name) else { return Err(format!("{name}: no block info")) };
    let bi = crate::blockinfo::load(store, &bp)?;
    let ground_v = flags & crate::blockmap::FLAG_GROUND != 0;
    let variant = (flags & 63) as usize;
    let sub = ((flags >> crate::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
    let addv = ((flags >> crate::blockmap::FLAG_ADDITIONAL_SHIFT) & 0x7F) as usize;
    let Some(p) = bi.pick_placement_add(ground_v, variant, sub, addv) else { return Err(format!("{name}: no variant for flags {flags:#x}")) };
    // bake only the mobils the placement draws (the picked list), on the variant's units
    let mut v = p.variant.clone();
    v.mobils = vec![p.mobils.iter().map(|m| (*m).clone()).collect()];
    variant_masks(store, &v, 8.0)
}

/// Turn a cell's 8×8 mask (bit zi·8+xi, dir 0) by the placement dir — the same frame as `turn_unit`: dir 1 maps
/// local (x, z) → (W − z, x) with W = 32 m → square (xi, zi) → (7 − zi, xi).
pub fn turn_mask(mask: u64, dir: u8) -> u64 {
    if dir & 3 == 0 { return mask; }
    let mut out = 0u64;
    for zi in 0..8 {
        for xi in 0..8 {
            if mask & (1u64 << (zi * 8 + xi)) == 0 { continue; }
            let (nx, nz) = match dir & 3 {
                1 => (7 - zi, xi),
                2 => (7 - xi, 7 - zi),
                _ => (zi, 7 - xi),
            };
            out |= 1u64 << (nz * 8 + nx);
        }
    }
    out
}

/// Hugo's round-4 design (2026-10-23 13:51): the MAIN PLANE = the grass cells the game keeps (whole cells, row
/// strips) and one CUTOUT item per block kind whose ground fill leaves standable grass inside its hidden cells.
/// The cutout trigger follows the fill's own collision outline: every standable triangle (upward, |y| < 0.3)
/// shrunk toward its centroid by `inset` metres, dropped when its footprint comes within `inset` of a structure
/// triangle's footprint, extruded `below`..`above` — in the block frame at dir 0 (the game's placement turns it).
/// The flag bits that pick a block's drawn variant: the low 6 (variant), 6..11 (subvariant), 12 (ground) and
/// 21..27 (additional) — the placement KIND for cutouts.
pub const VARIANT_FLAG_MASK: u32 = 0x3F | (0x3F << 6) | crate::blockmap::FLAG_GROUND | (0x7F << crate::blockmap::FLAG_ADDITIONAL_SHIFT);

/// GBX.NET's `CGameCtnBlock.Variant` byte for our flags: the low 6 bits the variant index, the high 2 bits… the
/// Variant byte GBX.NET exposes is `(Flags >> 0) & 0x3F` for the base variant; the additional index lives in
/// bits 21..27 and is what distinguishes `ground/add0` (Fall 05's DecoHill* at 00201000 → add 1). We encode a
/// kind's variant as `additional * 64 + variant` so one byte-ish number names it in the item file.
pub fn variant_byte(flags: u32) -> u32 {
    (flags & 0x3F) + 64 * ((flags >> crate::blockmap::FLAG_ADDITIONAL_SHIFT) & 0x7F)
}

pub fn flags_of_variant(var: u32) -> u32 {
    (var & 0x3F) | ((var / 64) << crate::blockmap::FLAG_ADDITIONAL_SHIFT) | crate::blockmap::FLAG_GROUND
}

pub struct Cutout {
    pub verts: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
    /// boxes in the item / 1 m squares dropped near structure
    pub kept: usize,
    pub dropped: usize,
    /// standable area kept, m²
    pub area: f32,
    pub bbox: ([f32; 3], [f32; 3]),
}

/// Bake a block's base variant (+ its units' Bottom clips) and split the triangles within the base unit row into
/// standable floor and structure, in the block frame.
pub fn variant_floor_and_structure(store: &mut crate::store::DataStore, variant: &crate::blockinfo::Variant) -> Result<(Vec<[[f32; 3]; 3]>, Vec<[[f32; 3]; 3]>), String> {
    let mut m = crate::static_item::build::Merged::default();
    m.keep_water = true;
    for list in &variant.mobils {
        for mobil in list {
            let Some(prefab) = mobil.prefab.as_deref() else { continue };
            let low = prefab.to_ascii_lowercase();
            if low.ends_with(".vegettreemodel.gbx") { continue; }
            let mut at = crate::geom::IDENTITY;
            if let Some(t) = mobil.translation { at[9] = t[0]; at[10] = t[1]; at[11] = t[2]; }
            let _ = if low.ends_with(".prefab.gbx") { crate::static_item::build::add_prefab(store, prefab, &at, 1.0, &mut m, 0) } else { crate::static_item::build::add_static_object_file(store, prefab, &at, 1.0, &mut m) };
        }
    }
    for u in &variant.block_units {
        for clip in &u.clips[5] {
            let Ok(ci) = crate::blockinfo::load(store, clip) else { continue };
            let Some(cv) = ci.variant_base_air.as_ref().or(ci.variant_base_ground.as_ref()) else { continue };
            for list in &cv.mobils {
                for mobil in list {
                    let Some(prefab) = mobil.prefab.as_deref() else { continue };
                    if !prefab.to_ascii_lowercase().ends_with(".prefab.gbx") { continue; }
                    let mut at = crate::geom::IDENTITY;
                    at[9] = 32.0 * u.offset[0] as f32;
                    at[10] = 8.0 * (u.offset[1] - 1) as f32;
                    at[11] = 32.0 * u.offset[2] as f32;
                    if let Some(t) = mobil.translation { at[9] += t[0]; at[10] += t[1]; at[11] += t[2]; }
                    let _ = crate::static_item::build::add_prefab(store, prefab, &at, 1.0, &mut m, 0);
                }
            }
        }
    }
    let mut floor: Vec<[[f32; 3]; 3]> = Vec::new();
    let mut structure: Vec<[[f32; 3]; 3]> = Vec::new();
    for t in &m.surf_triangles {
        let pts: Vec<[f32; 3]> = t.indices.iter().filter_map(|&i| m.surf_vertices.get(i as usize).copied()).collect();
        if pts.len() != 3 { continue; }
        let (ylo, yhi) = (pts.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min), pts.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max));
        let (a, b, c) = (pts[0], pts[1], pts[2]);
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let upward = len > 1e-6 && n[1] / len > 0.7;
        if upward && ylo > -0.3 && yhi < 0.3 {
            floor.push([a, b, c]);
        } else if ylo < 8.0 && yhi > 0.3 {
            structure.push([a, b, c]);
        }
    }
    for mv in &m.visuals {
        let Some(main) = mv.visual.main.as_ref() else { continue };
        let Some(crate::static_item::Node::VertexStream(stream)) = main.vertex_streams.first().and_then(|r| r.inline.as_deref()) else { continue };
        let compress = stream.compress_local3d.unwrap_or(false);
        let mut pos: &[[f32; 3]] = &[];
        for (d, e) in stream.decls.iter().zip(stream.elems.iter()) {
            if let (crate::static_item::vstream::N_POSITION, crate::static_item::vstream::T_FLOAT3, crate::static_item::vstream::Elem::Float3(p)) = (d.name(), d.stored_type(compress), e) { pos = p; }
        }
        if let Some(ib) = mv.visual.index_buffer.as_ref() {
            for t in ib.indices.chunks(3) {
                if t.len() == 3 && t.iter().all(|&i| (i as usize) < pos.len()) {
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let (ylo, yhi) = (p.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min), p.iter().map(|v| v[1]).fold(f32::NEG_INFINITY, f32::max));
                    if ylo < 8.0 && yhi > 0.3 { structure.push(p); }
                }
            }
        }
    }
    Ok((floor, structure))
}

/// Distance in xz from a point to a triangle's footprint (0 inside).
fn dist_xz(p: [f32; 2], t: &[[f32; 3]; 3]) -> f32 {
    let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
    let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    if !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0)) {
        return 0.0;
    }
    let seg = |a: [f32; 2], b: [f32; 2]| -> f32 {
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dz * dz;
        let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let (qx, qz) = (a[0] + t * dx, a[1] + t * dz);
        ((p[0] - qx).powi(2) + (p[1] - qz).powi(2)).sqrt()
    };
    seg(a, b).min(seg(b, c)).min(seg(c, a))
}

/// The cutout of a PLACEMENT KIND (name + flags): the variant the flags pick (base / additional, the mobil list
/// drawn), as the game places it — a block kind placed with two variants gets two items.
pub fn cutout_for_block(store: &mut crate::store::DataStore, name: &str, flags: u32, inset: f32, below: f32, above: f32) -> Result<Option<Cutout>, String> {
    let Some(bp) = crate::vista::find_block_info(store, name) else { return Err(format!("{name}: no block info")) };
    let bi = crate::blockinfo::load(store, &bp)?;
    let ground_v = flags & crate::blockmap::FLAG_GROUND != 0;
    let variant = (flags & 63) as usize;
    let sub = ((flags >> crate::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
    let addv = ((flags >> crate::blockmap::FLAG_ADDITIONAL_SHIFT) & 0x7F) as usize;
    let Some(p) = bi.pick_placement_add(ground_v, variant, sub, addv) else { return Ok(None) };
    let mut v = p.variant.clone();
    v.mobils = vec![p.mobils.iter().map(|m| (*m).clone()).collect()];
    let (floor, structure) = variant_floor_and_structure(store, &v)?;
    if std::env::var_os("FLOOR_DEBUG").is_some() {
        eprintln!("cutout {name} {flags:08x}: variant {} units {} mobils {}; floor tris {} structure tris {}", p.label, v.block_units.len(), v.mobils.iter().map(|l| l.len()).sum::<usize>(), floor.len(), structure.len());
    }
    if floor.is_empty() {
        return Ok(None);
    }
    // a 1 m raster over the variant's footprint: a square is FLOOR when its centre lies on a standable triangle,
    // and kept only when no structure triangle's footprint comes within `inset` of the square (its centre's
    // distance to the triangle < inset + 0.71 m, the half-diagonal) — the true curved outline at 1 m, inset
    let sb: Vec<([f32; 2], [f32; 2], &[[f32; 3]; 3])> = structure.iter().map(|t| ([t.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min)], [t.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max), t.iter().map(|v| v[2]).fold(f32::NEG_INFINITY, f32::max)], t)).collect();
    let reach = inset + 0.71;
    let near_structure = |p: [f32; 2]| -> bool {
        sb.iter().any(|(lo, hi, t)| p[0] >= lo[0] - reach && p[0] <= hi[0] + reach && p[1] >= lo[1] - reach && p[1] <= hi[1] + reach && dist_xz(p, t) < reach)
    };
    let fb: Vec<([f32; 2], [f32; 2], &[[f32; 3]; 3])> = floor.iter().map(|t| ([t.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min)], [t.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max), t.iter().map(|v| v[2]).fold(f32::NEG_INFINITY, f32::max)], t)).collect();
    let on_floor = |p: [f32; 2]| -> bool {
        fb.iter().any(|(lo, hi, t)| p[0] >= lo[0] && p[0] <= hi[0] && p[1] >= lo[1] && p[1] <= hi[1] && dist_xz(p, t) == 0.0)
    };
    let (sx, sz) = (v.block_units.iter().map(|u| u.offset[0] + 1).max().unwrap_or(1), v.block_units.iter().map(|u| u.offset[2] + 1).max().unwrap_or(1));
    let (nx, nz) = ((32 * sx) as usize, (32 * sz) as usize);
    let mut grid = vec![false; nx * nz];
    let (mut kept, mut dropped) = (0usize, 0usize);
    for zi in 0..nz {
        for xi in 0..nx {
            let c = [xi as f32 + 0.5, zi as f32 + 0.5];
            if !on_floor(c) { continue; }
            if near_structure(c) { dropped += 1; continue; }
            grid[zi * nx + xi] = true;
            kept += 1;
        }
    }
    if kept == 0 {
        return Ok(None);
    }
    // merge: x-runs per row, then equal consecutive rows
    let mut spans: Vec<(usize, usize, usize)> = Vec::new(); // (z, x0, x1)
    for zi in 0..nz {
        let mut xi = 0;
        while xi < nx {
            if !grid[zi * nx + xi] { xi += 1; continue; }
            let x0 = xi;
            while xi < nx && grid[zi * nx + xi] { xi += 1; }
            spans.push((zi, x0, xi));
        }
    }
    spans.sort_unstable_by_key(|&(z, x0, x1)| (x0, x1, z));
    let mut boxes: Vec<(f32, f32, f32, f32)> = Vec::new();
    let mut i = 0;
    while i < spans.len() {
        let (z0, x0, x1) = spans[i];
        let mut z1 = z0 + 1;
        while i + 1 < spans.len() && spans[i + 1].1 == x0 && spans[i + 1].2 == x1 && spans[i + 1].0 == z1 { i += 1; z1 += 1; }
        i += 1;
        boxes.push((x0 as f32, x1 as f32, z0 as f32, z1 as f32));
    }
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let quads: [[u32; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [1, 2, 6, 5], [0, 4, 7, 3]];
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    let (y0, y1) = (-below, above);
    for (x0, x1, z0, z1) in &boxes {
        let base = verts.len() as u32;
        verts.extend_from_slice(&[[*x0, y0, *z0], [*x1, y0, *z0], [*x1, y1, *z0], [*x0, y1, *z0], [*x0, y0, *z1], [*x1, y0, *z1], [*x1, y1, *z1], [*x0, y1, *z1]]);
        for q in quads {
            tris.push([base + q[0], base + q[1], base + q[2]]);
            tris.push([base + q[0], base + q[2], base + q[3]]);
        }
        for v in &verts[base as usize..] { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
    }
    let area = kept as f32;
    let _ = &v.block_units;
    let kept = boxes.len();
    Ok(Some(Cutout { verts, tris, kept, dropped, area, bbox: (lo, hi) }))
}

/// Build the cutout carrier for a block name: an invisible Finish item in the block frame.
/// The exact-outline cutout item (v4.4, Hugo's tester: "perfect cutouts of the blocks"): the fill region traced
/// as polygons (`cutout_outline::trace`), triangulated with holes, extruded into the trigger prism (−below ..
/// +above in the block frame) with the indicator as the same polygon mesh at +lift. Polygons under 1 m² or
/// thinner than a car (area / perimeter < 0.3 m) are dropped and reported in the Cutout's notes-ish `dropped`.
pub fn build_cutout_item_exact(store: &mut crate::store::DataStore, name: &str, flags: u32, ident: &str, collection: u32, inset: f32, below: f32, above: f32) -> Result<Option<(Vec<u8>, Cutout, crate::cutout_outline::Outline)>, String> {
    let (floor, structure) = floor_and_structure_of(store, name, flags)?;
    if floor.is_empty() {
        return Ok(None);
    }
    let size = variant_size_of(store, name, flags)?;
    let mut o = crate::cutout_outline::trace(&floor, &structure, (32.0 * size.0 as f32, 32.0 * size.1 as f32), inset, crate::cutout_outline::CONTOUR_STEP);
    let mut dropped_slivers = 0usize;
    o.polygons.retain(|(outer, holes)| {
        let area = polygon_area(outer) + holes.iter().map(|h| polygon_area(h)).sum::<f32>();
        let perim: f32 = (0..outer.len()).map(|i| { let (a, b) = (outer[i], outer[(i + 1) % outer.len()]); ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }).sum();
        let keep = area >= 1.0 && perim > 0.0 && area / perim >= 0.3;
        if !keep { dropped_slivers += 1; }
        keep
    });
    if o.polygons.is_empty() {
        return Ok(None);
    }
    o.area = o.polygons.iter().map(|(outer, holes)| polygon_area(outer) + holes.iter().map(|h| polygon_area(h)).sum::<f32>()).sum();
    o.vertices = o.polygons.iter().map(|(outer, holes)| outer.len() + holes.iter().map(|h| h.len()).sum::<usize>()).sum();
    if dropped_slivers > 0 {
        o.notes.push(format!("{dropped_slivers} sliver polygon(s) under 1 m² or thinner than 0.3 m area/perimeter dropped"));
    }
    let ((tv, tt), (iv, it)) = crate::cutout_outline::extrude(&o, below, above, above + INDICATOR_LIFT - above + INDICATOR_LIFT);
    // the indicator sits 2 cm over the block frame's grass (y 0), not over the slab top
    let iv: Vec<[f32; 3]> = iv.iter().map(|p| [p[0], INDICATOR_LIFT, p[2]]).collect();
    let donor = "Stadium\\Items\\LightCube2m.Item.Gbx";
    let mut mg = crate::static_item::build::pack_item_merged(store, donor, 1.0, collection, 0, None).map_err(|e| format!("donor {donor}: {e}"))?;
    crate::static_item::build::make_invisible_mesh_waypoint(&mut mg, crate::sttc::WP_FINISH, tv.clone(), tt.clone())?;
    crate::static_item::build::add_finish_mesh_visual(&mut mg, &iv, &it)?;
    let (bytes, _mg) = crate::static_item::build::finish_item(mg, ident, ident, 1.0, collection)?;
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for v in &tv { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
    let c = Cutout { verts: tv, tris: tt, kept: o.vertices, dropped: dropped_slivers, area: o.area, bbox: (lo, hi) };
    Ok(Some((bytes, c, o)))
}

pub fn build_cutout_item(store: &mut crate::store::DataStore, name: &str, flags: u32, ident: &str, collection: u32, inset: f32, below: f32, above: f32) -> Result<Option<(Vec<u8>, Cutout)>, String> {
    let Some(c) = cutout_for_block(store, name, flags, inset, below, above)? else { return Ok(None) };
    let donor = "Stadium\\Items\\LightCube2m.Item.Gbx";
    let mut mg = crate::static_item::build::pack_item_merged(store, donor, 1.0, collection, 0, None).map_err(|e| format!("donor {donor}: {e}"))?;
    crate::static_item::build::make_invisible_mesh_waypoint(&mut mg, crate::sttc::WP_FINISH, c.verts.clone(), c.tris.clone())?;
    let quads: Vec<(f32, f32, f32, f32, f32)> = c.verts.chunks(8).map(|b| (b[0][0], b[1][0], b[0][2], b[4][2], 0.0)).collect();
    crate::static_item::build::add_finish_plane_visual(&mut mg, &quads, INDICATOR_LIFT)?;
    let (bytes, _mg) = crate::static_item::build::finish_item(mg, ident, ident, 1.0, collection)?;
    Ok(Some((bytes, c)))
}

/// The MAIN PLANE mesh (round 4): the grass cells the game keeps (not hidden), whole cells as row strips.
pub fn stadium_plane_mesh(store: &mut crate::store::DataStore, m: &MapFile, below: f32, above: f32) -> Result<FloorMesh, String> {
    let f = stadium_floor(store, m)?;
    let mut rows: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    let mut free = 0usize;
    for &(cx, cz) in &f.grass {
        if !f.hidden.contains(&(cx, cz)) {
            rows.entry(cz).or_default().push(cx);
            free += 1;
        }
    }
    if free == 0 {
        return Err("every grass cell is hidden".into());
    }
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let quads: [[u32; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [1, 2, 6, 5], [0, 4, 7, 3]];
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    let (y0, y1) = (f.grass_y - below, f.grass_y + above);
    let mut strips = 0usize;
    let mut anchors: Vec<(f32, f32, f32, f32, f32)> = Vec::new();
    for (cz, mut xs) in rows {
        xs.sort_unstable();
        xs.dedup();
        let (z0, z1) = (32.0 * cz as f32, 32.0 * (cz + 1) as f32);
        let mut i = 0;
        while i < xs.len() {
            let start = xs[i];
            let mut end = start;
            while i + 1 < xs.len() && xs[i + 1] == end + 1 { i += 1; end = xs[i]; }
            i += 1;
            let (x0, x1) = (32.0 * start as f32, 32.0 * (end + 1) as f32);
            let base = verts.len() as u32;
            verts.extend_from_slice(&[[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]);
            for q in quads {
                tris.push([base + q[0], base + q[1], base + q[2]]);
                tris.push([base + q[0], base + q[2], base + q[3]]);
            }
            for v in &verts[base as usize..] { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
            anchors.push((x0, x1, z0, z1, f.grass_y));
            strips += 1;
        }
    }
    let mut notes = f.notes.clone();
    notes.push(format!("main plane: {free} kept grass cells of {} → {strips} row strips", f.grass.len()));
    Ok(FloorMesh { verts, tris, quads: anchors, cells: free, strips, shore_boxes: 0, shore_prisms: 0, notes, bbox: (lo, hi) })
}

/// The Stadium floor mesh: the FREE grass cells as row strips, slab on the grass plane.
pub fn stadium_floor_mesh(store: &mut crate::store::DataStore, m: &MapFile, below: f32, above: f32) -> Result<FloorMesh, String> {
    let f = stadium_floor(store, m)?;
    // the occupied 4 m squares on the world square grid, dilated by one square (8-neighbourhood): the safety
    // margin — the slab never borders structure (a car clipping along a deck edge falls beside it)
    let mut occ_world: std::collections::HashSet<(i64, i64)> = Default::default();
    for (&(cx, cz), &mask) in &f.occupied {
        for zi in 0..8i64 {
            for xi in 0..8i64 {
                if mask & (1u64 << (zi * 8 + xi)) != 0 {
                    occ_world.insert((cx as i64 * 8 + xi, cz as i64 * 8 + zi));
                }
            }
        }
    }
    let mut margin: std::collections::HashSet<(i64, i64)> = Default::default();
    for &(x, z) in &occ_world {
        for dz in -1..=1 {
            for dx in -1..=1 {
                margin.insert((x + dx, z + dz));
            }
        }
    }
    let mut rows: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    let (mut free_sq, mut grass_sq, mut green_sq, mut margin_sq) = (0usize, 0usize, 0usize, 0usize);
    for &(cx, cz) in &f.grass {
        let hidden = f.hidden.contains(&(cx, cz));
        let grn = f.green.get(&(cx, cz)).copied().unwrap_or(0);
        for zi in 0..8i64 {
            for xi in 0..8i64 {
                let bit = 1u64 << (zi * 8 + xi);
                let is_grass = if hidden { grn & bit != 0 } else { true };
                if !is_grass { continue; }
                if hidden { green_sq += 1 } else { grass_sq += 1 }
                let sq = (cx as i64 * 8 + xi, cz as i64 * 8 + zi);
                if margin.contains(&sq) { margin_sq += 1; continue; }
                rows.entry(sq.1 * 4).or_default().push(sq.0 * 4);
                free_sq += 1;
            }
        }
    }
    let mut notes = f.notes.clone();
    notes.push(format!("grass squares: {grass_sq} real grass (tile kept) + {green_sq} collidable fill in hidden cells; {margin_sq} within 4 m of structure dropped (margin) → {free_sq} FREE squares ({:.1} cells-eq of {})", free_sq as f32 / 64.0, f.grass.len()));
    if free_sq == 0 {
        return Err("every grass square is occupied".into());
    }
    let mut spans: Vec<(i64, i64, i64)> = Vec::new(); // (z0, x0, x1)
    for (z0, mut xs) in rows {
        xs.sort_unstable();
        xs.dedup();
        let mut i = 0;
        while i < xs.len() {
            let start = xs[i];
            let mut end = start;
            while i + 1 < xs.len() && xs[i + 1] == end + 4 { i += 1; end = xs[i]; }
            i += 1;
            spans.push((z0, start, end + 4));
        }
    }
    spans.sort_unstable_by_key(|&(z0, x0, x1)| (x0, x1, z0));
    let mut boxes: Vec<(f32, f32, f32, f32)> = Vec::new(); // x0 x1 z0 z1
    let mut i = 0;
    while i < spans.len() {
        let (z0, x0, x1) = spans[i];
        let mut z1 = z0 + 4;
        while i + 1 < spans.len() && spans[i + 1].1 == x0 && spans[i + 1].2 == x1 && spans[i + 1].0 == z1 { i += 1; z1 += 4; }
        i += 1;
        boxes.push((x0 as f32, x1 as f32, z0 as f32, z1 as f32));
    }
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let quads: [[u32; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [1, 2, 6, 5], [0, 4, 7, 3]];
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    let (y0, y1) = (f.grass_y - below, f.grass_y + above);
    for (x0, x1, z0, z1) in &boxes {
        let base = verts.len() as u32;
        verts.extend_from_slice(&[[*x0, y0, *z0], [*x1, y0, *z0], [*x1, y1, *z0], [*x0, y1, *z0], [*x0, y0, *z1], [*x1, y0, *z1], [*x1, y1, *z1], [*x0, y1, *z1]]);
        for q in quads {
            tris.push([base + q[0], base + q[1], base + q[2]]);
            tris.push([base + q[0], base + q[2], base + q[3]]);
        }
        for v in &verts[base as usize..] { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
    }
    notes.push(format!("→ {} boxes", boxes.len()));
    let anchors: Vec<(f32, f32, f32, f32, f32)> = boxes.iter().map(|&(x0, x1, z0, z1)| (x0, x1, z0, z1, f.grass_y)).collect();
    Ok(FloorMesh { verts, tris, quads: anchors, cells: free_sq, strips: boxes.len(), shore_boxes: 0, shore_prisms: 0, notes, bbox: (lo, hi) })
}

/// The world-frame slab mesh of a map's water: open-water cells merged into ROW STRIPS (same cz, same bed,
/// consecutive cx), the shore water parts as their own boxes. `below`/`above`: the slab's span under / over the bed.
pub fn floor_mesh(store: &mut crate::store::DataStore, m: &MapFile, collection: u32, below: f32, above: f32, shores: bool) -> Result<FloorMesh, String> {
    let (rects, n_open, n_shore, notes) = water_rects(store, m, collection, shores)?;
    if rects.is_empty() {
        return Err("no water tile in the map (authored or baked)".into());
    }
    // the full cells: (bed, z0) -> sorted x0 list (cells are 32 m); merge consecutive ones
    let mut rows: BTreeMap<(i64, i64), Vec<i64>> = BTreeMap::new();
    let mut boxes: Vec<(f32, f32, f32, f32, f32, f32)> = Vec::new(); // x0 x1 z0 z1 y_bed floor_y
    let prisms: Vec<([[f32; 2]; 3], f32)> = Vec::new();
    // a shore cell's water footprint, rasterised on a 4 m grid (8×8 per cell; a grid square is water when its
    // centre lies inside a water triangle) → grid-aligned boxes merged along x per grid row — a few boxes per
    // shore cell instead of one prism per (tessellated) water triangle
    let mut shore_rows: BTreeMap<(i64, i64, i64), Vec<i64>> = BTreeMap::new(); // (bed cm, floor cm, z0 of the 4 m row) -> x0 list (4 m)
    let inside = |p: [f32; 2], t: &[[f32; 2]; 3]| -> bool {
        let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
        let (d1, d2, d3) = (s(p, t[0], t[1]), s(p, t[1], t[2]), s(p, t[2], t[0]));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    };
    let mut floors: BTreeMap<i64, f32> = BTreeMap::new(); // bed cm -> floor y (open cells share one per row)
    for r in &rects {
        if r.full_cell {
            rows.entry(((r.y_bed * 100.0).round() as i64, (r.z0).round() as i64)).or_default().push(r.x0.round() as i64);
            floors.insert((r.y_bed * 100.0).round() as i64, r.floor_y);
        } else if !r.tris.is_empty() {
            let (cx0, cz0) = ((r.x0 / 32.0).floor() * 32.0, (r.z0 / 32.0).floor() * 32.0);
            for zi in 0..8 {
                for xi in 0..8 {
                    let p = [cx0 + 4.0 * xi as f32 + 2.0, cz0 + 4.0 * zi as f32 + 2.0];
                    if r.tris.iter().any(|t| inside(p, t)) {
                        shore_rows.entry(((r.y_bed * 100.0).round() as i64, (r.floor_y * 100.0).round() as i64, (cz0 + 4.0 * zi as f32).round() as i64)).or_default().push((cx0 + 4.0 * xi as f32).round() as i64);
                    }
                }
            }
        } else {
            boxes.push((r.x0, r.x1, r.z0, r.z1, r.y_bed, r.floor_y));
        }
    }
    let mut strips = 0usize;
    for ((ybed_c, z0), mut xs) in rows {
        xs.sort_unstable();
        xs.dedup();
        let y_bed = ybed_c as f32 / 100.0;
        let mut i = 0;
        while i < xs.len() {
            let start = xs[i];
            let mut end = start;
            while i + 1 < xs.len() && xs[i + 1] == end + 32 {
                i += 1;
                end = xs[i];
            }
            i += 1;
            boxes.push((start as f32, (end + 32) as f32, z0 as f32, (z0 + 32) as f32, y_bed, floors.get(&ybed_c).copied().unwrap_or(y_bed - water_tiles(collection).1)));
            strips += 1;
        }
    }
    // the shore 4 m rows → x-spans per row (merged across cell borders: the rows are keyed by world z), then
    // consecutive rows (z, z+4) with an identical span merge into one taller box
    let mut spans: BTreeMap<(i64, i64), Vec<(i64, i64, i64)>> = BTreeMap::new(); // (bed cm, floor cm) -> (z0, x0, x1)
    for ((ybed_c, floor_c, z0), mut xs) in shore_rows {
        xs.sort_unstable();
        xs.dedup();
        let mut i = 0;
        while i < xs.len() {
            let start = xs[i];
            let mut end = start;
            while i + 1 < xs.len() && xs[i + 1] == end + 4 {
                i += 1;
                end = xs[i];
            }
            i += 1;
            spans.entry((ybed_c, floor_c)).or_default().push((z0, start, end + 4));
        }
    }
    let mut shore_box_count = 0usize;
    for ((ybed_c, floor_c), mut list) in spans {
        let y_bed = ybed_c as f32 / 100.0;
        let floor_y = floor_c as f32 / 100.0;
        // group by (x0, x1), merge consecutive z rows
        list.sort_unstable_by_key(|&(z0, x0, x1)| (x0, x1, z0));
        let mut i = 0;
        while i < list.len() {
            let (z0, x0, x1) = list[i];
            let mut z1 = z0 + 4;
            while i + 1 < list.len() && list[i + 1].1 == x0 && list[i + 1].2 == x1 && list[i + 1].0 == z1 {
                i += 1;
                z1 += 4;
            }
            i += 1;
            boxes.push((x0 as f32, x1 as f32, z0 as f32, z1 as f32, y_bed, floor_y));
            shore_box_count += 1;
        }
    }
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let quads: [[u32; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [1, 2, 6, 5], [0, 4, 7, 3]];
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    // bed-anchored slab on every box (v4.1): bed − below .. bed + above
    for (x0, x1, z0, z1, y_bed, _floor_y) in &boxes {
        let (y0, y1) = (y_bed - below, y_bed + above);
        let base = verts.len() as u32;
        verts.extend_from_slice(&[[*x0, y0, *z0], [*x1, y0, *z0], [*x1, y1, *z0], [*x0, y1, *z0], [*x0, y0, *z1], [*x1, y0, *z1], [*x1, y1, *z1], [*x0, y1, *z1]]);
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
    }
    // the shore water triangles as closed prisms (bottom, top, 3 side quads = 8 triangles each)
    for (t, y_bed) in &prisms {
        let (y0, y1) = (y_bed - below, y_bed + above);
        let base = verts.len() as u32;
        verts.extend_from_slice(&[[t[0][0], y0, t[0][1]], [t[1][0], y0, t[1][1]], [t[2][0], y0, t[2][1]], [t[0][0], y1, t[0][1]], [t[1][0], y1, t[1][1]], [t[2][0], y1, t[2][1]]]);
        tris.push([base, base + 2, base + 1]); // bottom (facing down)
        tris.push([base + 3, base + 4, base + 5]); // top
        for e in 0..3u32 {
            let (a, b) = (e, (e + 1) % 3);
            tris.push([base + a, base + b, base + 3 + b]);
            tris.push([base + a, base + 3 + b, base + 3 + a]);
        }
        for v in &verts[base as usize..] {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
    }
    let anchors: Vec<(f32, f32, f32, f32, f32)> = boxes.iter().map(|&(x0, x1, z0, z1, y_bed, _)| (x0, x1, z0, z1, y_bed)).collect();
    Ok(FloorMesh { verts, tris, quads: anchors, cells: n_open, strips, shore_boxes: n_shore, shore_prisms: shore_box_count, notes, bbox: (lo, hi) })
}

/// The one-per-map carrier: an invisible Finish item (speck visual, no collision) whose trigger is the
/// floor mesh; ident = author = `<ident>`; collection = the map's. Returns (bytes, mesh).
pub fn build_floor_item(store: &mut crate::store::DataStore, m: &MapFile, collection: u32, ident: &str, below: f32, above: f32, shores: bool, lm_word: Option<u64>) -> Result<(Vec<u8>, FloorMesh), String> {
    let mesh = if collection == 0x1a { stadium_plane_mesh(store, m, below, above)? } else { floor_mesh(store, m, collection, below, above, shores)? };
    let donor = "Stadium\\Items\\LightCube2m.Item.Gbx";
    let mut mg = crate::static_item::build::pack_item_merged(store, donor, 1.0, collection, 0, None).map_err(|e| format!("donor {donor}: {e}"))?;
    crate::static_item::build::make_invisible_mesh_waypoint(&mut mg, crate::sttc::WP_FINISH, mesh.verts.clone(), mesh.tris.clone())?;
    // the indicator: a solid-red translucent CustomGlass pane (TargetColor 1,0,0; v4.5 — was the RaceTriggerFXFinish pattern) 2 cm over the ground each box marks
    crate::static_item::build::add_finish_plane_visual(&mut mg, &mesh.quads, INDICATOR_LIFT)?;
    if let Some(w) = lm_word {
        mg.file_write_time = w;
    }
    let (bytes, _mg) = crate::static_item::build::finish_item(mg, ident, ident, 1.0, collection)?;
    Ok((bytes, mesh))
}

/// The indicator plane sits this far over the ground it marks (Hugo: "2 cm above").
pub const INDICATOR_LIFT: f32 = 0.02;

/// Hugo's GroundClippers (2026-10-24 02:10–02:16): a finish for cars that clip through the ground into the void —
/// ONE box under the whole map, x 0..size_x·32, z 0..size_z·32, top = the map's FLOOR − 0.15 (a landing's wheel
/// penetration never reaches it), 1 m thick. The floor comes from the map: the lowest terrain row present
/// (authored + baked zone tiles: the open-water and shore tiles, the land tiles) → that row's floor + the
/// collection's bed-local constant (RedIsland/WhiteShore/GreenCoast 2.0, BlueBay 4.0, Stadium grass 0.0).
/// Measured on the 25 Fall sources: RedIsland row 14 → −6.0, WhiteShore row 14 → −6.0, BlueBay row 5 → 4.0,
/// GreenCoast row 4 → −6.0, Stadium row 9 → 8.0, the same on every map of a collection.
pub const CLIPPER_TOP_UNDER_FLOOR: f32 = 0.15;
pub const CLIPPER_THICKNESS: f32 = 1.0;

/// The terrain tiles whose rows define the floor (open water, shores, land) per collection.
pub fn terrain_tiles(collection: u32) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = water_tiles(collection).0.to_vec();
    v.extend_from_slice(shore_tiles(collection));
    v.extend_from_slice(match collection {
        0x10 => &["Dirt"],
        0x1d => &["Land"],
        0xf => &["Grass"],
        0x1a => &["Grass"],
        _ => &[],
    });
    v
}

/// The map's floor: (world y, lowest terrain row, bed-local constant), or None when no terrain tile is placed.
pub fn map_floor(m: &MapFile, collection: u32) -> Option<(f32, i32, f32)> {
    let tiles = terrain_tiles(collection);
    let row = m.blocks.iter().chain(m.baked.iter()).filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none() && tiles.iter().any(|t| t.eq_ignore_ascii_case(&b.name))).map(|b| b.coords().1).min()?;
    let bed = if collection == 0x1a { 0.0 } else { water_tiles(collection).1 };
    Some((tmmaps::map::ground_y(collection) + 8.0 * row as f32 + bed, row, bed))
}

/// The GroundClippers item of a map: one box 0..size·32 in x and z, floor − 1.15 .. floor − 0.15, the
/// indicator pane on its top (under the terrain, never seen), Finish waypoint. Returns the bytes and
/// the mesh (one box).
pub fn build_ground_clippers_item(store: &mut crate::store::DataStore, m: &MapFile, collection: u32, ident: &str) -> Result<(Vec<u8>, FloorMesh), String> {
    let (floor, row, bed) = map_floor(m, collection).ok_or("no terrain tile in the map (authored or baked): no floor to clip through")?;
    let (x1, z1) = (32.0 * m.size[0] as f32, 32.0 * m.size[2] as f32);
    let (y1, y0) = (floor - CLIPPER_TOP_UNDER_FLOOR, floor - CLIPPER_TOP_UNDER_FLOOR - CLIPPER_THICKNESS);
    let verts = vec![[0.0, y0, 0.0], [x1, y0, 0.0], [x1, y1, 0.0], [0.0, y1, 0.0], [0.0, y0, z1], [x1, y0, z1], [x1, y1, z1], [0.0, y1, z1]];
    let quads: [[u32; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [1, 2, 6, 5], [0, 4, 7, 3]];
    let mut tris = Vec::new();
    for q in quads {
        tris.push([q[0], q[1], q[2]]);
        tris.push([q[0], q[2], q[3]]);
    }
    let donor = "Stadium\\Items\\LightCube2m.Item.Gbx";
    let mut mg = crate::static_item::build::pack_item_merged(store, donor, 1.0, collection, 0, None).map_err(|e| format!("donor {donor}: {e}"))?;
    crate::static_item::build::make_invisible_mesh_waypoint(&mut mg, crate::sttc::WP_FINISH, verts.clone(), tris.clone())?;
    crate::static_item::build::add_finish_plane_visual(&mut mg, &[(0.0, x1, 0.0, z1, y1 - INDICATOR_LIFT)], INDICATOR_LIFT)?;
    let (bytes, _) = crate::static_item::build::finish_item(mg, ident, ident, 1.0, collection)?;
    let notes = vec![format!("ground clippers: map {}×{} cells → box 0..{x1:.0} × 0..{z1:.0} m, floor = lowest terrain row {row} (floor y {:.2} + bed {bed:.1}) = {floor:.2} → slab {y0:.2} .. {y1:.2}", m.size[0], m.size[2], floor - bed)];
    Ok((bytes, FloorMesh { verts, tris, quads: vec![(0.0, x1, 0.0, z1, y1 - INDICATOR_LIFT)], cells: (m.size[0] * m.size[2]) as usize, strips: 1, shore_boxes: 0, shore_prisms: 0, notes, bbox: ([0.0, y0, 0.0], [x1, y1, z1]) }))
}

/// The standable fill triangles and the structure triangles of a block's picked variant (block frame).
pub fn floor_and_structure_of(store: &mut crate::store::DataStore, name: &str, flags: u32) -> Result<(Vec<[[f32; 3]; 3]>, Vec<[[f32; 3]; 3]>), String> {
    let Some(bp) = crate::vista::find_block_info(store, name) else { return Err(format!("{name}: no block info")) };
    let bi = crate::blockinfo::load(store, &bp)?;
    let ground = flags & crate::blockmap::FLAG_GROUND != 0;
    let variant = (flags & 63) as usize;
    let sub = ((flags >> crate::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
    let addv = ((flags >> crate::blockmap::FLAG_ADDITIONAL_SHIFT) & 0x7F) as usize;
    let Some(p) = bi.pick_placement_add(ground, variant, sub, addv) else { return Err(format!("{name}: no variant for flags {flags:#x}")) };
    // the PICKED placement's mobils only (as cutout_for_block does): the variant lists every placement's
    let mut v = p.variant.clone();
    v.mobils = vec![p.mobils.iter().map(|m| (*m).clone()).collect()];
    variant_floor_and_structure(store, &v)
}

/// The boundary of a triangle soup in the xz plane: the directed edges that no other triangle shares (vertices
/// keyed on a 1 mm grid), chained into closed loops. Each loop is a polyline of the mesh's OWN vertices — the
/// pak's curb resolution, nothing invented.
pub fn boundary_loops(tris: &[[[f32; 3]; 3]]) -> Vec<Vec<[f32; 2]>> {
    use std::collections::HashMap;
    let key = |p: [f32; 3]| -> (i64, i64) { ((p[0] * 1000.0).round() as i64, (p[2] * 1000.0).round() as i64) };
    let mut count: HashMap<((i64, i64), (i64, i64)), i32> = HashMap::new();
    for t in tris {
        // make every triangle counter-clockwise in xz (looking down −y … use the signed area)
        let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if area.abs() < 1e-6 { continue; }
        let (ka, kb, kc) = (key(t[0]), key(t[1]), key(t[2]));
        let edges = if area > 0.0 { [(ka, kb), (kb, kc), (kc, ka)] } else { [(kb, ka), (kc, kb), (ka, kc)] };
        for (p, q) in edges {
            *count.entry((p, q)).or_default() += 1;
        }
    }
    // an edge is a boundary when its reverse is absent (interior edges appear once each way)
    let mut next: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    for ((p, q), _) in count.iter() {
        if !count.contains_key(&(*q, *p)) {
            next.entry(*p).or_default().push(*q);
        }
    }
    let mut loops = Vec::new();
    let mut used: std::collections::HashSet<((i64, i64), (i64, i64))> = Default::default();
    let mut starts: Vec<(i64, i64)> = next.keys().copied().collect();
    starts.sort_unstable();
    for s in starts {
        let Some(qs) = next.get(&s) else { continue };
        for q0 in qs {
            if used.contains(&(s, *q0)) { continue; }
            let mut lp = vec![s];
            let (mut p, mut q) = (s, *q0);
            used.insert((p, q));
            let mut guard = 0;
            while q != s && guard < 100_000 {
                lp.push(q);
                let Some(cands) = next.get(&q) else { break };
                let Some(&n) = cands.iter().find(|n| !used.contains(&(q, **n))) else { break };
                used.insert((q, n));
                p = n;
                let _ = p;
                let old = q;
                q = n;
                let _ = old;
                guard += 1;
            }
            if lp.len() >= 3 {
                loops.push(lp.into_iter().map(|(x, z)| [x as f32 / 1000.0, z as f32 / 1000.0]).collect());
            }
        }
    }
    loops
}

/// Signed area of a polygon (counter-clockwise positive in xz).
pub fn polygon_area(p: &[[f32; 2]]) -> f32 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (u, v) = (p[i], p[(i + 1) % p.len()]);
        a += u[0] * v[1] - v[0] * u[1];
    }
    a / 2.0
}

/// The footprint size (cells at dir 0) of a block's picked variant.
pub fn variant_size_of(store: &mut crate::store::DataStore, name: &str, flags: u32) -> Result<(i32, i32), String> {
    let Some(bp) = crate::vista::find_block_info(store, name) else { return Err(format!("{name}: no block info")) };
    let bi = crate::blockinfo::load(store, &bp)?;
    let ground = flags & crate::blockmap::FLAG_GROUND != 0;
    let variant = (flags & 63) as usize;
    let sub = ((flags >> crate::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
    let addv = ((flags >> crate::blockmap::FLAG_ADDITIONAL_SHIFT) & 0x7F) as usize;
    let Some(p) = bi.pick_placement_add(ground, variant, sub, addv) else { return Err(format!("{name}: no variant for flags {flags:#x}")) };
    let units = &p.variant.block_units;
    Ok(if units.is_empty() { (1, 1) } else { (units.iter().map(|u| u.offset[0]).max().unwrap_or(0) + 1, units.iter().map(|u| u.offset[2]).max().unwrap_or(0) + 1) })
}

pub fn dist_xz_pub(p: [f32; 2], t: &[[f32; 3]; 3]) -> f32 {
    dist_xz(p, t)
}
