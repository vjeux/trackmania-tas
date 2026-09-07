//! FEATURE_VERSION 2 — everything car-relative, gravity the only world reference
//! (coordinator ← vjeux, 2026-09-07 06:26Z). Geometry comes from GEOM's
//! `mapgeom::local::LocalScene` (every collision triangle tagged with its
//! placement family and gameplay special), not the plumb grid: rays see walls,
//! tunnels, loops, wallrides and stacked roads; path samples follow the LOCAL
//! surface's down, so a bank, a wallride and a drop are one representation.
//!
//! Blocks (each ablatable by name, `BLOCKS2`):
//!   state     car velocity/speed/vertical speed, up, angular velocity, wheels (contact, material class,
//!             contact normal — present flag 0 until GEN's records carry them), gear/rpm/turbo, active
//!             effects one-hot + remaining + strength, car kind one-hot (present flag 0 until located)
//!   path      10 samples along the ground-following / ballistic arc for the next h at current speed
//!             × 3 lateral offsets; per sample: surface normal (car frame), height rel car, nearest surface
//!             below and above, material class, family class, physics special, hit flag
//!   rays      13 yaws × 5 pitches in the car frame: distance/120, hit, normal·gravity, material, family, special
//!   obstacles 8 nearest collidable ITEMS within 80 m: rel pos, radius, family
//!   cells     5×5×3 block-family neighbourhood rotated to the heading
//!   target    the v1 target/normal blocks + gate kind one-hot + gates-collected share + h
//!   chord     8 chord points with the path-sample values
//!
//! Family and material are COARSE classes (8 and 5): with a flat MLP a one-hot
//! IS a learned embedding (one-hot × W1), and the row cache must stay small.

use crate::frame;
use mapgeom::local::{LocalScene, PlacementKind, FAMILIES};
use tmreach::tmr::CarState;
use tmroute::gates::WpKind;

pub const FEATURE_VERSION_2: u32 = 2;

pub const N_FAM: usize = 8; // road, dirt/grass/sand, ice/bump/water-road, platform, gate, wall/structure, terrain/water, other
pub const N_MAT: usize = 5; // hard road, dirt/sand, grass, ice/snow, water/other
pub const PER_SAMPLE: usize = 7 + N_MAT + N_FAM + 1; // normal 3, h_rel, below, above, hit, mat, fam, special = 21
pub const PER_RAY: usize = 3 + N_MAT + N_FAM + 1; // dist, hit, n·g, mat, fam, special = 17
pub const PATH_N: usize = 10;
pub const PATH_LAT: [f32; 3] = [-8.0, 0.0, 8.0];
pub const RAY_YAWS_DEG: [f32; 13] = [-90.0, -60.0, -40.0, -25.0, -15.0, -8.0, 0.0, 8.0, 15.0, 25.0, 40.0, 60.0, 90.0];
pub const RAY_PITCH_DEG: [f32; 5] = [-30.0, -10.0, 0.0, 15.0, 40.0];
pub const RAY_MAX_M: f32 = 120.0;
pub const N_OBST: usize = 8;
pub const OBST_RANGE_M: f32 = 80.0;
pub const CELLS: (i32, i32, i32) = (5, 5, 3); // lateral × ahead × up
pub const CHORD_N: usize = 8;

pub const OFF2_STATE: usize = 0;
pub const LEN_STATE: usize = 75;
pub const OFF2_PATH: usize = OFF2_STATE + LEN_STATE;
pub const LEN_PATH: usize = PATH_N * PATH_LAT.len() * PER_SAMPLE; // 630
pub const OFF2_RAYS: usize = OFF2_PATH + LEN_PATH;
pub const LEN_RAYS: usize = RAY_YAWS_DEG.len() * RAY_PITCH_DEG.len() * PER_RAY; // 1105
pub const OFF2_OBST: usize = OFF2_RAYS + LEN_RAYS;
pub const LEN_OBST: usize = N_OBST * (4 + N_FAM); // 96
pub const OFF2_CELLS: usize = OFF2_OBST + LEN_OBST;
pub const LEN_CELLS: usize = (CELLS.0 * CELLS.1 * CELLS.2) as usize * N_FAM; // 600
pub const OFF2_TARGET: usize = OFF2_CELLS + LEN_CELLS;
pub const LEN_TARGET: usize = 18;
pub const OFF2_CHORD: usize = OFF2_TARGET + LEN_TARGET;
pub const LEN_CHORD: usize = CHORD_N * PER_SAMPLE; // 168
pub const DIM2: usize = OFF2_CHORD + LEN_CHORD; // 2692

