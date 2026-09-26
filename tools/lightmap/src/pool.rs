//! A persistent worker pool: `pool().run(n, |i| …)` executes the closure for i in 0..n on warm threads
//! and returns when every call has finished. Spawning a wave of 160 threads per stage costs tens of
//! milliseconds on a loaded host (the wave's start latency, not the work) — the bake runs ~30 such
//! waves per direction, so the threads stay alive and are woken through a condvar instead.
//!
//! The task claims and completions are lock-free atomics (a mutex around every claim with 160 workers
//! and 640 tasks put the kernel's futex lock at 7 % of a bake); an idle worker spins briefly on the
//! job word before parking, so the back-to-back waves within a direction never round-trip
//! through the futex.
//!
//! THE JOB WORD (`generation`) is the run's number and its participant count in ONE atomic, stored last: a worker's
//! admission — "a run I have not seen, and I am in its set" — is a single load, never two words that can be read torn
//! (the hang of 2026-09-26; see `Shared::generation` and `tests::staged_job_admits_nobody`).
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
    /// job word that the workers wait for, and read after they see it).
    f_data: AtomicPtr<()>,
    f_vtable: AtomicPtr<()>,
    n: AtomicUsize,
    /// THE JOB WORD: the run's number in the high 48 bits, its participant count (worker k joins iff k < participants)
    /// in the low 16 — ONE atomic, stored last with Release (`commit`), so a worker's Acquire load of a value it has
    /// not seen is the whole admission in one snapshot: the run it answers for and whether it is in the set, with
    /// every field staged before it visible. A run's members are exactly the workers below its participant count,
    /// `active` counts them, and the caller returns only when each has left, so no field changes under a member.
    /// TWO WORDS HERE WERE THE HANG OF 2026-09-26 (`participants` and `job_gen` stored one after the other beside a
    /// bare counter): a worker that had missed a small run (parked or descheduled — nobody wakes a worker outside the
    /// set) read the NEXT run's participants with the finished run's number, joined the finished run, and decremented
    /// `active` once too often — every later run returned with a worker still inside (a `map` slot never written:
    /// "pool task result"), until a stray decrement met `active == 0` between runs and the next `run`'s wait for the
    /// previous job spun forever with every worker parked (stpad Day q3, direction 96 of sweep 1: 7–11 back-to-back
    /// probe maps of a few slices each, 128 workers outside their sets).
    generation: AtomicU64,
    /// Tasks handed out of the current job.
    next: AtomicUsize,
    /// Workers still inside the current job (a worker leaves only after a failed claim, so `active == 0`
    /// means every task was claimed and finished — no per-task completion counter).
    active: AtomicUsize,
    panicked: AtomicBool,
    /// The first task panic of the run (task index, message): `run` re-raises it with both.
    panic_msg: Mutex<Option<(usize, String)>>,
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
    /// One caller at a time: two threads calling `run` together both passed the `active == 0` wait and the second
    /// overwrote the first's job — its tasks never ran (`map` panicked on an empty slot; the probepass / probebake
    /// unit tests, run in parallel by the harness, hit it on the base). The bake calls from one thread, so the
    /// lock is uncontended (a task must never call `run` itself: it would wait on its own run).
    caller: Mutex<()>,
}

/// Idle spins on the job word before a worker parks (LMTOOL_POOL_SPIN, default 4000 iterations ≈ 100 µs).
/// Measured (perf 6, giant 4-dir directions total, 128 threads, 2 runs each): 500 / 1000 / 4000 / 16000 iterations =
/// 1.34 / 1.32 / 1.38 / 1.30 s — within the run-to-run noise (±0.04), so the default stays; a knob for other boxes
/// (a spinning worker sits on a working thread's SMT sibling: on the previous pool a 5 ms spin cost 5 %).
fn spin_limit() -> usize {
    static V: OnceLock<usize> = OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_POOL_SPIN").ok().and_then(|v| v.parse().ok()).unwrap_or(4000))
}
const FANOUT: usize = 4;

