//! A toy deterministic car and a toy track: the stub the explorer is built
//! against, and the positive control for the search machinery.
//!
//! # READ THIS BEFORE QUOTING ANY NUMBER OUT OF THIS MODULE
//!
//! **The toy proves the search, not the map.** Nothing measured here is a
//! statement about Trackmania, about a campaign map, or about whether the
//! explorer can finish one. A result on this track means exactly one thing:
//! the archive, the bin key, the macro alphabet and the selection policy
//! compose into something that can drive a rewindable vehicle from a standing
//! start to a finish line it has never seen, with no reference line and no
//! demonstration. That is worth establishing before the real engine is wired
//! in, and it is worth nothing else.
//!
//! The physics here are a caricature — one that deliberately keeps the three
//! properties that make the real problem hard:
//!
//! * **the steer→curvature gain falls with speed** (`ω_max = Ω/(1 + (v/vc)²)`),
//!   which is the one fact a speed-independent controller got wrong badly
//!   enough to drive into a wall;
//! * **there is a grip budget**, so entering a corner too fast means you
//!   cannot turn enough, and there is no wall to lean on — off the ribbon is
//!   off the map;
//! * **there is a gap with a kicker**, so one section is only passable
//!   airborne, at a speed you have to have *already* built. A progress metric
//!   that cannot see airtime scores the whole flight as "stopped".
//!
//! # And the control it CANNOT run
//!
//! The toy's plain oracle and the toy's fork share this physics. So the toy
//! cannot test whether the real fork server lies — checks that agree prove
//! nothing when they read the same source. What it CAN test, and does, is
//! whether the *guard plumbing* works: [`ToySim::inject_boundary_defect`]
//! reproduces the real defect ("a record already consumed cannot be
//! un-consumed") inside the toy fork, and the search must then report phantoms
//! and bank nothing. Defect off, zero phantoms; defect on, every claimed
//! finish refused. Either half alone passes for a broken guard.

use crate::action::Input;
use crate::branch::{Advance, Branch, BranchErr, CarState, Handle, PlainOracle, Progress, Route};
use crate::outcome::Verdict;
use std::collections::HashMap;

const DT: f32 = 0.01;
const A0: f32 = 22.0; // m/s² at a standstill
const VMAX: f32 = 90.0;
const B0: f32 = 30.0;
const DRAG: f32 = 0.0015;
const OMEGA0: f32 = 2.6; // rad/s at walking pace, full lock
const VC: f32 = 26.0;
const MU_G: f32 = 21.0; // lateral acceleration budget, m/s²
const SCRUB: f32 = 1.4;
const G: f32 = 9.81;
const FALL_DEAD_Y: f32 = -30.0;

// ---------------------------------------------------------------- the track

/// A ribbon in space: a centreline, a half-width, holes, and kickers.
pub struct ToyTrack {
    /// Centreline, resampled evenly.
    pts: Vec<[f32; 3]>,
    /// Cumulative arc length at each point.
    cum: Vec<f32>,
    step_m: f32,
    half_width: f32,
    /// Arc-length spans with no road under them.
    holes: Vec<(f32, f32)>,
    /// (arc length, vertical impulse as a fraction of forward speed)
    kickers: Vec<(f32, f32)>,
    cp_s: Vec<f32>,
    /// XZ bucket -> point indices, so `progress` is not a linear scan.
    grid: HashMap<(i32, i32), Vec<u32>>,
    cell: f32,
    spacing: f32,
}

