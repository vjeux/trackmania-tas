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
use forkoracle::layout::Row;
use std::sync::Arc;
use tmstate::{Action, CarState};

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
    /// The ENGINE ended the run inside a step: the car left the world, the
    /// validator declared the replay invalid, or the declared time was
    /// reached. The rows up to that point are real; nothing follows them.
    RunEnded,
}

#[derive(Clone, Debug)]
pub struct CoreCfg {
    /// Ticks held per action.
    pub k_ticks: usize,
    /// `tmobs` layout version: 1 = the 80 floats (LEARN's v1 policies), 2 = 100
    /// floats with the G3 vehicle blocks appended.
    pub obs_version: u32,
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
            obs_version: 1,
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
#[derive(Clone, Copy, Debug)]
pub struct Info {
    /// The car state at the end of the step, as every arm shares it.
    pub state: CarState,
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

impl Default for Info {
    fn default() -> Self {
        Info {
            state: CarState::unknown(),
            tick: 0,
            race_s: 0.0,
            s: 0.0,
            best_s: 0.0,
            gates: 0,
            speed: 0.0,
            lateral: 0.0,
            height: 0.0,
            dprog: 0.0,
            air_guarded: false,
        }
    }
}

#[derive(Clone)]
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
    /// The last `tmobs::N_PREV` actions, oldest first: the observation's
    /// action-history block.
    prev_actions: Vec<Action>,
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
            prev_actions: Vec::new(),
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

