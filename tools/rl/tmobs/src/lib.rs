//! `tmobs` — the ONE observation function.
//!
//! `observe(&TrackGeom, &CarState, prev_actions) -> Obs` is a **pure function of
//! its arguments**: no engine, no file, no hidden state. That is what makes
//! behaviour cloning on recorded ghosts possible at all — a recorded human
//! sample turned into a `CarState` and a live fork-server state turned into a
//! `CarState` go through this exact code and come out as the same floats.
//! `tmenv` does not have its own observation; it calls this.
//!
//! # Layout, `OBS_VERSION` 1, `OBS_DIM` 80
//!
//! Fixed width on every map and for every action head — a generalist policy
//! is trained across thousands of maps, so nothing in the vector may depend on
//! a map's gate count or on the discrete action table (the a7aa56c layout did
//! both; that is the one place this departs from it).
//!
//! | idx | n | block |
//! |---|---|---|
//! | 0 | 4 | velocity in the car frame `/V_SCALE`, then speed `/V_SCALE` |
//! | 4 | 3 | map-up in the car frame |
//! | 7 | 3 | angular velocity, car frame, `/W_SCALE` (0 when the state has NaN) |
//! | 10 | 1 | reserved — was tyre wetness; `CarState` v1 carries none, so 0 |
//! | 11 | 4 | signed lateral offset `/20`, height above road `/10`, corridor half-width `/20`, on-corridor flag |
//! | 15 | 4 | progress `s/length`, progress cap `/length`, distance owed `(cap−s)/D_SCALE` clamped ±10, race time `/T_SCALE` |
//! | 19 | 5 | gates: collected fraction, remaining count `/8` clamped, next gate ahead `/D_SCALE` clamped, finish ahead `/D_SCALE` clamped, next-is-finish flag |
//! | 24 | 36 | the track ahead: 9 route points at `LOOKAHEAD` metres along the route, in the car frame `/D_SCALE`, each followed by the corridor half-width there `/20` |
//! | 60 | 20 | the last 5 actions, most recent first: `steer/127, gas, brake, present` |
//!
//! Progress is arc length along `TrackGeom.pts`, found by projecting the car
//! onto the polyline's SEGMENTS (vertex snapping makes a staircase) **inside
//! the leg the car is on**: the range between the last credited gate and the
//! next one, `LEG_MARGIN` either side. Routes reuse roads (Summer 2026 - 01
//! drives its first straight again at the end), so a global nearest point
//! jumps between legs; `CarState.cps` says which leg, and it is the engine's
//! own count when the readout has it. The cap is the next gate's `s`: a car
//! that leaves the road and rejoins past a checkpoint is not paid for it.
//!
//! Every constant here is part of the observation's definition: change one and
//! bump `OBS_VERSION`.

use tmstate::{Action, CarState, GateKind, TrackGeom};

pub const OBS_VERSION: u32 = 1;
pub const OBS_DIM: usize = 80;

pub const V_SCALE: f32 = 100.0;
pub const D_SCALE: f32 = 100.0;
/// Race time scale, ms. Beyond this the feature simply exceeds 1.
pub const T_SCALE_MS: f32 = 30_000.0;
pub const W_SCALE: f32 = 10.0;
/// Distances ahead along the route, metres.
pub const LOOKAHEAD: [f32; 9] = [5.0, 10.0, 20.0, 35.0, 55.0, 80.0, 120.0, 170.0, 240.0];
/// How many previous actions the vector carries.
pub const N_PREV: usize = 5;
/// How far outside the current leg the progress projection may look, metres.
pub const LEG_MARGIN: f32 = 40.0;
/// Corridor half-width floor, metres (a point with a smaller one reads as this).
pub const HALF_WIDTH_FLOOR: f32 = 4.0;

pub type Obs = [f32; OBS_DIM];
pub type V3 = [f32; 3];

