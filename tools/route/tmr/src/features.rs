//! R's input features — all LOCAL (car frame) so the model transfers across
//! maps. The layout is fixed by `FEATURE_VERSION`; `FEATURES.md` in the bank is
//! generated from `describe()` so the document and the code cannot drift.
//!
//! Every block has a fixed offset (`OFF_*`) so the ablations can zero a block
//! by name and the planner can fill one without the others.

use crate::frame;
use mapgeom::probe::Index;
use tmreach::tmr::CarState;
use tmroute::gates::GateRec;

pub const FEATURE_VERSION: u32 = 1;

/// Probe grid: metres AHEAD (car +Z) and LATERAL (car +X), car frame, projected
/// onto the horizontal plane through the car (probes are plumb lines).
pub const AHEAD_M: [f32; 7] = [0.0, 5.0, 10.0, 20.0, 40.0, 80.0, 120.0];
pub const LATERAL_M: [f32; 5] = [-30.0, -10.0, 0.0, 10.0, 30.0];
/// Points along the straight chord to the target, at t = k / (CHORD_N + 1).
pub const CHORD_N: usize = 8;
/// A probe looks for the highest surface at or below (sample y + 3) within this reach.
pub const PROBE_REACH_M: f32 = 40.0;

pub const OFF_TARGET: usize = 0; // rel pos (3), horiz dist/100, ln(1+dist)/6, bearing cos, sin = 7
pub const OFF_NORMAL: usize = 7; // gate normal in car frame (3), half_width/10, group size/4 = 5
pub const OFF_MOTION: usize = 12; // speed/100, vel car (3)/100, vy/50 = 5
pub const OFF_ATTITUDE: usize = 17; // map-up in car frame (3) = 3
pub const OFF_ANGVEL: usize = 20; // present flag, ang_vel (3)/5 = 4
pub const OFF_WHEELS: usize = 24; // present flag, contact (4) = 5
pub const OFF_HORIZON: usize = 29; // h/500 = 1
pub const OFF_PROBES: usize = 30; // 35 probes × (rel height/20, none flag, road flag) = 105
pub const OFF_CHORD: usize = 135; // 8 points × (rel height/20, none flag, road flag) = 24
pub const DIM: usize = 159;

/// What the geometry probes read: the full-scene plumb index and the map's
/// road material names (`tmplan::surface::SurfaceModel`). `None` = no
/// geometry available; every probe then reports "no surface" and the flag says
/// so (the planner must never feed a half-built geometry silently).
pub struct Probe<'a> {
    pub idx: Option<&'a Index>,
    pub road: &'a [String],
}

impl<'a> Probe<'a> {
    pub fn none() -> Probe<'static> {
        Probe { idx: None, road: &[] }
    }
    /// (rel height / 20, none flag, road flag) of the highest surface at or
    /// below `y + 3` within reach at (x, z).
    fn sample(&self, x: f32, y: f32, z: f32, out: &mut [f32]) {
        out[0] = 0.0;
        out[1] = 1.0;
        out[2] = 0.0;
        let Some(idx) = self.idx else { return };
        let col = idx.column(x, z);
        if let Some((sy, mat)) = col.iter().find(|(sy, _)| *sy <= y + 3.0 && *sy >= y - PROBE_REACH_M) {
            out[0] = ((sy - y) / 20.0).clamp(-2.0, 2.0);
            out[1] = 0.0;
            out[2] = if self.road.iter().any(|r| r == mat) { 1.0 } else { 0.0 };
        }
    }
}

/// The target as R sees it: one gate (or a group's representative geometry).
#[derive(Clone, Debug)]
pub struct Target {
    pub centre: [f32; 3],
    pub normal: [f32; 3],
    pub half_width: f32,
    pub group_size: u32,
}

impl Target {
    pub fn of_gate(g: &GateRec, group_size: u32) -> Target {
        Target { centre: g.centre, normal: g.normal, half_width: g.half_width, group_size }
    }
}

fn finite(x: f32) -> Option<f32> {
    if x.is_finite() { Some(x) } else { None }
}

