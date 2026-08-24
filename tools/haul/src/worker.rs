//! A worker with a controllable failure mode.
//!
//! Two jobs, both load-bearing:
//!
//! * **It is the positive control for the whole harness.** Every alarm can be
//!   fired end to end — a real supervisor, a real process, real records on
//!   disk — rather than only in a unit test against a fixture. An alarm nobody
//!   has watched fire against a live process is still decoration.
//! * **It is the demonstration workload for recovery.** Kill it mid-flight and
//!   a fresh session must pick the run up from the repo and keep counting from
//!   where the banked state says it was. A synthetic counter makes that
//!   property checkable in seconds instead of hours.
//!
//! It is *not* the search, and nothing about the harness assumes it is. Any
//! process that honours the same contract works:
//!
//! * append `<iso8601>\tprogress\tevals=<cumulative>\tbest=<objective>` to
//!   `$TMHAUL_PROGRESS`, as often as it likes;
//! * start from `$TMHAUL_RESUME_EVALS` / `$TMHAUL_RESUME_BEST` when they are
//!   set, which is how a box dying costs nothing but the last banking window.

use crate::log::Log;
use crate::rec::Rec;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Counts up steadily and improves the objective now and then.
    Normal,
    /// Keeps writing samples with the counter frozen — today's real failure.
    Stall,
    /// Counts, but a fraction of the speed it was.
    Slow,
    /// Healthy throughput, objective pinned.
    Flat,
    /// Writes nothing at all, ever.
    Silent,
    /// Exits non-zero part way through.
    Crash,
}

pub fn mode_from_str(s: &str) -> Option<Mode> {
    Some(match s {
        "normal" => Mode::Normal,
        "stall" => Mode::Stall,
        "slow" => Mode::Slow,
        "flat" => Mode::Flat,
        "silent" => Mode::Silent,
        "crash" => Mode::Crash,
        _ => return None,
    })
}

pub struct Opts {
    pub progress: PathBuf,
    pub rate: u64,
    pub tick_ms: u64,
    pub duration_s: i64,
    pub mode: Mode,
    pub switch_after_s: i64,
}

/// Returns the exit code the process should use.
pub fn run(o: &Opts) -> i32 {
    let log = Log::at(&o.progress);
    let mut evals: u64 = std::env::var("TMHAUL_RESUME_EVALS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut best: f64 = std::env::var("TMHAUL_RESUME_BEST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);

    let started = crate::time::now();
    let deadline = started + o.duration_s;
    let mut ticks: u64 = 0;

    while crate::time::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(o.tick_ms));
        ticks += 1;
        let elapsed = crate::time::now() - started;
        let switched = o.switch_after_s > 0 && elapsed >= o.switch_after_s;

        match o.mode {
            Mode::Silent => continue,
            Mode::Crash if switched => {
                let _ = log.append(&Rec::new("worker_crash").f("why", "selftest crash mode"));
                return 3;
            }
            _ => {}
        }

        let per_tick = (o.rate * o.tick_ms) / 1000;
        let delta = match o.mode {
            Mode::Stall if switched => 0,
            Mode::Slow if switched => (per_tick / 20).max(1),
            _ => per_tick.max(1),
        };
        evals += delta;

        let improve = match o.mode {
            Mode::Flat => false,
            Mode::Stall if switched => false,
            _ => ticks % 10 == 0,
        };
        if improve {
            best += 1.0;
        }

        let _ = log.append(&Rec::new("progress").f("evals", evals).f("best", best));
    }
    let _ = log.append(&Rec::new("worker_done").f("evals", evals).f("best", best));
    0
}

/// The last progress record a worker wrote: `(evals, best, ts)`.
///
/// Missing file and empty file both read as `None` — the supervisor turns that
/// into a zero-throughput alarm, deliberately, rather than into a zero rate
/// that looks like data.
pub fn read_progress(path: &std::path::Path) -> Result<Option<(u64, Option<f64>, i64)>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let recs = Rec::parse_all(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(recs
        .iter()
        .rev()
        .find(|r| r.kind == "progress")
        .map(|r| (r.get_u64("evals").unwrap_or(0), r.get_f64("best"), r.ts)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpfile(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("haul-worker-{name}-{}.rec", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn a_normal_worker_advances_the_counter() {
        let p = tmpfile("normal");
        let code = run(&Opts {
            progress: p.clone(),
            rate: 1000,
            tick_ms: 50,
            duration_s: 1,
            mode: Mode::Normal,
            switch_after_s: 0,
        });
        assert_eq!(code, 0);
        let (evals, _, _) = read_progress(&p).unwrap().unwrap();
        assert!(evals > 0, "a normal worker must produce evals");
    }

    #[test]
    fn a_stalling_worker_keeps_writing_with_a_frozen_counter() {
        // This is the exact shape that fooled the old detector: records keep
        // arriving, so "is it alive?" says yes, while nothing is happening.
        let p = tmpfile("stall");
        run(&Opts {
            progress: p.clone(),
            rate: 1000,
            tick_ms: 50,
            duration_s: 2,
            mode: Mode::Stall,
            switch_after_s: 1,
        });
        let text = std::fs::read_to_string(&p).unwrap();
        let recs = Rec::parse_all(&text).unwrap();
        let evals: Vec<u64> = recs
            .iter()
            .filter(|r| r.kind == "progress")
            .map(|r| r.get_u64("evals").unwrap())
            .collect();
        assert!(evals.len() > 4);
        assert_eq!(
            evals.last(),
            evals.iter().max(),
            "the counter must plateau, not go backwards"
        );
        let tail = &evals[evals.len() / 2..];
        assert!(tail.iter().all(|e| e == tail.last().unwrap()), "the tail must be frozen");
    }

    #[test]
    fn a_worker_resumes_from_the_environment() {
        // The property that makes a dead box cheap: work already banked is not
        // done again.
        let p = tmpfile("resume");
        std::env::set_var("TMHAUL_RESUME_EVALS", "1000000");
        std::env::set_var("TMHAUL_RESUME_BEST", "31");
        run(&Opts {
            progress: p.clone(),
            rate: 100,
            tick_ms: 50,
            duration_s: 1,
            mode: Mode::Normal,
            switch_after_s: 0,
        });
        std::env::remove_var("TMHAUL_RESUME_EVALS");
        std::env::remove_var("TMHAUL_RESUME_BEST");
        let (evals, best, _) = read_progress(&p).unwrap().unwrap();
        assert!(evals > 1_000_000, "resumed run restarted from zero: {evals}");
        assert!(best.unwrap() >= 31.0);
    }

    #[test]
    fn a_missing_progress_file_is_none_and_not_a_zero() {
        let p = tmpfile("absent");
        assert_eq!(read_progress(&p).unwrap(), None);
    }

    #[test]
    fn a_crashing_worker_exits_non_zero() {
        let p = tmpfile("crash");
        let code = run(&Opts {
            progress: p,
            rate: 100,
            tick_ms: 50,
            duration_s: 3,
            mode: Mode::Crash,
            switch_after_s: 1,
        });
        assert_eq!(code, 3);
    }
}
