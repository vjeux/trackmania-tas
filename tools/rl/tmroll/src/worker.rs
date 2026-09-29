//! `tmroll worker`: the per-box server (ROLLOUT-WORKER.md §0, §6). One master
//! connection at a time; N env threads sharing the per-map archive store; a
//! writer thread owns the socket's output side.

use crate::episode::{run_episode, save_store, store_entries, MapEnv, MapStore, Stores};
use crate::maps::MapSet;
use crate::policy::{load_policy, Policy};
use tmproto::*;
use std::collections::VecDeque;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub struct WorkerCfg {
    pub listen: String,
    pub server: PathBuf,
    pub shim: PathBuf,
    pub work: PathBuf,
    pub n_workers: usize,
    pub git_head: String,
}

enum Job {
    Episode { batch_id: u64, policy_id: u64, policy: Arc<dyn Policy>, req: EpisodeReq },
    /// Build the map, seed its human line (snap_every > 0), answer MapLoaded.
    LoadMap { uid: String, snap_every: u16 },
}

struct Shared {
    queue: Mutex<VecDeque<Job>>,
    cv: Condvar,
    running: AtomicUsize,
    done: AtomicU64,
    steps: AtomicU64,
    quit: AtomicBool,
    cancelled: Mutex<Vec<u64>>,
    stores: Stores,
}

/// Make sure the store for `uid` is loaded from its file (once).
fn ensure_store(sh: &Shared, a: &crate::maps::MapAssets) -> Result<(), String> {
    let mut s = sh.stores.lock().unwrap();
    if s.contains_key(&a.uid) {
        return Ok(());
    }
    let m = if a.archive.exists() { MapStore::load(&a.archive)? } else { MapStore { template: a.template_name.clone(), ..Default::default() } };
    if m.template != a.template_name {
        return Err(format!("{}: archive driven in template {:?}, this worker built {:?}", a.archive.display(), m.template, a.template_name));
    }
    s.insert(a.uid.clone(), m);
    Ok(())
}

fn env_thread(idx: usize, cfg: Arc<WorkerCfg>, maps: Arc<MapSet>, sh: Arc<Shared>, tx: mpsc::Sender<Frame>) {
    let mut cur: Option<MapEnv> = None;
    let work = cfg.work.join(format!("w{idx}"));
    let rebuild = |cur: &mut Option<MapEnv>, uid: &str, k: u16, ov: u32, margin: f32| -> Result<(), String> {
        let need = match cur {
            Some(m) => m.uid != uid || m.k_ticks != k || m.obs_version != ov || m.margin_m != margin,
            None => true,
        };
        if need {
            *cur = None; // the old server goes first
            let a = maps.get(uid)?;
            ensure_store(&sh, &a)?;
            let _ = std::fs::remove_dir_all(&work);
            std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
            *cur = Some(MapEnv::build(&cfg.server, &cfg.shim, &work, &a, k, ov, margin)?);
        }
        Ok(())
    };
    loop {
        let job = {
            let mut q = sh.queue.lock().unwrap();
            loop {
                if sh.quit.load(Ordering::Relaxed) {
                    return;
                }
                // prefer a job on the map this thread already holds (a map switch
                // is a server restart, ~3 s); else the head of the queue
                let mine = cur.as_ref().map(|m| m.uid.clone());
                let pick = mine
                    .as_ref()
                    .and_then(|u| q.iter().position(|j| matches!(j, Job::Episode { req, .. } if &req.map_uid == u)))
                    .unwrap_or(0);
                if let Some(j) = q.remove(pick) {
                    break j;
                }
                q = sh.cv.wait_timeout(q, Duration::from_millis(200)).unwrap().0;
            }
        };
        sh.running.fetch_add(1, Ordering::Relaxed);
        let f = match job {
            Job::LoadMap { uid, snap_every } => {
                let r = (|| -> Result<u32, String> {
                    rebuild(&mut cur, &uid, 10, 2, 4.0)?;
                    let a = maps.get(&uid)?;
                    let me = cur.as_mut().unwrap();
                    let already = store_entries(&sh.stores, &uid).iter().any(|e| e.origin == 0);
                    if snap_every > 0 && !already {
                        if a.donor_actions.is_empty() {
                            // a no-ghost map (prebuilt template): nothing to seed
                        } else {
                                                    me.seed_human_line(&sh.stores, &a.donor_actions, snap_every as usize)?;
                        }
                        save_store(&sh.stores, &uid, &a.archive)?;
                    }
                    Ok(store_entries(&sh.stores, &uid).len() as u32)
                })();
                match r {
                    Ok(n) => Frame::MapLoaded { map_uid: uid, ok: true, err: String::new(), n_states: n },
                    Err(e) => {
                        cur = None;
                        Frame::MapLoaded { map_uid: uid, ok: false, err: e, n_states: 0 }
                    }
                }
            }
            Job::Episode { batch_id, policy_id, policy, req } => {
                if sh.cancelled.lock().unwrap().contains(&batch_id) {
                    sh.running.fetch_sub(1, Ordering::Relaxed);
                    continue;
                }
                let res = (|| -> Result<EpisodeOut, String> {
                    if let Some(pk) = policy.chunk_k() {
                        if pk != req.k_ticks as usize {
                            return Err(format!("the policy samples chunks of {pk} ticks; the episode asks k_ticks {}", req.k_ticks));
                        }
                    }
                    rebuild(&mut cur, &req.map_uid, req.k_ticks, policy.obs_version(), req.margin_m)?;
                    let me = cur.as_mut().unwrap();
                    match run_episode(me, &sh.stores, policy.as_ref(), &req, batch_id, policy_id) {
                        Ok(e) => Ok(e),
                        Err(e) => {
                            cur = None; // an env error: this server is suspect
                            Err(e)
                        }
                    }
                })();
                sh.done.fetch_add(1, Ordering::Relaxed);
                match res {
                    Ok(e) => {
                        sh.steps.fetch_add(e.steps.len() as u64, Ordering::Relaxed);
                        Frame::Episode(Box::new(e))
                    }
                    Err(err) => Frame::EpisodeError { ep_id: req.ep_id, err },
                }
            }
        };
        sh.running.fetch_sub(1, Ordering::Relaxed);
        if tx.send(f).is_err() {
            return;
        }
    }
}