/// The job word's split: the participant count in the low bits, the run's number above.
const PARTICIPANT_BITS: u32 = 16;
/// The most workers a pool can have (the participant field's range).
pub const MAX_THREADS: usize = (1 << PARTICIPANT_BITS) - 1;
#[inline]
fn job_word(number: u64, participants: usize) -> u64 {
    debug_assert!(participants <= MAX_THREADS);
    (number << PARTICIPANT_BITS) | participants as u64
}
#[inline]
fn job_number(word: u64) -> u64 {
    word >> PARTICIPANT_BITS
}
#[inline]
fn job_participants(word: u64) -> usize {
    (word & MAX_THREADS as u64) as usize
}

/// One task, timed per participant when the stats are on (perf engineer 6); a panic is recorded (its message and the
/// task's index, the first of the run) and re-raised by `run` after every task finished.
thread_local! {
    /// Set while this thread runs a pool task: a `run` from inside a task would wait on the lock its own caller
    /// holds (and the caller on this worker's exit) — a silent hang; the assertion in `run` names it instead
    /// (engineer 4's audit probe of 6.11; in release too since the pool hang of 2026-09-26 — one thread-local read
    /// per `run`, against a deadlock that would otherwise look exactly like that hang).
    static IN_TASK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn run_task(sh: &Shared, f: &(dyn Fn(usize) + Sync), i: usize, slot: usize) {
    let timed = stats::enabled();
    let t0 = if timed { Some(std::time::Instant::now()) } else { None };
    IN_TASK.with(|c| c.set(true));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(i)));
    IN_TASK.with(|c| c.set(false));
    if let Err(payload) = r {
        let msg = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(a non-string panic payload)".to_string());
        let mut first = sh.panic_msg.lock().unwrap_or_else(|e| e.into_inner());
        if first.is_none() {
            *first = Some((i, msg));
        }
        drop(first);
        sh.panicked.store(true, Ordering::Relaxed);
    }
    if let Some(t0) = t0 {
        let ns = t0.elapsed().as_nanos() as u64;
        sh.slot_busy[slot].fetch_add(ns, Ordering::Relaxed);
        sh.slot_tasks[slot].fetch_add(1, Ordering::Relaxed);
        sh.task_max.fetch_max(ns, Ordering::Relaxed);
    }
}

impl Shared {
    /// STAGES a job: waits for the previous job's workers to leave, then writes every field of the job — the task
    /// counter, the member count, the closure, `n`. Nothing here admits a worker: a worker acts on the job word alone,
    /// and `commit` stores it after everything else (the tests wake stale workers between the two and check).
    fn stage(&self, data: *mut (), vtable: *mut (), n: usize, participants: usize, timed: bool) {
        // the previous job's workers must all have left before the job slot is reused
        while self.active.load(Ordering::Acquire) != 0 {
            std::hint::spin_loop();
        }
        self.next.store(0, Ordering::Relaxed);
        self.panicked.store(false, Ordering::Relaxed);
        self.active.store(participants, Ordering::Relaxed);
        if timed {
            for s in &self.slot_busy { s.store(0, Ordering::Relaxed); }
            for s in &self.slot_tasks { s.store(0, Ordering::Relaxed); }
            self.task_max.store(0, Ordering::Relaxed);
        }
        self.f_data.store(data, Ordering::Relaxed);
        self.f_vtable.store(vtable, Ordering::Relaxed);
        self.n.store(n, Ordering::Relaxed);
    }

    /// COMMITS the staged job: the job word (the next number, the participants) — the release publishes the job whole; a
    /// worker's acquire load of the word sees every staged field — then the wake tree's roots (a parked worker wakes; a
    /// spinning one finds a token it clears at its next park).
    fn commit(&self, participants: usize) {
        let word = job_word(job_number(self.generation.load(Ordering::Relaxed)) + 1, participants);
        self.generation.store(word, Ordering::Release);
        if let Some(hs) = self.handles.get() {
            for h in hs.iter().take(FANOUT.min(participants)) {
                h.unpark();
            }
        }
    }
}

