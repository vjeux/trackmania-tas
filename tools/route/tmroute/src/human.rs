//! Human runs → gate crossings, orders, consensus, and the `router-human` route.
//!
//! Inputs: `.Ghost.Gbx` files (decoded by `gbx::record::decode_ghost`: 50 ms
//! telemetry samples + the result chunk's split times) and the map's
//! `gates.json`. No engine, no oracle: the split times are the ghost's own
//! declaration and the positions are its own telemetry.
//!
//! # Which gate fired a split
//! The car's position at the split instant (linear interpolation between the
//! two straddling samples — the grid is 50 ms, up to ~2.5 m at 50 m/s) is
//! matched to the nearest gate record in XZ whose height is within `MATCH_DY`.
//! `Crossing::dist_xz` is the residual; the consensus prints its distribution
//! so a mismatch shows as a number, not a silent wrong order.
//!
//! Control (brief R3): the number of splits == the header's declared checkpoint
//! count on every finished ghost — a ghost failing it is reported and excluded.
//!
//! # Respawns
//! A position jump > `RESPAWN_JUMP_M` between consecutive samples (60 m in
//! 50 ms is 1 200 m/s; no car does that) counts one respawn. Standard mode
//! keeps the clock running, so the split times still index the same run.

use crate::gates::{GatesFile, WpKind};
use crate::types::*;
use std::path::Path;

pub const MATCH_DY: f32 = 12.0;
pub const RESPAWN_JUMP_M: f32 = 60.0;
/// Resampling step of the consensus corridor.
pub const STEP_M: f32 = 2.0;
pub const HALF_WIDTH_FLOOR: f32 = 4.0;
/// A map "where humans agree": this share of finished runs drive the modal order.
pub const AGREE_SHARE: f64 = 0.8;

#[derive(Clone, Debug)]
pub struct Sample {
    pub t_ms: i32,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub speed: f32,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub path: String,
    pub md5: String,
    pub declared_ms: i32,
    pub splits_ms: Vec<i32>,
    pub samples: Vec<Sample>,
    pub respawns: u32,
}

#[derive(Clone, Debug)]
pub struct Crossing {
    pub ms: i32,
    pub waypoint: u32,
    pub group: u32,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    /// XZ distance from the interpolated position to the matched gate's centre.
    pub dist_xz: f32,
}

pub fn load_run(p: &Path) -> Result<Run, String> {
    let d = gbx::record::decode_ghost(p.to_str().ok_or("path")?).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut samples: Vec<Sample> = d
        .samples
        .iter()
        .map(|s| Sample {
            t_ms: s.time_ms,
            pos: [s.x, s.y, s.z],
            vel: [s.vx, s.vy, s.vz],
            speed: s.speed_ms,
        })
        .collect();
    samples.sort_by_key(|s| s.t_ms);
    let mut respawns = 0;
    for w in samples.windows(2) {
        let d = dist(w[0].pos, w[1].pos);
        if d > RESPAWN_JUMP_M {
            respawns += 1;
        }
    }
    Ok(Run {
        path: p.display().to_string(),
        md5: crate::md5::md5_file_hex(p)?,
        declared_ms: d.race_time_ms.unwrap_or(-1),
        splits_ms: d.checkpoints_ms.clone(),
        samples,
        respawns,
    })
}

