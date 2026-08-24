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

use forkoracle::EventSeen;
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
    /// It reached the gate AND the event fired. `after` is what happened next,
    /// maximised over the ticks after it.
    ///
    /// **A place and an event are not the same shape**, which is why this is a
    /// band and not another term in the key. On 228811 the state the gate
    /// scores is worth having only because the map then fires the car from 323
    /// to 751 km/h in one contact, and what makes a launch good is where it
    /// carries the car -- a quantity the car is nowhere near when the event
    /// happens.
    Fired { after: f64 },
    /// All of that, and it finished. Now it is a time again, and it is
    /// re-validated by the plain oracle exactly like any other time.
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
            GateState::Fired { after } => (2, after),
            GateState::Finished { ms } => (3, -(ms as f64)),
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
/// * `min_key` -- how good the state has to be before the higher bands are
///   available at all. `NEG_INFINITY` for no bar.
///
/// **The bands are cumulative.** Firing without reaching the gate is not the
/// fired band, and finishing without firing is not the finished band, for the
/// same reason the bar exists: each band means "everything below it, and this
/// too". A run that finished by driving the ordinary route has not done the
/// thing, and the whole point of the mode is that the ordinary route is the
/// local optimum being escaped.
pub fn gate_outcome(
    finish_ms: Option<i64>,
    key: Option<f64>,
    miss_m: Option<f64>,
    min_key: f64,
    event: EventSeen,
) -> Outcome {
    Outcome::Gate(match key {
        None => GateState::Missed { miss_m: miss_m.unwrap_or(f64::INFINITY) },
        Some(k) if k < min_key => GateState::Reached { key: k },
        Some(k) => match (event, finish_ms) {
            (EventSeen::Fired { .. }, Some(ms)) => GateState::Finished { ms },
            (EventSeen::Fired { after, .. }, None) => GateState::Fired { after: after as f64 },
            (EventSeen::Unarmed, Some(ms)) => GateState::Finished { ms },
            _ => GateState::Reached { key: k },
        },
    })
}

/// What the oracle said about a candidate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// The run finished. `ms` is the engine's own millisecond -- the only
    /// number this project calls a time.
    ///
    /// `us` is the SUB-TICK refinement, in microseconds of race time, and it
    /// exists because on a fast map the millisecond is a thousand times too
    /// coarse to search on: at 858 km/h one millisecond is 24 cm of road, so
    /// almost every mutation is invisible to the validator and the population
    /// random-walks a plateau. Measured on 191465: 170 workers, 240 000
    /// evaluations, fifteen minutes -- the plateau value was reached in nine
    /// seconds and never moved again.
    ///
    /// It is `Some` only when the search was armed with `--plane`, and then it
    /// is the fork child's own interpolated crossing of that plane, calibrated
    /// per worker against the plain oracle's millisecond for the incumbent.
    /// **It never replaces `ms`**: the guard still requires the plain oracle to
    /// reproduce the millisecond of the written file, and a bank still records
    /// the oracle's own answer. The microsecond only ORDERS candidates the
    /// oracle cannot tell apart.
    Finish { ms: i64, us: Option<i64> },
    /// It did not finish, and this is what the state objective saw. Only ever
    /// produced by a search armed with `--gate`.
    Gate(GateState),
    /// It did not finish, and this is how far it got.
    Dnf(Progress),
}

impl Outcome {
    /// A finisher with no sub-tick refinement -- the ordinary case.
    pub fn fin(ms: i64) -> Outcome {
        Outcome::Finish { ms, us: None }
    }

    pub fn finish_ms(&self) -> Option<i64> {
        match self {
            Outcome::Finish { ms, .. } => Some(*ms),
            Outcome::Gate(GateState::Finished { ms }) => Some(*ms),
            _ => None,
        }
    }

