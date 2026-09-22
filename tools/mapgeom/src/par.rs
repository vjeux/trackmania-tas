//! The bake worker pool of `tiny-library`: a job list run on every core, each
//! worker on a `DataStore::fork`, results handed back IN JOB ORDER.
//!
//! A bake is a pure function of its inputs (that is what `bake_cache` relies
//! on), so the jobs can run in any order and on any thread; what must stay
//! sequential is the bookkeeping that consumes the results (alias numbering,
//! the recipe -> alias table, the mapping rows), and `tiny_library::build`
//! keeps that loop exactly as it was, reading the pre-baked results instead
//! of baking in place. `TINY_BAKE_JOBS=N` sets the worker count (default: the
//! machine's parallelism; `1` = bake on the calling thread, in order).

use crate::store::DataStore;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// The worker count: `TINY_BAKE_JOBS`, else the available parallelism.
pub fn workers() -> usize {
    std::env::var("TINY_BAKE_JOBS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n >= 1)
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1))
}

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
