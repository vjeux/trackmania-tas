//! `Verdict` — what the oracle says about a tape, and how verdicts ORDER.
//!
//! This module is the contract between agent A (the oracle) and everyone who
//! consumes an answer: the explorer's archive keeps "the best entry per bin",
//! the savestate tree ranks branches, the polisher keeps an incumbent. All
//! three ask the same question — *is this one better?* — so the answer to that
//! question is defined exactly once, here.
//!
//! # THE ORDERING RULE, AND WHY IT IS A DERIVE
//!
//! **Every finisher outranks every non-finisher, by construction.**
//!
//! This project has already paid for the other way of doing it. A previous
//! search layer encoded "finished" as a magic `FINISH_BASE` sentinel added to a
//! score, so a deep enough DNF could out-score a real finish by arithmetic —
//! and it silently corrupted five maps' objectives before anyone noticed,
//! because nothing about a number saturating looks like a bug.
//!
//! So the ordering here is not written by hand and it is not arithmetic on a
//! score. [`Score`] is an enum whose `Ord` is **derived**, and Rust derives an
//! enum's `Ord` in *variant declaration order first*. `Dnf` is declared before
//! `Finish`. There is no input, no threshold and no future edit to a comparison
//! function that can make a DNF outrank a finish; you would have to physically
//! reorder the variants, in a file whose whole subject is that ordering.
//!
//! The second half is the same trick. Within a finish, a **smaller** time is
//! better, which is the opposite of what a derive would give — so the field is
//! a [`Reverse`], and the derive inverts it for us. Within a DNF, more
//! checkpoints is better, which is what the derive gives already.
//!
//! ```
//! use tmauto::verdict::Verdict;
//! // a finish, however slow, beats a DNF, however deep
//! assert!(Verdict::finish(999_999) > Verdict::Dnf { cps: 250 });
//! // faster is better
//! assert!(Verdict::finish(43_079) > Verdict::finish(43_080));
//! // deeper is better
//! assert!(Verdict::Dnf { cps: 3 } > Verdict::Dnf { cps: 2 });
//! ```

use core::cmp::Reverse;

/// The ordering key. **Do not add variants between these two.**
///
/// Derived `Ord` compares the variant index first, so everything about the
/// finisher/non-finisher rule is the declaration order below.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Score {
    /// Declared FIRST, so every `Dnf` is less than every `Finish`.
    /// `cps` ascending = better, which is what the derive does.
    Dnf { cps: u32 },
    /// Declared SECOND, so every `Finish` beats every `Dnf`.
    /// `Reverse` so that a smaller millisecond count compares GREATER.
    Finish { faster_is_greater: Reverse<u32> },
}

/// What the oracle says a tape does.
///
/// Ergonomic to match on (`Verdict::Finish { ms }`), and totally ordered by
/// *goodness* through [`Score`]. Sorting a `Vec<Verdict>` ascending puts the
/// best last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The car crossed the finish line, at `ms` milliseconds.
    Finish { ms: u32 },
    /// The car did not finish, having collected `cps` checkpoints.
    Dnf { cps: u32 },
}

impl Verdict {
    /// A finish at `ms` milliseconds.
    pub fn finish(ms: u32) -> Verdict {
        Verdict::Finish { ms }
    }

    /// The ordering key. This is the only place `Verdict` becomes comparable.
    pub fn score(self) -> Score {
        match self {
            Verdict::Dnf { cps } => Score::Dnf { cps },
            Verdict::Finish { ms } => Score::Finish { faster_is_greater: Reverse(ms) },
        }
    }

    /// The finish time, if it finished.
    pub fn ms(self) -> Option<u32> {
        match self {
            Verdict::Finish { ms } => Some(ms),
            Verdict::Dnf { .. } => None,
        }
    }

    /// Checkpoints reached. A finish reports none here — the checkpoint count
    /// of a finish is a property of the map, not of the run, and returning it
    /// would invite `cps`-based ranking that quietly ignores the finish.
    pub fn dnf_cps(self) -> Option<u32> {
        match self {
            Verdict::Dnf { cps } => Some(cps),
            Verdict::Finish { .. } => None,
        }
    }

    pub fn is_finish(self) -> bool {
        matches!(self, Verdict::Finish { .. })
    }

    /// Seconds with a decimal, the way this project writes a time.
    pub fn secs(self) -> String {
        match self {
            Verdict::Finish { ms } => format!("{}.{:03}", ms / 1000, ms % 1000),
            Verdict::Dnf { cps } => format!("DNF(cps={})", cps),
        }
    }
}

impl PartialOrd for Verdict {
    fn partial_cmp(&self, other: &Verdict) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Verdict {
    fn cmp(&self, other: &Verdict) -> core::cmp::Ordering {
        self.score().cmp(&other.score())
    }
}

/// Which oracle answered, and — for a fork answer — how far the tape was from
/// the reference the fork checkpointed on.
///
/// This rides along with every verdict because of a measured fact: **the fork
/// server is not trustworthy far from its reference.** 0 of 312 fork-reported
/// finishes survived a full re-simulation. A consumer that cannot see which
/// oracle answered cannot apply that rule, so the distance is not optional
/// decoration — it is the number that says whether the answer is inside its
/// regime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OracleSource {
    /// The dedicated server re-simulated the whole written file. Trustworthy
    /// without qualification; this is what "a result" means in this project.
    Plain,
    /// A fork server answered from a checkpointed state.
    Fork {
        /// The tick the fork resumed from.
        boundary: u32,
        /// Hash of the reference tape the fork checkpointed on.
        reference_hash: TapeHash,
        /// How far this tape is from that reference.
        distance: ForkDistance,
    },
}