// ------------------------------------------------------------------ vectors

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn norm(a: V3) -> f32 {
    dot(a, a).sqrt()
}
pub fn unit(a: V3) -> V3 {
    let n = norm(a);
    if n < 1e-9 { [0.0; 3] } else { scale(a, 1.0 / n) }
}

/// A unit quaternion `(w, x, y, z)` — `CarState::quat`'s order. The engine
/// stores `(x, y, z, w)`; the conversion into `CarState` reorders it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Quat {
    pub fn from_wxyz(q: [f32; 4]) -> Quat {
        Quat { w: q[0], x: q[1], y: q[2], z: q[3] }
    }
    pub fn conj(self) -> Quat {
        Quat { w: self.w, x: -self.x, y: -self.y, z: -self.z }
    }
    pub fn mul(self, o: Quat) -> Quat {
        Quat {
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
        }
    }
    /// Rotate a car-frame vector into the world (the engine's convention).
    pub fn rotate(self, v: V3) -> V3 {
        let u = [self.x, self.y, self.z];
        let s = self.w;
        let a = scale(u, 2.0 * dot(u, v));
        let b = scale(v, s * s - dot(u, u));
        let c = scale(cross(u, v), 2.0 * s);
        add(add(a, b), c)
    }
    /// Rotate a world vector into the car's frame.
    pub fn world_to_car(self, v: V3) -> V3 {
        self.conj().rotate(v)
    }
}

/// Angular velocity in the car frame from two consecutive orientations, rad/s.
///
/// The one place this is defined: the env differences its per-tick quaternions
/// with it, and the DATA arm differences its interpolated ones with it, so
/// `CarState::ang_vel` means the same thing from both sources.
pub fn ang_vel_from_quats(prev: [f32; 4], cur: [f32; 4], dt_s: f32) -> V3 {
    if dt_s <= 0.0 {
        return [0.0; 3];
    }
    let mut d = Quat::from_wxyz(prev).conj().mul(Quat::from_wxyz(cur));
    // q and -q are one rotation; take the short way round or a sign flip in
    // the convention reads as an enormous spin.
    if d.w < 0.0 {
        d = Quat { w: -d.w, x: -d.x, y: -d.y, z: -d.z };
    }
    let s = 2.0 / dt_s;
    [d.x * s, d.y * s, d.z * s]
}

// -------------------------------------------------------------------- route

/// Where the car is relative to the route.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    /// Arc length along the route, metres.
    pub s: f32,
    /// Signed horizontal offset from the centreline, positive to the route's right.
    pub lateral: f32,
    /// Height above the route point at `s`.
    pub height: f32,
    pub half_width: f32,
    /// Route tangent at `s`, unit.
    pub tangent: V3,
    /// Arc length progress may not exceed: the next gate still owed (the
    /// finish's `s`, or the route length, once every gate is credited).
    pub cap: f32,
    /// Index of the next gate owed, `gates.len()` when none.
    pub next_gate: usize,
}

/// The route point at arc length `s`, by linear interpolation on `pts`.
pub fn at(g: &TrackGeom, s: f32) -> V3 {
    let n = g.pts.len();
    if n == 0 {
        return [0.0; 3];
    }
    if n == 1 || s <= g.s[0] {
        return g.pts[0];
    }
    if s >= g.s[n - 1] {
        return g.pts[n - 1];
    }
    // binary search for the segment
    let mut lo = 0usize;
    let mut hi = n - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if g.s[mid] <= s { lo = mid } else { hi = mid }
    }
    let (a, b) = (g.pts[lo], g.pts[hi]);
    let ds = g.s[hi] - g.s[lo];
    let t = if ds <= 1e-9 { 0.0 } else { (s - g.s[lo]) / ds };
    add(a, scale(sub(b, a), t))
}

