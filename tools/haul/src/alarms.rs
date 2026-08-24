//! Alarms, each shaped like a real failure this project has actually had, and
//! each with a test that makes it fire.
//!
//! The bug class this module exists for has now been hit four times: **a check
//! that passes while doing nothing.** The sharpest instance was a stall
//! detector that watched "furthest station not improving across a 2M-eval
//! window" — which cannot fire when there are no evals, so three runs at zero
//! evals per second looked healthy for an hour.
//!
//! Two structural decisions follow from that, and they are the whole design:
//!
//! * **Zero throughput and no progress are different alarms.** They have
//!   different predicates and different tests. Neither is allowed to stand in
//!   for the other.
//! * **Absence of evidence is evidence.** If a run is marked active and *no
//!   samples arrive at all*, that is zero throughput, not "no data yet". A
//!   detector whose window can be empty must decide what empty means, out
//!   loud, or it will quietly mean "fine".
//!
//! Every alarm is a pure function of a `View`, so its test needs no box, no
//! engine and no search — which is why every one of them has a test that fires
//! it and a control that keeps it silent.

use crate::rec::Rec;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub ts: i64,
    /// Cumulative eval counter, monotonic per run.
    pub evals: u64,
    /// The objective the search is climbing — furthest station on our own
    /// route. Higher is better. `None` when the worker has not reported one.
    pub best: Option<f64>,
    pub disk_free_mb: Option<i64>,
    pub worker_alive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxState {
    /// Last time this box wrote anything at all.
    pub last_seen: i64,
    pub active: bool,
}

