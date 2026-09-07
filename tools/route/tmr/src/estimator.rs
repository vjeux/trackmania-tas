//! R as `tmplan`'s `EdgeEstimator` — the M2 seam. The planner reasons over
//! (gate group, arrival bucket) nodes; R wants a car STATE, so the estimator
//! synthesises one at the `from` node: the group's road-level anchor, a yaw-only
//! attitude along the arrival direction (prev → from; the Start gate's normal at
//! the spawn), speed = the bucket's representative speed, no angular velocity
//! and no wheel data (present flags 0 — exactly how the training rows from
//! `CarState::from_row` look). The horizon is the geometric leg time × 1.3
//! clamped to the horizons seen in training: R has only ever been asked about
//! h ≤ `h_max`, and an extrapolated h is a number nobody measured.

use crate::features::{self, Probe, Target, DIM};
use crate::frame;
use crate::net::Weights;
use tmplan::estimator::{Edge, EdgeEstimator, EdgeKind, Geometric, StateBucket};
use tmplan::surface::{Nodes, SurfaceModel};
use tmreach::tmr::CarState;
use tmroute::gates::{GatesFile, WpKind};

pub struct REstimator<'a> {
    pub w: &'a Weights,
    pub gates: &'a GatesFile,
    pub nodes: &'a Nodes,
    pub surf: &'a SurfaceModel,
    /// Largest horizon (ticks) in the training data.
    pub h_max: u16,
    pub h_min: u16,
    /// Feature blocks kept (ablation of the loaded model; "full" normally).
    pub keep: Vec<&'static str>,
    /// Below this p_reach the edge is refused (EdgeKind::None) so the beam prunes it.
    pub p_floor: f32,
}

impl<'a> REstimator<'a> {
    /// Direction of travel at node `from`. At a GATE node it is the gate normal: a
    /// training row that starts at a gate crossing has the car heading THROUGH the
    /// gate (that is what a crossing is), never along the chord from the previous
    /// gate — the chord version put every human leg at p_reach 0.000 on both maps.
    /// The chord is the fallback for a gate without an oriented normal.
    fn heading(&self, prev: Option<usize>, from: usize) -> [f32; 2] {
        if from != 0 {
            if let Some(rep) = self.gates.group_rep(self.nodes.groups[from]) {
                let l = (rep.normal[0] * rep.normal[0] + rep.normal[2] * rep.normal[2]).sqrt();
                if l > 1e-3 && rep.normal_source != "unknown" {
                    return [rep.normal[0] / l, rep.normal[2] / l];
                }
            }
        }
        if let Some(p) = prev {
            let a = self.nodes.pos[p];
            let b = self.nodes.pos[from];
            let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
            let l = (dx * dx + dz * dz).sqrt();
            if l > 1e-3 {
                return [dx / l, dz / l];
            }
        }
        if from == 0 {
            if let Some(g) = self.gates.gates.iter().find(|g| g.kind == WpKind::Start) {
                let l = (g.normal[0] * g.normal[0] + g.normal[2] * g.normal[2]).sqrt();
                if l > 1e-3 {
                    return [g.normal[0] / l, g.normal[2] / l];
                }
            }
            let y = self.gates.spawn.yaw;
            return [y.sin(), y.cos()];
        }
        // a gate node without a predecessor: its own normal
        if let Some(rep) = self.gates.group_rep(self.nodes.groups[from]) {
            let l = (rep.normal[0] * rep.normal[0] + rep.normal[2] * rep.normal[2]).sqrt();
            if l > 1e-3 {
                return [rep.normal[0] / l, rep.normal[2] / l];
            }
        }
        [0.0, 1.0]
    }

    /// The synthetic start state at node `from`.
    pub fn state_at(&self, bucket: StateBucket, prev: Option<usize>, from: usize) -> CarState {
        let dir = self.heading(prev, from);
        let v = bucket.speed();
        let vel = [dir[0] * v, 0.0, dir[1] * v];
        CarState {
            race_ms: 0,
            pos: self.nodes.pos[from],
            vel,
            quat: frame::yaw_quat(dir),
            ang_vel: [f32::NAN; 3],
            speed: v,
            gear: u8::MAX,
            rpm: f32::NAN,
            wheel_contact: [u8::MAX; 4],
            wheel_material: [u8::MAX; 4],
            wheel_slip: [f32::NAN; 4],
            turbo: f32::NAN,
            cps: 0,
            finished: false,
        }
    }

    pub fn target_of(&self, to: usize) -> Option<Target> {
        let grp = self.nodes.groups[to];
        let (c, n, hw) = self.gates.group_geometry(grp)?;
        Some(Target { centre: c, normal: n, half_width: hw, group_size: self.gates.gates_of_group(grp).len() as u32 })
    }

    pub fn horizon(&self, v_in: f32, length: f32) -> u16 {
        let (ms, _) = Geometric::leg_time_ms(v_in, length, 0.0);
        ((ms as f32 * 1.3 / 10.0).round() as i64).clamp(self.h_min as i64, self.h_max as i64) as u16
    }

    /// Features + decoded estimate for (bucket, prev, from, to).
    pub fn query(&self, bucket: StateBucket, prev: Option<usize>, from: usize, to: usize) -> Option<(crate::net::Estimate, u16, f32)> {
        let s = self.state_at(bucket, prev, from);
        let t = self.target_of(to)?;
        let (horiz, dy) = tmplan::estimator::chord(s.pos, t.centre);
        let length = (horiz * horiz + dy * dy).sqrt();
        let h = self.horizon(bucket.speed(), length);
        let probe = Probe { idx: Some(&self.surf.full), road: &self.surf.road_materials };
        let mut x = vec![0f32; DIM];
        features::features(&s, &t, &probe, h, &mut x);
        features::mask_blocks(&mut x, &self.keep);
        Some((self.w.estimate(&x, length), h, length))
    }
}

impl<'a> EdgeEstimator for REstimator<'a> {
    fn estimate(&self, bucket: StateBucket, prev: Option<usize>, from: usize, to: usize) -> Edge {
        let Some((e, _h, length)) = self.query(bucket, prev, from, to) else {
            return Edge { p_reach: 0.0, expected_ms: -1, arrival: bucket, length_m: f32::INFINITY, kind: EdgeKind::None };
        };
        if e.p_reach < self.p_floor {
            return Edge { p_reach: e.p_reach, expected_ms: -1, arrival: bucket, length_m: length, kind: EdgeKind::None };
        }
        Edge {
            p_reach: e.p_reach,
            expected_ms: (e.expected_ticks * 10.0).round() as i32,
            arrival: StateBucket::of_speed(e.speed_mu.max(0.0)),
            length_m: length,
            kind: EdgeKind::Surface,
        }
    }
    fn name(&self) -> String {
        format!("R(h {}..{}, p_floor {})", self.h_min, self.h_max, self.p_floor)
    }
}