impl ToyTrack {
    /// The demonstration track. ~1090 m: two corners that punish arriving too
    /// fast, a 38 m gap over nothing with a kicker in front of it, and a long
    /// sweeper.
    pub fn demo() -> ToyTrack {
        let mut c: Vec<[f32; 3]> = Vec::new();
        let mut p = [0.0f32, 0.0, 0.0];
        let mut th = 0.0f32; // heading, 0 = +z
        let mut push = |p: &mut [f32; 3], th: &mut f32, len: f32, turn: f32, c: &mut Vec<[f32; 3]>| {
            let n = (len / 1.0).round().max(1.0) as usize;
            for _ in 0..n {
                let d = len / n as f32;
                *th += turn / n as f32;
                p[0] += d * th.sin();
                p[2] += d * th.cos();
                c.push(*p);
            }
        };
        c.push(p);
        push(&mut p, &mut th, 200.0, 0.0, &mut c); // straight
        push(&mut p, &mut th, 62.8, std::f32::consts::FRAC_PI_2, &mut c); // r=40 right 90°
        push(&mut p, &mut th, 150.0, 0.0, &mut c);
        push(&mut p, &mut th, 39.3, -std::f32::consts::FRAC_PI_2, &mut c); // r=25 left 90°
        push(&mut p, &mut th, 130.0, 0.0, &mut c); // run-up to the gap
        push(&mut p, &mut th, 120.0, 0.0, &mut c); // gap + landing
        push(&mut p, &mut th, 188.5, std::f32::consts::PI, &mut c); // r=60 sweeper 180°
        push(&mut p, &mut th, 200.0, 0.0, &mut c);

        let mut cum = Vec::with_capacity(c.len());
        let mut s = 0.0;
        cum.push(0.0);
        for i in 1..c.len() {
            let d = dist3(c[i], c[i - 1]);
            s += d;
            cum.push(s);
        }
        let total = s;
        let gap_at = 200.0 + 62.8 + 150.0 + 39.3 + 130.0;
        let mut t = ToyTrack {
            pts: c,
            cum,
            step_m: 1.0,
            half_width: 8.0,
            holes: vec![(gap_at, gap_at + 38.0)],
            kickers: vec![(gap_at - 2.0, 0.26)],
            cp_s: vec![total * 0.30, total * 0.66],
            grid: HashMap::new(),
            cell: 16.0,
            spacing: 20.0,
        };
        t.build_grid();
        t
    }

    fn build_grid(&mut self) {
        self.grid.clear();
        for (i, p) in self.pts.iter().enumerate() {
            let k = (
                (p[0] / self.cell).floor() as i32,
                (p[2] / self.cell).floor() as i32,
            );
            self.grid.entry(k).or_default().push(i as u32);
        }
    }

    pub fn total(&self) -> f32 {
        *self.cum.last().unwrap()
    }

    /// The road surface under a point, if there is one.
    pub fn ground(&self, pos: [f32; 3]) -> Option<f32> {
        self.ground_at(&self.progress(pos))
    }

    /// The same question when `progress` has already been computed. Calling
    /// `progress` twice per tick was half the toy's cost.
    pub fn ground_at(&self, pr: &Progress) -> Option<f32> {
        if !pr.on_route {
            return None;
        }
        if self.holes.iter().any(|&(a, b)| pr.s >= a && pr.s < b) {
            return None;
        }
        Some(0.0)
    }

}fn dist3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

impl Route for ToyTrack {
    fn progress(&self, pos: [f32; 3]) -> Progress {
        // No allocation: iterate the nine buckets in place. Collecting them
        // into a `Vec` cost a heap allocation on every simulated tick, which
        // at ~10^8 ticks per ablation battery is the whole runtime.
        let cx = (pos[0] / self.cell).floor() as i32;
        let cz = (pos[2] / self.cell).floor() as i32;
        let mut best = (f32::INFINITY, usize::MAX);
        for dx in -1..=1 {
            for dz in -1..=1 {
                if let Some(l) = self.grid.get(&(cx + dx, cz + dz)) {
                    for &i in l {
                        let i = i as usize;
                        let d = (pos[0] - self.pts[i][0]).powi(2) + (pos[2] - self.pts[i][2]).powi(2);
                        if d < best.0 {
                            best = (d, i);
                        }
                    }
                }
            }
        }
        if best.1 == usize::MAX {
            // Off the grid entirely: scan, so the answer is never silently
            // wrong. An absent row is not a failure row.
            for i in 0..self.pts.len() {
                let d = (pos[0] - self.pts[i][0]).powi(2) + (pos[2] - self.pts[i][2]).powi(2);
                if d < best.0 {
                    best = (d, i);
                }
            }
        }
        let i = best.1;
        let lateral_abs = best.0.sqrt();
        // sign from the cross product with the local tangent
        let j = (i + 1).min(self.pts.len() - 1);
        let tx = self.pts[j][0] - self.pts[i][0];
        let tz = self.pts[j][2] - self.pts[i][2];
        let dx = pos[0] - self.pts[i][0];
        let dz = pos[2] - self.pts[i][2];
        let cross = tx * dz - tz * dx;
        let lateral = if cross < 0.0 { lateral_abs } else { -lateral_abs };
        Progress {
            s: self.cum[i],
            lateral,
            on_route: lateral_abs <= self.half_width,
        }
    }
    fn length(&self) -> f32 {
        self.total()
    }
    fn spacing(&self) -> f32 {
        self.spacing
    }
    fn n_checkpoints(&self) -> u32 {
        self.cp_s.len() as u32
    }
}