#[derive(Debug, Clone, Default)]
pub struct QueueView {
    pub pending: usize,
    pub claimed: usize,
    /// Claims whose lease has already run out — the box that held them is
    /// presumed gone.
    pub expired_claims: usize,
    pub last_completion: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct View {
    pub now: i64,
    pub run_active: bool,
    pub samples: Vec<Sample>,
    pub boxes: Vec<(String, BoxState)>,
    pub queue: QueueView,
    pub last_bank: Option<i64>,
}

impl View {
    pub fn empty(now: i64) -> View {
        View {
            now,
            run_active: false,
            samples: Vec::new(),
            boxes: Vec::new(),
            queue: QueueView::default(),
            last_bank: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    /// A run that reports nothing for this long is stalled.
    pub zero_window_s: i64,
    /// Recent window whose rate is compared against the trailing baseline.
    pub collapse_recent_s: i64,
    /// Trailing baseline window, ending where `collapse_recent_s` begins.
    pub collapse_baseline_s: i64,
    /// Fire when the recent rate is below this fraction of the baseline.
    pub collapse_frac: f64,
    /// Minimum baseline rate (evals/s) worth comparing against.
    pub collapse_min_baseline: f64,
    /// Evals of healthy throughput with a flat objective before we complain.
    pub no_progress_evals: u64,
    /// A box silent for this long has vanished.
    pub box_silence_s: i64,
    /// Pending work with no completion for this long is a stuck queue.
    pub queue_window_s: i64,
    /// Below this much free disk, fire immediately.
    pub disk_min_free_mb: i64,
    /// Fire if the disk trend projects zero free within this horizon.
    pub disk_horizon_s: i64,
    /// Local work not pushed anywhere durable for this long is at risk.
    pub bank_max_gap_s: i64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            zero_window_s: 600,
            collapse_recent_s: 600,
            collapse_baseline_s: 3600,
            collapse_frac: 0.25,
            collapse_min_baseline: 0.5,
            no_progress_evals: 2_000_000,
            box_silence_s: 1_800,
            queue_window_s: 7_200,
            disk_min_free_mb: 5_000,
            disk_horizon_s: 6 * 3600,
            bank_max_gap_s: 3_600,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warn,
    Critical,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Firing {
    pub id: &'static str,
    pub severity: Severity,
    pub detail: String,
}

impl Firing {
    pub fn to_rec(&self) -> Rec {
        Rec::new("alarm")
            .f("id", self.id)
            .f(
                "severity",
                match self.severity {
                    Severity::Warn => "warn",
                    Severity::Critical => "critical",
                },
            )
            .f("detail", &self.detail)
    }
}

fn samples_in(v: &View, since: i64) -> Vec<&Sample> {
    v.samples.iter().filter(|s| s.ts >= since).collect()
}

/// evals/s across a window, from its first and last sample. `None` when the
/// window does not contain two samples to measure between.
fn rate_over(v: &View, from: i64, to: i64) -> Option<f64> {
    let w: Vec<&Sample> = v.samples.iter().filter(|s| s.ts >= from && s.ts <= to).collect();
    let (first, last) = (w.first()?, w.last()?);
    let dt = (last.ts - first.ts) as f64;
    if dt <= 0.0 {
        return None;
    }
    Some(last.evals.saturating_sub(first.evals) as f64 / dt)
}

// ---------------------------------------------------------------- A1

/// **Zero throughput.** The eval counter has not moved — including the case
/// where nothing has been reported at all, which is the one that got us.
pub fn zero_throughput(v: &View, c: &Config) -> Option<Firing> {
    if !v.run_active {
        return None;
    }
    let since = v.now - c.zero_window_s;
    let w = samples_in(v, since);

    if w.is_empty() {
        // A run that is supposed to be running and is saying nothing at all is
        // producing nothing at all. This branch is the entire point of the
        // alarm: the old detector's window could be empty and empty read as
        // healthy.
        let last = v.samples.last().map(|s| s.ts);
        return Some(Firing {
            id: "zero_throughput",
            severity: Severity::Critical,
            detail: match last {
                Some(t) => format!(
                    "run active, no progress sample for {} (last at {})",
                    crate::time::dur(v.now - t),
                    crate::time::iso(t)
                ),
                None => "run active, never reported a single progress sample".to_string(),
            },
        });
    }

    // Sparse but present: measure against the last sample before the window
    // too, so one lonely sample inside the window still yields a delta.
    let base = v
        .samples
        .iter()
        .rev()
        .find(|s| s.ts < since)
        .or_else(|| w.first().copied());
    let (Some(base), Some(last)) = (base, w.last()) else { return None };
    if last.ts - base.ts < c.zero_window_s {
        return None; // not enough elapsed history to judge yet
    }
    if last.evals > base.evals {
        return None;
    }
    Some(Firing {
        id: "zero_throughput",
        severity: Severity::Critical,
        detail: format!(
            "eval counter stuck at {} for {}",
            last.evals,
            crate::time::dur(last.ts - base.ts)
        ),
    })
}

// ---------------------------------------------------------------- A2

/// **Throughput collapse.** Still moving, but far slower than it was — judged
/// against its own recent baseline, not an absolute number nobody can set.
///
/// Suppressed while `zero_throughput` is firing: that is the same episode, and
/// two alarms for one event trains people to ignore both.
pub fn throughput_collapse(v: &View, c: &Config) -> Option<Firing> {
    if !v.run_active || zero_throughput(v, c).is_some() {
        return None;
    }
    let recent_from = v.now - c.collapse_recent_s;
    let base_from = recent_from - c.collapse_baseline_s;
    let recent = rate_over(v, recent_from, v.now)?;
    let baseline = rate_over(v, base_from, recent_from)?;
    if baseline < c.collapse_min_baseline {
        return None;
    }
    if recent >= baseline * c.collapse_frac {
        return None;
    }
    Some(Firing {
        id: "throughput_collapse",
        severity: Severity::Warn,
        detail: format!(
            "{recent:.2} evals/s against a baseline of {baseline:.2} ({:.0}% of it)",
            100.0 * recent / baseline
        ),
    })
}

// ---------------------------------------------------------------- A3

/// **No progress despite healthy throughput.** The search is working hard and
/// getting nowhere — a real and different condition from a stall, and the only
/// one the old detector was ever able to see.
pub fn no_progress(v: &View, c: &Config) -> Option<Firing> {
    if !v.run_active || zero_throughput(v, c).is_some() {
        return None;
    }
    let last = v.samples.last()?;
    let best_now = last.best?;
    // Walk back to the sample `no_progress_evals` ago.
    let target = last.evals.checked_sub(c.no_progress_evals)?;
    let start = v.samples.iter().rev().find(|s| s.evals <= target)?;
    if start.best.map(|b| b < best_now).unwrap_or(true) {
        return None; // it improved somewhere in the window
    }
    if let Some(improved) =
        v.samples.iter().find(|s| s.ts > start.ts && s.best.map(|b| b > best_now).unwrap_or(false))
    {
        let _ = improved;
        return None;
    }
    Some(Firing {
        id: "no_progress",
        severity: Severity::Warn,
        detail: format!(
            "{} evals with the objective flat at {best_now}",
            last.evals - start.evals
        ),
    })
}

// ---------------------------------------------------------------- A4

/// **The worker process died.**
pub fn worker_died(v: &View, _c: &Config) -> Option<Firing> {
    if !v.run_active {
        return None;
    }
    let last = v.samples.last()?;
    if last.worker_alive {
        return None;
    }
    Some(Firing {
        id: "worker_died",
        severity: Severity::Critical,
        detail: format!("run marked active but no worker process as of {}", crate::time::iso(last.ts)),
    })
}

// ---------------------------------------------------------------- A5

/// **A box vanished.** Leases expire, machines get reclaimed, and the platform
/// does not tell the work about it.
pub fn box_vanished(v: &View, c: &Config) -> Option<Firing> {
    let gone: Vec<String> = v
        .boxes
        .iter()
        .filter(|(_, b)| b.active && v.now - b.last_seen > c.box_silence_s)
        .map(|(n, b)| format!("{n} (silent {})", crate::time::dur(v.now - b.last_seen)))
        .collect();
    if gone.is_empty() {
        return None;
    }
    Some(Firing {
        id: "box_vanished",
        severity: Severity::Critical,
        detail: gone.join(", "),
    })
}

// ---------------------------------------------------------------- A6

/// **The queue is not draining.** Either nothing is finishing, or claims are
/// expiring because whoever held them is gone.
pub fn queue_stalled(v: &View, c: &Config) -> Option<Firing> {
    if v.queue.expired_claims > 0 {
        return Some(Firing {
            id: "queue_stalled",
            severity: Severity::Warn,
            detail: format!("{} claim(s) expired and need reaping", v.queue.expired_claims),
        });
    }
    if v.queue.pending == 0 {
        return None;
    }
    let since = match v.queue.last_completion {
        Some(t) => v.now - t,
        None => c.queue_window_s + 1, // never completed anything, ever
    };
    if since <= c.queue_window_s {
        return None;
    }
    Some(Firing {
        id: "queue_stalled",
        severity: Severity::Warn,
        detail: format!(
            "{} pending, nothing completed for {}",
            v.queue.pending,
            crate::time::dur(since)
        ),
    })
}

// ---------------------------------------------------------------- A7

/// **Disk filling.** Both the cliff and the slope: a long-haul run dies of a
/// full disk days before anyone would have looked.
pub fn disk_filling(v: &View, c: &Config) -> Option<Firing> {
    let with_disk: Vec<&Sample> = v.samples.iter().filter(|s| s.disk_free_mb.is_some()).collect();
    let last = with_disk.last()?;
    let free = last.disk_free_mb?;
    if free < c.disk_min_free_mb {
        return Some(Firing {
            id: "disk_filling",
            severity: Severity::Critical,
            detail: format!("{free} MB free, below the {} MB floor", c.disk_min_free_mb),
        });
    }
    let first = with_disk.first()?;
    let dt = (last.ts - first.ts) as f64;
    if dt < 60.0 {
        return None;
    }
    let drop = (first.disk_free_mb? - free) as f64;
    if drop <= 0.0 {
        return None;
    }
    let per_s = drop / dt;
    let eta = free as f64 / per_s;
    if eta > c.disk_horizon_s as f64 {
        return None;
    }
    Some(Firing {
        id: "disk_filling",
        severity: Severity::Warn,
        detail: format!(
            "{free} MB free, falling {:.1} MB/min — empty in {}",
            per_s * 60.0,
            crate::time::dur(eta as i64)
        ),
    })
}

// ---------------------------------------------------------------- A8

/// **Work that exists only on a box.** A box can disappear at any moment; the
/// only work that survives is work that has been banked off it.
pub fn unbanked_drift(v: &View, c: &Config) -> Option<Firing> {
    let since = match v.last_bank {
        Some(t) => v.now - t,
        None => return Some(Firing {
            id: "unbanked_drift",
            severity: Severity::Critical,
            detail: "nothing has ever been banked from this run".to_string(),
        }),
    };
    if since <= c.bank_max_gap_s {
        return None;
    }
    Some(Firing {
        id: "unbanked_drift",
        severity: Severity::Critical,
        detail: format!("last bank was {} ago", crate::time::dur(since)),
    })
}

// ----------------------------------------------------------------

pub type AlarmFn = fn(&View, &Config) -> Option<Firing>;

pub const ALL: &[(&str, AlarmFn)] = &[
    ("zero_throughput", zero_throughput),
    ("throughput_collapse", throughput_collapse),
    ("no_progress", no_progress),
    ("worker_died", worker_died),
    ("box_vanished", box_vanished),
    ("queue_stalled", queue_stalled),
    ("disk_filling", disk_filling),
    ("unbanked_drift", unbanked_drift),
];

pub fn evaluate(v: &View, c: &Config) -> Vec<Firing> {
    ALL.iter().filter_map(|(_, f)| f(v, c)).collect()
}

// ---------------------------------------------------------------- fixtures
//
// These build the states each alarm is supposed to see. They are `pub` so
// that `tmhaul alarms selftest` can fire every alarm *on the operator's own
// box, at run time* — a test that only ever ran in CI is one more thing
// nobody has watched work.

pub mod fixtures {
    use super::*;

    pub const NOW: i64 = 1_800_000_000;

    /// A run doing exactly what it should: 10 evals/s, objective climbing,
    /// worker alive, plenty of disk, banked minutes ago.
    pub fn healthy() -> View {
        let mut samples = Vec::new();
        for i in 0..=120 {
            samples.push(Sample {
                ts: NOW - 7200 + i * 60,
                evals: (i as u64) * 600,
                best: Some(10.0 + i as f64),
                disk_free_mb: Some(200_000),
                worker_alive: true,
            });
        }
        View {
            now: NOW,
            run_active: true,
            samples,
            boxes: vec![("boxA".into(), BoxState { last_seen: NOW - 30, active: true })],
            queue: QueueView {
                pending: 3,
                claimed: 1,
                expired_claims: 0,
                last_completion: Some(NOW - 300),
            },
            last_bank: Some(NOW - 300),
        }
    }

    /// Today's actual failure: samples arriving, counter frozen.
    pub fn stalled() -> View {
        let mut v = healthy();
        let last = *v.samples.last().unwrap();
        for i in 1..=60 {
            v.samples.push(Sample { ts: last.ts + i * 60, ..last });
        }
        v.now = last.ts + 3600;
        v.last_bank = Some(v.now - 300);
        v
    }

    /// The nastier half of the same failure: the run is active and the worker
    /// has not said anything at all.
    pub fn silent() -> View {
        View { run_active: true, ..View::empty(NOW) }
    }

    pub fn collapsed() -> View {
        let mut v = healthy();
        let last = *v.samples.last().unwrap();
        // an hour at 10/s, then ten minutes at 0.5/s
        for i in 1..=10 {
            v.samples.push(Sample {
                ts: last.ts + i * 60,
                evals: last.evals + (i as u64) * 30,
                best: Some(last.best.unwrap() + i as f64),
                ..last
            });
        }
        v.now = last.ts + 600;
        v.last_bank = Some(v.now - 60);
        v
    }

    /// Healthy throughput, objective pinned.
    pub fn no_progress_state() -> View {
        let mut samples = Vec::new();
        for i in 0..=600 {
            samples.push(Sample {
                ts: NOW - 36_000 + i * 60,
                evals: (i as u64) * 6_000, // 100/s
                best: Some(25.0),          // never moves
                disk_free_mb: Some(200_000),
                worker_alive: true,
            });
        }
        View { samples, last_bank: Some(NOW - 60), ..healthy() }
    }

    pub fn worker_dead() -> View {
        let mut v = healthy();
        if let Some(last) = v.samples.last_mut() {
            last.worker_alive = false;
        }
        v.now = v.samples.last().unwrap().ts;
        v
    }

    pub fn box_gone() -> View {
        let mut v = healthy();
        v.boxes = vec![("boxA".into(), BoxState { last_seen: v.now - 7200, active: true })];
        v
    }

    pub fn queue_stuck() -> View {
        let mut v = healthy();
        v.queue = QueueView { pending: 5, claimed: 0, expired_claims: 0, last_completion: Some(v.now - 30_000) };
        v
    }

    pub fn queue_claims_expired() -> View {
        let mut v = healthy();
        v.queue.expired_claims = 2;
        v
    }

    pub fn disk_cliff() -> View {
        let mut v = healthy();
        for s in v.samples.iter_mut() {
            s.disk_free_mb = Some(100);
        }
        v
    }

    pub fn disk_slope() -> View {
        let mut v = healthy();
        let n = v.samples.len();
        for (i, s) in v.samples.iter_mut().enumerate() {
            // 200 GB down to 20 GB over two hours: empty in well under six.
            s.disk_free_mb = Some(200_000 - (180_000 * i as i64 / n as i64));
        }
        v
    }

    pub fn never_banked() -> View {
        View { last_bank: None, ..healthy() }
    }

    pub fn bank_drifted() -> View {
        let mut v = healthy();
        v.last_bank = Some(v.now - 9 * 3600);
        v
    }

    /// Every alarm, paired with a state that must fire it. `tmhaul alarms
    /// selftest` walks this and refuses to pass unless each one does.
    pub fn firing_cases() -> Vec<(&'static str, &'static str, View)> {
        vec![
            ("zero_throughput", "counter frozen for an hour", stalled()),
            ("zero_throughput", "run active, nothing ever reported", silent()),
            ("throughput_collapse", "10/s baseline down to 0.5/s", collapsed()),
            ("no_progress", "100/s with the objective pinned", no_progress_state()),
            ("worker_died", "no worker process behind an active run", worker_dead()),
            ("box_vanished", "box silent for two hours", box_gone()),
            ("queue_stalled", "pending work, nothing completing", queue_stuck()),
            ("queue_stalled", "claims expired with their box gone", queue_claims_expired()),
            ("disk_filling", "below the free-space floor", disk_cliff()),
            ("disk_filling", "on trend to full within the horizon", disk_slope()),
            ("unbanked_drift", "nothing ever banked", never_banked()),
            ("unbanked_drift", "no bank for nine hours", bank_drifted()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    fn ids(v: &View) -> Vec<&'static str> {
        evaluate(v, &Config::default()).iter().map(|f| f.id).collect()
    }

    // ---- the control that makes every other test in this file mean something

    #[test]
    fn a_healthy_run_fires_nothing() {
        assert_eq!(ids(&healthy()), Vec::<&str>::new());
    }

    // ---- one firing test per alarm, from the shared fixtures

    #[test]
    fn every_alarm_has_a_state_that_fires_it() {
        for (id, why, v) in firing_cases() {
            let fired = ids(&v);
            assert!(fired.contains(&id), "{id} did not fire for: {why} (got {fired:?})");
        }
    }

    #[test]
    fn every_alarm_in_all_appears_in_the_firing_cases() {
        // Otherwise an alarm could be added with no proof it can ever fire —
        // which is the decoration this module exists to prevent.
        let covered: Vec<&str> = firing_cases().iter().map(|(id, _, _)| *id).collect();
        for (id, _) in ALL {
            assert!(covered.contains(id), "alarm {id} has no firing case");
        }
    }

    // ---- the specific bug of 2026-08-24, pinned

    #[test]
    fn zero_throughput_fires_where_the_old_no_progress_detector_could_not() {
        // The old detector watched "furthest station not improving across a
        // 2M-eval window". With no evals, that window never closes.
        let v = stalled();
        assert!(zero_throughput(&v, &Config::default()).is_some());
        assert!(
            no_progress(&v, &Config::default()).is_none(),
            "no_progress must NOT be the thing that catches a stall — that is the bug"
        );
    }

    #[test]
    fn an_empty_window_is_zero_throughput_not_silence() {
        let v = silent();
        assert!(
            zero_throughput(&v, &Config::default()).is_some(),
            "a run that has never reported anything must fire, not read as healthy"
        );
    }

    #[test]
    fn a_stall_raises_exactly_one_throughput_alarm() {
        let fired = ids(&stalled());
        assert!(fired.contains(&"zero_throughput"));
        assert!(
            !fired.contains(&"throughput_collapse"),
            "one event must not raise two alarms, or people learn to ignore both"
        );
    }

    // ---- controls: each alarm must be silent in the states it is not about

    #[test]
    fn an_idle_system_fires_no_run_alarms() {
        // Nothing is supposed to be running: silence is correct, not a stall.
        let v = View { run_active: false, ..silent() };
        let fired = ids(&v);
        assert!(!fired.contains(&"zero_throughput"));
        assert!(!fired.contains(&"worker_died"));
    }

    #[test]
    fn a_slow_but_steady_run_is_not_a_collapse() {
        let mut v = healthy();
        for s in v.samples.iter_mut() {
            s.evals /= 100; // 0.1 evals/s throughout — slow, but not collapsing
        }
        assert!(throughput_collapse(&v, &Config::default()).is_none());
    }

    #[test]
    fn a_climbing_objective_is_not_a_no_progress() {
        assert!(no_progress(&healthy(), &Config::default()).is_none());
    }

    #[test]
    fn a_drained_queue_is_not_a_stuck_queue() {
        let mut v = healthy();
        v.queue = QueueView { pending: 0, claimed: 0, expired_claims: 0, last_completion: Some(v.now - 99_999) };
        assert!(queue_stalled(&v, &Config::default()).is_none());
    }

    #[test]
    fn a_disk_that_is_merely_large_and_static_is_fine() {
        let mut v = healthy();
        for s in v.samples.iter_mut() {
            s.disk_free_mb = Some(50_000);
        }
        assert!(disk_filling(&v, &Config::default()).is_none());
    }

    #[test]
    fn recent_banking_is_not_drift() {
        assert!(unbanked_drift(&healthy(), &Config::default()).is_none());
    }
}
