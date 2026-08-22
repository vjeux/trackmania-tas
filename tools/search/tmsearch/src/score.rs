//! What a candidate is worth, and the one ordering the search uses.
//!
//! # The defect this module is shaped to make unrepresentable
//!
//! The old score was a bare `i64` with `FINISH_BASE - ms` for a finisher and
//! `cps * SEG_UNIT - cp_time` for a failure, and `FINISH_BASE` was `1e8` with
//! `SEG_UNIT` `1e7`. On an eleven-checkpoint map a DNF at checkpoint 11 scores
//! `1.05e8` and **beats a finishing 96.281** (`9.99e7`). Every search on a
//! long map therefore abandoned finishers for deep failures, silently, and it
//! looked like progress. The same collision made a phantom guard whose test was
//! `score > FINISH_BASE / 2` fire on a six-checkpoint DNF and abort healthy
//! runs with a negative `want`.
//!
//! Raising the constant fixes the arithmetic and leaves the shape: two
//! meanings in one integer, ordered by luck. Here the two meanings are two
//! variants, and `Ord` puts **every** finisher above **every** non-finisher by
//! construction, for any checkpoint count, any map length and any progress
//! measure. The constant cannot be got wrong because there is no constant.

use ghost::secs;

/// How far a run that did not finish got. Two ladders, because the two
/// evaluators can see two different things.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Progress {
    /// The plain oracle's ladder: checkpoints collected, and -- if a segment
    /// map for that depth was supplied -- the exact time to reach it.
    ///
    /// On many maps this ladder has ONE usable rung: a failure comes back
    /// either "reached some checkpoints (2)" or the information-free "wrong
    /// simu", so `cps` alone is nearly binary. That is what the segment maps
    /// and the metres ladder are for.
    Checkpoints { cps: u32, seg_ms: Option<i64> },
    /// The fork evaluator's ladder: metres of arclength along the reference
    /// line, out of the line's total length.
    ///
    /// A maximum over ticks, computed identically whether the candidate was
    /// aborted early or ran to the end -- so arming the watchdog can only
    /// LOWER a score and a dead candidate can never displace a live one.
    Metres { m: f32, of: f32 },
}

impl Progress {
    /// Rank within the ladder. Only ever compared against another value of the
    /// same variant; see [`Outcome::cmp`].
    fn key(&self) -> i64 {
        match *self {
            // deeper first, then sooner within a depth. `seg_ms` is bounded by
            // any real race, so the depth term always dominates.
            Progress::Checkpoints { cps, seg_ms } => {
                (cps as i64) * 100_000_000 - seg_ms.unwrap_or(50_000_000)
            }
            Progress::Metres { m, of } => {
                let f = if of > 0.0 { (m / of).clamp(0.0, 1.0) } else { 0.0 };
                (f as f64 * 1e9) as i64
            }
        }
    }

    fn same_ladder(&self, o: &Progress) -> bool {
        matches!(
            (self, o),
            (Progress::Checkpoints { .. }, Progress::Checkpoints { .. })
                | (Progress::Metres { .. }, Progress::Metres { .. })
        )
    }
}

/// What the oracle said about a candidate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// The run finished. `ms` is the engine's own millisecond -- the only
    /// number this project calls a time.
    Finish { ms: i64 },
    /// It did not finish, and this is how far it got.
    Dnf(Progress),
}

impl Outcome {
    pub fn finish_ms(&self) -> Option<i64> {
        match self {
            Outcome::Finish { ms } => Some(*ms),
            Outcome::Dnf(_) => None,
        }
    }

    pub fn is_finish(&self) -> bool {
        matches!(self, Outcome::Finish { .. })
    }

    /// Milliseconds better (negative) or worse (positive) than `other`.
    ///
    /// Defined **only between two finishers**. Metropolis acceptance is a
    /// statement in milliseconds; applying it to a difference of checkpoint
    /// ranks or arclength fractions would be a temperature in units nobody can
    /// name.
    pub fn delta_ms(&self, other: &Outcome) -> Option<i64> {
        match (self, other) {
            (Outcome::Finish { ms: a }, Outcome::Finish { ms: b }) => Some(a - b),
            _ => None,
        }
    }
}

impl Eq for Outcome {}

