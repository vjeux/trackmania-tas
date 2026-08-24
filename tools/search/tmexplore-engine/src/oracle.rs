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
        })
    }

    /// How many ticks a tape may have. The container's archive length.
    pub fn capacity(&self) -> usize {
        self.patcher.n()
    }

    fn to_inputs(&self, tape: &[Input]) -> forkoracle::inputs::Inputs {
        let n = self.patcher.n();
        let mut steer = vec![0i8; n];
        let mut gas = vec![false; n];
        let mut brake = vec![false; n];
        for (i, t) in tape.iter().take(n).enumerate() {
            steer[i] = t.steer;
            gas[i] = t.gas;
            brake[i] = t.brake;
        }
        forkoracle::inputs::Inputs { steer, gas, brake }
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
        if tapes.iter().any(|t| t.len() > self.patcher.n()) {
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
                Some(r) => out.push(crate::verdict_of(r)),
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
