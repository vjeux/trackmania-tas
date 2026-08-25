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
    /// Which box wrote this. Most alarms are about the RUN, which spans
    /// boxes; disk is about a MACHINE, and comparing free space across two of
    /// them measures the rotation rather than the disk.
    pub node: u64,
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
    /// When the current run started. A run that began inside the alarm window
    /// has not yet had a chance to say anything, and firing at it would make
    /// every restart look like a stall — which is how an alarm gets ignored.
    pub run_started: Option<i64>,
    pub samples: Vec<Sample>,
    pub boxes: Vec<(String, BoxState)>,
    pub queue: QueueView,
    pub last_bank: Option<i64>,
    /// Horizontal metres between where the run's car started and where the
    /// map says the start line is. `None` means no sample has carried one.
    pub start_dev_m: Option<f64>,
    /// Can this box reach GitHub through the bridge? `None` means the check
    /// was not run on this pass, which is different from "no".
    pub credential: Option<bool>,
    /// Is a supervisor process alive on THIS box? `None` means not checked —
    /// which is the honest answer on a box that is merely reading the repo
    /// and is not the one running the job.
    pub supervisor_here: Option<bool>,
    /// The node this view was assembled on, when the caller is speaking about
    /// a specific machine.
    pub this_node: Option<String>,
}

