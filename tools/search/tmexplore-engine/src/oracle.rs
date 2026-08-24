//! [`EngineOracle`]: the explorer's `PlainOracle`, which is the only thing in
//! the system that can turn a candidate into a result.
//!
//! ```text
//!   Vec<Input>  --pad-->  Inputs  --Patcher-->  .Ghost.Gbx  --server-->  Verdict
//! ```
//!
//! Every arrow is somebody else's code. `Patcher` is `tmsearch`'s (and it
//! derives its bit positions by probing `ghost`'s encoder, so it cannot drift
//! from the codec); `validate_many` is `ghost`'s. This file is the wiring and
//! the rules about what may be believed.
//!
//! # What it refuses to do
//!
//! * **It does not read the declared time.** `SimResult::declared_ms` is what
//!   the FILE claims and `time_ms` is what the server SIMULATED; a patched
//!   tape inherits its template's header, so the declaration is the donor's
//!   number until something writes it. Reading it as a result is the recurring
//!   defect in this layer.
//! * **It does not accept a time from anywhere else.** There is no path in
//!   this type that produces a `Finish` without a server having simulated a
//!   file that exists on disk.

use ghost::oracle::{validate_many, MapsMode};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tmexplore::action::Input;
use tmexplore::branch::PlainOracle;
use tmexplore::outcome::Verdict;
use tmsearch::tape::Patcher;

pub struct EngineOracle {
    patcher: Arc<Patcher>,
    server: PathBuf,
    map: PathBuf,
    dir: PathBuf,
    seq: AtomicU64,
    pub calls: AtomicU64,
    prefix_ticks: AtomicU64,
}

impl EngineOracle {
    /// `template` is the container agent A synthesizes. It is not a driver, a
    /// seed or a reference: only its BIT LAYOUT is used, and every tick of it
    /// is overwritten.
    pub fn new(template: &Path, map: &Path, server: &Path, work: &Path) -> Result<EngineOracle, String> {
        let patcher = Patcher::build(&template.to_string_lossy())?;
        if !patcher.unwritable.is_empty() {
            // Stated, not silent. A tick the writer cannot patch is a tick the
            // search can plan and the file will not contain.
            return Err(format!(
                "the template has {} unwritable ticks (first: {:?}); a synthesized container should have none",
                patcher.unwritable.len(),
                patcher.unwritable.first()
            ));
        }
        std::fs::create_dir_all(work).map_err(|e| format!("{}: {}", work.display(), e))?;
        Ok(EngineOracle {
            patcher: Arc::new(patcher),
            server: server.to_path_buf(),
            map: map.to_path_buf(),
            dir: work.to_path_buf(),
            seq: AtomicU64::new(0),
            calls: AtomicU64::new(0),
            prefix_ticks: AtomicU64::new(0),
        })
    }

    /// The container's OWN input tape.
    ///
    /// This is what the fork server is simulating below its boundary and what
    /// the shim is keyed on. It is not a driver, a seed or a reference line —
    /// it is the bits the engine consumes before the search gets a say. Taking
    /// it from the container rather than assuming it is neutral is what makes
    /// the identity control able to pass at all.
    pub fn template_inputs(&self) -> forkoracle::inputs::Inputs {
        self.patcher.template.clone()
    }

    /// How many ticks a tape may have. The container's archive length.
    pub fn capacity(&self) -> usize {
        self.patcher.n()
    }

    /// Lay a search tape into the container's tick frame.
    ///
    /// # The search's tick 0 is NOT the file's tick 0
    ///
    /// The fork server resumes at its own probed boundary and the ticks below
    /// it are the container's own inputs — already consumed, and a write there
    /// is a silent no-op. So the search's tick 0 is the BOUNDARY, and writing
    /// its tape at file tick 0 produces a file that is a different run from
    /// the one the fork evaluated.
    ///
    /// Measured: the fork reported a tape reaching station 55 with one gate
    /// collected, and the plain oracle called the written file `Dnf cps 0`.
    /// That reads exactly like the phantom pattern and it was an alignment
    /// bug — which is why the first question to ask of a disagreement is
    /// whether the two instruments were given the same run.
    ///
    /// Ticks past the end of the candidate keep the container's own inputs
    /// rather than going neutral: a neutral pad lifts the throttle before the
    /// line on any tape whose length was underestimated.
    fn to_inputs(&self, tape: &[Input]) -> forkoracle::inputs::Inputs {
        let n = self.patcher.n();
        let base = &self.patcher.template;
        let mut steer = base.steer.clone();
        let mut gas = base.gas.clone();
        let mut brake = base.brake.clone();
        steer.resize(n, 0);
        gas.resize(n, false);
        brake.resize(n, false);
        let off = self.prefix_ticks.load(Ordering::Relaxed) as usize;
        for (i, t) in tape.iter().enumerate() {
            let j = off + i;
            if j >= n {
                break;
            }
            steer[j] = t.steer;
            gas[j] = t.gas;
            brake[j] = t.brake;
        }
        forkoracle::inputs::Inputs { steer, gas, brake }
    }

