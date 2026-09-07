//! The environment core: observation, reward, termination.
            vis: forkoracle::layout::Vis::UNKNOWN,
//!
            vis: forkoracle::layout::Vis::UNKNOWN,
//! **Deliberately backend-independent.** It consumes per-tick car state
            vis: forkoracle::layout::Vis::UNKNOWN,
//! (`forkoracle::layout::Row`) and knows nothing about where those rows came
            vis: forkoracle::layout::Vis::UNKNOWN,
//! from. The same code therefore runs
            vis: forkoracle::layout::Vis::UNKNOWN,
//!
            vis: forkoracle::layout::Vis::UNKNOWN,
//! * over a live savestate tree (`crate::forkenv`), which is the trainer's env;
            vis: forkoracle::layout::Vis::UNKNOWN,
//! * over a trajectory read back from a flat, from-zero simulation of the tape
            vis: forkoracle::layout::Vis::UNKNOWN,
//!   the env wrote, which is the **control** — if the two disagree the env is
            vis: forkoracle::layout::Vis::UNKNOWN,
//!   returning fiction and everything trained on it is fiction;
            vis: forkoracle::layout::Vis::UNKNOWN,
//! * later, inside the engine, without changing a line of reward code.
            vis: forkoracle::layout::Vis::UNKNOWN,
//!
            vis: forkoracle::layout::Vis::UNKNOWN,
//! An environment that silently returns garbage trains a policy on garbage and
            vis: forkoracle::layout::Vis::UNKNOWN,
//! nothing downstream can see it. That is why the core is separable at all: it
            vis: forkoracle::layout::Vis::UNKNOWN,
//! is the only way to run the identical reward on two independent trajectory
            vis: forkoracle::layout::Vis::UNKNOWN,
//! sources.
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
use crate::action::ActionSpace;
            vis: forkoracle::layout::Vis::UNKNOWN,
use crate::geom::*;
            vis: forkoracle::layout::Vis::UNKNOWN,
use crate::track::{GateTracker, Track};
            vis: forkoracle::layout::Vis::UNKNOWN,
use forkoracle::layout::Row;
            vis: forkoracle::layout::Vis::UNKNOWN,
use std::sync::Arc;
            vis: forkoracle::layout::Vis::UNKNOWN,
use tmstate::{Action, CarState};
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
pub const TICK_S: f32 = 0.010;
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
/// Why an episode ended.
            vis: forkoracle::layout::Vis::UNKNOWN,
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
            vis: forkoracle::layout::Vis::UNKNOWN,
pub enum Done {
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Every gate collected, finish included. The *authoritative* time still
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// comes from the plain oracle re-simulating the written tape; this is the
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// env's own reading and is never banked as a result.
            vis: forkoracle::layout::Vis::UNKNOWN,
    Finished,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Off the corridor for long enough that it is not a wide line.
            vis: forkoracle::layout::Vis::UNKNOWN,
    OffRoute,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Progress stopped and the car is not airborne.
            vis: forkoracle::layout::Vis::UNKNOWN,
    NoProgress,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Ran out of tape.
            vis: forkoracle::layout::Vis::UNKNOWN,
    TickCap,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The ENGINE ended the run inside a step: the car left the world, the
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// validator declared the replay invalid, or the declared time was
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// reached. The rows up to that point are real; nothing follows them.
            vis: forkoracle::layout::Vis::UNKNOWN,
    RunEnded,
            vis: forkoracle::layout::Vis::UNKNOWN,
}
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
#[derive(Clone, Debug)]
            vis: forkoracle::layout::Vis::UNKNOWN,
