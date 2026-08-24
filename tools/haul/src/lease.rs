//! The box registry and lease rotation.
//!
//! A node lease caps at 18 hours and cannot be renewed past it. No box
//! survives a month, so the harness treats a box as a consumable: it is
//! registered when it starts, it announces its own expiry, it banks and stands
//! down before that expiry arrives, and it is marked retired. Anything it was
//! holding — queue claims — comes back by itself.
//!
//! The registry is one small file per box (`state/boxes/<node>.rec`), append
//! only, so two boxes never write the same file.

use crate::log::{self, Log};
use crate::paths::Layout;
use crate::rec::Rec;

#[derive(Debug, Clone, PartialEq)]
pub struct BoxRecord {
    pub node: String,
    pub started: i64,
    pub lease_expires: Option<i64>,
    pub last_seen: i64,
    pub retired: bool,
    pub note: String,
}

pub fn register(l: &Layout, node: &str, lease_expires: Option<i64>, note: &str) -> Result<(), String> {
    register_at(l, node, crate::time::now(), lease_expires, note)
}

/// As `register`, with the timestamp given explicitly.
pub fn register_at(
    l: &Layout,
    node: &str,
    ts: i64,
    lease_expires: Option<i64>,
    note: &str,
) -> Result<(), String> {
    let log = Log::at(l.boxes_dir().join(format!("{node}.rec")));
    let mut r = Rec::at(ts, "box_start").f("node", node).f("note", note);
    if let Some(e) = lease_expires {
        r.set("lease_expires", e);
    }
    log.append(&r).map_err(|e| e.to_string())
}

pub fn touch(l: &Layout, node: &str) -> Result<(), String> {
    let log = Log::at(l.boxes_dir().join(format!("{node}.rec")));
    log.append(&Rec::new("box_alive").f("node", node)).map_err(|e| e.to_string())
}

pub fn retire(l: &Layout, node: &str, why: &str) -> Result<(), String> {
    let log = Log::at(l.boxes_dir().join(format!("{node}.rec")));
    log.append(&Rec::new("box_retired").f("node", node).f("why", why))
        .map_err(|e| e.to_string())
}

/// Fold the per-box logs into the current picture.
pub fn all(l: &Layout) -> Result<Vec<BoxRecord>, String> {
    let recs = log::read_all(&l.boxes_dir())?;
    let mut by_node: std::collections::BTreeMap<String, BoxRecord> = Default::default();
    for r in recs {
        let Some(node) = r.get("node") else { continue };
        let e = by_node.entry(node.to_string()).or_insert(BoxRecord {
            node: node.to_string(),
            started: r.ts,
            lease_expires: None,
            last_seen: r.ts,
            retired: false,
            note: String::new(),
        });
        e.last_seen = e.last_seen.max(r.ts);
        match r.kind.as_str() {
            "box_start" => {
                e.started = r.ts;
                e.retired = false;
                if let Some(x) = r.get_i64("lease_expires") {
                    e.lease_expires = Some(x);
                }
                if let Some(n) = r.get("note") {
                    e.note = n.to_string();
                }
            }
            "box_retired" => e.retired = true,
            _ => {}
        }
    }
    Ok(by_node.into_values().collect())
}

pub fn active(l: &Layout) -> Result<Vec<BoxRecord>, String> {
    Ok(all(l)?.into_iter().filter(|b| !b.retired).collect())
}

/// What the supervisor should do about the lease, right now.
#[derive(Debug, Clone, PartialEq)]
pub enum LeaseAction {
    /// Nothing to do; the number is how long until we must stand down.
    Continue(i64),
    /// Bank everything, verify it, stop the worker, mark the box retired.
    StandDown(String),
    /// No expiry was declared. Not an error — but it means nobody can plan a
    /// rotation, so it is said out loud rather than assumed infinite.
    Unknown,
}

pub fn lease_action(now: i64, lease_expires: Option<i64>, lead_s: i64) -> LeaseAction {
    match lease_expires {
        None => LeaseAction::Unknown,
        Some(exp) if now >= exp - lead_s => LeaseAction::StandDown(format!(
            "lease expires at {} ({} away), stand-down lead is {}",
            crate::time::iso(exp),
            crate::time::dur(exp - now),
            crate::time::dur(lead_s)
        )),
        Some(exp) => LeaseAction::Continue(exp - lead_s - now),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(name: &str) -> Layout {
        let p = std::env::temp_dir().join(format!("haul-lease-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        for d in Layout::new(&p).all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        Layout::new(p)
    }

    #[test]
    fn a_box_is_registered_kept_alive_and_retired() {
        let l = layout("lifecycle");
        register(&l, "boxA", Some(1_800_000_000), "the first one").unwrap();
        touch(&l, "boxA").unwrap();
        let a = active(&l).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].lease_expires, Some(1_800_000_000));
        retire(&l, "boxA", "lease expiring").unwrap();
        assert_eq!(active(&l).unwrap().len(), 0);
        assert_eq!(all(&l).unwrap().len(), 1, "a retired box stays in the history");
    }

    #[test]
    fn the_stand_down_happens_before_the_lease_ends_not_at_it() {
        let exp = 1_800_000_000;
        let lead = 1_800;
        assert_eq!(lease_action(exp - 4000, Some(exp), lead), LeaseAction::Continue(2200));
        assert!(matches!(lease_action(exp - 1000, Some(exp), lead), LeaseAction::StandDown(_)));
        assert!(matches!(lease_action(exp + 10, Some(exp), lead), LeaseAction::StandDown(_)));
    }

    #[test]
    fn an_undeclared_lease_is_unknown_rather_than_infinite() {
        assert_eq!(lease_action(0, None, 1800), LeaseAction::Unknown);
    }
}
