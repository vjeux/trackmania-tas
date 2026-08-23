//! `uwlab sweep` — a DIRECTED launch sweep: spawn × tape, traced and scored.
//!
//! The previous arm's nulls on this map were all measured with BLIND tape
//! families (random inputs) or with constant-steer straight lines from a
//! handful of spawns. Both answer "does something happen by accident", which
//! is not the question. This runs a cross product of
//!
//!   * a spawn — the start block moved to a cell, WITH A DIRECTION, so the car
//!     can be pointed at the thing we want it to reach; and
//!   * a tape — a short list of (t0,t1,steer,accel,brake) segments,
//!
//! traces every one with the live engine and scores it against a target box
//! per axis. Scoring is continuous and per-axis on purpose: a miss distance
//! that is not resolved per axis cannot tell a wall from a coverage failure.
//!
//! Nothing here is a RESULT — a trace is a measurement. A finish is only a
//! finish when the plain oracle re-simulates the written ghost.

use crate::traj::Traj;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Bx {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub x1: f64,
    pub y1: f64,
    pub z1: f64,
}

impl Bx {
    pub fn parse(s: &str) -> Option<Bx> {
        let (a, b) = s.split_once(':')?;
        let p: Vec<f64> = a
            .split(',')
            .chain(b.split(','))
            .map(|v| v.parse().unwrap_or(f64::NAN))
            .collect();
        if p.len() != 6 || p.iter().any(|v| v.is_nan()) {
            return None;
        }
        Some(Bx {
            x0: p[0].min(p[3]),
            y0: p[1].min(p[4]),
            z0: p[2].min(p[5]),
            x1: p[0].max(p[3]),
            y1: p[1].max(p[4]),
            z1: p[2].max(p[5]),
        })
    }
    pub fn miss(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64, f64) {
        let dx = (self.x0 - x).max(x - self.x1).max(0.0);
        let dy = (self.y0 - y).max(y - self.y1).max(0.0);
        let dz = (self.z0 - z).max(z - self.z1).max(0.0);
        ((dx * dx + dy * dy + dz * dz).sqrt(), dx, dy, dz)
    }
}

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

/// `a:b` or `a` → inclusive integer range.
fn irange(s: &str) -> Vec<i64> {
    match s.split_once(':') {
        Some((a, b)) => {
            let (a, b) = (a.parse().unwrap_or(0), b.parse().unwrap_or(0));
            (a..=b).collect()
        }
        None => vec![s.parse().unwrap_or(0)],
    }
}

#[derive(Clone)]
pub struct Spawn {
    pub cx: i64,
    pub cy: i64,
    pub cz: i64,
    pub dir: i64,
}
impl Spawn {
    pub fn tag(&self) -> String {
        format!("s{}_{}_{}_d{}", self.cx, self.cy, self.cz, self.dir)
    }
}

/// `cx[:cx1],cy[:cy1],cz[:cz1],dir[:dir1]`
fn parse_spawns(s: &str) -> Vec<Spawn> {
    let p: Vec<&str> = s.split(',').collect();
    if p.len() != 4 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for cx in irange(p[0]) {
        for cy in irange(p[1]) {
            for cz in irange(p[2]) {
                for dir in irange(p[3]) {
                    out.push(Spawn { cx, cy, cz, dir });
                }
            }
        }
    }
    out
}