pub const BLOCKS2: &[(&str, usize, usize)] = &[
    ("state", OFF2_STATE, OFF2_PATH),
    ("path", OFF2_PATH, OFF2_RAYS),
    ("rays", OFF2_RAYS, OFF2_OBST),
    ("obstacles", OFF2_OBST, OFF2_CELLS),
    ("cells", OFF2_CELLS, OFF2_TARGET),
    ("target", OFF2_TARGET, OFF2_CHORD),
    ("chord", OFF2_CHORD, DIM2),
];

pub fn mask_blocks2(x: &mut [f32], keep: &[&str]) {
    for (name, lo, hi) in BLOCKS2 {
        if !keep.contains(name) {
            for v in &mut x[*lo..*hi] {
                *v = 0.0;
            }
        }
    }
}

pub fn ablation_keep2(name: &str) -> Option<Vec<&'static str>> {
    let all: Vec<&str> = BLOCKS2.iter().map(|b| b.0).collect();
    match name {
        "full" => Some(all),
        "no-rays" => Some(all.into_iter().filter(|b| *b != "rays").collect()),
        "no-path" => Some(all.into_iter().filter(|b| *b != "path").collect()),
        "no-cells" => Some(all.into_iter().filter(|b| *b != "cells").collect()),
        "no-obstacles" => Some(all.into_iter().filter(|b| *b != "obstacles").collect()),
        "no-geometry" => Some(vec!["state", "target"]),
        "distance-only" => Some(vec!["target"]),
        _ => None,
    }
}

/// Coarse family class of a `FAMILIES` id (255 = miss/other).
pub fn fam_class_of_name(f: &str) -> usize {
    let l = f.to_ascii_lowercase();
    if l.starts_with("gate") {
        return 4;
    }
    if l.contains("wallride") || l.contains("loop") || l.contains("tunnel") || l.contains("ramp") || l.contains("booster") || l.contains("turbo") || l.contains("reactor") {
        return 5;
    }
    if l.contains("ice") || l.contains("bump") || l.contains("roadwater") || l.contains("snow") {
        return 2;
    }
    if l.contains("dirt") || l.contains("grass") || l.contains("sand") {
        return 1;
    }
    if l.starts_with("road") || l.starts_with("open") {
        return 0;
    }
    if l.starts_with("platform") || l.contains("decoplatform") {
        return 3;
    }
    if l.contains("wall") || l.contains("stadium") || l.contains("technics") || l.contains("pillar") || l.contains("fence") || l.contains("rail") || l.contains("sculpture") || l.contains("screen") || l.contains("advert") || l.contains("light") || l.contains("truss") || l.contains("support") {
        return 5;
    }
    if l.contains("land") || l.contains("beach") || l.contains("sea") || l.contains("cliff") || l.contains("hill") || l.contains("rock") || l.contains("water") || l.contains("tree") || l.contains("terrain") || l.contains("deco") {
        return 6;
    }
    7
}

pub fn mat_class(mat: u8) -> usize {
    match mapgeom::scene::physics_name(mat) {
        "Asphalt" | "Concrete" | "Pavement" | "Metal" | "Wood" | "Tech" | "Rubber" | "Stone" | "Rock" | "ResonantMetal" | "TechArmor" | "TechSafe" | "SlidingWood" | "SlidingRubber" | "WetAsphalt" | "WetPavement" | "MetalTrans" | "Plastic" => 0,
        "Dirt" | "DirtRoad" | "Sand" | "WetDirtRoad" => 1,
        "Grass" | "WetGrass" => 2,
        "Ice" | "Snow" => 3,
        _ => 4,
    }
}

/// One item placement's bounding sphere (for the obstacle block).
#[derive(Clone, Debug)]
pub struct Obstacle {
    pub centre: [f32; 3],
    pub radius: f32,
    pub fam: usize,
}

