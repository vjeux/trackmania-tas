//! What the explorer needs from the map, and what it needs from the
//! simulator — as two traits, so it can be built and tested before either
//! real one exists.
//!
//! `Route` is agent B's interface. `Branch` is agent D's.
//!
//! # The one design decision in this file
//!
//! Agent D is measuring whether a fork child can stay alive and serve as a new
//! fork point. Yes gives a savestate *tree* (~6 min per search pass); no gives
//! prefix re-simulation (~37 min). **The archive must not care which**, and the
//! way it does not care is [`Branch::open`]:
//!
//! ```text
//! open(prefix, live) -> Handle
//! ```
//!
//! The explorer always hands over the *whole committed input prefix* and, as a
//! hint, the handle it believes is already parked at the end of it. A backend
//! with a live savestate tree honours the hint and forks in 10 ms; a backend
//! without one ignores it and re-simulates the prefix. The caller's code is
//! identical, the results are identical, and only the clock changes.
//!
//! That also means an archive entry stays usable after its live handle dies —
//! which it will, because a fork fleet is a fleet of processes.

use crate::action::Input;
use crate::outcome::Verdict;

/// The whole car state at one tick, which is what the archive bins on.
///
/// "Whole" is load-bearing and was paid for on a real map: position and
/// velocity together were not enough — a launcher there triggers on which way
/// the car is *pointing*, and two states identical in every metre and every
/// metre per second have to be distinguishable. Hence the quaternion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarState {
    /// Ticks consumed. The state holds at the END of tick `tick - 1`, i.e.
    /// after `tick` ticks of input have been applied.
    pub tick: u32,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    /// Attitude. Identity is `[1, 0, 0, 0]` (w first).
    pub quat: [f32; 4],
    /// Per-wheel ground contact, one bit per wheel, bit 0 = front-left.
    ///
    /// A pattern, not a count: three wheels down on one side is a different
    /// car from three wheels down on the other.
    pub wheels: u8,
    /// Consecutive ticks with no wheel in contact, saturating.
    ///
    /// This is in the state — and in the bin key — because **a launch has no
    /// arc-length progress and a naive progress metric scores it as
    /// "stopped"**. Long airborne stretches are exactly where the prior
    /// attempt's car never went: the human on that map leaves the hill at
    /// 82 m/s and flies 259 m, and nothing in a progress-along-the-line
    /// objective can tell that from a car that has come to rest.
    pub airtime: u16,
    /// Checkpoints collected so far, from the map's own gates.
    pub cps: u32,
}

impl CarState {
    pub fn speed(&self) -> f32 {
        (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1] + self.vel[2] * self.vel[2]).sqrt()
    }
    pub fn airborne(&self) -> bool {
        self.wheels == 0
    }
    /// Heading of the car's own forward axis in the world XZ plane, radians in
    /// (−π, π]. Derived from the quaternion, not from the velocity, so that a
    /// car sliding sideways and a car driving straight are different states.
    pub fn yaw(&self) -> f32 {
        let [w, x, y, z] = self.quat;
        // forward = q * (0,0,1) * q^-1  (the game's forward axis is +z)
        let fx = 2.0 * (x * z + w * y);
        let fz = 1.0 - 2.0 * (x * x + y * y);
        fz.atan2(fx)
    }
}

/// Where the car is along the route.
///
/// The route answers three easy questions — which way is forward, are we still
/// on the track, and where are the checkpoints — and it never has to be good
/// enough to *drive on*. That is the change from the prior attempt, where the
/// computed line was the thing the car had to track and line accuracy became
/// the whole game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    /// Arc length along the route, metres.
    pub s: f32,
    /// Signed distance from the route centreline, metres. Positive is right.
    pub lateral: f32,
    /// Inside the corridor at this arc length.
    pub on_route: bool,
}

/// Agent B's interface.
pub trait Route: Send + Sync {
    fn progress(&self, pos: [f32; 3]) -> Progress;