pub fn cmd_sweep(a: &[String]) -> i32 {
    let need = |n: &str| -> String {
        match flag(a, n) {
            Some(v) => v.to_string(),
            None => {
                eprintln!("uwlab sweep: {n} is required");
                std::process::exit(2);
            }
        }
    };
    let base = need("--map");
    let carrier = need("--carrier");
    let template = need("--template");
    let outdir = flag(a, "--dir").unwrap_or("sweep").to_string();
    let tmmaps = flag(a, "--tmmaps").unwrap_or("tmmaps").to_string();
    let fk = flag(a, "--fk").unwrap_or("fk").to_string();
    let ghost = flag(a, "--ghost").unwrap_or("ghost").to_string();
    let block = flag(a, "--block").unwrap_or("4633").to_string();
    let at = flag(a, "--at").unwrap_or("tick:160").to_string();
    let ticks: usize = flag(a, "--ticks").and_then(|s| s.parse().ok()).unwrap_or(4000);
    let jobs: usize = flag(a, "--jobs").and_then(|s| s.parse().ok()).unwrap_or(32);
    let keep = a.iter().any(|s| s == "--keep-csv");
    let bx = flag(a, "--box").and_then(Bx::parse);
    let crossspec: Option<String> = flag(a, "--cross").map(|s| s.to_string());
    let extra: Vec<String> = flags(a, "--extra").iter().map(|s| s.to_string()).collect();
    let me = std::env::current_exe().unwrap_or_else(|_| "uwlab".into());

    let mut spawns: Vec<Spawn> = Vec::new();
    for s in flags(a, "--spawns") {
        spawns.extend(parse_spawns(s));
    }
    // --tape NAME=t0:t1:steer:accel:brake/...
    let mut tapes: Vec<(String, Vec<String>)> = Vec::new();
    for t in flags(a, "--tape") {
        let (name, spec) = t.split_once('=').unwrap_or(("t", t));
        tapes.push((name.to_string(), spec.split('/').map(|s| s.to_string()).collect()));
    }
    // --random N: N blind tapes, which is the right family when the question
    // is "does anything ever happen here" rather than "does this input work".
    if let Some(n) = flag(a, "--random") {
        let n: usize = n.parse().unwrap_or(0);
        let seed: u64 = flag(a, "--seed").and_then(|s| s.parse().ok()).unwrap_or(1);
        let mut r = crate::blitz::Lcg::new(seed);
        for i in 0..n {
            tapes.push((format!("r{i:04}"), crate::blitz::random_segs(&mut r, ticks)));
        }
    }
    // --plan FILE: one job family per line, "name<TAB>seg/seg/seg"
    if let Some(p) = flag(a, "--plan") {
        let text = std::fs::read_to_string(p).unwrap_or_default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (name, spec) = match line.split_once('\t') {
                Some(v) => v,
                None => continue,
            };
            tapes.push((name.to_string(), spec.split('/').map(|s| s.to_string()).collect()));
        }
    }
    if spawns.is_empty() || tapes.is_empty() {
        eprintln!("uwlab sweep: need at least one --spawns and one --tape/--plan");
        return 2;
    }
    let _ = std::fs::create_dir_all(format!("{outdir}/maps"));
    let _ = std::fs::create_dir_all(format!("{outdir}/tapes"));
    let _ = std::fs::create_dir_all(format!("{outdir}/csv"));

    // ---- build every map once
    eprintln!("sweep: {} spawns x {} tapes = {} runs", spawns.len(), tapes.len(), spawns.len() * tapes.len());
    let bad_map = std::sync::Mutex::new(Vec::<String>::new());
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..jobs.min(spawns.len().max(1)) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(sp) = spawns.get(i) else { return };
                let out = format!("{outdir}/maps/{}.Map.Gbx", sp.tag());
                if std::path::Path::new(&out).exists() {
                    continue;
                }
                let mut margs: Vec<String> = vec!["move".into(), base.clone(), "--out".into(), out.clone(), "--move".into(), format!("{}:{},{},{}:{}", block, sp.cx, sp.cy, sp.cz, sp.dir)];
                // --extra puts other blocks somewhere too: a spare roof slab
                // moved next to the deck is how the maximum climbable STEP
                // gets measured instead of assumed.
                for e in &extra { margs.push("--move".into()); margs.push(e.clone()); }
                let o = std::process::Command::new(&tmmaps).args(&margs).output();
                if o.map(|o| !o.status.success()).unwrap_or(true) {
                    bad_map.lock().unwrap().push(sp.tag());
                }
            });
        }
    });
    for b in bad_map.into_inner().unwrap() {
        eprintln!("sweep: MAP BUILD FAILED {b}");
    }

    // ---- build every tape once
    let next = AtomicUsize::new(0);
    let bad_tape = std::sync::Mutex::new(Vec::<String>::new());
    std::thread::scope(|s| {
        for _ in 0..jobs.min(tapes.len().max(1)) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some((name, segs)) = tapes.get(i) else { return };
                let gt = format!("{outdir}/tapes/{name}.gtape");
                let gh = format!("{outdir}/tapes/{name}.Ghost.Gbx");
                if std::path::Path::new(&gh).exists() {
                    continue;
                }
                let mut args: Vec<String> = vec![
                    "tape".into(),
                    "--from".into(),
                    template.clone(),
                    "--out".into(),
                    gt.clone(),
                    "--ticks".into(),
                    ticks.to_string(),
                ];
                for sg in segs {
                    args.push("--seg".into());
                    args.push(sg.clone());
                }
                let o = std::process::Command::new(&me).args(&args).output();
                if o.map(|o| !o.status.success()).unwrap_or(true) {
                    bad_tape.lock().unwrap().push(name.clone());
                    return;
                }
                let o = std::process::Command::new(&ghost)
                    .args(["tape", "inject", &carrier, &gh, "--tape", &gt])
                    .output();
                if o.map(|o| !o.status.success()).unwrap_or(true) {
                    bad_tape.lock().unwrap().push(name.clone());
                }
            });
        }
    });
    for b in bad_tape.into_inner().unwrap() {
        eprintln!("sweep: TAPE BUILD FAILED {b}");
    }

    // ---- the cross product
    let jobs_v: Vec<(usize, usize)> = (0..spawns.len())
        .flat_map(|i| (0..tapes.len()).map(move |j| (i, j)))
        .collect();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::<String>::new());
    let total = jobs_v.len();
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(&(si, ti)) = jobs_v.get(i) else { return };
                let sp = &spawns[si];
                let tname = &tapes[ti].0;
                let tag = format!("{}__{}", sp.tag(), tname);
                let map = format!("{outdir}/maps/{}.Map.Gbx", sp.tag());
                let tape = format!("{outdir}/tapes/{tname}.Ghost.Gbx");
                let csv = format!("{outdir}/csv/{tag}.csv");
                let work = format!("/tmp/uwsweep-{}", tag);
                let _ = std::fs::remove_dir_all(&work);
                let o = std::process::Command::new(&fk)
                    .args(["trace", "--tape", &tape, "--map", &map, "--work", &work, "--at", &at, "--out", &csv])
                    .env("FK_VERR_MAX", "3.0")
                    .output();
                let _ = std::fs::remove_dir_all(&work);
                let d = done.fetch_add(1, Ordering::SeqCst) + 1;
                if d % 200 == 0 {
                    eprintln!("  .. {d}/{total}");
                }
                // `fk trace` exits non-zero when its own self-check refuses to
                // CERTIFY a trajectory — most often "the car never moves",
                // which is a perfectly good measurement of a spawn that is
                // stuck. The CSV is written either way. Reading it back is how
                // the previous arm's "62 of 158 traces died in the car
                // locator" turns into 158 scored runs; the status column says
                // which ones the engine's own check would not sign.
                let (ok, why) = match &o {
                    Ok(o) if o.status.success() => (true, String::new()),
                    Ok(o) => {
                        let e = String::from_utf8_lossy(&o.stderr);
                        let w = e
                            .lines()
                            .find(|l| l.contains("SELF-CHECK") || l.contains("ABORT"))
                            .unwrap_or("failed")
                            .trim()
                            .to_string();
                        (false, w)
                    }
                    Err(e) => (false, e.to_string()),
                };
                let loaded = Traj::load(&csv).ok().filter(|t| t.rows.len() > 2);
                let line = match loaded {
                    Some(t) => {
                        let st = if ok { "ok" } else { "unsigned" };
                        let cr = match &crossspec {
                            Some(c) => cross(&t, c),
                            None => "\t\t\t".into(),
                        };
                        format!("{st}\t{}\t{cr}\t{}", score(&t, bx.as_ref()), contact(&t))
                    }
                    None => format!("TRACE_FAILED\t{why}"),
                };
                if !keep {
                    let _ = std::fs::remove_file(&csv);
                }
                out.lock().unwrap().push(format!("{tag}\t{line}"));
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort();
    let mut w = std::io::BufWriter::new(std::io::stdout());
    let _ = writeln!(
        w,
        "run\tstatus\ttend\txend\tyend\tzend\tvend\tmaxy\tmaxy_x\tmaxy_z\tmaxy_t\tmiss\tdx\tdy\tdz\tmiss_t\tmiss_x\tmiss_y\tmiss_z\tcr_t\tcr_y\tcr_x\tcr_z\tcy\tcx\tcz\tct"
    );
    for l in v {
        let _ = writeln!(w, "{l}");
    }
    0
}