// ------------------------------------------------------------------ the car

#[derive(Clone, Copy, Debug)]
pub struct ToyCar {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub yaw: f32,
    pub v: f32,
    pub wheels: u8,
    pub airtime: u16,
    pub cps: u32,
    pub tick: u32,
    pub dead: bool,
    pub finished: Option<i64>,
    pub max_s: f32,
    kicked: u32,
}

impl ToyCar {
    pub fn spawn() -> ToyCar {
        ToyCar {
            pos: [0.0, 0.0, 0.0],
            vel: [0.0; 3],
            yaw: 0.0,
            v: 0.0,
            wheels: 0b1111,
            airtime: 0,
            cps: 0,
            tick: 0,
            dead: false,
            finished: None,
            max_s: 0.0,
            kicked: 0,
        }
    }

    pub fn state(&self) -> CarState {
        let (s, c) = ((self.yaw * 0.5).sin(), (self.yaw * 0.5).cos());
        CarState {
            tick: self.tick,
            pos: self.pos,
            vel: self.vel,
            // rotation about +y by yaw
            quat: [c, 0.0, s, 0.0],
            wheels: self.wheels,
            airtime: self.airtime,
            cps: self.cps,
        }
    }

    pub fn alive(&self) -> bool {
        !self.dead && self.finished.is_none()
    }

    pub fn step(&mut self, t: &ToyTrack, inp: Input) {
        if !self.alive() {
            self.tick += 1;
            return;
        }
        self.tick += 1;
        if self.wheels != 0 {
            let mut v = self.v;
            if inp.gas {
                v += A0 * (1.0 - v / VMAX) * DT;
            }
            if inp.brake {
                v -= B0 * DT;
            }
            v -= DRAG * v * v * DT;
            if v < 0.0 {
                v = 0.0;
            }
            let s = inp.steer as f32 / 127.0;
            let omax = OMEGA0 / (1.0 + (v / VC) * (v / VC));
            let mut omega = s * omax;
            let alat = (v * omega).abs();
            if alat > MU_G {
                omega *= MU_G / alat;
                v -= SCRUB * (alat - MU_G) * DT;
                if v < 0.0 {
                    v = 0.0;
                }
            }
            self.yaw += omega * DT;
            self.v = v;
            self.vel = [v * self.yaw.sin(), 0.0, v * self.yaw.cos()];
        } else {
            self.vel[1] -= G * DT;
        }
        self.pos[0] += self.vel[0] * DT;
        self.pos[1] += self.vel[1] * DT;
        self.pos[2] += self.vel[2] * DT;

        let pr = t.progress(self.pos);
        if pr.s > self.max_s {
            self.max_s = pr.s;
        }

        // the kicker: a vertical impulse at a fixed place on the track, taken
        // once, and only by a car that is on the ground to take it.
        if self.wheels != 0 {
            for (i, &(ks, f)) in t.kickers.iter().enumerate() {
                let bit = 1u32 << i;
                if self.kicked & bit == 0 && pr.s >= ks {
                    self.kicked |= bit;
                    self.vel[1] = self.v * f;
                    self.wheels = 0;
                    self.airtime = 0;
                }
            }
        }

        match t.ground_at(&pr) {
            Some(gy) => {
                if self.pos[1] <= gy + 0.02 && self.vel[1] <= 0.0 {
                    if self.wheels == 0 {
                        // landing: recover forward motion from the horizontal
                        // velocity we flew with, and pay for the impact.
                        let vh = (self.vel[0] * self.vel[0] + self.vel[2] * self.vel[2]).sqrt();
                        self.yaw = self.vel[0].atan2(self.vel[2]);
                        self.v = vh * 0.94;
                    }
                    self.pos[1] = gy;
                    self.vel[1] = 0.0;
                    self.wheels = 0b1111;
                    self.airtime = 0;
                } else {
                    self.wheels = 0;
                    self.airtime = self.airtime.saturating_add(1);
                }
            }
            None => {
                self.wheels = 0;
                self.airtime = self.airtime.saturating_add(1);
                if self.pos[1] < FALL_DEAD_Y {
                    self.dead = true;
                }
            }
        }

        while (self.cps as usize) < t.cp_s.len() && pr.s >= t.cp_s[self.cps as usize] {
            self.cps += 1;
        }
        if pr.s >= t.total() - 1.0 && self.wheels != 0 {
            self.finished = Some(self.tick as i64 * 10);
        }
    }
}

