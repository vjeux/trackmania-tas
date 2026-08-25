//! The supervisor: the thing that stays awake.
//!
//! The first durability failure of 2026-08-24 was a supervising *agent* that
//! settled while its child was still running, and an hour disappeared with
//! nobody watching. The fix is structural: liveness is an ordinary detached
//! OS process, not an agent's decision to keep its turn open. `tmhaul watch`
//! outlives the session that started it, and if the box takes it down with it,
//! the cron heartbeat notices from the repo — from committed state, not from
//! anybody's context.
//!
//! Each pass does exactly five things, in this order, and journals all of
//! them: sample the worker, fold the budget, evaluate alarms, bank on the
//! cadence, check the lease. Nothing here waits on a human.

use crate::alarms::Firing;
use crate::bank;
use crate::config::Job;
use crate::disk;
use crate::lease::{self, LeaseAction};
use crate::log::Log;
use crate::paths::Layout;
use crate::rec::Rec;
use crate::state;
use crate::worker;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub struct Options {
    pub node: String,
    pub lease_expires: Option<i64>,
    /// Stop after this many passes. 0 = run until the lease or a stop file.
    pub max_passes: u64,
    pub note: String,
}

/// What one pass concluded about the worker. Pure, so it can be tested
/// without a process: the supervisor's judgement is the part that has to be
/// right, and it must not need a six-hour run to check.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerVerdict {
    Alive { evals: u64, best: Option<f64> },
    /// Running, but has never written a progress record.
    AliveButSilent,
    Exited { code: i32 },
}

pub fn classify(
    alive: Option<i32>,
    progress: Option<(u64, Option<f64>, i64)>,
) -> WorkerVerdict {
    match (alive, progress) {
        (Some(code), _) => WorkerVerdict::Exited { code },
        (None, Some((evals, best, _))) => WorkerVerdict::Alive { evals, best },
        (None, None) => WorkerVerdict::AliveButSilent,
    }
}

pub struct Supervisor {
    pub l: Layout,
    pub job: Job,
    pub node: String,
    pub start: i64,
    pub jlog: Log,
    pub lease_expires: Option<i64>,
    firing: BTreeSet<&'static str>,
    last_bank: i64,
    last_evals: u64,
    restarts: i64,
}

impl Supervisor {
    pub fn new(l: Layout, job: Job, o: &Options) -> Result<Supervisor, String> {
        let start = crate::time::now();
        let jlog = Log::shard(&l.journal_dir(), &o.node, start).map_err(|e| e.to_string())?;
        lease::register(&l, &o.node, o.lease_expires, &o.note)?;
        Ok(Supervisor {
            l,
            job,
            node: o.node.clone(),
            start,
            jlog,
            lease_expires: o.lease_expires,
            firing: BTreeSet::new(),
            last_bank: 0,
            last_evals: 0,
            restarts: 0,
        })
    }

    fn journal(&self, r: &Rec) {
        if let Err(e) = self.jlog.append(r) {
            eprintln!("tmhaul: JOURNAL WRITE FAILED: {e}");
        }
    }

    fn progress_path(&self) -> PathBuf {
        PathBuf::from(&self.job.progress_file)
    }

    fn stop_path(&self) -> PathBuf {
        self.progress_path().with_file_name("STOP")
    }

    /// What the worker should resume from, read out of the **repo** — not out
    /// of the box's own progress file, which a fresh box does not have.
    pub fn last_evals(&self) -> u64 {
        self.last_evals
    }

    pub fn resume_point(&self) -> Result<(u64, Option<f64>), String> {
        let r = state::reconstruct(&self.l, crate::time::now())?;
        let evals = r.view.samples.iter().map(|s| s.evals).max().unwrap_or(0);
        Ok((evals, state::best_objective(&r.view)))
    }

    fn spawn_worker(&self) -> Result<Child, String> {
        let p = self.progress_path();
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let (evals, best) = self.resume_point()?;
        let logfile = p.with_file_name("worker.log");
        let out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&logfile)
            .map_err(|e| format!("{}: {e}", logfile.display()))?;
        let err = out.try_clone().map_err(|e| e.to_string())?;