pub fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn dist_xz(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

impl Run {
    /// Position and velocity at `t_ms`, interpolated. Clamped at the ends.
    pub fn at(&self, t_ms: i32) -> Option<(([f32; 3], [f32; 3]), usize)> {
        let s = &self.samples;
        if s.is_empty() {
            return None;
        }
        if t_ms <= s[0].t_ms {
            return Some(((s[0].pos, s[0].vel), 0));
        }
        if t_ms >= s[s.len() - 1].t_ms {
            let l = &s[s.len() - 1];
            return Some(((l.pos, l.vel), s.len() - 1));
        }
        let i = s.partition_point(|x| x.t_ms <= t_ms); // first sample after t
        let (a, b) = (&s[i - 1], &s[i]);
        let f = (t_ms - a.t_ms) as f32 / (b.t_ms - a.t_ms).max(1) as f32;
        Some(((lerp3(a.pos, b.pos, f), lerp3(a.vel, b.vel, f)), i - 1))
    }

    /// Samples with `t0 <= t <= t1`, plus interpolated end points.
    pub fn segment(&self, t0: i32, t1: i32) -> Vec<[f32; 3]> {
        let mut out = Vec::new();
        if let Some(((p, _), _)) = self.at(t0) {
            out.push(p);
        }
        for s in &self.samples {
            if s.t_ms > t0 && s.t_ms < t1 {
                out.push(s.pos);
            }
        }
        if let Some(((p, _), _)) = self.at(t1) {
            out.push(p);
        }
        out
    }
}

/// Which gate fired each split. `None` for a split that matches no gate within
/// `max_xz` metres of a gate centre (the caller reports it).
pub fn crossings(run: &Run, gates: &GatesFile, max_xz: f32) -> Vec<Option<Crossing>> {
    run.splits_ms
        .iter()
        .map(|&t| {
            let ((p, v), _) = run.at(t)?;
            let mut best: Option<(f32, &crate::gates::GateRec)> = None;
            for g in &gates.gates {
                if g.kind == WpKind::Start {
                    continue;
                }
                if (p[1] - g.centre[1]).abs() > MATCH_DY {
                    continue;
                }
                // distance to the gate SEGMENT (centre ± half_width along the row), not the centre
                let row = [-g.normal[2], 0.0, g.normal[0]];
                let dx = p[0] - g.centre[0];
                let dz = p[2] - g.centre[2];
                let along = (dx * row[0] + dz * row[2]).clamp(-g.half_width, g.half_width);
                let rx = dx - along * row[0];
                let rz = dz - along * row[2];
                let d = (rx * rx + rz * rz).sqrt();
                if best.map_or(true, |(bd, _)| d < bd) {
                    best = Some((d, g));
                }
            }
            let (d, g) = best?;
            if d > max_xz {
                return None;
            }
            Some(Crossing { ms: t, waypoint: g.waypoint, group: g.group, pos: p, vel: v, dist_xz: dist_xz(p, g.centre) })
        })
        .collect()
}

/// One row of `human-orders.tsv`.
pub struct OrderRow {
    pub md5: String,
    pub rank: u32,
    pub ms: i32,
    pub order_wp: Vec<u32>,
    pub order_group: Vec<u32>,
    pub respawns: u32,
    pub cp_ms: Vec<i32>,
    pub unmatched: usize,
    pub max_dist_xz: f32,
    pub file: String,
}

pub const ORDERS_HEADER: &str = "ghost_md5\trank\tms\torder\trespawns\tcp_ms\tgroups\tunmatched\tmax_dist_xz\tfile";

pub fn order_row(row: &OrderRow) -> String {
    let j = |v: &[u32]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{}",
        row.md5,
        row.rank,
        row.ms,
        j(&row.order_wp),
        row.respawns,
        row.cp_ms.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","),
        j(&row.order_group),
        row.unmatched,
        row.max_dist_xz,
        row.file
    )
}

// ---------------------------------------------------------------------------
// statistics helpers
// ---------------------------------------------------------------------------

pub fn percentile(v: &mut Vec<f32>, p: f32) -> f32 {
    if v.is_empty() {
        return f32::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let r = (p / 100.0) * (v.len() - 1) as f32;
    let i = r.floor() as usize;
    let f = r - i as f32;
    if i + 1 < v.len() {
        v[i] * (1.0 - f) + v[i + 1] * f
    } else {
        v[i]
    }
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n < 1e-6 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / n, v[1] / n, v[2] / n]
    }
}

/// Resample a polyline every `step` metres (3D arc length), keeping the ends.
pub fn resample(poly: &[[f32; 3]], step: f32) -> Vec<[f32; 3]> {
    let mut out = Vec::new();
    if poly.is_empty() {
        return out;
    }
    out.push(poly[0]);
    let mut carry = 0.0f32;
    for w in poly.windows(2) {
        let seg = dist(w[0], w[1]);
        if seg < 1e-6 {
            continue;
        }
        let mut d = step - carry;
        while d <= seg {
            out.push(lerp3(w[0], w[1], d / seg));
            d += step;
        }
        carry = seg - (d - step);
    }
    let last = *poly.last().unwrap();
    if dist(*out.last().unwrap(), last) > step * 0.25 {
        out.push(last);
    }
    out
}

/// Closest point of a polyline to `p` (3D). Returns the point.
pub fn closest_on_polyline(poly: &[[f32; 3]], p: [f32; 3]) -> [f32; 3] {
    let mut best = poly[0];
    let mut bd = f32::INFINITY;
    for w in poly.windows(2) {
        let (a, b) = (w[0], w[1]);
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
        let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
        let t = if l2 < 1e-9 { 0.0 } else { ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / l2).clamp(0.0, 1.0) };
        let q = lerp3(a, b, t);
        let d = dist(q, p);
        if d < bd {
            bd = d;
            best = q;
        }
    }
    if poly.len() == 1 {
        return poly[0];
    }
    best
}

