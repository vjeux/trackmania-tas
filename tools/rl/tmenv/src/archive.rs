//! The state archive for reset-anywhere: a few kept states per progress
//! bucket along the best rollouts, so an episode can start where progress
//! stalls instead of at the line every time (the backward-curriculum idea the
//! savestate tree makes cheap: a kept state is a paused process, and resuming
//! it costs one fork).
//!
//! The archive holds `StateId`s and their scores; the env owns the nodes.
//! `offer` says which id it evicted so the caller can `drop_snapshot` it.

use crate::forkenv::StateId;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub id: StateId,
    /// Arc-length progress at the state, metres.
    pub progress_m: f32,
    /// Race clock at the state.
    pub race_ms: i32,
    /// Higher is better; ties broken by the earlier race clock.
    pub score: f32,
    /// 0 = a human-line snapshot (the donor tape replayed: its prefix carries
    /// HUMAN inputs and must never be banked as a policy best), 1 = a policy
    /// snapshot. LEARN's rule since 2719fe0f.
    pub origin: u8,
}

#[derive(Clone, Debug)]
pub struct StateArchive {
    pub bucket_m: f32,
    pub per_bucket: usize,
    buckets: BTreeMap<u32, Vec<Entry>>,
}

impl StateArchive {
    pub fn new(bucket_m: f32, per_bucket: usize) -> StateArchive {
        StateArchive { bucket_m: bucket_m.max(1.0), per_bucket: per_bucket.max(1), buckets: BTreeMap::new() }
    }

    fn bucket_of(&self, progress_m: f32) -> u32 {
        (progress_m.max(0.0) / self.bucket_m) as u32
    }

    /// Offer a state. Returns the id evicted to make room, if any (the caller
    /// must drop that snapshot), or the offered id itself when the bucket is
    /// full of better states.
    pub fn offer(&mut self, e: Entry) -> Option<StateId> {
        let b = self.bucket_of(e.progress_m);
        let v = self.buckets.entry(b).or_default();
        v.push(e);
        v.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.race_ms.cmp(&b.race_ms))
        });
        if v.len() > self.per_bucket {
            v.pop().map(|x| x.id)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.buckets.values().map(|v| v.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn buckets(&self) -> usize {
        self.buckets.len()
    }

    /// Uniform over buckets, then uniform inside the bucket, from one random
    /// word: every stretch of the track is equally likely to be trained from,
    /// however many states it holds.
    pub fn sample(&self, r: u64) -> Option<Entry> {
        if self.buckets.is_empty() {
            return None;
        }
        let bi = (r % self.buckets.len() as u64) as usize;
        let v = self.buckets.values().nth(bi)?;
        let ei = ((r / self.buckets.len() as u64) % v.len() as u64) as usize;
        v.get(ei).copied()
    }

    /// The furthest-progress bucket's best entry.
    pub fn frontier(&self) -> Option<Entry> {
        self.buckets.values().next_back().and_then(|v| v.first().copied())
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.buckets.values().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: u64, p: f32, ms: i32, score: f32) -> Entry {
        Entry { id: StateId(id), progress_m: p, race_ms: ms, score }
    }

    #[test]
    fn a_full_bucket_evicts_the_worst_and_says_which() {
        let mut a = StateArchive::new(50.0, 2);
        assert_eq!(a.offer(e(1, 10.0, 1000, 1.0)), None);
        assert_eq!(a.offer(e(2, 20.0, 1200, 2.0)), None);
        // worse than both: evicted itself
        assert_eq!(a.offer(e(3, 30.0, 1300, 0.5)), Some(StateId(3)));
        // better than 1: 1 goes
        assert_eq!(a.offer(e(4, 40.0, 900, 3.0)), Some(StateId(1)));
        assert_eq!(a.len(), 2);
        assert_eq!(a.frontier().unwrap().id, StateId(4));
    }

    #[test]
    fn buckets_are_by_progress_and_sampling_covers_them() {
        let mut a = StateArchive::new(100.0, 3);
        for i in 0..30u64 {
            a.offer(e(i, i as f32 * 20.0, 0, 1.0));
        }
        assert_eq!(a.buckets(), 6);
        assert_eq!(a.len(), 18);
        let mut seen = std::collections::HashSet::new();
        for r in 0..600u64 {
            seen.insert(a.sample(r * 7919 + 1).unwrap().id);
        }
        assert_eq!(seen.len(), 18, "every kept state is reachable");
    }

    #[test]
    fn ties_prefer_the_earlier_clock() {
        let mut a = StateArchive::new(50.0, 1);
        a.offer(e(1, 10.0, 2000, 1.0));
        assert_eq!(a.offer(e(2, 12.0, 1500, 1.0)), Some(StateId(1)));
    }
}

/// One persisted state: the archive's entry plus the action prefix that
/// re-materialises it (`ForkEnv::import_snapshot`). Steer as i8, gas/brake as
/// 0/1, one triple per tick from the root.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SavedEntry {
    pub progress_m: f32,
    pub race_ms: i32,
    pub score: f32,
    #[serde(default)]
    pub origin: u8,
    pub prefix: Vec<[i8; 3]>,
}

