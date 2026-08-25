//! `tmhaul beat` — what a woken session reads.
//!
//! The heartbeat wakes an agent that may have no memory of this project at
//! all: a new session, a restarted orchestrator, a context that was compacted
//! away. So this command answers, from committed files only, the four
//! questions such a session actually has — *what is this, what is happening,
//! what is wrong, what should I do now* — and every answer it gives is a
//! command it can run.
//!
//! It deliberately does **not** rely on: the agent's context, environment
//! variables set by whoever started the run, a process on this box, or a
//! previous session's notes. Those are all things that will be gone.

use crate::alarms::Severity;
use crate::bank;
use crate::config::Job;
use crate::lease;
use crate::paths::Layout;
use crate::queue::Queue;
use crate::state;
use crate::time::{dur, iso};

pub struct Beat {
    pub text: String,
    pub actions: Vec<String>,
    pub critical: bool,
}

pub fn brief(l: &Layout, job: &Job, now: i64, watch_alive: Option<u32>) -> Result<Beat, String> {
    let mut r = state::reconstruct(l, now)?;
    r.view = state::with_credential(r.view);
    let v = &r.view;
    let fired = crate::alarms::evaluate(v, &job.alarms);
    let counters = crate::budget::total_for(&l.budget_dir(), Some(&job.budget_key))?;
    let boxes = lease::all(l)?;
    let q = Queue::open(l).map_err(|e| e.to_string())?;
    let node = crate::paths::node_id();

    let mut s = String::new();
    let mut actions: Vec<String> = Vec::new();

    s.push_str(&format!(
        "TM2020 autopilot — heartbeat at {}\n\
         =================================================\n\n\
         What this is: an autonomous search for an input tape that beats the author\n\
         time of {} without ever consulting a human ghost. Rung: {}.\n\
         The repo is the state of record; this briefing was reconstructed from\n\
         `autopilot/state/` in {} and from nothing else.\n\n",
        iso(now),
        job.map_name,
        job.rung,
        l.repo.display()
    ));

    // ---- is anything running?
    let newest = v.samples.last();
    s.push_str("RUN\n");
    match (v.run_active, newest) {
        (false, _) => s.push_str("  No run is marked active.\n"),
        (true, None) => s.push_str("  A run is marked active and has never reported a sample.\n"),
        (true, Some(sm)) => s.push_str(&format!(
            "  Active. Last sample {} ago: {} evals, objective {}.\n  Recent throughput: {}\n",
            dur(now - sm.ts),
            sm.evals,
            sm.best.map(|b| b.to_string()).unwrap_or_else(|| "not reported".into()),
            state::recent_rate(v, job.alarms.collapse_recent_s)
                .map(|x| format!("{x:.2} evals/s"))
                .unwrap_or_else(|| "not measurable".into())
        )),
    }
    s.push_str(&format!(
        "  Supervisor on this box ({node}): {}\n",
        match watch_alive {
            Some(pid) => format!("running, pid {pid}"),
            None => "NOT RUNNING".to_string(),
        }
    ));
    s.push_str(&format!(
        "  Last bank: {}\n\n",
        v.last_bank
            .map(|t| format!("{} ago — {}", dur(now - t), r.last_bank_receipt.clone().unwrap_or_default()))
            .unwrap_or_else(|| "NEVER".into())
    ));

    // ---- budget
    s.push_str(&format!(
        "BUDGET (work, not wall-clock)\n  {} evals · {} productive · {} stalled · {:.1}% of the switch threshold{}\n\n",
        counters.evals,
        dur(counters.productive_s),
        dur(counters.stalled_s),
        100.0 * counters.spent_fraction(&job.budget),
        if counters.switch_reached(&job.budget) {
            " — THRESHOLD REACHED, the pre-committed switch is due (DESIGN.md §3.2)"
        } else {
            ""
        }
    ));

    // ---- alarms
    s.push_str("ALARMS\n");
    if fired.is_empty() {
        s.push_str("  none firing\n\n");
    } else {
        for f in &fired {
            s.push_str(&format!(
                "  [{}] {} — {}\n",
                match f.severity {
                    Severity::Critical => "CRITICAL",
                    Severity::Warn => "warn",
                },
                f.id,
                f.detail
            ));
        }
        s.push('\n');
    }

    // ---- boxes
    s.push_str("BOXES\n");
    if boxes.is_empty() {
        s.push_str("  none registered\n");
    }
    for b in &boxes {
        s.push_str(&format!(
            "  {} — {} · lease {} · last seen {} ago\n",
            b.node,
            if b.retired { "retired" } else { "ACTIVE" },
            b.lease_expires
                .map(|e| if e > now { format!("{} left", dur(e - now)) } else { "EXPIRED".into() })
                .unwrap_or_else(|| "not declared".into()),
            dur(now - b.last_seen)
        ));
    }
    s.push('\n');

    // ---- queue
    let expired = q.expired_count(now)?;
    s.push_str(&format!(
        "QUEUE\n  {} pending · {} claimed ({} expired) · {} done\n\n",
        q.pending()?.len(),
        q.claimed()?.len(),
        expired,
        q.done()?.len()
    ));

    // ---- what to do
    if expired > 0 {
        actions.push(format!(
            "tmhaul queue reap   # {expired} claim(s) held by a box that is gone"
        ));
    }
    let box_here_active = boxes.iter().any(|b| b.node == node && !b.retired);
    if watch_alive.is_none() {
        if box_here_active || !v.run_active {
            actions.push(
                "tmhaul watch --detach   # nothing is supervising this box; start it".to_string(),
            );
        } else {
            actions.push(
                "tmhaul recover   # the run belongs to a box that is gone; take it over here"
                    .to_string(),
            );
        }
    }
    for f in &fired {
        match f.id {
            "zero_throughput" => actions.push(
                "tail -50 $(dirname $(tmhaul config get progress_file))/worker.log   # the worker is producing nothing; read what it is saying".into(),
            ),
            "box_vanished" => actions.push(
                "provision a replacement box, then on it: tmhaul recover && tmhaul watch --detach".into(),
            ),
            "unbanked_drift" => actions.push("tmhaul bank --why heartbeat".into()),
            "disk_filling" => actions.push("free space on the worker volume, then tmhaul bank".into()),
            _ => {}
        }
    }
    if actions.is_empty() {
        actions.push("nothing — the run is healthy; regenerate the status page and go back to sleep".into());
    }
    actions.push("tmhaul status --write && tmhaul bank --why heartbeat".into());

    s.push_str("DO NOW\n");
    for a in &actions {
        s.push_str(&format!("  $ {a}\n"));
    }
    s.push('\n');
    s.push_str(&format!(
        "If this box is unusable, everything above is recoverable elsewhere:\n  \
         git clone https://github.com/vjeux/trackmania-tas.git /tmp/tmtas\n  \
         cd /tmp/tmtas/tools && cargo build --release -p haul\n  \
         ./target/release/tmhaul recover      # pulls the newest '{}' mirror and verifies it\n",
        bank::MIRROR_TITLE_PREFIX
    ));

    let critical = fired.iter().any(|f| f.severity == Severity::Critical);
    Ok(Beat { text: s, actions, critical })
}