/// Simulate a whole tape from a standing start. The one definition of what a
/// tape means in the toy, used by both the fork and the oracle.
pub fn simulate(t: &ToyTrack, tape: &[Input]) -> ToyCar {
    let mut c = ToyCar::spawn();
    for &i in tape {
        c.step(t, i);
        if !c.alive() {
            break;
        }
    }
    c
}

// ------------------------------------------------------------- the backends

/// The toy's plain oracle: re-simulate the written tape from zero.
pub struct ToyOracle<'a> {
    pub track: &'a ToyTrack,
    pub calls: std::cell::Cell<u64>,
}

impl<'a> ToyOracle<'a> {
    pub fn new(track: &'a ToyTrack) -> ToyOracle<'a> {
        ToyOracle { track, calls: std::cell::Cell::new(0) }
    }
}

impl<'a> PlainOracle for ToyOracle<'a> {
    fn confirm(&self, tape: &[Input]) -> Result<Verdict, String> {
        self.calls.set(self.calls.get() + 1);
        let c = simulate(self.track, tape);
        Ok(match c.finished {
            Some(ms) => Verdict::Finish { ms },
            None => Verdict::Dnf { cps: c.cps },
        })
    }
}

struct Parked {
    car: ToyCar,
    tick: u32,
    /// A hash of the prefix that produced `car`.
    ///
    /// **The handle is a hint and it is VERIFIED.** Trusting a handle's tick
    /// alone is exactly the shape of the real defect: two different prefixes
    /// of the same length, one snapshot, and the answer is about a run nobody
    /// asked for.
    tag: u64,
}

/// The toy fork server.
pub struct ToySim<'a> {
    pub track: &'a ToyTrack,
    /// `true` = a live savestate tree (D's rung 1 answers yes). `false` = every
    /// open re-simulates its prefix (the fallback). **The archive must produce
    /// the same result either way**, and `tests/backend_indifference.rs`
    /// checks that it does.
    pub tree: bool,
    parked: HashMap<Handle, Parked>,
    next: Handle,
    pub sim_ticks: u64,
    pub opens: u64,
    pub tick_limit: u32,
    /// Reproduce the real fork defect: silently drop the first `n` inputs of
    /// every advance, because "a record already consumed cannot be
    /// un-consumed". Off by default. Only meaningful in `tree` mode.
    pub defect_ticks: usize,
}