/// The v2 geometry provider: the LocalScene plus tables derived once per map.
pub struct Geo2<'a> {
    pub scene: &'a LocalScene,
    /// FAMILIES id (0..255) → coarse class.
    pub fam_class: Vec<usize>,
    pub obstacles: Vec<Obstacle>,
    /// Block-family class per map cell (32 m × 8 m × 32 m), keyed (cx, cy, cz).
    pub cells: std::collections::HashMap<(i32, i32, i32), usize>,
    pub yoff: f32,
}

impl<'a> Geo2<'a> {
    pub fn new(scene: &'a LocalScene, map: &tmmaps::map::MapFile, yoff: f32) -> Geo2<'a> {
        let mut fam_class = vec![7usize; 256];
        for (i, f) in FAMILIES.iter().enumerate() {
            fam_class[i] = fam_class_of_name(f);
        }
        // obstacles: bounding spheres of collidable ITEM placements
        let mut lo: Vec<[f32; 3]> = vec![[f32::INFINITY; 3]; scene.placements.len()];
        let mut hi: Vec<[f32; 3]> = vec![[f32::NEG_INFINITY; 3]; scene.placements.len()];
        let mut any = vec![false; scene.placements.len()];
        for t in &scene.tris {
            let p = t.tag as usize;
            if scene.placements[p].kind != PlacementKind::Item {
                continue;
            }
            let name = mapgeom::scene::physics_name(t.mat);
            if name == "NotCollidable" || name == "OffZone" {
                continue;
            }
            any[p] = true;
            for v in &t.v {
                for a in 0..3 {
                    lo[p][a] = lo[p][a].min(v[a]);
                    hi[p][a] = hi[p][a].max(v[a]);
                }
            }
        }
        let mut obstacles = Vec::new();
        for (p, pl) in scene.placements.iter().enumerate() {
            if !any[p] {
                continue;
            }
            let c = [(lo[p][0] + hi[p][0]) / 2.0, (lo[p][1] + hi[p][1]) / 2.0, (lo[p][2] + hi[p][2]) / 2.0];
            let r = ((hi[p][0] - lo[p][0]).powi(2) + (hi[p][1] - lo[p][1]).powi(2) + (hi[p][2] - lo[p][2]).powi(2)).sqrt() / 2.0;
            if r > 60.0 {
                continue; // a huge item is scenery, not an obstacle the rays undersample
            }
            obstacles.push(Obstacle { centre: c, radius: r, fam: fam_class_of_name(&pl.family) });
        }
        // cells: grid blocks by family
        let mut cells = std::collections::HashMap::new();
        for b in &map.blocks {
            let cx = b.raw_coords[0] as i32 - 1;
            let cy = b.raw_coords[1] as i32;
            let cz = b.raw_coords[2] as i32 - 1;
            let fam = fam_class_of_name(&mapgeom::local::family_of(&b.name));
            cells.insert((cx, cy, cz), fam);
        }
        Geo2 { scene, fam_class, obstacles, cells, yoff }
    }

    fn cell_of(&self, p: [f32; 3]) -> (i32, i32, i32) {
        ((p[0] / 32.0).floor() as i32, ((p[1] - self.yoff) / 8.0).floor() as i32, (p[2] / 32.0).floor() as i32)
    }

    /// Nearest surface along `dir` from `origin`: (dist, normal facing the origin, mat class, fam class, special).
    fn cast(&self, origin: [f32; 3], dir: [f32; 3], max_m: f32) -> Option<(f32, [f32; 3], usize, usize, bool)> {
        let (dist, ti) = self.scene.raycast_raw(origin, dir, max_m, true)?;
        let t = &self.scene.tris[ti as usize];
        let e1 = [t.v[1][0] - t.v[0][0], t.v[1][1] - t.v[0][1], t.v[1][2] - t.v[0][2]];
        let e2 = [t.v[2][0] - t.v[0][0], t.v[2][1] - t.v[0][1], t.v[2][2] - t.v[0][2]];
        let mut n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = frame::norm3(n).max(1e-9);
        n = [n[0] / l, n[1] / l, n[2] / l];
        if n[0] * dir[0] + n[1] * dir[1] + n[2] * dir[2] > 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        let pl = &self.scene.placements[t.tag as usize];
        let fam = fam_class_of_name(&pl.family);
        let special = !matches!(pl.special, mapgeom::local::Special::None | mapgeom::local::Special::Checkpoint | mapgeom::local::Special::Finish | mapgeom::local::Special::Start | mapgeom::local::Special::Multilap);
        Some((dist, n, mat_class(t.mat), fam, special))
    }
}

fn one_hot(out: &mut [f32], k: usize) {
    for v in out.iter_mut() {
        *v = 0.0;
    }
    if k < out.len() {
        out[k] = 1.0;
    }
}

/// Per-sample values of a ground point: cast down from `above` metres over `p`
/// along `down` (unit), look up for the layer above.
fn sample_at(geo: &Geo2, m: &[[f32; 3]; 3], car_y: f32, p: [f32; 3], down: [f32; 3], out: &mut [f32]) -> Option<([f32; 3], [f32; 3])> {
    for v in out.iter_mut() {
        *v = 0.0;
    }
    let up = [-down[0], -down[1], -down[2]];
    let origin = [p[0] + up[0] * 2.0, p[1] + up[1] * 2.0, p[2] + up[2] * 2.0];
    let hit = geo.cast(origin, down, 14.0);
    let above = geo.cast(origin, up, 30.0);
    out[5] = above.map_or(1.0, |(d, ..)| d / 30.0);
    match hit {
        Some((d, n, mat, fam, special)) => {
            let nc = frame::to_car(m, n);
            out[0] = nc[0];
            out[1] = nc[1];
            out[2] = nc[2];
            let hp = [origin[0] + down[0] * d, origin[1] + down[1] * d, origin[2] + down[2] * d];
            out[3] = ((hp[1] - car_y) / 20.0).clamp(-3.0, 3.0);
            out[4] = (d - 2.0) / 12.0;
            out[6] = 1.0;
            one_hot(&mut out[7..7 + N_MAT], mat);
            one_hot(&mut out[7 + N_MAT..7 + N_MAT + N_FAM], fam);
            out[7 + N_MAT + N_FAM] = if special { 1.0 } else { 0.0 };
            Some((hp, n))
        }
        None => {
            out[4] = 1.0;
            one_hot(&mut out[7 + N_MAT..7 + N_MAT + N_FAM], 7);
            None
        }
    }
}

/// Target kinds for the target block's one-hot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TargetKind {
    Checkpoint,
    Finish,
    Multilap,
    LocalPoint,
}

