//! A persistent worker pool: `pool().run(n, |i| …)` executes the closure for i in 0..n on warm threads
//! and returns when every call has finished. Spawning a wave of 160 threads per stage costs tens of
//! milliseconds on a loaded host (the wave's start latency, not the work) — the bake runs ~30 such
//! waves per direction, so the threads stay alive and are woken through a condvar instead.
//!
//! The task claims and completions are lock-free atomics (a mutex around every claim with 160 workers
//! and 640 tasks put the kernel's futex lock at 7 % of a bake); an idle worker spins briefly on the
//! generation counter before parking, so the back-to-back waves within a direction never round-trip
//! through the futex.
//!
//! PER-THREAD BUSY TIME (`stats`, on under `--profile`): every task is timed on the participant that ran
//! it (the pool threads and the calling thread), so a run knows its wall, the busy time of every
//! participant, the slowest participant and the slowest task. The runs are summed per STAGE (the label
//! `stats::stage(..)` set by the bake around each stage: raster, clip, gather, …) and `stats::report`
//! prints the table: per stage the wall spent inside pool runs, the utilisation Σbusy / (wall × P), the
//! tail (slowest participant / mean participant, slowest task / mean task) and the task counts — the
//! measurement that separates "idle threads" from "latency-bound loops" (a 90 % utilised stage that is
//! slow is bound inside the visit bodies; a 40 % one is waiting for its slowest band).

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

struct Job {
    /// The task closure, its lifetime erased: `run` does not return before every task has completed,
    /// so the borrow it holds outlives every use.
    f: *const (dyn Fn(usize) + Sync),
    n: usize,
}
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

struct Shared {
    /// The current job (set by `run`, cleared after every worker has left it).
    job: Mutex<Option<Job>>,
    /// Bumped per job; the workers wait for a value they have not seen.
    generation: AtomicU64,
    /// Tasks handed out of the current job.
    next: AtomicUsize,
    /// Tasks finished in the current job.
    done: AtomicUsize,
    /// Workers still inside the current job.
    active: AtomicUsize,
    panicked: AtomicBool,
    /// The parking lot of idle workers (paired with `job`'s mutex).
    work: Condvar,
    /// Per participant (worker k = slot k, the caller = the last slot): busy nanoseconds and tasks of the
    /// current run — written only while `stats::enabled()`.
    slot_busy: Vec<AtomicU64>,
    slot_tasks: Vec<AtomicU64>,
    /// The slowest task of the current run (nanoseconds).
    task_max: AtomicU64,
}

pub struct Pool {
    shared: Arc<Shared>,
    pub threads: usize,
}

/// Idle spins on the generation counter before a worker parks (~10–50 µs).
const SPIN: usize = 4000;

/// Runs `f(i)` and books it on participant `slot` when the stats are on.
#[inline]
fn run_task(sh: &Shared, f: &(dyn Fn(usize) + Sync), i: usize, slot: usize) {
    let timed = stats::enabled();
    let t0 = if timed { Some(std::time::Instant::now()) } else { None };
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(i)));
    if r.is_err() {
        sh.panicked.store(true, Ordering::SeqCst);
    }
    if let Some(t0) = t0 {
        let ns = t0.elapsed().as_nanos() as u64;
        sh.slot_busy[slot].fetch_add(ns, Ordering::Relaxed);
        sh.slot_tasks[slot].fetch_add(1, Ordering::Relaxed);
        sh.task_max.fetch_max(ns, Ordering::Relaxed);
    }
    sh.done.fetch_add(1, Ordering::SeqCst);
}

impl Pool {
    pub fn new(threads: usize) -> Pool {
        let threads = threads.max(1);
        let shared = Arc::new(Shared {
            job: Mutex::new(None),
            generation: AtomicU64::new(0),
            next: AtomicUsize::new(0),
            done: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            panicked: AtomicBool::new(false),
            work: Condvar::new(),
            slot_busy: (0..=threads).map(|_| AtomicU64::new(0)).collect(),
            slot_tasks: (0..=threads).map(|_| AtomicU64::new(0)).collect(),
            task_max: AtomicU64::new(0),
        });
        for k in 0..threads {
            let sh = shared.clone();
            std::thread::Builder::new()
                .name("lm-pool".into())
                .spawn(move || worker(sh, k))
                .expect("pool thread");
        }
        Pool { shared, threads }
    }

