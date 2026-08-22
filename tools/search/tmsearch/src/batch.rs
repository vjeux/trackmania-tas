//! The plain evaluator: write the batch out, hand it to the dedicated server,
//! read the times back.
//!
//! Slow and authoritative. One server launch per batch, because the launch
//! dominates: thirty candidates in one call cost about what one costs.
//!
//! # Reward shaping, and the trap in it
//!
//! A run that does not finish returns no time, and on many maps the failure
//! signal is nearly binary -- "reached some checkpoints (2)" or the
//! information-free "wrong simu". A **segment map** (the same map with the
//! finish moved to checkpoint k) turns that into an exact millisecond for the
//! part of the run that did happen.
//!
//! The trap: optimising the segment is not optimising the map. The naive ladder
//! ranked "fastest to CP3" first and its winner finished a full second slower
//! on the real map, because it bought a state the car could not use. So a
//! candidate is scored on the FULL map first and only re-scored on a segment if
//! it failed -- and [`crate::score::Outcome`] puts every finisher above every
//! failure, so the shaping can guide a failure towards completing and can never
//! make "fast to a checkpoint" look better than "actually finished".
//!
//! A second trap, paid for on another map: a segment map made by swapping a
//! `GateCheckpointLeft32m` for a `GateFinish32m` is **not a faithful trigger**
//! (-0.206 s of phantom gain), and the reference-ghost identity control cannot
//! catch it because the reference line is inside both volumes. A promoted gate
//! is a fine ruler and an unsafe objective; prefer a position-only relocation.

use crate::guard::Provenance;
use forkoracle::inputs::Inputs;
use crate::score::{Outcome, Progress};
use crate::search::Evaluator;
use crate::tape::Patcher;
use ghost::oracle::{validate_many, MapsMode};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct BatchEval {
    patcher: Arc<Patcher>,
    server: PathBuf,
    map: PathBuf,
    /// checkpoint depth -> the segment map that ends there
    segs: Vec<(u32, PathBuf)>,
    dir: PathBuf,
    tag: String,
    reference: Inputs,
}

impl BatchEval {
    pub fn new(
        patcher: Arc<Patcher>,
        root: &Path,
        server: &Path,
        map: &Path,
        segs: &[(u32, PathBuf)],
        wi: usize,
        reference: Inputs,
    ) -> Result<BatchEval, String> {
        let dir = root.join(format!("w{:03}", wi));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
        Ok(BatchEval {
            patcher,
            server: server.to_path_buf(),
            map: map.to_path_buf(),
            segs: segs.to_vec(),
            dir,
            tag: format!("w{:03}", wi),
            reference,
        })
    }

    fn write_batch(&self, cands: &[Inputs]) -> Vec<PathBuf> {
        let mut buf = self.patcher.base.clone();
        let mut files = Vec::with_capacity(cands.len());
        for (i, c) in cands.iter().enumerate() {
            self.patcher.apply(&mut buf, c);
            let p = self.dir.join(format!("c{:04}.Ghost.Gbx", i));
            std::fs::write(&p, &buf).expect("write a candidate");
            files.push(p);
        }
        files
    }
}

fn index_of(name: &str) -> Option<usize> {
    name.trim_start_matches('c').trim_end_matches(".Ghost.Gbx").parse().ok()
}

impl Evaluator for BatchEval {
    fn evaluate(&mut self, cands: &[Inputs]) -> Vec<Outcome> {
        let files = self.write_batch(cands);
        let refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
        let mut out =
            vec![Outcome::Dnf(Progress::Checkpoints { cps: 0, seg_ms: None }); cands.len()];

        let rows = match validate_many(&self.server, &refs, MapsMode::One(&self.map), &self.tag) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("oracle: {}", e);
                return out;
            }
        };
        let mut need: HashMap<u32, Vec<usize>> = HashMap::new();
        for r in &rows {
            let Some(i) = index_of(&r.file) else { continue };
            if i >= out.len() {
                continue;
            }
            match r.time_ms {
                Some(ms) => out[i] = Outcome::Finish { ms },
                None => {
                    let cps = r.cps.unwrap_or(0);
                    out[i] = Outcome::Dnf(Progress::Checkpoints { cps, seg_ms: None });
                    if cps > 0 && self.segs.iter().any(|(k, _)| *k == cps) {
                        need.entry(cps).or_default().push(i);
                    }
                }
            }
        }
        for (k, idxs) in need {
            let Some((_, map)) = self.segs.iter().find(|(d, _)| *d == k) else { continue };
            let sub: Vec<&Path> = idxs.iter().map(|&i| files[i].as_path()).collect();
            let tag = format!("{}_s{}", self.tag, k);
            let Ok(rows) = validate_many(&self.server, &sub, MapsMode::One(map), &tag) else {
                continue;
            };
            for r in rows {
                let Some(i) = index_of(&r.file) else { continue };
                if let (true, Some(ms)) = (i < out.len(), r.time_ms) {
                    out[i] = Outcome::Dnf(Progress::Checkpoints { cps: k, seg_ms: Some(ms) });
                }
            }
        }
        out
    }

    fn provenance(&self, _idx: usize, inputs: &Inputs) -> Provenance {
        Provenance {
            from_fork: false,
            resume_tick: None,
            distance: inputs.distance_from(&self.reference),
            // The plain oracle reports a time and a checkpoint count and
            // nothing about the car, so a state objective is not available on
            // this evaluator at all -- `--gate` requires `--fork`, and
            // `cmd_search` refuses the combination rather than quietly
            // producing this `None` for every candidate.
            gate: None,
        }
    }
}
