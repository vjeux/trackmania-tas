//! R as `tmplan`'s `EdgeEstimator` — the M2 seam. The planner reasons over
//! (gate group, arrival bucket) nodes; R wants a car STATE, so the estimator
//! synthesises one at the `from` node: the group's road-level anchor, a yaw-only
//! attitude along the arrival direction (prev → from; the Start gate's normal at
//! the spawn), speed = the bucket's representative speed, no angular velocity
//! and no wheel data (present flags 0 — exactly how the training rows from
//! `CarState::from_row` look). The horizon is the geometric leg time × 1.3
//! clamped to the horizons seen in training: R has only ever been asked about
//! h ≤ `h_max`, and an extrapolated h is a number nobody measured.

use crate::feat::{Featurizer, TargetSpec};
use crate::features2::TargetKind;
use crate::frame;
use crate::net::Weights;
use tmplan::estimator::{Edge, EdgeEstimator, EdgeKind, Geometric, StateBucket};
use tmplan::surface::{Nodes, SurfaceModel};
use tmreach::tmr::CarState;
use tmroute::gates::{GatesFile, WpKind};

pub struct REstimator<'a> {
    pub w: &'a Weights,
    pub feat: &'a Featurizer<'a>,
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
            car: u8::MAX,
        }
    }

    pub fn target_of(&self, to: usize) -> Option<TargetSpec> {
        let grp = self.nodes.groups[to];
        let (c, n, hw) = self.gates.group_geometry(grp)?;
        let rep = self.gates.group_rep(grp)?;
        Some(TargetSpec { centre: c, normal: n, half_width: hw, group_size: self.gates.gates_of_group(grp).len() as u32, kind: TargetKind::of_wp(rep.kind), collected_share: 0.0 })
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
        let mut x = vec![0f32; self.feat.dim()];
        self.feat.fill(&s, &t, h, &mut x);
        crate::feat::mask_blocks(self.feat.version(), &mut x, &self.keep);
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

// ───────────────────────── the chained (horizon-native) estimator ─────────────────────────
//
// Design decision (coordinator, 2026-09-07 05:40Z): price a leg between two gates by
// CHAINING short steps of the horizon-native model instead of one gate lookup that
// extrapolates a 5–20 s leg from 2–4 s rollouts. Beam over synthetic states: expand each
// state with a fan of LOCAL targets (2 s and 4 s horizons, distances and bearings towards
// the gate), keep the targets the local head says are reachable, dedupe on a cell over
// position × speed × yaw, stop when a target is the gate itself. The gate heads (R's
// `REstimator`) remain the fast ORDER PRIOR; this is the price.

use std::collections::HashMap;

#[derive(Clone, Debug)]
struct ChainState {
    pos: [f32; 3],
    speed: f32,
    dir: [f32; 2],
    ticks: i32,
    logp: f32,
    steps: usize,
}

pub struct Chained<'a> {
    pub local: &'a Weights,
    pub feat: &'a Featurizer<'a>,
    pub gates: &'a GatesFile,
    pub nodes: &'a Nodes,
    pub surf: &'a SurfaceModel,
    pub keep: Vec<&'static str>,
    /// Beam width over states and the step budget (steps × 4 s bounds the leg).
    pub beam: usize,
    pub max_steps: usize,
    /// A step below this P(reach) is not expanded.
    pub p_step_floor: f32,
    /// Penalty in ms per unit of −ln p when ranking states (tmplan::planner::PENALTY_MS).
    pub penalty_ms: f32,
    cache: std::sync::Mutex<HashMap<(u8, usize, usize), Edge>>,
    pub trace: bool,
    /// Fast fan (fewer bearings/fractions) — see FAN_FRAC_FAST.
    pub fast: bool,
    /// Fallback when an edge was not chained (budget exhausted): the gate head, if given.
    pub fallback: Option<REstimator<'a>>,
    pub fallbacks_used: std::sync::atomic::AtomicUsize,
}

