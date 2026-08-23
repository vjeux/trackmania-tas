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

/// THE STATE OBJECTIVE's two bands, and the reason they are two types.
///
/// When finish time cannot cross a valley you score the car's STATE at a place
/// instead: arm a box, score a continuous property of the car inside it. The
/// ranking that worked is
///
/// > never got there < got there, ranked by the key < got there and finished
///
/// and the failure mode it exists to prevent is a **near miss outscoring an
/// arrival**. That happened: the continuous extension below the box was
/// `-miss`, the in-box key was itself large and negative (a distance in state
/// space, -36 for the reference line), and grazing the boundary at -0.001 beat
/// every candidate that got inside. The search then sat on the edge of the gate
/// for 100 000 evaluations perfecting a miss.
///
/// The fix at the time was `-(500 + miss)`: a convention, a constant, and one
/// more number to get wrong -- the same shape as the `FINISH_BASE` this crate
/// deleted. Here the two bands are two VARIANTS. `Reached` outranks `Missed`
/// for every pair of values either can hold, there is no arithmetic between
/// them, and there is no constant to tune, exactly as there is none between a
/// finisher and a non-finisher.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GateState {
    /// The car never got inside the box (or never did so above the gate's
    /// minimum speed). `miss_m` is its closest approach in metres -- the
    /// gradient that points a search which has never once fired the gate
    /// towards it. `INFINITY` when nothing was measured at all.
    ///
    /// **A tape that finishes the map without reaching the gate lands here**,
    /// and that is the whole point of the mode: on the map this was proven on,
    /// the ordinary finishing route is a local optimum with a two-second moat
    /// around it, and every step towards the thing the search is hunting looks
    /// like a catastrophe under finish time.
    Missed { miss_m: f64 },
    /// The car was inside. `key` is the state key at the best tick; bigger is
    /// better, and it may be any finite value, positive or negative.
    Reached { key: f64 },
    /// It reached the gate AND finished the map. Now it is a time again, and
    /// it is re-validated by the plain oracle exactly like any other time.
    Finished { ms: i64 },
}

impl GateState {
    fn rank(&self) -> (u8, f64) {
        match *self {
            // a nearer miss is better, so the miss is negated: the pair is
            // ordered by `>` throughout and there is one direction in this
            // module.
            GateState::Missed { miss_m } => (0, -miss_m),
            GateState::Reached { key } => (1, key),
            GateState::Finished { ms } => (2, -(ms as f64)),
        }
    }
}

/// THE BANDS, as one function, so the rule lives in one place and can be
/// tested without a game server.
///
/// * `finish_ms` -- what the validator said, if the candidate ran to the end.
/// * `key` -- the state key, if the car was ever inside the box above its
///   minimum speed.
/// * `miss_m` -- its closest approach otherwise; `None` when nothing was
///   measured at all.
/// * `min_key` -- how good the state has to be before FINISHING counts as
///   having done the thing. `NEG_INFINITY` for no bar.
pub fn gate_outcome(
    finish_ms: Option<i64>,
    key: Option<f64>,
    miss_m: Option<f64>,
    min_key: f64,
) -> Outcome {
    Outcome::Gate(match (finish_ms, key) {
        (Some(ms), Some(k)) if k >= min_key => GateState::Finished { ms },
        // Finished, but not having done the thing: it is still only a state,
        // and on a map whose gate sits on the line everybody drives, this is
        // the branch that keeps the seed beatable.
        (_, Some(k)) => GateState::Reached { key: k },
        (_, None) => GateState::Missed { miss_m: miss_m.unwrap_or(f64::INFINITY) },
    })
}

/// What the oracle said about a candidate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// The run finished. `ms` is the engine's own millisecond -- the only
    /// number this project calls a time.
    Finish { ms: i64 },
    /// It did not finish, and this is what the state objective saw. Only ever
    /// produced by a search armed with `--gate`.
    Gate(GateState),
    /// It did not finish, and this is how far it got.
    Dnf(Progress),
}

impl Outcome {
    pub fn finish_ms(&self) -> Option<i64> {
        match self {
            Outcome::Finish { ms } => Some(*ms),
            Outcome::Gate(GateState::Finished { ms }) => Some(*ms),
            _ => None,
        }
    }

    pub fn is_finish(&self) -> bool {
        self.finish_ms().is_some()
    }