/// A second connection while a master holds the envs: answer Hello (a health
/// check), refuse work, close on Quit.
fn serve_busy(stream: TcpStream, cfg: &Arc<WorkerCfg>) -> Result<(), String> {
    let mut rd = stream.try_clone().map_err(|e| e.to_string())?;
    let mut wr = stream;
    loop {
        let Some(f) = read_frame(&mut rd)? else { return Ok(()) };
        match f {
            Frame::Hello { .. } => write_frame(
                &mut wr,
                &Frame::Ready {
                    proto_version: PROTO_VERSION,
                    worker_id: format!("{} (BUSY: another master holds the envs)", hostname()),
                    n_workers: cfg.n_workers as u32,
                    obs_versions: 0b111,
                    state_version: tmstate::STATE_VERSION,
                    git_head: cfg.git_head.clone(),
                },
            )?,
            Frame::Quit => return Ok(()),
            Frame::RunEpisodes { episodes, .. } => {
                for e in episodes {
                    write_frame(&mut wr, &Frame::EpisodeError { ep_id: e.ep_id, err: "worker busy: another master holds the envs".into() })?;
                }
            }
            Frame::LoadMaps { uids, .. } => {
                for u in uids {
                    write_frame(&mut wr, &Frame::MapLoaded { map_uid: u, ok: false, err: "worker busy".into(), n_states: 0 })?;
                }
            }
            Frame::SetPolicy { policy_id, .. } => write_frame(&mut wr, &Frame::PolicyAck { policy_id, ok: false, err: "worker busy".into() })?,
            Frame::ArchiveList { map_uid } => write_frame(&mut wr, &Frame::Archive { map_uid, entries: Vec::new() })?,
            _ => {}
        }
    }
}

