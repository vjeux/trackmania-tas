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
    /// `--must MAP`: variant maps a finisher must ALSO finish on. See the
    /// module docs.
    musts: Vec<PathBuf>,
    /// How far a variant map's finish may sit from the real map's, in ms.
    must_window_ms: i64,
    dir: PathBuf,
    tag: String,
    reference: Inputs,
}

impl BatchEval {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        patcher: Arc<Patcher>,
        root: &Path,
        server: &Path,
        map: &Path,
        segs: &[(u32, PathBuf)],
        musts: &[PathBuf],
        must_window_ms: i64,
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
            musts: musts.to_vec(),
            must_window_ms,
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

/// The rung a finisher sits on while it still fails a `--must` map.
///
/// Far above any real checkpoint count, so a partially compliant finisher can
/// never be confused with — or outranked by — a run that merely collected
/// checkpoints. The ladder is `MUST_RUNG + (number of --must maps passed)`,
/// and a run that passes them all becomes a true `Finish`.
pub const MUST_RUNG: u32 = 50;

impl BatchEval {
    /// `--must`: a candidate that finishes the real map must finish the
    /// variant maps too, or it is demoted to a rung of its own.
    ///
    /// This is a HARD constraint made of engine truth, not a shaped reward:
    /// each variant is the same map with one object moved, so "finishes on all
    /// of them" is a statement about where the car actually was, measured by
    /// the same oracle that decides the result. Short-circuit in a fixed
    /// order, so a candidate that fails the first one costs one extra launch
    /// and not `n`.
    ///
    /// A variant finish only counts if it happens AT THE SAME INSTANT as the
    /// real one, within `--must-window`. Without that a run can miss the moved
    /// trigger entirely, carry on, and be caught by it -- or by another gate --
    /// a second later; the oracle says "finished" and the constraint has
    /// measured nothing. One seed did exactly this: it failed nothing and
    /// finished 1.281 s late on four of the six.
    ///
    /// Demotion keeps the main map's millisecond as `seg_ms`, so runs on the
    /// same rung are still ordered by lap time; the rung dominates, so a run
    /// that climbs one is always preferred to a faster run that did not.
    fn apply_musts(&self, files: &[PathBuf], out: &mut [Outcome]) {
        if self.musts.is_empty() {
            return;
        }
        // (index, the main map's millisecond) for everything still alive.
        let mut alive: Vec<(usize, i64)> = out
            .iter()
            .enumerate()
            .filter_map(|(i, o)| match o {
                Outcome::Finish { ms, .. } => Some((i, *ms)),
                _ => None,
            })
            .collect();
        for (j, map) in self.musts.iter().enumerate() {
            if alive.is_empty() {
                return;
            }
            let sub: Vec<&Path> = alive.iter().map(|&(i, _)| files[i].as_path()).collect();
            let tag = format!("{}_m{}", self.tag, j);
            let rows = match validate_many(&self.server, &sub, MapsMode::One(map), &tag) {
                Ok(r) => r,
                Err(e) => {
                    // An oracle failure must not silently promote a
                    // non-compliant candidate: demote everything still alive.
                    eprintln!("must[{}] oracle: {}", j, e);
                    for &(i, ms) in &alive {
                        out[i] = Outcome::Dnf(Progress::Checkpoints {
                            cps: MUST_RUNG + j as u32,
                            seg_ms: Some(ms),
                        });
                    }
                    return;
                }
            };
            let mut got: HashMap<usize, Option<i64>> = HashMap::new();
            for r in &rows {
                if let Some(i) = index_of(&r.file) {
                    got.insert(i, r.time_ms);
                }
            }
            let mut next = Vec::with_capacity(alive.len());
            for &(i, ms) in &alive {
                let ok = match got.get(&i).copied().flatten() {
                    Some(t) => (t - ms).abs() <= self.must_window_ms,
                    None => false,
                };
                if ok {
                    next.push((i, ms));
                } else {
                    out[i] = Outcome::Dnf(Progress::Checkpoints {
                        cps: MUST_RUNG + j as u32,
                        seg_ms: Some(ms),
                    });
                }
            }
            alive = next;
        }
    }
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
                Some(ms) => out[i] = Outcome::fin(ms),
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
        self.apply_musts(&files, &mut out);
        out
    }

    fn provenance(&self, _idx: usize, inputs: &Inputs) -> Provenance {
        Provenance {
            from_fork: false,
            resume_tick: None,
            distance: inputs.distance_from(&self.reference),
            gate_edge: None,
            // The plain oracle reports a time and a checkpoint count and
            // nothing about the car, so a state objective is not available on
            // this evaluator at all -- `--gate` requires `--fork`, and
            // `cmd_search` refuses the combination rather than quietly
            // producing this `None` for every candidate.
            gate: None,
        }
    }
}