    /// Milliseconds better (negative) or worse (positive) than `other`.
    ///
    /// Defined **only between two finishers**. Metropolis acceptance is a
    /// statement in milliseconds; applying it to a difference of checkpoint
    /// ranks, arclength fractions or state keys would be a temperature in
    /// units nobody can name.
    pub fn delta_ms(&self, other: &Outcome) -> Option<i64> {
        match (self.finish_ms(), other.finish_ms()) {
            (Some(a), Some(b)) => Some(a - b),
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
    ///
    /// The gate's two bands are ordered by construction: an arrival outranks
    /// every miss, whatever the two numbers are.
    fn cmp(&self, o: &Outcome) -> std::cmp::Ordering {
        use Outcome::*;
        match (self, o) {
            (Finish { ms: a }, Finish { ms: b }) => b.cmp(a),
            (Gate(a), Gate(b)) => {
                let (ba, ka) = a.rank();
                let (bb, kb) = b.rank();
                // band first, key second, and the key comparison never sees
                // two different bands.
                ba.cmp(&bb).then_with(|| ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal))
            }
            (Dnf(a), Dnf(b)) => {
                debug_assert!(
                    a.same_ladder(b),
                    "two progress ladders compared in one search: {:?} vs {:?}",
                    a,
                    b
                );
                a.key().cmp(&b.key())
            }
            // A search either has a state objective or it does not, so these
            // never meet in one run. If they do it is a wiring mistake and not
            // a ranking question.
            _ => {
                debug_assert!(
                    false,
                    "two different objectives compared in one search: {:?} vs {:?}",
                    self, o
                );
                match (self.is_finish(), o.is_finish()) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Equal,
                }
            }
        }
    }
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Outcome::Finish { ms } => write!(f, "{}", secs(*ms)),
            Outcome::Gate(GateState::Reached { key }) => write!(f, "GATE key {:+.4}", key),
            Outcome::Gate(GateState::Finished { ms }) => write!(f, "GATE and finished, {}", secs(*ms)),
            Outcome::Gate(GateState::Missed { miss_m }) if miss_m.is_finite() => {
                write!(f, "no gate, {:.2} m away", miss_m)
            }
            Outcome::Gate(GateState::Missed { .. }) => write!(f, "no gate, never measured"),
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
        Outcome::Gate(GateState::Reached { key }) => format!("gate_{:.4}", key).replace(['.', '-'], "_"),
        Outcome::Gate(GateState::Finished { ms }) => format!("gate_{}", secs(*ms).replace('.', "_")),
        Outcome::Gate(GateState::Missed { miss_m }) => {
            format!("nogate_{:.2}m", miss_m).replace('.', "_")
        }
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

    fn reached(key: f64) -> Outcome {
        Outcome::Gate(GateState::Reached { key })
    }
    fn missed(m: f64) -> Outcome {
        Outcome::Gate(GateState::Missed { miss_m: m })
    }
    fn gatefin(ms: i64) -> Outcome {
        Outcome::Gate(GateState::Finished { ms })
    }

    /// THE regression this feature exists to make unrepresentable.
    ///
    /// With the bands flattened onto one scalar -- the shape the working
    /// version of this had, `-miss` for a non-arrival against an in-box key
    /// that is itself large and negative -- grazing the boundary at 0.001 m
    /// beat every candidate that got inside, and a search spent 100 000
    /// evaluations perfecting a miss. Here it cannot be written down.
    #[test]
    fn a_near_miss_never_outranks_an_arrival() {
        for miss in [0.0, 1e-3, 0.5, 12.0, 500.0, 1e6, f64::INFINITY] {
            for key in [-1e9, -900.0, -501.0, -500.0, -36.1, -0.001, 0.0, 1e9] {
                assert!(
                    reached(key) > missed(miss),
                    "a miss of {} m outranked an arrival scoring {}",
                    miss,
                    key
                );
            }
        }
    }

    /// And within each band the order is the obvious one, in the same
    /// direction: bigger is better.
    #[test]
    fn gate_bands_order_internally() {
        assert!(reached(1.0) > reached(0.9));
        assert!(missed(0.5) > missed(3.0), "a nearer miss must rank higher");
        assert!(missed(3.0) > missed(f64::INFINITY), "a measured miss beats no measurement");
    }

