//! `pkz2 climb` — one round of a climb search, scored on the height the car
//! actually reaches on the UNMODIFIED map.
//!
//! # Why this is not scored on a rung
//!
//! The depth ladder every arm on this map has used moves eight checkpoint
//! platforms into a 32 m cell and asks the plain oracle whether they fired.
//! Two things measured here say that cannot grade a climb:
//!
//! * **A rung is a LEDGE.** With the platforms in (27,24,14) — floor y = 128 —
//!   nineteen switchback candidates read `cps=11`, every checkpoint on the map
//!   including the real CP3. With the platforms at CP3's own cell instead and
//!   nothing added on the climb, the same nineteen read `cps=2`. The rung was
//!   not detecting the climb, it was providing it.
//! * **A rung is not cell occupancy.** The best candidate's own trajectory
//!   apexes at (896.48, 130.44, 469.81), inside cell (28,24,14) — and that
//!   rung, proved live by the spawn-inside control, does not fire. A relocated
//!   platform sits on its cell's floor, and on a ramp the terrain is above it.
//!
//! So the objective here is the apex height off the candidate's own
//! trajectory, read from `fk trace` on the untouched map. It costs ~2 minutes
//! a candidate against 0.44 s for a rung, and it is the quantity in question.
//!
//! # The identity control is not optional
//!
//! Round one traces the base itself. If the base does not come back at its own
//! known apex the harness is measuring something else, and every score in the
//! round is void — the previous arm lost a round to two climb processes
//! sharing an output directory and only caught it because the incumbent scored
//! 113.71 instead of 126.84.

use crate::mkcand::Spec;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Cfg {
    pub base: String,
    pub map: String,
    pub outdir: String,
    pub at: String,
    pub par: usize,
    pub from: f64,
    pub to: f64,
    /// Score by closest approach to this point instead of by apex height.
    ///
    /// Apex height cannot prefer the side of the ramp that leads somewhere.
    /// The driver leaves this ramp at (916.41, 137.17, 465.13), on its high-z
    /// side, and his own failed attempt dies 1.5 m higher than our best on the
    /// LOW-z side -- so a candidate can score better on height while going the
    /// way that is known to dead-end.
    pub target: Option<(f64, f64, f64)>,
}

fn trace_one(cfg: &Cfg, ghost: &str, out: &str) -> Result<(), String> {
    let st = Command::new("fk")
        .args(["trace", "--tape", ghost, "--map", &cfg.map, "--at", &cfg.at, "--out", out])
        .output()
        .map_err(|e| format!("fk: {}", e))?;
    if !std::path::Path::new(out).exists() {
        return Err(String::from_utf8_lossy(&st.stderr).lines().last().unwrap_or("no output").to_string());
    }
    Ok(())
}

pub fn run(cfg: &Cfg, specs: Vec<Spec>) {
    std::fs::create_dir_all(&cfg.outdir).unwrap();
    // The identity carries no edit, so it is written by hand rather than
    // through the no-op guard that would (correctly) refuse it.
    let ident = format!("{}/IDENTITY.Ghost.Gbx", cfg.outdir);
    std::fs::copy(&cfg.base, &ident).unwrap();
    let real: Vec<Spec> = specs;
    match crate::mkcand::run(&cfg.base, &cfg.outdir, &real) {
        Ok(n) => eprintln!("wrote {} of {} candidates", n, real.len()),
        Err(e) => { eprintln!("{}", e); return }
    }
    let mut jobs: Vec<String> = vec!["IDENTITY".into()];
    jobs.extend(real.iter().map(|s| s.name.clone()));
    jobs.retain(|n| std::path::Path::new(&format!("{}/{}.Ghost.Gbx", cfg.outdir, n)).exists());

    let next = AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..cfg.par.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= jobs.len() {
                    break;
                }
                let name = &jobs[i];
                let g = format!("{}/{}.Ghost.Gbx", cfg.outdir, name);
                let o = format!("{}/{}.csv", cfg.outdir, name);
                let line = match trace_one(cfg, &g, &o) {
                    Ok(()) => match crate::csv::read(&o, cfg.from, cfg.to) {
                        Ok(rows) => {
                            let mut top = rows[0];
                            for r in &rows {
                                if r.y > top.y { top = *r; }
                            }
                            let maxx = rows.iter().fold(f64::NEG_INFINITY, |m, r| m.max(r.x));
                            let d = match cfg.target {
                                Some((tx, ty, tz)) => rows.iter().fold(f64::INFINITY, |m, r| {
                                    m.min(((r.x - tx).powi(2) + (r.y - ty).powi(2) + (r.z - tz).powi(2)).sqrt())
                                }),
                                None => f64::NAN,
                            };
                            // Sorting is textual, so the leading column is the key and a
                            // distance is negated to keep "bigger is better" true of both.
                            // The sort is textual, so the key must be a POSITIVE number of
                            // fixed width: a negated distance sorts "-33.71" above "-11.79".
                            let key = if cfg.target.is_some() { 1000.0 - d } else { top.y };
                            format!("{:>9.2}\t{:>8.2}\t{:>9.3}\t{:>8.2}\t{:>8.2}\t{:>7.1}\t{:>8.2}", key, top.y, top.t, top.x, top.z, top.v, maxx)
                        }
                        Err(e) => format!("     ---\tread failed: {}", e),
                    },
                    Err(e) => format!("     ---\ttrace failed: {}", e),
                };
                results.lock().unwrap().push((name.clone(), line));
            });
        }
    });
    let mut r = results.into_inner().unwrap();
    r.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    println!("{:>9}\t{:>8}\t{:>9}\t{:>8}\t{:>8}\t{:>7}\t{:>8}\t{}", if cfg.target.is_some() { "-dist" } else { "key" }, "apex_y", "at_s", "x", "z", "kmh", "max_x", "candidate");
    for (n, l) in &r {
        println!("{}\t{}", l, n);
    }
    if let Some((_, l)) = r.iter().find(|(n, _)| n == "IDENTITY") {
        println!("# IDENTITY control: {}", l.trim());
    } else {
        println!("# IDENTITY control MISSING -- this round is void");
    }
}