/// The archive on disk (JSON): its parameters and every entry with its prefix.
/// A state IS its prefix in this env (G4: replaying it from the root lands on
/// the same bytes), so nothing engine-side is stored -- a new run on the same
/// map and template loads the file and starts from the human line without
/// replaying it first; each state is re-materialised the first time it is
/// reset to (k-tick chunks, ~0.4 s for a 20 s prefix) and cached under the pin
/// budget after that.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SavedArchive {
    pub bucket_m: f32,
    pub per_bucket: usize,
    /// The template the prefixes were driven in (its file name); a different
    /// template makes the prefixes meaningless, so `load` refuses a mismatch.
    pub template: String,
    pub entries: Vec<SavedEntry>,
}

impl StateArchive {
    /// Write the archive, prefixes included, for `env`'s snapshots.
    pub fn save(&self, env: &crate::forkenv::ForkEnv, template: &str, path: &std::path::Path) -> Result<usize, String> {
        let mut entries = Vec::new();
        for e in self.entries() {
            let Some(p) = env.snapshot_prefix(&e.id) else { continue };
            entries.push(SavedEntry {
                progress_m: e.progress_m,
                race_ms: e.race_ms,
                score: e.score,
                origin: e.origin,
                prefix: p.iter().map(|a| [a.steer, a.gas as i8, a.brake as i8]).collect(),
            });
        }
        let n = entries.len();
        let s = SavedArchive { bucket_m: self.bucket_m, per_bucket: self.per_bucket, template: template.to_string(), entries };
        let j = serde_json::to_string(&s).map_err(|e| e.to_string())?;
        std::fs::write(path, j).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(n)
    }

    /// Read an archive back into `env`: every prefix becomes an imported
    /// snapshot, every entry is offered as saved. `template` must be the one
    /// the file names.
    pub fn load(path: &std::path::Path, env: &mut crate::forkenv::ForkEnv, template: &str) -> Result<StateArchive, String> {
        let j = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let s: SavedArchive = serde_json::from_str(&j).map_err(|e| format!("{}: {e}", path.display()))?;
        if s.template != template {
            return Err(format!(
                "{}: the archive was driven in template {:?}, this env runs {:?} -- the prefixes would not be the same states",
                path.display(),
                s.template,
                template
            ));
        }
        let mut a = StateArchive::new(s.bucket_m, s.per_bucket);
        for e in s.entries {
            let prefix: Vec<tmstate::Action> =
                e.prefix.iter().map(|t| tmstate::Action { steer: t[0], gas: t[1] != 0, brake: t[2] != 0 }).collect();
            let id = env.import_snapshot(prefix)?;
            if let Some(evicted) = a.offer(Entry { id, progress_m: e.progress_m, race_ms: e.race_ms, score: e.score, origin: e.origin }) {
                env.drop_snapshot(&evicted);
            }
        }
        Ok(a)
    }
}
