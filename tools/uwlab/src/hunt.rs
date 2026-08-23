//! `uwlab hunt` — a hill climber whose evaluator is `fk trace`, for a map whose
//! fork server cannot locate the car.
//!
//! ## Why this exists
//!
//! The proper way to hunt a state is `tmsearch --fork --gate`: thousands of
//! candidates a minute on mid-simulation fork servers. **On 173691 it does not
//! start.** Measured here, six distinct fork checkpoints — tape ticks 22500,
//! 23000, 23400, 23700, 24000, 24300, 24650, 25000, whose calibrated boundaries
//! span race 216.1 to 241 — and every worker aborts identically:
//!
//! ```text
//! worker N: ABORT, the car's state was not located:
//!           no self-consistent vehicle state found: state not located
//! ```
//!
//! 40 of 40 workers, then 3 of 3, at every checkpoint. **A batch that fails
//! UNIFORMLY is the harness, not the physics** — and the same binary's
//! in-process locate (`fk trace`) finds the car on this map at these very ticks
//! and writes a self-checked trajectory. So the car is findable; the fork's
//! blind locate is what cannot find it here.
//!
//! That leaves `fk trace` as the only evaluator on this map: ~90 s a candidate,
//! because each one boots a server and simulates the whole 240 s approach.
//! Forty in parallel is ~26 candidates a minute, which is a real search — as
//! long as the driver around it is honest. This is that driver.
//!
//! ## What it does differently from the shell loop it replaces
//!
//! Three things, each of which cost this arm something:
//!
//! 1. **It accepts a round's best only if it beats the INCUMBENT.** A driver
//!    that accepts the best of the round is not a hill climber: it walks
//!    downhill whenever a round is bad, and the tell is the same number
//!    reported round after round. (Seen: four rounds, one number.)
//! 2. **It scores through the fire clause**, so a candidate that arrives at the
//!    place without doing the thing cannot outrank one that acts. The decoy
//!    this arm hit — the car wedging at (1233.0, 157.6, 284.8), the tape's own
//!    respawn returning it to the route, and a drift past the lip 40 s later at
//!    26 m/s — scored three rounds of "progress" under a plain nearest-approach
//!    key. Under the clause it does not fire at all.
//! 3. **It reports a uniform failure as a failure.** If no candidate in a round
//!    produced a trajectory, that is the harness and the run stops, rather than
//!    reporting a round of no improvement.

use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

fn flag_val(a: &[String], n: &str) -> Option<String> {
    a.iter().position(|x| x == n).and_then(|i| a.get(i + 1)).cloned()
}

/// The band a candidate landed in. Cumulative and ordered by construction:
/// there is no constant anyone can tune wrong.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Band {
    /// no trajectory at all — the harness, not the candidate
    NoRun,
    /// never entered the gate box
    Missed(f64),
    /// entered it; the key is how good the state was
    Reached(f64),
    /// and did the thing the fire clause names
    Fired(f64),
}

impl Band {
    fn rank(&self) -> (u8, f64) {
        match self {
            Band::NoRun => (0, f64::NEG_INFINITY),
            Band::Missed(m) => (1, -m),
            Band::Reached(k) => (2, *k),
            Band::Fired(k) => (3, *k),
        }
    }
    fn better_than(&self, o: &Band) -> bool {
        let (a, b) = (self.rank(), o.rank());
        a.0 > b.0 || (a.0 == b.0 && a.1 > b.1)
    }
    fn show(&self) -> String {
        match self {
            Band::NoRun => "NO-RUN".into(),
            Band::Missed(m) => format!("missed  {:.3} m", m),
            Band::Reached(k) => format!("reached key {:+.4}", k),
            Band::Fired(k) => format!("FIRED   key {:+.4}", k),
        }
    }
}