impl TargetKind {
    pub fn of_wp(k: WpKind) -> TargetKind {
        match k {
            WpKind::Checkpoint => TargetKind::Checkpoint,
            WpKind::Finish => TargetKind::Finish,
            WpKind::Multilap => TargetKind::Multilap,
            WpKind::Start => TargetKind::Checkpoint,
        }
    }
}

pub struct Target2 {
    pub centre: [f32; 3],
    pub normal: [f32; 3],
    pub half_width: f32,
    pub group_size: u32,
    pub kind: TargetKind,
    /// Share of the map's checkpoint groups already credited at the start.
    pub collected_share: f32,
}

/// Fill `out` (len DIM2).
pub fn features2(s: &CarState, t: &Target2, geo: &Geo2, h_ticks: u16, out: &mut [f32]) {
    assert_eq!(out.len(), DIM2);
    for o in out.iter_mut() {
        *o = 0.0;
    }
    let m = frame::matrix(s.quat);
    let speed = if s.speed.is_finite() { s.speed } else { frame::norm3(s.vel) };
    // ── state
    let v = frame::to_car(&m, s.vel);
    let o = OFF2_STATE;
    out[o] = speed / 100.0;
    out[o + 1] = v[0] / 100.0;
    out[o + 2] = v[1] / 100.0;
    out[o + 3] = v[2] / 100.0;
    out[o + 4] = s.vel[1] / 50.0;
    let up = frame::to_car(&m, [0.0, 1.0, 0.0]);
    out[o + 5] = up[0];
    out[o + 6] = up[1];
    out[o + 7] = up[2];
    if s.ang_vel.iter().all(|a| a.is_finite()) {
        out[o + 8] = 1.0;
        for k in 0..3 {
            out[o + 9 + k] = (s.ang_vel[k] / 5.0).clamp(-3.0, 3.0);
        }
    }
    // wheels: flag, contact 4, material class 4×5, normal 4×3 (normals absent from the records: stay 0)
    if s.wheel_contact.iter().all(|c| *c != u8::MAX) {
        out[o + 12] = 1.0;
        for k in 0..4 {
            out[o + 13 + k] = if s.wheel_contact[k] != 0 { 1.0 } else { 0.0 };
            if s.wheel_material[k] != u8::MAX {
                one_hot(&mut out[o + 17 + 5 * k..o + 22 + 5 * k], mat_class(s.wheel_material[k]));
            }
        }
    }
    // o+37..o+49 wheel contact normals (reserved), o+49 gear/rpm/turbo flag + 3
    if s.gear != u8::MAX && s.rpm.is_finite() {
        out[o + 49] = 1.0;
        out[o + 50] = s.gear as f32 / 6.0;
        out[o + 51] = s.rpm / 12000.0;
        out[o + 52] = if s.turbo.is_finite() { s.turbo } else { 0.0 };
    }
    // o+53 effects flag + 10 one-hot + remaining + strength (o+53..o+66): reserved, 0
    // o+66 car kind: flag + one-hot (Stadium, Snow, Rally, Desert) when the record carries it
    if s.car != u8::MAX {
        out[o + 66] = 1.0;
        one_hot(&mut out[o + 67..o + 71], (s.car as usize).min(3));
    }
    debug_assert!(o + 71 <= OFF2_PATH);

    // ── path samples: ground-following / ballistic arc at current speed for h
    let fwd = frame::rotate(&m, [0.0, 0.0, 1.0]);
    let right = frame::rotate(&m, [1.0, 0.0, 0.0]);
    let fh = (fwd[0] * fwd[0] + fwd[2] * fwd[2]).sqrt();
    let heading = if fh > 1e-3 { [fwd[0] / fh, 0.0, fwd[2] / fh] } else { [0.0, 0.0, 1.0] };
    let _ = right;
    let lat = [heading[2], 0.0, -heading[0]];
    let total_m = crate::estimator::reach_m(speed, h_ticks);
    let dt = h_ticks as f32 / 100.0 / PATH_N as f32;
    // local down under the car
    let under = geo.cast([s.pos[0], s.pos[1] + 1.5, s.pos[2]], [0.0, -1.0, 0.0], 14.0);
    let mut down0 = match under {
        Some((_, n, ..)) => [-n[0], -n[1], -n[2]],
        None => [0.0, -1.0, 0.0],
    };
    if down0[1] > -0.2 {
        down0 = [0.0, -1.0, 0.0];
    }
    for (li, loff) in PATH_LAT.iter().enumerate() {
        let mut p = [s.pos[0] + lat[0] * loff, s.pos[1], s.pos[2] + lat[2] * loff];
        let mut dir = heading;
        let mut down = down0;
        let mut vy = s.vel[1];
        let mut airborne = false;
        for k in 0..PATH_N {
            let ds = total_m / PATH_N as f32;
            // advance along the surface-projected direction
            let dn = dir[0] * down[0] + dir[1] * down[1] + dir[2] * down[2];
            let mut d2 = [dir[0] - dn * down[0], dir[1] - dn * down[1], dir[2] - dn * down[2]];
            let l = frame::norm3(d2);
            if l > 1e-3 {
                d2 = [d2[0] / l, d2[1] / l, d2[2] / l];
            } else {
                d2 = dir;
            }
            p = [p[0] + d2[0] * ds, p[1] + d2[1] * ds, p[2] + d2[2] * ds];
            if airborne {
                p[1] += vy * dt - 0.5 * 9.81 * dt * dt;
                vy -= 9.81 * dt;
            }
            let oi = OFF2_PATH + (k * PATH_LAT.len() + li) * PER_SAMPLE;
            match sample_at(geo, &m, s.pos[1], p, down, &mut out[oi..oi + PER_SAMPLE]) {
                Some((hp, n)) => {
                    p = hp;
                    down = [-n[0], -n[1], -n[2]];
                    if down[1] > -0.2 {
                        down = [0.0, -1.0, 0.0];
                    }
                    airborne = false;
                    vy = 0.0;
                }
                None => {
                    airborne = true;
                    down = [0.0, -1.0, 0.0];
                }
            }
            dir = d2;
        }
    }

    // ── rays in the car frame
    let mut ri = OFF2_RAYS;
    let origin = [s.pos[0], s.pos[1] + 0.8, s.pos[2]];
    for yaw in RAY_YAWS_DEG {
        for pitch in RAY_PITCH_DEG {
            let (ya, pa) = (yaw.to_radians(), pitch.to_radians());
            let local = [ya.sin() * pa.cos(), pa.sin(), ya.cos() * pa.cos()];
            let dir = frame::rotate(&m, local);
            match geo.cast(origin, dir, RAY_MAX_M) {
                Some((d, n, mat, fam, special)) => {
                    out[ri] = d / RAY_MAX_M;
                    out[ri + 1] = 1.0;
                    out[ri + 2] = -n[1]; // normal · gravity (gravity = −y)
                    one_hot(&mut out[ri + 3..ri + 3 + N_MAT], mat);
                    one_hot(&mut out[ri + 3 + N_MAT..ri + 3 + N_MAT + N_FAM], fam);
                    out[ri + 3 + N_MAT + N_FAM] = if special { 1.0 } else { 0.0 };
                }
                None => {
                    out[ri] = 1.0;
                    one_hot(&mut out[ri + 3 + N_MAT..ri + 3 + N_MAT + N_FAM], 7);
                }
            }
            ri += PER_RAY;
        }
    }
    debug_assert_eq!(ri, OFF2_OBST);

    // ── obstacles: nearest collidable items within range, sorted by distance
    let mut near: Vec<(f32, &Obstacle)> = geo
        .obstacles
        .iter()
        .filter_map(|ob| {
            let d = frame::norm3([ob.centre[0] - s.pos[0], ob.centre[1] - s.pos[1], ob.centre[2] - s.pos[2]]);
            if d <= OBST_RANGE_M { Some((d, ob)) } else { None }
        })
        .collect();
    near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    for (k, (_, ob)) in near.iter().take(N_OBST).enumerate() {
        let oi = OFF2_OBST + k * (4 + N_FAM);
        let rel = frame::to_car(&m, [ob.centre[0] - s.pos[0], ob.centre[1] - s.pos[1], ob.centre[2] - s.pos[2]]);
        out[oi] = rel[0] / 50.0;
        out[oi + 1] = rel[1] / 50.0;
        out[oi + 2] = rel[2] / 50.0;
        out[oi + 3] = ob.radius / 10.0;
        one_hot(&mut out[oi + 4..oi + 4 + N_FAM], ob.fam);
    }

    // ── cells: 5 lateral × 5 ahead × 3 up, rotated to the heading (yaw only)
    let (cx0, cy0, cz0) = geo.cell_of(s.pos);
    let mut ci = OFF2_CELLS;
    for du in -1..=1i32 {
        for da in 0..CELLS.1 {
            for dl in -(CELLS.0 / 2)..=(CELLS.0 / 2) {
                // offset in metres in the heading frame → world cell
                let ox = heading[0] * (da as f32 * 32.0) + lat[0] * (dl as f32 * 32.0);
                let oz = heading[2] * (da as f32 * 32.0) + lat[2] * (dl as f32 * 32.0);
                let wx = s.pos[0] + ox;
                let wz = s.pos[2] + oz;
                let key = ((wx / 32.0).floor() as i32, cy0 + du, (wz / 32.0).floor() as i32);
                let _ = (cx0, cz0);
                let fam = geo.cells.get(&key).cloned().unwrap_or(7);
                one_hot(&mut out[ci..ci + N_FAM], fam);
                ci += N_FAM;
            }
        }
    }
    debug_assert_eq!(ci, OFF2_TARGET);

    // ── target
    let o = OFF2_TARGET;
    let rel_w = [t.centre[0] - s.pos[0], t.centre[1] - s.pos[1], t.centre[2] - s.pos[2]];
    let rel = frame::to_car(&m, rel_w);
    let dist_h = (rel_w[0] * rel_w[0] + rel_w[2] * rel_w[2]).sqrt();
    out[o] = rel[0] / 100.0;
    out[o + 1] = rel[1] / 100.0;
    out[o + 2] = rel[2] / 100.0;
    out[o + 3] = dist_h / 100.0;
    out[o + 4] = (1.0 + dist_h).ln() / 6.0;
    let bh = (rel[0] * rel[0] + rel[2] * rel[2]).sqrt();
    if bh > 1e-3 {
        out[o + 5] = rel[2] / bh;
        out[o + 6] = rel[0] / bh;
    }
    let n = frame::to_car(&m, t.normal);
    out[o + 7] = n[0];
    out[o + 8] = n[1];
    out[o + 9] = n[2];
    out[o + 10] = t.half_width / 10.0;
    out[o + 11] = t.group_size as f32 / 4.0;
    out[o + 12] = h_ticks as f32 / 500.0;
    let k = match t.kind {
        TargetKind::Checkpoint => 0,
        TargetKind::Finish => 1,
        TargetKind::Multilap => 2,
        TargetKind::LocalPoint => 3,
    };
    one_hot(&mut out[o + 13..o + 17], k);
    out[o + 17] = t.collected_share;

    // ── chord probes: path-sample values under the straight chord
    for i in 1..=CHORD_N {
        let tt = i as f32 / (CHORD_N + 1) as f32;
        let p = [s.pos[0] + rel_w[0] * tt, s.pos[1] + rel_w[1] * tt, s.pos[2] + rel_w[2] * tt];
        let oi = OFF2_CHORD + (i - 1) * PER_SAMPLE;
        let _ = sample_at(geo, &m, s.pos[1], p, [0.0, -1.0, 0.0], &mut out[oi..oi + PER_SAMPLE]);
    }
}

