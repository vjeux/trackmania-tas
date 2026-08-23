//! `pkz2` — the 153527 arm's analysis subcommands (2026-08-22, `pkz2_` arm).
//!
//! Everything here reads. Nothing here writes a ghost or a map.
//!
//! Why it exists: the published mechanism for this map's climb rests on a
//! deceleration compared against `9.81 * sin(theta)`, and TM2020's gravity is
//! not 9.81. Before any more search is spent on that hill the energy
//! bookkeeping has to be redone with this map's own gravity, measured on this
//! map's own recording.

mod csv;
mod cells;
mod climb;
mod dev;
mod gen;
mod mkcand;
mod edit;
mod energy;
mod gravity;
mod jumps;
mod tape;

fn usage() -> ! {
    eprintln!(
        "pkz2 <cmd>

  gravity <traj.csv> [--from S] [--to S]
        measure gravity from a recording's own free fall: the mode of the
        vertical acceleration over airborne stretches, with the sample count
        and the spread, plus the same figure computed from the derived
        is_ground_contact bit as a cross-check.

  energy <traj.csv> --from S --to S [--g M/S2] [--step S]
        per-sample energy bookkeeping: E = v^2/2 + g*y (J/kg), the rate it
        changes, the along-path acceleration, and the slope gravity that
        accounts for part of it. A car with a live engine on a flat stretch
        MAKES energy; a coasting car cannot.

  traj <traj.csv> --from S --to S [--step S]
        dump the window."
    );
    std::process::exit(2)
}

fn flag(a: &[String], name: &str) -> Option<String> {
    let pre = format!("--{}", name);
    for (i, s) in a.iter().enumerate() {
        if s == &pre {
            return a.get(i + 1).cloned();
        }
        if let Some(v) = s.strip_prefix(&format!("{}=", pre)) {
            return Some(v.to_string());
        }
    }
    None
}

