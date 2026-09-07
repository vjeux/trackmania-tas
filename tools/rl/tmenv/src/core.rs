//! The environment core: observation, reward, termination.
//!
//! **Deliberately backend-independent.** It consumes per-tick car state
//! (`forkoracle::layout::Row`) and knows nothing about where those rows came
//! from. The same code therefore runs
//!
//! * over a live savestate tree (`crate::forkenv`), which is the trainer's env;
//! * over a trajectory read back from a flat, from-zero simulation of the tape
//!   the env wrote, which is the **control** — if the two disagree the env is
//!   returning fiction and everything trained on it is fiction;
//! * later, inside the engine, without changing a line of reward code.
//!
//! An environment that silently returns garbage trains a policy on garbage and
//! nothing downstream can see it. That is why the core is separable at all: it
//! is the only way to run the identical reward on two independent trajectory
//! sources.

use crate::action::ActionSpace;
use crate::geom::*;
use crate::track::{GateTracker, Track};
use std::sync::Arc;
use forkoracle::layout::Row;

pub const TICK_S: f32 = 0.010;

/// Why an episode ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Done {
    /// Every gate collected, finish included. The *authoritative* time still
    /// comes from the plain oracle re-simulating the written tape; this is the
    /// env's own reading and is never banked as a result.
    Finished,
    /// Off the corridor for long enough that it is not a wide line.
    OffRoute,
    /// Progress stopped and the car is not airborne.
    NoProgress,
    /// Ran out of tape.
    TickCap,
}

#[derive(Clone, Debug)]
pub struct CoreCfg {
    /// Ticks held per action.
    pub k_ticks: usize,
    /// Episode cap in ticks. Must not exceed the container's declared time, or
    /// the engine stops before the cap and the truncation looks like a crash.
    pub max_ticks: usize,

    // ---- reward
    /// Cost per second of race time.
    pub c_time: f32,
    /// Reward per metre of NEW saturated progress.
    pub c_prog: f32,
    /// Paid once, on collecting the finish gate.
    pub finish_bonus: f32,
    /// Paid once, on an off-route or no-progress termination.
    pub crash_penalty: f32,

    // ---- termination
    /// Off-route once |lateral| exceeds `half_width * offroute_scale +
    /// offroute_margin`.
    pub offroute_scale: f32,
    pub offroute_margin: f32,
    /// Consecutive ticks off the corridor before the run is cut.
    pub offroute_ticks: usize,
    /// Ticks without new progress before the run is cut.
    pub noprog_ticks: usize,
    /// The airborne guard: no-progress never fires while the car is this far
    /// above the road or moving this fast vertically.
    pub air_height: f32,
    pub air_vy: f32,

    // ---- observation
    /// Distances ahead along the route, metres.
    pub lookahead: Vec<f32>,
    /// Speed used to scale velocity features, m/s.
    pub v_scale: f32,
    /// Distance used to scale lookahead features, m.
    pub d_scale: f32,

    /// Saturate progress at the first gate still owed.
    ///
    /// # Off by default, and that is a STATED TRADE, not an oversight
    ///
    /// The rule it implements is real: arc length is gameable, and a car that
    /// cuts a corner accrues progress it did not earn. But the cap is only as
    /// good as the gate detector feeding it, and on Summer 2026 - 01 that
    /// detector does NOT agree with the plain oracle -- 40 tapes, the server
    /// credits a checkpoint on 20 of them, and no radius, no arc-length
    /// window and no plane-crossing direction reproduces that split. Which
    /// geometric event the validator credits is UNKNOWN and is a task.
    ///
    /// A cap driven by a detector that under-reports pins progress at the
    /// first gate's arc length forever, and the policy can then never be paid
    /// past the first sixth of the map -- silently, with a reward that looks
    /// fine. That is a worse failure than the one the cap prevents.
    ///
    /// What replaces it here: the CORRIDOR. Off-route termination fires when
    /// the car leaves the road by more than its half-width plus a margin, and
    /// on this map the road physically passes through every gate, so a car
    /// that stays on it cannot skip one. That substitution is map-specific and
    /// it is written down rather than assumed: on a map with a genuine
    /// shortcut it does not hold, and the cap must come back with a detector
    /// the oracle agrees with.
    ///
    /// And underneath all of it, the standing rule is untouched: a result is a
    /// tape the plain oracle re-simulates, so a shaping term that flatters a
    /// shortcut costs signal quality and can never make a false result true.
    pub gate_cap: bool,
}