    /// Run `f(i)` for every i in 0..n across the pool (the caller's thread joins in); returns when all
    /// have completed. A task panic is re-raised here after the others finished.
    pub fn run<F: Fn(usize) + Sync>(&self, n: usize, f: F) {
        if n == 0 {
            return;
        }
        let sh = &self.shared;
        let timed = stats::enabled();
        let t_run = std::time::Instant::now();
        let fref: &(dyn Fn(usize) + Sync) = &f;
        // SAFETY: the pointer is used only until every worker has left the job, below in this function
        let fptr: *const (dyn Fn(usize) + Sync) = unsafe { std::mem::transmute::<&(dyn Fn(usize) + Sync), &'static (dyn Fn(usize) + Sync)>(fref) };
        // the previous job's workers must all have left before the job slot is reused
        while sh.active.load(Ordering::Acquire) != 0 {
            std::hint::spin_loop();
        }
        {
            let mut job = sh.job.lock().unwrap();
            sh.next.store(0, Ordering::SeqCst);
            sh.done.store(0, Ordering::SeqCst);
            sh.panicked.store(false, Ordering::SeqCst);
            sh.active.store(self.threads, Ordering::SeqCst);
            if timed {
                for s in &sh.slot_busy { s.store(0, Ordering::Relaxed); }
                for s in &sh.slot_tasks { s.store(0, Ordering::Relaxed); }
                sh.task_max.store(0, Ordering::Relaxed);
            }
            *job = Some(Job { f: fptr, n });
            sh.generation.fetch_add(1, Ordering::SeqCst);
        }
        sh.work.notify_all();
        // the caller helps
        let caller_slot = self.threads;
        loop {
            let i = sh.next.fetch_add(1, Ordering::SeqCst);
            if i >= n {
                break;
            }
            run_task(sh, fref, i, caller_slot);
        }
        // every task done, then every worker out of the job (a worker may still be between its last
        // claim and its exit)
        let mut spins = 0usize;
        while sh.done.load(Ordering::Acquire) < n || sh.active.load(Ordering::Acquire) != 0 {
            spins += 1;
            if spins < 20_000 { std::hint::spin_loop(); } else { std::thread::yield_now(); }
        }
        *sh.job.lock().unwrap() = None;
        if timed {
            stats::record(sh, n, t_run.elapsed().as_nanos() as u64);
        }
        if sh.panicked.load(Ordering::SeqCst) {
            panic!("a pool task panicked");
        }
    }

    /// `run` collecting one value per task (in task order).
    pub fn map<T: Send, F: Fn(usize) -> T + Sync>(&self, n: usize, f: F) -> Vec<T> {
        let mut slots: Vec<Option<T>> = (0..n).map(|_| None).collect();
        let base = slots.as_mut_ptr() as usize;
        self.run(n, |i| {
            let v = f(i);
            // SAFETY: task i alone writes slot i; `run` returns after every task has finished
            unsafe { *(base as *mut Option<T>).add(i) = Some(v); }
        });
        slots.into_iter().map(|s| s.expect("pool task result")).collect()
    }
}

fn worker(sh: Arc<Shared>, slot: usize) {
    let mut seen = 0u64;
    loop {
        // a new generation: spin first, then park on the condvar
        let mut spins = 0usize;
        loop {
            let g = sh.generation.load(Ordering::Acquire);
            if g != seen {
                seen = g;
                break;
            }
            spins += 1;
            if spins < SPIN {
                std::hint::spin_loop();
                continue;
            }
            let job = sh.job.lock().unwrap();
            if sh.generation.load(Ordering::Acquire) == seen {
                let _g = sh.work.wait(job).unwrap();
            }
            spins = 0;
        }
        let (job_ptr, n) = {
            let job = sh.job.lock().unwrap();
            match job.as_ref() {
                Some(j) => (j.f, j.n),
                None => {
                    // the job was cleared before this worker saw it: it was never counted in
                    continue;
                }
            }
        };
        let f: &(dyn Fn(usize) + Sync) = unsafe { &*job_ptr };
        loop {
            let i = sh.next.fetch_add(1, Ordering::SeqCst);
            if i >= n {
                break;
            }
            run_task(&sh, f, i, slot);
        }
        sh.active.fetch_sub(1, Ordering::AcqRel);
    }
}

static POOL: OnceLock<Pool> = OnceLock::new();

/// The process-wide pool (LMTOOL_THREADS sets its size; default = the available parallelism, at most 160).
pub fn pool() -> &'static Pool {
    POOL.get_or_init(|| {
        let n = std::env::var("LMTOOL_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160));
        Pool::new(n)
    })
}

/// The per-stage utilisation statistics of the pool's runs (see the module doc).
pub mod stats {
    use super::*;

