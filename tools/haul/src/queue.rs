//! The durable work queue.
//!
//! One file per item, moved between `pending/`, `claimed/` and `done/`. A
//! directory of small files rather than one index file, for the same reason
//! the logs are sharded: two boxes touching different items never conflict in
//! git, and a half-written item is one unreadable file rather than a corrupt
//! index.
//!
//! **A claim is a lease, not a lock.** It carries an expiry and the id of the
//! box that took it. When a box dies — and on an 18-hour lease cap, every box
//! dies — its claims expire and `reap` returns them to `pending`. Nothing
//! waits for a human to notice.

use crate::md5::md5_hex;
use crate::paths::Layout;
use crate::rec::Rec;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub payload: String,
    pub priority: i64,
    pub created: i64,
    pub claimed_by: Option<String>,
    pub claim_expires: Option<i64>,
    pub attempts: i64,
}

impl Item {
    pub fn new(id: &str, kind: &str, payload: &str, priority: i64) -> Item {
        Item {
            id: id.to_string(),
            kind: kind.to_string(),
            payload: payload.to_string(),
            priority,
            created: crate::time::now(),
            claimed_by: None,
            claim_expires: None,
            attempts: 0,
        }
    }

    pub fn to_rec(&self) -> Rec {
        let mut r = Rec::at(self.created, "queue_item")
            .f("id", &self.id)
            .f("kind", &self.kind)
            .f("payload", &self.payload)
            .f("priority", self.priority)
            .f("attempts", self.attempts);
        if let Some(b) = &self.claimed_by {
            r.set("claimed_by", b);
        }
        if let Some(e) = self.claim_expires {
            r.set("claim_expires", e);
        }
        r
    }

    pub fn from_rec(r: &Rec) -> Result<Item, String> {
        Ok(Item {
            id: r.get("id").ok_or("item has no id")?.to_string(),
            kind: r.get("kind").unwrap_or("").to_string(),
            payload: r.get("payload").unwrap_or("").to_string(),
            priority: r.get_i64("priority").unwrap_or(0),
            created: r.ts,
            claimed_by: r.get("claimed_by").map(|s| s.to_string()),
            claim_expires: r.get_i64("claim_expires"),
            attempts: r.get_i64("attempts").unwrap_or(0),
        })
    }

    fn filename(&self) -> String {
        format!("{}.rec", crate::paths::sanitize(&self.id))
    }
}

pub struct Queue {
    pending: PathBuf,
    claimed: PathBuf,
    done: PathBuf,
}

impl Queue {
    pub fn open(l: &Layout) -> std::io::Result<Queue> {
        let q = Queue { pending: l.queue_pending(), claimed: l.queue_claimed(), done: l.queue_done() };
        for d in [&q.pending, &q.claimed, &q.done] {
            std::fs::create_dir_all(d)?;
        }
        Ok(q)
    }

    /// A stable id for a piece of work, so pushing the same work twice is one
    /// item rather than two.
    pub fn derive_id(kind: &str, payload: &str) -> String {
        format!("{kind}-{}", &md5_hex(format!("{kind}\u{1}{payload}").as_bytes())[..12])
    }

    fn write(dir: &Path, it: &Item) -> std::io::Result<()> {
        std::fs::write(dir.join(it.filename()), format!("{}\n", it.to_rec().render()))
    }

