//! The OUTER RING (Hugo, 2026-10-24 08:27: "people fly off the map and land on the grass/water outside the
//! buildable area" — can the finish extend ~6 cells outside the grid?). The terrain outside the grid is the
//! map's DECORATION, measured here per 4 m square over the band −6..+6 cells around the grid:
//!
//! * **Stadium** (`48x48*` decorations): the decoration is itself a map in the `Stadium256` collection (256 m
//!   cells, 16×12 = 4096×3072 m; the 1536 m grid sits 1280 / 768 m in — cells 5 / 3) holding two `Stade1536`
//!   halves (east/west, each spanning exactly the grid's z range) and two `Stade4096` halves (north/south, the
//!   long sides). Each half prefab = a hit shape (`<Name>.HitShape.Gbx`, the stands' collision) + a grass static
//!   object (`Grass1536` 213 m × 1536 m, `Grass4096` 1630 m × 78 m) at y 128 in the deco frame = the grid's own
//!   grass height 8.0 (deco y − 120). The second half of each pair is the point reflection of the first through the
//!   stadium centre (dir 2 = yaw π: `deco = origin + (W − x, D − z)`, footprints (1280, 1536) and (4096, 768)).
//!   The clear flat grass at 8.0 (no hit-shape face within 3 m above it in the square) reaches 188–192 m (6 cells)
//!   along the east/west sides, tapering to 40–80 m at the corners, and only 0–72 m along north/south; beyond it the
//!   ground dips into a trench (−7 / −16) or climbs into the stands (+9 / +23). Not a constant → a table of
//!   4 m-row strips. `NoStadium48x48*` decorations: one flat `NoStadium` piece — measured the same way.
//! * **Vistas** (`Base64x64*`): the `Base64x64.Scene3d` is a huge sea (`Water` plane) over a `WarpSand` /
//!   `WarpGround` seabed, with an `InvisibleShadowCaster` box under the grid ± 198 m. The scene frame is the map
//!   frame horizontally; vertically it is `WaterDesc.top − the scene's Water plane y` (BlueBay 0, the other three
//!   +8.0 m — their scene sits one row lower). In the band the sea covers every square; the bed starts at 4.0 at
//!   the grid edge (all four) and undulates down to about −3.5 by the 6th cell. Ring slab = ONE body anchored on the
//!   band's highest bed (4.0 — identical to BlueBay's inside open bed, 2 m over the other three's 2.0): a car sinking
//!   anywhere in the ring passes through 3.9..5.15 before it can reach a bed ≤ 4.0, so one y catches everything.
use std::collections::BTreeMap;

use tmmaps::map::MapFile;

static ZONE_STEP_MILLI: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1000);
pub fn set_zone_step(m: f32) {
    ZONE_STEP_MILLI.store((m * 1000.0).round().max(10.0) as u32, std::sync::atomic::Ordering::Relaxed);
}
pub fn zone_step() -> f32 {
    ZONE_STEP_MILLI.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0
}

/// The band outside the grid, in cells: Stadium's flat grass ends at the stands within 6; the vistas' sea goes on
/// (v4.11, Hugo: "going out 16 cells each way instead of 6").
pub const RING_CELLS: i32 = 6;
pub const RING_CELLS_VISTA: i32 = 16;
pub fn ring_cells(collection: u32) -> i32 {
    if collection == 0x1a { RING_CELLS } else { RING_CELLS_VISTA }
}
/// Band raster for the Stadium flat-grass test.
pub const STEP: f32 = 4.0;
/// Stadium: a hit-shape face this far above the grass inside a square makes it structure (walls, seats, banks).
const WALL_ABOVE: f32 = 0.3;
const WALL_UNDER: f32 = 3.0;

#[derive(Clone, Debug)]
pub struct OuterRect {
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    /// the ground the slab is anchored on (Stadium grass 8.0; vista seabed anchor)
    pub floor: f32,
    pub kind: &'static str,
}

/// v4.13: a SLOPED ring wedge — a prism whose bottom / top / indicator are planar between the inner edge (at the
/// grid side, height `y_inner`) and the outer edge (`y_outer`), `side` = the outward direction ('N' = −z, 'S' = +z,
/// 'W' = −x, 'E' = +x). The heights are bed ENVELOPE values; the slab offsets (−0.1 / +1.15) are applied by the builder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ramp {
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    pub y_inner: f32,
    pub y_outer: f32,
    pub side: char,
}