    static ENABLED: AtomicBool = AtomicBool::new(false);
    /// The label the next runs are booked under (set by the bake at every stage boundary).
    static STAGE: Mutex<&'static str> = Mutex::new("other");

    #[derive(Clone, Debug, Default)]
    pub struct StageAcc {
        pub label: &'static str,
        /// Pool runs booked under the label.
        pub runs: u64,
        /// Wall inside those runs (ns), summed.
        pub wall_ns: u64,
        /// Σ busy over every participant (ns) — the core-time actually spent in tasks.
        pub busy_ns: u64,
        /// Σ over the runs of the slowest participant's busy time (ns): the run can never be shorter.
        pub tail_ns: u64,
        /// Σ over the runs of the slowest task (ns).
        pub task_max_ns: u64,
        /// The slowest single task of any run (ns).
        pub task_peak_ns: u64,
        pub tasks: u64,
        /// Participants that ran at least one task, summed over the runs (the mean is Σ / runs).
        pub participants: u64,
        /// Runs whose task count was below the participant count (the pool could not be filled).
        pub starved_runs: u64,
    }

    static ACCS: Mutex<Vec<StageAcc>> = Mutex::new(Vec::new());

    #[inline]
    pub fn enabled() -> bool {
        ENABLED.load(Ordering::Relaxed)
    }

    /// Turns the per-task timing on (the bake does under `--profile`; `LMTOOL_POOL_STATS=1` as well).
    pub fn enable() {
        ENABLED.store(true, Ordering::Relaxed);
    }

    /// Books the runs that follow under `label`.
    pub fn stage(label: &'static str) {
        if enabled() {
            *STAGE.lock().unwrap() = label;
        }
    }

    pub(super) fn record(sh: &Shared, n: usize, wall_ns: u64) {
        let label = *STAGE.lock().unwrap();
        let mut busy = 0u64;
        let mut tail = 0u64;
        let mut participants = 0u64;
        for (b, t) in sh.slot_busy.iter().zip(&sh.slot_tasks) {
            let b = b.load(Ordering::Relaxed);
            if t.load(Ordering::Relaxed) > 0 {
                participants += 1;
            }
            busy += b;
            tail = tail.max(b);
        }
        let tmax = sh.task_max.load(Ordering::Relaxed);
        let mut accs = ACCS.lock().unwrap();
        let acc = match accs.iter_mut().find(|a| std::ptr::eq(a.label, label) || a.label == label) {
            Some(a) => a,
            None => {
                accs.push(StageAcc { label, ..Default::default() });
                accs.last_mut().unwrap()
            }
        };
        acc.runs += 1;
        acc.wall_ns += wall_ns;
        acc.busy_ns += busy;
        acc.tail_ns += tail;
        acc.task_max_ns += tmax;
        acc.task_peak_ns = acc.task_peak_ns.max(tmax);
        acc.tasks += n as u64;
        acc.participants += participants;
        if (n as u64) < sh.slot_busy.len() as u64 {
            acc.starved_runs += 1;
        }
    }

    /// A snapshot of the accumulators (sorted by wall, largest first).
    pub fn snapshot() -> Vec<StageAcc> {
        let mut v = ACCS.lock().unwrap().clone();
        v.sort_by(|a, b| b.wall_ns.cmp(&a.wall_ns));
        v
    }

    /// Clears the accumulators (after a report).
    pub fn reset() {
        ACCS.lock().unwrap().clear();
    }

