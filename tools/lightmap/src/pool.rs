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
//! PER-THREAD BUSY TIME (`stats`, on under `--profile`): every task is timed on the participant that ran it;
//! `stats::stage` labels the runs, `stats::report` prints the utilisation table (perf engineer 6).

use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// The current job's closure, its lifetime erased: `run` does not return before every worker has left the
/// job, so the borrow it holds outlives every use. Published to the workers through atomics (a mutex here
/// had 128 workers queueing on one lock at the start of every call), and the workers woken through a TREE of
/// `Thread::unpark`s: a condvar's `notify_all` had the caller wake 128 parked threads one by one in the kernel
/// — 300–470 µs per call, ~20 calls per frame; now the caller unparks four workers, each unparks four more.
struct Shared {
    /// The job's closure (a fat pointer split in two words; `n` and both words are published before the
    /// generation bump that the workers wait for, and read after they see it).
    f_data: AtomicPtr<()>,
    f_vtable: AtomicPtr<()>,
    n: AtomicUsize,
    /// Bumped per job; the workers wait for a value they have not seen.
    generation: AtomicU64,
    /// Tasks handed out of the current job.
    next: AtomicUsize,
    /// Workers still inside the current job (a worker leaves only after a failed claim, so `active == 0`
    /// means every task was claimed and finished — no per-task completion counter).
    active: AtomicUsize,
    panicked: AtomicBool,
    /// The workers' handles, in worker order (set once after the spawns): worker i's children in the wake
    /// tree are 4i+1 ..= 4i+4.
    handles: OnceLock<Vec<std::thread::Thread>>,
    /// Per participant (worker k = slot k, the caller = the last slot): busy nanoseconds and tasks of the
    /// current run — written only while `stats::enabled()` (perf engineer 6).
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
const FANOUT: usize = 4;

/// One task, timed per participant when the stats are on (perf engineer 6); a panic is recorded, not propagated.
fn run_task(sh: &Shared, f: &(dyn Fn(usize) + Sync), i: usize, slot: usize) {
    let timed = stats::enabled();
    let t0 = if timed { Some(std::time::Instant::now()) } else { None };
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(i)));
    if r.is_err() {
        sh.panicked.store(true, Ordering::Relaxed);
    }
    if let Some(t0) = t0 {
        let ns = t0.elapsed().as_nanos() as u64;
        sh.slot_busy[slot].fetch_add(ns, Ordering::Relaxed);
        sh.slot_tasks[slot].fetch_add(1, Ordering::Relaxed);
        sh.task_max.fetch_max(ns, Ordering::Relaxed);
    }
}