/// Fill `out` (len DIM) with the features of (state, target, geometry, h).
pub fn features(s: &CarState, t: &Target, probe: &Probe, h_ticks: u16, out: &mut [f32]) {
    assert_eq!(out.len(), DIM);
    for o in out.iter_mut() {
        *o = 0.0;
    }
    let m = frame::matrix(s.quat);
    // target
    let rel_w = [t.centre[0] - s.pos[0], t.centre[1] - s.pos[1], t.centre[2] - s.pos[2]];
    let rel = frame::to_car(&m, rel_w);
    let dist_h = (rel_w[0] * rel_w[0] + rel_w[2] * rel_w[2]).sqrt();
    out[OFF_TARGET] = rel[0] / 100.0;
    out[OFF_TARGET + 1] = rel[1] / 100.0;
    out[OFF_TARGET + 2] = rel[2] / 100.0;
    out[OFF_TARGET + 3] = dist_h / 100.0;
    out[OFF_TARGET + 4] = (1.0 + dist_h).ln() / 6.0;
    let bh = (rel[0] * rel[0] + rel[2] * rel[2]).sqrt();
    if bh > 1e-3 {
        out[OFF_TARGET + 5] = rel[2] / bh; // cos of the bearing from car +Z
        out[OFF_TARGET + 6] = rel[0] / bh; // sin
    }
    // normal + size
    let n = frame::to_car(&m, t.normal);
    out[OFF_NORMAL] = n[0];
    out[OFF_NORMAL + 1] = n[1];
    out[OFF_NORMAL + 2] = n[2];
    out[OFF_NORMAL + 3] = t.half_width / 10.0;
    out[OFF_NORMAL + 4] = t.group_size as f32 / 4.0;
    // motion
    let v = frame::to_car(&m, s.vel);
    let speed = finite(s.speed).unwrap_or_else(|| frame::norm3(s.vel));
    out[OFF_MOTION] = speed / 100.0;
    out[OFF_MOTION + 1] = v[0] / 100.0;
    out[OFF_MOTION + 2] = v[1] / 100.0;
    out[OFF_MOTION + 3] = v[2] / 100.0;
    out[OFF_MOTION + 4] = s.vel[1] / 50.0;
    // attitude: map-up in the car frame
    let up = frame::to_car(&m, [0.0, 1.0, 0.0]);
    out[OFF_ATTITUDE] = up[0];
    out[OFF_ATTITUDE + 1] = up[1];
    out[OFF_ATTITUDE + 2] = up[2];
    // angular velocity (NaN when the source has none)
    if s.ang_vel.iter().all(|a| a.is_finite()) {
        out[OFF_ANGVEL] = 1.0;
        for k in 0..3 {
            out[OFF_ANGVEL + 1 + k] = (s.ang_vel[k] / 5.0).clamp(-3.0, 3.0);
        }
    }
    // wheel contacts (u8::MAX when the source has none)
    if s.wheel_contact.iter().all(|c| *c != u8::MAX) {
        out[OFF_WHEELS] = 1.0;
        for k in 0..4 {
            out[OFF_WHEELS + 1 + k] = if s.wheel_contact[k] != 0 { 1.0 } else { 0.0 };
        }
    }
    out[OFF_HORIZON] = h_ticks as f32 / 500.0;
    // geometry probes: a 5 × 7 grid in the car's HORIZONTAL frame (yaw only:
    // forward = car +Z projected on XZ, so a rolled car probes the same ground)
    let fwd = frame::rotate(&m, [0.0, 0.0, 1.0]);
    let fh = (fwd[0] * fwd[0] + fwd[2] * fwd[2]).sqrt();
    let (fx, fz) = if fh > 1e-3 { (fwd[0] / fh, fwd[2] / fh) } else { (0.0, 1.0) };
    // lateral = forward turned +90° about Y (car +X when unrolled)
    let (lx, lz) = (fz, -fx);
    let mut k = OFF_PROBES;
    for a in AHEAD_M {
        for l in LATERAL_M {
            let x = s.pos[0] + fx * a + lx * l;
            let z = s.pos[2] + fz * a + lz * l;
            probe.sample(x, s.pos[1], z, &mut out[k..k + 3]);
            k += 3;
        }
    }
    debug_assert_eq!(k, OFF_CHORD);
    // chord profile to the target
    for i in 1..=CHORD_N {
        let t_ = i as f32 / (CHORD_N + 1) as f32;
        let x = s.pos[0] + rel_w[0] * t_;
        let y = s.pos[1] + rel_w[1] * t_;
        let z = s.pos[2] + rel_w[2] * t_;
        probe.sample(x, y, z, &mut out[k..k + 3]);
        k += 3;
    }
    debug_assert_eq!(k, DIM);
}

