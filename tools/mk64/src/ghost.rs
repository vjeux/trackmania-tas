//! `mk64 ghost COURSE --out FILE --donor GHOST [--laps N] [--vmax KMH] [--uid U]`
//! — a NAIVE ghost that drives the course's centre path: the kart follows
//! the MK64 centreline (the same points the waypoints sit on), on the road
//! surface, at the speed a kart could hold through each bend, and the file is
//! a ghost the client loads (a donor container laid out on a 50 ms grid,
//! every sample's transform overwritten — the `ghost static` recipe, moving).
//!
//! What "naive" means here: no racing line (the centreline), no drift, no
//! banking; a speed profile limited by lateral grip in the bends and by
//! acceleration/braking between them, from rest at the start.
//!
//! The car's frame in a ghost sample (measured on a real ghost by
//! `quat_convention`, below): the quaternion (x, y, z, w) rotates the car's
//! LOCAL axes into the world, local +z = forward, +y = up, +x = left; the
//! sample position is the car's origin, `CAR_ORIGIN_Y` above the road.

use std::path::Path;

use gbx::recwrite::{mat_to_quat_pub, rewrite_ghost, write_transform, Xform};

use crate::course::Course;
use crate::mesh::{self, CollTri, Frame};

/// The car origin's height over the road it stands on: the play-mode car at
/// Luigi Raceway's start rested at y = 33.0918 with the road at 33.0778 under
/// it (2026-09-24, `/car` vs the collision mesh) — the CarSport mesh has its
/// origin at ground level (its wheel joints sit one wheel radius, 0.352 m, up).
pub const CAR_ORIGIN_Y: f32 = 0.015;

/// Speed model.
#[derive(Clone, Copy, Debug)]
pub struct Drive {
    /// top speed, m/s
    pub vmax: f32,
    /// lateral acceleration the kart holds in a bend, m/s²
    pub a_lat: f32,
    /// forward acceleration, m/s²
    pub a_acc: f32,
    /// braking, m/s²
    pub a_brk: f32,
}

impl Default for Drive {
    fn default() -> Self {
        // 160 km/h top speed, firm cornering: the ghost lands around the gold
        // time of the medal estimate (a lap at 150 km/h average)
        Drive { vmax: 160.0 / 3.6, a_lat: 14.0, a_acc: 9.0, a_brk: 18.0 }
    }
}

/// One 50 ms sample of the drive.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub pos: [f32; 3],
    pub fwd: [f32; 3],
    pub up: [f32; 3],
    pub speed: f32,
}