impl Default for CoreCfg {
    fn default() -> Self {
        CoreCfg {
            k_ticks: 10,
            max_ticks: 4000,

            // Break-even speed is `c_time / c_prog` = 30 m/s. Above it a step
            // pays; below it a step costs.
            //
            // This is the one place we deliberately depart from Linesight's
            // shape rather than its numbers. Theirs is
            // `-0.0012/ms + 0.01/m`, i.e. break-even at 120 m/s = 432 km/h, so
            // **every step is negative** and an episode-return objective would
            // reward self-termination; they buy that back with a fixed 7.000 s
            // horizon that makes Q an undiscounted sum over a window rather
            // than over the episode. We do not need that machinery: putting
            // break-even below any speed a moving car sustains removes the
            // incentive to die, and the episode return is then exactly
            // `c_prog * L - c_time * T` plus the bonus, which is minimised-T by
            // construction.
            c_time: 0.30,
            c_prog: 0.01,
            finish_bonus: 10.0,
            crash_penalty: 1.0,

            offroute_scale: 1.0,
            offroute_margin: 4.0,
            offroute_ticks: 40,
            noprog_ticks: 250,
            air_height: 2.0,
            air_vy: 3.0,

            lookahead: vec![5.0, 10.0, 20.0, 35.0, 55.0, 80.0, 120.0, 170.0, 240.0],
            v_scale: 100.0,
            d_scale: 100.0,
            gate_cap: false,
        }
    }
}

/// What the caller learns about a step, beyond the reward.
#[derive(Clone, Copy, Debug, Default)]
pub struct Info {
    pub tick: usize,
    pub race_s: f32,
    pub s: f32,
    pub best_s: f32,
    pub gates: usize,
    pub speed: f32,
    pub lateral: f32,
    pub height: f32,
    /// New saturated progress this step, metres.
    pub dprog: f32,
    /// True if the airborne guard suppressed a no-progress cut this step.
    pub air_guarded: bool,
}

pub struct Core {
    pub cfg: CoreCfg,
    pub track: Arc<Track>,
    pub acts: ActionSpace,
    pub gates: GateTracker,

    prev: Option<Row>,
    cur: Row,
    best_s: f32,
    last_gain_tick: usize,
    off_run: usize,
    tick: usize,
    prev_action: usize,
    /// Where the route probe was last, so arc length is tracked forward from
    /// where the car IS rather than re-found on the whole polyline every tick.
    cur_s: f32,
    /// Sticky: an episode that already ended stays ended.
    done: Option<Done>,
    obs_dim: usize,
}

impl Core {
    pub fn new(cfg: CoreCfg, track: Arc<Track>, acts: ActionSpace) -> Core {
        let gates = GateTracker::new(12.0, 25.0);
        let mut c = Core {
            cfg,
            track,
            acts,
            gates,
            prev: None,
            cur: zero_row(),
            best_s: 0.0,
            last_gain_tick: 0,
            off_run: 0,
            tick: 0,
            prev_action: usize::MAX,
            cur_s: 0.0,
            done: None,
            obs_dim: 0,
        };
        c.obs_dim = c.observe().len();
        c
    }

    /// The observation width.
    ///
    /// Measured from `observe` itself rather than recomputed from a formula.
    /// The formula and the code disagreed -- 77 against 80 -- and the
    /// `debug_assert_eq!` that would have caught it is compiled out of a
    /// release build, so the first thing that noticed was a slice index panic
    /// inside a rollout worker after the fleet had spent 45 s coming up. Two
    /// statements of one fact with nothing forcing them to agree; now there is
    /// one statement.
    pub fn obs_dim(&self) -> usize {
        self.obs_dim
    }