fn serve_one(stream: TcpStream, cfg: &Arc<WorkerCfg>, maps: &Arc<MapSet>, stores: &Stores) -> Result<(), String> {
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    eprintln!("tmroll: master connected from {peer}");
    let mut rd = stream.try_clone().map_err(|e| e.to_string())?;
    let mut wr = stream;
    let (tx, rx) = mpsc::channel::<Frame>();
    let sh = Arc::new(Shared {
        queue: Mutex::new(VecDeque::new()),
        cv: Condvar::new(),
        running: AtomicUsize::new(0),
        done: AtomicU64::new(0),
        steps: AtomicU64::new(0),
        quit: AtomicBool::new(false),
        cancelled: Mutex::new(Vec::new()),
        stores: stores.clone(),
    });
    let mut threads = Vec::new();
    for i in 0..cfg.n_workers {
        let (c, m, s, t) = (cfg.clone(), maps.clone(), sh.clone(), tx.clone());
        threads.push(std::thread::spawn(move || env_thread(i, c, m, s, t)));
    }
    let writer = {
        let sh2 = sh.clone();
        std::thread::spawn(move || {
            let mut last_stats = Instant::now();
            let mut last_steps = 0u64;
            let mut batch_id = 0u64;
            loop {
                match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(f) => {
                        if let Frame::Episode(e) = &f {
                            batch_id = e.batch_id;
                        }
                        if write_frame(&mut wr, &f).is_err() {
                            return;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
                if sh2.quit.load(Ordering::Relaxed) {
                    return;
                }
                if last_stats.elapsed() >= Duration::from_secs(2) {
                    let steps = sh2.steps.load(Ordering::Relaxed);
                    let rate = (steps - last_steps) as f32 / last_stats.elapsed().as_secs_f32();
                    last_steps = steps;
                    last_stats = Instant::now();
                    let queued = sh2.queue.lock().unwrap().len() as u32;
                    let running = sh2.running.load(Ordering::Relaxed) as u32;
                    if queued + running > 0 {
                        let load1 = std::fs::read_to_string("/proc/loadavg")
                            .ok()
                            .and_then(|s| s.split_whitespace().next().and_then(|x| x.parse().ok()))
                            .unwrap_or(0.0);
                        let f = Frame::Stats { batch_id, done: sh2.done.load(Ordering::Relaxed) as u32, running, queued, env_steps_per_s: rate, load1 };
                        if write_frame(&mut wr, &f).is_err() {
                            return;
                        }
                    }
                }
            }
        })
    };
    let mut policy: Option<(u64, Arc<dyn Policy>)> = None;
    let result = (|| -> Result<(), String> {
        loop {
            let Some(f) = read_frame(&mut rd)? else { return Ok(()) };
            match f {
                Frame::Hello { proto_version, master_id } => {
                    if proto_version != PROTO_VERSION {
                        return Err(format!("master {master_id} speaks protocol {proto_version}, this worker {PROTO_VERSION}"));
                    }
                    tx.send(Frame::Ready {
                        proto_version: PROTO_VERSION,
                        worker_id: hostname(),
                        n_workers: cfg.n_workers as u32,
                        obs_versions: 0b111,
                        state_version: tmstate::STATE_VERSION,
                        git_head: cfg.git_head.clone(),
                    })
                    .map_err(|e| e.to_string())?;
                }
                Frame::SetPolicy { policy_id, obs_version, tmw } => {
                    let ack = match load_policy(&tmw, obs_version) {
                        Ok(p) => {
                            policy = Some((policy_id, Arc::from(p)));
                            Frame::PolicyAck { policy_id, ok: true, err: String::new() }
                        }
                        Err(e) => Frame::PolicyAck { policy_id, ok: false, err: e },
                    };
                    tx.send(ack).map_err(|e| e.to_string())?;
                }
                Frame::RunEpisodes { batch_id, episodes } => {
                    let Some((pid, p)) = &policy else {
                        for e in &episodes {
                            tx.send(Frame::EpisodeError { ep_id: e.ep_id, err: "no policy set".into() }).map_err(|e| e.to_string())?;
                        }
                        continue;
                    };
                    let mut q = sh.queue.lock().unwrap();
                    for req in episodes {
                        q.push_back(Job::Episode { batch_id, policy_id: *pid, policy: p.clone(), req });
                    }
                    drop(q);
                    sh.cv.notify_all();
                }
                Frame::Cancel { batch_id } => {
                    sh.cancelled.lock().unwrap().push(batch_id);
                    sh.queue.lock().unwrap().retain(|j| !matches!(j, Job::Episode { batch_id: b, .. } if *b == batch_id));
                }
                Frame::LoadMaps { uids, snap_every } => {
                    let mut q = sh.queue.lock().unwrap();
                    for uid in uids {
                        q.push_back(Job::LoadMap { uid, snap_every });
                    }
                    drop(q);
                    sh.cv.notify_all();
                }
                Frame::ArchiveList { map_uid } => {
                    let entries = store_entries(&sh.stores, &map_uid);
                    tx.send(Frame::Archive { map_uid, entries }).map_err(|e| e.to_string())?;
                }
                Frame::Quit => return Ok(()),
                other => return Err(format!("a master must not send {other:?}")),
            }
        }
    })();
    sh.quit.store(true, Ordering::Relaxed);
    sh.cv.notify_all();
    drop(tx);
    for t in threads {
        let _ = t.join();
    }
    let _ = writer.join();
    // persist every dirty store
    for (uid, a) in maps.all() {
        let _ = save_store(&sh.stores, &uid, &a.archive);
    }
    eprintln!("tmroll: master {peer} gone ({result:?})");
    result
}

pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname").map(|s| s.trim().to_string()).unwrap_or_else(|_| "?".into())
}

pub fn serve(cfg: WorkerCfg, maps_dir: &Path) -> Result<(), String> {
    let cfg = Arc::new(cfg);
    let maps = Arc::new(MapSet::new(maps_dir, &cfg.work));
    let stores: Stores = Arc::new(Mutex::new(Default::default()));
    std::fs::create_dir_all(&cfg.work).map_err(|e| e.to_string())?;
    tmenv::warn_if_not_tmpfs(&cfg.work);
    let listener = TcpListener::bind(&cfg.listen).map_err(|e| format!("{}: {e}", cfg.listen))?;
    eprintln!("tmroll worker: {} listening on {}, {} env workers, maps {}", hostname(), cfg.listen, cfg.n_workers, maps_dir.display());
    let busy = Arc::new(AtomicBool::new(false));
    for s in listener.incoming() {
        match s {
            Ok(s) => {
                let _ = s.set_nodelay(true);
                if busy.swap(true, Ordering::SeqCst) {
                    // a master already holds the envs: a side thread answers this one
                    let c = cfg.clone();
                    std::thread::spawn(move || {
                        let _ = serve_busy(s, &c);
                    });
                    continue;
                }
                let (c, m, st, b) = (cfg.clone(), maps.clone(), stores.clone(), busy.clone());
                std::thread::spawn(move || {
                    if let Err(e) = serve_one(s, &c, &m, &st) {
                        eprintln!("tmroll: connection ended: {e}");
                    }
                    b.store(false, Ordering::SeqCst);
                });
            }
            Err(e) => eprintln!("tmroll: accept: {e}"),
        }
    }
    Ok(())
}