pub fn describe2() -> String {
    let mut s = format!("# R input features — FEATURE_VERSION 2, DIM {DIM2} (generated by `tmr features --fv 2`)\n\n");
    s.push_str("Everything in the CAR frame (full attitude for rays and vectors; yaw frame for the path lateral offsets and the cell grid); gravity enters only as normal·gravity and the vertical speed. Geometry = GEOM's `mapgeom::local::LocalScene` (every collision triangle tagged with placement family + gameplay special), coarse classes: family 8 (road, dirt/grass/sand, ice/bump/water-road, platform, gate, wall/structure/special-track, terrain/water, other/none), material 5 (hard road, dirt/sand, grass, ice/snow, water/other).\n\n");
    s.push_str("| block | offset | len | content |\n|---|---|---|---|\n");
    let d = [
        "speed/100, velocity car (3)/100, vy/50, up car (3), angvel flag+3, wheels flag + contact 4 + material class 4×5 + contact normal 4×3 (reserved), gear/rpm/turbo flag+3, active effects flag + 10 one-hot + remaining + strength (reserved), car kind flag + 4 (reserved)",
        "10 samples along the ground-following/ballistic arc for h at current speed × 3 lateral offsets (−8, 0, +8 m); per sample: surface normal car-frame (3), (surface y − car y)/20, below dist/12, above dist/30, hit, material 5, family 8, special",
        "13 yaws (−90..+90) × 5 pitches (−30,−10,0,+15,+40) in the car frame; per ray: dist/120, hit, normal·gravity, material 5, family 8, special",
        "8 nearest collidable ITEM placements within 80 m: rel pos car-frame /50 (3), radius/10, family 8; padded with zeros",
        "5 lateral × 5 ahead × 3 up block cells (32 × 8 × 32 m) rotated to the heading: family 8 one-hot each (from the map's grid blocks)",
        "target centre car-frame /100 (3), horiz dist/100, ln(1+dist)/6, bearing cos/sin, normal car-frame (3), half_width/10, group size/4, h/500, kind one-hot (checkpoint, finish, multilap, local point), gates-collected share",
        "8 chord points (t = k/9): the path-sample values cast straight down",
    ];
    for ((name, lo, hi), desc) in BLOCKS2.iter().zip(d.iter()) {
        s.push_str(&format!("| {name} | {lo} | {} | {desc} |\n", hi - lo));
    }
    s.push_str("\nAblations (`--ablation`): full, no-rays, no-path, no-cells, no-obstacles, no-geometry (state+target), distance-only (target).\n");
    s
}
