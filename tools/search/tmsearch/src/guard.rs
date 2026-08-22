//! THE GUARD: nothing leaves the search as a result until the plain oracle
//! re-simulates the written tape and agrees.
//!
//! # Why this type exists instead of a function
//!
//! Four separate defects have made this search report a time for a tape that
//! does not achieve it -- a *phantom*. They were not one bug:
//!
//! 1. **The resume-boundary no-op.** A fork resume rewrites the engine's
//!    already-decoded input records from tick `from` onward, but a record the
//!    engine has ALREADY CONSUMED cannot be un-consumed: the rewrite is a
//!    silent no-op. `from` was calibrated once in the master while each
//!    worker's server stops where it stops (135 of 150 workers stopped past the
//!    calibration in one real run). The mutation is invisible to the evaluator
//!    and present in the written file; it scores exactly the incumbent's score,
//!    `delta == 0` is accepted, and that worker's lineage is contaminated for
//!    free.
//! 2. **A shared root.** Two searches with worker directories named by index
//!    validate each other's candidates.
//! 3. **A per-worker tick label.** The same tape read 13.080 95 and 13.070 95
//!    on two workers of one run.
//! 4. **A sub-tick surrogate used as a score.** Exact for the seed it was
//!    calibrated on and wrong by ~19 ms for anything else.
//!
//! Each has its own fix and each fix is in this crate. **The guard is the only
//! defence that does not care which defect it is**, including a fifth nobody
//! has found: it takes the bytes that were actually written, hands them to the
//! authoritative oracle, and compares. About 0.1 s per improvement.
//!
//! So it is not a function you can forget to call. [`Bank`] owns the output
//! directory, and the only way to put a file in it is [`Bank::offer`], which
//! validates first. A caller cannot bank an unconfirmed time, because there is
//! no method that does that.

use forkoracle::inputs::{Distance, Inputs};
use crate::score::{tag, Outcome};
use crate::tape::Patcher;
use ghost::secs;
use ghost::oracle::{validate, MapsMode};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Where a candidate's score came from, carried with the result so a reader can
/// see which regime it was measured in.
#[derive(Clone, Debug)]
pub struct Provenance {
    /// `true` if the score came from a mid-simulation fork rather than a full
    /// simulation.
    pub from_fork: bool,
    /// The tick the fork resumed from, if any.
    pub resume_tick: Option<usize>,
    /// How far this tape is from the reference the fork checkpointed on.
    ///
    /// **This is the number that decides whether a fork answer means
    /// anything**: 0 of 312 fork-reported finishes survived a plain
    /// re-validation when the tape was not a small, late perturbation of its
    /// reference.
    pub distance: Distance,
}

impl std::fmt::Display for Provenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.from_fork {
            write!(
                f,
                "fork resume at tick {}, {}",
                self.resume_tick.map(|t| t.to_string()).unwrap_or_else(|| "?".into()),
                self.distance
            )
        } else {
            write!(f, "full simulation, {}", self.distance)
        }
    }
}

/// A result that survived the plain oracle.
#[derive(Clone, Debug)]
pub struct Banked {
    pub path: PathBuf,
    /// The oracle's own answer for the written bytes. Not the search's claim.
    pub confirmed: Outcome,
}

/// A claim the plain oracle refused.
#[derive(Clone, Debug)]
pub struct Phantom {
    pub path: PathBuf,
    pub claimed: Outcome,
    /// What the oracle said instead, or `None` when it could not answer at all.
    /// An oracle that cannot answer is not permission to bank: the guard fails
    /// CLOSED.
    pub actual: Option<Outcome>,
}

impl std::fmt::Display for Phantom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.actual {
            Some(a) => write!(
                f,
                "PHANTOM: the search claimed {} and the plain oracle says {} for the written tape ({})",
                self.claimed,
                a,
                self.path.display()
            ),
            None => write!(
                f,
                "REFUSED: the plain oracle could not answer for the written tape, so {} is not \
                 a result ({})",
                self.claimed,
                self.path.display()
            ),
        }
    }
}

pub struct Bank {
    dir: PathBuf,
    server: PathBuf,
    map: PathBuf,
    scratch: PathBuf,
    /// A validator working directory nobody else uses.
    ///
    /// `ghost::oracle` keys its scratch on `(pid, tag)`, and it wipes that
    /// directory on every call -- so two banks in one process sharing a tag
    /// validate each other's tapes and report each other's times. That is the
    /// shared-directory defect that produced fabricated improvements in the
    /// first place, and it turned up again in this crate's own test suite the
    /// day it was written. One tag per bank, always.
    tag: String,
    log: Option<std::fs::File>,
    pub confirmed: u64,
    pub phantoms: u64,
}