        let child = Command::new("sh")
            .arg("-c")
            .arg(&self.job.worker_cmd)
            .current_dir(&self.job.worker_dir)
            .env("TMHAUL_PROGRESS", &self.job.progress_file)
            .env("TMHAUL_RESUME_EVALS", evals.to_string())
            .env("TMHAUL_RESUME_BEST", best.map(|b| b.to_string()).unwrap_or_default())
            .env("TMHAUL_NODE", &self.node)
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .map_err(|e| format!("spawn worker: {e}"))?;

        self.journal(
            &Rec::new("worker_start")
                .f("pid", child.id())
                .f("cmd", &self.job.worker_cmd)
                .f("resume_evals", evals)
                .f("resume_best", best.map(|b| b.to_string()).unwrap_or_else(|| "none".into())),
        );
        Ok(child)
    }

    fn bank_now(&mut self, why: &str) {
        let opts = bank::Options {
            message: format!("autopilot: {why} ({}, {})", self.node, crate::time::iso(crate::time::now())),
            mirror: bank::mirror_from_str(&self.job.mirror),
            mirror_dir: if self.job.mirror_dir.is_empty() {
                None
            } else {
                Some(PathBuf::from(&self.job.mirror_dir))
            },
            push: bank::push_from_str(&resolve_push(&self.job.push)),
            branch: self.job.branch.clone(),
        };
        match bank::bank(&self.l, &self.node, &opts) {
            Ok(r) => {
                self.journal(&Rec::new("bank").f("why", why).f("receipt", r.summary()));
                if let Some(e) = &r.mirror_error {
                    eprintln!("tmhaul: mirror failed: {e}");
                }
                if let Some(e) = &r.push_error {
                    eprintln!("tmhaul: push failed: {e}");
                }
            }
            Err(e) => {
                eprintln!("tmhaul: BANK FAILED: {e}");
                self.journal(&Rec::new("bank_failed").f("why", why).f("error", e));
            }
        }
        self.last_bank = crate::time::now();
    }

    fn evaluate_alarms(&mut self) {
        let now = crate::time::now();
        let fired: Vec<Firing> = match state::alarm_state(&self.l, now, &self.job.alarms) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("tmhaul: alarm evaluation failed: {e}");
                return;
            }
        };
        let ids: BTreeSet<&'static str> = fired.iter().map(|f| f.id).collect();
        let alog = Log::shard(&self.l.alarm_dir(), &self.node, self.start).ok();
        for f in &fired {
            if !self.firing.contains(f.id) {
                eprintln!("tmhaul: ALARM {} — {}", f.id, f.detail);
                if let Some(a) = &alog {
                    let _ = a.append(&f.to_rec());
                }
                self.journal(&Rec::new("alarm").f("id", f.id).f("detail", &f.detail));
            }
        }
        for was in self.firing.clone() {
            if !ids.contains(was) {
                if let Some(a) = &alog {
                    let _ = a.append(&Rec::new("alarm_clear").f("id", was));
                }
                self.journal(&Rec::new("alarm_clear").f("id", was));
            }
        }
        self.firing = ids;
    }

    /// Seed the eval baseline from the repo before the first sample.
    ///
    /// Without this the budget over-counts by the entire resume point every
    /// time a supervisor restarts: `last_evals` would start at zero, so the
    /// first sample's "delta" would be the whole cumulative counter. On a
    /// system whose normal mode of operation is *box dies, new box resumes*,
    /// that quietly inflates the one number the pre-committed switch condition
    /// is measured against.
    pub fn seed(&mut self) -> Result<(), String> {
        self.last_evals = self.resume_point()?.0;
        Ok(())
    }

    pub fn run(&mut self, o: &Options) -> Result<i32, String> {
        self.seed()?;
        let mut child = self.spawn_worker()?;
        self.journal(
            &Rec::new("run_start")
                .f("node", &self.node)
                .f("cmd", &self.job.worker_cmd)
                .f("map", &self.job.map_name)
                .f("rung", &self.job.rung),
        );
        self.bank_now("run start");

        let mut passes = 0u64;
        let mut stop_reason = "max passes".to_string();

        loop {
            std::thread::sleep(std::time::Duration::from_secs(self.job.sample_s.max(1) as u64));
            passes += 1;
            let now = crate::time::now();

            // ---- 1. sample
            let alive = child.try_wait().map_err(|e| e.to_string())?.map(|s| s.code().unwrap_or(-1));
            let progress = worker::read_progress(&self.progress_path()).unwrap_or_else(|e| {
                eprintln!("tmhaul: progress file unreadable: {e}");
                None
            });
            let verdict = classify(alive, progress);
            let free = disk::free_mb(&self.progress_path().parent().unwrap_or(Path::new("/tmp")))
                .unwrap_or(-1);

            let (evals, best) = match &verdict {
                WorkerVerdict::Alive { evals, best } => (*evals, *best),
                _ => (self.last_evals, None),
            };
            let worker_alive = !matches!(verdict, WorkerVerdict::Exited { .. });
            let mut sample = Rec::at(now, "sample")
                .f("evals", evals)
                .f("worker_alive", if worker_alive { 1 } else { 0 })
                .f("node", &self.node);
            if let Some(b) = best {
                sample.set("best", b);
            }
            if free >= 0 {
                sample.set("disk_free_mb", free);
            }
            self.journal(&sample);

            // ---- 2. budget: only intervals that moved the counter spend it
            let delta = evals.saturating_sub(self.last_evals);
            let _ = crate::budget::record(
                &self.l.budget_dir(),
                &self.node,
                &self.job.budget_key,
                delta,
                self.job.sample_s,
            );
            self.last_evals = evals;
            let _ = lease::touch(&self.l, &self.node);

            // ---- 3. alarms
            self.evaluate_alarms();

            // ---- 4. the worker itself
            if let WorkerVerdict::Exited { code } = verdict {
                self.journal(&Rec::new("worker_exit").f("code", code).f("restarts", self.restarts));
                if self.restarts >= self.job.restart_max {
                    stop_reason = format!("worker exited {code} and the restart budget is spent");
                    break;
                }
                self.restarts += 1;
                eprintln!("tmhaul: worker exited {code}; restart {} of {}", self.restarts, self.job.restart_max);
                std::thread::sleep(std::time::Duration::from_secs(
                    self.job.restart_backoff_s.max(0) as u64,
                ));
                child = self.spawn_worker()?;
            }

            // ---- 5. bank, lease, stop
            if now - self.last_bank >= self.job.bank_s {
                self.bank_now("periodic");
            }
            match lease::lease_action(now, self.lease_expires, self.job.lease_bank_lead_s) {
                LeaseAction::StandDown(why) => {
                    stop_reason = format!("lease: {why}");
                    break;
                }
                LeaseAction::Unknown | LeaseAction::Continue(_) => {}
            }
            if self.stop_path().exists() {
                stop_reason = "stop file".into();
                break;
            }
            if o.max_passes > 0 && passes >= o.max_passes {
                break;
            }
        }

        // ---- stand down: stop the worker, bank, verify, retire
        let _ = child.kill();
        let _ = child.wait();
        self.journal(&Rec::new("run_stop").f("why", &stop_reason).f("node", &self.node));
        self.bank_now("stand down");

        match bank::verify(&self.l, bank::Source::Committed) {
            Ok(bad) if bad.is_empty() => {
                self.journal(&Rec::new("verified").f("result", "every banked file matches its md5 in the commit"));
            }
            Ok(bad) => {
                self.journal(&Rec::new("verify_failed").f("files", bad.len()).f("detail", bad.join("; ")));
                eprintln!("tmhaul: VERIFY FAILED on {} file(s) — do NOT release this box", bad.len());
                return Ok(2);
            }
            Err(e) => {
                self.journal(&Rec::new("verify_failed").f("error", &e));
                return Ok(2);
            }
        }
        lease::retire(&self.l, &self.node, &stop_reason)?;
        // The retirement itself is state, so it has to be banked too.
        self.bank_now("retire");
        println!("tmhaul: stood down — {stop_reason}");
        Ok(0)
    }
}