/// Named blocks, for FEATURES.md and the ablations.
pub const BLOCKS: &[(&str, usize, usize, &str)] = &[
    ("target", OFF_TARGET, OFF_NORMAL, "target centre in the car frame /100 (3), horizontal distance /100, ln(1+dist)/6, bearing cos/sin from car +Z"),
    ("normal", OFF_NORMAL, OFF_MOTION, "target normal (direction of travel) in the car frame (3), half_width /10, group size /4"),
    ("motion", OFF_MOTION, OFF_ATTITUDE, "speed /100, velocity in the car frame /100 (3), vertical speed /50"),
    ("attitude", OFF_ATTITUDE, OFF_ANGVEL, "map-up in the car frame (3)"),
    ("angvel", OFF_ANGVEL, OFF_WHEELS, "present flag, angular velocity /5 (3), 0 with flag 0 when the source has none"),
    ("wheels", OFF_WHEELS, OFF_HORIZON, "present flag, wheel contact (4), 0 with flag 0 when the source has none"),
    ("horizon", OFF_HORIZON, OFF_PROBES, "h ticks /500"),
    ("probes", OFF_PROBES, OFF_CHORD, "7 ahead (0,5,10,20,40,80,120 m) × 5 lateral (−30,−10,0,10,30 m) plumb probes in the car's yaw frame: (surface y − car y)/20 clamped ±2, no-surface flag, road-material flag"),
    ("chord", OFF_CHORD, DIM, "8 points along the straight chord to the target at t=k/9: (surface y − chord y)/20, no-surface flag, road flag — the gaps are what a jump looks like"),
];

/// Ablation: zero every block not in `keep`.
pub fn mask_blocks(x: &mut [f32], keep: &[&str]) {
    for (name, lo, hi, _) in BLOCKS {
        if !keep.contains(name) {
            for v in &mut x[*lo..*hi] {
                *v = 0.0;
            }
        }
    }
}

pub fn ablation_keep(name: &str) -> Option<Vec<&'static str>> {
    let all: Vec<&str> = BLOCKS.iter().map(|b| b.0).collect();
    match name {
        "full" => Some(all),
        "no-probes" => Some(all.into_iter().filter(|b| *b != "probes" && *b != "chord").collect()),
        "no-attitude" => Some(all.into_iter().filter(|b| *b != "attitude" && *b != "angvel" && *b != "wheels").collect()),
        "distance-only" => Some(vec!["target", "horizon"]),
        _ => None,
    }
}

pub fn describe() -> String {
    let mut s = format!("# R input features — FEATURE_VERSION {FEATURE_VERSION}, DIM {DIM} (generated by `tmr features`; do not edit by hand)\n\n");
    s.push_str("All in the CAR frame: quaternion (w, x, y, z) as tmstate stores it, `frame::to_car` = the transpose of its rotation matrix; forward = local +Z (MEASURED, `tmr frame`). The probe grid and the chord use the car's YAW frame only (forward projected on XZ), so a rolled or pitched car probes the same ground.\n\n");
    s.push_str("| block | offset | len | content |\n|---|---|---|---|\n");
    for (name, lo, hi, d) in BLOCKS {
        s.push_str(&format!("| {name} | {lo} | {} | {d} |\n", hi - lo));
    }
    s.push_str("\nNaN never reaches the net: a source without angular velocity / wheel contacts sets the block's present flag to 0 and the values to 0. Probe 'no surface' means no triangle at or below (sample y + 3) within 40 m — the plumb index over the FULL scene (track + decoration), `tmplan::surface::SurfaceModel::full`; 'road' means the material is one of the map's learned road materials (`SurfaceModel::road_materials`).\n");
    s.push_str("\nAblations (`--ablation`): `no-probes` zeroes probes+chord; `no-attitude` zeroes attitude+angvel+wheels; `distance-only` keeps target+horizon.\n");
    s
}
