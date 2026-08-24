//! Recovery: a fresh box plus the repo, and nothing else.
//!
//! The 18-hour lease cap means recovery is not an exceptional path — it is the
//! normal way this project moves from one day to the next. So it is a command,
//! not a runbook, and it is exercised rather than described.
//!
//! Two sources, and the merge between them is the interesting part:
//!
//! * the **repo**, which a public clone gives anyone, needing no credential;
//! * the newest **mirror** paste, which carries whatever the last box banked
//!   after its final commit — or everything, if it never managed to push.
//!
//! Every durable file is an append-only record log, which is what makes the
//! merge safe: the union of two logs is a log, and re-running the union
//! produces the same state. A record that appears in both is one record, not
//! two, because a record's identity is its rendered line.

use crate::bank;
use crate::pack;
use crate::paths::Layout;
use crate::rec::Rec;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Default)]
pub struct Report {
    pub files_seen: usize,
    pub files_written: usize,
    pub files_merged: usize,
    pub records_added: usize,
    pub identical: usize,
    pub conflicts: Vec<String>,
    pub source: String,
}

/// Union two record logs, keeping every distinct line, ordered by timestamp.
pub fn merge_records(ours: &str, theirs: &str) -> Result<(String, usize), String> {
    let mine = Rec::parse_all(ours)?;
    let yours = Rec::parse_all(theirs)?;
    let mut seen: BTreeSet<String> = mine.iter().map(|r| r.render()).collect();
    let before = seen.len();
    let mut all: Vec<Rec> = mine;
    for r in yours {
        if seen.insert(r.render()) {
            all.push(r);
        }
    }
    all.sort_by(|a, b| a.ts.cmp(&b.ts).then_with(|| a.render().cmp(&b.render())));
    let added = seen.len() - before;
    let mut out = String::new();
    for r in &all {
        out.push_str(&r.render());
        out.push('\n');
    }
    Ok((out, added))
}