impl Ramp {
    /// 0 at the inner edge, 1 at the outer edge.
    pub fn t(&self, x: f32, z: f32) -> f32 {
        match self.side {
            'N' => (self.z1 - z) / (self.z1 - self.z0).max(1e-6),
            'S' => (z - self.z0) / (self.z1 - self.z0).max(1e-6),
            'W' => (self.x1 - x) / (self.x1 - self.x0).max(1e-6),
            _ => (x - self.x0) / (self.x1 - self.x0).max(1e-6),
        }
    }
    pub fn y_at(&self, x: f32, z: f32) -> f32 {
        self.y_inner + (self.y_outer - self.y_inner) * self.t(x, z).clamp(0.0, 1.0)
    }
}

static RING_WEDGES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub fn set_ring_wedges(on: bool) {
    RING_WEDGES.store(on, std::sync::atomic::Ordering::Relaxed);
}
pub fn ring_wedges() -> bool {
    RING_WEDGES.load(std::sync::atomic::Ordering::Relaxed)
}

pub struct Ring {
    pub rects: Vec<OuterRect>,
    /// v4.13 vista wedges (empty when rows are used)
    pub ramps: Vec<Ramp>,
    /// per-square (x index, z index) → floor for the drawing tools (Stadium only; vistas are uniform)
    pub squares: BTreeMap<(i32, i32), f32>,
    pub notes: Vec<String>,
}

fn scene_tris(c: &crate::geom::Collector<'_>, group_starts: &str) -> Vec<[[f32; 3]; 3]> {
    let mut out = Vec::new();
    for (name, g) in &c.scene.groups {
        if !name.starts_with(group_starts) {
            continue;
        }
        for t in &g.tris {
            if let (Some(a), Some(b), Some(cc)) = (g.verts.get(t[0] as usize), g.verts.get(t[1] as usize), g.verts.get(t[2] as usize)) {
                out.push([*a, *b, *cc]);
            }
        }
    }
    out
}

fn all_tris(c: &crate::geom::Collector<'_>) -> Vec<(String, [[f32; 3]; 3])> {
    let mut out = Vec::new();
    for (name, g) in &c.scene.groups {
        for t in &g.tris {
            if let (Some(a), Some(b), Some(cc)) = (g.verts.get(t[0] as usize), g.verts.get(t[1] as usize), g.verts.get(t[2] as usize)) {
                out.push((name.clone(), [*a, *b, *cc]));
            }
        }
    }
    out
}

fn upward(t: &[[f32; 3]; 3]) -> bool {
    let (a, b, c) = (t[0], t[1], t[2]);
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    len > 1e-9 && n[1] / len > 0.7
}

