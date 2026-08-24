//! Reconstructing the whole picture **from the repo alone**.
//!
//! This module is the answer to the requirement that a woken session, on a box
//! that has never seen this project, with no conversational context and no
//! memory of what it was doing, can work out what is going on. Every input is
//! a committed file; nothing is read from a process, an environment variable
//! or an agent's context.

use crate::alarms::{self, BoxState, QueueView, Sample, View};
use crate::lease;
use crate::log;
use crate::paths::Layout;
use crate::queue::Queue;
use crate::rec::Rec;

pub struct Reconstructed {
    pub view: View,
    pub journal: Vec<Rec>,
    pub run_started: Option<i64>,
    pub last_bank_receipt: Option<String>,
}

pub fn reconstruct(l: &Layout, now: i64) -> Result<Reconstructed, String> {
    let journal = log::read_all(&l.journal_dir())?;

    // A run is active if the newest run_start is newer than the newest
    // run_stop. Absence of a stop is *not* taken as "still running" on its own
    // — a box that vanished never wrote one, and that case is what
    // `box_vanished` is for.
    let last_start = journal.iter().rev().find(|r| r.kind == "run_start").map(|r| r.ts);
    let last_stop = journal.iter().rev().find(|r| r.kind == "run_stop").map(|r| r.ts);
    let run_active = match (last_start, last_stop) {
        (Some(s), Some(t)) => s > t,
        (Some(_), None) => true,
        _ => false,
    };

    let samples: Vec<Sample> = journal
        .iter()
        .filter(|r| r.kind == "sample")
        .map(|r| Sample {
            ts: r.ts,
            // A stable numeric id for the writing box, hashed from its name:
            // the alarms only ever compare nodes for equality, and carrying a
            // String through a Copy struct would cost more than it says.
            node: {
                let n = r.get("node").unwrap_or("");
                u64::from_str_radix(&crate::md5::md5_hex(n.as_bytes())[..8], 16).unwrap_or(0)
            },
            evals: r.get_u64("evals").unwrap_or(0),
            best: r.get_f64("best"),
            disk_free_mb: r.get_i64("disk_free_mb"),
            worker_alive: r.get("worker_alive").map(|v| v == "1").unwrap_or(false),
        })
        .collect();

    let boxes: Vec<(String, BoxState)> = lease::all(l)?
        .into_iter()
        .map(|b| (b.node.clone(), BoxState { last_seen: b.last_seen, active: !b.retired }))
        .collect();

    let q = Queue::open(l).map_err(|e| e.to_string())?;
    let done = q.done()?;
    let queue = QueueView {
        pending: q.pending()?.len(),
        claimed: q.claimed()?.len(),
        expired_claims: q.expired_count(now)?,
        last_completion: done.iter().map(|i| i.created).max(),
    };

    let last_bank_rec = journal.iter().rev().find(|r| r.kind == "bank").cloned();
    let view = View {
        now,
        run_active,
        run_started: if run_active { last_start } else { None },
        samples,
        boxes,
        queue,
        last_bank: last_bank_rec.as_ref().map(|r| r.ts),
        // The newest sample that carried one. It is a property of the RUN, not
        // of the instant, so a worker that reports it once at startup is
        // enough — but a run that never reports it at all is a firing state,
        // not a silent one.
        start_dev_m: journal
            .iter()
            .rev()
            .filter(|r| r.kind == "sample" || r.kind == "run_start")
            .find_map(|r| r.get_f64("start_dev_m")),
    };

    Ok(Reconstructed {
        view,
        journal,
        run_started: last_start,
        last_bank_receipt: last_bank_rec
            .as_ref()
            .and_then(|r| r.get("receipt"))
            .map(|s| s.to_string()),
    })
}

/// The most recent throughput reading, evals/s, over the last `window_s`.
pub fn recent_rate(v: &View, window_s: i64) -> Option<f64> {
    let w: Vec<&Sample> = v.samples.iter().filter(|s| s.ts >= v.now - window_s).collect();
    let (first, last) = (w.first()?, w.last()?);
    let dt = (last.ts - first.ts) as f64;
    if dt <= 0.0 {
        return None;
    }
    Some(last.evals.saturating_sub(first.evals) as f64 / dt)
}

pub fn best_objective(v: &View) -> Option<f64> {
    v.samples.iter().filter_map(|s| s.best).fold(None, |acc: Option<f64>, b| {
        Some(match acc {
            Some(a) if a >= b => a,
            _ => b,
        })
    })
}

pub fn alarm_state(l: &Layout, now: i64, cfg: &alarms::Config) -> Result<Vec<alarms::Firing>, String> {
    let r = reconstruct(l, now)?;
    Ok(alarms::evaluate(&r.view, cfg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::Log;

    fn layout(name: &str) -> Layout {
        let p = std::env::temp_dir().join(format!("haul-state-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        for d in Layout::new(&p).all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        Layout::new(p)
    }

    #[test]
    fn a_run_that_started_and_never_stopped_reads_as_active() {
        let l = layout("active");
        let log = Log::shard(&l.journal_dir(), "boxA", 1).unwrap();
        log.append(&Rec::at(1000, "run_start")).unwrap();
        assert!(reconstruct(&l, 2000).unwrap().view.run_active);
        log.append(&Rec::at(1500, "run_stop")).unwrap();
        assert!(!reconstruct(&l, 2000).unwrap().view.run_active);
    }

    #[test]
    fn the_whole_picture_comes_back_from_files_written_by_a_box_that_is_gone() {
        // This is the recovery property, tested without a box: write what a
        // dead box would have left behind, then reconstruct.
        let l = layout("recover");
        let log = Log::shard(&l.journal_dir(), "deadbox", 1).unwrap();
        log.append(&Rec::at(1000, "run_start").f("cmd", "tmsearch ...")).unwrap();
        for i in 0..10 {
            log.append(
                &Rec::at(1000 + i * 60, "sample")
                    .f("evals", i as u64 * 600)
                    .f("best", 20 + i)
                    .f("disk_free_mb", 100_000)
                    .f("worker_alive", 1),
            )
            .unwrap();
        }
        log.append(&Rec::at(1500, "bank").f("receipt", "commit abc123 · mirror P1")).unwrap();
        lease::register_at(&l, "deadbox", 1500, Some(9999), "the box that died").unwrap();

        let r = reconstruct(&l, 1600).unwrap();
        assert!(r.view.run_active);
        assert_eq!(r.view.samples.len(), 10);
        assert_eq!(best_objective(&r.view), Some(29.0));
        assert_eq!(r.last_bank_receipt.as_deref(), Some("commit abc123 · mirror P1"));
        assert_eq!(recent_rate(&r.view, 3600), Some(10.0));
        assert_eq!(r.view.boxes.len(), 1);
    }

    #[test]
    fn an_empty_repo_reconstructs_to_an_idle_system_and_says_so() {
        let l = layout("empty");
        let r = reconstruct(&l, 100).unwrap();
        assert!(!r.view.run_active);
        assert!(r.view.samples.is_empty());
        // And crucially: an idle system is not an alarming one.
        assert_eq!(
            alarms::evaluate(&r.view, &alarms::Config::default())
                .iter()
                .map(|f| f.id)
                .collect::<Vec<_>>(),
            vec!["unbanked_drift"],
            "nothing banked yet is the one true thing to say about an empty repo"
        );
    }
}