impl Pool {
    pub fn new(threads: usize) -> Pool {
        let threads = threads.clamp(1, MAX_THREADS);
        let shared = Arc::new(Shared {
            f_data: AtomicPtr::new(std::ptr::null_mut()),
            f_vtable: AtomicPtr::new(std::ptr::null_mut()),
            n: AtomicUsize::new(0),
            generation: AtomicU64::new(0),
            next: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            panicked: AtomicBool::new(false),
            panic_msg: Mutex::new(None),
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
        Pool { shared, threads, caller: Mutex::new(()) }
    }

    /// Run `f(i)` for every i in 0..n across the pool (the caller's thread joins in); returns when all
    /// have completed. A task panic is re-raised here after the others finished.
    pub fn run<F: Fn(usize) + Sync>(&self, n: usize, f: F) {
        if n == 0 {
            return;
        }
        assert!(!IN_TASK.with(|c| c.get()), "pool::run called from inside a pool task: it would wait on its own caller (deadlock)");
        let _one_caller = self.caller.lock().unwrap_or_else(|e| e.into_inner());
        // LMTOOL_POOL_STATS=1: every task timed — the regions' busy / capacity table in the profile report
        let stats = *POOL_STATS;
        let busy = AtomicU64::new(0);
        let maxt = AtomicU64::new(0);
        let f = |i: usize| {
            if stats {
                let t = std::time::Instant::now();
                f(i);
                let ns = t.elapsed().as_nanos() as u64;
                busy.fetch_add(ns, Ordering::Relaxed);
                maxt.fetch_max(ns, Ordering::Relaxed);
            } else {
                f(i);
            }
        };
        let _rec = if stats { Some(RunRecord { busy: &busy, maxt: &maxt, n, threads: self.threads, t0: std::time::Instant::now() }) } else { None };
        let sh = &self.shared;
        let timed = stats::enabled();
        let t_run = std::time::Instant::now();
        let fref: &(dyn Fn(usize) + Sync) = &f;
        // SAFETY: the pointer is used only until every worker has left the job, below in this function
        let fptr: *const (dyn Fn(usize) + Sync) = unsafe { std::mem::transmute::<&(dyn Fn(usize) + Sync), &'static (dyn Fn(usize) + Sync)>(fref) };
        let (data, vtable): (*mut (), *mut ()) = unsafe { std::mem::transmute(fptr) };
        // A RUN TAKES NO MORE WORKERS THAN TASKS (perf 6 / engineer 8's 8.9): worker k joins iff k < participants =
        // min(threads, n) — a run of 40 tasks does not wake, nor wait for, 128 workers; the wake tree stops at the
        // set's edge (children carry greater indices, so every participant is still reached). The count rides in the
        // job word with the run's number, so a worker left out of a run never joins the next one twice.
        let participants = self.threads.min(n);
        sh.stage(data, vtable, n, participants, timed);
        sh.commit(participants);
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
            stats::record(sh, n, participants + 1, t_run.elapsed().as_nanos() as u64);
        }
        if sh.panicked.load(Ordering::Relaxed) {
            let first = sh.panic_msg.lock().unwrap_or_else(|e| e.into_inner()).take();
            match first {
                Some((i, msg)) => panic!("pool task {i} of {n} panicked: {msg}"),
                None => panic!("a pool task panicked"),
            }
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
        slots
            .into_iter()
            .enumerate()
            .map(|(i, s)| match s {
                Some(v) => v,
                // `run` came back with a task never run: a worker was admitted to a run it was not counted in (the
                // pool's completion count is off) — name the task and the counters, not "pool task result"
                None => panic!(
                    "pool: task {i} of {n} never ran although the run completed (active = {}, job word = {:#x}: the completion count is off by a worker admitted to a run it was not counted in)",
                    self.shared.active.load(Ordering::Relaxed),
                    self.shared.generation.load(Ordering::Relaxed)
                ),
            })
            .collect()
    }
}

/// LMTOOL_PIN=1 pins worker k to logical CPU k (mod the CPU count). MEASURED NEGATIVE on a Genoa guest whose vCPUs
/// float over the host's cores (128 pinned: directions 2.16 → 2.29 s; 159 pinned: any other process on the box
/// stalls a band, 2.15 → 5.13 s median). Off by default; the knob is for a quiet bare-metal box with fixed SMT pairs.
#[cfg(target_os = "linux")]
fn pin_worker(k: usize) {
    static PIN: OnceLock<bool> = OnceLock::new();
    if !*PIN.get_or_init(|| std::env::var_os("LMTOOL_PIN").is_some()) {
        return;
    }
    extern "C" {
        fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const u64) -> i32;
    }
    let ncpu = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(1);
    let cpu = k % ncpu;
    let mut mask = [0u64; 16];
    if cpu / 64 < mask.len() {
        mask[cpu / 64] |= 1u64 << (cpu % 64);
        // SAFETY: a plain syscall wrapper over a mask on this stack
        unsafe { sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()); }
    }
}

#[cfg(not(target_os = "linux"))]
fn pin_worker(_k: usize) {}

fn worker(sh: Arc<Shared>, me: usize) {
    pin_worker(me);
    let mut seen = 0u64;
    loop {
        // a new job word: spin first, then park (an unpark token left by a wake we did not need is
        // consumed by the first park, which then returns at once and re-checks)
        let mut spins = 0usize;
        loop {
            let g = sh.generation.load(Ordering::Acquire);
            if g != seen {
                seen = g;
                break;
            }
            spins += 1;
            if spins < spin_limit() {
                std::hint::spin_loop();
                continue;
            }
            std::thread::park();
            spins = spin_limit() / 2;
        }
        // the word we just saw is the whole admission: the run's members are the workers below its participant count
        // (a worker left out of a run may see the next job's word instead — it decides on that one, and only once, since
        // the number in the word never repeats); the job's fields were staged before the word, so a member reads its own
        // job's — and the caller cannot restage them before this member has left
        let participants = job_participants(seen);
        if me >= participants {
            continue;
        }
        // pass the wake down the tree, inside the set (cheap when the child is not parked: a token, no syscall)
        if let Some(hs) = sh.handles.get() {
            for c in me * FANOUT + 1..=me * FANOUT + FANOUT {
                if c < participants {
                    if let Some(h) = hs.get(c) {
                        h.unpark();
                    }
                }
            }
        }
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

/// The process-wide pool. LMTOOL_THREADS sets its size; the default is the available parallelism less the calling
/// thread (it takes tasks too), AT MOST 128: the Genoa boxes are 80–88 physical cores behind 160–176 logical CPUs,
/// and past the physical cores a thread only shares a core. Measured (perf 6, directions total, 2 runs each) — a
/// 166-cpu guest with the SMT pairs exposed: giant 4 dirs 83 / 128 / 165 threads = 1.43 / 1.30 / 1.33 s, tiny 16 sweep 0
/// × 64 dirs 5.53 / 5.18 / 5.53; a 160-cpu guest with the pairs hidden: giant 88 / 128 / 159 = 2.23 / 2.14 / 2.12,
/// tiny 7.3 / 7.1 / 7.2; engineer 2 on the 176-thread bare-metal box: 128 vs 176 = 0 %. 128 is best or equal
/// everywhere; every logical CPU costs the tiny maps 3–7 %.
pub fn pool() -> &'static Pool {
    POOL.get_or_init(|| {
        let n = std::env::var("LMTOOL_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| std::thread::available_parallelism().map(|x| x.get().saturating_sub(1).max(1)).unwrap_or(8).min(128));
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
        /// Runs whose task count was below the pool's participant count (the run took fewer workers).
        pub starved_runs: u64,
        /// Serial time (ns) between the previous pool run's end (or the direction's start, `epoch`) and this
        /// stage's runs' starts — the glue the calling thread spends outside the pool while the workers idle.
        pub gap_ns: u64,
        /// Σ over the runs of wall × the run's participants (ns): the capacity util% is measured against.
        pub cap_ns: u64,
    }

    /// When the last pool run ended (the serial gaps are measured from it).
    static LAST_END: Mutex<Option<std::time::Instant>> = Mutex::new(None);

    static ACCS: Mutex<Vec<StageAcc>> = Mutex::new(Vec::new());

    #[inline]
    pub fn enabled() -> bool {
        ENABLED.load(Ordering::Relaxed)
    }

    /// Turns the per-task timing on (the bake does under `--profile`; `LMTOOL_POOL_STATS=1` as well).
    pub fn enable() {
        ENABLED.store(true, Ordering::Relaxed);
    }

    /// Marks the start of a direction: the serial gap before the direction's first pool run is measured from
    /// here (not from the previous direction's last run, which would count the setup between them).
    pub fn epoch() {
        if enabled() {
            *LAST_END.lock().unwrap() = Some(std::time::Instant::now());
        }
    }

    /// A serial CHECKPOINT: the time since the last pool run ended (or the last checkpoint) is booked as the gap
    /// of a pseudo-stage `label` (no runs), so a long serial stretch can be attributed piecewise.
    pub fn checkpoint(label: &'static str) {
        if !enabled() {
            return;
        }
        let now = std::time::Instant::now();
        let gap = { let mut le = LAST_END.lock().unwrap(); let g = le.map(|t| now.duration_since(t).as_nanos() as u64).unwrap_or(0); *le = Some(now); g };
        let mut accs = ACCS.lock().unwrap();
        match accs.iter_mut().find(|a| a.label == label) {
            Some(a) => a.gap_ns += gap,
            None => accs.push(StageAcc { label, gap_ns: gap, ..Default::default() }),
        }
    }

    /// Books the runs that follow under `label`.
    pub fn stage(label: &'static str) {
        if enabled() {
            *STAGE.lock().unwrap() = label;
        }
    }

    pub(super) fn record(sh: &Shared, n: usize, run_participants: usize, wall_ns: u64) {
        let label = *STAGE.lock().unwrap();
        let now = std::time::Instant::now();
        let gap = { let mut le = LAST_END.lock().unwrap(); let g = le.map(|t| now.duration_since(t).as_nanos() as u64).unwrap_or(0).saturating_sub(wall_ns); *le = Some(now); g };
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
        acc.gap_ns += gap;
        acc.cap_ns += wall_ns * run_participants as u64;
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
        *LAST_END.lock().unwrap() = None;
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
        eprintln!("pool [{label}]: {:<18} {:>7} {:>8} {:>7} {:>6} {:>6} {:>7} {:>8} {:>9} {:>9} {:>9} {:>7}", "stage", "runs", "wall s", "gap s", "util%", "busy P", "tail×", "task×", "mean ms", "max ms", "peak ms", "tasks");
        for a in &accs {
            let runs = a.runs.max(1) as f64;
            let mean_participant_busy = a.busy_ns as f64 / a.participants.max(1) as f64; // per run-participant
            let tail_x = (a.tail_ns as f64 / runs) / mean_participant_busy.max(1.0);
            let mean_task = a.busy_ns as f64 / a.tasks.max(1) as f64;
            let task_x = (a.task_max_ns as f64 / runs) / mean_task.max(1.0);
            eprintln!(
                "pool [{label}]: {:<18} {:>7} {:>8.3} {:>7.3} {:>6.1} {:>6.1} {:>7.2} {:>8.1} {:>9.3} {:>9.3} {:>9.3} {:>7}{}",
                a.label, a.runs, s(a.wall_ns), s(a.gap_ns), 100.0 * a.busy_ns as f64 / a.cap_ns.max(1) as f64, a.busy_ns as f64 / a.wall_ns.max(1) as f64, tail_x, task_x,
                mean_task / 1e6, a.task_max_ns as f64 / runs / 1e6, a.task_peak_ns as f64 / 1e6, a.tasks,
                if a.starved_runs > 0 { format!("  ({} runs with fewer tasks than the pool: fewer workers taken)", a.starved_runs) } else { String::new() }
            );
        }
        eprintln!("pool [{label}]: gap s = serial time before the stage's runs (the caller working alone; a (name) row is a checkpoint); util% = Σbusy / Σ(wall × the run's participants); busy P = Σbusy / wall = participants busy on average; tail× = slowest participant / mean participant per run; task× = slowest task / mean task per run; ms per task");
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

    /// A closure as the pool publishes it (the same transmute as `run`); the closure must outlive every use.
    fn erase<'a>(f: &'a (dyn Fn(usize) + Sync)) -> (*mut (), *mut ()) {
        let fptr: *const (dyn Fn(usize) + Sync) = unsafe { std::mem::transmute::<&(dyn Fn(usize) + Sync), &'static (dyn Fn(usize) + Sync)>(f) };
        unsafe { std::mem::transmute(fptr) }
    }

    /// Long enough for every idle worker to spin out (spin_limit iterations, ~0.1 ms) and park.
    fn let_workers_park() {
        std::thread::sleep(std::time::Duration::from_millis(30));
    }

    /// THE HANG OF 2026-09-26, hand-driven. A worker that missed a small run (parked, outside its set — nobody wakes a
    /// worker outside the set) is woken while the caller of the NEXT run has STAGED that run (every field written: the
    /// task counter, `active`, the closure, `n`) but not yet committed its job word. The old pool published the
    /// admission as two words, `participants` then `job_gen`; a worker woken between the two stores read the new
    /// participants with the finished run's number and JOINED THE FINISHED RUN — one `active` decrement too many, and
    /// from there every run returned with a worker still inside ("pool task result": a `map` slot never written) until
    /// a stray decrement met `active == 0` between runs and the next `run` spun forever with every worker parked. On the
    /// old pool this test's staged step leaves `active` at 18446744073709551615 (−1). Here: nothing but the committed
    /// job word admits a worker — the staged run's fields must be untouched by a wake at any point before the commit,
    /// and the commit must then run the job exactly once.
    #[test]
    fn staged_job_admits_nobody() {
        let p = Pool::new(8);
        let sh = &p.shared;
        // run 1: every worker sees the word and parks
        p.run(8, |_| {});
        let_workers_park();
        // run 2 by hand, worker 0 only (n = 1): workers 1..8 stay parked with run 1's word as their last seen
        let calls2 = AtomicUsize::new(0);
        let f2 = |_i: usize| { calls2.fetch_add(1, Ordering::SeqCst); };
        let (d2, v2) = erase(&f2);
        sh.stage(d2, v2, 1, 1, false);
        sh.commit(1);
        loop { let i = sh.next.fetch_add(1, Ordering::Relaxed); if i >= 1 { break; } f2(i); }
        while sh.active.load(Ordering::Acquire) != 0 { std::hint::spin_loop(); }
        assert_eq!(calls2.load(Ordering::SeqCst), 1);
        let_workers_park();
        // run 3 staged for all 8 (n = 16) but NOT committed; every worker woken as if by a stale token, a spurious
        // futex return, or a timer — a stale worker (1..8) wakes and reads run 2's word: not its run
        let calls3: Vec<AtomicUsize> = (0..16).map(|_| AtomicUsize::new(0)).collect();
        let f3 = |i: usize| { calls3[i].fetch_add(1, Ordering::SeqCst); };
        let (d3, v3) = erase(&f3);
        sh.stage(d3, v3, 16, 8, false);
        for round in 0..3 {
            for h in sh.handles.get().unwrap() { h.unpark(); }
            let_workers_park();
            let active = sh.active.load(Ordering::Acquire);
            assert_eq!(active, 8, "round {round}: a worker joined a run that was only staged (active = {active} = {} as i64)", active as i64);
            assert_eq!(sh.next.load(Ordering::Relaxed), 0, "round {round}: a task of the staged run was claimed");
            assert!(calls3.iter().all(|c| c.load(Ordering::SeqCst) == 0), "round {round}: a task of the staged run ran");
        }
        // the commit admits the 8 members; the caller takes its share; every task exactly once
        sh.commit(8);
        loop { let i = sh.next.fetch_add(1, Ordering::Relaxed); if i >= 16 { break; } f3(i); }
        while sh.active.load(Ordering::Acquire) != 0 { std::hint::spin_loop(); }
        assert!(calls3.iter().all(|c| c.load(Ordering::SeqCst) == 1), "{:?}", calls3.iter().map(|c| c.load(Ordering::SeqCst)).collect::<Vec<_>>());
        // and the pool is whole: a normal run after the hand-driven ones
        let v = p.map(40, |i| i + 1);
        assert_eq!(v, (1..=40).collect::<Vec<_>>());
        assert_eq!(sh.active.load(Ordering::Acquire), 0);
    }

    /// The probe pass's shape (probebake::world_layer → probepass::probe_set_ilightdir per block): bursts of 7–11
    /// back-to-back maps of a few slices each — runs of 1–16 tasks whose set leaves most workers outside — between
    /// full-width runs, thousands of times; every slot written with its own value, the completion count back to zero
    /// after every run. ON THE OLD POOL THIS LOOP HANGS OR FAILS IN MOST RUNS (2 000 rounds, 32–256 workers: 4 of 6
    /// runs hung, one died with the old "pool task result" at pool.rs:220) — a run of 1–2 tasks whose members spin is
    /// over in a few hundred ns, while a worker outside its set, its three loads each missing on the line the members'
    /// claims and exits hammer, is still reading the finished run's words when the next run's stores land: the torn
    /// admission needs no preemption. The loop runs on its own thread under a watchdog, so a corrupted count fails the
    /// test instead of hanging the suite.
    fn probe_shape(threads: usize, rounds: usize) {
        let (tx, rx) = std::sync::mpsc::channel::<Result<usize, String>>();
        std::thread::spawn(move || {
            let p = Pool::new(threads);
            let sh = &p.shared;
            let mut seed = 0x9E37_79B9_7F4A_7C15u64 ^ threads as u64;
            let mut next = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
            let mut maps = 0usize;
            for round in 0..rounds {
                // a peel's blocks: 7–11 maps of 1–16 slices
                let blocks = 7 + (next() % 5) as usize;
                for b in 0..blocks {
                    let n = 1 + (next() % 16) as usize;
                    let v = p.map(n, |i| (round, b, i));
                    maps += 1;
                    if v.len() != n || !v.iter().enumerate().all(|(i, x)| *x == (round, b, i)) {
                        let _ = tx.send(Err(format!("round {round} block {b}: a {n}-task map returned {v:?}")));
                        return;
                    }
                    let a = sh.active.load(Ordering::Acquire);
                    if a != 0 {
                        let _ = tx.send(Err(format!("round {round} block {b}: active = {} after a {n}-task map", a as i64)));
                        return;
                    }
                }
                // the direction's wide runs (the raster, the gather): every worker in the set
                let n = threads * (1 + (next() % 4) as usize);
                let hits: Vec<AtomicUsize> = (0..n).map(|_| AtomicUsize::new(0)).collect();
                p.run(n, |i| { hits[i].fetch_add(1, Ordering::SeqCst); });
                maps += 1;
                if !hits.iter().all(|h| h.load(Ordering::SeqCst) == 1) {
                    let _ = tx.send(Err(format!("round {round}: a task of the {n}-task run ran {} times", hits.iter().map(|h| h.load(Ordering::SeqCst)).max().unwrap())));
                    return;
                }
                let a = sh.active.load(Ordering::Acquire);
                if a != 0 {
                    let _ = tx.send(Err(format!("round {round}: active = {} after a {n}-task run", a as i64)));
                    return;
                }
            }
            let _ = tx.send(Ok(maps));
        });
        match rx.recv_timeout(std::time::Duration::from_secs(60)) {
            Ok(Ok(maps)) => assert!(maps > 8 * rounds, "{maps} maps"),
            Ok(Err(e)) => panic!("{threads} workers: {e}"),
            Err(_) => panic!("{threads} workers: a run never completed (the completion count is corrupted: a worker was admitted to a run it was not counted in)"),
        }
    }

    #[test]
    fn probe_shape_thousands_of_tiny_maps_32_workers() {
        probe_shape(32, 2000);
    }

    #[test]
    fn probe_shape_thousands_of_tiny_maps_128_workers() {
        probe_shape(128, 2000);
    }

    /// The same shape under SPURIOUS WAKES: the workers are parked and stale (they sat out a 1-task run), and every one
    /// of them is unparked at a random moment around the next full-width run's publication. A wake is always legitimate
    /// for the protocol (`park` may return at any time, a stale token from an earlier tree wake does the same); no wake
    /// at any moment may admit a worker to a run it is not counted in, or touch a staged run.
    #[test]
    fn spurious_wakes_around_a_publication_admit_nobody() {
        let threads = 32;
        let p = Pool::new(threads);
        let sh = &p.shared;
        let handles = sh.handles.get().unwrap();
        let fire = AtomicUsize::new(0); // 0 idle, 1 wake everyone, 2 stop
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
        std::thread::scope(|sc| {
            sc.spawn(|| loop {
                match fire.load(Ordering::Acquire) {
                    2 => break,
                    1 => {
                        for h in handles { h.unpark(); }
                        fire.store(0, Ordering::Release);
                    }
                    _ => std::hint::spin_loop(),
                }
            });
            for round in 0..1500 {
                // everyone parks; a 1-task run takes worker 0 alone — 31 workers now hold a stale word
                std::thread::sleep(std::time::Duration::from_micros(250));
                let v = p.map(1, |i| i + round);
                assert_eq!(v, vec![round]);
                // the wakes go out; the publication follows after a random 0–40 µs, so the stale workers' reads of the
                // word fall before, inside and after the caller's stores
                fire.store(1, Ordering::Release);
                let delay_ns = (next() % 40_000) as u128;
                let t = std::time::Instant::now();
                while t.elapsed().as_nanos() < delay_ns { std::hint::spin_loop(); }
                let n = threads + (next() % 64) as usize;
                let hits: Vec<AtomicUsize> = (0..n).map(|_| AtomicUsize::new(0)).collect();
                p.run(n, |i| { hits[i].fetch_add(1, Ordering::SeqCst); });
                let a = sh.active.load(Ordering::Acquire);
                assert_eq!(a, 0, "round {round}: active = {a} (= {} as i64) after the wide run", a as i64);
                assert!(hits.iter().all(|h| h.load(Ordering::SeqCst) == 1), "round {round}: a task ran {} times", hits.iter().map(|h| h.load(Ordering::SeqCst)).max().unwrap());
                while fire.load(Ordering::Acquire) == 1 { std::hint::spin_loop(); }
            }
            fire.store(2, Ordering::Release);
        });
        let v = p.map(100, |i| i * 3);
        assert_eq!(v, (0..100).map(|i| i * 3).collect::<Vec<_>>());
    }

    /// A task's panic reaches the caller with the task's index and its message (the old pool said "a pool task
    /// panicked" and nothing else); the pool is whole afterwards.
    #[test]
    fn a_task_panic_names_the_task_and_the_message() {
        let p = Pool::new(4);
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // the expected panics stay out of the test output
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            p.map(8, |i| { if i == 5 { panic!("slice {i} is bad"); } i })
        }));
        std::panic::set_hook(hook);
        let msg = match r {
            Err(payload) => payload.downcast_ref::<String>().cloned().unwrap_or_default(),
            Ok(_) => panic!("the task's panic did not reach the caller"),
        };
        assert!(msg.contains("pool task 5 of 8 panicked: slice 5 is bad"), "{msg}");
        assert_eq!(p.shared.active.load(Ordering::Acquire), 0);
        assert_eq!(p.map(6, |i| i), (0..6).collect::<Vec<_>>());
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
            // publish by hand (the same steps as `run`: stage, then commit)
            let f = |_i: usize| { std::hint::black_box(0); };
            let fref: &(dyn Fn(usize) + Sync) = &f;
            let fptr: *const (dyn Fn(usize) + Sync) = unsafe { std::mem::transmute::<&(dyn Fn(usize) + Sync), &'static (dyn Fn(usize) + Sync)>(fref) };
            let (data, vtable): (*mut (), *mut ()) = unsafe { std::mem::transmute(fptr) };
            while sh.active.load(std::sync::atomic::Ordering::Acquire) != 0 { std::hint::spin_loop(); }
            let ta = t.elapsed();
            sh.stage(data, vtable, n, n, false);
            let tb = t.elapsed();
            sh.commit(n);
            let t1 = t.elapsed();
            while sh.next.load(std::sync::atomic::Ordering::Relaxed) < n { std::hint::spin_loop(); }
            let t2 = t.elapsed();
            while sh.active.load(std::sync::atomic::Ordering::Acquire) != 0 { std::hint::spin_loop(); }
            let t3 = t.elapsed();
            eprintln!("round {round}: prev left {:.1} µs, staged {:.1} µs, committed {:.1} µs, all claimed {:.1} µs, all left {:.1} µs", ta.as_secs_f64() * 1e6, tb.as_secs_f64() * 1e6, t1.as_secs_f64() * 1e6, t2.as_secs_f64() * 1e6, t3.as_secs_f64() * 1e6);
        }
    }
}

/// LMTOOL_POOL_STATS=1 (measurement): every task of every run timed; the profile report prints, per task
/// count, the runs' wall, the tasks' busy sum, busy / (threads · wall) and the longest task's share of the wall.
pub static POOL_STATS: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var_os("LMTOOL_POOL_STATS").is_some());

/// MEASUREMENT: per run (wall ns, busy ns, max task ns, n, threads).
pub static RUN_LOG: std::sync::Mutex<Vec<(u64, u64, u64, usize, usize)>> = std::sync::Mutex::new(Vec::new());
struct RunRecord<'a> { busy: &'a AtomicU64, maxt: &'a AtomicU64, n: usize, threads: usize, t0: std::time::Instant }
impl Drop for RunRecord<'_> {
    fn drop(&mut self) {
        let wall = self.t0.elapsed().as_nanos() as u64;
        if let Ok(mut l) = RUN_LOG.lock() { l.push((wall, self.busy.load(Ordering::Relaxed), self.maxt.load(Ordering::Relaxed), self.n, self.threads)); }
    }
}
