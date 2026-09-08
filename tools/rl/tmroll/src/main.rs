//! tmroll -- the multi-box rollout worker and its controls (ROLLOUT-WORKER.md).
//!
//!   tmroll worker  --listen 0.0.0.0:7001 --maps-dir D --workers 96 --work /dev/shm/rw
//!   tmroll control --maps-dir D --map UID [--worker host:port] [--episodes 50] --work /dev/shm/rwc
//!       the byte-identity control: N episodes run locally (this process, the
//!       same code path) against the same N from a worker (a loopback one this
//!       binary starts, or --worker's), frame for frame.

mod episode;
mod maps;
mod policy;
use tmproto as protocol;
mod worker;

use protocol::*;
use std::net::TcpStream;
use std::path::PathBuf;

fn flag(a: &[String], k: &str) -> Option<String> {
    a.iter().position(|x| x == k).and_then(|i| a.get(i + 1).cloned())
}
fn num<T: std::str::FromStr>(a: &[String], k: &str, d: T) -> T {
    flag(a, k).and_then(|v| v.parse().ok()).unwrap_or(d)
}
fn die(m: String) -> ! {
    eprintln!("tmroll: {m}");
    std::process::exit(2)
}

fn server_shim(a: &[String]) -> (PathBuf, PathBuf) {
    let server = PathBuf::from(
        flag(a, "--server").or_else(|| std::env::var("TM_SERVER").ok()).unwrap_or_else(|| "/tmp/tmoracle/server".into()),
    );
    let shim = PathBuf::from(
        flag(a, "--shim").or_else(|| std::env::var("FK_SHIM").ok()).unwrap_or_else(|| "/tmp/tmtas/tools/search/target/release/libforkshim.so".into()),
    );
    (server, shim)
}

fn git_head() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn cmd_worker(a: &[String]) {
    let (server, shim) = server_shim(a);
    let maps_dir = PathBuf::from(flag(a, "--maps-dir").unwrap_or_else(|| die("--maps-dir DIR".into())));
    let cfg = worker::WorkerCfg {
        listen: flag(a, "--listen").unwrap_or_else(|| "[::]:7001".into()),
        server,
        shim,
        work: PathBuf::from(flag(a, "--work").unwrap_or_else(|| format!("/dev/shm/tmroll/{}", std::process::id()))),
        n_workers: num(a, "--workers", 96),
        git_head: git_head(),
    };
    worker::serve(cfg, &maps_dir).unwrap_or_else(|e| die(e));
}

/// A master's view of one worker: send frames, collect episodes.
struct Master {
    s: TcpStream,
}
impl Master {
    fn connect(addr: &str) -> Result<Master, String> {
        let s = TcpStream::connect(addr).map_err(|e| format!("{addr}: {e}"))?;
        let _ = s.set_nodelay(true);
        let mut m = Master { s };
        m.send(&Frame::Hello { proto_version: PROTO_VERSION, master_id: "tmroll-control".into() })?;
        match m.recv()? {
            Frame::Ready { proto_version, worker_id, n_workers, .. } => {
                eprintln!("control: worker {worker_id} ready, protocol {proto_version}, {n_workers} envs");
            }
            f => return Err(format!("expected Ready, got {f:?}")),
        }
        Ok(m)
    }
    fn send(&mut self, f: &Frame) -> Result<(), String> {
        write_frame(&mut self.s, f)
    }
    fn recv(&mut self) -> Result<Frame, String> {
        read_frame(&mut self.s)?.ok_or_else(|| "worker closed the connection".to_string())
    }
    /// Run a batch and collect its episodes (Stats frames are printed).
    fn run(&mut self, batch_id: u64, reqs: &[EpisodeReq]) -> Result<Vec<Frame>, String> {
        self.send(&Frame::RunEpisodes { batch_id, episodes: reqs.to_vec() })?;
        let mut out = Vec::new();
        while out.len() < reqs.len() {
            match self.recv()? {
                Frame::Stats { done, running, queued, env_steps_per_s, load1, .. } => {
                    eprintln!("  worker: done {done} running {running} queued {queued}  {env_steps_per_s:.0} env-steps/s  load {load1:.1}");
                }
                f @ Frame::Episode(_) | f @ Frame::EpisodeError { .. } => out.push(f),
                f => return Err(format!("unexpected {f:?}")),
            }
        }
        Ok(out)
    }
}