/// Is a `tmhaul watch` running on this box? Read from `/proc`, so it is a fact
/// about the machine rather than a claim in a file that a crash could have
/// left behind.
pub fn watch_pid() -> Option<u32> {
    let me = std::process::id();
    let dir = std::fs::read_dir("/proc").ok()?;
    for e in dir.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(pid) = name.parse::<u32>() else { continue };
        if pid == me {
            continue;
        }
        let Ok(cmdline) = std::fs::read(e.path().join("cmdline")) else { continue };
        let parts: Vec<String> = cmdline
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).to_string())
            .collect();
        if parts.len() >= 2
            && parts[0].ends_with("tmhaul")
            && parts.iter().any(|p| p == "watch")
        {
            return Some(pid);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::Log;
    use crate::rec::Rec;

    fn layout(name: &str) -> Layout {
        let p = std::env::temp_dir().join(format!("haul-beat-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        for d in Layout::new(&p).all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        Layout::new(p)
    }

    #[test]
    fn a_woken_session_with_no_context_is_told_what_to_do() {
        let l = layout("cold");
        let b = brief(&l, &Job::default(), 1_800_000_000, None).unwrap();
        assert!(b.text.contains("state of record"));
        assert!(b.text.contains("Summer 2026 - 01"), "the map must be named, not numbered");
        assert!(!b.actions.is_empty());
        assert!(
            b.text.contains("git clone"),
            "the briefing must carry the recovery route for a box that cannot be used"
        );
    }

    #[test]
    fn a_dead_run_on_a_vanished_box_asks_to_be_taken_over_not_restarted_blindly() {
        let l = layout("takeover");
        let now = 1_800_000_000;
        let log = Log::shard(&l.journal_dir(), "deadbox", 1).unwrap();
        log.append(&Rec::at(now - 20_000, "run_start")).unwrap();
        log.append(&Rec::at(now - 20_000, "sample").f("evals", 10).f("worker_alive", 1)).unwrap();
        lease::register_at(&l, "deadbox", now - 20_000, Some(now - 100), "gone").unwrap();
        let b = brief(&l, &Job::default(), now, None).unwrap();
        assert!(b.critical, "a vanished box holding an active run is critical");
        assert!(
            b.actions.iter().any(|a| a.contains("recover")),
            "{:?}",
            b.actions
        );
    }

    #[test]
    fn a_healthy_run_is_told_to_go_back_to_sleep() {
        let l = layout("healthy");
        let now = 1_800_000_000;
        let log = Log::shard(&l.journal_dir(), "boxA", 1).unwrap();
        log.append(&Rec::at(now - 3600, "run_start")).unwrap();
        for i in 0..=60 {
            log.append(
                &Rec::at(now - 3600 + i * 60, "sample")
                    .f("evals", i as u64 * 600)
                    .f("best", 20 + i)
                    .f("disk_free_mb", 500_000)
                    .f("worker_alive", 1),
            )
            .unwrap();
        }
        log.append(&Rec::at(now - 60, "bank").f("receipt", "commit abc · mirror P1")).unwrap();
        lease::register_at(&l, "boxA", now - 60, Some(now + 10_000), "fine").unwrap();
        let b = brief(&l, &Job::default(), now, Some(4242)).unwrap();
        assert!(!b.critical, "{}", b.text);
        assert!(b.text.contains("pid 4242"));
        assert!(b.actions[0].contains("nothing"), "{:?}", b.actions);
    }
}
