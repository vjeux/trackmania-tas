//! The bake worker pool of `tiny-library`: a job list run on every core, each
//! worker on a `DataStore::fork`, results handed back IN JOB ORDER.
//!
//! A bake is a pure function of its inputs (that is what `bake_cache` relies
//! on), so the jobs can run in any order and on any thread; what must stay
//! sequential is the bookkeeping that consumes the results (alias numbering,
//! the recipe -> alias table, the mapping rows), and `tiny_library::build`
//! keeps that loop exactly as it was, reading the pre-baked results instead
//! of baking in place. `TINY_BAKE_JOBS=N` sets the worker count (default: the
//! machine's parallelism, at most `DEFAULT_MAX_WORKERS`; `1` = bake on the
//! calling thread, in order).

use crate::store::DataStore;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// The worker count: `TINY_BAKE_JOBS`, else the available parallelism capped
/// at `DEFAULT_MAX_WORKERS`. The cap is measured, not principled: the bakes
/// are allocation-heavy and short (5 ms for a Stadium block), and past ~32
/// threads this VM spends more time in the kernel's page-fault and futex
/// spinlocks than baking — Summer 15's 358 blocks took 0.16 s on 16 workers,
/// 0.40 s on 32, 0.80 s on 64, 1.0 s on all 166; Summer 11's heavier BlueBay
/// blocks 1.44 / 0.96 / 0.67 / 0.80 s. 32 is the compromise for both kinds.
pub fn workers() -> usize {
    std::env::var("TINY_BAKE_JOBS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n >= 1)
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(DEFAULT_MAX_WORKERS))
}

/// See `workers`.
pub const DEFAULT_MAX_WORKERS: usize = 32;

/// `f` over every job, on up to `workers()` threads; `out[i]` is `f`'s result
/// for `jobs[i]`. Jobs are taken in list order (put the long ones first when
/// their cost is known). With one worker the jobs run on the calling thread
/// against `store` itself.
pub fn map<J: Sync, R: Send>(store: &mut DataStore, jobs: &[J], f: impl Fn(&mut DataStore, usize, &J) -> R + Sync) -> Vec<R> {
    let n = jobs.len();
    let workers = workers().min(n.max(1));
    if workers <= 1 {
        return jobs.iter().enumerate().map(|(i, j)| f(store, i, j)).collect();
    }
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..workers {
            let mut st = store.fork();
            let next = &next;
            let results = &results;
            let f = &f;
            // the bake walks deep node graphs: a roomy stack, like the main thread's
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn_scoped(s, move || loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let r = f(&mut st, i, &jobs[i]);
                    results.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
                })
                .expect("spawn bake worker");
        }
    });
    results.into_inner().unwrap_or_else(|e| e.into_inner()).into_iter().map(|r| r.expect("every job ran")).collect()
}

static PROCESS_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Called first thing in `main`: the instant the process started, for the
/// wall-time reports (a build's time includes opening the packs, before any
/// command runs).
pub fn process_started() {
    let _ = PROCESS_START.set(std::time::Instant::now());
}

/// Seconds since `process_started` (or since the first call, without it).
pub fn process_secs() -> f64 {
    PROCESS_START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

/// glibc malloc tuned for many short-lived worker threads (Linux/glibc only;
/// a no-op elsewhere): without it 64 workers baking 5 ms blocks spent 20 s of
/// kernel time on the heap's growth and trimming — mprotect / madvise / munmap
/// under the address-space lock, page faults on the re-grown pages — for 5 s
/// of actual work. Large blocks come from the heap, not from mmap, and freed
/// memory is kept for the next bake instead of handed back page by page.
pub fn tune_malloc() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn mallopt(param: std::os::raw::c_int, value: std::os::raw::c_int) -> std::os::raw::c_int;
        }
        const M_TRIM_THRESHOLD: std::os::raw::c_int = -1;
        const M_TOP_PAD: std::os::raw::c_int = -2;
        const M_MMAP_THRESHOLD: std::os::raw::c_int = -3;
        // SAFETY: plain libc calls with constant arguments, before any thread exists
        unsafe {
            mallopt(M_MMAP_THRESHOLD, 32 << 20);
            mallopt(M_TRIM_THRESHOLD, 1 << 30);
            mallopt(M_TOP_PAD, 64 << 20);
        }
    }
}
