//! `uwlab platladder` — how high does the finish trigger reach?
//!
//! The gate wall is three rows of `GateFinish` blocks with bases at y = 130,
//! 162 and 194, and the measured firings sit at 133.9…136 and 162…170.2 —
//! the bottom of each block — while a crossing at 156 does not fire and the
//! deck at 114.16 does not fire. That is a hypothesis about the trigger's
//! shape, not a measurement, because nothing has ever crossed the plane at a
//! CHOSEN height: every crossing so far was wherever a falling car happened
//! to be.
//!
//! This chooses the height. It moves a spare roof platform into the gate cell
//! at cell-height `cy`, moves the spawn one cell above it, and drives the car
//! across the trigger plane at that floor's height. One run per rung, plus the
//! two rungs that are already known — 130 fires, 114 does not — as the
//! controls that say whether the ladder itself works.

use crate::traj::Traj;
use std::sync::atomic::{AtomicUsize, Ordering};

fn flag<'a>(a: &'a [String], n: &str) -> Option<&'a str> {
    a.iter().position(|s| s == n).and_then(|i| a.get(i + 1)).map(|s| s.as_str())
}

pub fn cmd_platladder(a: &[String]) -> i32 {
    let need = |n: &str| -> String {
        match flag(a, n) {
            Some(v) => v.to_string(),
            None => {
                eprintln!("uwlab platladder: {n} is required");
                std::process::exit(2);
            }
        }
    };
    let base = need("--map");
    let carrier = need("--carrier");
    let template = need("--template");
    let outdir = flag(a, "--dir").unwrap_or("plat").to_string();
    let tmmaps = flag(a, "--tmmaps").unwrap_or("tmmaps").to_string();
    let fk = flag(a, "--fk").unwrap_or("fk").to_string();
    let ghostb = flag(a, "--ghost").unwrap_or("ghost").to_string();
    let plat = flag(a, "--plat-block").unwrap_or("5066").to_string();
    let spawnb = flag(a, "--block").unwrap_or("4633").to_string();
    let cxs: Vec<i64> = flag(a, "--cx").unwrap_or("44").split(',').filter_map(|s| s.parse().ok()).collect();
    let (cy0, cy1) = flag(a, "--cy")
        .and_then(|s| s.split_once(':'))
        .map(|(a, b)| (a.parse().unwrap_or(22), b.parse().unwrap_or(30)))
        .unwrap_or((22, 30));
    let jobs: usize = flag(a, "--jobs").and_then(|s| s.parse().ok()).unwrap_or(16);
    let ticks: usize = flag(a, "--ticks").and_then(|s| s.parse().ok()).unwrap_or(4000);
    let me = std::env::current_exe().unwrap_or_else(|_| "uwlab".into());
    let _ = std::fs::create_dir_all(format!("{outdir}/maps"));
    let _ = std::fs::create_dir_all(format!("{outdir}/csv"));

    // two tapes: drive forward hard, and drive forward then brake (so the
    // crossing speed differs and a tunnelling miss cannot masquerade as a
    // dead band).
    // The crossing must happen at the GATE, not at the cell seam: a car that
    // drives straight from a cell-corner spawn crosses the trigger plane at
    // x = the cell boundary, which is 16 m from either gate centre and dead.
    // So the family turns: each tape holds a steer for N ticks after landing
    // and then runs straight, which fans the crossing across x.
    let mut tapes: Vec<(String, Vec<String>)> = vec![("go".to_string(), vec![format!("160:{ticks}:1:1:0")])];
    for s in [-127i32, -96, -64, -48, -32, -16, 16, 32, 48, 64, 96, 127] {
        for n in [40usize, 80, 130, 190] {
            tapes.push((
                format!("t{s}_{n}"),
                vec![
                    "160:420:1:1:0".to_string(),
                    format!("420:{}:{s}:1:0", 420 + n),
                    format!("{}:{ticks}:1:1:0", 420 + n),
                ],
            ));
        }
    }
    let jobs_t = jobs;
    let nextt = AtomicUsize::new(0);
    std::thread::scope(|s| { for _ in 0..jobs_t { s.spawn(|| loop {
        let i = nextt.fetch_add(1, Ordering::SeqCst);
        let Some((name, segs)) = tapes.get(i) else { return };
        let gt = format!("{outdir}/{name}.gtape");
        let gh = format!("{outdir}/{name}.Ghost.Gbx");
        let mut args: Vec<String> = vec!["tape".into(), "--from".into(), template.clone(), "--out".into(), gt.clone(), "--ticks".into(), ticks.to_string()];
        for s in segs {
            args.push("--seg".into());
            args.push(s.clone());
        }
        let _ = std::process::Command::new(&me).args(&args).output();
        let _ = std::process::Command::new(&ghostb).args(["tape", "inject", &carrier, &gh, "--tape", &gt]).output();
    }); } });

    let mut jobsv: Vec<(i64, i64, i64, usize)> = Vec::new();
    for &cx in &cxs {
        for cy in cy0..=cy1 {
            for d in [0i64, 2] {
                for ti in 0..tapes.len() {
                    jobsv.push((cx, cy, d, ti));
                }
            }
        }
    }
    let next = AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::<String>::new());
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(&(cx, cy, d, ti)) = jobsv.get(i) else { return };
                let tag = format!("c{cx}_y{cy}_d{d}_{}", tapes[ti].0);
                let map = format!("{outdir}/maps/{tag}.Map.Gbx");
                let o = std::process::Command::new(&tmmaps)
                    .args([
                        "move", &base, "--out", &map,
                        "--move", &format!("{plat}:{cx},{cy},15"),
                        "--move", &format!("{spawnb}:{cx},{},15:{d}", cy + 1),
                    ])
                    .output();
                if o.map(|o| !o.status.success()).unwrap_or(true) {
                    out.lock().unwrap().push(format!("{tag}\tMAP_FAILED"));
                    continue;
                }
                let csv = format!("{outdir}/csv/{tag}.csv");
                let work = format!("/tmp/uwplat-{tag}");
                let _ = std::fs::remove_dir_all(&work);
                let tape = format!("{outdir}/{}.Ghost.Gbx", tapes[ti].0);
                let r = std::process::Command::new(&fk)
                    .args(["trace", "--tape", &tape, "--map", &map, "--work", &work, "--at", "tick:160", "--out", &csv])
                    .env("FK_VERR_MAX", "3.0")
                    .output();
                let _ = std::fs::remove_dir_all(&work);
                let _ = r;
                let line = match Traj::load(&csv) {
                    Ok(t) => {
                        let last = t.rows.last().cloned().unwrap_or_default();
                        let fired = last.t < 47.0;
                        // where it crossed the trigger plane, and how high it drove
                        let mut cross = String::from("\t\t");
                        for w in t.rows.windows(2) {
                            if (w[0].z - 494.5) * (w[1].z - 494.5) <= 0.0 && (w[0].z - w[1].z).abs() > 1e-9 {
                                cross = format!("{:.2}\t{:.2}\t{:.3}", w[1].y, w[1].x, w[1].t);
                                break;
                            }
                        }
                        let mut ymax = f64::MIN;
                        for r in &t.rows {
                            if r.y > ymax {
                                ymax = r.y;
                            }
                        }
                        format!(
                            "{}\tfloor {:6.1}\ttend {:7.3}\tend ({:7.1},{:7.2},{:7.1})\tmaxy {:7.2}\tcross {}",
                            if fired { "FIRED " } else { "no    " },
                            cy as f64 * 8.0 - 62.0,
                            last.t, last.x, last.y, last.z, ymax, cross
                        )
                    }
                    Err(e) => format!("TRACE_FAILED\t{e}"),
                };
                out.lock().unwrap().push(format!("{tag}\t{line}"));
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort();
    for l in v {
        println!("{l}");
    }
    0
}