/// `push = auto` picks the strongest route this box actually has.
///
/// No on-demand box holds a GitHub credential, so `direct` is usually not it;
/// the bridge to the render box is. Deciding at run time, rather than baking a
/// route into the config, is what lets a replacement box on different hardware
/// come up and bank without anyone editing anything.
/// `push = auto` picks the strongest route this box actually has.
///
/// Takes the home directory rather than reading `$HOME`, because its test
/// used to point the variable at a nonexistent path — and `set_var` is
/// process-global, so it leaked into every other test running in parallel and
/// made unrelated ones fail with a story about credentials. A function that
/// reads a global is a function whose test can only be written by mutating
/// one.
pub fn resolve_push_in(setting: &str, home: &str) -> String {
    if setting != "auto" {
        return setting.to_string();
    }
    if Path::new(&format!("{home}/bin/whitestick")).exists()
        && Path::new(&format!("{home}/.navi/credentials.json")).exists()
    {
        return "whitestick".into();
    }
    if Path::new(&format!("{home}/.git-credentials")).exists() {
        return "direct".into();
    }
    "none".into()
}

pub fn resolve_push(setting: &str) -> String {
    resolve_push_in(setting, &std::env::var("HOME").unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_live_worker_with_progress_is_alive() {
        assert_eq!(
            classify(None, Some((100, Some(25.0), 5))),
            WorkerVerdict::Alive { evals: 100, best: Some(25.0) }
        );
    }

    #[test]
    fn a_live_worker_with_no_progress_is_not_reported_as_zero_evals() {
        // If this returned `Alive { evals: 0 }` the difference between "has
        // not started counting" and "is counting, at zero" would vanish — and
        // that difference is exactly what today's failure turned on.
        assert_eq!(classify(None, None), WorkerVerdict::AliveButSilent);
    }

    #[test]
    fn an_exited_worker_beats_a_stale_progress_record() {
        // The dangerous ordering: the progress file still holds the last
        // record the dead process wrote. Death must win.
        assert_eq!(
            classify(Some(3), Some((100, Some(25.0), 5))),
            WorkerVerdict::Exited { code: 3 }
        );
    }

    #[test]
    fn auto_push_never_silently_resolves_to_a_route_that_does_not_exist() {
        assert_eq!(resolve_push("none"), "none");
        assert_eq!(resolve_push("whitestick"), "whitestick");
        // `auto` on a box with neither route must say `none` rather than
        // pretending: a push that no-ops is worse than one that is off.
        // Passed in, never set globally: this test used to mutate $HOME and
        // break unrelated tests running beside it.
        assert_eq!(resolve_push_in("auto", "/nonexistent-home-for-this-test"), "none");
    }
}

#[cfg(test)]
mod seed_tests {
    use super::*;
    use crate::log::Log;
    use crate::rec::Rec;

    #[test]
    fn a_restarted_supervisor_does_not_re_spend_the_whole_budget() {
        // The bug this catches was live for twenty minutes and inflated the
        // eval count by the resume point on every restart — 582 counted for
        // 308 actually done. The budget is the one number the pre-committed
        // switch condition turns on, so a quiet inflation of it is a decision
        // made for the wrong reason months later.
        let repo = std::env::temp_dir().join(format!("haul-seed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&repo);
        let l = Layout::new(&repo);
        for d in l.all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        let log = Log::shard(&l.journal_dir(), "deadbox", 1).unwrap();
        log.append(&Rec::at(1000, "run_start")).unwrap();
        log.append(&Rec::at(1100, "sample").f("evals", 194).f("best", 100).f("worker_alive", 1))
            .unwrap();

        let o = Options {
            node: "newbox".into(),
            lease_expires: None,
            max_passes: 1,
            note: "seed test".into(),
        };
        let mut sup = Supervisor::new(l, crate::config::Job::default(), &o).unwrap();
        assert_eq!(sup.last_evals(), 0, "before seeding");
        sup.seed().unwrap();
        assert_eq!(sup.last_evals(), 194, "the baseline must come from the repo");

        // Control: on a genuinely fresh project there is nothing to seed from,
        // and the baseline must be zero rather than something invented.
        let fresh = std::env::temp_dir().join(format!("haul-seed2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&fresh);
        let l2 = Layout::new(&fresh);
        for d in l2.all_dirs() {
            std::fs::create_dir_all(d).unwrap();
        }
        let mut sup2 = Supervisor::new(l2, crate::config::Job::default(), &o).unwrap();
        sup2.seed().unwrap();
        assert_eq!(sup2.last_evals(), 0);
    }
}
