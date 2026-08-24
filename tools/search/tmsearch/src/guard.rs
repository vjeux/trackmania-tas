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
use forkoracle::pred::GateRecord;
use crate::score::{tag, GateState, Outcome};
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
    /// The car's whole state where the state objective scored it, when the
    /// search was armed with a gate and this candidate reached it.
    ///
    /// A band-0 or band-1 result is a STATE, not a time, so there is no
    /// millisecond for the oracle to contradict. This is what makes the claim
    /// checkable by hand instead: it is written out beside the tape.
    pub gate: Option<GateRecord>,
    /// Set when the scored state has migrated a long way from where the SEED's
    /// own state sat in the same box. See `Gate::migrated`: a box the optimum
    /// wanders across is a region, not a place, and which part of the region
    /// the search chose is a fact worth seeing.
    pub gate_edge: Option<String>,
}

impl std::fmt::Display for Provenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.from_fork {
            write!(
                f,
                "fork resume at tick {}, {}",
                self.resume_tick.map(|t| t.to_string()).unwrap_or_else(|| "?".into()),
                self.distance
            )?;
        } else {
            write!(f, "full simulation, {}", self.distance)?;
        }
        if let Some(g) = &self.gate {
            write!(f, "; gate {}", g)?;
        }
        if let Some(e) = &self.gate_edge {
            write!(f, "; in the box, vs the seed: {}", e)?;
        }
        Ok(())
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
                Some(ms) => Outcome::fin(ms),
                None => Outcome::Dnf(crate::score::Progress::Checkpoints {
                    cps: r.cps.unwrap_or(0),
                    seg_ms: None,
                }),
            },
            Err(e) => {
                // An oracle that cannot answer is not permission to bank. The
                // guard fails CLOSED.
                let path = self.dir.join(format!("PHANTOM_unvalidated_{}.Ghost.Gbx", stamp));
                install(&tmp, &path);
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
            (Outcome::Finish { ms: a, .. }, Outcome::Finish { ms: b, .. }) => a == b,
            // A DNF incumbent claims no time, so there is nothing for the
            // oracle to contradict -- but it must still not FINISH, or the
            // search is scoring on something unrelated to the file.
            (Outcome::Dnf(_), Outcome::Dnf(_)) => true,
            // THE STATE OBJECTIVE, band 2: it reached the gate AND finished,
            // so it is a time again and it is checked like any other time.
            (Outcome::Gate(GateState::Finished { ms: a }), Outcome::Finish { ms: b, .. }) => a == b,
            // Bands 0 and 1 are a STATE, not a time, and the oracle has no
            // state to offer. There is nothing here for it to contradict: a
            // candidate the watchdog aborted has no time by construction, and
            // one that finishes without reaching the gate is exactly what
            // band 0 is for. The claim is checkable by hand instead -- the
            // measured state is written out beside the tape -- and the file
            // never acquires a millisecond it did not earn.
            (Outcome::Gate(_), _) => true,
            _ => false,
        };

        if !agrees {
            let path = self
                .dir
                .join(format!("PHANTOM_{}_{}.Ghost.Gbx", tag(&claimed), stamp));
            install(&tmp, &path);
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

        // WHAT GOES IN THE BANK IS THE ORACLE'S OWN ANSWER, never the search's
        // claim -- with one exception the type makes visible. For a gate
        // result the oracle has no such answer to give: what it confirmed is
        // that these bytes do not finish the map, and the state itself is the
        // fork's measurement, which is why it is an `Outcome::Gate` and not a
        // time, and why it is written out beside the tape to be checked.
        // WHAT GOES IN THE BANK IS THE ORACLE'S OWN ANSWER wherever the oracle
        // has an answer on the same ladder the search ranks on -- which is
        // exactly the finishing case, where the answer is a millisecond.
        //
        // A FAILURE IS BANKED ON THE SEARCH'S OWN LADDER, and returning the
        // oracle's instead was a silent stall. The plain oracle only ever
        // reports checkpoints; a fork search ranks failures by METRES along the
        // reference line, and a plain search with segment maps ranks them by
        // checkpoints WITH a time. Handing either of those back as a bare
        // `Checkpoints { cps, seg_ms: None }` returns a value from a different
        // ladder, `confirmed > incumbent` then compares two unrelated numbers,
        // and the improvement is confirmed, written to disk, and never adopted.
        // The guard's job here is done by the kind check above -- it did not
        // finish -- and the rank is the search's own measurement.
        let banked = match (claimed, actual) {
            // THE ORACLE'S MILLISECOND, THE SEARCH'S MICROSECOND. The bank
            // records the time the plain oracle measured on the written bytes
            // -- that has not changed. What it keeps from the claim is the
            // sub-tick crossing, because the oracle has no such number to give
            // and dropping it would put an incumbent with no sub-tick value
            // back into a population that is ordered by one: every later
            // candidate would then be compared on the millisecond, and the
            // search would stall on the plateau it just left.
            (Outcome::Finish { us, .. }, Outcome::Finish { ms, .. }) => Outcome::Finish { ms, us },
            // Band 2 stays on the gate's ladder -- one search, one objective --
            // but it takes the ORACLE's millisecond, like every other time
            // this bank writes. (They are equal: `agrees` above required it.)
            (Outcome::Gate(GateState::Finished { .. }), Outcome::Finish { ms, .. }) => {
                Outcome::Gate(GateState::Finished { ms })
            }
            _ => claimed,
        };
        // A band-0 or band-1 tape that turns out to FINISH is worth saying out
        // loud: the search ranked it at the bottom because it did not do the
        // thing, and a human reading the bank should still know it exists.
        if let (Outcome::Gate(g), Outcome::Finish { ms, .. }) = (claimed, actual) {
            if !matches!(g, GateState::Finished { .. }) {
                eprintln!(
                    "note: this tape does not reach the gate and the plain oracle says it \
                     FINISHES at {}. The state objective ranks it at the bottom on purpose; \
                     the time is real.",
                    secs(ms)
                );
            }
        }
        let path = self.dir.join(format!("best_{}.Ghost.Gbx", tag(&banked)));
        install(&tmp, &path);
        self.confirmed += 1;
        // THE STATE BESIDE THE TAPE. A gate result never acquires a
        // millisecond it did not earn: what it earned is a state, so that is
        // what is written down, next to the file, in the units it was measured
        // in. Anyone can check it by hand against the same map.
        if let Some(g) = &prov.gate {
            let side = path.with_extension("state.json");
            let b = g.body_vel();
            let _ = std::fs::write(
                &side,
                format!(
                    "{{\"claim\":\"{}\",\"gate_tick\":{},\"key\":{},\
                     \"pos\":[{},{},{}],\"vel\":[{},{},{}],\"quat\":[{},{},{},{}],\
                     \"speed\":{},\"body_right\":{},\"body_up\":{},\"body_fwd\":{},\
                     \"tape\":\"{}\"}}\n",
                    claimed,
                    g.tick,
                    g.key,
                    g.pos[0], g.pos[1], g.pos[2],
                    g.vel[0], g.vel[1], g.vel[2],
                    g.quat[0], g.quat[1], g.quat[2], g.quat[3],
                    g.speed(), b[0], b[1], b[2],
                    path.display()
                ),
            );
        }
        self.note(&format!(
            "{{\"confirmed\":\"{}\",\"provenance\":\"{}\",\"file\":\"{}\"}}",
            banked,
            prov,
            path.display()
        ));
        Ok(Banked { path, confirmed: banked })
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

/// Move a banked tape from the scratch root into the bank, and REFUSE to
/// continue if it does not arrive.
///
/// This was `let _ = std::fs::rename(&tmp, &path);` — three times, once for
/// each thing the guard writes. The scratch root is per-pid and defaults to
/// `/dev/shm`; a bank directory on ordinary disk is a different filesystem, so
/// the rename fails with `EXDEV` and the error was thrown away. The log then
/// records `{"confirmed":"45.140", ..., "file":"…/best_45_140.Ghost.Gbx"}` for
/// a file that does not exist, and the bank directory is empty at the end of a
/// two-and-a-half-hour run with 234 confirmed improvements in its log.
///
/// The whole point of the guard is that a result is a FILE the plain oracle
/// has agreed with. Discarding the error on the one call that produces that
/// file makes the guard's own record unfalsifiable — and it is the sort of
/// failure that only shows up when someone goes looking for the artefact,
/// which on this project is often days later and on another machine.
fn install(tmp: &Path, path: &Path) {
    if std::fs::rename(tmp, path).is_ok() {
        return;
    }
    // Cross-device: copy, then drop the original.
    match std::fs::copy(tmp, path) {
        Ok(_) => {
            let _ = std::fs::remove_file(tmp);
        }
        Err(e) => panic!(
            "the guard validated a tape and then could not put it in the bank: {} -> {}: {}. \
             A confirmed result that is not on disk is not a result.",
            tmp.display(),
            path.display(),
            e
        ),
    }
}