    /// Progress, matched in a window around `hint`. See [`ANCHOR`].
    ///
    /// The default ignores the hint, which is correct for a route that never
    /// approaches itself and wrong for every real map.
    fn progress_from(&self, pos: [f32; 3], hint: u32) -> (Progress, u32) {
        let _ = hint;
        (self.progress(pos), 0)
    }
    /// Total route length, metres.
    fn length(&self) -> f32;
    /// Station spacing, metres. ~20 m.
    fn spacing(&self) -> f32;
    /// How many checkpoints the map has, finish excluded.
    fn n_checkpoints(&self) -> u32;

    fn n_stations(&self) -> u32 {
        (self.length() / self.spacing()).ceil() as u32 + 1
    }
    fn station_of(&self, s: f32) -> u32 {
        if s <= 0.0 {
            0
        } else {
            (s / self.spacing()) as u32
        }
    }
}

/// A handle on a paused simulation. Opaque; the backend owns the meaning.
pub type Handle = u64;

#[derive(Clone, Debug, PartialEq)]
pub enum BranchErr {
    /// The forward-only rule, enforced in the type rather than in discipline:
    /// **a record the engine has already consumed cannot be un-consumed, and
    /// rewriting it is a silent no-op.** Every one of the 312 false finishes
    /// came from a tape that differed from its reference below the fork
    /// boundary. So a write at or below the handle's probed boundary is an
    /// error, not a clamp — a clamp silently changes what you searched.
    BelowBoundary { asked: u32, boundary: u32 },
    /// The handle is gone (process died, fleet recycled it). Recoverable: the
    /// caller re-opens from the prefix.
    Stale,
    Other(String),
}

/// What one macro advance produced.
#[derive(Clone, Debug)]
pub struct Advance {
    /// One entry per tick consumed, in order. `trace[i]` is the state after
    /// `from_tick + i + 1` ticks of input.
    pub trace: Vec<CarState>,
    /// The handle parked at the end, if the backend can keep one alive.
    /// `None` is not a failure — it is the prefix-re-simulation world.
    pub handle: Option<Handle>,
    /// Set when the run ENDED inside this macro (crossed the finish, or the
    /// simulation stopped). A fork answer, so never bankable on its own.
    pub ended: Option<Verdict>,
}

/// Agent D's interface.
pub trait Branch {
    /// Park a simulation at the end of `prefix`.
    ///
    /// `live` is a *hint*: the handle the caller believes is already there. A
    /// backend with a savestate tree forks from it; one without ignores it and
    /// re-simulates. Ignoring the hint must never change the answer.
    fn open(&mut self, prefix: &[Input], live: Option<Handle>) -> Result<Handle, BranchErr>;

    /// Consume `inputs` from `from_tick`.
    ///
    /// `from_tick` is passed explicitly so the forward-only rule is checkable
    /// at the call rather than assumed.
    fn advance(
        &mut self,
        h: Handle,
        from_tick: u32,
        inputs: &[Input],
    ) -> Result<Advance, BranchErr>;

    /// Release a handle. Idempotent.
    fn close(&mut self, h: Handle);

    /// The car before any input. Needed to seed the archive's root bin.
    fn initial_state(&mut self) -> Result<CarState, BranchErr>;

    /// How many ticks of input the map's own time limit allows.
    fn tick_limit(&self) -> u32;
}

/// Agent A's interface: write the tape into a container and have the dedicated
/// server re-simulate the written file.
///
/// **This is the only thing in the system that can produce a result.** A fork
/// answer is a hypothesis; this is the verdict. It is a trait with one method
/// because that is the entire contract, and because a stub of it is what lets
/// the explorer be tested end to end with no engine.
pub trait PlainOracle {
    /// Re-simulate `tape` and return the server's own answer.
    fn confirm(&self, tape: &[Input]) -> Result<Verdict, String>;
}

