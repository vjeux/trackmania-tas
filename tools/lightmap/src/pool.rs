//! A persistent worker pool: `pool().run(n, |i| …)` executes the closure for i in 0..n on warm threads
//! and returns when every call has finished. Spawning a wave of 160 threads per stage costs tens of
//! milliseconds on a loaded host (the wave's start latency, not the work) — the bake runs ~30 such
//! waves per direction, so the threads stay alive and are woken through a condvar instead.
//!
//! The task claims and completions are lock-free atomics (a mutex around every claim with 160 workers
//! and 640 tasks put the kernel's futex lock at 7 % of a bake); an idle worker spins briefly on the
//! generation counter before parking, so the back-to-back waves within a direction never round-trip
//! through the futex.

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
}

pub struct Pool {
    shared: Arc<Shared>,
    pub threads: usize,
}

/// Idle spins on the generation counter before a worker parks (~10–50 µs).
const SPIN: usize = 4000;

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
        });
        for _ in 0..threads {
            let sh = shared.clone();
            std::thread::Builder::new()
                .name("lm-pool".into())
                .spawn(move || worker(sh))
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
            *job = Some(Job { f: fptr, n });
            sh.generation.fetch_add(1, Ordering::SeqCst);
        }
        sh.work.notify_all();
        // the caller helps
        loop {
            let i = sh.next.fetch_add(1, Ordering::SeqCst);
            if i >= n {
                break;
            }
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(i)));
            if r.is_err() {
                sh.panicked.store(true, Ordering::SeqCst);
            }
            sh.done.fetch_add(1, Ordering::SeqCst);
        }
        // every task done, then every worker out of the job (a worker may still be between its last
        // claim and its exit)
        let mut spins = 0usize;
        while sh.done.load(Ordering::Acquire) < n || sh.active.load(Ordering::Acquire) != 0 {
            spins += 1;
            if spins < 20_000 { std::hint::spin_loop(); } else { std::thread::yield_now(); }
        }
        *sh.job.lock().unwrap() = None;
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

fn worker(sh: Arc<Shared>) {
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
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(i)));
            if r.is_err() {
                sh.panicked.store(true, Ordering::SeqCst);
            }
            sh.done.fetch_add(1, Ordering::SeqCst);
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
