//! A persistent worker pool: `pool().run(n, |i| …)` executes the closure for i in 0..n on warm threads
//! and returns when every call has finished. Spawning a wave of 160 threads per stage costs tens of
//! milliseconds on a loaded host (the wave's start latency, not the work) — the bake runs ~10 such
//! waves per direction, so the threads stay alive and are woken through a condvar instead.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

struct Job {
    /// The task closure, its lifetime erased: `run` does not return before every task has completed,
    /// so the borrow it holds outlives every use.
    f: *const (dyn Fn(usize) + Sync),
    n: usize,
}
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

struct State {
    job: Option<Job>,
    generation: u64,
    /// Workers still inside the current job (started a task or about to check for one).
    busy: usize,
    /// Tasks handed out of the current job.
    next: AtomicUsize,
    /// Tasks finished in the current job.
    done: AtomicUsize,
    panicked: bool,
}

struct Shared {
    st: Mutex<State>,
    work: Condvar,
    finished: Condvar,
}

pub struct Pool {
    shared: Arc<Shared>,
    pub threads: usize,
}

impl Pool {
    pub fn new(threads: usize) -> Pool {
        let threads = threads.max(1);
        let shared = Arc::new(Shared {
            st: Mutex::new(State { job: None, generation: 0, busy: 0, next: AtomicUsize::new(0), done: AtomicUsize::new(0), panicked: false }),
            work: Condvar::new(),
            finished: Condvar::new(),
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
        if n == 1 {
            f(0);
            return;
        }
        let fref: &(dyn Fn(usize) + Sync) = &f;
        // erase the lifetime (see Job)
        let ptr: *const (dyn Fn(usize) + Sync) = unsafe { std::mem::transmute::<&(dyn Fn(usize) + Sync), &'static (dyn Fn(usize) + Sync)>(fref) };
        {
            let mut st = self.shared.st.lock().unwrap();
            while st.job.is_some() {
                // a nested / concurrent run: wait for the current job to clear
                st = self.shared.finished.wait(st).unwrap();
            }
            st.job = Some(Job { f: ptr, n });
            st.generation += 1;
            st.next.store(0, Ordering::SeqCst);
            st.done.store(0, Ordering::SeqCst);
            st.busy = self.threads;
            st.panicked = false;
        }
        self.shared.work.notify_all();
        // the caller helps
        let mut caller_panicked = false;
        loop {
            let i = {
                let st = self.shared.st.lock().unwrap();
                st.next.fetch_add(1, Ordering::SeqCst)
            };
            if i >= n {
                break;
            }
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(i))).is_err() {
                caller_panicked = true;
            }
            self.shared.st.lock().unwrap().done.fetch_add(1, Ordering::SeqCst);
        }
        // wait for the workers to drain
        let panicked = {
            let mut st = self.shared.st.lock().unwrap();
            while st.busy > 0 || st.done.load(Ordering::SeqCst) < n {
                st = self.shared.finished.wait(st).unwrap();
            }
            st.job = None;
            st.panicked
        };
        self.shared.finished.notify_all();
        if panicked || caller_panicked {
            panic!("a pool task panicked");
        }
    }

    /// `run` collecting one result per task, in task order.
    pub fn map<T: Send, F: Fn(usize) -> T + Sync>(&self, n: usize, f: F) -> Vec<T> {
        let slots: Vec<Mutex<Option<T>>> = (0..n).map(|_| Mutex::new(None)).collect();
        self.run(n, |i| {
            let v = f(i);
            *slots[i].lock().unwrap() = Some(v);
        });
        slots.into_iter().map(|m| m.into_inner().unwrap().expect("pool task result")).collect()
    }
}

fn worker(sh: Arc<Shared>) {
    let mut seen = 0u64;
    loop {
        let (job_ptr, n) = {
            let mut st = sh.st.lock().unwrap();
            while st.generation == seen || st.job.is_none() {
                if st.generation != seen && st.job.is_none() {
                    // a job we missed entirely (already cleared): skip its generation
                    seen = st.generation;
                }
                st = sh.work.wait(st).unwrap();
            }
            seen = st.generation;
            let j = st.job.as_ref().unwrap();
            (j.f, j.n)
        };
        let f: &(dyn Fn(usize) + Sync) = unsafe { &*job_ptr };
        loop {
            let i = {
                let st = sh.st.lock().unwrap();
                st.next.fetch_add(1, Ordering::SeqCst)
            };
            if i >= n {
                break;
            }
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(i)));
            let mut st = sh.st.lock().unwrap();
            if r.is_err() {
                st.panicked = true;
            }
            st.done.fetch_add(1, Ordering::SeqCst);
        }
        let mut st = sh.st.lock().unwrap();
        st.busy -= 1;
        if st.busy == 0 {
            sh.finished.notify_all();
        }
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
        let p = Pool::new(8);
        let hits: Vec<AtomicUsize> = (0..1000).map(|_| AtomicUsize::new(0)).collect();
        p.run(1000, |i| {
            hits[i].fetch_add(1, Ordering::SeqCst);
        });
        assert!(hits.iter().all(|h| h.load(Ordering::SeqCst) == 1));
        // and again (the generation advances)
        p.run(1000, |i| {
            hits[i].fetch_add(1, Ordering::SeqCst);
        });
        assert!(hits.iter().all(|h| h.load(Ordering::SeqCst) == 2));
        let v = p.map(50, |i| i * i);
        assert_eq!(v[7], 49);
    }
}