/// Fan of local targets per horizon: (h ticks, distance as a fraction of what the car covers at its
/// current speed in h — the endpoint cloud lives there; a 138 m/s car covers 276 m in 2 s and a fixed
/// 120 m fan priced it at half speed).
const FAN_H: &[u16] = &[200, 400];
/// Fan fractions and bearings: the FULL fan (F7 numbers) and the FAST fan (`Chained::fast`, ~5× fewer
/// queries per step) — the coordinator's ≤ 5 min per map budget (Poland, 15 CPs: 1 h 15 min with the full fan).
const FAN_FRAC: &[f32] = &[0.5, 0.75, 1.0, 1.2, 1.4];
const FAN_FRAC_FAST: &[f32] = &[0.6, 0.9, 1.2];
const BEARINGS_FAST: &[f32] = &[-60.0, -30.0, -12.0, 0.0, 12.0, 30.0, 60.0];

/// Metres a full-throttle Stadium car covers in h from speed v under tmplan's two-phase model
/// (20 m/s² below 50 m/s, 5 m/s² above, top 140 m/s — Geometric::leg_time_ms's constants).
pub fn reach_m(v: f32, h: u16) -> f32 {
    let mut v = v.max(0.0);
    let mut s = 0.0f32;
    for _ in 0..h {
        let a = if v < 50.0 { 20.0 } else { 5.0 };
        v = (v + a * 0.01).min(140.0);
        s += v * 0.01;
    }
    s
}
const FAN_MIN_M: f32 = 25.0;
/// Bearings relative to the CAR's heading (the road bends away from the gate direction; on Summer
/// 2026 - 01 leg 1 a gate-relative ±60° fan missed the road and chained 6 short steps west), plus
/// the gate direction itself.
const BEARINGS_DEG: &[f32] = &[-90.0, -60.0, -40.0, -25.0, -12.0, 0.0, 12.0, 25.0, 40.0, 60.0, 90.0];