    /// Where the search's tick 0 lands in the file: the fork server's own
    /// probed boundary.
    pub fn set_prefix_ticks(&self, n: u64) {
        self.prefix_ticks.store(n, Ordering::Relaxed);
    }
    pub fn prefix_ticks(&self) -> u64 {
        self.prefix_ticks.load(Ordering::Relaxed)
    }

    /// Confirm one tape and also return **the engine's own echo of the input
    /// tape it decoded**.
    ///
    /// This is the control that matters at startup and it costs nothing. Two
    /// tapes that differ in every tick must produce different echoes; if they
    /// do not, the writer is not reaching the engine and every number after
    /// that is about a run nobody wrote. Comparing verdicts cannot do this
    /// job — two tapes that both crash at the first corner both return
    /// `Dnf cps 0`, which is a true statement about the driving and no
    /// statement at all about the plumbing.
    pub fn confirm_echo(&self, tape: &[Input]) -> Result<(Verdict, String, String, String), String> {
        let batch = self.seq.fetch_add(1, Ordering::Relaxed);
        let mut buf = self.patcher.base.clone();
        self.patcher.apply(&mut buf, &self.to_inputs(tape));
        let p = self.dir.join(format!("echo_{}.Ghost.Gbx", batch));
        std::fs::write(&p, &buf).map_err(|e| format!("{}: {}", p.display(), e))?;
        let res = validate_many(&self.server, &[p.as_path()], MapsMode::One(&self.map), "tmexplore-echo")?;
        let _ = std::fs::remove_file(&p);
        let r = res.first().ok_or("the server mentioned no file")?;
        Ok((crate::verdict_of(r)?, r.inputs.clone(), r.map_uid.clone(), r.desc.clone()))
    }

    /// Confirm a batch in one server launch.
    ///
    /// One launch dominates the per-file cost, so a batch of thirty costs
    /// about what one does. The explorer confirms one candidate at a time
    /// today because candidates are rare; this exists for the campaign sweep.
    pub fn confirm_many(&self, tapes: &[Vec<Input>]) -> Result<Vec<Verdict>, String> {
        if tapes.is_empty() {
            return Ok(Vec::new());
        }
        let off = self.prefix_ticks.load(Ordering::Relaxed) as usize;
        if tapes.iter().any(|t| t.len() + off > self.patcher.n()) {
            return Err(format!(
                "a tape is longer than the container's input archive ({} ticks). \
                 A longer tape is a different run, and truncating it silently would be a result \
                 about a run nobody asked for.",
                self.patcher.n()
            ));
        }
        let batch = self.seq.fetch_add(1, Ordering::Relaxed);
        let mut buf = self.patcher.base.clone();
        let mut files: Vec<PathBuf> = Vec::with_capacity(tapes.len());
        for (i, t) in tapes.iter().enumerate() {
            self.patcher.apply(&mut buf, &self.to_inputs(t));
            let p = self.dir.join(format!("cand_{}_{:04}.Ghost.Gbx", batch, i));
            std::fs::write(&p, &buf).map_err(|e| format!("{}: {}", p.display(), e))?;
            files.push(p);
        }
        let refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
        let res = validate_many(&self.server, &refs, MapsMode::One(&self.map), "tmexplore")?;
        self.calls.fetch_add(tapes.len() as u64, Ordering::Relaxed);

        // Match answers back to files BY NAME. The server reports in the order
        // it read them, which is not the order they were written, and a
        // positional match silently attributes one run's time to another.
        let mut out = Vec::with_capacity(tapes.len());
        for f in &files {
            let want = f.file_name().unwrap().to_string_lossy().to_string();
            match res.iter().find(|r| r.file.ends_with(&want)) {
                Some(r) => out.push(crate::verdict_of(r)?),
                // A file the server never mentioned is a fact worth having,
                // not a silent DNF: an absent row is not a failure row.
                None => {
                    return Err(format!("the server never mentioned {}", want));
                }
            }
        }
        for f in &files {
            let _ = std::fs::remove_file(f);
        }
        Ok(out)
    }
}

impl PlainOracle for EngineOracle {
    fn confirm(&self, tape: &[Input]) -> Result<Verdict, String> {
        Ok(self.confirm_many(&[tape.to_vec()])?[0])
    }
}