pub struct Trajectory {
    pub poses: Vec<Pose>,
    /// lap completion times, ms
    pub lap_ms: Vec<i64>,
    /// the closed centre path's length, m
    pub lap_len: f32,
    /// every waypoint crossing in driving order (the map's checkpoint
    /// fractions, then the lap line, per lap), ms — the ghost's split list
    pub wp_ms: Vec<i64>,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn len(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

/// The road under (x, z): the height of the upward-facing collision triangle
/// containing the point whose height is nearest `near_y` (bridges and tunnels
/// stack surfaces; the centre path's own height picks the deck it means).
pub fn road_y(soup: &[CollTri], x: f32, z: f32, near_y: f32) -> Option<f32> {
    let mut best: Option<f32> = None;
    for t in soup {
        if mesh::face_normal(&t.p)[1] < 0.3 {
            continue;
        }
        if let Some(h) = crate::tm::height_under(t, x, z) {
            if best.map_or(true, |b| (h - near_y).abs() < (b - near_y).abs()) {
                best = Some(h);
            }
        }
    }
    best
}

/// The centre path as a closed polyline in TM space, resampled every `step`
/// metres (arc length in the ground plane), each point on the road surface.
fn resample(path: &[[f32; 3]], soup: &[CollTri], step: f32) -> (Vec<[f32; 3]>, f32) {
    let n = path.len();
    // cumulative ground-plane arc length, closing the loop
    let mut cum = vec![0.0f32; n + 1];
    for i in 0..n {
        let a = path[i];
        let b = path[(i + 1) % n];
        cum[i + 1] = cum[i] + ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
    }
    let total = cum[n];
    let count = (total / step).ceil().max(1.0) as usize;
    let mut out = Vec::with_capacity(count);
    let mut seg = 0usize;
    for k in 0..count {
        let s = k as f32 * total / count as f32;
        while seg + 1 < n + 1 && cum[seg + 1] <= s {
            seg += 1;
        }
        let a = path[seg % n];
        let b = path[(seg + 1) % n];
        let l = (cum[seg + 1] - cum[seg]).max(1e-6);
        let t = ((s - cum[seg]) / l).clamp(0.0, 1.0);
        let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
        let y = road_y(soup, p[0], p[2], p[1]).unwrap_or(p[1]);
        out.push([p[0], y, p[2]]);
    }
    (out, total)
}

/// The closed polyline smoothed with a box filter of ±`w` points (the MK64
/// centre path is hand-placed every ~3 m and zigzags a little; taken raw, the
/// zigzag reads as curvature and slows the ghost to 80 km/h on straights).
fn smooth(pts: &[[f32; 3]], w: usize) -> Vec<[f32; 3]> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let mut acc = [0.0f32; 3];
            for k in 0..=2 * w {
                let p = pts[(i + n + k - w) % n];
                for c in 0..3 {
                    acc[c] += p[c];
                }
            }
            let m = (2 * w + 1) as f32;
            [acc[0] / m, acc[1] / m, acc[2] / m]
        })
        .collect()
}

/// Curvature (1/m) of the closed polyline at each point, from the turning
/// angle over a window of `w` points each side.
fn curvature(pts: &[[f32; 3]], step: f32, w: usize) -> Vec<f32> {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let a = pts[(i + n - w) % n];
            let b = pts[i];
            let c = pts[(i + w) % n];
            let d1 = [b[0] - a[0], b[2] - a[2]];
            let d2 = [c[0] - b[0], c[2] - b[2]];
            let ang = (d1[0] * d2[1] - d1[1] * d2[0]).atan2(d1[0] * d2[0] + d1[1] * d2[1]);
            (ang.abs() / (w as f32 * step)).max(0.0)
        })
        .collect()
}

/// The feasible speed at every resampled point: the bend limit, then a forward
/// pass (acceleration) and a backward pass (braking) around the loop, twice,
/// so the closing point is consistent.
fn speed_profile(pts: &[[f32; 3]], step: f32, d: &Drive, from_rest: bool) -> Vec<f32> {
    let n = pts.len();
    // curvature on the path smoothed over ±6 m, measured over ±4 m
    let kappa = curvature(&smooth(pts, (6.0 / step) as usize), step, (4.0 / step) as usize);
    let mut v: Vec<f32> = kappa.iter().map(|k| if *k > 1e-6 { (d.a_lat / k).sqrt().min(d.vmax) } else { d.vmax }).collect();
    for _pass in 0..2 {
        // forward: v_{i+1} <= sqrt(v_i² + 2 a s)
        for i in 0..n {
            let j = (i + 1) % n;
            let lim = (v[i] * v[i] + 2.0 * d.a_acc * step).sqrt();
            if v[j] > lim {
                v[j] = lim;
            }
        }
        // backward: v_i <= sqrt(v_{i+1}² + 2 b s)
        for i in (0..n).rev() {
            let j = (i + 1) % n;
            let lim = (v[j] * v[j] + 2.0 * d.a_brk * step).sqrt();
            if v[i] > lim {
                v[i] = lim;
            }
        }
    }
    if from_rest {
        v[0] = 0.0;
    }
    v
}

/// Drive `laps` laps of the centre path from rest at path[0], sampled every
/// `dt_ms`.
pub fn centreline(c: &Course, frame: &Frame, soup: &[CollTri], laps: u32, d: &Drive, dt_ms: i64) -> Trajectory {
    let path: Vec<[f32; 3]> = c.path.iter().map(|p| frame.to_tm(p.pos)).collect();
    drive_route(c, frame, soup, laps, d, dt_ms, &path, &Line::default())
}

