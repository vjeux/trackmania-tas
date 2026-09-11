//! The self-check every build runs before writing the map: nothing the car
//! can touch may stand above the road surface anywhere on the lap, the road
//! must be continuous, and its vertical profile must not launch a car at
//! racing speed. Measured on the placements' own world-space collision
//! triangles — the same bytes the game gets — not on the source data.
//!
//! Written after the first lap video: the car left the ground at 14 places
//! (1.09 m at Chapel Curve). Coarse 24 m terrain cells straddling a 13 m road
//! had all four corners outside the sink margin and their flat plane cut
//! through the tarmac; a checker on the source heights would not have seen
//! it, the triangles do.

use crate::edges::Edges;
use crate::mapbuild::{Frame, Placement};
use crate::track::Track;
use std::collections::HashMap;

const CELL: f32 = 4.0;

struct Grid {
    cells: HashMap<(i32, i32), Vec<(usize, usize)>>, // (placement, tri)
}

fn cell_of(x: f32, z: f32) -> (i32, i32) {
    ((x / CELL).floor() as i32, (z / CELL).floor() as i32)
}

impl Grid {
    fn build(pl: &[Placement]) -> Grid {
        let mut cells: HashMap<(i32, i32), Vec<(usize, usize)>> = HashMap::new();
        for (pi, p) in pl.iter().enumerate() {
            for (ti, t) in p.coll.iter().enumerate() {
                let (mut x0, mut z0, mut x1, mut z1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for q in t {
                    x0 = x0.min(q[0]);
                    z0 = z0.min(q[2]);
                    x1 = x1.max(q[0]);
                    z1 = z1.max(q[2]);
                }
                let (a, b) = (cell_of(x0, z0), cell_of(x1, z1));
                // skip absurdly large triangles' full fan-out (a 256 m terrain
                // tile has 24 m cells: 7x7 cells at most — fine)
                for cx in a.0..=b.0 {
                    for cz in a.1..=b.1 {
                        cells.entry((cx, cz)).or_default().push((pi, ti));
                    }
                }
            }
        }
        Grid { cells }
    }
}

/// Height of triangle `t` at (x, z), None when the point is outside its
/// projection or the triangle is (near) vertical.
fn height_at(t: &[[f32; 3]; 3], x: f32, z: f32) -> Option<f32> {
    let (ax, az) = (t[0][0] as f64, t[0][2] as f64);
    let (bx, bz) = (t[1][0] as f64, t[1][2] as f64);
    let (cx, cz) = (t[2][0] as f64, t[2][2] as f64);
    let (px, pz) = (x as f64, z as f64);
    let det = (bx - ax) * (cz - az) - (cx - ax) * (bz - az);
    if det.abs() < 1e-3 {
        return None; // vertical or degenerate in plan
    }
    let l1 = ((bx - px) * (cz - pz) - (cx - px) * (bz - pz)) / det;
    let l2 = ((cx - px) * (az - pz) - (ax - px) * (cz - pz)) / det;
    let l0 = 1.0 - l1 - l2;
    let eps = -1e-4;
    if l0 < eps || l1 < eps || l2 < eps {
        return None;
    }
    // l0 weights a? (barycentric as derived: l1 -> a, l2 -> b, l0 -> c)
    Some((l1 * t[0][1] as f64 + l2 * t[1][1] as f64 + l0 * t[2][1] as f64) as f32)
}

pub struct Offender {
    pub ident: String,
    pub kind: &'static str,
    pub first_station: usize,
    pub last_station: usize,
    pub worst: f32,
    pub count: usize,
}

pub struct Report {
    pub offenders: Vec<Offender>,
    pub holes: Vec<(usize, f64)>,
    pub crests: Vec<(usize, f64)>,
    pub queries: usize,
}

impl Report {
    pub fn fatal(&self) -> bool {
        self.offenders.iter().any(|o| o.kind == "above tarmac") || !self.holes.is_empty()
    }
}

/// Which placements are a road (their surface IS the reference).
fn is_road(ident: &str) -> bool {
    let stem = ident.rsplit('\\').next().unwrap_or(ident);
    stem.starts_with("Road") || stem.starts_with("Start") || stem.starts_with("Finish") || stem.starts_with("Checkpoint") || stem.starts_with("Tarmac")
}

/// The same surface test over an extra road (its drawn slices only; no
/// hole or crest test, and two roads meeting at the same height is fine).
pub fn run_road(pl: &[Placement], road: &crate::roads::Road, fr: &Frame) -> Report {
    let (tr, ed, skip) = (&road.track, &road.edges, &road.skip);
    let grid = Grid::build(pl);
    let mut hits: HashMap<(usize, &'static str), Offender> = HashMap::new();
    let mut queries = 0usize;
    for i in 0..tr.len() {
        if skip[i] {
            continue;
        }
        let (l, r) = (ed.left[i], ed.right[i]);
        let mut off = -r + 0.3;
        while off <= l - 0.3 {
            let w = tr.offset(i, off);
            let p = fr.to_tm(w[0], w[1], road.drawn_height(i, off));
            queries += 1;
            if let Some(list) = grid.cells.get(&cell_of(p[0], p[2])) {
                for &(pi, ti) in list {
                    let Some(y) = height_at(&pl[pi].coll[ti], p[0], p[2]) else { continue };
                    let d = y - p[1];
                    if d > 0.03 && d < 3.5 {
                        let e = hits.entry((pi, "above tarmac")).or_insert(Offender { ident: pl[pi].ident.clone(), kind: "above tarmac", first_station: i, last_station: i, worst: 0.0, count: 0 });
                        e.last_station = i;
                        e.worst = e.worst.max(d);
                        e.count += 1;
                    }
                }
            }
            off += 0.5;
        }
    }
    let mut offenders: Vec<Offender> = hits.into_values().collect();
    offenders.sort_by(|a, b| b.worst.partial_cmp(&a.worst).unwrap());
    Report { offenders, holes: Vec::new(), crests: Vec::new(), queries }
}

pub fn run(pl: &[Placement], tr: &Track, ed: &Edges, fr: &Frame, v_max: f64) -> Report {
    let grid = Grid::build(pl);
    let mut hits: HashMap<(usize, &'static str), Offender> = HashMap::new();
    let mut holes = Vec::new();
    let mut queries = 0usize;
    let n = tr.len();
    for i in 0..n {
        let (l, r) = (ed.left[i], ed.right[i]);
        let (kl, kr) = (ed.kerb_left[i].min(4.0), ed.kerb_right[i].min(4.0));
        // across the tarmac and the kerbs, every 0.5 m
        let lo = -(r + kr + 0.5);
        let hi = l + kl + 0.5;
        let mut off = lo;
        while off <= hi {
            let w = tr.offset(i, off);
            let p = fr.to_tm(w[0], w[1], w[2]);
            let on_tarmac = off > -r + 0.3 && off < l - 0.3;
            let on_kerb = !on_tarmac && off > -(r + kr) && off < l + kl;
            queries += 1;
            let c = cell_of(p[0], p[2]);
            let mut road_here = false;
            if let Some(list) = grid.cells.get(&c) {
                for &(pi, ti) in list {
                    let t = &pl[pi].coll[ti];
                    let Some(y) = height_at(t, p[0], p[2]) else { continue };
                    let d = y - p[1];
                    let road_item = is_road(&pl[pi].ident);
                    if road_item && d.abs() < 0.05 {
                        road_here = true;
                    }
                    // something above the driving surface (up to 3.5 m: a
                    // bridge deck above that is legitimate)
                    // another road meeting the lap flush may sit 2 cm proud
                    // (an expansion joint); anything else 1 cm
                    let tol = if pl[pi].ident.contains("Tarmac") { 0.02 } else { 0.01 };
                    if d > tol && d < 3.5 && (on_tarmac || on_kerb) {
                        let kind = if on_tarmac { "above tarmac" } else { "above kerb" };
                        if std::env::var_os("CIRCUIT_DEBUG_CHECK").is_some() && d > 0.03 {
                            println!("  debug: station {i} off {off:+.1} (l {l:.1} kl {kl:.1} r {r:.1} kr {kr:.1}) road y {:.2}; {} tri y {y:.2} at {:?}", p[1], pl[pi].ident, t);
                        }
                        let e = hits.entry((pi, kind)).or_insert(Offender { ident: pl[pi].ident.clone(), kind, first_station: i, last_station: i, worst: 0.0, count: 0 });
                        e.last_station = i;
                        e.worst = e.worst.max(d);
                        e.count += 1;
                    }
                }
            }
            if on_tarmac && !road_here {
                holes.push((i, off));
            }
            off += 0.5;
        }
    }
    // vertical profile: a crest whose radius is below v^2/g throws the car
    // (downward curvature k = z'' ; airborne when v^2 k > g)
    let mut crests = Vec::new();
    let g = 9.81;
    let k_lim = g / (v_max * v_max);
    for i in 1..n - 1 {
        let k = tr.stations[i - 1].z - 2.0 * tr.stations[i].z + tr.stations[i + 1].z; // ds = 1 m
        if -k > k_lim {
            crests.push((i, -k));
        }
    }
    let mut offenders: Vec<Offender> = hits.into_values().collect();
    offenders.sort_by(|a, b| b.worst.partial_cmp(&a.worst).unwrap());
    Report { offenders, holes, crests, queries }
}

pub fn print(r: &Report) {
    println!("check: {} surface queries along the lap", r.queries);
    if r.offenders.is_empty() && r.holes.is_empty() && r.crests.is_empty() {
        println!("check: CLEAN -- nothing above the tarmac or the kerbs, no holes, no launching crests");
        return;
    }
    for o in &r.offenders {
        println!("check: {:<12} {:<40} stations {:>4}..{:<4} worst +{:.3} m ({} samples)", o.kind, o.ident, o.first_station, o.last_station, o.worst, o.count);
    }
    if !r.holes.is_empty() {
        println!("check: {} tarmac samples with NO road surface (holes), first at station {} off {:.1}", r.holes.len(), r.holes[0].0, r.holes[0].1);
    }
    if !r.crests.is_empty() {
        let worst = r.crests.iter().cloned().fold((0usize, 0.0f64), |a, b| if b.1 > a.1 { b } else { a });
        println!("check: {} launching crests, worst at station {} (curvature {:.5} /m, radius {:.0} m)", r.crests.len(), worst.0, worst.1, 1.0 / worst.1);
    }
}