    /// End the episode from outside (the engine ended the run). Sticky, like
    /// every other termination; a crash penalty is not charged for it.
    pub fn end(&mut self, why: Done) {
        // Overrides a softer verdict already recorded (OffRoute, NoProgress):
        // there is no engine left to step, and that is the fact a caller that
        // kept stepping past the soft cut must see.
        self.done = Some(why);
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

    /// Checkpoints credited so far.
    ///
    /// **The engine's own counter when the row carries it** (`Row::cps`,
    /// resolved from the validator's participant — gap G1), else the geometric
    /// tracker. The finish increments the counter too, so on a map with n
    /// gates `finished` is `cps >= n`.
    pub fn gates_hit(&self) -> usize {
        if self.cur.cps != u32::MAX {
            self.cur.cps as usize
        } else {
            self.gates.hit()
        }
    }

    /// Whether the row carries the engine's counter at all.
    pub fn engine_cps(&self) -> bool {
        self.cur.cps != u32::MAX
    }

    /// The geometric detector's own count, kept as the cross-check it is.
    pub fn geometric_gates_hit(&self) -> usize {
        self.gates.hit()
    }

    fn is_finished(&self) -> bool {
        if self.cur.cps != u32::MAX {
            self.track.n_gates() > 0 && self.cur.cps as usize >= self.track.n_gates()
        } else {
            self.gates.finished(&self.track)
        }
    }

    /// The arc length progress may not exceed: the first gate still owed,
    /// by the engine's count when it has one.
    fn progress_cap(&self) -> f32 {
        if self.cur.cps != u32::MAX {
            let k = self.cur.cps as usize;
            if k < self.track.n_gates() { self.track.gate_s[k] } else { self.track.length() }
        } else {
            self.gates.cap(&self.track)
        }
    }

    /// Start an episode from the state the engine is in at `row0`.
    pub fn reset(&mut self, row0: Row, tick0: usize) -> Vec<f32> {
        self.prev = None;
        self.cur = row0;
        self.gates.reset();
        self.tick = tick0;
        self.last_gain_tick = tick0;
        self.off_run = 0;
        self.prev_actions.clear();
        self.done = None;
        self.cur_s = 0.0;
        let p = self.pos();
        // The first probe is windowed around the START of the route, not taken
        // over the whole polyline: on a route that reuses a road the global
        // nearest point at the start line can be the LAST leg.
        let pr = self.track.probe_near(p, Some(0.0));
        self.cur_s = pr.s;
        self.gates.observe(&self.track, p, pr.s);
        self.best_s = if self.cfg.gate_cap { pr.s.min(self.progress_cap()) } else { pr.s };
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
    ///
    /// `actions` are the per-TICK inputs the chunk wrote, in order; row `j`
    /// is paired with `actions[min(j, last)]` for the observation's action
    /// history (the last `tmobs::N_PREV` TICKS, which is what a recorded ghost
    /// gives the DATA arm too).
    pub fn ingest(&mut self, actions: &[Action], rows: &[Row]) -> (Vec<f32>, f32, Option<Done>, Info) {
        let mut info = Info::default();
        if self.done.is_some() {
            return (self.observe(), 0.0, self.done, info);
        }
        let mut reward = 0.0f32;
        let ticks = rows.len();

        for (j, r) in rows.iter().enumerate() {
            self.prev = Some(self.cur);
            self.cur = *r;
            self.tick += 1;
            if let Some(a) = actions.get(j.min(actions.len().saturating_sub(1))) {
                self.push_action(*a);
            }

            let p = self.pos();
            let pr = self.track.probe_near(p, Some(self.cur_s));
            self.cur_s = pr.s;
            self.gates.observe(&self.track, p, pr.s);

            // Saturating progress, and only ever the NEW maximum: a car that
            // drives back and forth over the same stretch is not paid twice,
            // so there is no reward to farm short of the finish.
            let cap = if self.cfg.gate_cap { self.progress_cap() } else { self.track.length() };
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

            if self.is_finished() {
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

        info.tick = self.tick;
        info.state = self.state();
        info.race_s = self.cur.time_ms as f32 / 1000.0;
        info.best_s = self.best_s;
        info.gates = self.gates_hit();
        info.speed = norm(self.vel());
        (self.observe(), reward, self.done, info)
    }

    /// The car state at the current tick, as every arm shares it
    /// (`tmstate::CarState`, STATE_VERSION 1).
    ///
    /// What the readout provides: race clock, position, world velocity,
    /// orientation (reordered to `(w, x, y, z)`), speed = |v|, angular velocity
    /// differenced from the previous tick's quaternion (NaN on the first row —
    /// there is no previous), `cps` and `finished` from the geometric gate
    /// tracker. Everything else is UNKNOWN (NaN / u8::MAX), never zero: gear,
    /// rpm, wheel contact/material/slip and turbo are gap G3 — the engine
    /// computes them and the readout does not expose them yet.
    ///
    /// `cps` from the geometric tracker is PROVISIONAL: the oracle does not
    /// agree with it on Summer 2026 - 01 (RL-agentG §5.1). Gap G1 replaces it
    /// with the engine's own counter in this same field.
    pub fn state(&self) -> CarState {
        let mut st = CarState::unknown();
        let r = &self.cur;
        st.race_ms = r.time_ms as i32;
        st.pos = [r.x as f32, r.y as f32, r.z as f32];
        st.vel = [r.vx as f32, r.vy as f32, r.vz as f32];
        st.quat = [r.qw as f32, r.qx as f32, r.qy as f32, r.qz as f32];
        st.speed = norm(st.vel);
        st.ang_vel = match self.prev {
            Some(pv) => tmobs::ang_vel_from_quats(
                [pv.qw as f32, pv.qx as f32, pv.qy as f32, pv.qz as f32],
                st.quat,
                TICK_S,
            ),
            None => [f32::NAN; 3],
        };
        st.cps = self.gates_hit().min(u8::MAX as usize) as u8;
        st.finished = self.is_finished();
        // G3: the live vis state (gear, rpm, wheels, turbo, car). Wheel order is
        // remapped from the engine's FL, FR, RR, RL to tmstate's FL, FR, RL, RR.
        // These describe the car one tick before `race_ms` (tmstate
        // VIS_PHASE_MS_DEFAULT; measured per map by `tmenv wheels-control`).
        let v = &r.vis;
        if v.known {
            const ENGINE_TO_TMSTATE: [usize; 4] = [0, 1, 3, 2];
            st.gear = v.gear;
            st.rpm = v.rpm;
            for (i, k) in ENGINE_TO_TMSTATE.iter().enumerate() {
                st.wheel_contact[i] = v.wheel_contact[*k] as u8;
                st.wheel_material[i] = v.wheel_material[*k];
                st.wheel_slip[i] = v.wheel_slip[*k];
            }
            st.turbo = v.turbo_time;
            st.car = v.car;
        }
        st
    }

    /// The action history the observation sees, oldest first.
    pub fn prev_actions(&self) -> &[Action] {
        &self.prev_actions
    }

    fn push_action(&mut self, a: Action) {
        self.prev_actions.push(a);
        if self.prev_actions.len() > tmobs::N_PREV {
            let drop = self.prev_actions.len() - tmobs::N_PREV;
            self.prev_actions.drain(0..drop);
        }
    }

    /// The observation vector: `tmobs::observe` on this core's state. There is
    /// no other observation code in the env — a recorded human sample turned
    /// into a `CarState` goes through the same function and gets the same
    /// floats (`tmobs` is the ONE observation function; INTERFACES.md).
    pub fn observe(&self) -> Vec<f32> {
        let o = tmobs::observe_version(self.cfg.obs_version, &self.track.geom, &self.state(), &self.prev_actions);
        assert!(
            self.obs_dim == 0 || o.len() == self.obs_dim,
            "the observation is {} wide but this Core was built at {}",
            o.len(),
            self.obs_dim
        );
        o.to_vec()
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
        cps: u32::MAX,
        vis: forkoracle::layout::Vis::UNKNOWN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tmstate::{Gate, GateKind, TrackGeom};

    /// A straight 400 m track along -z at y = 10, one checkpoint, a finish.
    fn straight_geom() -> TrackGeom {
        let n = 201;
        let gate = |k, s: f32| Gate {
            kind: k,
            centre: [0.0, 10.0, -s],
            normal: [0.0, 0.0, -1.0],
            half_width: 8.0,
            s,
            map_waypoint: u32::MAX,
        };
        TrackGeom {
            geom_version: 1,
            map_uid: "straight".into(),
            pts: (0..n).map(|i| [0.0, 10.0, -(2.0 * i as f32)]).collect(),
            half_width: vec![8.0; n],
            s: (0..n).map(|i| 2.0 * i as f32).collect(),
            gates: vec![gate(GateKind::Checkpoint, 200.0), gate(GateKind::Finish, 400.0)],
            spawn: [0.0, 10.0, 0.0],
            spawn_yaw: 0.0,
            source: "test".into(),
            legs: None,
            route: None,
        }
    }

    fn row(t: i64, z: f64, vz: f64) -> Row {
        Row { time_ms: t, x: 0.3, y: 10.0, z, vx: 0.0, vy: 0.0, vz, qx: 0.0, qy: 0.0, qz: 0.0, qw: 1.0, wetness: 0.0, cps: u32::MAX }
    }

    /// THE identity the interface promises: the env's live observation IS
    /// `tmobs::observe` on the `CarState` the env exposes. Bit for bit, on
    /// every step of an episode, including the first (no previous row, so
    /// `ang_vel` is NaN and reads as 0) and after a gate is credited.
    #[test]
    fn the_live_observation_is_tmobs_on_the_exposed_state() {
        let track = Arc::new(Track::from_geom(straight_geom()));
        let mut core = Core::new(CoreCfg { k_ticks: 10, max_ticks: 4000, ..Default::default() }, track.clone(), ActionSpace::default());
        let o0 = core.reset(row(-20, 0.0, 0.0), 0);
        let via_tmobs = tmobs::observe(&track.geom, &core.state(), core.prev_actions());
        assert_eq!(o0.len(), tmobs::OBS_DIM);
        assert!(o0.iter().zip(via_tmobs.iter()).all(|(a, b)| a.to_bits() == b.to_bits()));

        // 60 macros of 10 ticks at 30 m/s: 0.3 m per tick, 180 m -- then past the CP.
        let mut t = -20i64;
        let mut z = 0.0f64;
        for step in 0..80usize {
            let rows: Vec<Row> = (0..10)
                .map(|_| {
                    t += 10;
                    z -= 0.3;
                    row(t, z, -30.0)
                })
                .collect();
            let act = tmstate::Action { steer: ((step as i32 % 5) * 60 - 120) as i8, gas: true, brake: step % 7 == 0 };
            let (obs, _r, done, info) = core.ingest(&[act; 10], &rows);
            let st = core.state();
            let again = tmobs::observe(&track.geom, &st, core.prev_actions());
            assert!(
                obs.iter().zip(again.iter()).all(|(a, b)| a.to_bits() == b.to_bits()),
                "step {step}: the live observation and tmobs disagree"
            );
            // Info carries the same state the observation was computed from.
            // NaN != NaN under PartialEq; compare the rendering, which prints NaN as NaN.
            assert_eq!(format!("{:?}", info.state), format!("{:?}", st));
            assert_eq!(st.race_ms as i64, t);
            assert!((st.speed - 30.0).abs() < 1e-4);
            if done.is_some() {
                break;
            }
        }
        // The straight run crossed the checkpoint plane at 200 m: the tracker
        // credited it, so cps moved and with it the leg the progress is in.
        assert_eq!(core.state().cps, 1, "the checkpoint at 200 m was crossed");
        assert!(core.best_s() > 200.0);
    }

    /// geom.json round trip: what the DATA arm writes, the env reads back to
    /// the same geometry, and the observation on it is unchanged.
    #[test]
    fn geom_json_round_trips_and_observes_identically() {
        let g = straight_geom();
        let dir = std::env::temp_dir().join(format!("tmenv-geom-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("geom.json");
        let a = Track::from_geom(g.clone());
        a.save_geom_json(&p).unwrap();
        let b = Track::load_geom_json(&p).unwrap();
        assert_eq!(*a.geom, *b.geom);
        let mut st = tmstate::CarState::unknown();
        st.pos = [1.0, 10.5, -123.0];
        st.vel = [0.0, 0.0, -40.0];
        st.speed = 40.0;
        st.quat = [1.0, 0.0, 0.0, 0.0];
        st.race_ms = 4000;
        let oa = tmobs::observe(&a.geom, &st, &[]);
        let ob = tmobs::observe(&b.geom, &st, &[]);
        assert!(oa.iter().zip(ob.iter()).all(|(x, y)| x.to_bits() == y.to_bits()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