/// Consensus result over one map.
pub struct Consensus {
    pub n_runs: usize,
    pub n_finished_ok: usize,
    pub modal_group_order: Vec<u32>,
    pub n_modal: usize,
    pub share: f64,
    pub agree: bool,
    pub distinct_orders: usize,
    pub route: Option<TrackGeom>,
    pub notes: Vec<String>,
}

/// Build the modal order and the `router-human` route.
/// `runs` are loaded ghosts; `rows` their order rows (same index).
pub fn consensus(gates: &GatesFile, runs: &[Run], rows: &[OrderRow], produced_by: &str) -> Consensus {
    let mut notes = Vec::new();
    let declared = gates.declared_checkpoints;
    // finished + fully matched + right split count
    let ok: Vec<usize> = (0..runs.len())
        .filter(|&i| rows[i].unmatched == 0 && declared > 0 && rows[i].cp_ms.len() as i32 == declared && rows[i].ms > 0)
        .collect();
    let mut counts: std::collections::BTreeMap<Vec<u32>, Vec<usize>> = Default::default();
    for &i in &ok {
        counts.entry(rows[i].order_group.clone()).or_default().push(i);
    }
    let distinct = counts.len();
    let Some((modal, members)) = counts.iter().max_by_key(|(k, v)| (v.len(), std::cmp::Reverse((*k).clone()))) else {
        return Consensus { n_runs: runs.len(), n_finished_ok: 0, modal_group_order: vec![], n_modal: 0, share: 0.0, agree: false, distinct_orders: 0, route: None, notes: vec!["no usable run".into()] };
    };
    let share = members.len() as f64 / ok.len() as f64;
    let mut members = members.clone();
    members.sort_by_key(|&i| rows[i].ms);
    let best = members[0];

    // ---- corridor: per leg, the median line over the agreeing runs
    let mut pts: Vec<[f32; 3]> = Vec::new();
    let mut hws: Vec<f32> = Vec::new();
    let mut legs: Vec<Leg> = Vec::new();
    let mut tg_gates: Vec<Gate> = Vec::new();
    let mut gate_order = Vec::new();
    let mut s_acc = 0.0f32;
    let n_legs = modal.len();
    let mut lat_p90_all: Vec<f32> = Vec::new();
    for li in 0..n_legs {
        // time window per run
        let win = |i: usize| -> (i32, i32) {
            let t1 = rows[i].cp_ms[li];
            let t0 = if li == 0 { 0 } else { rows[i].cp_ms[li - 1] };
            (t0, t1)
        };
        let (b0, b1) = win(best);
        let ref_poly = resample(&runs[best].segment(b0, b1), STEP_M);
        // tangents of the reference
        let n_ref = ref_poly.len();
        let mut leg_pts = Vec::with_capacity(n_ref);
        let mut leg_hw = Vec::with_capacity(n_ref);
        let others: Vec<Vec<[f32; 3]>> = members.iter().map(|&i| { let (t0, t1) = win(i); runs[i].segment(t0, t1) }).collect();
        for k in 0..n_ref {
            let p = ref_poly[k];
            let a = if k + 1 < n_ref { ref_poly[k + 1] } else { p };
            let b = if k > 0 { ref_poly[k - 1] } else { p };
            let tan = norm([a[0] - b[0], 0.0, a[2] - b[2]]);
            let left = [-tan[2], 0.0, tan[0]];
            let mut lats = Vec::with_capacity(others.len());
            let mut ups = Vec::with_capacity(others.len());
            for o in &others {
                if o.len() < 2 {
                    continue;
                }
                let q = closest_on_polyline(o, p);
                let d = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
                lats.push(d[0] * left[0] + d[2] * left[2]);
                ups.push(d[1]);
            }
            let med_l = percentile(&mut lats.clone(), 50.0);
            let med_u = percentile(&mut ups.clone(), 50.0);
            let mut abs_l: Vec<f32> = lats.iter().map(|x| (x - med_l).abs()).collect();
            let p90 = percentile(&mut abs_l, 90.0);
            lat_p90_all.push(p90);
            let hw = if p90.is_nan() { HALF_WIDTH_FLOOR } else { p90.ceil().max(HALF_WIDTH_FLOOR) };
            leg_pts.push([p[0] + med_l * left[0], p[1] + med_u, p[2] + med_l * left[2]]);
            leg_hw.push(hw);
        }
        // append (skip the duplicate joint point)
        let start_idx = if pts.is_empty() { 0 } else { 1 };
        let s_start = s_acc;
        for k in start_idx..leg_pts.len() {
            if let Some(prev) = pts.last() {
                s_acc += dist(*prev, leg_pts[k]);
            }
            pts.push(leg_pts[k]);
            hws.push(leg_hw[k]);
        }
        // arrival bands at the crossing
        let mut speeds = Vec::new();
        let mut heights = Vec::new();
        let mut heads: Vec<[f32; 3]> = Vec::new();
        let mut leg_ms: Vec<i32> = Vec::new();
        for &i in &members {
            let r = &runs[i];
            let t = rows[i].cp_ms[li];
            if let Some(((p, v), _)) = r.at(t) {
                speeds.push((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt());
                heights.push(p[1]);
                heads.push(norm(v));
            }
            let (t0, t1) = win(i);
            leg_ms.push(t1 - t0);
        }
        let mut mean = [0.0f32; 3];
        for h in &heads {
            for a in 0..3 {
                mean[a] += h[a];
            }
        }
        let mean = norm(mean);
        let mut angs: Vec<f32> = heads.iter().map(|h| (h[0] * mean[0] + h[1] * mean[1] + h[2] * mean[2]).clamp(-1.0, 1.0).acos()).collect();
        let tol = percentile(&mut angs, 90.0);
        let group = modal[li];
        let (centre, _axis, half) = gates.group_geometry(group).unwrap();
        let rep = gates.group_rep(group).unwrap();
        let kind = if li + 1 == n_legs { GateKind::Finish } else if rep.kind == WpKind::Multilap { GateKind::Multilap } else { GateKind::Checkpoint };
        tg_gates.push(Gate { kind, centre, normal: [mean[0], 0.0, mean[2]], half_width: half, s: s_acc, map_waypoint: rep.waypoint });
        legs.push(Leg {
            gate_idx: li as u32,
            map_waypoint: rep.waypoint,
            s_start,
            s_end: s_acc,
            connection: ConnectionClass::Unknown,
            arrival_speed: [percentile(&mut speeds.clone(), 10.0), percentile(&mut speeds.clone(), 90.0)],
            arrival_heading: mean,
            arrival_heading_tol: tol,
            arrival_height: [percentile(&mut heights.clone(), 10.0), percentile(&mut heights.clone(), 90.0)],
            p_reach: f32::NAN,
            expected_ms: -1,
            evidence: LegEvidence::Human { runs: members.len() as u32, best_ms: *leg_ms.iter().min().unwrap_or(&-1) },
        });
        gate_order.push(rep.waypoint);
    }
    let mut s = Vec::with_capacity(pts.len());
    let mut acc = 0.0f32;
    for k in 0..pts.len() {
        if k > 0 {
            acc += dist(pts[k - 1], pts[k]);
        }
        s.push(acc);
    }
    // gates' s must equal the leg joints exactly
    for (li, l) in legs.iter().enumerate() {
        tg_gates[li].s = l.s_end;
    }
    let mut p90s = lat_p90_all.clone();
    notes.push(format!("lateral spread P90 over stations: median {:.1} m, P90 {:.1} m", percentile(&mut p90s.clone(), 50.0), percentile(&mut p90s, 90.0)));

    let spawn_dir = norm(runs[best].samples.get(20).map(|s| [s.pos[0] - runs[best].samples[0].pos[0], 0.0, s.pos[2] - runs[best].samples[0].pos[2]]).unwrap_or([0.0, 0.0, 1.0]));
    let route = TrackGeom {
        geom_version: GEOM_VERSION,
        map_uid: gates.map_uid.clone(),
        pts,
        half_width: hws,
        s,
        gates: tg_gates,
        spawn: gates.spawn.pos,
        spawn_yaw: spawn_dir[0].atan2(spawn_dir[2]),
        source: "router-human".into(),
        legs: Some(legs),
        route: Some(RouteMeta {
            route_version: ROUTE_VERSION,
            source: "router-human".into(),
            rank: 0,
            predicted_ms: rows[best].ms,
            status: RouteStatus::Hypothesis,
            gate_order,
            produced_by: format!("{produced_by}; consensus of {} runs ({} agreeing, share {:.2}), corridor = median line of the agreeing runs, reference {}", ok.len(), members.len(), share, rows[best].md5),
        }),
    };
    Consensus {
        n_runs: runs.len(),
        n_finished_ok: ok.len(),
        modal_group_order: modal.clone(),
        n_modal: members.len(),
        share,
        agree: share >= AGREE_SHARE,
        distinct_orders: distinct,
        route: Some(route),
        notes,
    }
}