/// Height of a triangle's plane at (x, z) when the point lies inside its xz projection.
fn height_at(t: &[[f32; 3]; 3], x: f32, z: f32) -> Option<f32> {
    let (a, b, c) = (t[0], t[1], t[2]);
    let det = (b[0] - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (b[2] - a[2]);
    if det.abs() < 1e-9 {
        return None;
    }
    let l1 = ((b[0] - x) * (c[2] - z) - (c[0] - x) * (b[2] - z)) / det;
    let l2 = ((c[0] - x) * (a[2] - z) - (a[0] - x) * (c[2] - z)) / det;
    let l3 = 1.0 - l1 - l2;
    if l1 < -1e-5 || l2 < -1e-5 || l3 < -1e-5 {
        return None;
    }
    Some(l1 * a[1] + l2 * b[1] + l3 * c[1])
}

/// The vista ring: 4 frame rectangles on one anchor.
fn vista_ring(store: &mut crate::store::DataStore, collection: u32, size: (i32, i32), cells: i32) -> Result<Ring, String> {
    let coll = crate::sttc::collection_name(collection);
    let water = crate::terrain::collection_water(store, coll)?;
    let path = format!("{coll}\\GameCtnDecoration\\Scene3d\\Base64x64.Scene3d.Gbx");
    let mut c = crate::geom::Collector::new(store);
    crate::scene3d::collect(&mut c, &path)?;
    let sea = scene_tris(&c, "Water");
    let ground = scene_tris(&c, "Warp");
    if sea.is_empty() || ground.is_empty() {
        return Err(format!("{path}: no Water / Warp* solids ({} groups: {:?})", c.scene.groups.len(), c.scene.groups.keys().collect::<Vec<_>>()));
    }
    let scene_water_y = sea.iter().map(|t| t[0][1]).fold(f32::NEG_INFINITY, f32::max);
    let yoff = water.top - scene_water_y;
    let (gx, gz) = (32.0 * size.0 as f32, 32.0 * size.1 as f32);
    let margin = 32.0 * cells as f32;
    // sample the band's bed on the 4 m raster (centres), the sea's coverage too; the per-32 m-cell maxima feed the zones
    let n = ((gx + 2.0 * margin) / STEP).round() as i32;
    let nz = ((gz + 2.0 * margin) / STEP).round() as i32;
    let (mut bed_min, mut bed_max, mut bed_sum, mut cnt, mut dry, mut no_bed) = (f32::INFINITY, f32::NEG_INFINITY, 0.0f64, 0usize, 0usize, 0usize);
    let mut per_ring: BTreeMap<i32, (f32, f32)> = BTreeMap::new();
    // 32 m cell (cx, cz) in the band frame (cell 0 = the first cell outside at −margin) → max bed
    let mut cell_max: BTreeMap<(i32, i32), f32> = BTreeMap::new();
    let ground_up: Vec<&[[f32; 3]; 3]> = ground.iter().filter(|t| upward(t)).collect();
    for j in 0..nz {
        for i in 0..n {
            let (x, z) = (-margin + (i as f32 + 0.5) * STEP, -margin + (j as f32 + 0.5) * STEP);
            if x >= 0.0 && x <= gx && z >= 0.0 && z <= gz {
                continue;
            }
            let bed = ground_up.iter().filter_map(|t| height_at(t, x, z)).fold(f32::INFINITY, f32::min);
            let wet = sea.iter().any(|t| height_at(t, x, z).is_some());
            if !wet {
                dry += 1;
            }
            if bed == f32::INFINITY {
                no_bed += 1;
                continue;
            }
            let bed = bed + yoff;
            bed_min = bed_min.min(bed);
            bed_max = bed_max.max(bed);
            bed_sum += bed as f64;
            cnt += 1;
            let d = (-x).max(x - gx).max(-z).max(z - gz);
            let e = per_ring.entry((d / 32.0).floor() as i32).or_insert((f32::INFINITY, f32::NEG_INFINITY));
            e.0 = e.0.min(bed);
            e.1 = e.1.max(bed);
            let key = (((x + margin) / 32.0).floor() as i32, ((z + margin) / 32.0).floor() as i32);
            let m = cell_max.entry(key).or_insert(f32::NEG_INFINITY);
            *m = m.max(bed);
        }
    }
    if cnt == 0 {
        return Err("outer ring: no seabed under the band".into());
    }
    // zones (v4.11): per 32 m cell the slab anchor = that cell's highest bed, up to the next 5 cm (never under the
    // bed); along each side of each ring, consecutive cells with the same anchor merge into one row. Rings are
    // frames: the N and S bands span the full width (corners included), E and W the grid's own z range.
    // the anchor quantum (ZONE_STEP, default 1 m — Hugo: "per 1 m bed step"): a coarser step merges more cells into one row; the slab then sits
    // up to one step above the cell's highest bed — still above every bed of the zone, which is all the rule needs
    let step = zone_step();
    let anchor_of = |v: f32| ((v - 0.005) / step).ceil() * step;
    let (ncx, ncz) = (size.0 + 2 * cells, size.1 + 2 * cells);
    let mut rects: Vec<OuterRect> = Vec::new();
    let mut push_run = |cx0: i32, cx1: i32, cz0: i32, cz1: i32, floor: f32| {
        rects.push(OuterRect { x0: -margin + 32.0 * cx0 as f32, x1: -margin + 32.0 * cx1 as f32, z0: -margin + 32.0 * cz0 as f32, z1: -margin + 32.0 * cz1 as f32, floor, kind: "sea" });
    };
    for d in 0..cells {
        // ring d: the frame of cells at distance d outside the grid (band frame indices)
        let (lo_x, hi_x, lo_z, hi_z) = (cells - 1 - d, size.0 + cells + d, cells - 1 - d, size.1 + cells + d);
        // N row (cz = lo_z) and S row (cz = hi_z): cx lo_x..=hi_x; W column (cx = lo_x) and E column (cx = hi_x): cz lo_z+1..hi_z
        for (cz, horizontal) in [(lo_z, true), (hi_z, true)] {
            let _ = horizontal;
            let mut cx = lo_x;
            while cx <= hi_x {
                let v = anchor_of(cell_max.get(&(cx, cz)).copied().unwrap_or(bed_max));
                let start = cx;
                while cx + 1 <= hi_x && anchor_of(cell_max.get(&(cx + 1, cz)).copied().unwrap_or(bed_max)) == v {
                    cx += 1;
                }
                push_run(start, cx + 1, cz, cz + 1, v);
                cx += 1;
            }
        }
        for cx in [lo_x, hi_x] {
            let mut cz = lo_z + 1;
            while cz < hi_z {
                let v = anchor_of(cell_max.get(&(cx, cz)).copied().unwrap_or(bed_max));
                let start = cz;
                while cz + 1 < hi_z && anchor_of(cell_max.get(&(cx, cz + 1)).copied().unwrap_or(bed_max)) == v {
                    cz += 1;
                }
                push_run(cx, cx + 1, start, cz + 1, v);
                cz += 1;
            }
        }
    }
    let _ = (ncx, ncz);
    let zones: std::collections::BTreeSet<i64> = rects.iter().map(|r| (r.floor * 100.0).round() as i64).collect();
    let mut notes = vec![
        format!("outer ring ({coll}, Base64x64 Scene3d, {cells} cells): scene frame = map frame + {yoff:.1} m in y (water type {:?} top {:.2} vs the scene's Water plane {scene_water_y:.2}); band −{margin:.0}..+{margin:.0} m: {cnt} squares of 4 m, {dry} dry, {no_bed} without a bed; seabed {bed_min:.2}..{bed_max:.2} (mean {:.2}); per ring cell (min..max): {}", water.name, water.top, bed_sum / cnt as f64, per_ring.iter().map(|(k, (lo, hi))| format!("{k}: {lo:.2}..{hi:.2}")).collect::<Vec<_>>().join(", ")),
        format!("zones: per 32 m cell the slab sits on that cell's highest bed (up to the next {step:.2} m), runs merged along each side → {} rows over {} distinct anchors ({:.2}..{:.2})", rects.len(), zones.len(), zones.iter().next().map(|v| *v as f32 / 100.0).unwrap_or(0.0), zones.iter().next_back().map(|v| *v as f32 / 100.0).unwrap_or(0.0)),
    ];
    // v4.13 WEDGES: the crest ENVELOPE vs distance — env[d] = max over rings d' ≥ d of that ring's max bed (so any
    // square at ring ≥ d, corners included, has bed ≤ env[d]), rounded up 0.25 m; the requirement at the integer
    // position k (the outer edge of cell k−1) is E[k] = env[k−1] (E[0] = env[0]); the fewest planar segments
    // through the knots (a, E[a]) → (b, E[b]) that stay ≥ E at every knot between (greedy longest from the edge).
    let mut env: Vec<f32> = (0..cells).map(|d| per_ring.get(&d).map(|e| e.1).unwrap_or(bed_max)).collect();
    for d in (0..cells as usize).rev() {
        if d + 1 < cells as usize {
            env[d] = env[d].max(env[d + 1]);
        }
    }
    let q = |v: f32| ((v - 0.001) / 0.25).ceil() * 0.25;
    let e: Vec<f32> = (0..=cells as usize).map(|k| if k == 0 { q(env[0]) } else { q(env[k - 1]) }).collect();
    let mut segs: Vec<(usize, usize)> = Vec::new();
    let mut a = 0usize;
    while a < cells as usize {
        let mut b = cells as usize;
        while b > a + 1 {
            let ok = (a + 1..b).all(|k| {
                let line = e[a] + (e[b] - e[a]) * ((k - a) as f32 / (b - a) as f32);
                line + 1e-4 >= e[k]
            });
            if ok {
                break;
            }
            b -= 1;
        }
        segs.push((a, b));
        a = b;
    }
    let mut ramps: Vec<Ramp> = Vec::new();
    for &(a, b) in &segs {
        let (d0, d1) = (32.0 * a as f32, 32.0 * b as f32);
        let (yi, yo) = (e[a], e[b]);
        // N / S bands span the full width (corners included); W / E the grid's z range
        ramps.push(Ramp { x0: -margin, x1: gx + margin, z0: -d1, z1: -d0, y_inner: yi, y_outer: yo, side: 'N' });
        ramps.push(Ramp { x0: -margin, x1: gx + margin, z0: gz + d0, z1: gz + d1, y_inner: yi, y_outer: yo, side: 'S' });
        ramps.push(Ramp { x0: -d1, x1: -d0, z0: 0.0, z1: gz, y_inner: yi, y_outer: yo, side: 'W' });
        ramps.push(Ramp { x0: gx + d0, x1: gx + d1, z0: 0.0, z1: gz, y_inner: yi, y_outer: yo, side: 'E' });
    }
    // clearance of every band square's bed under the wedge it falls in: top (envelope) and slab bottom (−0.1)
    let (vb, va) = crate::floor_finish::vista_slab(collection);
    let (mut c_min, mut c_max, mut c_sum, mut c_n, mut under) = (f32::INFINITY, f32::NEG_INFINITY, 0.0f64, 0usize, 0usize);
    for j in 0..nz {
        for i in 0..n {
            let (x, z) = (-margin + (i as f32 + 0.5) * STEP, -margin + (j as f32 + 0.5) * STEP);
            if x >= 0.0 && x <= gx && z >= 0.0 && z <= gz {
                continue;
            }
            let bed = ground_up.iter().filter_map(|t| height_at(t, x, z)).fold(f32::INFINITY, f32::min);
            if bed == f32::INFINITY {
                continue;
            }
            let bed = bed + yoff;
            let Some(r) = ramps.iter().find(|r| x >= r.x0 && x <= r.x1 && z >= r.z0 && z <= r.z1) else { continue };
            let c = r.y_at(x, z) - bed;
            c_min = c_min.min(c);
            c_max = c_max.max(c);
            c_sum += c as f64;
            c_n += 1;
            if c - vb < 0.0 {
                under += 1;
            }
        }
    }
    notes.push(format!("wedges (v4.13): envelope per cell {} → E at the knots {}; {} planar segment(s) per side {:?} → {} ramps (slab {:.2} under .. {:.2} over the envelope); bed-to-envelope clearance over {c_n} squares: min {c_min:.2} max {c_max:.2} mean {:.2}; squares where the slab BOTTOM would be under the bed: {under}", env.iter().map(|v| format!("{:.2}", v)).collect::<Vec<_>>().join(" "), e.iter().map(|v| format!("{:.2}", v)).collect::<Vec<_>>().join(" "), segs.len(), segs.iter().map(|(a, b)| format!("{a}→{b}: {:.2}→{:.2}", e[*a], e[*b])).collect::<Vec<_>>(), ramps.len(), vb, va, c_sum / c_n.max(1) as f64));
    if ring_wedges() {
        Ok(Ring { rects: Vec::new(), ramps, squares: BTreeMap::new(), notes })
    } else {
        Ok(Ring { rects, ramps: Vec::new(), squares: BTreeMap::new(), notes })
    }
}

/// A Stadium decoration piece placed in the deco map: (x, y, z) local → map frame.
struct Placement {
    origin: [f32; 2],
    /// dir 2: `deco = origin + (W − x, D − z)`; dir 0: `deco = origin + (x, z)`
    footprint: Option<[f32; 2]>,
}

impl Placement {
    fn to_map(&self, p: [f32; 3]) -> [f32; 3] {
        let (x, z) = match self.footprint {
            None => (self.origin[0] + p[0], self.origin[1] + p[2]),
            Some([w, d]) => (self.origin[0] + w - p[0], self.origin[1] + d - p[2]),
        };
        [x - 1280.0, p[1] - 120.0, z - 768.0]
    }
}

fn stadium_ring(store: &mut crate::store::DataStore, m: &MapFile, size: (i32, i32)) -> Result<Ring, String> {
    // the deco map behind the decoration id (48x48Day, 48x48Screen155Sunset, NoStadium48x48Night …)
    let deco_id = m.decoration_id.clone();
    let deco_map = if deco_id.starts_with("NoStadium") {
        "Stadium\\GameCtnDecoration\\Map\\DecoNoStadium48x48.Map.Gbx"
    } else if deco_id.contains("Screen155") {
        "Stadium\\GameCtnDecoration\\Map\\Deco48x48_Screen155.Map.Gbx"
    } else {
        "Stadium\\GameCtnDecoration\\Map\\Deco48x48.Map.Gbx"
    };
    let bytes = store.read(deco_map)?;
    let tmp = std::env::temp_dir().join(format!("mapgeom-deco-{}.Map.Gbx", std::process::id()));
    std::fs::write(&tmp, &bytes[..]).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let deco = MapFile::try_load(&tmp).map_err(|e| format!("{deco_map}: {e}"))?;
    let _ = std::fs::remove_file(&tmp);
    let mut notes = vec![format!("outer ring (Stadium, decoration {deco_id:?} → {deco_map}): {} blocks", deco.blocks.len())];
    // every placed piece: its hit shape + grass object, into the map frame
    let mut pieces: Vec<(String, [[f32; 3]; 3])> = Vec::new();
    for b in &deco.blocks {
        if b.flags == 0xFFFF_FFFF || b.name.eq_ignore_ascii_case("Grass") {
            continue;
        }
        let (cx, cz) = (b.coords().0, b.coords().2);
        let dir = b.dir;
        let origin = [256.0 * cx as f32, 256.0 * cz as f32];
        let (footprint, grass) = if b.name.starts_with("Stade1536") {
            ([1280.0, 1536.0], "Grass1536")
        } else if b.name.starts_with("Stade4096") {
            ([4096.0, 768.0], "Grass4096")
        } else {
            ([4096.0, 3072.0], "")
        };
        let pl = Placement { origin, footprint: if dir == 2 { Some(footprint) } else { None } };
        if dir != 0 && dir != 2 {
            notes.push(format!("  {} at ({cx},{cz}) dir {dir}: only dir 0/2 placements are understood — skipped", b.name));
            continue;
        }
        // the piece's prefab from the block info (Stade1536_Screen155 → Stade1536v2.Prefab: its own Mesh/HitShape,
        // the shared Grass1536); a `Solid\Warp` prefab splits its hit shape / grass into sibling files, any other
        // prefab (NoStadium: three inline sub-prefabs) is baked whole
        let prefab = crate::vista::find_block_info(store, &b.name).and_then(|bp| crate::blockinfo::load(store, &bp).ok()).and_then(|bi| {
            let v = bi.variant_base_air.as_ref().or(bi.variant_base_ground.as_ref())?;
            v.mobils.iter().flatten().find_map(|mb| mb.prefab.clone())
        });
        let Some(prefab) = prefab else {
            notes.push(format!("  {} at ({cx},{cz}): no prefab in its block info — skipped", b.name));
            continue;
        };
        let prefab = prefab.trim().to_string();
        let prefab = if prefab.to_ascii_lowercase().ends_with(".prefab.gbx") { prefab } else { format!("{prefab}.Prefab.Gbx") };
        let mut files: Vec<String> = Vec::new();
        if let Some((dir_name, file)) = prefab.rsplit_once('\\') {
            if dir_name.to_ascii_lowercase().ends_with("solid\\warp") {
                let stem = file.trim_end_matches(".Prefab.Gbx").trim_end_matches(".prefab.gbx");
                files.push(format!("{dir_name}\\{stem}.HitShape.Gbx"));
                if !grass.is_empty() {
                    files.push(format!("{dir_name}\\{grass}.StaticObject.Gbx"));
                }
            }
        }
        if files.is_empty() {
            // bake the prefab's static objects (collision) whole
            let mut mg = crate::static_item::build::Merged::default();
            match crate::static_item::build::add_prefab(store, &prefab, &crate::geom::IDENTITY, 1.0, &mut mg, 0) {
                Ok(_) => {
                    let mut n = 0;
                    for t in &mg.surf_triangles {
                        let pts: Vec<[f32; 3]> = t.indices.iter().filter_map(|&i| mg.surf_vertices.get(i as usize).copied()).collect();
                        if pts.len() == 3 {
                            pieces.push((format!("phys{}", t.material_id), [pl.to_map(pts[0]), pl.to_map(pts[1]), pl.to_map(pts[2])]));
                            n += 1;
                        }
                    }
                    notes.push(format!("  {} at ({cx},{cz}) dir {dir}: {prefab} baked whole: {n} collision triangles", b.name));
                }
                Err(e) => notes.push(format!("  {} at ({cx},{cz}): {prefab}: {e}", b.name)),
            }
            continue;
        }
        for f in files {
            let loaded = match store.load_model(&f) {
                Ok(l) => l,
                Err(e) => {
                    notes.push(format!("  {f}: {e}"));
                    continue;
                }
            };
            let mut c = crate::geom::Collector::new(store);
            c.model(&loaded, &crate::geom::IDENTITY, 0);
            let tris = all_tris(&c);
            notes.push(format!("  {} at ({cx},{cz}) dir {dir}: {f}: {} triangles", b.name, tris.len()));
            for (g, t) in tris {
                pieces.push((g, [pl.to_map(t[0]), pl.to_map(t[1]), pl.to_map(t[2])]));
            }
        }
    }
    if pieces.is_empty() {
        return Err(format!("{deco_map}: no decoration geometry loaded"));
    }
    let (gx, gz) = (32.0 * size.0 as f32, 32.0 * size.1 as f32);
    let margin = 32.0 * RING_CELLS as f32;
    let n = ((gx + 2.0 * margin) / STEP).round() as i32;
    let nz = ((gz + 2.0 * margin) / STEP).round() as i32;
    // bucket the triangles touching the band by 4 m square: lowest upward face min-y (floor), structure flag
    let mut low: BTreeMap<(i32, i32), f32> = BTreeMap::new();
    let mut wall: BTreeMap<(i32, i32), bool> = BTreeMap::new();
    let grass_y = 8.0f32;
    for (_g, t) in &pieces {
        let (x0, x1) = (t.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max));
        let (z0, z1) = (t.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[2]).fold(f32::NEG_INFINITY, f32::max));
        let (y0, y1) = (t.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[1]).fold(f32::NEG_INFINITY, f32::max));
        if x1 < -margin || x0 > gx + margin || z1 < -margin || z0 > gz + margin || y0 > grass_y + 40.0 {
            continue;
        }
        let (i0, i1) = ((((x0 + margin) / STEP).floor() as i32).max(0), (((x1 + margin) / STEP).floor() as i32).min(n - 1));
        let (j0, j1) = ((((z0 + margin) / STEP).floor() as i32).max(0), (((z1 + margin) / STEP).floor() as i32).min(nz - 1));
        let up = upward(t);
        for j in j0..=j1 {
            for i in i0..=i1 {
                if up {
                    let e = low.entry((i, j)).or_insert(f32::INFINITY);
                    *e = e.min(y0);
                }
                if y1 > grass_y + WALL_ABOVE && y0 < grass_y + WALL_UNDER + WALL_ABOVE {
                    wall.insert((i, j), true);
                }
            }
        }
    }
    let mut squares: BTreeMap<(i32, i32), f32> = BTreeMap::new();
    let (mut clear, mut flat_blocked, mut other, mut none) = (0usize, 0usize, 0usize, 0usize);
    let mut rows: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    for j in 0..nz {
        for i in 0..n {
            let (x, z) = (-margin + (i as f32 + 0.5) * STEP, -margin + (j as f32 + 0.5) * STEP);
            if x >= 0.0 && x <= gx && z >= 0.0 && z <= gz {
                continue;
            }
            match low.get(&(i, j)) {
                None => none += 1,
                Some(&y) if (y - grass_y).abs() <= 0.1 && !wall.contains_key(&(i, j)) => {
                    clear += 1;
                    squares.insert((i, j), grass_y);
                    rows.entry(j).or_default().push(i);
                }
                Some(&y) if (y - grass_y).abs() <= 0.1 => flat_blocked += 1,
                Some(_) => other += 1,
            }
        }
    }
    // strips along x per 4 m row
    let mut rects = Vec::new();
    for (j, mut xs) in rows {
        xs.sort_unstable();
        let mut k = 0;
        while k < xs.len() {
            let start = xs[k];
            let mut end = start;
            while k + 1 < xs.len() && xs[k + 1] == end + 1 {
                k += 1;
                end = xs[k];
            }
            k += 1;
            rects.push(OuterRect { x0: -margin + start as f32 * STEP, x1: -margin + (end + 1) as f32 * STEP, z0: -margin + j as f32 * STEP, z1: -margin + (j + 1) as f32 * STEP, floor: grass_y, kind: "grass" });
        }
    }
    // depth of the clear ring per side, sampled every 128 m, for the report
    let depth = |side: char| -> Vec<i32> {
        let mut out = Vec::new();
        let len = if side == 'W' || side == 'E' { gz } else { gx };
        let mut t = 64.0;
        while t < len {
            let mut d = 0;
            for k in 1..=(margin / STEP) as i32 {
                let (x, z) = match side {
                    'W' => (-(k as f32) * STEP + STEP / 2.0, t),
                    'E' => (gx + k as f32 * STEP - STEP / 2.0, t),
                    'N' => (t, -(k as f32) * STEP + STEP / 2.0),
                    _ => (t, gz + k as f32 * STEP - STEP / 2.0),
                };
                let key = (((x + margin) / STEP).floor() as i32, ((z + margin) / STEP).floor() as i32);
                if squares.contains_key(&key) {
                    d = k;
                } else {
                    break;
                }
            }
            out.push(d * STEP as i32);
            t += 128.0;
        }
        out
    };
    notes.push(format!("band 4 m squares: {clear} clear flat grass at {grass_y:.1} (→ {} row strips), {flat_blocked} flat but structure within {WALL_UNDER:.0} m above, {other} other floor heights, {none} without a floor; clear depth (m) every 128 m — W {:?} E {:?} N {:?} S {:?}", rects.len(), depth('W'), depth('E'), depth('N'), depth('S')));
    Ok(Ring { rects, ramps: Vec::new(), squares, notes })
}