/// **Progress must saturate before an uncollected required gate.**
///
/// Arc length is gameable, and a frontier ordered on arc length will game it:
/// a car that cuts a corner accrues `s` without passing the checkpoint, and
/// that is the *cheapest* way to score well, so the search will find it. The
/// symptom is a run that makes beautiful progress down the track and never
/// collects a checkpoint.
///
/// The cure is not a penalty term — a penalty is a number to tune, and the
/// search will trade against it. It is a **cap**: a car that has not collected
/// gate `g` cannot score past gate `g`'s station, however far down the map it
/// flies. Nothing to tune, and the ordering stays a partial order over places
/// on the track.
///
/// (The TAS-community lineage reached the same fix from the other direction:
/// Linesight refuses to advance its progress index past a checkpoint unless
/// the car came within 12 m of it.)
pub struct GateLadder {
    /// Tour order: arc length and world position of each required gate, the
    /// finish last.
    pub gates: Vec<(f32, [f32; 3])>,
    /// How close counts as collected, metres.
    pub radius: f32,
}

impl GateLadder {
    pub fn empty() -> GateLadder {
        GateLadder { gates: Vec::new(), radius: 8.0 }
    }

    /// Update the collected count for a car at `pos`, and return the arc
    /// length it is allowed to be credited with.
    ///
    /// `on_route` is required as well as proximity, and the radius is tight,
    /// because **"near the checkpoint" is not "through the checkpoint"**. At
    /// 16 m the search collected gate 0 geometrically on tapes the plain
    /// oracle called `Dnf cps 0`: the car passed beside the gate, the cap
    /// lifted, and the frontier wandered on down a route it had not earned.
    /// That is E's corner-cut failure wearing a loose tolerance instead of no
    /// gate at all.
    ///
    /// A loose ladder is worse than no ladder: it MOVES THE CAP, so it does
    /// not merely miscount, it unlocks progress the run did not make.
    pub fn saturate(&self, collected: &mut u32, pos: [f32; 3], s: f32, on_route: bool) -> f32 {
        if on_route {
            if let Some(&(_, g)) = self.gates.get(*collected as usize) {
                let d2 = (pos[0] - g[0]).powi(2) + (pos[1] - g[1]).powi(2) + (pos[2] - g[2]).powi(2);
                if d2 <= self.radius * self.radius {
                    *collected += 1;
                }
            }
        }
        match self.gates.get(*collected as usize) {
            Some(&(gs, _)) => s.min(gs),
            None => s,
        }
    }
}

/// **A route that comes back near itself makes nearest-vertex progress a lie.**
///
/// Measured, on *Summer 2026 - 01*: the spawn at (1584, 16, 784) is nearest to
/// a route vertex at **s = 1483 m — station 74 of 97 — so the car scored 74/97
/// before it had moved at all.** The archive's best entry was the root: station
/// 74, zero ticks, zero gates. Every rollout was correctly capped at the first
/// gate while the headline number came from a parked car.
///
/// The cure is the one the predicate work already paid for: **an argmin over a
/// small window around where the car was last, not a global one.** A run walks
/// the route, so its index walks with it; only the first sample anchors
/// globally.
///
/// `hint` is the vertex index the previous sample matched, or
/// [`ANCHOR`] to search the whole route.
pub const ANCHOR: u32 = u32::MAX;

/// The window searched around the hint, **in metres of arc length, not in
/// vertices**.
///
/// Vertices was the obvious spelling and it was wrong: this map's route is 111
/// vertices over 1900 m, so a 400-vertex window is 6.8 km — the whole route,
/// i.e. a global argmin wearing a window's name. The parked car went on
/// scoring 1483 m, capped to the first gate, and the archive's best entry was
/// still a car that had not moved.
///
/// Back a little, because a car can be pushed backwards; forward far enough
/// that one TICK of travel cannot outrun it (a tick at 90 m/s is 0.9 m).
pub const CURSOR_BACK_M: f32 = 30.0;
pub const CURSOR_AHEAD_M: f32 = 250.0;