/// How a driver deviates from the route: a lateral offset (m, + = right of
/// travel) as a function of the lap fraction, a start offset along the route
/// (m ahead of the line — a grid slot), and DRIFT zones (lap-fraction ranges
/// where the kart slides through the bend, yawed toward its inside).
#[derive(Default, Clone)]
pub struct Line {
    pub lateral: Vec<(f32, f32, f32)>, // (from fraction, to fraction, offset m)
    pub base_lateral: f32,
    pub start_ahead_m: f32,
    pub drift: Vec<(f32, f32)>,
    pub drift_deg: f32,
}

impl Line {
    fn lateral_at(&self, f: f32) -> f32 {
        for (a, b, off) in &self.lateral {
            if f >= *a && f < *b {
                return self.base_lateral + off;
            }
        }
        self.base_lateral
    }
    fn drifting(&self, f: f32) -> bool {
        self.drift.iter().any(|(a, b)| f >= *a && f < *b)
    }
}

/// A trajectory along `route` (TM space, closed) with the driver's `line`.
pub fn drive_route(c: &Course, frame: &Frame, soup: &[CollTri], laps: u32, d: &Drive, dt_ms: i64, route: &[[f32; 3]], line: &Line) -> Trajectory {
    // the checkpoints' lap fractions (the same plan the map's gates follow)
    let mut cp_fr = crate::tm::checkpoint_fractions(c, frame, 4);
    cp_fr.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let step = 0.5f32;
    let (pts, lap_len) = resample(route, soup, step);
    let n = pts.len();
    {
        let kappa = curvature(&smooth(&pts, (6.0 / step) as usize), step, (4.0 / step) as usize);
        let mut vc: Vec<f32> = kappa.iter().map(|k| if *k > 1e-6 { (d.a_lat / k).sqrt().min(d.vmax) } else { d.vmax }).collect();
        vc.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q = |f: f32| vc[((vc.len() - 1) as f32 * f) as usize] * 3.6;
        println!("  bend-limited speed along the lap (km/h): min {:.0}  p10 {:.0}  p50 {:.0}  p90 {:.0}  max {:.0}", q(0.0), q(0.1), q(0.5), q(0.9), q(1.0));
    }
    // the standing-start profile for lap 1, the flying one after
    let v_start = speed_profile(&pts, step, d, true);
    let v_fly = speed_profile(&pts, step, d, false);
    let dt = dt_ms as f32 / 1000.0;
    let smoothed = smooth(&pts, (2.0 / step) as usize);
    let mut poses = Vec::new();
    let mut lap_ms = Vec::new();
    let mut wp_ms = Vec::new();
    let mut next_cp = 0usize;
    let mut s = line.start_ahead_m.max(0.0); // distance along the current lap (a grid slot starts ahead)
    while next_cp < cp_fr.len() && s >= cp_fr[next_cp] * lap_len {
        next_cp += 1;
    }
    let mut lap = 0u32;
    let mut t_ms: i64 = 0;
    let mut v = 0.0f32;
    let at = |s: f32, _prof: &[f32]| -> (usize, f32) {
        let f = (s / step).rem_euclid(n as f32);
        let i = (f.floor() as usize) % n;
        (i, f - f.floor())
    };
    let interp = |i: usize, t: f32, arr: &[[f32; 3]]| -> [f32; 3] {
        let a = arr[i];
        let b = arr[(i + 1) % n];
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
    };
    while lap < laps {
        let prof = if lap == 0 { &v_start } else { &v_fly };
        let (i, _t) = at(s, prof);
        // the target is the profile a little AHEAD of the car (the standing
        // start pins prof[0] to zero; a car aiming at where it is never moves —
        // the first version looped forever here)
        let target = prof[i].max(prof[(i + 1) % n]).max(prof[(i + 2) % n]);
        // the speed follows the profile within the acceleration limits
        let dv = (target - v).clamp(-d.a_brk * dt, d.a_acc * dt);
        v = (v + dv).max(d.a_acc * dt);
        if poses.len() > 40 * 60 * 1000 / dt_ms as usize {
            eprintln!("centreline: 40 minutes of samples without finishing {laps} laps — stopping");
            break;
        }
        let t = _t;
        let pos = interp(i, t, &pts);
        // heading over ±2 m so a 3 m polyline joint does not snap the car
        let ahead = interp(((s + 2.0) / step).rem_euclid(n as f32).floor() as usize % n, 0.0, &smoothed);
        let behind = interp(((s - 2.0) / step).rem_euclid(n as f32).floor() as usize % n, 0.0, &smoothed);
        let mut fwd = sub(ahead, behind);
        if len(fwd) < 1e-4 {
            fwd = [0.0, 0.0, 1.0];
        }
        let l = len(fwd);
        fwd = [fwd[0] / l, fwd[1] / l, fwd[2] / l];
        let right = cross([0.0, 1.0, 0.0], fwd);
        let rl = len(right).max(1e-6);
        let right = [right[0] / rl, right[1] / rl, right[2] / rl];
        let up = cross(fwd, right);
        // the driver's line: sideways by the lateral offset (the road height
        // re-read there), and yawed into the bend through a drift zone
        let frac = s / lap_len;
        let lat = line.lateral_at(frac);
        let mut pos = pos;
        if lat.abs() > 1e-3 {
            let (x, z) = (pos[0] + right[0] * lat, pos[2] + right[2] * lat);
            let y = road_y(soup, x, z, pos[1]).unwrap_or(pos[1]);
            pos = [x, y, z];
        }
        let mut fwd_out = fwd;
        if line.drifting(frac) && line.drift_deg != 0.0 {
            // slide: the nose points into the bend (the sign of the curvature)
            let turn = {
                let a2 = interp(((s + 6.0) / step).rem_euclid(n as f32).floor() as usize % n, 0.0, &smoothed);
                let b2 = interp(((s - 6.0) / step).rem_euclid(n as f32).floor() as usize % n, 0.0, &smoothed);
                let d1 = sub(a2, pos);
                let d0 = sub(pos, b2);
                d0[2] * d1[0] - d0[0] * d1[2]
            };
            let ang = line.drift_deg.to_radians() * if turn > 0.0 { 1.0 } else { -1.0 };
            let (sn, cs) = (ang.sin(), ang.cos());
            fwd_out = [cs * fwd[0] + sn * fwd[2], fwd[1], -sn * fwd[0] + cs * fwd[2]];
        }
        poses.push(Pose { pos: [pos[0], pos[1] + CAR_ORIGIN_Y, pos[2]], fwd: fwd_out, up, speed: v });
        // advance
        let s_next = s + v * dt;
        while next_cp < cp_fr.len() && s_next >= cp_fr[next_cp] * lap_len {
            wp_ms.push(t_ms + dt_ms);
            next_cp += 1;
        }
        if s_next >= lap_len {
            lap += 1;
            lap_ms.push(t_ms + dt_ms);
            wp_ms.push(t_ms + dt_ms);
            next_cp = 0;
            s = s_next - lap_len;
        } else {
            s = s_next;
        }
        t_ms += dt_ms;
    }
    Trajectory { poses, lap_ms, lap_len, wp_ms }
}

