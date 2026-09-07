//! The planner's seam: `EdgeEstimator`. The first implementation is GEOMETRIC
//! (surface-graph path length or a flight chord, a speed model from the human
//! corpus); the second will be the MODEL arm's R.

use crate::surface::{Nodes, SurfaceModel};

/// Coarse arrival-state bucket at a gate. The geometric estimator carries only a
/// speed bin; R will carry more.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StateBucket {
    /// 0 rest (<15 m/s), 1 slow (15–50), 2 medium (50–90), 3 fast (90–120), 4 very fast (>120)
    pub speed_bin: u8,
}

impl StateBucket {
    pub fn of_speed(v: f32) -> StateBucket {
        StateBucket { speed_bin: if v < 15.0 { 0 } else if v < 50.0 { 1 } else if v < 90.0 { 2 } else if v < 120.0 { 3 } else { 4 } }
    }
    pub fn speed(&self) -> f32 {
        [0.0, 35.0, 70.0, 105.0, 135.0][self.speed_bin as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EdgeKind {
    /// Along the surface graph.
    Surface,
    /// No surface path; a ballistic/drag-limited flight chord was allowed.
    Flight,
    /// Nothing: the estimator refuses this leg.
    None,
}

#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub p_reach: f32,
    pub expected_ms: i32,
    pub arrival: StateBucket,
    /// Path length the time was computed over (surface path or chord).
    pub length_m: f32,
    pub kind: EdgeKind,
}

pub trait EdgeEstimator {
    /// Estimate the leg from node `from` (arrived in `bucket`, having come from
    /// `prev` — None at the spawn) to node `to`.
    fn estimate(&self, bucket: StateBucket, prev: Option<usize>, from: usize, to: usize) -> Edge;
    fn name(&self) -> String;
}

/// The geometric estimator.
///
/// Time = length / v(bucket, leg): the campaign's own implied speeds (route
/// length / author time over the 19 routed maps, CAMPAIGN-GEOMETRY.md) run
/// 43–90 m/s, median ~70 m/s; a Stadium car does not AVERAGE above ~95 m/s.
/// The arrival speed grows with leg length (a long straight leg ends fast, a
/// short twisty one slow): v_end = clamp(v_in + 0.06 · L, 25, 110) m/s and the
/// leg's mean is (v_in + v_end) / 2 · (1 − 0.35 · turn/π), turn = the heading
/// change from the previous chord to this one. Every constant is documented
/// here and printed by `tmplan --show-model`; it is a HYPOTHESIS generator,
/// not a clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimeModel {
    /// length / v(bucket, leg) — the speed model below.
    Speed,
    /// expected_ms := surface-graph COST in metres, p_reach := 1. Reproduces the
    /// cartographer's Held–Karp objective exactly; the CONTROL of the search.
    Cost,
}

pub struct Geometric<'a> {
    pub time_model: TimeModel,
    /// Surface-graph cost (ordering proxy; INFINITY = no path).
    pub d: &'a [Vec<f32>],
    /// True path length in metres (the time model divides by this).
    pub len: &'a [Vec<f32>],
    pub nodes: &'a Nodes,
    /// Allow a flight chord where the surface graph has no path.
    pub flight: Option<FlightModel>,
    pub surface: Option<&'a SurfaceModel>,
    /// Path end directions (`SurfaceModel::directions`) for the turn term; None = no turn term.
    pub dirs: Option<&'a (Vec<Vec<[f32; 2]>>, Vec<Vec<[f32; 2]>>)>,
    /// Metres of step-down > 1.6 m along each path (`distance_matrix_full`), and the cost added per metre of
    /// it (cost model) — a car that falls off a ledge loses its speed; the graph's drop edges are free.
    pub drop: Option<&'a [Vec<f32>]>,
    pub drop_penalty: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct FlightModel {
    /// Max horizontal chord a flight edge may span (m).
    pub max_horiz: f32,
    /// Max rise (landing above take-off) a flight edge may make (m).
    pub max_rise: f32,
    /// Cost multiplier on the chord (the cartographer's leap edges use 4×).
    pub cost_mult: f32,
    /// P(reach) assigned to a flight edge.
    pub p_reach: f32,
}

impl FlightModel {
    /// Ballistic regime: a launch at v m/s over a flat gap covers v²·sin(2θ)/g;
    /// at 90 m/s and 20° that is ~530 m — the map scale. The drag-limited
    /// regime the memory documents caps the effective range; we probe both by
    /// running the planner twice (`--flight ballistic` / `--flight drag`).
    pub fn ballistic() -> FlightModel {
        FlightModel { max_horiz: 400.0, max_rise: 12.0, cost_mult: 2.0, p_reach: 0.5 }
    }
    pub fn drag() -> FlightModel {
        FlightModel { max_horiz: 160.0, max_rise: 4.0, cost_mult: 3.0, p_reach: 0.35 }
    }
    /// Any chord: for CHARACTERISING a map that has no tour under the physical
    /// models — never for a route hypothesis.
    pub fn any() -> FlightModel {
        FlightModel { max_horiz: 1.0e9, max_rise: 1.0e9, cost_mult: 4.0, p_reach: 0.1 }
    }
}

pub fn chord(a: [f32; 3], b: [f32; 3]) -> (f32, f32) {
    let dx = b[0] - a[0];
    let dz = b[2] - a[2];
    ((dx * dx + dz * dz).sqrt(), b[1] - a[1])
}

impl<'a> Geometric<'a> {
    /// Mean leg speed, FITTED 2026-09-07 to the human corpus (Summer 2026 - 01/02/08,
    /// 15 legs, `tmplan plan --human-orders`): from rest a leg of L metres averages
    /// ≈ 0.11·L + 10 m/s (344 m → 48, 378 m → 56, 512 m → 67 measured); a leg entered
    /// at v_in adds ≈ 0.5·v_in (later legs measured 86–143 m/s). Arrival speed
    /// ≈ 1.3 × the mean (Summer 2026 - 01 leg 0: mean 67 → arrives 88). Caps 145/150.
    /// No turn term yet: the estimator sees (from, to) only.
    pub fn leg_time_ms(v_in: f32, length: f32, turn: f32) -> (i32, f32) {
        // Two-phase acceleration, FITTED to Summer 2026 - 01 leg 0 (512 m from rest in
        // 7.607 s arriving at 88 m/s): a = 20 m/s² below 50 m/s, 5 m/s² above, top
        // speed 140 m/s. A turn at the entry gate costs speed: v_in × (1 − 0.6·turn/π).
        const A_LOW: f32 = 20.0;
        const V_SPLIT: f32 = 50.0;
        const A_HIGH: f32 = 5.0;
        const V_MAX: f32 = 140.0;
        let mut v = (v_in * (1.0 - 0.6 * (turn / std::f32::consts::PI).clamp(0.0, 1.0))).max(0.0);
        let mut s = length.max(0.0);
        let mut t = 0.0f32;
        // phase 1: accelerate at A_LOW up to V_SPLIT
        if v < V_SPLIT {
            let d = (V_SPLIT * V_SPLIT - v * v) / (2.0 * A_LOW);
            if d >= s {
                let v2 = (v * v + 2.0 * A_LOW * s).sqrt();
                t += (v2 - v) / A_LOW;
                return ((1000.0 * t).round() as i32, v2);
            }
            t += (V_SPLIT - v) / A_LOW;
            s -= d;
            v = V_SPLIT;
        }
        // phase 2: accelerate at A_HIGH up to V_MAX
        if v < V_MAX {
            let d = (V_MAX * V_MAX - v * v) / (2.0 * A_HIGH);
            if d >= s {
                let v2 = (v * v + 2.0 * A_HIGH * s).sqrt();
                t += (v2 - v) / A_HIGH;
                return ((1000.0 * t).round() as i32, v2);
            }
            t += (V_MAX - v) / A_HIGH;
            s -= d;
            v = V_MAX;
        }
        t += s / v;
        ((1000.0 * t).round() as i32, v)
    }
}

impl<'a> EdgeEstimator for Geometric<'a> {
    fn estimate(&self, bucket: StateBucket, prev: Option<usize>, from: usize, to: usize) -> Edge {
        let v_in = bucket.speed();
        let cost = self.d[from][to];
        let (horiz, dy) = chord(self.nodes.pos[from], self.nodes.pos[to]);
        // the turn at `from`: arrival direction (prev→from) vs departure (from→to)
        let turn = match (prev, self.dirs) {
            (Some(p), Some((out, inn))) => {
                let a = inn[p][from];
                let b = out[from][to];
                if a[0].is_nan() || b[0].is_nan() { 0.0 } else { (a[0] * b[0] + a[1] * b[1]).clamp(-1.0, 1.0).acos() }
            }
            _ => 0.0,
        };
        if cost.is_finite() {
            if self.time_model == TimeModel::Cost {
                let dr = self.drop.map_or(0.0, |m| m[from][to]);
                let dr = if dr.is_finite() { dr } else { 0.0 };
                let cost = cost + self.drop_penalty * dr;
                return Edge { p_reach: 1.0, expected_ms: (cost * 10.0).round() as i32, arrival: bucket, length_m: self.len[from][to], kind: EdgeKind::Surface }; // decimetres: keeps near-ties (Summer 2026 - 13: 1614.6 vs 1615.2) honest
            }
            let length = self.len[from][to];
            let (ms, v_end) = Self::leg_time_ms(v_in, length, turn);
            // off-road share: cost/length runs 1.0 on pure road and up to 20 on grass
            let offroad = ((cost / length.max(1.0)) - 1.0) / 19.0;
            let p = (0.95 - 0.5 * offroad.clamp(0.0, 1.0)).max(0.3);
            return Edge { p_reach: p, expected_ms: ms, arrival: StateBucket::of_speed(v_end), length_m: length, kind: EdgeKind::Surface };
        }
        if let Some(f) = self.flight {
            if horiz <= f.max_horiz && dy <= f.max_rise {
                let len = (horiz * horiz + dy * dy).sqrt() * f.cost_mult;
                if self.time_model == TimeModel::Cost {
                    return Edge { p_reach: f.p_reach, expected_ms: (len * 10.0).round() as i32, arrival: bucket, length_m: len, kind: EdgeKind::Flight };
                }
                let (ms, v_end) = Self::leg_time_ms(v_in, len, 0.0);
                return Edge { p_reach: f.p_reach, expected_ms: ms, arrival: StateBucket::of_speed(v_end), length_m: len, kind: EdgeKind::Flight };
            }
        }
        Edge { p_reach: 0.0, expected_ms: -1, arrival: bucket, length_m: f32::INFINITY, kind: EdgeKind::None }
    }
    fn name(&self) -> String {
        let base = match self.time_model { TimeModel::Speed => "geometric", TimeModel::Cost => "geometric-cost" };
        match self.flight {
            None => base.into(),
            Some(f) => format!("{base}+flight(h<={:.0},rise<={:.0},x{:.1})", f.max_horiz, f.max_rise, f.cost_mult),
        }
    }
}