    /// A finish is still the true objective: the gate exists to give
    /// non-finishers a gradient, never to outrank one.
    #[test]
    fn no_gate_state_ever_outranks_a_finisher() {
        for key in [-1e9, 0.0, 1e9, f64::MAX] {
            assert!(
                gatefin(3_600_000) > reached(key),
                "a gate key of {} outranked a run that reached the gate AND finished",
                key
            );
        }
        assert!(gatefin(3_600_000) > missed(0.0));
        assert!(gatefin(20_237) > gatefin(20_555), "band 2 is ordered by time, sooner first");
    }

    /// THE ONE PLACE THIS CRATE'S FIRST RULE IS SUSPENDED, and it is suspended
    /// in the type rather than by a comment.
    ///
    /// In gate mode a tape that finishes the map WITHOUT reaching the gate
    /// ranks at the bottom, with every other non-arrival. That is deliberate:
    /// the finishing route is the local optimum the search is trying to leave,
    /// and under finish time every step towards the target looks like a
    /// catastrophe. A finish only outranks the state bands when it also did
    /// the thing.
    #[test]
    fn a_finish_that_missed_the_gate_ranks_with_the_misses() {
        assert!(reached(-1e9) > missed(0.0));
        assert!(gatefin(999_999_999) > reached(1e9));
    }

    /// The gate key is a MAXIMUM over ticks and aborting only removes ticks,
    /// so an aborted candidate can never displace the same candidate run to
    /// the end -- the same property the metres ladder has, and the reason a
    /// search may arm the watchdog and the gate together.
    #[test]
    fn aborting_can_only_lower_a_gate_score() {
        assert!(reached(12.0) > reached(4.0));
        assert!(reached(-100.0) > missed(0.0));
        assert!(missed(2.0) > missed(9.0));
    }

    /// THE BAR ON THE KEY, and the local optimum it exists to break.
    ///
    /// 228811's gate sits on 96 m of boost deck that all 48 runs on the
    /// leaderboard drive across. The human world record clips it with a key of
    /// 0.06 -- doing none of the thing -- and then finishes at 22.637. Without
    /// a bar that is a band-2 result, the seed is unbeatable except by a faster
    /// ordinary lap, and the state hunt is a finish-time search with extra
    /// steps: exactly the moat the mode exists to cross. Measured, not
    /// imagined: it is what the first run of this feature on that map did.
    #[test]
    fn a_finish_that_did_not_clear_the_bar_does_not_take_the_top_band() {
        // the human world record's own numbers
        let wr = gate_outcome(Some(22_637), Some(0.0604), None, 60.0);
        assert_eq!(wr, reached(0.0604), "a 0.06 key took the top band");
        // a candidate that does the thing and dies still outranks it
        assert!(gate_outcome(None, Some(70.0), None, 60.0) > wr);
        // and one that does the thing and finishes outranks everything
        let did_it = gate_outcome(Some(20_237), Some(86.8), None, 60.0);
        assert!(did_it > gate_outcome(None, Some(1e9), None, 60.0));
        // with no bar -- the default -- the seed is back on top and nothing
        // can beat it, which is the behaviour the flag exists to turn off.
        let no_bar = gate_outcome(Some(22_637), Some(0.0604), None, f64::NEG_INFINITY);
        assert!(no_bar > gate_outcome(None, Some(1e9), None, f64::NEG_INFINITY));
    }

    #[test]
    fn the_bands_come_from_the_measurement_and_nothing_else() {
        assert_eq!(gate_outcome(None, None, Some(3.5), 0.0), missed(3.5));
        assert_eq!(gate_outcome(None, None, None, 0.0), missed(f64::INFINITY));
        assert_eq!(gate_outcome(None, Some(-4.0), None, f64::NEG_INFINITY), reached(-4.0));
        assert_eq!(gate_outcome(Some(1234), None, Some(2.0), 0.0), missed(2.0));
    }

    #[test]
    fn a_gate_outcome_prints_its_band() {
        assert_eq!(reached(69.84).to_string(), "GATE key +69.8400");
        assert_eq!(missed(3.5).to_string(), "no gate, 3.50 m away");
        assert_eq!(missed(f64::INFINITY).to_string(), "no gate, never measured");
        assert_eq!(gatefin(20237).to_string(), "GATE and finished, 20.237");
        assert_eq!(tag(&reached(-36.1224)), "gate__36_1224");
        assert_eq!(tag(&gatefin(20237)), "gate_20_237");
    }
}