/// The (x, y, z, w) quaternion rotating local axes (+x left, +y up, +z
/// forward) into the world frame given by `fwd` and `up`.
pub fn quat_of(fwd: [f32; 3], up: [f32; 3]) -> [f64; 4] {
    // the car's local +x is its LEFT: right-handed with +y up and +z forward
    let left = cross(up, fwd);
    // rotation matrix R (row-major) with columns = the local axes in world:
    // R = [left | up | fwd]
    let m: [f64; 9] = [
        left[0] as f64, up[0] as f64, fwd[0] as f64, //
        left[1] as f64, up[1] as f64, fwd[1] as f64, //
        left[2] as f64, up[2] as f64, fwd[2] as f64,
    ];
    mat_to_quat_pub(&m)
}

/// Rotate `v` by the unit quaternion `q` (x, y, z, w).
pub fn rotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    // v' = v + 2w(q×v) + 2 q×(q×v)
    let c1 = [y * v[2] - z * v[1], z * v[0] - x * v[2], x * v[1] - y * v[0]];
    let c2 = [y * c1[2] - z * c1[1], z * c1[0] - x * c1[2], x * c1[1] - y * c1[0]];
    [v[0] + 2.0 * (w * c1[0] + c2[0]), v[1] + 2.0 * (w * c1[1] + c2[1]), v[2] + 2.0 * (w * c1[2] + c2[2])]
}

