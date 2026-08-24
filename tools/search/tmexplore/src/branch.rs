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