impl PartialOrd for Outcome {
    fn partial_cmp(&self, o: &Outcome) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Outcome {
    /// A finisher always outranks a non-finisher. Among finishers, sooner
    /// wins. Among failures, further wins -- within one ladder.
    fn cmp(&self, o: &Outcome) -> std::cmp::Ordering {
        use Outcome::*;
        match (self, o) {
            (Finish { ms: a }, Finish { ms: b }) => b.cmp(a),
            (Finish { .. }, Dnf(_)) => std::cmp::Ordering::Greater,
            (Dnf(_), Finish { .. }) => std::cmp::Ordering::Less,
            (Dnf(a), Dnf(b)) => {
                debug_assert!(
                    a.same_ladder(b),
                    "two progress ladders compared in one search: {:?} vs {:?}",
                    a,
                    b
                );
                a.key().cmp(&b.key())
            }
        }
    }
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Outcome::Finish { ms } => write!(f, "{}", secs(*ms)),
            Outcome::Dnf(Progress::Checkpoints { cps, seg_ms: Some(t) }) => {
                write!(f, "DNF cp{} at {}", cps, secs(*t))
            }
            Outcome::Dnf(Progress::Checkpoints { cps, seg_ms: None }) => write!(f, "DNF cp{}", cps),
            Outcome::Dnf(Progress::Metres { m, of }) => {
                write!(f, "DNF {:.0} m of {:.0} ({:.0}%)", m, of, 100.0 * m / of.max(1.0))
            }
        }
    }
}

/// A filename tag for a banked candidate. Seconds, with the decimal point
/// replaced so it is still one path component: `36.049` -> `36_049`.
pub fn tag(o: &Outcome) -> String {
    match o {
        Outcome::Finish { ms } => secs(*ms).replace('.', "_"),
        Outcome::Dnf(Progress::Checkpoints { cps, .. }) => format!("dnf_cp{}", cps),
        Outcome::Dnf(Progress::Metres { m, .. }) => format!("dnf_{:.0}m", m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fin(ms: i64) -> Outcome {
        Outcome::Finish { ms }
    }
    fn cp(cps: u32, seg: Option<i64>) -> Outcome {
        Outcome::Dnf(Progress::Checkpoints { cps, seg_ms: seg })
    }

    /// THE regression. A deep DNF must never outrank a finisher, at any depth.
    /// With the old scalar this failed from eleven checkpoints upward.
    #[test]
    fn no_dnf_ever_outranks_a_finisher() {
        // a slow finisher: an hour
        let slow = fin(3_600_000);
        for cps in 0..64u32 {
            for seg in [None, Some(0), Some(1), Some(3_600_000)] {
                assert!(
                    slow > cp(cps, seg),
                    "DNF at cp{} (seg {:?}) outranked a finisher",
                    cps,
                    seg
                );
            }
        }
    }

    #[test]
    fn finishers_order_by_time_and_failures_by_depth() {
        assert!(fin(22963) > fin(22971));
        assert!(cp(3, None) > cp(2, None));
        assert!(cp(3, Some(9000)) > cp(3, Some(9001)));
        let a = Outcome::Dnf(Progress::Metres { m: 900.0, of: 1647.0 });
        let b = Outcome::Dnf(Progress::Metres { m: 899.0, of: 1647.0 });
        assert!(a > b);
    }

    /// Progress is a maximum over ticks and aborting only removes ticks, so an
    /// aborted candidate's score is <= the same candidate's unaborted score.
    /// The ordering must respect that with no special case.
    #[test]
    fn aborting_can_only_lower_a_metres_score() {
        let full = Outcome::Dnf(Progress::Metres { m: 1200.0, of: 1647.0 });
        let cut = Outcome::Dnf(Progress::Metres { m: 800.0, of: 1647.0 });
        assert!(full > cut);
        assert!(fin(99_999_999) > full);
    }

    #[test]
    fn delta_is_milliseconds_only_between_finishers() {
        assert_eq!(fin(22963).delta_ms(&fin(22971)), Some(-8));
        assert_eq!(fin(22963).delta_ms(&cp(2, None)), None);
        assert_eq!(cp(2, None).delta_ms(&cp(3, None)), None);
    }

    #[test]
    fn everything_prints_as_seconds() {
        assert_eq!(fin(36049).to_string(), "36.049");
        assert_eq!(tag(&fin(36049)), "36_049");
        assert!(!fin(36049).to_string().contains("36049"));
    }
}