impl Bank {
    pub fn new(dir: &Path, server: &Path, map: &Path, log: Option<&Path>) -> Result<Bank, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
        let scratch =
            std::env::temp_dir().join(format!("tmsearch-bank-{}-{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {}", scratch.display(), e))?;
        let log = match log {
            Some(p) => Some(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)
                    .map_err(|e| format!("{}: {}", p.display(), e))?,
            ),
            None => None,
        };
        Ok(Bank {
            dir: dir.to_path_buf(),
            server: server.to_path_buf(),
            map: map.to_path_buf(),
            scratch,
            tag: format!("guard{}", id),
            log,
            confirmed: 0,
            phantoms: 0,
        })
    }

    /// Offer a candidate as the new incumbent.
    ///
    /// Writes the tape, asks the plain oracle what THAT FILE does, and only
    /// then puts it in the bank. A disagreement is preserved as
    /// `PHANTOM_*.Ghost.Gbx` next to the log line that describes it, and
    /// returned as an error: the caller must roll the incumbent back.
    pub fn offer(
        &mut self,
        p: &Patcher,
        inputs: &Inputs,
        claimed: Outcome,
        prov: &Provenance,
    ) -> Result<Banked, Phantom> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let tmp = self.scratch.join(format!("offer_{}.Ghost.Gbx", stamp));
        let bytes = p.file(inputs);
        std::fs::write(&tmp, &bytes).expect("write the offered tape");

        let actual = match validate(&self.server, &tmp, MapsMode::One(&self.map), &self.tag) {
            Ok(r) => match r.time_ms {
                Some(ms) => Outcome::Finish { ms },
                None => Outcome::Dnf(crate::score::Progress::Checkpoints {
                    cps: r.cps.unwrap_or(0),
                    seg_ms: None,
                }),
            },
            Err(e) => {
                // An oracle that cannot answer is not permission to bank. The
                // guard fails CLOSED.
                let path = self.dir.join(format!("PHANTOM_unvalidated_{}.Ghost.Gbx", stamp));
                let _ = std::fs::rename(&tmp, &path);
                self.phantoms += 1;
                self.note(&format!(
                    "{{\"phantom\":true,\"reason\":\"the oracle did not answer: {}\",\
                     \"claimed\":\"{}\",\"file\":\"{}\"}}",
                    e.replace('"', "'"),
                    claimed,
                    path.display()
                ));
                return Err(Phantom { path, claimed, actual: None });
            }
        };

        let agrees = match (claimed, actual) {
            (Outcome::Finish { ms: a }, Outcome::Finish { ms: b }) => a == b,
            // A DNF incumbent claims no time, so there is nothing for the
            // oracle to contradict -- but it must still not FINISH, or the
            // search is scoring on something unrelated to the file.
            (Outcome::Dnf(_), Outcome::Dnf(_)) => true,
            _ => false,
        };

        if !agrees {
            let path = self
                .dir
                .join(format!("PHANTOM_{}_{}.Ghost.Gbx", tag(&claimed), stamp));
            let _ = std::fs::rename(&tmp, &path);
            self.phantoms += 1;
            let ph = Phantom { path: path.clone(), claimed, actual: Some(actual) };
            self.note(&format!(
                "{{\"phantom\":true,\"claimed\":\"{}\",\"actual\":\"{}\",\"provenance\":\"{}\",\"file\":\"{}\"}}",
                claimed,
                actual,
                prov,
                path.display()
            ));
            eprintln!("{}", ph);
            return Err(ph);
        }

        let path = self.dir.join(format!("best_{}.Ghost.Gbx", tag(&actual)));
        let _ = std::fs::rename(&tmp, &path);
        self.confirmed += 1;
        self.note(&format!(
            "{{\"confirmed\":\"{}\",\"provenance\":\"{}\",\"file\":\"{}\"}}",
            actual,
            prov,
            path.display()
        ));
        Ok(Banked { path, confirmed: actual })
    }

    fn note(&mut self, line: &str) {
        if let Some(f) = self.log.as_mut() {
            let _ = writeln!(f, "{}", line);
            let _ = f.flush();
        }
    }

    /// What to print when the search ends.
    pub fn summary(&self) -> String {
        format!(
            "{} improvement(s) confirmed by the plain oracle, {} refused as phantoms",
            self.confirmed, self.phantoms
        )
    }
}

/// Format a claim/actual pair for a human, in seconds.
pub fn disagreement(claimed_ms: i64, actual_ms: i64) -> String {
    format!(
        "claimed {}, actually {} ({})",
        secs(claimed_ms),
        secs(actual_ms),
        crate::report::delta(actual_ms - claimed_ms)
    )
}