fn score(t: &Traj, bx: Option<&Bx>) -> String {
    let Some(last) = t.rows.last() else {
        return "EMPTY".into();
    };
    let mut my = f64::MIN;
    let mut mr = last.clone();
    for r in &t.rows {
        if r.y > my {
            my = r.y;
            mr = r.clone();
        }
    }
    let b = match bx {
        Some(b) => {
            let mut best = (f64::MAX, 0.0, 0.0, 0.0);
            let mut br = last.clone();
            for r in &t.rows {
                let m = b.miss(r.x, r.y, r.z);
                if m.0 < best.0 {
                    best = m;
                    br = r.clone();
                }
            }
            format!(
                "{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.2}\t{:.2}\t{:.2}",
                best.0, best.1, best.2, best.3, br.t, br.x, br.y, br.z
            )
        }
        None => "\t\t\t\t\t\t\t".into(),
    };
    format!(
        "{:.3}\t{:.2}\t{:.3}\t{:.2}\t{:.2}\t{:.3}\t{:.2}\t{:.2}\t{:.3}\t{}",
        last.t, last.x, last.y, last.z, last.speed_ms, my, mr.x, mr.z, mr.t, b
    )
}

/// The state at the first crossing of a plane, e.g. `x:1310` — the one number
/// that says whether a hop cleared the far wall or fell short of it.
pub fn cross(t: &Traj, spec: &str) -> String {
    let Some((ax, v)) = spec.split_once(':') else {
        return "\t\t\t".into();
    };
    let v: f64 = v.parse().unwrap_or(0.0);
    let get = |r: &crate::traj::Row| match ax {
        "x" => r.x,
        "y" => r.y,
        _ => r.z,
    };
    for w in t.rows.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if (get(a) - v) * (get(b) - v) <= 0.0 && (get(a) - get(b)).abs() > 1e-9 {
            return format!("{:.3}\t{:.3}\t{:.2}\t{:.2}", b.t, b.y, b.x, b.z);
        }
    }
    "\t\t\t".into()
}