fn cmd_control(a: &[String]) {
    let (server, shim) = server_shim(a);
    let maps_dir = PathBuf::from(flag(a, "--maps-dir").unwrap_or_else(|| die("--maps-dir DIR".into())));
    let uid = flag(a, "--map").unwrap_or_else(|| die("--map UID".into()));
    let n: usize = num(a, "--episodes", 50);
    let k: u16 = num(a, "--k", 10);
    let max_steps: u32 = num(a, "--max-steps", 60);
    let obs_version: u32 = num(a, "--obs-version", 2);
    let work = PathBuf::from(flag(a, "--work").unwrap_or_else(|| format!("/dev/shm/tmroll-control/{}", std::process::id())));
    std::fs::create_dir_all(&work).unwrap_or_else(|e| die(e.to_string()));
    println!("# tmroll control  map {uid}  {n} episodes  k {k}  max_steps {max_steps}  obs v{obs_version}");

    // the worker: --worker host:port, else a loopback one in this process
    let addr = match flag(a, "--worker") {
        Some(w) => w,
        None => {
            let listen = "127.0.0.1:0".to_string();
            let l = std::net::TcpListener::bind(&listen).unwrap_or_else(|e| die(e.to_string()));
            let addr = l.local_addr().unwrap().to_string();
            drop(l);
            let cfg = worker::WorkerCfg {
                listen: addr.clone(),
                server: server.clone(),
                shim: shim.clone(),
                work: work.join("worker"),
                n_workers: num(a, "--workers", 8),
                git_head: git_head(),
            };
            let md = maps_dir.clone();
            std::thread::spawn(move || worker::serve(cfg, &md));
            std::thread::sleep(std::time::Duration::from_millis(300));
            addr
        }
    };

    let reqs: Vec<EpisodeReq> = (0..n as u64)
        .map(|i| EpisodeReq {
            ep_id: i,
            map_uid: uid.clone(),
            start: 0,
            state_id: 0,
            seed: 0x9E3779B97F4A7C15u64.wrapping_mul(i + 1),
            temperature: 1.0,
            max_steps,
            k_ticks: k,
            flags: FLAG_ROWS | FLAG_TAPE,
            snap_every: 0,
            margin_m: 4.0,
        })
        .collect();

    // remote half
    let t0 = std::time::Instant::now();
    let mut m = Master::connect(&addr).unwrap_or_else(|e| die(e));
    let tmw = flag(a, "--policy").map(|p| std::fs::read(&p).unwrap_or_else(|e| die(format!("{p}: {e}")))).unwrap_or_default();
    m.send(&Frame::SetPolicy { policy_id: 1, obs_version, tmw }).unwrap_or_else(|e| die(e));
    match m.recv().unwrap_or_else(|e| die(e)) {
        Frame::PolicyAck { ok: true, .. } => {}
        f => die(format!("policy refused: {f:?}")),
    }
    let remote = m.run(1, &reqs).unwrap_or_else(|e| die(e));
    let remote_s = t0.elapsed().as_secs_f64();
    // the archive half: LoadMaps seeds the human line; episodes start from its states
    let snap_every: u16 = num(a, "--snap-every", 50);
    let n_arch: usize = num(a, "--archive-episodes", 10);
    m.send(&Frame::LoadMaps { uids: vec![uid.clone()], snap_every }).unwrap_or_else(|e| die(e));
    let n_states = loop {
        match m.recv().unwrap_or_else(|e| die(e)) {
            Frame::MapLoaded { ok: true, n_states, .. } => break n_states,
            Frame::MapLoaded { ok: false, err, .. } => die(format!("LoadMaps: {err}")),
            Frame::Stats { .. } => {}
            f => die(format!("unexpected {f:?}")),
        }
    };
    m.send(&Frame::ArchiveList { map_uid: uid.clone() }).unwrap_or_else(|e| die(e));
    let entries = loop {
        match m.recv().unwrap_or_else(|e| die(e)) {
            Frame::Archive { entries, .. } => break entries,
            Frame::Stats { .. } => {}
            f => die(format!("unexpected {f:?}")),
        }
    };
    let human: Vec<&tmproto::ArchiveEntry> = entries.iter().filter(|e| e.origin == 0).collect();
    println!("remote archive: {n_states} states after LoadMaps (snap every {snap_every} ticks), {} human-line", human.len());
    let arch_reqs: Vec<EpisodeReq> = (0..n_arch.min(human.len()))
        .map(|i| {
            let e = human[i * human.len() / n_arch.min(human.len()).max(1)];
            EpisodeReq { ep_id: 1000 + i as u64, map_uid: uid.clone(), start: 1, state_id: e.state_id, seed: 0x51ED270693u64.wrapping_mul(i as u64 + 1), temperature: 1.0, max_steps, k_ticks: k, flags: FLAG_ROWS | FLAG_TAPE, snap_every: 0, margin_m: 4.0 }
        })
        .collect();
    let remote_arch = if arch_reqs.is_empty() { Vec::new() } else { m.run(2, &arch_reqs).unwrap_or_else(|e| die(e)) };
    let _ = m.send(&Frame::Quit);
    let mut remote_by: std::collections::HashMap<u64, Frame> = std::collections::HashMap::new();
    let mut errors = 0usize;
    for f in remote.into_iter().chain(remote_arch.into_iter()) {
        match &f {
            Frame::Episode(e) => {
                remote_by.insert(e.ep_id, f);
            }
            Frame::EpisodeError { ep_id, err } => {
                errors += 1;
                println!("  remote ep {ep_id}: ERROR {err}");
            }
            _ => {}
        }
    }
    println!("remote: {} episodes, {errors} errors in {remote_s:.1} s", remote_by.len());

    // local half: the same code path, this process, one env
    let t1 = std::time::Instant::now();
    let maps = maps::MapSet::new(&maps_dir, &work.join("local"));
    let assets = maps.get(&uid).unwrap_or_else(|e| die(e));
    let policy = match flag(a, "--policy") {
        Some(p) => policy::load_policy(&std::fs::read(&p).unwrap_or_else(|e| die(format!("{p}: {e}"))), obs_version).unwrap_or_else(|e| die(e)),
        None => policy::load_policy(&[], obs_version).unwrap_or_else(|e| die(e)),
    };
    let stores: episode::Stores = std::sync::Arc::new(std::sync::Mutex::new(Default::default()));
    let mut me = episode::MapEnv::build(&server, &shim, &work.join("local").join("env"), &assets, k, obs_version, 4.0).unwrap_or_else(|e| die(e));
    if !arch_reqs.is_empty() {
        let n_local = me.seed_human_line(&stores, &assets.donor_actions, snap_every as usize).unwrap_or_else(|e| die(e));
        println!("local archive: {n_local} human-line states seeded");
    }
    let mut same = 0usize;
    let mut differ = 0usize;
    let mut missing = 0usize;
    let all_reqs: Vec<EpisodeReq> = reqs.iter().cloned().chain(arch_reqs.iter().cloned()).collect();
    for r in &all_reqs {
        let batch = if r.start == 1 { 2 } else { 1 };
        let local = episode::run_episode(&mut me, &stores, policy.as_ref(), r, batch, 1).unwrap_or_else(|e| die(e));
        let Some(Frame::Episode(rem)) = remote_by.get(&r.ep_id) else {
            missing += 1;
            continue;
        };
        // wall_ms is the one field allowed to differ
        let mut l = local.clone();
        let mut rr = (**rem).clone();
        l.wall_ms = 0;
        rr.wall_ms = 0;
        if Frame::Episode(Box::new(l.clone())).encode() == Frame::Episode(Box::new(rr.clone())).encode() {
            same += 1;
        } else {
            differ += 1;
            let first_step = l.steps.iter().zip(rr.steps.iter()).position(|(x, y)| x != y);
            let rows_eq = l.rows == rr.rows;
            let first_row = l.rows.chunks(120).zip(rr.rows.chunks(120)).position(|(x, y)| x != y);
            let tape_eq = l.tape == rr.tape;
            let (mut lh, mut rh) = (l.clone(), rr.clone());
            lh.steps.clear(); rh.steps.clear(); lh.rows.clear(); rh.rows.clear(); lh.tape.clear(); rh.tape.clear();
            println!(
                "  ep {}: DIFFERS  steps {} vs {}  done {} vs {}  reward {:.4} vs {:.4}  rows {} vs {} B (equal {rows_eq}, first differing row {:?})  tape {} vs {} B (equal {tape_eq})  header equal {}  first differing step {:?}  best_s {} vs {}  worker_ticks {} vs {}",
                r.ep_id, l.steps.len(), rr.steps.len(), l.done, rr.done, l.reward_sum, rr.reward_sum, l.rows.len(), rr.rows.len(), first_row, l.tape.len(), rr.tape.len(), lh == rh, first_step, l.best_s, rr.best_s, l.worker_ticks, rr.worker_ticks
            );
        }
    }
    println!("local: {} episodes in {:.1} s", all_reqs.len(), t1.elapsed().as_secs_f64());
    let pass = differ == 0 && missing == 0 && errors == 0 && same == all_reqs.len();
    println!("ROLLOUT-WORKER byte identity  {same} same ({} from the root, {} from human-line archive states), {differ} differ, {missing} missing  {}", reqs.len(), arch_reqs.len(), if pass { "PASS" } else { "FAIL" });
    if !pass {
        std::process::exit(1);
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(|s| s.as_str()) {
        Some("worker") => cmd_worker(&a),
        Some("control") => cmd_control(&a),
        Some("bench") => cmd_bench(&a),
        Some("status") => cmd_status(&a),
        _ => {
            eprintln!("tmroll worker --listen [::]:7001 --maps-dir D --workers N --work /dev/shm/rw\n\
                       tmroll control --maps-dir D --map UID [--worker host:port] [--episodes 50] [--k 10] [--max-steps 60] [--obs-version 2]");
            std::process::exit(2);
        }
    }
}

/// `tmroll bench --workers h1:7001,h2:7001 --map UID --episodes 2000 [--k 10 --max-steps 60]`:
/// the throughput table -- every worker gets the same episode list at once,
/// env-steps/s and episodes/s summed over the boxes.
fn cmd_bench(a: &[String]) {
    // `--map a,b,c` round-robins the episodes over several maps (the map-switch cost)
    let uids: Vec<String> = flag(a, "--map").unwrap_or_else(|| die("--map UID[,UID...]".into())).split(',').map(|s| s.to_string()).collect();
    let uid = uids.join(",");
    let workers: Vec<String> = flag(a, "--workers").unwrap_or_else(|| die("--workers h:p,...".into())).split(',').map(|s| s.to_string()).collect();
    let n: usize = num(a, "--episodes", 2000);
    let k: u16 = num(a, "--k", 10);
    let max_steps: u32 = num(a, "--max-steps", 60);
    let obs_version: u32 = num(a, "--obs-version", 2);
    let flags: u8 = num(a, "--flags", 0);
    println!("# tmroll bench  {} worker box(es)  map {uid}  {n} episodes per box  k {k}  max_steps {max_steps}  flags {flags}", workers.len());
    let reqs: Vec<EpisodeReq> = (0..n as u64)
        .map(|i| EpisodeReq { ep_id: i, map_uid: uids[i as usize % uids.len()].clone(), start: 0, state_id: 0, seed: 0x9E3779B97F4A7C15u64.wrapping_mul(i + 1), temperature: 1.0, max_steps, k_ticks: k, flags, snap_every: 0, margin_m: 4.0 })
        .collect();
    let t0 = std::time::Instant::now();
    let mut hs = Vec::new();
    for w in workers.clone() {
        let reqs = reqs.clone();
        hs.push(std::thread::spawn(move || -> Result<(usize, usize, usize, f64), String> {
            let mut m = Master::connect(&w)?;
            m.send(&Frame::SetPolicy { policy_id: 1, obs_version, tmw: Vec::new() })?;
            match m.recv()? {
                Frame::PolicyAck { ok: true, .. } => {}
                f => return Err(format!("policy refused: {f:?}")),
            }
            // warm the map on every env first (one tiny batch), then the timed batch
            let warm: Vec<EpisodeReq> = reqs.iter().take(64).cloned().map(|mut r| { r.max_steps = 1; r }).collect();
            let _ = m.run(0, &warm)?;
            let t = std::time::Instant::now();
            let out = m.run(1, &reqs)?;
            let secs = t.elapsed().as_secs_f64();
            let mut steps = 0usize;
            let mut bytes = 0usize;
            let mut errs = 0usize;
            // per-map progress: best_s max, mean length_m, finishes (the tiny campaign's zero-shot question)
            let mut per: std::collections::BTreeMap<String, (usize, f32, f64, usize, String)> = std::collections::BTreeMap::new();
            for f in &out {
                match f {
                    Frame::Episode(e) => {
                        steps += e.steps.len();
                        bytes += f.encode().len();
                        let p = per.entry(e.map_uid.clone()).or_insert((0, 0.0, 0.0, 0, String::new()));
                        p.0 += 1;
                        p.1 = p.1.max(e.best_s);
                        p.2 += e.length_m as f64;
                        // best_s and length_m are METRES along the route; a finish is best_s within a metre of the length
                        if e.length_m > 0.0 && e.best_s >= e.length_m - 1.0 {
                            p.3 += 1;
                        }
                        let last = e.steps.last().map(|s| format!("{}{}", if s.terminal { "T" } else { "" }, if s.truncated { "t" } else { "" })).unwrap_or_default();
                        p.4 = last;
                    }
                    Frame::EpisodeError { err, .. } => {
                        errs += 1;
                        eprintln!("  episode error: {}", err.chars().take(160).collect::<String>());
                    }
                    _ => errs += 1,
                }
            }
            for (uid, (n, best, len, fin, _)) in &per {
                let l = len / *n as f64;
                println!("  map {uid}: {n} episodes, best progress {:.1} m of {:.0} m ({:.1} %), {fin} finished", best, l, if l > 0.0 { 100.0 * *best as f64 / l } else { 0.0 });
            }
            let _ = m.send(&Frame::Quit);
            Ok((steps, bytes, errs, secs))
        }));
    }
    let mut tot_steps = 0usize;
    let mut tot_bytes = 0usize;
    let mut tot_errs = 0usize;
    let mut max_secs = 0f64;
    for (h, w) in hs.into_iter().zip(workers.iter()) {
        let (steps, bytes, errs, secs) = h.join().unwrap().unwrap_or_else(|e| die(e));
        println!("  {w}: {n} episodes, {steps} env-steps in {secs:.1} s = {:.0} env-steps/s, {:.1} episodes/s, {errs} errors, {:.1} MB", steps as f64 / secs, n as f64 / secs, bytes as f64 / 1e6);
        tot_steps += steps;
        tot_bytes += bytes;
        tot_errs += errs;
        max_secs = max_secs.max(secs);
    }
    println!(
        "TOTAL {} box(es): {} env-steps in {max_secs:.1} s = {:.0} env-steps/s ({:.0} game-ticks/s), {:.0} episodes/s, {tot_errs} errors, {:.1} MB/s to the master  (wall {:.1} s)",
        workers.len(),
        tot_steps,
        tot_steps as f64 / max_secs,
        tot_steps as f64 * k as f64 / max_secs,
        (n * workers.len()) as f64 / max_secs,
        tot_bytes as f64 / 1e6 / max_secs,
        t0.elapsed().as_secs_f64()
    );
}

/// `tmroll status host:port[,host:port...]`: Hello → Ready per worker (a health check for launchers).
fn cmd_status(a: &[String]) {
    let Some(list) = a.get(1) else { die("tmroll status host:port[,...]".into()) };
    let mut bad = 0;
    for w in list.split(',') {
        match std::net::TcpStream::connect_timeout(&w.to_socket_addrs_first(), std::time::Duration::from_secs(5)) {
            Ok(s) => {
                let mut s = s;
                let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                if write_frame(&mut s, &Frame::Hello { proto_version: PROTO_VERSION, master_id: "tmroll-status".into() }).is_err() {
                    println!("{w}: no hello");
                    bad += 1;
                    continue;
                }
                match read_frame(&mut s) {
                    Ok(Some(Frame::Ready { worker_id, n_workers, git_head, state_version, .. })) => {
                        println!("{w}: READY  {worker_id}  {n_workers} envs  head {git_head}  state v{state_version}")
                    }
                    other => {
                        println!("{w}: not ready ({other:?})");
                        bad += 1;
                    }
                }
                let _ = write_frame(&mut s, &Frame::Quit);
            }
            Err(e) => {
                println!("{w}: DOWN ({e})");
                bad += 1;
            }
        }
    }
    if bad > 0 {
        std::process::exit(1);
    }
}

trait FirstAddr {
    fn to_socket_addrs_first(&self) -> std::net::SocketAddr;
}
impl FirstAddr for str {
    fn to_socket_addrs_first(&self) -> std::net::SocketAddr {
        use std::net::ToSocketAddrs;
        self.to_socket_addrs().ok().and_then(|mut i| i.next()).unwrap_or_else(|| die(format!("{self}: cannot resolve")))
    }
}