/// Read `fk watch replay`'s two lines back into a band.
fn band_of(out: &str) -> Band {
    let fired = out.lines().any(|l| l.trim_start().starts_with("fire: at tick"));
    let mut key: Option<f64> = None;
    let mut miss: Option<f64> = None;
    for l in out.lines() {
        let t = l.trim_start();
        if let Some(r) = t.strip_prefix("gate: key ") {
            key = r.split_whitespace().next().and_then(|v| v.parse().ok());
        }
        if t.starts_with("gate: never") || t.contains("never entered") {
            miss = Some(f64::INFINITY);
        }
        if let Some(r) = t.strip_prefix("gate: miss ") {
            miss = r.split_whitespace().next().and_then(|v| v.parse().ok());
        }
    }
    match (fired, key, miss) {
        (true, Some(k), _) => Band::Fired(k),
        (false, Some(k), _) => Band::Reached(k),
        (_, None, Some(m)) => Band::Missed(m),
        _ => Band::NoRun,
    }
}

const USAGE: &str = "\
usage: uwlab hunt --template G.Ghost.Gbx --map M.Map.Gbx --base T.gtape
                  --gate SPEC --gate-key EXPR [--fire F --fire-at V --fire-where SPEC]
                  --lo TICK --hi TICK [--width N] [--steers a,b,c] [--rounds N]
                  [--jobs N] [--dir D] [--ghost P] [--fk P]

A hill climber whose evaluator is `fk trace`. Each candidate overwrites the
steer over one window of the incumbent, simulates, and is scored by the band
its trajectory lands in: missed < reached < fired.

A round's best is accepted ONLY if it beats the incumbent. A round in which
nothing produced a trajectory stops the run: that is the harness.
";