/// Write the trajectory into a ghost: the donor rebuilt on a 50 ms grid of the
/// drive's length, every sample's transform ours, the declared time = the last
/// lap, the map uid rewritten when given. Returns a one-line report.
pub fn write(traj: &Trajectory, donor: &str, out: &str, uid: Option<&str>, dt_ms: i64) -> Result<String, String> {
    // the grid is inclusive of both ends: N samples span (N-1) periods
    let span_ms = (traj.poses.len() as i64 - 1).max(1) * dt_ms;
    let tmp1 = format!("{out}.grid.tmp");
    let tmp2 = format!("{out}.xf.tmp");
    ghost::record::rebuild_to(donor, &tmp1, span_ms, None, dt_ms, false)?;
    let mut written = 0usize;
    rewrite_ghost(&tmp1, &tmp2, |rd| {
        let vi = rd
            .ents
            .iter()
            .enumerate()
            .filter(|(_, e)| e.sample_size >= 100 && !e.times.is_empty())
            .max_by_key(|(_, e)| e.times.len())
            .map(|(i, _)| i)
            .ok_or("no vehicle entity in the rebuilt record")?;
        let e = &mut rd.ents[vi];
        let ss = e.sample_size;
        let n = e.raw.len() / ss;
        for k in 0..n {
            let p = traj.poses[k.min(traj.poses.len() - 1)];
            let q = quat_of(p.fwd, p.up);
            let vel = [(p.fwd[0] * p.speed) as f64, (p.fwd[1] * p.speed) as f64, (p.fwd[2] * p.speed) as f64];
            write_transform(&mut e.raw[k * ss..(k + 1) * ss], 47, &Xform { pos: p.pos, quat: q, vel });
            written += 1;
        }
        Ok(())
    })?;
    let _ = std::fs::remove_file(&tmp1);
    // declared time: the finish
    let c = ghost::Container::load(&tmp2)?;
    let mut body = c.body().to_vec();
    let finish_ms = traj.lap_ms.last().copied().unwrap_or(span_ms) as u32;
    ghost::trim::set_all_declared(&mut body, finish_ms);
    // the split list: our waypoint crossings (the donor's three checkpoints
    // rode along until 2026-09-25 — a validation ghost's list must be the
    // map's, in driving order, the last entry the race time)
    if !traj.wp_ms.is_empty() {
        if let Some(mut r) = gbx::container::read_result(&body) {
            r.race_ms = finish_ms as i32;
            r.entries = traj.wp_ms.iter().map(|t| (*t as i32, 1)).collect();
            if let Some(last) = r.entries.last_mut() {
                last.0 = finish_ms as i32;
            }
            body = gbx::container::write_result(&body, &r)?;
        }
    }
    // map uid: every 27-char uid literal becomes the map's
    let mut uids = 0usize;
    if let Some(u) = uid {
        if u.len() != 27 {
            return Err(format!("--uid {u}: a map uid is 27 characters"));
        }
        let mut i = 0usize;
        while i + 31 <= body.len() {
            let l = u32::from_le_bytes(body[i..i + 4].try_into().unwrap());
            if l == 27 {
                if let Ok(s) = std::str::from_utf8(&body[i + 4..i + 31]) {
                    if s.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
                        body[i + 4..i + 31].copy_from_slice(u.as_bytes());
                        uids += 1;
                        i += 31;
                        continue;
                    }
                }
            }
            i += 1;
        }
    }
    gbx::container::write_gbx(&c.gbx, body, out)?;
    let _ = std::fs::remove_file(&tmp2);
    // control: decode and compare
    let back = gbx::record::decode_ghost(out)?;
    if back.samples.len() != traj.poses.len() {
        return Err(format!("{out}: {} samples decode, {} were written", back.samples.len(), traj.poses.len()));
    }
    let mut worst = 0.0f32;
    let mut worst_fwd = 0.0f64;
    for (s, p) in back.samples.iter().zip(&traj.poses) {
        worst = worst.max(((s.x - p.pos[0]).powi(2) + (s.y - p.pos[1]).powi(2) + (s.z - p.pos[2]).powi(2)).sqrt());
        let f = rotate([s.qx as f64, s.qy as f64, s.qz as f64, s.qw as f64], [0.0, 0.0, 1.0]);
        let d = 1.0 - (f[0] * p.fwd[0] as f64 + f[1] * p.fwd[1] as f64 + f[2] * p.fwd[2] as f64);
        worst_fwd = worst_fwd.max(d);
    }
    Ok(format!(
        "{out}: {written} samples ({:.1} s), finish {:.3} s, laps at {:?} s, lap {:.0} m, uid literals {uids}; readback worst pos {:.4} m, worst forward 1-cos {:.4}",
        span_ms as f64 / 1000.0,
        finish_ms as f64 / 1000.0,
        traj.lap_ms.iter().map(|m| *m as f64 / 1000.0).collect::<Vec<_>>(),
        traj.lap_len,
        worst,
        worst_fwd
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Which way does a real ghost's quaternion turn the car? For every fast
    /// sample of the donor, rotate the candidate local forward axes by q and
    /// compare with the velocity direction.
    #[test]
    fn quat_convention() {
        let donor = std::env::var("MK64_GHOST_DONOR").unwrap_or_else(|_| "../testdata/human_22730.Ghost.Gbx".into());
        if !Path::new(&donor).is_file() {
            return;
        }
        let g = gbx::record::decode_ghost(&donor).unwrap();
        let mut agree = [0.0f64; 6];
        let mut n = 0;
        for s in &g.samples {
            if s.speed_ms < 8.0 {
                continue;
            }
            let vd = [s.vx as f64 / s.speed_ms as f64, s.vy as f64 / s.speed_ms as f64, s.vz as f64 / s.speed_ms as f64];
            let q = [s.qx as f64, s.qy as f64, s.qz as f64, s.qw as f64];
            let axes = [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]];
            for (k, a) in axes.iter().enumerate() {
                let f = rotate(q, *a);
                agree[k] += f[0] * vd[0] + f[1] * vd[1] + f[2] * vd[2];
            }
            n += 1;
        }
        for k in 0..6 {
            agree[k] /= n.max(1) as f64;
        }
        eprintln!("{n} fast samples; mean cos(axis, velocity) for local +x -x +y -y +z -z: {agree:.3?}");
        // and the up axis: local +y should point up on a car that drives on its wheels
        let mut up = 0.0f64;
        for s in &g.samples {
            let f = rotate([s.qx as f64, s.qy as f64, s.qz as f64, s.qw as f64], [0.0, 1.0, 0.0]);
            up += f[1];
        }
        eprintln!("mean world-y of rotated local +y: {:.3}", up / g.samples.len().max(1) as f64);
        assert!(agree[4] > 0.9, "local +z is not the forward axis under `rotate`: {agree:?}");
    }

    #[test]
    fn quat_of_roundtrip() {
        let fwd = [0.6f32, 0.0, 0.8];
        let up = [0.0f32, 1.0, 0.0];
        let q = quat_of(fwd, up);
        let f = rotate(q, [0.0, 0.0, 1.0]);
        let u = rotate(q, [0.0, 1.0, 0.0]);
        assert!((f[0] - 0.6).abs() < 1e-6 && (f[2] - 0.8).abs() < 1e-6, "{f:?}");
        assert!((u[1] - 1.0).abs() < 1e-6, "{u:?}");
    }
}