    fn read_dir(dir: &Path) -> Result<Vec<Item>, String> {
        let mut out = Vec::new();
        if !dir.exists() {
            return Ok(out);
        }
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(|e| format!("read_dir {dir:?}: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "rec").unwrap_or(false))
            .collect();
        paths.sort();
        for p in paths {
            let text = std::fs::read_to_string(&p).map_err(|e| format!("read {p:?}: {e}"))?;
            let recs = Rec::parse_all(&text).map_err(|e| format!("{}: {e}", p.display()))?;
            let r = recs.first().ok_or_else(|| format!("{}: empty item file", p.display()))?;
            out.push(Item::from_rec(r).map_err(|e| format!("{}: {e}", p.display()))?);
        }
        Ok(out)
    }

    pub fn pending(&self) -> Result<Vec<Item>, String> {
        Queue::read_dir(&self.pending)
    }
    pub fn claimed(&self) -> Result<Vec<Item>, String> {
        Queue::read_dir(&self.claimed)
    }
    pub fn done(&self) -> Result<Vec<Item>, String> {
        Queue::read_dir(&self.done)
    }

    /// Idempotent: pushing an id that already exists anywhere changes nothing.
    pub fn push(&self, it: &Item) -> Result<bool, String> {
        for d in [&self.pending, &self.claimed, &self.done] {
            if d.join(it.filename()).exists() {
                return Ok(false);
            }
        }
        Queue::write(&self.pending, it).map_err(|e| e.to_string())?;
        Ok(true)
    }

    /// Take the highest-priority (then oldest) pending item, with a lease.
    pub fn claim(&self, node: &str, ttl_s: i64) -> Result<Option<Item>, String> {
        let mut ps = self.pending()?;
        ps.sort_by_key(|i| (-i.priority, i.created, i.id.clone()));
        let Some(mut it) = ps.into_iter().next() else { return Ok(None) };
        it.claimed_by = Some(node.to_string());
        it.claim_expires = Some(crate::time::now() + ttl_s);
        it.attempts += 1;
        Queue::write(&self.claimed, &it).map_err(|e| e.to_string())?;
        std::fs::remove_file(self.pending.join(it.filename())).map_err(|e| e.to_string())?;
        Ok(Some(it))
    }

    pub fn complete(&self, id: &str, outcome: &str) -> Result<bool, String> {
        let fname = format!("{}.rec", crate::paths::sanitize(id));
        let src = self.claimed.join(&fname);
        if !src.exists() {
            return Ok(false);
        }
        let text = std::fs::read_to_string(&src).map_err(|e| e.to_string())?;
        let r = Rec::parse_all(&text)?.into_iter().next().ok_or("empty item")?;
        let mut it = Item::from_rec(&r)?;
        it.claimed_by = None;
        it.claim_expires = None;
        let mut rec = it.to_rec();
        rec.set("outcome", outcome);
        rec.set("completed", crate::time::now());
        std::fs::write(self.done.join(&fname), format!("{}\n", rec.render()))
            .map_err(|e| e.to_string())?;
        std::fs::remove_file(&src).map_err(|e| e.to_string())?;
        Ok(true)
    }

    /// Return expired claims to `pending`. This is what makes a dead box
    /// harmless: nobody has to notice, the work simply becomes available
    /// again on the next pass.
    pub fn reap(&self, now: i64) -> Result<Vec<String>, String> {
        let mut reaped = Vec::new();
        for mut it in self.claimed()? {
            if it.claim_expires.map(|e| e <= now).unwrap_or(false) {
                let fname = it.filename();
                let holder = it.claimed_by.clone().unwrap_or_default();
                it.claimed_by = None;
                it.claim_expires = None;
                Queue::write(&self.pending, &it).map_err(|e| e.to_string())?;
                std::fs::remove_file(self.claimed.join(&fname)).map_err(|e| e.to_string())?;
                reaped.push(format!("{} (was {holder})", it.id));
            }
        }
        Ok(reaped)
    }

    pub fn expired_count(&self, now: i64) -> Result<usize, String> {
        Ok(self
            .claimed()?
            .iter()
            .filter(|i| i.claim_expires.map(|e| e <= now).unwrap_or(false))
            .count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(name: &str) -> Layout {
        let p = std::env::temp_dir().join(format!("haul-q-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Layout::new(p)
    }

    #[test]
    fn an_item_survives_a_round_trip_through_the_filesystem() {
        let l = layout("roundtrip");
        let q = Queue::open(&l).unwrap();
        let it = Item::new("job-1", "search", "map=summer01 seed=7", 5);
        assert!(q.push(&it).unwrap());
        let back = q.pending().unwrap().into_iter().next().unwrap();
        assert_eq!(back.id, "job-1");
        assert_eq!(back.payload, "map=summer01 seed=7");
        assert_eq!(back.priority, 5);
    }

    #[test]
    fn pushing_the_same_id_twice_does_not_duplicate_work() {
        let l = layout("dedupe");
        let q = Queue::open(&l).unwrap();
        let it = Item::new("job-1", "search", "x", 0);
        assert!(q.push(&it).unwrap());
        assert!(!q.push(&it).unwrap());
        assert_eq!(q.pending().unwrap().len(), 1);
    }

    #[test]
    fn claiming_takes_priority_order_then_age() {
        let l = layout("order");
        let q = Queue::open(&l).unwrap();
        let mut low = Item::new("low", "k", "p", 1);
        low.created = 100;
        let mut high = Item::new("high", "k", "p", 9);
        high.created = 200;
        q.push(&low).unwrap();
        q.push(&high).unwrap();
        assert_eq!(q.claim("boxA", 600).unwrap().unwrap().id, "high");
        assert_eq!(q.claim("boxA", 600).unwrap().unwrap().id, "low");
        assert!(q.claim("boxA", 600).unwrap().is_none());
    }

    #[test]
    fn a_dead_boxs_claim_comes_back_by_itself() {
        // The 18-hour lease cap means every box dies holding something. This
        // is the mechanism that makes that a non-event.
        let l = layout("reap");
        let q = Queue::open(&l).unwrap();
        q.push(&Item::new("job-1", "k", "p", 0)).unwrap();
        let claimed = q.claim("doomed-box", 60).unwrap().unwrap();
        assert_eq!(q.pending().unwrap().len(), 0);
        let after_expiry = claimed.claim_expires.unwrap() + 1;

        assert_eq!(q.reap(after_expiry - 120).unwrap().len(), 0, "a live claim must not be reaped");
        let reaped = q.reap(after_expiry).unwrap();

        assert_eq!(reaped.len(), 1);
        assert!(reaped[0].contains("doomed-box"));
        let back = q.pending().unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].attempts, 1, "the retry count must survive the box that died");
    }

    #[test]
    fn completion_moves_the_item_and_records_the_outcome() {
        let l = layout("complete");
        let q = Queue::open(&l).unwrap();
        q.push(&Item::new("job-1", "k", "p", 0)).unwrap();
        q.claim("boxA", 600).unwrap();
        assert!(q.complete("job-1", "banked ms=23144").unwrap());
        assert_eq!(q.claimed().unwrap().len(), 0);
        let done = q.done().unwrap();
        assert_eq!(done.len(), 1);
        assert!(!q.complete("job-1", "again").unwrap(), "completing twice is a no-op");
    }

    #[test]
    fn derived_ids_are_stable_and_distinct() {
        let a = Queue::derive_id("search", "map=summer01 seed=7");
        let b = Queue::derive_id("search", "map=summer01 seed=8");
        assert_eq!(a, Queue::derive_id("search", "map=summer01 seed=7"));
        assert_ne!(a, b);
    }
}
