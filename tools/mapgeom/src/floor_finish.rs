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
    for t in &m.surf_triangles {
        let pts: Vec<[f32; 3]> = t.indices.iter().filter_map(|&i| m.surf_vertices.get(i as usize).copied()).collect();
        if pts.len() < 3 { continue; }
        let c = [(pts[0][0] + pts[1][0] + pts[2][0]) / 3.0, (pts[0][1] + pts[1][1] + pts[2][1]) / 3.0, (pts[0][2] + pts[1][2] + pts[2][2]) / 3.0];
        if c[0] >= lo[0] - 0.01 && c[0] <= hi[0] + 0.01 && c[2] >= lo[2] - 0.01 && c[2] <= hi[2] + 0.01 && c[1] < surface - 0.05 {
            for p in &pts { blo = blo.min(p[1]); bhi = bhi.max(p[1]); }
            nb += 1;
        }
    }
    Ok(Some(WaterRect { prefab: prefab.rsplit('\\').next().unwrap_or(&prefab).to_string(), x0: lo[0], x1: hi[0], z0: lo[2], z1: hi[2], surface, bed_lo: blo, bed_hi: bhi, bed_tris: nb, tris: wtris }))
}

/// The 4 m-grid water mask of a shore kind in the block frame: bit (zi·8 + xi) = the square at x 4·xi..4·xi+4,
/// z 4·zi..4·zi+4 (its centre inside a water triangle). The same rasterisation `floor_mesh` uses.
pub fn footprint_mask(w: &WaterRect) -> u64 {
    let inside = |p: [f32; 2], t: &[[f32; 3]; 3]| -> bool {
        let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
        let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
        let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    };
    let mut mask = 0u64;
    for zi in 0..8 {
        for xi in 0..8 {
            let p = [4.0 * xi as f32 + 2.0, 4.0 * zi as f32 + 2.0];
            if w.tris.iter().any(|t| inside(p, t)) {
                mask |= 1u64 << (zi * 8 + xi);
            }
        }
    }
    mask
}

pub struct FloorMesh {
    pub verts: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
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
    match collection {
        0x1c => &["Beach", "SeaCliff13", "Sea_Land0_Land12"],
        0x1d => &["WaterShore1"],
        0xf => &["LakeShore"],
        0x10 => &["WaterShore1"],
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
    pub full_cell: bool,
    /// shore water: the footprint triangles in world xz (empty for a full cell)
    pub tris: Vec<[[f32; 2]; 3]>,
}

/// A block-frame (dir 0) point → the world xz of a placement at `cell` with `dir`.
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
pub fn water_rects(store: &mut crate::store::DataStore, m: &MapFile, collection: u32) -> Result<(Vec<WorldRect>, usize, usize, Vec<String>), String> {
    let (tiles, bed) = water_tiles(collection);
    if tiles.is_empty() {
        return Err(format!("collection {collection:#x}: no open-water tile table (vista collections only)"));
    }
    let ground = tmmaps::map::ground_y(collection);
    let shore = shore_tiles(collection);
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
            out.push(WorldRect { x0: 32.0 * c.0 as f32, x1: 32.0 * (c.0 + 1) as f32, z0: 32.0 * c.2 as f32, z1: 32.0 * (c.2 + 1) as f32, y_bed: ground + 8.0 * c.1 as f32 + bed, full_cell: true, tris: Vec::new() });
            n_open += 1;
        } else if shore.iter().any(|t| t.eq_ignore_ascii_case(&b.name)) {
            let key = (b.name.clone(), b.flags);
            if !shore_cache.contains_key(&key) {
                let r = water_rect(store, collection, &b.name, b.flags).unwrap_or_else(|e| { notes.push(format!("{}: {e}", b.name)); None });
                shore_cache.insert(key.clone(), r);
            }
            if let Some(Some(w)) = shore_cache.get(&key) {
                let (x0, x1, z0, z1) = turn_rect(c, b.dir, w.x0, w.x1, w.z0, w.z1);
                let tris: Vec<[[f32; 2]; 3]> = w.tris.iter().map(|t| [turn_pt(c, b.dir, t[0][0], t[0][2]), turn_pt(c, b.dir, t[1][0], t[1][2]), turn_pt(c, b.dir, t[2][0], t[2][2])]).collect();
                // the bed under a shore's water = its lowest collision (the open-water bed; a beach slopes down to it)
                out.push(WorldRect { x0, x1, z0, z1, y_bed: ground + 8.0 * c.1 as f32 + w.bed_lo, full_cell: false, tris });
                n_shore += 1;
            }
        }
    }
    for ((name, flags), r) in &shore_cache {
        match r {
            Some(w) => notes.push(format!("{name} flags {flags:08x}: {} water x {:.2}..{:.2} z {:.2}..{:.2} (block frame), surface {:.2}, bed {:.2}..{:.2}", w.prefab, w.x0, w.x1, w.z0, w.z1, w.surface, w.bed_lo, w.bed_hi)),
            None => notes.push(format!("{name} flags {flags:08x}: no water quad")),
        }
    }
    Ok((out, n_open, n_shore, notes))
}