/// Where a falling car FIRST stops falling — the plumb-probe reading, as
/// columns rather than prose. `contact_y` is the surface the column has.
pub fn contact(t: &Traj) -> String {
    // The engine trace leaves vx,vy,vz at zero whenever fk could not sign the
    // car locator, so the sink rate is read off the POSITIONS: a velocity
    // column that is silently zero would make every column report contact at
    // its first sample.
    let n = t.rows.len();
    let vy = |i: usize| -> f64 {
        if i + 1 >= n { return 0.0; }
        let (a, b) = (&t.rows[i], &t.rows[i + 1]);
        if b.t - a.t <= 0.0 { 0.0 } else { (b.y - a.y) / (b.t - a.t) }
    };
    let mut i = 0;
    while i < n {
        let r = &t.rows[i];
        if r.t < 1.0 || vy(i) < -2.0 {
            i += 1;
            continue;
        }
        let t0 = r.t;
        let mut j = i;
        let mut ok = true;
        while j < n && t.rows[j].t - t0 < 1.0 {
            if vy(j) < -2.0 {
                ok = false;
                break;
            }
            j += 1;
        }
        if ok && j < n {
            return format!("{:.3}\t{:.2}\t{:.2}\t{:.3}", r.y, r.x, r.z, r.t);
        }
        i = j.max(i + 1);
    }
    "\t\t\t".into()
}