/// Apply an unpacked mirror onto the state tree.
pub fn apply(l: &Layout, u: &pack::Unpacked) -> Result<Report, String> {
    let mut rep = Report { source: format!("mirror generated {} on {}", u.generated, u.node), ..Default::default() };
    for (rel, data) in &u.files {
        rep.files_seen += 1;
        let dest = l.root().join(rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        if !dest.exists() {
            std::fs::write(&dest, data).map_err(|e| e.to_string())?;
            rep.files_written += 1;
            continue;
        }
        let ours = std::fs::read(&dest).map_err(|e| e.to_string())?;
        if ours == *data {
            rep.identical += 1;
            continue;
        }
        if dest.extension().map(|x| x == "rec").unwrap_or(false) {
            let ours_s = String::from_utf8_lossy(&ours).to_string();
            let theirs_s = String::from_utf8_lossy(data).to_string();
            match merge_records(&ours_s, &theirs_s) {
                Ok((merged, added)) => {
                    std::fs::write(&dest, merged).map_err(|e| e.to_string())?;
                    rep.files_merged += 1;
                    rep.records_added += added;
                }
                Err(e) => rep.conflicts.push(format!("{rel}: {e}")),
            }
        } else {
            // Not a log: the local copy wins and the difference is reported.
            // Silently overwriting a tape with an older one would be the
            // worst possible behaviour here.
            rep.conflicts.push(format!(
                "{rel}: differs from the mirror and is not a record log — left as it is on disk"
            ));
        }
    }
    Ok(rep)
}

/// The whole recovery: pull the newest mirror, verify it, merge it in.
pub fn recover(l: &Layout) -> Result<Report, String> {
    let Some((id, title)) = bank::latest_mirror()? else {
        return Err("no mirror paste found — is `meta` working on this box?".into());
    };
    let body = bank::read_mirror(&id)?;
    let u = pack::unpack(&body)?;
    let mut rep = apply(l, &u)?;
    rep.source = format!("{id} ({title})");
    Ok(rep)
}

/// Fast-forward the checkout from the public remote. Read-only access needs no
/// credential, which is the point: a fresh box can get the state of record
/// before it can push anything.
pub fn pull(l: &Layout, branch: &str) -> Result<String, String> {
    let o = crate::gitcmd::try_run(&l.repo, "git", &["pull", "--ff-only", "origin", branch])?;
    if o.code != 0 {
        return Err(format!("git pull failed: {}", o.stderr.trim()));
    }
    Ok(o.stdout.trim().to_string())
}

pub fn assert_within(repo: &Path) -> Result<(), String> {
    if !repo.join(".git").exists() {
        return Err(format!("{} is not a git checkout", repo.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::Log;

    fn layout(name: &str) -> Layout {
        let p = std::env::temp_dir().join(format!("haul-recover-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        for d in Layout::new(&p).all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        Layout::new(p)
    }

    #[test]
    fn merging_two_logs_keeps_everything_once() {
        let a = "2026-08-24T00:00:01Z\tsample\tevals=1\n2026-08-24T00:00:02Z\tsample\tevals=2\n";
        let b = "2026-08-24T00:00:02Z\tsample\tevals=2\n2026-08-24T00:00:03Z\tsample\tevals=3\n";
        let (merged, added) = merge_records(a, b).unwrap();
        assert_eq!(added, 1, "the shared record must not be duplicated");
        assert_eq!(merged.lines().count(), 3);
        // and it is ordered
        let ts: Vec<i64> = Rec::parse_all(&merged).unwrap().iter().map(|r| r.ts).collect();
        let mut sorted = ts.clone();
        sorted.sort();
        assert_eq!(ts, sorted);
    }

    #[test]
    fn merging_is_idempotent() {
        // Recovery may be run twice — by a heartbeat and then by a human —
        // and the second run must change nothing.
        let a = "2026-08-24T00:00:01Z\tsample\tevals=1\n";
        let b = "2026-08-24T00:00:02Z\tsample\tevals=2\n";
        let (once, _) = merge_records(a, b).unwrap();
        let (twice, added) = merge_records(&once, b).unwrap();
        assert_eq!(once, twice);
        assert_eq!(added, 0);
    }

    #[test]
    fn a_mirror_restores_a_box_that_wrote_nothing_locally() {
        let src = layout("src");
        Log::shard(&src.journal_dir(), "deadbox", 1)
            .unwrap()
            .append(&Rec::at(1000, "run_start").f("cmd", "search"))
            .unwrap();
        let text = pack::pack(&src.state(), &src.root(), "deadbox").unwrap();

        let dst = layout("dst");
        let u = pack::unpack(&text).unwrap();
        let rep = apply(&dst, &u).unwrap();
        assert_eq!(rep.files_written, 1);
        let recs = crate::log::read_all(&dst.journal_dir()).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].kind, "run_start");
    }

    #[test]
    fn a_mirror_and_a_local_log_that_both_advanced_are_unioned() {
        // Two boxes, both alive for a while, neither's history lost.
        let src = layout("union-src");
        let s = Log::shard(&src.journal_dir(), "boxA", 1).unwrap();
        s.append(&Rec::at(1000, "sample").f("evals", 1)).unwrap();
        s.append(&Rec::at(2000, "sample").f("evals", 2)).unwrap();
        let text = pack::pack(&src.state(), &src.root(), "boxA").unwrap();

        let dst = layout("union-dst");
        let d = Log::shard(&dst.journal_dir(), "boxA", 1).unwrap();
        d.append(&Rec::at(1000, "sample").f("evals", 1)).unwrap();
        d.append(&Rec::at(3000, "sample").f("evals", 9)).unwrap();

        let rep = apply(&dst, &pack::unpack(&text).unwrap()).unwrap();
        assert_eq!(rep.files_merged, 1);
        assert_eq!(rep.records_added, 1);
        let recs = crate::log::read_all(&dst.journal_dir()).unwrap();
        assert_eq!(recs.len(), 3, "the union of both histories");
    }

    #[test]
    fn a_non_log_difference_is_reported_rather_than_overwritten() {
        let src = layout("bin-src");
        std::fs::write(src.frontier().join("best.bin"), [1u8, 2, 3]).unwrap();
        let text = pack::pack(&src.state(), &src.root(), "boxA").unwrap();
        let dst = layout("bin-dst");
        std::fs::write(dst.frontier().join("best.bin"), [9u8, 9, 9]).unwrap();
        let rep = apply(&dst, &pack::unpack(&text).unwrap()).unwrap();
        assert_eq!(rep.conflicts.len(), 1, "{rep:?}");
        assert_eq!(std::fs::read(dst.frontier().join("best.bin")).unwrap(), vec![9u8, 9, 9]);
    }
}