/// The world-frame slab mesh of a map's water: open-water cells merged into ROW STRIPS (same cz, same bed,
/// consecutive cx), the shore water parts as their own boxes. `below`/`above`: the slab's span under / over the bed.
pub fn floor_mesh(store: &mut crate::store::DataStore, m: &MapFile, collection: u32, below: f32, above: f32) -> Result<FloorMesh, String> {
    let (rects, n_open, n_shore, notes) = water_rects(store, m, collection)?;
    if rects.is_empty() {
        return Err("no water tile in the map (authored or baked)".into());
    }
    // the full cells: (bed, z0) -> sorted x0 list (cells are 32 m); merge consecutive ones
    let mut rows: BTreeMap<(i64, i64), Vec<i64>> = BTreeMap::new();
    let mut boxes: Vec<(f32, f32, f32, f32, f32)> = Vec::new(); // x0 x1 z0 z1 y_bed
    let prisms: Vec<([[f32; 2]; 3], f32)> = Vec::new();
    // a shore cell's water footprint, rasterised on a 4 m grid (8×8 per cell; a grid square is water when its
    // centre lies inside a water triangle) → grid-aligned boxes merged along x per grid row — a few boxes per
    // shore cell instead of one prism per (tessellated) water triangle
    let mut shore_rows: BTreeMap<(i64, i64), Vec<i64>> = BTreeMap::new(); // (bed cm, z0 of the 4 m row) -> x0 list (4 m)
    let inside = |p: [f32; 2], t: &[[f32; 2]; 3]| -> bool {
        let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
        let (d1, d2, d3) = (s(p, t[0], t[1]), s(p, t[1], t[2]), s(p, t[2], t[0]));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    };
    for r in &rects {
        if r.full_cell {
            rows.entry(((r.y_bed * 100.0).round() as i64, (r.z0).round() as i64)).or_default().push(r.x0.round() as i64);
        } else if !r.tris.is_empty() {
            let (cx0, cz0) = ((r.x0 / 32.0).floor() * 32.0, (r.z0 / 32.0).floor() * 32.0);
            for zi in 0..8 {
                for xi in 0..8 {
                    let p = [cx0 + 4.0 * xi as f32 + 2.0, cz0 + 4.0 * zi as f32 + 2.0];
                    if r.tris.iter().any(|t| inside(p, t)) {
                        shore_rows.entry(((r.y_bed * 100.0).round() as i64, (cz0 + 4.0 * zi as f32).round() as i64)).or_default().push((cx0 + 4.0 * xi as f32).round() as i64);
                    }
                }
            }
        } else {
            boxes.push((r.x0, r.x1, r.z0, r.z1, r.y_bed));
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
            boxes.push((start as f32, (end + 32) as f32, z0 as f32, (z0 + 32) as f32, y_bed));
            strips += 1;
        }
    }
    // the shore 4 m rows → x-spans per row (merged across cell borders: the rows are keyed by world z), then
    // consecutive rows (z, z+4) with an identical span merge into one taller box
    let mut spans: BTreeMap<i64, Vec<(i64, i64, i64)>> = BTreeMap::new(); // bed cm -> (z0, x0, x1)
    for ((ybed_c, z0), mut xs) in shore_rows {
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
            spans.entry(ybed_c).or_default().push((z0, start, end + 4));
        }
    }
    let mut shore_box_count = 0usize;
    for (ybed_c, mut list) in spans {
        let y_bed = ybed_c as f32 / 100.0;
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
            boxes.push((x0 as f32, x1 as f32, z0 as f32, z1 as f32, y_bed));
            shore_box_count += 1;
        }
    }
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let quads: [[u32; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [1, 2, 6, 5], [0, 4, 7, 3]];
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for (x0, x1, z0, z1, y_bed) in &boxes {
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
    Ok(FloorMesh { verts, tris, cells: n_open, strips, shore_boxes: n_shore, shore_prisms: shore_box_count, notes, bbox: (lo, hi) })
}

/// The one-per-map carrier: an invisible Finish item (speck visual, no collision) whose trigger is the
/// floor mesh; ident = author = `<ident>`; collection = the map's. Returns (bytes, mesh).
pub fn build_floor_item(store: &mut crate::store::DataStore, m: &MapFile, collection: u32, ident: &str, below: f32, above: f32, lm_word: Option<u64>) -> Result<(Vec<u8>, FloorMesh), String> {
    let mesh = floor_mesh(store, m, collection, below, above)?;
    let donor = "Stadium\\Items\\LightCube2m.Item.Gbx";
    let mut mg = crate::static_item::build::pack_item_merged(store, donor, 1.0, collection, 0, None).map_err(|e| format!("donor {donor}: {e}"))?;
    crate::static_item::build::make_invisible_mesh_waypoint(&mut mg, crate::sttc::WP_FINISH, mesh.verts.clone(), mesh.tris.clone())?;
    if let Some(w) = lm_word {
        mg.file_write_time = w;
    }
    let (bytes, _mg) = crate::static_item::build::finish_item(mg, ident, ident, 1.0, collection)?;
    Ok((bytes, mesh))
}