/// Corridor half-width at `s`: the nearer point's, floored.
pub fn half_width(g: &TrackGeom, s: f32) -> f32 {
    let n = g.pts.len();
    if n == 0 || g.half_width.is_empty() {
        return HALF_WIDTH_FLOOR;
    }
    let mut lo = 0usize;
    let mut hi = n - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if g.s[mid] <= s { lo = mid } else { hi = mid }
    }
    let i = if (g.s[hi] - s).abs() < (s - g.s[lo]).abs() { hi } else { lo };
    g.half_width.get(i).copied().unwrap_or(HALF_WIDTH_FLOOR).max(HALF_WIDTH_FLOOR)
}

/// Route tangent at `s`, central difference over 1 m.
pub fn tangent(g: &TrackGeom, s: f32) -> V3 {
    let len = g.length();
    let a = at(g, (s - 1.0).max(0.0));
    let b = at(g, (s + 1.0).min(len));
    let t = unit(sub(b, a));
    if norm(t) < 0.5 { [1.0, 0.0, 0.0] } else { t }
}

/// The arc-length range the car may be projected into, from its credited
/// gate count.
pub fn leg_range(g: &TrackGeom, cps: u8) -> (f32, f32) {
    let n = g.gates.len();
    let k = (cps as usize).min(n);
    let lo = if k == 0 { 0.0 } else { g.gates[k - 1].s };
    let hi = if k < n { g.gates[k].s } else { g.length() };
    // Gate arc lengths are not guaranteed monotone (2 of DATA's 87 maps had a
    // later gate at a smaller s); a reversed range must not panic in `clamp`
    // -- it becomes the span between the two.
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let a = (lo - LEG_MARGIN).max(0.0);
    let b = (hi + LEG_MARGIN).min(g.length()).max(a);
    (a, b)
}

/// Nearest point on the polyline's segments within `[lo, hi]`, as arc length.
pub fn project(g: &TrackGeom, p: V3, lo: f32, hi: f32) -> f32 {
    let n = g.pts.len();
    if n < 2 {
        return 0.0;
    }
    let mut best = (f32::INFINITY, lo);
    for i in 0..n - 1 {
        let (sa, sb) = (g.s[i], g.s[i + 1]);
        if sb < lo || sa > hi {
            continue;
        }
        let (a, b) = (g.pts[i], g.pts[i + 1]);
        let ab = sub(b, a);
        let len2 = dot(ab, ab);
        let t = if len2 < 1e-9 { 0.0 } else { (dot(sub(p, a), ab) / len2).clamp(0.0, 1.0) };
        let q = add(a, scale(ab, t));
        let d = norm(sub(p, q));
        if d < best.0 {
            best = (d, sa + (sb - sa) * t);
        }
    }
    best.1.clamp(lo, hi)
}

/// The route probe for a state: leg-windowed projection, lateral/height split,
/// corridor width, the progress cap.
pub fn probe(g: &TrackGeom, st: &CarState) -> Probe {
    let (lo, hi) = leg_range(g, st.cps);
    let s = project(g, st.pos, lo, hi);
    let c = at(g, s);
    let t = tangent(g, s);
    let d = sub(st.pos, c);
    let up: V3 = [0.0, 1.0, 0.0];
    let right = unit(cross(t, up));
    let n = g.gates.len();
    let next_gate = (st.cps as usize).min(n);
    let cap = if next_gate < n { g.gates[next_gate].s } else { g.length() };
    Probe { s, lateral: dot(d, right), height: d[1], half_width: half_width(g, s), tangent: t, cap, next_gate }
}

// -------------------------------------------------------------- observation