    /// Prints the per-stage table and the whole-pool line against `outer_wall_s` (the wall the runs were
    /// part of — the sweep's directions total), then resets. P = the pool's participants (threads + the
    /// caller).
    pub fn report(label: &str, outer_wall_s: f64) {
        if !enabled() {
            return;
        }
        let accs = snapshot();
        if accs.is_empty() {
            return;
        }
        let p = super::pool().threads as f64 + 1.0;
        let s = |ns: u64| ns as f64 / 1e9;
        let (mut wall, mut busy) = (0u64, 0u64);
        for a in &accs {
            wall += a.wall_ns;
            busy += a.busy_ns;
        }
        eprintln!(
            "pool [{label}]: P = {p:.0} participants; {:.2}s inside {} pool runs of the {outer_wall_s:.2}s measured ({:.0} %; outside the pool = serial glue {:.2}s = {:.0} %); Σbusy {:.1} core-s = {:.0} % of the pool wall × P, {:.0} % of the measured wall × P",
            s(wall), accs.iter().map(|a| a.runs).sum::<u64>(), 100.0 * s(wall) / outer_wall_s.max(1e-9), outer_wall_s - s(wall), 100.0 * (outer_wall_s - s(wall)) / outer_wall_s.max(1e-9),
            s(busy), 100.0 * s(busy) / (s(wall) * p).max(1e-9), 100.0 * s(busy) / (outer_wall_s * p).max(1e-9)
        );
        eprintln!("pool [{label}]: {:<12} {:>7} {:>8} {:>6} {:>7} {:>8} {:>9} {:>9} {:>9} {:>7}", "stage", "runs", "wall s", "util%", "tail×", "task×", "mean ms", "max ms", "peak ms", "tasks");
        for a in &accs {
            let runs = a.runs.max(1) as f64;
            let mean_participant_busy = a.busy_ns as f64 / a.participants.max(1) as f64; // per run-participant
            let tail_x = (a.tail_ns as f64 / runs) / mean_participant_busy.max(1.0);
            let mean_task = a.busy_ns as f64 / a.tasks.max(1) as f64;
            let task_x = (a.task_max_ns as f64 / runs) / mean_task.max(1.0);
            eprintln!(
                "pool [{label}]: {:<12} {:>7} {:>8.3} {:>6.1} {:>7.2} {:>8.1} {:>9.3} {:>9.3} {:>9.3} {:>7}{}",
                a.label, a.runs, s(a.wall_ns), 100.0 * a.busy_ns as f64 / (a.wall_ns as f64 * p).max(1.0), tail_x, task_x,
                mean_task / 1e6, a.task_max_ns as f64 / runs / 1e6, a.task_peak_ns as f64 / 1e6, a.tasks,
                if a.starved_runs > 0 { format!("  ({} runs with fewer tasks than participants)", a.starved_runs) } else { String::new() }
            );
        }
        eprintln!("pool [{label}]: util% = Σbusy / (wall × P); tail× = slowest participant / mean participant per run; task× = slowest task / mean task per run; ms per task");
        reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_every_task_once() {
        let p = Pool::new(7);
        for round in 0..50 {
            let n = 1 + round * 13;
            let hits: Vec<AtomicUsize> = (0..n).map(|_| AtomicUsize::new(0)).collect();
            p.run(n, |i| { hits[i].fetch_add(1, Ordering::SeqCst); });
            assert!(hits.iter().all(|h| h.load(Ordering::SeqCst) == 1), "round {round}");
            let v = p.map(n, |i| i * 2);
            assert_eq!(v, (0..n).map(|i| i * 2).collect::<Vec<_>>());
        }
    }

    #[test]
    fn stats_book_every_task() {
        stats::enable();
        stats::reset();
        let p = Pool::new(3);
        stats::stage("test-stage");
        p.run(40, |i| { std::hint::black_box(i * i); });
        p.run(2, |_| {});
        let snap = stats::snapshot();
        let a = snap.iter().find(|a| a.label == "test-stage").expect("the stage is booked");
        assert_eq!(a.runs, 2);
        assert_eq!(a.tasks, 42);
        assert!(a.busy_ns <= a.wall_ns * 4 + 1_000_000, "busy {} ≤ wall {} × P", a.busy_ns, a.wall_ns);
        assert_eq!(a.starved_runs, 1);
        stats::reset();
    }
}