impl<'a> ToySim<'a> {
    pub fn new(track: &'a ToyTrack, tree: bool, tick_limit: u32) -> ToySim<'a> {
        ToySim {
            track,
            tree,
            parked: HashMap::new(),
            next: 1,
            sim_ticks: 0,
            opens: 0,
            tick_limit,
            defect_ticks: 0,
        }
    }

    /// Arm the injected defect. See the module header: this is the negative
    /// half of the guard's two-sided control.
    pub fn inject_boundary_defect(&mut self, n: usize) {
        self.defect_ticks = n;
    }

    fn tag(prefix: &[Input]) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for i in prefix {
            for b in [i.steer as u8, i.gas as u8, i.brake as u8] {
                h ^= b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
        h
    }
}

impl<'a> Branch for ToySim<'a> {
    fn open(&mut self, prefix: &[Input], live: Option<Handle>) -> Result<Handle, BranchErr> {
        self.opens += 1;
        let tag = Self::tag(prefix);
        if self.tree {
            if let Some(h) = live {
                if let Some(p) = self.parked.get(&h) {
                    if p.tick as usize == prefix.len() && p.tag == tag {
                        let car = p.car;
                        let id = self.next;
                        self.next += 1;
                        self.parked.insert(id, Parked { car, tick: p.tick, tag });
                        return Ok(id);
                    }
                }
            }
        }
        let car = simulate(self.track, prefix);
        self.sim_ticks += prefix.len() as u64;
        let id = self.next;
        self.next += 1;
        self.parked.insert(id, Parked { car, tick: prefix.len() as u32, tag });
        Ok(id)
    }

    fn advance(
        &mut self,
        h: Handle,
        from_tick: u32,
        inputs: &[Input],
    ) -> Result<Advance, BranchErr> {
        let p = self.parked.get(&h).ok_or(BranchErr::Stale)?;
        if from_tick < p.tick {
            // The forward-only rule, refused rather than clamped.
            return Err(BranchErr::BelowBoundary { asked: from_tick, boundary: p.tick });
        }
        let mut car = p.car;
        let mut trace = Vec::with_capacity(inputs.len());
        let mut ended = None;
        for (i, &inp) in inputs.iter().enumerate() {
            // the injected defect: the first `defect_ticks` writes are no-ops,
            // so the child runs a hybrid and answers honestly about a run
            // nobody asked for.
            let eff = if i < self.defect_ticks { Input::NEUTRAL } else { inp };
            car.step(self.track, eff);
            self.sim_ticks += 1;
            trace.push(car.state());
            if let Some(ms) = car.finished {
                ended = Some(Verdict::Finish { ms });
                break;
            }
            if car.dead {
                ended = Some(Verdict::Dnf { cps: car.cps });
                break;
            }
            if car.tick >= self.tick_limit {
                ended = Some(Verdict::Dnf { cps: car.cps });
                break;
            }
        }
        let tick = from_tick + trace.len() as u32;
        let handle = if self.tree && ended.is_none() {
            let id = self.next;
            self.next += 1;
            // the tag of the extended prefix is not recomputed here (we do not
            // hold the prefix); it is stamped from the parent plus the macro,
            // which is what `open` will hash.
            let mut t = p.tag;
            for &inp in inputs.iter() {
                for b in [inp.steer as u8, inp.gas as u8, inp.brake as u8] {
                    t ^= b as u64;
                    t = t.wrapping_mul(0x100000001b3);
                }
            }
            self.parked.insert(id, Parked { car, tick, tag: t });
            Some(id)
        } else {
            None
        };
        self.parked.remove(&h);
        Ok(Advance { trace, handle, ended })
    }

    fn close(&mut self, h: Handle) {
        self.parked.remove(&h);
    }

    fn initial_state(&mut self) -> Result<CarState, BranchErr> {
        Ok(ToyCar::spawn().state())
    }

    fn tick_limit(&self) -> u32 {
        self.tick_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_track_is_a_ribbon_with_a_hole_in_it() {
        let t = ToyTrack::demo();
        assert!(t.total() > 1000.0, "{}", t.total());
        // on the centreline at the start there is road
        assert!(t.ground([0.0, 0.0, 5.0]).is_some());
        // twenty metres to the side of the start there is not
        assert!(t.ground([20.0, 0.0, 5.0]).is_none());
        // and there is a hole
        let (a, b) = t.holes[0];
        let mid = (a + b) * 0.5;
        let i = t.cum.partition_point(|&c| c < mid);
        assert!(t.ground(t.pts[i]).is_none(), "the gap has a floor");
    }

    #[test]
    fn full_throttle_and_no_steering_does_not_finish() {
        // The laziest driven tape. If this finished, the track would not be a
        // test of anything.
        let t = ToyTrack::demo();
        let tape = vec![Input { steer: 0, gas: true, brake: false }; 6000];
        let c = simulate(&t, &tape);
        assert!(c.finished.is_none());
        assert!(c.max_s < t.total() * 0.5, "got {} of {}", c.max_s, t.total());
    }

    #[test]
    fn doing_nothing_does_not_finish_either() {
        let t = ToyTrack::demo();
        let c = simulate(&t, &vec![Input::NEUTRAL; 6000]);
        assert!(c.finished.is_none());
        assert!(c.max_s < 1.0);
    }

    #[test]
    fn the_gain_falls_with_speed() {
        // The property the prior attempt's controller got wrong: full lock
        // bends the car to a tight radius at walking pace and barely at all at
        // speed. If this were flat, the toy would not exercise the grip budget.
        let t = ToyTrack::demo();
        let slow = {
            let mut c = ToyCar::spawn();
            c.v = 5.0;
            let y0 = c.yaw;
            c.step(&t, Input { steer: 127, gas: false, brake: false });
            (c.yaw - y0).abs()
        };
        let fast = {
            let mut c = ToyCar::spawn();
            c.v = 70.0;
            let y0 = c.yaw;
            c.step(&t, Input { steer: 127, gas: false, brake: false });
            (c.yaw - y0).abs()
        };
        assert!(slow > fast * 3.0, "slow {} fast {}", slow, fast);
    }

    #[test]
    fn the_simulation_is_deterministic() {
        let t = ToyTrack::demo();
        let tape: Vec<Input> = (0..2000)
            .map(|i| Input {
                steer: ((i * 37) % 255) as i8,
                gas: i % 3 != 0,
                brake: i % 11 == 0,
            })
            .collect();
        let a = simulate(&t, &tape);
        let b = simulate(&t, &tape);
        assert_eq!(a.tick, b.tick);
        assert_eq!(a.pos, b.pos);
        assert_eq!(a.finished, b.finished);
    }
}
