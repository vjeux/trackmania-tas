//! `uwlab blitz` — a blind tape search scored by the ORACLE, not by a trace.
//!
//! `fk trace` costs a per-tick state readout and answers "where did the car
//! go"; when the question is only "did anything finish", the plain oracle
//! validates a whole batch of ghosts against one map in one server and is an
//! order of magnitude cheaper. That is the right instrument for the one door
//! this map has left: a rare collision event somewhere under the finish gates,
//! which needs runs by the thousand rather than trajectories by the hundred.
//!
//! Every batch carries a POSITIVE CONTROL: a tape and spawn known to finish,
//! run through the same instrument on the same day. A blind search that finds
//! nothing is worth exactly as much as its control.

use std::sync::atomic::{AtomicUsize, Ordering};

fn flag<'a>(a: &'a [String], n: &str) -> Option<&'a str> {
    a.iter().position(|s| s == n).and_then(|i| a.get(i + 1)).map(|s| s.as_str())
}
fn flags<'a>(a: &'a [String], n: &str) -> Vec<&'a str> {
    let mut v = Vec::new();
    for (i, s) in a.iter().enumerate() {
        if s == n {
            if let Some(x) = a.get(i + 1) {
                v.push(x.as_str());
            }
        }
    }
    v
}

pub struct Lcg(u64);
impl Lcg {
    pub fn new(seed: u64) -> Lcg {
        Lcg(seed.wrapping_mul(2862933555777941757).wrapping_add(3037000493))
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 11
    }
    fn upto(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// One random tape as a `--seg` list. Segments are short enough that the car
/// keeps changing what it is doing — a constant steer just makes it circle,
/// which is how a previous arm's straight-line sweep managed never to ram
/// anything.
pub fn random_segs(r: &mut Lcg, ticks: usize) -> Vec<String> {
    let steers = [-127i32, -96, -64, -32, -8, 1, 8, 32, 64, 96, 127];
    let mut out = Vec::new();
    let mut t = 160usize;
    while t < ticks {
        let len = 20 + r.upto(280) as usize;
        let s = steers[r.upto(steers.len() as u64) as usize];
        let g = if r.upto(10) < 8 { 1 } else { 0 };
        let b = if r.upto(10) < 2 { 1 } else { 0 };
        let e = (t + len).min(ticks);
        out.push(format!("{t}:{e}:{s}:{g}:{b}"));
        t = e;
    }
    out
}

pub fn cmd_blitz(a: &[String]) -> i32 {
    let need = |n: &str| -> String {
        match flag(a, n) {
            Some(v) => v.to_string(),
            None => {
                eprintln!("uwlab blitz: {n} is required");
                std::process::exit(2);
            }
        }
    };
    let base = need("--map");
    let carrier = need("--carrier");
    let template = need("--template");
    let outdir = flag(a, "--dir").unwrap_or("blitz").to_string();
    let tmmaps = flag(a, "--tmmaps").unwrap_or("tmmaps").to_string();
    let ghost = flag(a, "--ghost").unwrap_or("ghost").to_string();
    let block = flag(a, "--block").unwrap_or("4633").to_string();
    let ticks: usize = flag(a, "--ticks").and_then(|s| s.parse().ok()).unwrap_or(4000);
    let ntapes: usize = flag(a, "--tapes").and_then(|s| s.parse().ok()).unwrap_or(200);
    let seed: u64 = flag(a, "--seed").and_then(|s| s.parse().ok()).unwrap_or(1);
    let jobs: usize = flag(a, "--jobs").and_then(|s| s.parse().ok()).unwrap_or(32);
    let me = std::env::current_exe().unwrap_or_else(|_| "uwlab".into());

    let extra: Vec<String> = flags(a, "--extra").iter().map(|s| s.to_string()).collect();
    let mut spawns: Vec<(i64, i64, i64, i64)> = Vec::new();
    for s in flags(a, "--spawns") {
        let p: Vec<&str> = s.split(',').collect();
        if p.len() != 4 {
            continue;
        }
        let rg = |v: &str| -> Vec<i64> {
            match v.split_once(':') {
                Some((x, y)) => (x.parse().unwrap_or(0)..=y.parse().unwrap_or(0)).collect(),
                None => vec![v.parse().unwrap_or(0)],
            }
        };
        for cx in rg(p[0]) {
            for cy in rg(p[1]) {
                for cz in rg(p[2]) {
                    for d in rg(p[3]) {
                        spawns.push((cx, cy, cz, d));
                    }
                }
            }
        }
    }
    if spawns.is_empty() {
        eprintln!("uwlab blitz: need --spawns cx,cy,cz,dir");
        return 2;
    }
    let _ = std::fs::create_dir_all(format!("{outdir}/maps"));
    let _ = std::fs::create_dir_all(format!("{outdir}/tapes"));

    // ---- tapes
    let mut r = Lcg(seed.wrapping_mul(2862933555777941757).wrapping_add(3037000493));
    let plans: Vec<(String, Vec<String>)> = (0..ntapes)
        .map(|i| (format!("r{i:04}"), random_segs(&mut r, ticks)))
        .collect();
    eprintln!("blitz: {} spawns x {} tapes = {} runs", spawns.len(), plans.len(), spawns.len() * plans.len());
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some((name, segs)) = plans.get(i) else { return };
                let gt = format!("{outdir}/tapes/{name}.gtape");
                let gh = format!("{outdir}/tapes/{name}.Ghost.Gbx");
                if std::path::Path::new(&gh).exists() {
                    continue;
                }
                let mut args: Vec<String> = vec![
                    "tape".into(), "--from".into(), template.clone(),
                    "--out".into(), gt.clone(), "--ticks".into(), ticks.to_string(),
                ];
                for sg in segs {
                    args.push("--seg".into());
                    args.push(sg.clone());
                }
                if std::process::Command::new(&me).args(&args).output().map(|o| !o.status.success()).unwrap_or(true) {
                    eprintln!("blitz: tape {name} FAILED");
                    return;
                }
                let _ = std::process::Command::new(&ghost)
                    .args(["tape", "inject", &carrier, &gh, "--tape", &gt])
                    .output();
            });
        }
    });

    // ---- maps
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..jobs.min(spawns.len()) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(&(cx, cy, cz, d)) = spawns.get(i) else { return };
                let out = format!("{outdir}/maps/s{cx}_{cy}_{cz}_d{d}.Map.Gbx");
                if std::path::Path::new(&out).exists() {
                    continue;
                }
                let mut margs: Vec<String> = vec!["move".into(), base.clone(), "--out".into(), out.clone(), "--move".into(), format!("{block}:{cx},{cy},{cz}:{d}")];
                for e in &extra { margs.push("--move".into()); margs.push(e.clone()); }
                let o = std::process::Command::new(&tmmaps).args(&margs).output();
                if o.map(|o| !o.status.success()).unwrap_or(true) {
                    eprintln!("blitz: map s{cx}_{cy}_{cz}_d{d} FAILED");
                }
            });
        }
    });

    // ---- one oracle batch per map
    let mut tapefiles: Vec<String> = plans
        .iter()
        .map(|(n, _)| format!("{outdir}/tapes/{n}.Ghost.Gbx"))
        .collect();
    for extra in flags(a, "--also-tape") {
        tapefiles.push(extra.to_string());
    }
    let shard: usize = flag(a, "--shard").and_then(|s| s.parse().ok()).unwrap_or(jobs);
    for &(cx, cy, cz, d) in &spawns {
        let map = format!("{outdir}/maps/s{cx}_{cy}_{cz}_d{d}.Map.Gbx");
        if !std::path::Path::new(&map).exists() {
            continue;
        }
        let mut args: Vec<String> = vec!["oracle".into(), "--map".into(), map.clone(), "--ghosts".into()];
        args.extend(tapefiles.iter().cloned());
        args.push("--shard".into());
        args.push("-j".into());
        args.push(shard.to_string());
        let o = std::process::Command::new(&tmmaps).args(&args).output();
        match o {
            Ok(o) => {
                let txt = String::from_utf8_lossy(&o.stdout);
                let mut fin = 0;
                for line in txt.lines() {
                    if line.contains("DNF") {
                        continue;
                    }
                    // any non-DNF verdict line mentioning a ghost is a finish
                    if line.contains(".Ghost.Gbx") {
                        fin += 1;
                        println!("FINISH\ts{cx}_{cy}_{cz}_d{d}\t{}", line.trim());
                    }
                }
                eprintln!("blitz: s{cx}_{cy}_{cz}_d{d}  {fin} finishes / {} tapes", tapefiles.len());
                if !o.status.success() && txt.is_empty() {
                    eprintln!("  (oracle stderr) {}", String::from_utf8_lossy(&o.stderr).lines().take(3).collect::<Vec<_>>().join(" | "));
                }
            }
            Err(e) => eprintln!("blitz: oracle failed on s{cx}_{cy}_{cz}_d{d}: {e}"),
        }
    }
    0
}
