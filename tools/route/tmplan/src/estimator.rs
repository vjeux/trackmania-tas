//! The planner's seam: `EdgeEstimator`. The first implementation is GEOMETRIC
//! (surface-graph path length or a flight chord, a speed model from the human
//! corpus); the second will be the MODEL arm's R.

use crate::surface::{Nodes, SurfaceModel};

/// Coarse arrival-state bucket at a gate. The geometric estimator carries only a
/// speed bin; R will carry more.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StateBucket {
    /// 0 slow (<40 m/s), 1 medium (40–80), 2 fast (>80)
    pub speed_bin: u8,
}

impl StateBucket {
    pub fn of_speed(v: f32) -> StateBucket {
        StateBucket { speed_bin: if v < 40.0 { 0 } else if v < 80.0 { 1 } else { 2 } }
    }
    pub fn speed(&self) -> f32 {
        [25.0, 60.0, 100.0][self.speed_bin as usize]
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
    /// Estimate the leg from node `from` (arrived in `bucket`) to node `to`.
    fn estimate(&self, bucket: StateBucket, from: usize, to: usize) -> Edge;
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
}

pub fn chord(a: [f32; 3], b: [f32; 3]) -> (f32, f32) {
    let dx = b[0] - a[0];
    let dz = b[2] - a[2];
    ((dx * dx + dz * dz).sqrt(), b[1] - a[1])
}

impl<'a> Geometric<'a> {
    pub fn leg_time_ms(v_in: f32, length: f32, turn: f32) -> (i32, f32) {
        let v_end = (v_in + 0.06 * length).clamp(25.0, 110.0);
        let mean = ((v_in + v_end) / 2.0) * (1.0 - 0.35 * (turn / std::f32::consts::PI).clamp(0.0, 1.0));
        let ms = (1000.0 * length / mean.max(10.0)).round() as i32;
        (ms, v_end)
    }
}

impl<'a> EdgeEstimator for Geometric<'a> {
    fn estimate(&self, bucket: StateBucket, from: usize, to: usize) -> Edge {
        let v_in = bucket.speed();
        let cost = self.d[from][to];
        let (horiz, dy) = chord(self.nodes.pos[from], self.nodes.pos[to]);
        if cost.is_finite() {
            if self.time_model == TimeModel::Cost {
                return Edge { p_reach: 1.0, expected_ms: cost.round() as i32, arrival: bucket, length_m: self.len[from][to], kind: EdgeKind::Surface };
            }
            let length = self.len[from][to];
            let (ms, v_end) = Self::leg_time_ms(v_in, length, 0.0);
            // off-road share: cost/length runs 1.0 on pure road and up to 20 on grass
            let offroad = ((cost / length.max(1.0)) - 1.0) / 19.0;
            let p = (0.95 - 0.5 * offroad.clamp(0.0, 1.0)).max(0.3);
            return Edge { p_reach: p, expected_ms: ms, arrival: StateBucket::of_speed(v_end), length_m: length, kind: EdgeKind::Surface };
        }
        if let Some(f) = self.flight {
            if horiz <= f.max_horiz && dy <= f.max_rise {
                let len = (horiz * horiz + dy * dy).sqrt() * f.cost_mult;
                if self.time_model == TimeModel::Cost {
                    return Edge { p_reach: f.p_reach, expected_ms: len.round() as i32, arrival: bucket, length_m: len, kind: EdgeKind::Flight };
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