    /// The sub-tick crossing in microseconds, when one was measured.
    pub fn finish_us(&self) -> Option<i64> {
        match self {
            Outcome::Finish { us, .. } => *us,
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

    /// The same difference in MICROseconds, when both sides carry a sub-tick
    /// measurement; otherwise the millisecond difference widened.
    ///
    /// Metropolis needs a real gradient, and with a plane armed the whole point
    /// is that the millisecond difference is zero for every candidate worth
    /// looking at.
    pub fn delta_us(&self, other: &Outcome) -> Option<i64> {
        match (self.finish_us(), other.finish_us()) {
            (Some(a), Some(b)) => Some(a - b),
            _ => self.delta_ms(other).map(|d| d * 1000),
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
            // Sooner wins. When BOTH sides carry a sub-tick crossing the
            // microsecond decides, because that is the finer measurement of the
            // same quantity; when either does not, the millisecond does, and a
            // search never mixes the two regimes within one run.
            (Finish { ms: a, us: ua }, Finish { ms: b, us: ub }) => match (ua, ub) {
                (Some(x), Some(y)) => y.cmp(x),
                _ => b.cmp(a),
            },
            (Finish { .. }, _) => std::cmp::Ordering::Greater,
            (_, Finish { .. }) => std::cmp::Ordering::Less,
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
            Outcome::Finish { ms, us: None } => write!(f, "{}", secs(*ms)),
            Outcome::Finish { ms, us: Some(u) } => write!(
                f,
                "{} (plane {}.{:06})",
                secs(*ms),
                *u / 1_000_000,
                (*u % 1_000_000).unsigned_abs()
            ),
            Outcome::Gate(GateState::Reached { key }) => write!(f, "GATE key {:+.4}", key),
            Outcome::Gate(GateState::Fired { after }) if !after.is_finite() => {
                write!(f, "GATE FIRED, but the run ENDED on the firing tick (no after-window)")
            }
            Outcome::Gate(GateState::Fired { after }) => write!(f, "GATE FIRED, after {:+.4}", after),
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
        Outcome::Finish { ms, us: None } => secs(*ms).replace('.', "_"),
        // Two candidates the oracle cannot tell apart must not overwrite each
        // other in the bank, so the sub-tick value is part of the name.
        Outcome::Finish { ms, us: Some(u) } => {
            format!("{}_u{}", secs(*ms).replace('.', "_"), u)
        }
        Outcome::Gate(GateState::Reached { key }) => format!("gate_{:.4}", key).replace(['.', '-'], "_"),
        Outcome::Gate(GateState::Fired { after }) if !after.is_finite() => "fired_noafter".into(),
        Outcome::Gate(GateState::Fired { after }) => format!("fired_{:.4}", after).replace(['.', '-'], "_"),
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
        Outcome::fin(ms)
    }
    fn finu(ms: i64, us: i64) -> Outcome {
        Outcome::Finish { ms, us: Some(us) }
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

    /// THE SUB-TICK ORDERING, and the plateau it exists to cross.
    ///
    /// Two tapes the plain oracle reports as the same millisecond are the whole
    /// problem on a fast map: 1 ms is 24 cm of road at 858 km/h, so the
    /// validator cannot see the improvement and the search random-walks. With a
    /// plane armed they are ordered by the crossing, and the millisecond is
    /// still what gets banked and still what the oracle has to reproduce.
    #[test]
    fn the_microsecond_orders_two_tapes_the_millisecond_cannot() {
        assert!(finu(13071, 13_070_100) > finu(13071, 13_070_700));
        assert_eq!(finu(13071, 13_070_100).finish_ms(), Some(13071));
        assert_eq!(finu(13071, 13_070_100).delta_us(&finu(13071, 13_070_700)), Some(-600));
        // the millisecond still decides between a measured and an unmeasured
        // one, so a plain evaluator's answer never loses to a plane's noise
        assert!(fin(13070) > finu(13071, 13_070_100));
        assert!(finu(13070, 13_069_900) > fin(13071));
        // and a plain pair is ordered exactly as before
        assert!(fin(13070) > fin(13071));
        // two candidates the oracle calls the same do not collide in the bank
        assert_ne!(tag(&finu(13071, 13_070_100)), tag(&finu(13071, 13_070_700)));
        assert!(finu(13071, 13_070_697).to_string().contains("13.070697"), "{}", finu(13071, 13_070_697));
    }

    /// A finisher still outranks every failure whatever the sub-tick value is:
    /// the refinement orders finishers and nothing else.
    #[test]
    fn a_sub_tick_finisher_is_still_a_finisher() {
        assert!(finu(3_600_000, 3_600_000_000) > cp(63, Some(0)));
        assert!(finu(13071, 13_070_100).is_finish());
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
    fn firedb(after: f64) -> Outcome {
        Outcome::Gate(GateState::Fired { after })
    }
    fn fired_ev(after: f32) -> EventSeen {
        EventSeen::Fired { tick: 2019, value: 109.3, pos: [70.0, 50.0, 709.0], after, after_tick: 2100 }
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
        let wr = gate_outcome(Some(22_637), Some(0.0604), None, 60.0, EventSeen::Unarmed);
        assert_eq!(wr, reached(0.0604), "a 0.06 key took the top band");
        // a candidate that does the thing and dies still outranks it
        assert!(gate_outcome(None, Some(70.0), None, 60.0, EventSeen::Unarmed) > wr);
        // and one that does the thing and finishes outranks everything
        let did_it = gate_outcome(Some(20_237), Some(86.8), None, 60.0, EventSeen::Unarmed);
        assert!(did_it > gate_outcome(None, Some(1e9), None, 60.0, EventSeen::Unarmed));
        // with no bar -- the default -- the seed is back on top and nothing
        // can beat it, which is the behaviour the flag exists to turn off.
        let no_bar = gate_outcome(Some(22_637), Some(0.0604), None, f64::NEG_INFINITY, EventSeen::Unarmed);
        assert!(no_bar > gate_outcome(None, Some(1e9), None, f64::NEG_INFINITY, EventSeen::Unarmed));
    }

    #[test]
    fn the_bands_come_from_the_measurement_and_nothing_else() {
        assert_eq!(gate_outcome(None, None, Some(3.5), 0.0, EventSeen::Unarmed), missed(3.5));
        assert_eq!(gate_outcome(None, None, None, 0.0, EventSeen::Unarmed), missed(f64::INFINITY));
        assert_eq!(gate_outcome(None, Some(-4.0), None, f64::NEG_INFINITY, EventSeen::Unarmed), reached(-4.0));
        assert_eq!(gate_outcome(Some(1234), None, Some(2.0), 0.0, EventSeen::Unarmed), missed(2.0));
    }


    /// THE FOURTH BAND, and the cumulative rule that makes it mean something.
    ///
    /// A place and an event are different shapes: on 228811 the state the gate
    /// scores is worth having only because the map then fires the car. So the
    /// bands are `missed < reached < fired < finished`, and each REQUIRES the
    /// one below -- a run that finishes without firing has driven the ordinary
    /// route, which is the local optimum this mode exists to escape.
    #[test]
    fn the_event_band_sits_above_the_state_and_below_a_finish() {
        let ev = fired_ev(-17.9);
        // reached, fired, finished: strictly increasing
        assert!(firedb(-17.9) > reached(1e9), "a fired run lost to a merely good state");
        assert!(firedb(-1e9) > reached(1e9), "the bands are not ordered by value across them");
        assert!(gatefin(20_237) > firedb(1e9), "a finish lost to a fired non-finisher");
        // within the band, the after-key orders it: nearer the finish is better
        assert!(firedb(-17.9) > firedb(-30.0));
        // and the whole thing comes out of the band rule
        // the after-key crosses the wire as f32, so the band carries what the
        // child measured and not a widened literal
        assert_eq!(gate_outcome(None, Some(86.8), None, 60.0, ev), firedb(-17.9f32 as f64));
        assert_eq!(gate_outcome(Some(20_237), Some(86.8), None, 60.0, ev), gatefin(20_237));
    }

    /// **AN EMPTY AFTER-WINDOW IS THE WORST MEASURED VALUE, NOT THE BEST.**
    ///
    /// Measured on 267460, 2026-08-23, and it cost a search. With a watchdog
    /// that aborts every candidate at a fixed tick -- the honest way to write
    /// "get there EARLY" when the key language cannot see a clock -- a run can
    /// fire the event on the very tick it is aborted. The window after the
    /// event is then EMPTY, and `GateReport::event` reported `after = 0` for it.
    ///
    /// Every after-key the feature documents is a **negated distance**, so it
    /// is never positive and **0 is unbeatable**. The search duly climbed
    /// `-27.93` -> `+0.0000` and stopped: the winner's firing tick was the abort
    /// tick, it had done nothing whatever after the event, and no candidate that
    /// really flew towards the target could ever outrank it. That is the
    /// `-(500 + miss)` failure of the working version, one band up.
    ///
    /// An empty window now scores negative infinity, which is what "measured
    /// nothing" has to mean when bigger is better.
    #[test]
    fn an_empty_after_window_loses_to_every_real_after_key() {
        let empty = firedb(f64::NEG_INFINITY);
        for after in [-1e9, -298.75, -27.93, -19.99, -0.005, 0.0, 1e9] {
            assert!(
                firedb(after) > empty,
                "a fired run measuring {} lost to one that measured nothing at all",
                after
            );
        }
        // it is still a `fired`, so it outranks every state and loses to a finish
        assert!(empty > reached(1e9), "an empty after-window fell out of its band");
        assert!(gatefin(20_237) > empty, "an empty after-window outranked a finish");
        // and it says so rather than printing a bare -inf
        assert!(empty.to_string().contains("ENDED on the firing tick"), "{}", empty);
        assert_eq!(tag(&empty), "fired_noafter");
    }

    /// **A FINISH THAT DID NOT FIRE IS NOT A FINISH, once an event is armed.**
    ///
    /// This is the whole reason the event is a band and not a bonus. With a
    /// launch clause armed on 228811, the human world record finishes at 22.637
    /// having fired nothing -- and if that took the top band the search would be
    /// ranking on time again from the first evaluation.
    #[test]
    fn a_finish_without_the_event_does_not_take_the_top_band() {
        let silent = EventSeen::Silent;
        assert_eq!(
            gate_outcome(Some(22_637), Some(86.8), None, 60.0, silent),
            reached(86.8),
            "a finish that fired nothing took a band above the state"
        );
        // and one candidate that DID fire, and died, outranks it
        assert!(gate_outcome(None, Some(70.0), None, 60.0, fired_ev(-99.0)) > reached(86.8));
        // with NO clause armed, the same finish is the top band, as before:
        // the gate is then the whole objective.
        assert_eq!(
            gate_outcome(Some(22_637), Some(86.8), None, 60.0, EventSeen::Unarmed),
            gatefin(22_637)
        );
    }

    /// The bar governs everything above it. A state below the bar cannot reach
    /// the fired band either, however spectacular the event: firing somewhere
    /// the search was not pointed at is not progress towards anything.
    #[test]
    fn the_bar_gates_the_event_band_too() {
        assert_eq!(gate_outcome(None, Some(0.06), None, 60.0, fired_ev(0.0)), reached(0.06));
        assert_eq!(
            gate_outcome(Some(20_237), Some(0.06), None, 60.0, fired_ev(0.0)),
            reached(0.06)
        );
    }

    #[test]
    fn the_event_band_prints_and_tags_as_itself() {
        assert_eq!(firedb(-17.9).to_string(), "GATE FIRED, after -17.9000");
        assert_eq!(tag(&firedb(-17.9)), "fired__17_9000");
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