/// THE observation. `prev` is the action history, most recent LAST (as a
/// trace is naturally kept); only the last `N_PREV` are used.
pub fn observe(g: &TrackGeom, st: &CarState, prev: &[Action]) -> Obs {
    let mut o = [0.0f32; OBS_DIM];
    let mut i = 0usize;
    let mut push = |v: f32| {
        o[i] = if v.is_finite() { v } else { 0.0 };
        i += 1;
    };
    let q = Quat::from_wxyz(st.quat);
    let pr = probe(g, st);

    // 0..4 velocity in the car frame, speed
    let vc = q.world_to_car(st.vel);
    push(vc[0] / V_SCALE);
    push(vc[1] / V_SCALE);
    push(vc[2] / V_SCALE);
    let speed = if st.speed.is_finite() { st.speed } else { norm(st.vel) };
    push(speed / V_SCALE);

    // 4..7 map-up in the car frame
    let up = q.world_to_car([0.0, 1.0, 0.0]);
    push(up[0]);
    push(up[1]);
    push(up[2]);

    // 7..10 angular velocity
    push(st.ang_vel[0] / W_SCALE);
    push(st.ang_vel[1] / W_SCALE);
    push(st.ang_vel[2] / W_SCALE);

    // 10 reserved (wetness in the a7aa56c layout; not in CarState v1)
    push(0.0);

    // 11..15 lateral, height, half-width, on-corridor
    push(pr.lateral / 20.0);
    push(pr.height / 10.0);
    push(pr.half_width / 20.0);
    push(if pr.lateral.abs() <= pr.half_width { 1.0 } else { 0.0 });

    // 15..19 progress, cap, owed, time
    let len = g.length().max(1.0);
    push(pr.s / len);
    push(pr.cap / len);
    push(((pr.cap - pr.s) / D_SCALE).clamp(-10.0, 10.0));
    push(st.race_ms as f32 / T_SCALE_MS);

    // 19..24 gates
    let n = g.gates.len();
    let credited = (st.cps as usize).min(n);
    push(if n == 0 { 1.0 } else { credited as f32 / n as f32 });
    push(((n - credited) as f32 / 8.0).min(2.0));
    push(((pr.cap - pr.s) / D_SCALE).clamp(0.0, 10.0));
    let finish_s = g
        .gates
        .iter()
        .find(|gt| gt.kind == GateKind::Finish)
        .map(|gt| gt.s)
        .unwrap_or(g.length());
    push(((finish_s - pr.s) / D_SCALE).clamp(0.0, 10.0));
    push(if pr.next_gate < n && g.gates[pr.next_gate].kind == GateKind::Finish { 1.0 } else { 0.0 });

    // 24..60 the track ahead in the car frame, with its half-width
    for la in LOOKAHEAD {
        let s2 = (pr.s + la).min(g.length());
        let rel = q.world_to_car(sub(at(g, s2), st.pos));
        push(rel[0] / D_SCALE);
        push(rel[1] / D_SCALE);
        push(rel[2] / D_SCALE);
        push(half_width(g, s2) / 20.0);
    }

    // 60..80 the last N_PREV actions, most recent first
    for k in 0..N_PREV {
        match prev.len().checked_sub(1 + k).and_then(|j| prev.get(j)) {
            Some(a) => {
                push(a.steer as f32 / 127.0);
                push(if a.gas { 1.0 } else { 0.0 });
                push(if a.brake { 1.0 } else { 0.0 });
                push(1.0);
            }
            None => {
                push(0.0);
                push(0.0);
                push(0.0);
                push(0.0);
            }
        }
    }
    assert_eq!(i, OBS_DIM, "the observation layout table and the code disagree");
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use tmstate::Gate;

    /// A straight track along −z from the origin, 400 m, one checkpoint at
    /// 200 m and the finish at 400 m, 8 m wide.
    fn straight() -> TrackGeom {
        let n = 201;
        let pts: Vec<V3> = (0..n).map(|i| [0.0, 10.0, -(2.0 * i as f32)]).collect();
        let s: Vec<f32> = (0..n).map(|i| 2.0 * i as f32).collect();
        let gate = |k, at_s: f32| Gate {
            kind: k,
            centre: [0.0, 10.0, -at_s],
            normal: [0.0, 0.0, -1.0],
            half_width: 8.0,
            s: at_s,
            map_waypoint: u32::MAX,
        };
        TrackGeom {
            geom_version: 1,
            map_uid: "test".into(),
            half_width: vec![8.0; n],
            pts,
            s,
            gates: vec![gate(GateKind::Checkpoint, 200.0), gate(GateKind::Finish, 400.0)],
            spawn: [0.0, 10.0, 0.0],
            spawn_yaw: 0.0,
            source: "test".into(),
            legs: None,
            route: None,
        }
    }

    fn car(pos: V3, vel: V3, cps: u8) -> CarState {
        let mut c = CarState::unknown();
        c.pos = pos;
        c.vel = vel;
        c.speed = norm(vel);
        c.quat = [1.0, 0.0, 0.0, 0.0]; // identity: car axes = world axes
        c.ang_vel = [0.0; 3];
        c.cps = cps;
        c.race_ms = 3000;
        c
    }

    #[test]
    fn the_layout_is_eighty_wide_and_the_blocks_land_where_the_table_says() {
        let g = straight();
        let st = car([1.5, 10.0, -50.0], [0.0, 0.0, -30.0], 0);
        let o = observe(&g, &st, &[Action { steer: -127, gas: true, brake: false }]);
        assert_eq!(o.len(), OBS_DIM);
        // speed block
        assert!((o[3] - 0.3).abs() < 1e-6, "speed {}", o[3]);
        // map-up in an identity frame is +y
        assert_eq!(&o[4..7], &[0.0, 1.0, 0.0]);
        // progress 50 of 400, cap = first gate 200
        assert!((o[15] - 0.125).abs() < 1e-6);
        assert!((o[16] - 0.5).abs() < 1e-6);
        assert!((o[17] - 1.5).abs() < 1e-6, "owed {}", o[17]);
        // race time 3 s of 30
        assert!((o[18] - 0.1).abs() < 1e-6);
        // gates: 0 of 2 credited, 2 remaining, next 150 m, finish 350 m, next is not the finish
        assert_eq!(o[19], 0.0);
        assert_eq!(o[20], 0.25);
        assert!((o[21] - 1.5).abs() < 1e-6);
        assert!((o[22] - 3.5).abs() < 1e-6);
        assert_eq!(o[23], 0.0);
        // previous action: hard left, gas
        assert_eq!(&o[60..64], &[-1.0, 1.0, 0.0, 1.0]);
        assert_eq!(&o[64..68], &[0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn lateral_is_signed_to_the_routes_right_and_height_is_split_off() {
        let g = straight();
        // route runs −z; right = tangent × up = (0,0,−1)×(0,1,0) = (1,0,0)
        let st = car([2.0, 13.0, -100.0], [0.0; 3], 0);
        let p = probe(&g, &st);
        assert!((p.s - 100.0).abs() < 1e-4);
        assert!((p.lateral - 2.0).abs() < 1e-4, "lateral {}", p.lateral);
        assert!((p.height - 3.0).abs() < 1e-4, "height {}", p.height);
        assert_eq!(p.half_width, 8.0);
    }

    #[test]
    fn the_credited_gate_count_picks_the_leg_and_moves_the_cap() {
        let g = straight();
        let mut st = car([0.0, 10.0, -250.0], [0.0; 3], 0);
        // cps 0: the projection is confined to the first leg (+ margin), cap = CP
        let p0 = probe(&g, &st);
        assert!(p0.s <= 200.0 + LEG_MARGIN + 1e-3, "s {}", p0.s);
        assert_eq!(p0.cap, 200.0);
        assert_eq!(p0.next_gate, 0);
        // cps 1: second leg, cap = finish
        st.cps = 1;
        let p1 = probe(&g, &st);
        assert!((p1.s - 250.0).abs() < 1e-4);
        assert_eq!(p1.cap, 400.0);
        assert_eq!(p1.next_gate, 1);
        let o = observe(&g, &st, &[]);
        assert_eq!(o[23], 1.0, "the next gate is the finish");
        // cps 2 (all credited): cap = length
        st.cps = 2;
        assert_eq!(probe(&g, &st).cap, 400.0);
    }

    #[test]
    fn the_track_ahead_is_in_the_car_frame() {
        let g = straight();
        let st = car([0.0, 10.0, -100.0], [0.0; 3], 0);
        let o = observe(&g, &st, &[]);
        // first lookahead point: 5 m further along −z
        assert!((o[24] - 0.0).abs() < 1e-6);
        assert!((o[25] - 0.0).abs() < 1e-6);
        assert!((o[26] + 0.05).abs() < 1e-6, "{}", o[26]);
        assert!((o[27] - 0.4).abs() < 1e-6);
        // the 240 m point is 240 m ahead (still on the track)
        let far = 24 + 8 * 4;
        assert!((o[far + 2] + 2.4).abs() < 1e-6, "{}", o[far + 2]);
        // from 300 m on, the far points saturate at the finish (400 m)
        let st2 = car([0.0, 10.0, -300.0], [0.0; 3], 1);
        let o2 = observe(&g, &st2, &[]);
        assert!((o2[far + 2] + 1.0).abs() < 1e-6, "{}", o2[far + 2]);
    }

    #[test]
    fn nan_fields_read_as_zero_never_poison_the_vector() {
        let g = straight();
        let mut st = car([0.0, 10.0, -10.0], [0.0, 0.0, -5.0], 0);
        st.ang_vel = [f32::NAN; 3];
        st.speed = f32::NAN;
        let o = observe(&g, &st, &[]);
        assert!(o.iter().all(|v| v.is_finite()));
        assert!((o[3] - 0.05).abs() < 1e-6, "speed falls back to |vel|");
    }

    #[test]
    fn angular_velocity_is_the_short_way_round() {
        // 0.02 rad about y in one 10 ms tick = 2 rad/s about y (small-angle:
        // the differenced form is 2 sin(theta/2)/dt, exact to 1e-5 here)
        let h = 0.01f32;
        let prev = [1.0, 0.0, 0.0, 0.0];
        let cur = [h.cos(), 0.0, h.sin(), 0.0];
        let w = ang_vel_from_quats(prev, cur, 0.01);
        assert!((w[1] - 2.0).abs() < 1e-3, "{:?}", w);
        // the negated quaternion is the same rotation
        let w2 = ang_vel_from_quats(prev, [-cur[0], -cur[1], -cur[2], -cur[3]], 0.01);
        assert!((w2[1] - 2.0).abs() < 1e-3, "{:?}", w2);
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;
    use tmstate::Gate;

    /// DATA's report: a later gate at a SMALLER arc length made `leg_range`
    /// return lo > hi and `f32::clamp` panicked on a stored record.
    #[test]
    fn a_non_monotone_gate_order_does_not_panic() {
        let n = 101;
        let gate = |s: f32| Gate {
            kind: GateKind::Checkpoint,
            centre: [0.0, 10.0, -s],
            normal: [0.0, 0.0, -1.0],
            half_width: 8.0,
            s,
            map_waypoint: u32::MAX,
        };
        let g = TrackGeom {
            geom_version: 1,
            map_uid: "t".into(),
            pts: (0..n).map(|i| [0.0, 10.0, -(2.0 * i as f32)]).collect(),
            half_width: vec![8.0; n],
            s: (0..n).map(|i| 2.0 * i as f32).collect(),
            gates: vec![gate(150.0), gate(40.0), gate(200.0)],
            spawn: [0.0, 10.0, 0.0],
            spawn_yaw: 0.0,
            source: "test".into(),
            legs: None,
            route: None,
        };
        let mut st = CarState::unknown();
        st.pos = [0.0, 10.0, -100.0];
        st.vel = [0.0; 3];
        st.speed = 0.0;
        st.quat = [1.0, 0.0, 0.0, 0.0];
        for cps in 0..=3u8 {
            st.cps = cps;
            let (lo, hi) = leg_range(&g, cps);
            assert!(lo <= hi, "cps {cps}: {lo} > {hi}");
            let o = observe(&g, &st, &[]);
            assert!(o.iter().all(|v| v.is_finite()));
        }
    }
}