impl OracleSource {
    /// Is this an answer a result may be banked on?
    ///
    /// Only a plain answer is. A fork answer is a search signal, never a
    /// result: **a result is a written file the plain oracle re-simulates.**
    pub fn is_bankable(&self) -> bool {
        matches!(self, OracleSource::Plain)
    }
}

/// How far a candidate tape is from the reference its fork checkpointed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForkDistance {
    /// The first tick at which the candidate differs from the reference, or
    /// `None` when they are identical.
    pub first_differing_tick: Option<u32>,
    /// How many ticks differ in total.
    pub differing_ticks: u32,
}

impl ForkDistance {
    /// Identical to the reference.
    pub const IDENTICAL: ForkDistance =
        ForkDistance { first_differing_tick: None, differing_ticks: 0 };

    /// Is every difference strictly after `boundary`?
    ///
    /// This is the forward-only condition. A resume cannot un-consume a record
    /// already consumed, so a tape differing at or below the boundary makes the
    /// engine run a hybrid — template inputs for the prefix, ours for the
    /// suffix — and answer honestly about a run nobody asked for.
    pub fn is_forward_only(&self, boundary: u32) -> bool {
        match self.first_differing_tick {
            None => true,
            Some(t) => t > boundary,
        }
    }
}

/// A tape's content hash. See [`crate::tape::Tape::hash`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TapeHash(pub [u8; 32]);

impl TapeHash {
    pub fn hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }
    /// Parse the 64-character hex form. `None` on anything else.
    pub fn from_hex(s: &str) -> Option<TapeHash> {
        if s.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()?;
        }
        Some(TapeHash(out))
    }
}

impl core::fmt::Debug for TapeHash {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", &self.hex()[..16])
    }
}

impl core::fmt::Display for TapeHash {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.hex())
    }
}

/// A verdict together with where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eval {
    pub verdict: Verdict,
    pub source: OracleSource,
}

impl Eval {
    pub fn plain(verdict: Verdict) -> Eval {
        Eval { verdict, source: OracleSource::Plain }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule the `FINISH_BASE` sentinel broke, stated as a test over a
    /// deliberately hostile population: the slowest imaginable finish against
    /// the deepest imaginable DNF.
    #[test]
    fn every_finisher_outranks_every_non_finisher() {
        let finishes: Vec<Verdict> =
            [0u32, 1, 43_079, 1_000_000, u32::MAX].iter().map(|&m| Verdict::finish(m)).collect();
        let dnfs: Vec<Verdict> =
            [0u32, 1, 5, 250, u32::MAX].iter().map(|&c| Verdict::Dnf { cps: c }).collect();
        for f in &finishes {
            for d in &dnfs {
                assert!(f > d, "{:?} should outrank {:?}", f, d);
            }
        }
    }

    #[test]
    fn faster_is_better_and_deeper_is_better() {
        assert!(Verdict::finish(43_079) > Verdict::finish(43_080));
        assert!(Verdict::Dnf { cps: 3 } > Verdict::Dnf { cps: 2 });
    }

    /// Sorting ascending puts the best last — the property the archive relies
    /// on when it keeps `max()` per bin.
    #[test]
    fn sort_puts_the_best_last() {
        let mut v = vec![
            Verdict::Dnf { cps: 9 },
            Verdict::finish(50_000),
            Verdict::Dnf { cps: 1 },
            Verdict::finish(40_000),
        ];
        v.sort();
        assert_eq!(v, vec![
            Verdict::Dnf { cps: 1 },
            Verdict::Dnf { cps: 9 },
            Verdict::finish(50_000),
            Verdict::finish(40_000),
        ]);
        assert_eq!(v.iter().max(), Some(&Verdict::finish(40_000)));
    }

    #[test]
    fn seconds_have_a_decimal() {
        assert_eq!(Verdict::finish(43_079).secs(), "43.079");
        assert_eq!(Verdict::finish(5_347).secs(), "5.347");
        assert_eq!(Verdict::finish(265_159).secs(), "265.159");
    }

    #[test]
    fn forward_only_is_about_the_boundary() {
        let d = ForkDistance { first_differing_tick: Some(100), differing_ticks: 7 };
        assert!(d.is_forward_only(99));
        assert!(!d.is_forward_only(100), "a difference AT the boundary is not forward-only");
        assert!(!d.is_forward_only(101));
        assert!(ForkDistance::IDENTICAL.is_forward_only(0));
    }

    #[test]
    fn only_a_plain_answer_is_bankable() {
        assert!(OracleSource::Plain.is_bankable());
        assert!(!OracleSource::Fork {
            boundary: 10,
            reference_hash: TapeHash([0; 32]),
            distance: ForkDistance::IDENTICAL,
        }
        .is_bankable());
    }

    #[test]
    fn tape_hash_hex_round_trips() {
        let h = TapeHash([0xAB; 32]);
        assert_eq!(TapeHash::from_hex(&h.hex()), Some(h));
        assert_eq!(TapeHash::from_hex("nope"), None);
    }
}