/// The ring for a map: Stadium → the flat-grass strips of its decoration; vistas → 4 sea frame boxes.
pub fn ring(store: &mut crate::store::DataStore, m: &MapFile, collection: u32) -> Result<Ring, String> {
    ring_cells_of(store, m, collection, ring_cells(collection))
}

pub fn ring_cells_of(store: &mut crate::store::DataStore, m: &MapFile, collection: u32, cells: i32) -> Result<Ring, String> {
    let size = (m.size[0] as i32, m.size[2] as i32);
    if collection == 0x1a {
        stadium_ring(store, m, size)
    } else {
        vista_ring(store, collection, size, cells)
    }
}

/// Append ring boxes to a floor mesh: Stadium rects as slabs floor−below..floor+above (the plane's own), vista
/// rects on the vista slab (bed − 0.1 .. bed + 1.15). Returns the number of boxes added.
pub fn append_to_mesh(mesh: &mut crate::floor_finish::FloorMesh, ring: &Ring, collection: u32, below: f32, above: f32) -> usize {
    let quads: [[u32; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [1, 2, 6, 5], [0, 4, 7, 3]];
    let (vb, va) = crate::floor_finish::vista_slab(collection);
    let mut n = 0;
    for r in &ring.rects {
        let (y0, y1) = if collection == 0x1a { (r.floor - below, r.floor + above) } else { (r.floor - vb, r.floor + va) };
        let base = mesh.verts.len() as u32;
        mesh.verts.extend_from_slice(&[[r.x0, y0, r.z0], [r.x1, y0, r.z0], [r.x1, y1, r.z0], [r.x0, y1, r.z0], [r.x0, y0, r.z1], [r.x1, y0, r.z1], [r.x1, y1, r.z1], [r.x0, y1, r.z1]]);
        for q in quads {
            mesh.tris.push([base + q[0], base + q[1], base + q[2]]);
            mesh.tris.push([base + q[0], base + q[2], base + q[3]]);
        }
        for v in &mesh.verts[base as usize..] {
            for k in 0..3 {
                mesh.bbox.0[k] = mesh.bbox.0[k].min(v[k]);
                mesh.bbox.1[k] = mesh.bbox.1[k].max(v[k]);
            }
        }
        mesh.quads.push((r.x0, r.x1, r.z0, r.z1, r.floor));
        n += 1;
    }
    mesh.notes.extend(ring.notes.iter().cloned());
    mesh.notes.push(format!("outer ring: {n} boxes appended, {} ramps", ring.ramps.len()));
    n
}

/// `OuterTerrain.tsv` rows for the C#: collection, decoration (Stadium: the deco map's stem; vistas "Base64x64"),
/// x0 x1 z0 z1 (m, map frame), floor, kind.
pub fn tsv_rows(collection_name: &str, decoration: &str, ring: &Ring) -> String {
    let mut s = String::new();
    for r in &ring.rects {
        s.push_str(&format!("{collection_name}\t{decoration}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.2}\t{}\t{:.2}\t-\n", r.x0, r.x1, r.z0, r.z1, r.floor, r.kind, r.floor));
    }
    for r in &ring.ramps {
        s.push_str(&format!("{collection_name}\t{decoration}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.2}\tramp\t{:.2}\t{}\n", r.x0, r.x1, r.z0, r.z1, r.y_inner, r.y_outer, r.side));
    }
    s
}