impl View {
    pub fn empty(now: i64) -> View {
        View {
            now,
            run_active: false,
            run_started: None,
            samples: Vec::new(),
            boxes: Vec::new(),
            queue: QueueView::default(),
            last_bank: None,
            start_dev_m: None,
            credential: None,
            supervisor_here: None,
            this_node: None,
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
    /// Shortest span of samples, from ONE box, that counts as a trend.
    pub disk_trend_min_window_s: i64,
    /// Local work not pushed anywhere durable for this long is at risk.
    pub bank_max_gap_s: i64,
    /// A run whose car starts further than this from the map's start line is
    /// not driving the map anybody asked about.
    pub start_dev_max_m: f64,
    /// Most boxes this project may hold at once. A bug in the rotation logic
    /// must not be able to provision without bound.
    pub max_boxes: usize,
    /// Does this job's worker DRIVE a car? A sweep over already-written tapes
    /// does not, and has no run-level start position to report.
    ///
    /// It defaults to `true`, so the start-position alarm fires at a silent
    /// worker unless the job SAYS IN THE CONFIG that the check does not
    /// apply. A declaration a human wrote and committed is a different thing
    /// from a worker quietly saying nothing, and only the first should be
    /// able to switch a check off.
    pub worker_drives: bool,
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
            disk_trend_min_window_s: 1_800,
            bank_max_gap_s: 3_600,
            start_dev_max_m: 32.0,
            max_boxes: 2,
            worker_drives: true,
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
    // A run that started inside the window has not had a full window to speak
    // in. Firing here would make every restart and every box rotation look
    // like a stall, and an alarm that cries at routine events is one nobody
    // reads. The grace period is the window itself, not a separate knob.
    if let Some(started) = v.run_started {
        if v.now - started < c.zero_window_s {
            return None;
        }
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
///
/// **The trend is computed per BOX, and this is not a detail.** Free space is
/// a property of a machine; the run spans machines. The first real rotation
/// had a box with 1.23 TB free replaced by one with 380 GB, and comparing
/// across them reported the disk "falling 7740 MB/min, empty in 49m" when
/// nothing was filling at all — it had measured the rotation. A false critical
/// on a routine event is how an alarm gets ignored.
pub fn disk_filling(v: &View, c: &Config) -> Option<Firing> {
    let last_any = v.samples.iter().rev().find(|s| s.disk_free_mb.is_some())?;
    let with_disk: Vec<&Sample> = v
        .samples
        .iter()
        .filter(|s| s.disk_free_mb.is_some() && s.node == last_any.node)
        .collect();
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
    // A trend needs a window worth extrapolating from. A box's first minutes
    // always show a steep fall — the oracle server is 385 MB and a release
    // build is more — and projecting six hours off two minutes of bootstrap
    // is arithmetic, not evidence.
    if dt < c.disk_trend_min_window_s as f64 {
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

// ---------------------------------------------------------------- A9

/// **The car is not starting at the start line.**
///
/// Added 2026-08-24, from a real finding: a run confirmed at `cps 3` had a
/// trajectory beginning at (1359.5, 1103) on a map whose start line is at
/// (1584, 784) — 390 m away, at checkpoint 3 — and spanning 217 m of a 1900 m
/// map. Whatever that run was, it was not the map from the start, and it took
/// four hours and two independent instruments to notice.
///
/// The check is **horizontal**. A spawn read from a block gives its cell, and
/// world y needs a per-map decoration offset that is fitted rather than read
/// (`mapgeom::place::Yoff`); x and z are exact. Saying so is better than a
/// three-dimensional number with one made-up axis.
///
/// The second branch is the important one and it is the same shape as
/// `zero_throughput`'s: **a run that never reports a start position fires
/// too.** An alarm that can only fire when the worker volunteers the evidence
/// is one the worker can switch off by saying nothing — which is precisely
/// the bug class this project keeps paying for.
pub fn start_position(v: &View, c: &Config) -> Option<Firing> {
    if !v.run_active || !c.worker_drives {
        return None;
    }
    if let Some(started) = v.run_started {
        if v.now - started < c.zero_window_s {
            return None; // too new to have said anything
        }
    }
    match v.start_dev_m {
        None => Some(Firing {
            id: "start_position",
            severity: Severity::Warn,
            detail: "the run has never reported where its car started, so nothing here can \
                     tell whether it is driving the map from the start line"
                .to_string(),
        }),
        Some(d) if d > c.start_dev_max_m => Some(Firing {
            id: "start_position",
            severity: Severity::Critical,
            detail: format!(
                "the car starts {d:.1} m from the map's start line (tolerance {:.0} m) — \
                 this run is not driving the map from the beginning",
                c.start_dev_max_m
            ),
        }),
        Some(_) => None,
    }
}

// ---------------------------------------------------------------- A12

/// **The supervisor itself died.**
///
/// Observed 2026-08-25: a supervisor vanished on a healthy box — no
/// `run_stop`, no line in its own log, no OOM, no reboot, the worker gone with
/// it. `tmhaul beat` said `NOT RUNNING` because it reads `/proc`; **the alarms
/// said nothing was firing**, and would have kept saying it for ten minutes
/// until `zero_throughput`'s window closed.
///
/// Ten minutes is not the problem. The problem is that the harness KNEW and
/// the alarm surface did not say so, which is the gap between "a check exists"
/// and "the check is wired to the thing people read".
///
/// Only fires on the box that owns the run. A box merely reading the repo —
/// a heartbeat on a fresh machine, say — reports `supervisor_here: None` and
/// gets silence, because "no supervisor on a box that was never running one"
/// is not a fault.
pub fn supervisor_died(v: &View, _c: &Config) -> Option<Firing> {
    if !v.run_active {
        return None;
    }
    if v.supervisor_here != Some(false) {
        return None;
    }
    // Is this box the one whose run is active? The newest sample names its
    // writer; if that is not us, somebody else owns this run and its absence
    // here is expected.
    let owner = v.samples.last().map(|s| s.node);
    let me = v.this_node.as_ref().map(|n| node_key(n));
    if owner.is_none() || me.is_none() || owner != me {
        return None;
    }
    Some(Firing {
        id: "supervisor_died",
        severity: Severity::Critical,
        detail: format!(
            "the run is active and this box wrote its last sample, but no supervisor process is \
             alive here. Nothing is banking or watching the worker: `tmhaul watch --detach \
             --lease-expires <expiry>`{}",
            v.samples
                .last()
                .map(|s| format!(" (last sample {})", crate::time::iso(s.ts)))
                .unwrap_or_default()
        ),
    })
}

/// Sample records carry a hashed node id; this is the same hash, so a caller
/// holding a node NAME can ask whether it wrote them.
pub fn node_key(node: &str) -> u64 {
    u64::from_str_radix(&crate::md5::md5_hex(node.as_bytes())[..8], 16).unwrap_or(0)
}

// ---------------------------------------------------------------- A11

/// **GitHub banking is degraded.**
///
/// The paste mirror needs only an x509 cert and keeps working; the push to
/// GitHub needs the bridge credential, which a fresh box does not have. So
/// this failure loses no WORK — but the repo is the state of record a human
/// reads, and it going stale while everything looks fine is exactly the
/// silence this project keeps paying for.
///
/// It is a warning rather than a critical because nothing is lost and the
/// credential server should heal it within its own cycle; `unbanked_drift`
/// stays armed underneath and escalates if banking stops entirely.
pub fn banking_degraded(v: &View, _c: &Config) -> Option<Firing> {
    match &v.credential {
        None => None, // not evaluated on this pass
        Some(true) => None,
        Some(false) => Some(Firing {
            id: "banking_degraded",
            severity: Severity::Warn,
            detail: "the bridge credential is missing or the bridge does not answer, so pushes \
                     to GitHub are failing. The paste mirror still works, so no work is at \
                     risk — but the repo a human reads is going stale. `tmhaul credential \
                     check`; the devserver's `tmhaul credential serve` should heal it"
                .to_string(),
        }),
    }
}

// ---------------------------------------------------------------- A10

/// **More boxes than the ceiling allows.**
///
/// The heartbeat provisions replacement boxes without a human, which is what
/// the brief asks for; a bug in that logic must not be able to provision
/// without bound. Leases cost money and the failure would be silent.
pub fn fleet_over_cap(v: &View, c: &Config) -> Option<Firing> {
    let live: Vec<&String> = v.boxes.iter().filter(|(_, b)| b.active).map(|(n, _)| n).collect();
    if live.len() <= c.max_boxes {
        return None;
    }
    Some(Firing {
        id: "fleet_over_cap",
        severity: Severity::Critical,
        detail: format!(
            "{} boxes are registered active and the ceiling is {}: {}. Retire the extras \
             (`tmhaul stop` on each) and release them",
            live.len(),
            c.max_boxes,
            live.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ),
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
    ("start_position", start_position),
    ("fleet_over_cap", fleet_over_cap),
    ("banking_degraded", banking_degraded),
    ("supervisor_died", supervisor_died),
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
                node: node_key("boxA"),
                evals: (i as u64) * 600,
                best: Some(10.0 + i as f64),
                disk_free_mb: Some(200_000),
                worker_alive: true,
            });
        }
        View {
            now: NOW,
            run_active: true,
            run_started: Some(NOW - 7200),
            samples,
            boxes: vec![("boxA".into(), BoxState { last_seen: NOW - 30, active: true })],
            queue: QueueView {
                pending: 3,
                claimed: 1,
                expired_claims: 0,
                last_completion: Some(NOW - 300),
            },
            last_bank: Some(NOW - 300),
            start_dev_m: Some(0.8),
            credential: Some(true),
            supervisor_here: Some(true),
            this_node: Some("boxA".to_string()),
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
        View { run_active: true, run_started: Some(NOW - 86_400), ..View::empty(NOW) }
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
                node: node_key("boxA"),
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

    /// A box replaced by one with less disk. Nothing is filling; the numbers
    /// are simply from two different machines.
    pub fn rotation_not_a_disk_fall() -> View {
        let mut v = healthy();
        for s in v.samples.iter_mut() {
            s.disk_free_mb = Some(1_232_500);
        }
        let last = *v.samples.last().unwrap();
        for i in 1..=2 {
            v.samples.push(Sample {
                ts: last.ts + i * 60,
                node: 2,
                disk_free_mb: Some(380_543),
                ..last
            });
        }
        v.now = last.ts + 120;
        v
    }

    /// A box whose first minutes are its own bootstrap: a 385 MB server
    /// download and a release build. Steep, brief, and not a trend.
    pub fn fresh_box_bootstrap() -> View {
        let mut v = healthy();
        v.samples.retain(|s| s.ts >= v.now - 120);
        for (i, s) in v.samples.iter_mut().enumerate() {
            s.node = 2;
            s.disk_free_mb = Some(400_000 - 9_000 * i as i64);
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

    /// A car that began its run 390 m from the start line, at checkpoint 3.
    /// These are the real numbers from *Summer 2026 - 01* on 2026-08-24.
    pub fn wrong_start() -> View {
        View { start_dev_m: Some(390.0), ..healthy() }
    }

    /// A run that has never said where its car started.
    pub fn start_unreported() -> View {
        View { start_dev_m: None, ..healthy() }
    }

    /// The run is active, this box wrote the last sample, and the supervisor
    /// that was doing it is gone.
    pub fn supervisor_gone() -> View {
        View { supervisor_here: Some(false), ..healthy() }
    }

    pub fn banking_degraded() -> View {
        View { credential: Some(false), ..healthy() }
    }

    pub fn too_many_boxes() -> View {
        let mut v = healthy();
        v.boxes = (0..4)
            .map(|i| (format!("box{i}"), BoxState { last_seen: v.now - 30, active: true }))
            .collect();
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
            ("start_position", "car starts 390 m away, at checkpoint 3", wrong_start()),
            ("start_position", "run never reported a start position", start_unreported()),
            ("fleet_over_cap", "four boxes against a ceiling of two", too_many_boxes()),
            ("banking_degraded", "no bridge credential on this box", banking_degraded()),
            ("supervisor_died", "run active, this box owns it, no supervisor process", supervisor_gone()),
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
    fn a_run_that_has_only_just_started_is_not_a_stall() {
        // Every restart and every box rotation begins with a run that has said
        // nothing yet. Firing at those would put a CRITICAL on the board
        // several times a day for a healthy system, and the alarm would be
        // ignored within a week.
        let v = View { run_started: Some(NOW - 30), ..silent() };
        assert!(zero_throughput(&v, &Config::default()).is_none());

        // The control, and it is the same fixture: once the window has passed
        // with nothing said, it must fire.
        let v = View { run_started: Some(NOW - 3600), ..silent() };
        assert!(zero_throughput(&v, &Config::default()).is_some());
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
    fn a_box_rotation_is_not_a_disk_filling_up() {
        // The real false positive, in its real numbers: 1.23 TB free on the
        // old box, 380 GB on its replacement, reported as "falling
        // 7740 MB/min — empty in 49m" while nothing was filling at all.
        // Free space is a property of a MACHINE; the run spans machines.
        assert!(
            disk_filling(&rotation_not_a_disk_fall(), &Config::default()).is_none(),
            "comparing free space across two boxes measures the rotation, not the disk"
        );
    }

    #[test]
    fn a_fresh_boxs_bootstrap_is_not_a_trend() {
        // Every box's first minutes fall steeply: a 385 MB server download and
        // a release build. Projecting six hours from two minutes of that is
        // arithmetic, not evidence.
        assert!(disk_filling(&fresh_box_bootstrap(), &Config::default()).is_none());
    }

    #[test]
    fn but_a_real_slope_on_one_box_still_fires() {
        // The control that keeps the two tests above honest: after suppressing
        // the rotation and the bootstrap, the alarm must still catch a disk
        // that is genuinely filling.
        assert!(disk_filling(&disk_slope(), &Config::default()).is_some());
        assert!(disk_filling(&disk_cliff(), &Config::default()).is_some());
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
    fn a_car_on_the_start_line_does_not_fire_the_start_alarm() {
        // The control for the start-position alarm, and the reason a tolerance
        // exists at all: a car sits a metre or so from the exact centre of the
        // start block, always.
        assert!(start_position(&healthy(), &Config::default()).is_none());
        let v = View { start_dev_m: Some(31.0), ..healthy() };
        assert!(start_position(&v, &Config::default()).is_none());
    }

    #[test]
    fn the_start_alarm_is_critical_for_a_wrong_start_and_a_warning_for_silence() {
        // Different facts, different severities: "it is driving from the wrong
        // place" is a result-invalidating certainty; "nobody said" is a gap.
        let f = start_position(&wrong_start(), &Config::default()).unwrap();
        assert_eq!(f.severity, Severity::Critical);
        let f = start_position(&start_unreported(), &Config::default()).unwrap();
        assert_eq!(f.severity, Severity::Warn);
    }

    #[test]
    fn the_fleet_cap_permits_exactly_its_ceiling() {
        let mut v = healthy();
        v.boxes = (0..2)
            .map(|i| (format!("box{i}"), BoxState { last_seen: v.now - 30, active: true }))
            .collect();
        assert!(fleet_over_cap(&v, &Config::default()).is_none(), "two boxes is the ceiling");
        v.boxes.push(("box2".into(), BoxState { last_seen: v.now - 30, active: true }));
        assert!(fleet_over_cap(&v, &Config::default()).is_some());
    }

    #[test]
    fn retired_boxes_do_not_count_against_the_fleet_cap() {
        // Over a month the registry accumulates dozens of retired boxes. If
        // those counted, the alarm would fire permanently and be ignored.
        let mut v = healthy();
        v.boxes = (0..40)
            .map(|i| (format!("old{i}"), BoxState { last_seen: v.now - 99_999, active: false }))
            .collect();
        v.boxes.push(("current".into(), BoxState { last_seen: v.now - 10, active: true }));
        assert!(fleet_over_cap(&v, &Config::default()).is_none());
    }

    #[test]
    fn recent_banking_is_not_drift() {
        assert!(unbanked_drift(&healthy(), &Config::default()).is_none());
    }
}

#[cfg(test)]
mod drives_tests {
    use super::fixtures::*;
    use super::*;

    #[test]
    fn a_worker_that_does_not_drive_is_not_asked_where_its_car_started() {
        let c = Config { worker_drives: false, ..Config::default() };
        assert!(start_position(&start_unreported(), &c).is_none());
    }

    #[test]
    fn but_only_the_config_can_say_so_and_it_defaults_to_asking() {
        // The distinction that matters: a job DECLARING the check does not
        // apply is a line a human wrote and committed. A worker saying nothing
        // is not, and must still fire — otherwise any worker could switch the
        // check off by omission, which is the whole bug class.
        assert!(start_position(&start_unreported(), &Config::default()).is_some());
        assert!(Config::default().worker_drives);
    }

    #[test]
    fn a_non_driving_job_still_gets_every_other_alarm() {
        // Switching off one check must not switch off the rest.
        let c = Config { worker_drives: false, ..Config::default() };
        assert!(zero_throughput(&stalled(), &c).is_some());
        assert!(worker_died(&worker_dead(), &c).is_some());
    }
}

#[cfg(test)]
mod supervisor_tests {
    use super::fixtures::*;
    use super::*;

    #[test]
    fn a_dead_supervisor_on_the_box_that_owns_the_run_is_critical() {
        let f = supervisor_died(&supervisor_gone(), &Config::default()).unwrap();
        assert_eq!(f.severity, Severity::Critical);
        assert!(f.detail.contains("watch --detach"), "{}", f.detail);
    }

    #[test]
    fn a_live_supervisor_is_silent() {
        assert!(supervisor_died(&healthy(), &Config::default()).is_none());
    }

    #[test]
    fn a_box_that_does_not_own_the_run_says_nothing_about_it() {
        // The control that keeps this alarm from screaming on every machine
        // that merely reads the repo: a heartbeat on a fresh box, or the
        // devserver running `credential serve`, has no supervisor and should
        // not — the run belongs to somebody else.
        let v = View {
            supervisor_here: Some(false),
            this_node: Some("some-other-box".into()),
            ..healthy()
        };
        assert!(supervisor_died(&v, &Config::default()).is_none());
    }

    #[test]
    fn an_unchecked_box_is_not_a_dead_one() {
        // `None` means nobody looked. It must not read as "no supervisor".
        let v = View { supervisor_here: None, ..healthy() };
        assert!(supervisor_died(&v, &Config::default()).is_none());
    }

    #[test]
    fn an_idle_project_does_not_want_a_supervisor() {
        let v = View { run_active: false, supervisor_here: Some(false), ..healthy() };
        assert!(supervisor_died(&v, &Config::default()).is_none());
    }
}