    pub fn done(&self) -> Option<Done> {
        self.done
    }

    pub fn tick(&self) -> usize {
        self.tick
    }

    pub fn best_s(&self) -> f32 {
        self.best_s
    }

    /// The most recent row the core ingested, for diagnostics.
    pub fn last_row(&self) -> Row {
        self.cur
    }

    pub fn gates_hit(&self) -> usize {
        self.gates.hit()
    }

    /// Start an episode from the state the engine is in at `row0`.
    pub fn reset(&mut self, row0: Row, tick0: usize) -> Vec<f32> {
        self.prev = None;
        self.cur = row0;
        self.gates.reset();
        self.tick = tick0;
        self.last_gain_tick = tick0;
        self.off_run = 0;
        self.prev_action = usize::MAX;
        self.done = None;
        self.cur_s = 0.0;
        let p = self.pos();
        // The first probe is windowed around the START of the route, not taken
        // over the whole polyline: on a route that reuses a road the global
        // nearest point at the start line can be the LAST leg.
        let pr = self.track.probe_near(p, Some(0.0));
        self.cur_s = pr.s;
        self.gates.observe(&self.track, p, pr.s);
        self.best_s = if self.cfg.gate_cap { pr.s.min(self.gates.cap(&self.track)) } else { pr.s };
        self.observe()
    }

    fn pos(&self) -> V3 {
        [self.cur.x as f32, self.cur.y as f32, self.cur.z as f32]
    }

    fn vel(&self) -> V3 {
        [self.cur.vx as f32, self.cur.vy as f32, self.cur.vz as f32]
    }

    fn quat(&self) -> Quat {
        Quat(self.cur.qx as f32, self.cur.qy as f32, self.cur.qz as f32, self.cur.qw as f32)
    }

    /// Fold in the per-tick rows one action produced, and score them.
    ///
    /// Every tick is examined, not just the last: a gate collected in the
    /// middle of a ten-tick macro is collected, and a car that left the
    /// corridor and came back within one macro really did leave it.
    pub fn ingest(&mut self, action: usize, rows: &[Row]) -> (Vec<f32>, f32, Option<Done>, Info) {
        let mut info = Info::default();
        if self.done.is_some() {
            return (self.observe(), 0.0, self.done, info);
        }
        let mut reward = 0.0f32;
        let ticks = rows.len();

        for r in rows {
            self.prev = Some(self.cur);
            self.cur = *r;
            self.tick += 1;

            let p = self.pos();
            let pr = self.track.probe_near(p, Some(self.cur_s));
            self.cur_s = pr.s;
            self.gates.observe(&self.track, p, pr.s);

            // Saturating progress, and only ever the NEW maximum: a car that
            // drives back and forth over the same stretch is not paid twice,
            // so there is no reward to farm short of the finish.
            let cap = if self.cfg.gate_cap { self.gates.cap(&self.track) } else { self.track.length() };
            let s_eff = pr.s.min(cap);
            if s_eff > self.best_s {
                info.dprog += s_eff - self.best_s;
                self.best_s = s_eff;
                self.last_gain_tick = self.tick;
            }

            let limit = pr.half_width * self.cfg.offroute_scale + self.cfg.offroute_margin;
            if pr.lateral.abs() > limit {
                self.off_run += 1;
            } else {
                self.off_run = 0;
            }

            let airborne = pr.height > self.cfg.air_height
                || (self.cur.vy as f32).abs() > self.cfg.air_vy;
            if airborne {
                info.air_guarded = true;
            }

            if self.gates.finished(&self.track) {
                self.done = Some(Done::Finished);
            } else if self.off_run >= self.cfg.offroute_ticks {
                self.done = Some(Done::OffRoute);
            } else if !airborne && self.tick - self.last_gain_tick >= self.cfg.noprog_ticks {
                self.done = Some(Done::NoProgress);
            } else if self.tick >= self.cfg.max_ticks {
                self.done = Some(Done::TickCap);
            }

            info.s = pr.s;
            info.lateral = pr.lateral;
            info.height = pr.height;
            if self.done.is_some() {
                break;
            }
        }

        reward += self.cfg.c_prog * info.dprog;
        reward -= self.cfg.c_time * (ticks as f32) * TICK_S;
        match self.done {
            Some(Done::Finished) => reward += self.cfg.finish_bonus,
            Some(Done::OffRoute) | Some(Done::NoProgress) => reward -= self.cfg.crash_penalty,
            _ => {}
        }

        self.prev_action = action;
        info.tick = self.tick;
        info.race_s = self.cur.time_ms as f32 / 1000.0;
        info.best_s = self.best_s;
        info.gates = self.gates.hit();
        info.speed = norm(self.vel());
        (self.observe(), reward, self.done, info)
    }

