//! Run one closure per ghost on a pool of workers, each worker owning one
//! fork server in its own scratch directory. Keeps ≥ 8 cores free by
//! construction: the caller passes `workers`, `pool::cap` clamps it.

use crate::rig::Worker;
use crate::tele::Telemetry;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct PoolCfg {
    pub server: PathBuf,
    pub map: PathBuf,
    pub shim: PathBuf,
    pub work_root: PathBuf,
    pub workers: usize,
    pub verbose: bool,
}

/// Never more workers than cores − 8, never fewer than 1.
pub fn cap(workers: usize) -> usize {
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(16);
    workers.clamp(1, cores.saturating_sub(8).max(1))
}

/// For every ghost, on some worker: start a server on it, load its telemetry,
/// call `f`. Results come back in ghost order; a failed ghost is `Err` with the
/// reason and the run continues (a failed control on one ghost is a fact to
/// report, not a reason to lose the other 43).
pub fn run_per_ghost<R, F>(cfg: &PoolCfg, ghosts: &[PathBuf], f: F) -> Vec<Result<R, String>>
where
    R: Send + 'static,
    F: Fn(usize, &mut Worker, &Telemetry) -> Result<R, String> + Send + Sync + 'static,
{
    let n = ghosts.len();
    let f = Arc::new(f);
    let queue: Arc<Mutex<std::collections::VecDeque<usize>>> = Arc::new(Mutex::new((0..n).collect()));
    let results: Arc<Mutex<Vec<Option<Result<R, String>>>>> = Arc::new(Mutex::new((0..n).map(|_| None).collect()));
    let ghosts: Arc<Vec<PathBuf>> = Arc::new(ghosts.to_vec());
    let workers = cap(cfg.workers).min(n.max(1));
    let (server, map, shim, root, verbose) = (cfg.server.clone(), cfg.map.clone(), cfg.shim.clone(), cfg.work_root.clone(), cfg.verbose);
    let mut hs = Vec::new();
    for wi in 0..workers {
        let (queue, results, ghosts, f) = (queue.clone(), results.clone(), ghosts.clone(), f.clone());
        let (server, map, shim, root) = (server.clone(), map.clone(), shim.clone(), root.clone());
        hs.push(std::thread::spawn(move || loop {
            let gi = match queue.lock().unwrap().pop_front() {
                Some(g) => g,
                None => break,
            };
            let work = root.join(format!("w{}", wi));
            let r = one(&server, &map, &shim, &work, &ghosts[gi], verbose, gi, &*f);
            results.lock().unwrap()[gi] = Some(r);
        }));
    }
    for h in hs {
        let _ = h.join();
    }
    let mut out = results.lock().unwrap();
    out.drain(..).map(|r| r.unwrap_or_else(|| Err("worker panicked".into()))).collect()
}

fn one<R, F>(server: &Path, map: &Path, shim: &Path, work: &Path, ghost: &Path, verbose: bool, gi: usize, f: &F) -> Result<R, String>
where
    F: Fn(usize, &mut Worker, &Telemetry) -> Result<R, String>,
{
    let tel = Telemetry::load(&ghost.to_string_lossy())?;
    let mut w = Worker::start(server, map, shim, work, ghost, verbose)?;
    let r = f(gi, &mut w, &tel);
    drop(w);
    r
}

/// `*.Ghost.Gbx` in a directory, sorted by name.
pub fn ghosts_in(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {}", dir.display(), e))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".Ghost.Gbx"))
        .collect();
    v.sort();
    Ok(v)
}