fn fnum(a: &[String], name: &str, d: f64) -> f64 {
    flag(a, name).map(|s| s.parse().expect("number")).unwrap_or(d)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.is_empty() {
        usage()
    }
    let cmd = a[0].clone();
    let path = a.get(1).cloned().unwrap_or_else(|| usage());
    let from = fnum(&a, "from", f64::NEG_INFINITY);
    let to = fnum(&a, "to", f64::INFINITY);
    match cmd.as_str() {
        "gravity" => {
            let s = csv::read(&path, from, to).unwrap_or_else(|e| { eprintln!("{}", e); std::process::exit(1) });
            gravity::report(&s);
        }
        "energy" => {
            let s = csv::read(&path, from, to).unwrap_or_else(|e| { eprintln!("{}", e); std::process::exit(1) });
            energy::report(&s, fnum(&a, "g", 24.6), fnum(&a, "step", 0.0));
        }
        "traj" => {
            let s = csv::read(&path, from, to).unwrap_or_else(|e| { eprintln!("{}", e); std::process::exit(1) });
            let step = fnum(&a, "step", 0.0);
            let mut want = f64::NEG_INFINITY;
            println!("{:>9} {:>9} {:>8} {:>9} {:>8} {:>4} {:>4} {:>7}", "race_s", "x", "y", "z", "kmh", "gas", "brk", "steer");
            for r in &s {
                if r.t < want { continue; }
                want = if step > 0.0 { r.t + step - 1e-9 } else { f64::NEG_INFINITY };
                println!(
                    "{:>9.3} {:>9.2} {:>8.2} {:>9.2} {:>8.1} {:>4} {:>4} {:>7}",
                    r.t, r.x, r.y, r.z, r.v,
                    r.gas.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "-".into()),
                    r.brake.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "-".into()),
                    r.steer.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".into()),
                );
            }
        }
        "climb" => {
            let base = flag(&a, "base").expect("--base");
            let outdir = flag(&a, "outdir").expect("--outdir");
            let nums = |n: &str, d: &str| -> Vec<i64> {
                flag(&a, n).unwrap_or_else(|| d.to_string()).split(',').map(|s| s.parse().unwrap()).collect()
            };
            let mut fixed = Vec::new();
            for (i, s) in a.iter().enumerate() {
                if s == "--edit" { fixed.push(edit::parse_edit(&a[i + 1]).unwrap()); }
            }
            let mut zigs = Vec::new();
            for bf in nums("brakefrom", "147600") { for bm in nums("brakems", "0") {
              for lg in nums("legms", "450") { for l2 in nums("legms2", "0") {
                for n in nums("legs", "20") { for f in nums("first", "127") {
                  for rv in nums("rev", "0") {
                  zigs.push(gen::Zig { brake_from: bf, brake_ms: bm, start_ms: bf + bm,
                                       leg_ms: lg, leg2_ms: if l2 == 0 { lg } else { l2 },
                                       legs: n, first: f, gas: true, rev: rv != 0 }); }
            } } } } } }
            climb::run(&climb::Cfg { base, map: flag(&a, "map").expect("--map"), outdir,
                                     at: flag(&a, "at").unwrap_or_else(|| "tick:14400".into()),
                                     par: fnum(&a, "par", 20.0) as usize,
                                     from: fnum(&a, "from", 146.0), to: fnum(&a, "to", 300.0),
                                     target: flag(&a, "target").map(|s| { let p: Vec<f64> = s.split(',').map(|x| x.parse().unwrap()).collect(); (p[0], p[1], p[2]) }) }, &zigs, &fixed);
        }
        "sweep" => {
            let base = flag(&a, "base").expect("--base");
            let outdir = flag(&a, "outdir").expect("--outdir");
            let mut fixed = Vec::new();
            for (i, s) in a.iter().enumerate() {
                if s == "--edit" { fixed.push(edit::parse_edit(&a[i + 1]).unwrap()); }
            }
            let specs = match path.as_str() {
                "respawn" => gen::respawn_sweep(fnum(&a,"from",0.0) as i64, fnum(&a,"to",0.0) as i64, fnum(&a,"step",50.0) as i64, &fixed),
                "zigzag" => {
                    let nums = |n: &str, d: &str| -> Vec<i64> {
                        flag(&a, n).unwrap_or_else(|| d.to_string()).split(',').map(|s| s.parse().unwrap()).collect()
                    };
                    let mut v = Vec::new();
                    for bf in nums("brakefrom", "146000") {
                        for bm in nums("brakems", "600") {
                            for lg in nums("legms", "800") {
                                for n in nums("legs", "12") {
                                    for f in nums("first", "127,-127") {
                                        for g in nums("gas", "1") {
                                          for rv in nums("rev", "0") {
                                            for l2 in nums("legms2", "0") {
                                                v.push(gen::zigzag(&gen::Zig {
                                                    brake_from: bf, brake_ms: bm,
                                                    start_ms: bf + bm, leg_ms: lg,
                                                    leg2_ms: if l2 == 0 { lg } else { l2 },
                                                    legs: n, first: f, gas: g != 0, rev: rv != 0,
                                                }, &fixed));
                                            }
                                          }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    v
                }
                other => { eprintln!("unknown sweep kind {}", other); std::process::exit(2) }
            };
            match mkcand::run(&base, &outdir, &specs) { Ok(n) => println!("wrote {} of {} candidates to {}", n, specs.len(), outdir), Err(e) => { eprintln!("{}", e); std::process::exit(1) } }
        }
        "tape" => { tape::report(&path); }
        "dev" => {
            let other = a.get(2).cloned().expect("pkz2 dev A.csv B.csv");
            let sa = csv::read(&path, from, to).unwrap();
            let sb = csv::read(&other, f64::NEG_INFINITY, f64::INFINITY).unwrap();
            if a.iter().any(|s| s == "--lag") { dev::lag_scan(&sa, &sb, fnum(&a,"lo",-6.0), fnum(&a,"hi",6.0), fnum(&a,"lagstep",0.05), (from.max(0.0), if to.is_finite() { to } else { 90.0 })); } else { dev::report(&sa, &sb, fnum(&a, "step", 0.0)); }
        }
        "edit" => {
            let out = flag(&a, "out").expect("--out");
            let mut es = Vec::new();
            for (i, s) in a.iter().enumerate() {
                if s == "--edit" { es.push(edit::parse_edit(&a[i + 1]).unwrap_or_else(|e| { eprintln!("{}", e); std::process::exit(2) })); }
            }
            std::process::exit(edit::run(&path, &out, &es));
        }
        "jumps" => {
            let s = csv::read(&path, from, to).unwrap_or_else(|e| { eprintln!("{}", e); std::process::exit(1) });
            jumps::report(&s, fnum(&a, "min", 5.0));
        }
        "apex" => {
            for p in a.iter().skip(1).filter(|s| !s.starts_with("--")) {
                let tg = flag(&a, "target").map(|s| { let q: Vec<f64> = s.split(',').map(|x| x.parse().unwrap()).collect(); (q[0], q[1], q[2]) });
                match csv::read(p, from, to) { Ok(s) => cells::report_apex(p, &s, tg), Err(e) => eprintln!("{}", e) }
            }
        }
        "cells" => {
            let s = csv::read(&path, from, to).unwrap_or_else(|e| { eprintln!("{}", e); std::process::exit(1) });
            let claims: Vec<String> = a.iter().skip(2).filter(|s| s.contains('=') && !s.starts_with("--")).cloned().collect();
            if claims.is_empty() { cells::report_first(&s) } else { cells::report_check(&s, &claims) }
        }
        _ => usage(),
    }
}