    /// The observation vector.
    pub fn observe(&self) -> Vec<f32> {
        let cfg = &self.cfg;
        let mut o = Vec::with_capacity(self.obs_dim());
        let p = self.pos();
        let q = self.quat();
        let v = self.vel();
        let pr = self.track.probe_near(p, Some(self.cur_s));

        let vc = q.world_to_car(v);
        o.push(vc[0] / cfg.v_scale);
        o.push(vc[1] / cfg.v_scale);
        o.push(vc[2] / cfg.v_scale);
        o.push(norm(v) / cfg.v_scale);

        let up = q.world_to_car([0.0, 1.0, 0.0]);
        o.extend_from_slice(&up);

        let w = match self.prev {
            Some(pv) => ang_vel_car(
                Quat(pv.qx as f32, pv.qy as f32, pv.qz as f32, pv.qw as f32),
                q,
                TICK_S,
            ),
            None => [0.0, 0.0, 0.0],
        };
        o.push(w[0] / 10.0);
        o.push(w[1] / 10.0);
        o.push(w[2] / 10.0);

        o.push(self.cur.wetness as f32);

        o.push(pr.lateral / 20.0);
        o.push(pr.height / 10.0);
        o.push(pr.half_width / 20.0);
        o.push(if pr.lateral.abs() <= pr.half_width { 1.0 } else { 0.0 });

        let len = self.track.length().max(1.0);
        o.push(pr.s / len);
        let cap = if self.cfg.gate_cap { self.gates.cap(&self.track) } else { self.track.length() };
        o.push(cap / len);
        o.push(((cap - pr.s) / cfg.d_scale).clamp(-10.0, 10.0));
        o.push(self.tick as f32 / cfg.max_ticks as f32);

        // Gates collected, one-hot over 0..=n.
        let n = self.track.n_gates();
        for i in 0..=n {
            o.push(if self.gates.hit() == i { 1.0 } else { 0.0 });
        }

        // The track ahead, in the car's frame. Linesight's single most
        // load-bearing observation block (their 40 zone centres, 400 m of
        // lookahead); the Neinders ablation moved 44.413 to 38.402 on adding
        // curvature lookahead alone.
        for (i, la) in self.track.lookahead(pr.s, p, q, &cfg.lookahead).iter().enumerate() {
            o.push(la[0] / cfg.d_scale);
            o.push(la[1] / cfg.d_scale);
            o.push(la[2] / cfg.d_scale);
            o.push(self.track.half_width(pr.s + cfg.lookahead[i]) / 20.0);
        }

        for i in 0..self.acts.n() {
            o.push(if self.prev_action == i { 1.0 } else { 0.0 });
        }

        // A real assert, not a debug_assert: the release build is the one that
        // runs, and this is the invariant every consumer of the vector depends
        // on. It is free next to a fork.
        assert!(
            self.obs_dim == 0 || o.len() == self.obs_dim,
            "the observation is {} wide but this Core was built at {}",
            o.len(),
            self.obs_dim
        );
        o
    }
}

pub fn zero_row() -> Row {
    Row {
        time_ms: 0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
        vx: 0.0,
        vy: 0.0,
        vz: 0.0,
        qx: 0.0,
        qy: 0.0,
        qz: 0.0,
        qw: 1.0,
        wetness: 0.0,
    }
}
