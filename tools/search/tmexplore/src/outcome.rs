//! What a run is worth, in a type where a non-finisher cannot outrank a
//! finisher.
//!
//! # There is no constant in this file, and that is the whole design
//!
//! The defect this shape exists to make unrepresentable is `FINISH_BASE`: a
//! bare `i64` score where a finisher was `FINISH_BASE - ms` and a failure was
//! `depth * UNIT - time`. On a long enough map the failure ladder climbs past
//! the finisher band and every search on that map silently abandons finishers
//! for deep failures. It corrupted five maps' objectives, and because a null
//! result does not look like a claim, that whole class went unnoticed for
//! weeks.
//!
//! Raising the constant fixes the arithmetic and keeps the shape. Here the two
//! meanings are two *variants* and `Ord` puts every `Finished` above every
//! `Stopped` for any station count, any checkpoint count, any tick count and
//! any millisecond, because the variant rank is compared first. The constant
//! cannot be got wrong because there is no constant.
//!
//! `tests/ordering.rs` sweeps the corners rather than trusting the sentence.

/// Which oracle produced a verdict, and — for a fork answer — how far the tape
/// was from the reference the fork checkpointed on.
///
/// This is carried because **a fork answer is never a result on its own**: 0 of
/// 312 fork-reported finishes once survived full re-validation, and every one
/// of them came from a tape that was not a small perturbation of its
/// reference. A verdict that does not say which instrument produced it cannot
/// be audited later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OracleKind {
    /// The dedicated server re-simulated a written file. Authoritative.
    Plain,
    /// A fork server answered from a paused simulation.
    Fork {
        /// The tick the fork resumed at.
        boundary: u32,
        /// First tick at which this tape differs from the fork's reference,
        /// or `u32::MAX` if it never does.
        first_diff: u32,
        /// How many ticks differ from the reference in total.
        ticks_differing: u32,
    },
}

impl OracleKind {
    /// Is this verdict allowed to be *banked* as a result?
    ///
    /// Only a plain answer is. This is a method rather than a comment because
    /// the rule has been broken by accident twice.
    pub fn is_authoritative(&self) -> bool {
        matches!(self, OracleKind::Plain)
    }
}

/// The oracle's answer about a whole tape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Finish { ms: i64 },
    Dnf { cps: u32 },
}

/// How far a run got, ordered so that a finish always wins.
///
/// `Stopped` carries three numbers and they are ranked in this order:
///
/// 1. **`cps`, checkpoints collected** — ground truth, read off the map's own
///    gates by the oracle. It dominates because it is the one progress measure
///    that does not depend on our route being right.
/// 2. **`station`, furthest station on OUR OWN route** — the dense ladder.
///    Checkpoints on a campaign map are minutes apart; stations are ~20 m
///    apart, so this is what actually gives a search a gradient. It is
///    subordinate to `cps` on purpose: if a wrong route ever made station
///    disagree with checkpoints, the map's own gates win.
/// 3. **`ticks`, fewer is better** — a tie-break only, within one place on the
///    track.
///
/// Note what `station` is NOT: it is not distance from a human's line. There
/// is no human line in this project. It is arc length along a route derived
/// from the map file, and "furthest station reached" is a debuggable statement
/// about a place on the map — which is the whole point of reporting it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reached {
    Stopped { cps: u32, station: u32, ticks: u32 },
    Finished { ms: i64 },
}

impl Reached {
    /// The ordering key. The leading `u8` is the variant rank and it is
    /// compared first, so no value any later field can take will let a
    /// `Stopped` reach a `Finished`.
    fn rank(&self) -> (u8, i64, i64, i64) {
        match *self {
            Reached::Stopped { cps, station, ticks } => {
                (0, cps as i64, station as i64, -(ticks as i64))
            }
            // Within the finishing band, sooner is better. The other two slots
            // are constant, so they can never reorder two finishers.
            Reached::Finished { ms } => (1, 0, 0, -ms),
        }
    }

    pub fn finished(&self) -> bool {
        matches!(self, Reached::Finished { .. })
    }

    /// The station this outcome reached, for the furthest-station histogram.
    /// A finisher reached the end by definition; callers pass the route's
    /// station count.
    pub fn station_or(&self, end: u32) -> u32 {
        match *self {
            Reached::Stopped { station, .. } => station,
            Reached::Finished { .. } => end,
        }
    }
}

impl Ord for Reached {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}
impl PartialOrd for Reached {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for Reached {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            // Seconds with a decimal, never raw milliseconds.
            Reached::Finished { ms } => {
                write!(f, "FINISH {}.{:03}", ms / 1000, (ms % 1000).abs())
            }
            Reached::Stopped { cps, station, ticks } => write!(
                f,
                "stopped at station {} (cps {}, {}.{:03} elapsed)",
                station,
                cps,
                (ticks as i64 * 10) / 1000,
                ((ticks as i64 * 10) % 1000)
            ),
        }
    }
}
