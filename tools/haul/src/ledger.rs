//! The ledger: what was tried, what it produced, and **why**.
//!
//! The journal says what happened. The ledger says what we were thinking. In
//! three months the artifact of a run is nearly useless on its own — "there is
//! a tape here that reached station 31" does not tell the next person whether
//! the ordering heuristic that produced it was worth keeping. An entry that
//! records the configuration, the outcome and the reasoning does.
//!
//! Entries are append-only and writer-sharded like every other log, and each
//! one carries a `claim` tag from the project's own vocabulary — MEASURED,
//! INFERRED, UNKNOWN, SUPERSEDED — so that a later reader can tell a result
//! from a guess without re-deriving it.

use crate::log::{self, Log};
use crate::paths::Layout;
use crate::rec::Rec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    Measured,
    Inferred,
    Unknown,
    Superseded,
}

impl Claim {
    pub fn parse(s: &str) -> Option<Claim> {
        match s.to_ascii_lowercase().as_str() {
            "measured" => Some(Claim::Measured),
            "inferred" => Some(Claim::Inferred),
            "unknown" => Some(Claim::Unknown),
            "superseded" => Some(Claim::Superseded),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Claim::Measured => "MEASURED",
            Claim::Inferred => "INFERRED",
            Claim::Unknown => "UNKNOWN",
            Claim::Superseded => "SUPERSEDED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub ts: i64,
    pub what: String,
    pub config: String,
    pub produced: String,
    pub why: String,
    pub claim: Claim,
    pub control: String,
    pub node: String,
}

pub fn add(l: &Layout, node: &str, start: i64, e: &Entry) -> Result<(), String> {
    let log = Log::shard(&l.ledger_dir(), node, start).map_err(|x| x.to_string())?;
    log.append(
        &Rec::at(e.ts, "tried")
            .f("what", &e.what)
            .f("config", &e.config)
            .f("produced", &e.produced)
            .f("claim", e.claim.as_str())
            .f("control", &e.control)
            .f("why", &e.why)
            .f("node", &e.node),
    )
    .map_err(|x| x.to_string())
}

pub fn all(l: &Layout) -> Result<Vec<Entry>, String> {
    Ok(log::read_all(&l.ledger_dir())?
        .into_iter()
        .filter(|r| r.kind == "tried")
        .map(|r| Entry {
            ts: r.ts,
            what: r.get("what").unwrap_or("").to_string(),
            config: r.get("config").unwrap_or("").to_string(),
            produced: r.get("produced").unwrap_or("").to_string(),
            why: r.get("why").unwrap_or("").to_string(),
            claim: r.get("claim").and_then(Claim::parse).unwrap_or(Claim::Unknown),
            control: r.get("control").unwrap_or("").to_string(),
            node: r.get("node").unwrap_or("").to_string(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(name: &str) -> Layout {
        let p = std::env::temp_dir().join(format!("haul-ledger-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        for d in Layout::new(&p).all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        Layout::new(p)
    }

    #[test]
    fn an_entry_keeps_its_reasoning_across_a_round_trip() {
        let l = layout("why");
        let e = Entry {
            ts: 1_787_000_000,
            what: "archive bin key with airtime".to_string(),
            config: "bins=station,airtime,contact k=10".to_string(),
            produced: "furthest station 31 of 97, no CP1".to_string(),
            why: "the launch at station 28 has no progress signal without airtime\nin the key, so every candidate binned identically".to_string(),
            claim: Claim::Measured,
            control: "same seed with airtime removed reached 24 — two-sided".to_string(),
            node: "boxA".to_string(),
        };
        add(&l, "boxA", 1, &e).unwrap();
        let back = all(&l).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].why, e.why, "multi-line reasoning must survive");
        assert_eq!(back[0].claim, Claim::Measured);
        assert_eq!(back[0].control, e.control);
    }

    #[test]
    fn an_unlabelled_claim_reads_as_unknown_not_as_measured() {
        // Failing toward "we measured this" would be the worst possible
        // default in a project whose rules are about exactly that.
        assert_eq!(Claim::parse("something else"), None);
        let l = layout("claim-default");
        let log = Log::shard(&l.ledger_dir(), "boxA", 1).unwrap();
        log.append(&Rec::at(1, "tried").f("what", "x")).unwrap();
        assert_eq!(all(&l).unwrap()[0].claim, Claim::Unknown);
    }
}