pub fn cmd_hunt(a: &[String]) -> i32 {
    let need = |n: &str| -> String {
        flag_val(a, n).unwrap_or_else(|| {
            eprint!("uwlab hunt: {n} is required\n\n{USAGE}");
            std::process::exit(2);
        })
    };
    let template = need("--template");
    let map = need("--map");
    let base0 = need("--base");
    let gate = need("--gate");
    let gate_key = need("--gate-key");
    let fire = flag_val(a, "--fire");
    let fire_at = flag_val(a, "--fire-at");
    let fire_where = flag_val(a, "--fire-where");
    let lo: i64 = need("--lo").parse().expect("--lo");
    let hi: i64 = need("--hi").parse().expect("--hi");
    let width: i64 = flag_val(a, "--width").and_then(|s| s.parse().ok()).unwrap_or(120);
    let rounds: usize = flag_val(a, "--rounds").and_then(|s| s.parse().ok()).unwrap_or(8);
    let jobs: usize = flag_val(a, "--jobs").and_then(|s| s.parse().ok()).unwrap_or(32);
    let dir = flag_val(a, "--dir").unwrap_or_else(|| "hunt".into());
    let ghost = flag_val(a, "--ghost").unwrap_or_else(|| "ghost".into());
    let fk = flag_val(a, "--fk").unwrap_or_else(|| "fk".into());
    let tracetick = flag_val(a, "--tracetick").unwrap_or_else(|| "23800".into());
    let steers: Vec<i64> = flag_val(a, "--steers")
        .unwrap_or_else(|| "-90,-60,-40,-25,-12,0,12,25,40,60,90".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let _ = std::fs::create_dir_all(&dir);

    // The window starts to try, one per round position.
    let starts: Vec<i64> = (lo..hi.saturating_sub(width)).step_by(width as usize / 2).collect();
    if starts.is_empty() {
        eprintln!("uwlab hunt: --lo/--hi/--width leave no window to search");
        return 2;
    }

    let mut base = base0.clone();
    let mut incumbent = Band::NoRun;
    let mut log = std::fs::File::create(format!("{dir}/hunt.log")).ok();

    for round in 1..=rounds {
        let cands: Vec<(i64, i64)> =
            starts.iter().flat_map(|s| steers.iter().map(move |v| (*s, *v))).collect();
        let next = AtomicUsize::new(0);
        let results: Mutex<Vec<(Band, String, i64, i64)>> = Mutex::new(Vec::new());
        std::thread::scope(|sc| {
            for _ in 0..jobs {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(&(t0, sv)) = cands.get(i) else { return };
                    let tag = format!("r{round}_t{t0}_s{sv}");
                    let gt = format!("{dir}/{tag}.gtape");
                    let gg = format!("{dir}/{tag}.Ghost.Gbx");
                    let cs = format!("{dir}/{tag}.csv");
                    let ok = Command::new(&ghost)
                        .args([
                            "tape", "set", &base, "--out", &gt, "--from", &t0.to_string(), "--to",
                            &(t0 + width).to_string(), "--steer", &sv.to_string(),
                        ])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    if !ok {
                        return;
                    }
                    let ok = Command::new(&ghost)
                        .args(["tape", "inject", &template, &gg, "--tape", &gt, "--allow-telemetry-mismatch"])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    if !ok {
                        return;
                    }
                    let ok = Command::new(&fk)
                        .args([
                            "trace", "--tape", &gg, "--map", &map, "--at",
                            &format!("tick:{tracetick}"), "--work",
                            &format!("/tmp/uwhunt-{tag}"), "--out", &cs,
                        ])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    if !ok {
                        results.lock().unwrap().push((Band::NoRun, gt, t0, sv));
                        return;
                    }
                    let mut args: Vec<String> = vec![
                        "watch".into(), "replay".into(), "--template".into(), template.clone(),
                        "--map".into(), map.clone(), "--trajectory".into(), cs.clone(),
                        "--gate".into(), gate.clone(), "--gate-key".into(), gate_key.clone(),
                    ];
                    if let Some(f) = &fire {
                        args.push("--fire".into());
                        args.push(f.clone());
                        if let Some(v) = &fire_at {
                            args.push("--fire-at".into());
                            args.push(v.clone());
                        }
                        if let Some(w) = &fire_where {
                            args.push("--fire-where".into());
                            args.push(w.clone());
                        }
                    }
                    let out = Command::new(&fk).args(&args).output();
                    let text = out
                        .map(|o| String::from_utf8_lossy(&o.stdout).to_string()
                            + &String::from_utf8_lossy(&o.stderr))
                        .unwrap_or_default();
                    let _ = std::fs::remove_dir_all(format!("/tmp/uwhunt-{tag}"));
                    results.lock().unwrap().push((band_of(&text), gt, t0, sv));
                });
            }
        });
        let rs = results.into_inner().unwrap();
        let ran = rs.iter().filter(|r| r.0 != Band::NoRun).count();
        // A round in which nothing ran is the harness, not a plateau.
        if ran == 0 {
            eprintln!(
                "round {round}: NONE of {} candidates produced a trajectory. That is uniform, so \
                 it is the harness and not the physics -- check the locate at --tracetick before \
                 reading anything into it.",
                rs.len()
            );
            return 3;
        }
        let best = rs.iter().max_by(|a, b| {
            a.0.rank().partial_cmp(&b.0.rank()).unwrap_or(std::cmp::Ordering::Equal)
        });
        let fired = rs.iter().filter(|r| matches!(r.0, Band::Fired(_))).count();
        if let Some((band, path, t0, sv)) = best {
            let line = format!(
                "round {round}: {} of {} ran, {} FIRED, best {} (t{} s{})",
                ran, rs.len(), fired, band.show(), t0, sv
            );
            println!("{line}");
            if let Some(f) = log.as_mut() {
                let _ = writeln!(f, "{line}");
            }
            // ACCEPT ONLY IF IT BEATS THE INCUMBENT.
            if band.better_than(&incumbent) {
                incumbent = *band;
                base = path.clone();
                println!("  ACCEPTED -> {}", base);
            } else {
                println!("  no improvement on {} -- keeping the incumbent", incumbent.show());
            }
            for (b, p, _, _) in rs.iter().filter(|r| matches!(r.0, Band::Fired(_))) {
                println!("  FIRED: {}  {}", b.show(), p);
            }
        }
    }
    println!("FINAL {}  base {}", incumbent.show(), base);
    0
}