impl<'a> Chained<'a> {
    pub fn new(local: &'a Weights, feat: &'a Featurizer<'a>, gates: &'a GatesFile, nodes: &'a Nodes, surf: &'a SurfaceModel, keep: Vec<&'static str>) -> Chained<'a> {
        Chained { local, feat, gates, nodes, surf, keep, beam: 24, max_steps: 10, p_step_floor: 0.05, penalty_ms: tmplan::planner::PENALTY_MS, cache: std::sync::Mutex::new(HashMap::new()), trace: false, fast: false, fallback: None, fallbacks_used: std::sync::atomic::AtomicUsize::new(0) }
    }

    fn state_of(&self, s: &ChainState) -> CarState {
        CarState {
            race_ms: 0,
            pos: s.pos,
            vel: [s.dir[0] * s.speed, 0.0, s.dir[1] * s.speed],
            quat: frame::yaw_quat(s.dir),
            ang_vel: [f32::NAN; 3],
            speed: s.speed,
            gear: u8::MAX,
            rpm: f32::NAN,
            wheel_contact: [u8::MAX; 4],
            wheel_material: [u8::MAX; 4],
            wheel_slip: [f32::NAN; 4],
            turbo: f32::NAN,
            cps: 0,
            finished: false,
            car: u8::MAX,
        }
    }

    /// Ground height for a target at (x, z): the highest surface at or below y_ref + 3 within 40 m, else y_ref.
    fn ground_y(&self, x: f32, y_ref: f32, z: f32) -> f32 {
        let col = self.surf.full.column(x, z);
        col.iter().find(|(sy, _)| *sy <= y_ref + 3.0 && *sy >= y_ref - 40.0).map(|(sy, _)| *sy).unwrap_or(y_ref)
    }

    /// The local head on (state → target at h).
    pub fn local_query(&self, s: &ChainState, target: [f32; 3], h: u16) -> crate::net::Estimate {
        let cs = self.state_of(s);
        let dir = frame::unit3([target[0] - s.pos[0], target[1] - s.pos[1], target[2] - s.pos[2]]).unwrap_or([0.0, 0.0, 1.0]);
        let t = TargetSpec { centre: target, normal: dir, half_width: crate::data::R_LOCAL, group_size: 0, kind: TargetKind::LocalPoint, collected_share: 0.0 };
        let mut x = vec![0f32; self.feat.dim()];
        self.feat.fill(&cs, &t, h, &mut x);
        crate::feat::mask_blocks(self.feat.version(), &mut x, &self.keep);
        let d = frame::norm3([target[0] - s.pos[0], target[1] - s.pos[1], target[2] - s.pos[2]]);
        self.local.estimate(&x, d)
    }

    /// Chain from a synthetic state at `from` (speed v_in, heading `dir`) to the gate node `to`.
    /// Returns (p, ticks, arrival speed, steps, path) of the best terminal state, or None.
    pub fn chain(&self, from_pos: [f32; 3], v_in: f32, dir: [f32; 2], to: usize) -> Option<(f32, i32, f32, usize, Vec<[f32; 3]>)> {
        let goal = self.nodes.pos[to];
        let (gc, gn, ghw) = self.gates.group_geometry(self.nodes.groups[to])?;
        let _ = gc;
        let start = ChainState { pos: from_pos, speed: v_in, dir, ticks: 0, logp: 0.0, steps: 0 };
        let mut frontier = vec![start];
        let mut paths: Vec<Vec<[f32; 3]>> = vec![vec![from_pos]];
        let mut best: Option<(f32, ChainState, Vec<[f32; 3]>)> = None;
        // step budget scales with the leg: a 10-step cap cannot cross a long leg when the head keeps the steps
        // short (GEOM's 14 "p = 0 on every tour" maps, 23:33Z). ~1 step per 40 m of chord + 5, between max_steps and 40.
        let chord_m = ((goal[0] - from_pos[0]).powi(2) + (goal[2] - from_pos[2]).powi(2)).sqrt();
        let steps = self.max_steps.max((chord_m / 40.0) as usize + 5).min(40);
        for _step in 0..steps {
            let mut next: Vec<(f32, ChainState, Vec<[f32; 3]>)> = Vec::new();
            for (s, path) in frontier.iter().zip(&paths) {
                let to_goal = [goal[0] - s.pos[0], goal[2] - s.pos[2]];
                let dg = (to_goal[0] * to_goal[0] + to_goal[1] * to_goal[1]).sqrt();
                let gdir = if dg > 1e-3 { [to_goal[0] / dg, to_goal[1] / dg] } else { s.dir };
                // terminal candidate: the gate itself, when within the local range
                if dg <= crate::data::LOCAL_MAX_M {
                    for h in FAN_H {
                        if (*h as f32) * 1.6 + 30.0 < dg {
                            continue; // could not cover it in h at any speed
                        }
                        let e = self.local_query(s, goal, *h);
                        if e.p_reach >= self.p_step_floor {
                            // time to the gate = the head's first-passage time (PASSAGE labels), ≤ h
                            let t = (e.expected_ticks.round() as i32).clamp(10, *h as i32);
                            let cand = ChainState { pos: goal, speed: e.speed_mu.max(0.0), dir: [gn[0], gn[2]], ticks: s.ticks + t, logp: s.logp + e.p_reach.ln(), steps: s.steps + 1 };
                            let score = cand.ticks as f32 * 10.0 - self.penalty_ms * cand.logp;
                            if best.as_ref().map_or(true, |(b, _, _)| score < *b) {
                                let mut p = path.clone();
                                p.push(goal);
                                best = Some((score, cand, p));
                            }
                        }
                    }
                }
                // expansion fan (distances scale with what the car covers in h — reach_m)
                let fracs: &[f32] = if self.fast { FAN_FRAC_FAST } else { FAN_FRAC };
                let bearings: &[f32] = if self.fast { BEARINGS_FAST } else { BEARINGS_DEG };
                for h in FAN_H {
                    let reach = reach_m(s.speed, *h);
                    for f in fracs {
                        let d = &(reach * f).max(FAN_MIN_M).min(crate::data::LOCAL_MAX_M);
                        if *d > dg + ghw + 20.0 {
                            continue; // overshooting the gate
                        }
                        let mut dirs: Vec<[f32; 2]> = bearings
                            .iter()
                            .map(|b| {
                                let a = b.to_radians();
                                let (ca, sa) = (a.cos(), a.sin());
                                [s.dir[0] * ca + s.dir[1] * sa, -s.dir[0] * sa + s.dir[1] * ca]
                            })
                            .collect();
                        dirs.push(gdir);
                        for tdir in dirs {
                            let x = s.pos[0] + tdir[0] * d;
                            let z = s.pos[2] + tdir[1] * d;
                            let y = self.ground_y(x, s.pos[1], z);
                            let target = [x, y, z];
                            let e = self.local_query(s, target, *h);
                            if e.p_reach < self.p_step_floor {
                                continue;
                            }
                            // step time = first-passage time at the target (the head's ticks), not h: a near target is
                            // passed early, not braked for
                            let t = (e.expected_ticks.round() as i32).clamp(10, *h as i32);
                            let cand = ChainState { pos: target, speed: e.speed_mu.clamp(0.0, 150.0), dir: tdir, ticks: s.ticks + t, logp: s.logp + e.p_reach.ln(), steps: s.steps + 1 };
                            let mut p = path.clone();
                            p.push(target);
                            let score = cand.ticks as f32 * 10.0 - self.penalty_ms * cand.logp;
                            next.push((score, cand, p));
                        }
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            // prune: nothing worse than the best terminal already found; cell dedupe; beam
            if let Some((bs, _, _)) = &best {
                next.retain(|(sc, _, _)| sc < bs);
            }
            next.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            let mut seen = std::collections::HashSet::new();
            next.retain(|(_, s, _)| {
                let key = ((s.pos[0] / 8.0).round() as i32, (s.pos[2] / 8.0).round() as i32, (s.pos[1] / 4.0).round() as i32, (s.speed / 10.0).round() as i32, ((s.dir[0].atan2(s.dir[1])).to_degrees() / 30.0).round() as i32);
                seen.insert(key)
            });
            next.truncate(self.beam);
            frontier = next.iter().map(|(_, s, _)| s.clone()).collect();
            paths = next.into_iter().map(|(_, _, p)| p).collect();
        }
        best.map(|(_, s, p)| (s.logp.exp(), s.ticks, s.speed, s.steps, p))
    }

    fn heading_at(&self, prev: Option<usize>, from: usize) -> [f32; 2] {
        // same rule as REstimator: through the gate at a gate node, the Start normal at the spawn
        let r = REstimator { w: self.local, feat: self.feat, gates: self.gates, nodes: self.nodes, surf: self.surf, h_max: 400, h_min: 200, keep: self.keep.clone(), p_floor: 0.0 };
        r.heading_public(prev, from)
    }
}

impl<'a> REstimator<'a> {
    pub fn heading_public(&self, prev: Option<usize>, from: usize) -> [f32; 2] {
        self.heading(prev, from)
    }
}

impl<'a> Chained<'a> {
    /// Chain one edge (no cache).
    fn price(&self, bucket: StateBucket, prev: Option<usize>, from: usize, to: usize) -> Edge {
        let dir = self.heading_at(prev, from);
        let (horiz, dy) = tmplan::estimator::chord(self.nodes.pos[from], self.nodes.pos[to]);
        let length = (horiz * horiz + dy * dy).sqrt();
        match self.chain(self.nodes.pos[from], bucket.speed(), dir, to) {
            Some((p, ticks, v_arr, _steps, _path)) => Edge { p_reach: p, expected_ms: ticks * 10, arrival: StateBucket::of_speed(v_arr), length_m: length, kind: EdgeKind::Surface },
            None => Edge { p_reach: 0.0, expected_ms: -1, arrival: bucket, length_m: length, kind: EdgeKind::None },
        }
    }

    /// Price every (bucket, from, to) edge in parallel within `budget` seconds; the beam then reads the
    /// cache. Returns (edges priced, edges total, seconds). Edges left unpriced fall back to the gate
    /// head in `estimate` (counted in `fallbacks_used`). The cache is filled in a deterministic order
    /// (spawn edges first, then by from/to/bucket) so a budget cut drops the same edges every run.
    pub fn precompute(&self, threads: usize, budget: std::time::Duration) -> (usize, usize, f64) {
        let n = self.nodes.pos.len();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for from in 0..n {
            for to in 1..n {
                if from != to {
                    edges.push((from, to));
                }
            }
        }
        self.precompute_only(threads, budget, &edges)
    }

    /// Price all 5 arrival buckets for just these (from, to) edges — the hybrid's R-priced set (graph-missing,
    /// detour and override-candidate legs, 10–25 % of the edges on a 30-group map; GEOM 23:33Z).
    pub fn precompute_only(&self, threads: usize, budget: std::time::Duration, edges: &[(usize, usize)]) -> (usize, usize, f64) {
        let mut jobs: Vec<(u8, usize, usize)> = Vec::new();
        for &(from, to) in edges {
            if from == to || to == 0 {
                continue;
            }
            for b in 0..5u8 {
                if from == 0 && b != 0 {
                    continue; // the spawn is left from rest only
                }
                jobs.push((b, from, to));
            }
        }
        let total = jobs.len();
        let t0 = std::time::Instant::now();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let results = std::sync::Mutex::new(Vec::<((u8, usize, usize), Edge)>::with_capacity(total));
        std::thread::scope(|sc| {
            for _ in 0..threads.max(1) {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= total || t0.elapsed() > budget {
                        break;
                    }
                    let (b, from, to) = jobs[i];
                    let prev = if from == 0 { None } else { Some(0) };
                    let e = self.price(StateBucket { speed_bin: b, car: 0 }, prev, from, to);
                    results.lock().unwrap().push(((b, from, to), e));
                });
            }
        });
        let results = results.into_inner().unwrap();
        let done = results.len();
        let mut c = self.cache.lock().unwrap();
        for (k, e) in results {
            c.insert(k, e);
        }
        (done, total, t0.elapsed().as_secs_f64())
    }
}

impl<'a> EdgeEstimator for Chained<'a> {
    fn estimate(&self, bucket: StateBucket, prev: Option<usize>, from: usize, to: usize) -> Edge {
        let key = (bucket.speed_bin, from, to);
        if let Some(e) = self.cache.lock().unwrap().get(&key) {
            return *e;
        }
        if let Some(fb) = &self.fallback {
            // budget exhausted for this edge: the gate head prices it (flagged)
            self.fallbacks_used.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let e = fb.estimate(bucket, prev, from, to);
            self.cache.lock().unwrap().insert(key, e);
            return e;
        }
        let dir = self.heading_at(prev, from);
        let (horiz, dy) = tmplan::estimator::chord(self.nodes.pos[from], self.nodes.pos[to]);
        let length = (horiz * horiz + dy * dy).sqrt();
        let e = match self.chain(self.nodes.pos[from], bucket.speed(), dir, to) {
            Some((p, ticks, v_arr, _steps, _path)) => Edge { p_reach: p, expected_ms: ticks * 10, arrival: StateBucket::of_speed(v_arr), length_m: length, kind: EdgeKind::Surface },
            None => Edge { p_reach: 0.0, expected_ms: -1, arrival: bucket, length_m: length, kind: EdgeKind::None },
        };
        self.cache.lock().unwrap().insert(key, e);
        e
    }
    fn name(&self) -> String {
        format!("chained(beam {}, steps ≤ {}, p_step ≥ {})", self.beam, self.max_steps, self.p_step_floor)
    }
}