impl Pool {
    pub fn new(threads: usize) -> Pool {
        let threads = threads.max(1);
        let shared = Arc::new(Shared {
            f_data: AtomicPtr::new(std::ptr::null_mut()),
            f_vtable: AtomicPtr::new(std::ptr::null_mut()),
            n: AtomicUsize::new(0),
            generation: AtomicU64::new(0),
            next: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            panicked: AtomicBool::new(false),
            handles: OnceLock::new(),
            slot_busy: (0..=threads).map(|_| AtomicU64::new(0)).collect(),
            slot_tasks: (0..=threads).map(|_| AtomicU64::new(0)).collect(),
            task_max: AtomicU64::new(0),
        });
        let mut handles = Vec::with_capacity(threads);
        for i in 0..threads {
            let sh = shared.clone();
            let h = std::thread::Builder::new()
                .name("lm-pool".into())
                .spawn(move || worker(sh, i))
                .expect("pool thread");
            handles.push(h.thread().clone());
        }
        shared.handles.set(handles).ok();
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
        let (data, vtable): (*mut (), *mut ()) = unsafe { std::mem::transmute(fptr) };
        // the previous job's workers must all have left before the job slot is reused
        while sh.active.load(Ordering::Acquire) != 0 {
            std::hint::spin_loop();
        }
        sh.next.store(0, Ordering::Relaxed);
        sh.panicked.store(false, Ordering::Relaxed);
        sh.active.store(self.threads, Ordering::Relaxed);
        if timed {
            for s in &sh.slot_busy { s.store(0, Ordering::Relaxed); }
            for s in &sh.slot_tasks { s.store(0, Ordering::Relaxed); }
            sh.task_max.store(0, Ordering::Relaxed);
        }
        sh.f_data.store(data, Ordering::Relaxed);
        sh.f_vtable.store(vtable, Ordering::Relaxed);
        sh.n.store(n, Ordering::Relaxed);
        // the release publishes the job; a worker's acquire load of the generation sees it whole
        sh.generation.fetch_add(1, Ordering::Release);
        // the wake tree's roots (a parked worker wakes; a spinning one finds a token it clears at its next park)
        if let Some(hs) = sh.handles.get() {
            for h in hs.iter().take(FANOUT) {
                h.unpark();
            }
        }
        // the caller helps
        let caller_slot = self.threads;
        loop {
            let i = sh.next.fetch_add(1, Ordering::Relaxed);
            if i >= n {
                break;
            }
            run_task(sh, fref, i, caller_slot);
        }
        // every worker out of the job (each leaves after its own failed claim, so every task is done)
        let mut spins = 0usize;
        while sh.active.load(Ordering::Acquire) != 0 {
            spins += 1;
            if spins < 20_000 { std::hint::spin_loop(); } else { std::thread::yield_now(); }
        }
        if timed {
            stats::record(sh, n, t_run.elapsed().as_nanos() as u64);
        }
        if sh.panicked.load(Ordering::Relaxed) {
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

fn worker(sh: Arc<Shared>, me: usize) {
    let mut seen = 0u64;
    loop {
        // a new generation: spin first, then park (an unpark token left by a wake we did not need is
        // consumed by the first park, which then returns at once and re-checks)
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
            std::thread::park();
            spins = SPIN / 2;
        }
        // pass the wake down the tree (cheap when the child is not parked: a token, no syscall)
        if let Some(hs) = sh.handles.get() {
            for c in me * FANOUT + 1..=me * FANOUT + FANOUT {
                if let Some(h) = hs.get(c) {
                    h.unpark();
                }
            }
        }
        // the job, published before the generation bump we just saw
        let (data, vtable, n) = (sh.f_data.load(Ordering::Relaxed), sh.f_vtable.load(Ordering::Relaxed), sh.n.load(Ordering::Relaxed));
        let fptr: *const (dyn Fn(usize) + Sync) = unsafe { std::mem::transmute((data, vtable)) };
        let f: &(dyn Fn(usize) + Sync) = unsafe { &*fptr };
        loop {
            let i = sh.next.fetch_add(1, Ordering::Relaxed);
            if i >= n {
                break;
            }
            run_task(&sh, f, i, me);
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
}

#[cfg(test)]
mod latency {
    /// `cargo test --release -p lightmap -- --ignored --nocapture pool_call_latency`: the cost of an empty pool
    /// call (the wake-up and the barrier) at the bake's thread count, back to back and after a pause.
    #[test]
    #[ignore]
    fn pool_call_latency() {
        let n = std::env::var("LMTOOL_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(128);
        let p = super::Pool::new(n);
        for (label, gap_us) in [("back to back", 0u64), ("after 100 µs", 100), ("after 2 ms", 2000)] {
            let mut total = std::time::Duration::ZERO;
            let rounds = 200;
            for _ in 0..rounds {
                if gap_us > 0 { let t = std::time::Instant::now(); while t.elapsed().as_micros() < gap_us as u128 { std::hint::spin_loop(); } }
                let t = std::time::Instant::now();
                p.run(n, |_| { std::hint::black_box(0); });
                total += t.elapsed();
            }
            eprintln!("{label}: {:.1} µs per empty pool call ({n} threads)", total.as_secs_f64() * 1e6 / rounds as f64);
        }
    }
}

#[cfg(test)]
mod latency2 {
    /// Where an empty call's time goes: all tasks claimed vs all workers left.
    #[test]
    #[ignore]
    fn pool_call_phases() {
        let n = 128usize;
        let p = super::Pool::new(n);
        let sh = &p.shared;
        for round in 0..6 {
            let t = std::time::Instant::now();
            // publish by hand (the same steps as `run`)
            let f = |_i: usize| { std::hint::black_box(0); };
            let fref: &(dyn Fn(usize) + Sync) = &f;
            let fptr: *const (dyn Fn(usize) + Sync) = unsafe { std::mem::transmute::<&(dyn Fn(usize) + Sync), &'static (dyn Fn(usize) + Sync)>(fref) };
            let (data, vtable): (*mut (), *mut ()) = unsafe { std::mem::transmute(fptr) };
            while sh.active.load(std::sync::atomic::Ordering::Acquire) != 0 { std::hint::spin_loop(); }
            let ta = t.elapsed();
            sh.next.store(0, std::sync::atomic::Ordering::Relaxed);
            sh.active.store(n, std::sync::atomic::Ordering::Relaxed);
            sh.f_data.store(data, std::sync::atomic::Ordering::Relaxed);
            sh.f_vtable.store(vtable, std::sync::atomic::Ordering::Relaxed);
            sh.n.store(n, std::sync::atomic::Ordering::Relaxed);
            sh.generation.fetch_add(1, std::sync::atomic::Ordering::Release);
            let tb = t.elapsed();
            for h in sh.handles.get().unwrap().iter().take(super::FANOUT) { h.unpark(); }
            let t1 = t.elapsed();
            while sh.next.load(std::sync::atomic::Ordering::Relaxed) < n { std::hint::spin_loop(); }
            let t2 = t.elapsed();
            while sh.active.load(std::sync::atomic::Ordering::Acquire) != 0 { std::hint::spin_loop(); }
            let t3 = t.elapsed();
            eprintln!("round {round}: prev left {:.1} µs, bumped {:.1} µs, publish {:.1} µs, all claimed {:.1} µs, all left {:.1} µs", ta.as_secs_f64() * 1e6, tb.as_secs_f64() * 1e6, t1.as_secs_f64() * 1e6, t2.as_secs_f64() * 1e6, t3.as_secs_f64() * 1e6);
        }
    }
}