pub struct CoreCfg {
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Ticks held per action.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub k_ticks: usize,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// `tmobs` layout version: 1 = the 80 floats (LEARN's v1 policies), 2 = 100
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// floats with the G3 vehicle blocks appended.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub obs_version: u32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Episode cap in ticks. Must not exceed the container's declared time, or
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// the engine stops before the cap and the truncation looks like a crash.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub max_ticks: usize,
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    // ---- reward
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Cost per second of race time.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub c_time: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Reward per metre of NEW saturated progress.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub c_prog: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Paid once, on collecting the finish gate.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub finish_bonus: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Paid once, on an off-route or no-progress termination.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub crash_penalty: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    // ---- termination
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Off-route once |lateral| exceeds `half_width * offroute_scale +
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// offroute_margin`.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub offroute_scale: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub offroute_margin: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Consecutive ticks off the corridor before the run is cut.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub offroute_ticks: usize,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Ticks without new progress before the run is cut.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub noprog_ticks: usize,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The airborne guard: no-progress never fires while the car is this far
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// above the road or moving this fast vertically.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub air_height: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub air_vy: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    // ---- observation
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Distances ahead along the route, metres.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub lookahead: Vec<f32>,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Speed used to scale velocity features, m/s.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub v_scale: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Distance used to scale lookahead features, m.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub d_scale: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Saturate progress at the first gate still owed.
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// # Off by default, and that is a STATED TRADE, not an oversight
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The rule it implements is real: arc length is gameable, and a car that
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// cuts a corner accrues progress it did not earn. But the cap is only as
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// good as the gate detector feeding it, and on Summer 2026 - 01 that
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// detector does NOT agree with the plain oracle -- 40 tapes, the server
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// credits a checkpoint on 20 of them, and no radius, no arc-length
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// window and no plane-crossing direction reproduces that split. Which
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// geometric event the validator credits is UNKNOWN and is a task.
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// A cap driven by a detector that under-reports pins progress at the
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// first gate's arc length forever, and the policy can then never be paid
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// past the first sixth of the map -- silently, with a reward that looks
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// fine. That is a worse failure than the one the cap prevents.
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// What replaces it here: the CORRIDOR. Off-route termination fires when
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// the car leaves the road by more than its half-width plus a margin, and
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// on this map the road physically passes through every gate, so a car
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// that stays on it cannot skip one. That substitution is map-specific and
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// it is written down rather than assumed: on a map with a genuine
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// shortcut it does not hold, and the cap must come back with a detector
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// the oracle agrees with.
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// And underneath all of it, the standing rule is untouched: a result is a
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// tape the plain oracle re-simulates, so a shaping term that flatters a
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// shortcut costs signal quality and can never make a false result true.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub gate_cap: bool,
            vis: forkoracle::layout::Vis::UNKNOWN,
}
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
impl Default for CoreCfg {
            vis: forkoracle::layout::Vis::UNKNOWN,
    fn default() -> Self {
            vis: forkoracle::layout::Vis::UNKNOWN,
        CoreCfg {
            vis: forkoracle::layout::Vis::UNKNOWN,
            k_ticks: 10,
            vis: forkoracle::layout::Vis::UNKNOWN,
            obs_version: 1,
            vis: forkoracle::layout::Vis::UNKNOWN,
            max_ticks: 4000,
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
            // Break-even speed is `c_time / c_prog` = 30 m/s. Above it a step
            vis: forkoracle::layout::Vis::UNKNOWN,
            // pays; below it a step costs.
            vis: forkoracle::layout::Vis::UNKNOWN,
            //
            vis: forkoracle::layout::Vis::UNKNOWN,
            // This is the one place we deliberately depart from Linesight's
            vis: forkoracle::layout::Vis::UNKNOWN,
            // shape rather than its numbers. Theirs is
            vis: forkoracle::layout::Vis::UNKNOWN,
            // `-0.0012/ms + 0.01/m`, i.e. break-even at 120 m/s = 432 km/h, so
            vis: forkoracle::layout::Vis::UNKNOWN,
            // **every step is negative** and an episode-return objective would
            vis: forkoracle::layout::Vis::UNKNOWN,
            // reward self-termination; they buy that back with a fixed 7.000 s
            vis: forkoracle::layout::Vis::UNKNOWN,
            // horizon that makes Q an undiscounted sum over a window rather
            vis: forkoracle::layout::Vis::UNKNOWN,
            // than over the episode. We do not need that machinery: putting
            vis: forkoracle::layout::Vis::UNKNOWN,
            // break-even below any speed a moving car sustains removes the
            vis: forkoracle::layout::Vis::UNKNOWN,
            // incentive to die, and the episode return is then exactly
            vis: forkoracle::layout::Vis::UNKNOWN,
            // `c_prog * L - c_time * T` plus the bonus, which is minimised-T by
            vis: forkoracle::layout::Vis::UNKNOWN,
            // construction.
            vis: forkoracle::layout::Vis::UNKNOWN,
            c_time: 0.30,
            vis: forkoracle::layout::Vis::UNKNOWN,
            c_prog: 0.01,
            vis: forkoracle::layout::Vis::UNKNOWN,
            finish_bonus: 10.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            crash_penalty: 1.0,
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
            offroute_scale: 1.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            offroute_margin: 4.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            offroute_ticks: 40,
            vis: forkoracle::layout::Vis::UNKNOWN,
            noprog_ticks: 250,
            vis: forkoracle::layout::Vis::UNKNOWN,
            air_height: 2.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            air_vy: 3.0,
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
            lookahead: vec![5.0, 10.0, 20.0, 35.0, 55.0, 80.0, 120.0, 170.0, 240.0],
            vis: forkoracle::layout::Vis::UNKNOWN,
            v_scale: 100.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            d_scale: 100.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            gate_cap: false,
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,
}
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
/// What the caller learns about a step, beyond the reward.
            vis: forkoracle::layout::Vis::UNKNOWN,
#[derive(Clone, Copy, Debug)]
            vis: forkoracle::layout::Vis::UNKNOWN,
pub struct Info {
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The car state at the end of the step, as every arm shares it.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub state: CarState,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub tick: usize,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub race_s: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub s: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub best_s: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub gates: usize,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub speed: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub lateral: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub height: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// New saturated progress this step, metres.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub dprog: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// True if the airborne guard suppressed a no-progress cut this step.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub air_guarded: bool,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The participant's live vehicle slot changed during this step (a
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// car-switch block): the readout followed it from this step on.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub car_switched: bool,
            vis: forkoracle::layout::Vis::UNKNOWN,
}
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
impl Default for Info {
            vis: forkoracle::layout::Vis::UNKNOWN,
    fn default() -> Self {
            vis: forkoracle::layout::Vis::UNKNOWN,
        Info {
            vis: forkoracle::layout::Vis::UNKNOWN,
            state: CarState::unknown(),
            vis: forkoracle::layout::Vis::UNKNOWN,
            tick: 0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            race_s: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            s: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            best_s: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            gates: 0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            speed: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            lateral: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            height: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            dprog: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            air_guarded: false,
            vis: forkoracle::layout::Vis::UNKNOWN,
            car_switched: false,
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,
}
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
#[derive(Clone)]
            vis: forkoracle::layout::Vis::UNKNOWN,
pub struct Core {
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub cfg: CoreCfg,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub track: Arc<Track>,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub acts: ActionSpace,
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub gates: GateTracker,
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    prev: Option<Row>,
            vis: forkoracle::layout::Vis::UNKNOWN,
    cur: Row,
            vis: forkoracle::layout::Vis::UNKNOWN,
    best_s: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    last_gain_tick: usize,
            vis: forkoracle::layout::Vis::UNKNOWN,
    off_run: usize,
            vis: forkoracle::layout::Vis::UNKNOWN,
    tick: usize,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The last `tmobs::N_PREV` actions, oldest first: the observation's
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// action-history block.
            vis: forkoracle::layout::Vis::UNKNOWN,
    prev_actions: Vec<Action>,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Where the route probe was last, so arc length is tracked forward from
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// where the car IS rather than re-found on the whole polyline every tick.
            vis: forkoracle::layout::Vis::UNKNOWN,
    cur_s: f32,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Sticky: an episode that already ended stays ended.
            vis: forkoracle::layout::Vis::UNKNOWN,
    done: Option<Done>,
            vis: forkoracle::layout::Vis::UNKNOWN,
    obs_dim: usize,
            vis: forkoracle::layout::Vis::UNKNOWN,
}
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
impl Core {
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn new(cfg: CoreCfg, track: Arc<Track>, acts: ActionSpace) -> Core {
            vis: forkoracle::layout::Vis::UNKNOWN,
        let gates = GateTracker::new(12.0, 25.0);
            vis: forkoracle::layout::Vis::UNKNOWN,
        let mut c = Core {
            vis: forkoracle::layout::Vis::UNKNOWN,
            cfg,
            vis: forkoracle::layout::Vis::UNKNOWN,
            track,
            vis: forkoracle::layout::Vis::UNKNOWN,
            acts,
            vis: forkoracle::layout::Vis::UNKNOWN,
            gates,
            vis: forkoracle::layout::Vis::UNKNOWN,
            prev: None,
            vis: forkoracle::layout::Vis::UNKNOWN,
            cur: zero_row(),
            vis: forkoracle::layout::Vis::UNKNOWN,
            best_s: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            last_gain_tick: 0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            off_run: 0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            tick: 0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            prev_actions: Vec::new(),
            vis: forkoracle::layout::Vis::UNKNOWN,
            cur_s: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            done: None,
            vis: forkoracle::layout::Vis::UNKNOWN,
            obs_dim: 0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        };
            vis: forkoracle::layout::Vis::UNKNOWN,
        c.obs_dim = c.observe().len();
            vis: forkoracle::layout::Vis::UNKNOWN,
        c
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The observation width.
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Measured from `observe` itself rather than recomputed from a formula.
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The formula and the code disagreed -- 77 against 80 -- and the
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// `debug_assert_eq!` that would have caught it is compiled out of a
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// release build, so the first thing that noticed was a slice index panic
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// inside a rollout worker after the fleet had spent 45 s coming up. Two
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// statements of one fact with nothing forcing them to agree; now there is
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// one statement.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn obs_dim(&self) -> usize {
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.obs_dim
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn done(&self) -> Option<Done> {
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.done
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// End the episode from outside (the engine ended the run). Sticky, like
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// every other termination; a crash penalty is not charged for it.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn end(&mut self, why: Done) {
            vis: forkoracle::layout::Vis::UNKNOWN,
        // Overrides a softer verdict already recorded (OffRoute, NoProgress):
            vis: forkoracle::layout::Vis::UNKNOWN,
        // there is no engine left to step, and that is the fact a caller that
            vis: forkoracle::layout::Vis::UNKNOWN,
        // kept stepping past the soft cut must see.
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.done = Some(why);
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn tick(&self) -> usize {
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.tick
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn best_s(&self) -> f32 {
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.best_s
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The most recent row the core ingested, for diagnostics.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn last_row(&self) -> Row {
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.cur
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Checkpoints credited so far.
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// **The engine's own counter when the row carries it** (`Row::cps`,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// resolved from the validator's participant — gap G1), else the geometric
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// tracker. The finish increments the counter too, so on a map with n
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// gates `finished` is `cps >= n`.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn gates_hit(&self) -> usize {
            vis: forkoracle::layout::Vis::UNKNOWN,
        if self.cur.cps != u32::MAX {
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.cur.cps as usize
            vis: forkoracle::layout::Vis::UNKNOWN,
        } else {
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.gates.hit()
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Whether the row carries the engine's counter at all.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn engine_cps(&self) -> bool {
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.cur.cps != u32::MAX
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The geometric detector's own count, kept as the cross-check it is.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn geometric_gates_hit(&self) -> usize {
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.gates.hit()
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    fn is_finished(&self) -> bool {
            vis: forkoracle::layout::Vis::UNKNOWN,
        if self.cur.cps != u32::MAX {
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.track.n_gates() > 0 && self.cur.cps as usize >= self.track.n_gates()
            vis: forkoracle::layout::Vis::UNKNOWN,
        } else {
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.gates.finished(&self.track)
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The arc length progress may not exceed: the first gate still owed,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// by the engine's count when it has one.
            vis: forkoracle::layout::Vis::UNKNOWN,
    fn progress_cap(&self) -> f32 {
            vis: forkoracle::layout::Vis::UNKNOWN,
        if self.cur.cps != u32::MAX {
            vis: forkoracle::layout::Vis::UNKNOWN,
            let k = self.cur.cps as usize;
            vis: forkoracle::layout::Vis::UNKNOWN,
            if k < self.track.n_gates() { self.track.gate_s[k] } else { self.track.length() }
            vis: forkoracle::layout::Vis::UNKNOWN,
        } else {
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.gates.cap(&self.track)
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Start an episode from the state the engine is in at `row0`.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn reset(&mut self, row0: Row, tick0: usize) -> Vec<f32> {
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.prev = None;
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.cur = row0;
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.gates.reset();
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.tick = tick0;
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.last_gain_tick = tick0;
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.off_run = 0;
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.prev_actions.clear();
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.done = None;
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.cur_s = 0.0;
            vis: forkoracle::layout::Vis::UNKNOWN,
        let p = self.pos();
            vis: forkoracle::layout::Vis::UNKNOWN,
        // The first probe is windowed around the START of the route, not taken
            vis: forkoracle::layout::Vis::UNKNOWN,
        // over the whole polyline: on a route that reuses a road the global
            vis: forkoracle::layout::Vis::UNKNOWN,
        // nearest point at the start line can be the LAST leg.
            vis: forkoracle::layout::Vis::UNKNOWN,
        let pr = self.track.probe_near(p, Some(0.0));
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.cur_s = pr.s;
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.gates.observe(&self.track, p, pr.s);
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.best_s = if self.cfg.gate_cap { pr.s.min(self.progress_cap()) } else { pr.s };
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.observe()
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    fn pos(&self) -> V3 {
            vis: forkoracle::layout::Vis::UNKNOWN,
        [self.cur.x as f32, self.cur.y as f32, self.cur.z as f32]
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    fn vel(&self) -> V3 {
            vis: forkoracle::layout::Vis::UNKNOWN,
        [self.cur.vx as f32, self.cur.vy as f32, self.cur.vz as f32]
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    fn quat(&self) -> Quat {
            vis: forkoracle::layout::Vis::UNKNOWN,
        Quat(self.cur.qx as f32, self.cur.qy as f32, self.cur.qz as f32, self.cur.qw as f32)
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Fold in the per-tick rows one action produced, and score them.
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// Every tick is examined, not just the last: a gate collected in the
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// middle of a ten-tick macro is collected, and a car that left the
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// corridor and came back within one macro really did leave it.
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// `actions` are the per-TICK inputs the chunk wrote, in order; row `j`
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// is paired with `actions[min(j, last)]` for the observation's action
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// history (the last `tmobs::N_PREV` TICKS, which is what a recorded ghost
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// gives the DATA arm too).
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn ingest(&mut self, actions: &[Action], rows: &[Row]) -> (Vec<f32>, f32, Option<Done>, Info) {
            vis: forkoracle::layout::Vis::UNKNOWN,
        let mut info = Info::default();
            vis: forkoracle::layout::Vis::UNKNOWN,
        if self.done.is_some() {
            vis: forkoracle::layout::Vis::UNKNOWN,
            return (self.observe(), 0.0, self.done, info);
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,
        let mut reward = 0.0f32;
            vis: forkoracle::layout::Vis::UNKNOWN,
        let ticks = rows.len();
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
        for (j, r) in rows.iter().enumerate() {
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.prev = Some(self.cur);
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.cur = *r;
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.tick += 1;
            vis: forkoracle::layout::Vis::UNKNOWN,
            if let Some(a) = actions.get(j.min(actions.len().saturating_sub(1))) {
            vis: forkoracle::layout::Vis::UNKNOWN,
                self.push_action(*a);
            vis: forkoracle::layout::Vis::UNKNOWN,
            }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
            let p = self.pos();
            vis: forkoracle::layout::Vis::UNKNOWN,
            let pr = self.track.probe_near(p, Some(self.cur_s));
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.cur_s = pr.s;
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.gates.observe(&self.track, p, pr.s);
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
            // Saturating progress, and only ever the NEW maximum: a car that
            vis: forkoracle::layout::Vis::UNKNOWN,
            // drives back and forth over the same stretch is not paid twice,
            vis: forkoracle::layout::Vis::UNKNOWN,
            // so there is no reward to farm short of the finish.
            vis: forkoracle::layout::Vis::UNKNOWN,
            let cap = if self.cfg.gate_cap { self.progress_cap() } else { self.track.length() };
            vis: forkoracle::layout::Vis::UNKNOWN,
            let s_eff = pr.s.min(cap);
            vis: forkoracle::layout::Vis::UNKNOWN,
            if s_eff > self.best_s {
            vis: forkoracle::layout::Vis::UNKNOWN,
                info.dprog += s_eff - self.best_s;
            vis: forkoracle::layout::Vis::UNKNOWN,
                self.best_s = s_eff;
            vis: forkoracle::layout::Vis::UNKNOWN,
                self.last_gain_tick = self.tick;
            vis: forkoracle::layout::Vis::UNKNOWN,
            }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
            let limit = pr.half_width * self.cfg.offroute_scale + self.cfg.offroute_margin;
            vis: forkoracle::layout::Vis::UNKNOWN,
            if pr.lateral.abs() > limit {
            vis: forkoracle::layout::Vis::UNKNOWN,
                self.off_run += 1;
            vis: forkoracle::layout::Vis::UNKNOWN,
            } else {
            vis: forkoracle::layout::Vis::UNKNOWN,
                self.off_run = 0;
            vis: forkoracle::layout::Vis::UNKNOWN,
            }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
            let airborne = pr.height > self.cfg.air_height
            vis: forkoracle::layout::Vis::UNKNOWN,
                || (self.cur.vy as f32).abs() > self.cfg.air_vy;
            vis: forkoracle::layout::Vis::UNKNOWN,
            if airborne {
            vis: forkoracle::layout::Vis::UNKNOWN,
                info.air_guarded = true;
            vis: forkoracle::layout::Vis::UNKNOWN,
            }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
            if self.is_finished() {
            vis: forkoracle::layout::Vis::UNKNOWN,
                self.done = Some(Done::Finished);
            vis: forkoracle::layout::Vis::UNKNOWN,
            } else if self.off_run >= self.cfg.offroute_ticks {
            vis: forkoracle::layout::Vis::UNKNOWN,
                self.done = Some(Done::OffRoute);
            vis: forkoracle::layout::Vis::UNKNOWN,
            } else if !airborne && self.tick - self.last_gain_tick >= self.cfg.noprog_ticks {
            vis: forkoracle::layout::Vis::UNKNOWN,
                self.done = Some(Done::NoProgress);
            vis: forkoracle::layout::Vis::UNKNOWN,
            } else if self.tick >= self.cfg.max_ticks {
            vis: forkoracle::layout::Vis::UNKNOWN,
                self.done = Some(Done::TickCap);
            vis: forkoracle::layout::Vis::UNKNOWN,
            }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
            info.s = pr.s;
            vis: forkoracle::layout::Vis::UNKNOWN,
            info.lateral = pr.lateral;
            vis: forkoracle::layout::Vis::UNKNOWN,
            info.height = pr.height;
            vis: forkoracle::layout::Vis::UNKNOWN,
            if self.done.is_some() {
            vis: forkoracle::layout::Vis::UNKNOWN,
                break;
            vis: forkoracle::layout::Vis::UNKNOWN,
            }
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
        reward += self.cfg.c_prog * info.dprog;
            vis: forkoracle::layout::Vis::UNKNOWN,
        reward -= self.cfg.c_time * (ticks as f32) * TICK_S;
            vis: forkoracle::layout::Vis::UNKNOWN,
        match self.done {
            vis: forkoracle::layout::Vis::UNKNOWN,
            Some(Done::Finished) => reward += self.cfg.finish_bonus,
            vis: forkoracle::layout::Vis::UNKNOWN,
            Some(Done::OffRoute) | Some(Done::NoProgress) => reward -= self.cfg.crash_penalty,
            vis: forkoracle::layout::Vis::UNKNOWN,
            _ => {}
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
        info.tick = self.tick;
            vis: forkoracle::layout::Vis::UNKNOWN,
        info.state = self.state();
            vis: forkoracle::layout::Vis::UNKNOWN,
        info.race_s = self.cur.time_ms as f32 / 1000.0;
            vis: forkoracle::layout::Vis::UNKNOWN,
        info.best_s = self.best_s;
            vis: forkoracle::layout::Vis::UNKNOWN,
        info.gates = self.gates_hit();
            vis: forkoracle::layout::Vis::UNKNOWN,
        info.speed = norm(self.vel());
            vis: forkoracle::layout::Vis::UNKNOWN,
        (self.observe(), reward, self.done, info)
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The car state at the current tick, as every arm shares it
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// (`tmstate::CarState`, STATE_VERSION 1).
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// What the readout provides: race clock, position, world velocity,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// orientation (reordered to `(w, x, y, z)`), speed = |v|, angular velocity
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// differenced from the previous tick's quaternion (NaN on the first row —
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// there is no previous), `cps` and `finished` from the geometric gate
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// tracker. Everything else is UNKNOWN (NaN / u8::MAX), never zero: gear,
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// rpm, wheel contact/material/slip and turbo are gap G3 — the engine
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// computes them and the readout does not expose them yet.
            vis: forkoracle::layout::Vis::UNKNOWN,
    ///
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// `cps` from the geometric tracker is PROVISIONAL: the oracle does not
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// agree with it on Summer 2026 - 01 (RL-agentG §5.1). Gap G1 replaces it
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// with the engine's own counter in this same field.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn state(&self) -> CarState {
            vis: forkoracle::layout::Vis::UNKNOWN,
        let mut st = CarState::unknown();
            vis: forkoracle::layout::Vis::UNKNOWN,
        let r = &self.cur;
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.race_ms = r.time_ms as i32;
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.pos = [r.x as f32, r.y as f32, r.z as f32];
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.vel = [r.vx as f32, r.vy as f32, r.vz as f32];
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.quat = [r.qw as f32, r.qx as f32, r.qy as f32, r.qz as f32];
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.speed = norm(st.vel);
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.ang_vel = match self.prev {
            vis: forkoracle::layout::Vis::UNKNOWN,
            Some(pv) => tmobs::ang_vel_from_quats(
            vis: forkoracle::layout::Vis::UNKNOWN,
                [pv.qw as f32, pv.qx as f32, pv.qy as f32, pv.qz as f32],
            vis: forkoracle::layout::Vis::UNKNOWN,
                st.quat,
            vis: forkoracle::layout::Vis::UNKNOWN,
                TICK_S,
            vis: forkoracle::layout::Vis::UNKNOWN,
            ),
            vis: forkoracle::layout::Vis::UNKNOWN,
            None => [f32::NAN; 3],
            vis: forkoracle::layout::Vis::UNKNOWN,
        };
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.cps = self.gates_hit().min(u8::MAX as usize) as u8;
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.finished = self.is_finished();
            vis: forkoracle::layout::Vis::UNKNOWN,
        // G3: the live vis state (gear, rpm, wheels, turbo, car). Wheel order is
            vis: forkoracle::layout::Vis::UNKNOWN,
        // remapped from the engine's FL, FR, RR, RL to tmstate's FL, FR, RL, RR.
            vis: forkoracle::layout::Vis::UNKNOWN,
        // They are what the ghost's telemetry sample stamped `race_ms` carries
            vis: forkoracle::layout::Vis::UNKNOWN,
        // (the engine's vis state, the car one tick earlier); see tmstate's
            vis: forkoracle::layout::Vis::UNKNOWN,
        // LABEL CONVENTION and `tmenv wheels-control` (phase 0 ms measured).
            vis: forkoracle::layout::Vis::UNKNOWN,
        let v = &r.vis;
            vis: forkoracle::layout::Vis::UNKNOWN,
        if v.known {
            vis: forkoracle::layout::Vis::UNKNOWN,
            const ENGINE_TO_TMSTATE: [usize; 4] = [0, 1, 3, 2];
            vis: forkoracle::layout::Vis::UNKNOWN,
            st.gear = v.gear;
            vis: forkoracle::layout::Vis::UNKNOWN,
            st.rpm = v.rpm;
            vis: forkoracle::layout::Vis::UNKNOWN,
            for (i, k) in ENGINE_TO_TMSTATE.iter().enumerate() {
            vis: forkoracle::layout::Vis::UNKNOWN,
                st.wheel_contact[i] = v.wheel_contact[*k] as u8;
            vis: forkoracle::layout::Vis::UNKNOWN,
                st.wheel_material[i] = v.wheel_material[*k];
            vis: forkoracle::layout::Vis::UNKNOWN,
                st.wheel_slip[i] = v.wheel_slip[*k];
            vis: forkoracle::layout::Vis::UNKNOWN,
            }
            vis: forkoracle::layout::Vis::UNKNOWN,
            st.turbo = v.turbo_time;
            vis: forkoracle::layout::Vis::UNKNOWN,
            st.car = v.car;
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,
        st
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The action history the observation sees, oldest first.
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn prev_actions(&self) -> &[Action] {
            vis: forkoracle::layout::Vis::UNKNOWN,
        &self.prev_actions
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    fn push_action(&mut self, a: Action) {
            vis: forkoracle::layout::Vis::UNKNOWN,
        self.prev_actions.push(a);
            vis: forkoracle::layout::Vis::UNKNOWN,
        if self.prev_actions.len() > tmobs::N_PREV {
            vis: forkoracle::layout::Vis::UNKNOWN,
            let drop = self.prev_actions.len() - tmobs::N_PREV;
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.prev_actions.drain(0..drop);
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// The observation vector: `tmobs::observe` on this core's state. There is
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// no other observation code in the env — a recorded human sample turned
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// into a `CarState` goes through the same function and gets the same
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// floats (`tmobs` is the ONE observation function; INTERFACES.md).
            vis: forkoracle::layout::Vis::UNKNOWN,
    pub fn observe(&self) -> Vec<f32> {
            vis: forkoracle::layout::Vis::UNKNOWN,
        let o = tmobs::observe_version(self.cfg.obs_version, &self.track.geom, &self.state(), &self.prev_actions);
            vis: forkoracle::layout::Vis::UNKNOWN,
        assert!(
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.obs_dim == 0 || o.len() == self.obs_dim,
            vis: forkoracle::layout::Vis::UNKNOWN,
            "the observation is {} wide but this Core was built at {}",
            vis: forkoracle::layout::Vis::UNKNOWN,
            o.len(),
            vis: forkoracle::layout::Vis::UNKNOWN,
            self.obs_dim
            vis: forkoracle::layout::Vis::UNKNOWN,
        );
            vis: forkoracle::layout::Vis::UNKNOWN,
        o.to_vec()
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,
}
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
pub fn zero_row() -> Row {
            vis: forkoracle::layout::Vis::UNKNOWN,
    Row {
            vis: forkoracle::layout::Vis::UNKNOWN,
        time_ms: 0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        x: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        y: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        z: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        vx: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        vy: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        vz: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        qx: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        qy: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        qz: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        qw: 1.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        wetness: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
        cps: u32::MAX,
            vis: forkoracle::layout::Vis::UNKNOWN,
        vis: forkoracle::layout::Vis::UNKNOWN,
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,
}
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
#[cfg(test)]
            vis: forkoracle::layout::Vis::UNKNOWN,
mod tests {
            vis: forkoracle::layout::Vis::UNKNOWN,
    use super::*;
            vis: forkoracle::layout::Vis::UNKNOWN,
    use tmstate::{Gate, GateKind, TrackGeom};
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// A straight 400 m track along -z at y = 10, one checkpoint, a finish.
            vis: forkoracle::layout::Vis::UNKNOWN,
    fn straight_geom() -> TrackGeom {
            vis: forkoracle::layout::Vis::UNKNOWN,
        let n = 201;
            vis: forkoracle::layout::Vis::UNKNOWN,
        let gate = |k, s: f32| Gate {
            vis: forkoracle::layout::Vis::UNKNOWN,
            kind: k,
            vis: forkoracle::layout::Vis::UNKNOWN,
            centre: [0.0, 10.0, -s],
            vis: forkoracle::layout::Vis::UNKNOWN,
            normal: [0.0, 0.0, -1.0],
            vis: forkoracle::layout::Vis::UNKNOWN,
            half_width: 8.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            s,
            vis: forkoracle::layout::Vis::UNKNOWN,
            map_waypoint: u32::MAX,
            vis: forkoracle::layout::Vis::UNKNOWN,
        };
            vis: forkoracle::layout::Vis::UNKNOWN,
        TrackGeom {
            vis: forkoracle::layout::Vis::UNKNOWN,
            geom_version: 1,
            vis: forkoracle::layout::Vis::UNKNOWN,
            map_uid: "straight".into(),
            vis: forkoracle::layout::Vis::UNKNOWN,
            pts: (0..n).map(|i| [0.0, 10.0, -(2.0 * i as f32)]).collect(),
            vis: forkoracle::layout::Vis::UNKNOWN,
            half_width: vec![8.0; n],
            vis: forkoracle::layout::Vis::UNKNOWN,
            s: (0..n).map(|i| 2.0 * i as f32).collect(),
            vis: forkoracle::layout::Vis::UNKNOWN,
            gates: vec![gate(GateKind::Checkpoint, 200.0), gate(GateKind::Finish, 400.0)],
            vis: forkoracle::layout::Vis::UNKNOWN,
            spawn: [0.0, 10.0, 0.0],
            vis: forkoracle::layout::Vis::UNKNOWN,
            spawn_yaw: 0.0,
            vis: forkoracle::layout::Vis::UNKNOWN,
            source: "test".into(),
            vis: forkoracle::layout::Vis::UNKNOWN,
            legs: None,
            vis: forkoracle::layout::Vis::UNKNOWN,
            route: None,
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    fn row(t: i64, z: f64, vz: f64) -> Row {
            vis: forkoracle::layout::Vis::UNKNOWN,
        Row { time_ms: t, x: 0.3, y: 10.0, z, vx: 0.0, vy: 0.0, vz, qx: 0.0, qy: 0.0, qz: 0.0, qw: 1.0, wetness: 0.0, cps: u32::MAX }
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// THE identity the interface promises: the env's live observation IS
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// `tmobs::observe` on the `CarState` the env exposes. Bit for bit, on
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// every step of an episode, including the first (no previous row, so
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// `ang_vel` is NaN and reads as 0) and after a gate is credited.
            vis: forkoracle::layout::Vis::UNKNOWN,
    #[test]
            vis: forkoracle::layout::Vis::UNKNOWN,
    fn the_live_observation_is_tmobs_on_the_exposed_state() {
            vis: forkoracle::layout::Vis::UNKNOWN,
        let track = Arc::new(Track::from_geom(straight_geom()));
            vis: forkoracle::layout::Vis::UNKNOWN,
        let mut core = Core::new(CoreCfg { k_ticks: 10, max_ticks: 4000, ..Default::default() }, track.clone(), ActionSpace::default());
            vis: forkoracle::layout::Vis::UNKNOWN,
        let o0 = core.reset(row(-20, 0.0, 0.0), 0);
            vis: forkoracle::layout::Vis::UNKNOWN,
        let via_tmobs = tmobs::observe(&track.geom, &core.state(), core.prev_actions());
            vis: forkoracle::layout::Vis::UNKNOWN,
        assert_eq!(o0.len(), tmobs::OBS_DIM);
            vis: forkoracle::layout::Vis::UNKNOWN,
        assert!(o0.iter().zip(via_tmobs.iter()).all(|(a, b)| a.to_bits() == b.to_bits()));
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
        // 60 macros of 10 ticks at 30 m/s: 0.3 m per tick, 180 m -- then past the CP.
            vis: forkoracle::layout::Vis::UNKNOWN,
        let mut t = -20i64;
            vis: forkoracle::layout::Vis::UNKNOWN,
        let mut z = 0.0f64;
            vis: forkoracle::layout::Vis::UNKNOWN,
        for step in 0..80usize {
            vis: forkoracle::layout::Vis::UNKNOWN,
            let rows: Vec<Row> = (0..10)
            vis: forkoracle::layout::Vis::UNKNOWN,
                .map(|_| {
            vis: forkoracle::layout::Vis::UNKNOWN,
                    t += 10;
            vis: forkoracle::layout::Vis::UNKNOWN,
                    z -= 0.3;
            vis: forkoracle::layout::Vis::UNKNOWN,
                    row(t, z, -30.0)
            vis: forkoracle::layout::Vis::UNKNOWN,
                })
            vis: forkoracle::layout::Vis::UNKNOWN,
                .collect();
            vis: forkoracle::layout::Vis::UNKNOWN,
            let act = tmstate::Action { steer: ((step as i32 % 5) * 60 - 120) as i8, gas: true, brake: step % 7 == 0 };
            vis: forkoracle::layout::Vis::UNKNOWN,
            let (obs, _r, done, info) = core.ingest(&[act; 10], &rows);
            vis: forkoracle::layout::Vis::UNKNOWN,
            let st = core.state();
            vis: forkoracle::layout::Vis::UNKNOWN,
            let again = tmobs::observe(&track.geom, &st, core.prev_actions());
            vis: forkoracle::layout::Vis::UNKNOWN,
            assert!(
            vis: forkoracle::layout::Vis::UNKNOWN,
                obs.iter().zip(again.iter()).all(|(a, b)| a.to_bits() == b.to_bits()),
            vis: forkoracle::layout::Vis::UNKNOWN,
                "step {step}: the live observation and tmobs disagree"
            vis: forkoracle::layout::Vis::UNKNOWN,
            );
            vis: forkoracle::layout::Vis::UNKNOWN,
            // Info carries the same state the observation was computed from.
            vis: forkoracle::layout::Vis::UNKNOWN,
            // NaN != NaN under PartialEq; compare the rendering, which prints NaN as NaN.
            vis: forkoracle::layout::Vis::UNKNOWN,
            assert_eq!(format!("{:?}", info.state), format!("{:?}", st));
            vis: forkoracle::layout::Vis::UNKNOWN,
            assert_eq!(st.race_ms as i64, t);
            vis: forkoracle::layout::Vis::UNKNOWN,
            assert!((st.speed - 30.0).abs() < 1e-4);
            vis: forkoracle::layout::Vis::UNKNOWN,
            if done.is_some() {
            vis: forkoracle::layout::Vis::UNKNOWN,
                break;
            vis: forkoracle::layout::Vis::UNKNOWN,
            }
            vis: forkoracle::layout::Vis::UNKNOWN,
        }
            vis: forkoracle::layout::Vis::UNKNOWN,
        // The straight run crossed the checkpoint plane at 200 m: the tracker
            vis: forkoracle::layout::Vis::UNKNOWN,
        // credited it, so cps moved and with it the leg the progress is in.
            vis: forkoracle::layout::Vis::UNKNOWN,
        assert_eq!(core.state().cps, 1, "the checkpoint at 200 m was crossed");
            vis: forkoracle::layout::Vis::UNKNOWN,
        assert!(core.best_s() > 200.0);
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,

            vis: forkoracle::layout::Vis::UNKNOWN,
    /// geom.json round trip: what the DATA arm writes, the env reads back to
            vis: forkoracle::layout::Vis::UNKNOWN,
    /// the same geometry, and the observation on it is unchanged.
            vis: forkoracle::layout::Vis::UNKNOWN,
    #[test]
            vis: forkoracle::layout::Vis::UNKNOWN,
    fn geom_json_round_trips_and_observes_identically() {
            vis: forkoracle::layout::Vis::UNKNOWN,
        let g = straight_geom();
            vis: forkoracle::layout::Vis::UNKNOWN,
        let dir = std::env::temp_dir().join(format!("tmenv-geom-{}", std::process::id()));
            vis: forkoracle::layout::Vis::UNKNOWN,
        std::fs::create_dir_all(&dir).unwrap();
            vis: forkoracle::layout::Vis::UNKNOWN,
        let p = dir.join("geom.json");
            vis: forkoracle::layout::Vis::UNKNOWN,
        let a = Track::from_geom(g.clone());
            vis: forkoracle::layout::Vis::UNKNOWN,
        a.save_geom_json(&p).unwrap();
            vis: forkoracle::layout::Vis::UNKNOWN,
        let b = Track::load_geom_json(&p).unwrap();
            vis: forkoracle::layout::Vis::UNKNOWN,
        assert_eq!(*a.geom, *b.geom);
            vis: forkoracle::layout::Vis::UNKNOWN,
        let mut st = tmstate::CarState::unknown();
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.pos = [1.0, 10.5, -123.0];
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.vel = [0.0, 0.0, -40.0];
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.speed = 40.0;
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.quat = [1.0, 0.0, 0.0, 0.0];
            vis: forkoracle::layout::Vis::UNKNOWN,
        st.race_ms = 4000;
            vis: forkoracle::layout::Vis::UNKNOWN,
        let oa = tmobs::observe(&a.geom, &st, &[]);
            vis: forkoracle::layout::Vis::UNKNOWN,
        let ob = tmobs::observe(&b.geom, &st, &[]);
            vis: forkoracle::layout::Vis::UNKNOWN,
        assert!(oa.iter().zip(ob.iter()).all(|(x, y)| x.to_bits() == y.to_bits()));
            vis: forkoracle::layout::Vis::UNKNOWN,
        let _ = std::fs::remove_dir_all(&dir);
            vis: forkoracle::layout::Vis::UNKNOWN,
    }
            vis: forkoracle::layout::Vis::UNKNOWN,
}
            vis: forkoracle::layout::Vis::UNKNOWN,
