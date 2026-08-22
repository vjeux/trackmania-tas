//! Cells, and when a path first enters one.
//!
//! The map is a 32 x 8 x 32 m grid: `cell = (floor(x/32), floor((y+64)/8),
//! floor(z/32))`. A relocated-checkpoint rung asks exactly one question — did
//! the car enter this cell — so a rung is only as good as the claim about
//! where the reference car was when.
//!
//! `first` prints the first-visit time of every cell a path enters in a
//! window. `at` prints the cell a path is in at named times. `check` takes
//! `cell=time` claims and grades each one against the path, which is the audit
//! a ladder needs before any depth read off it means anything.

use crate::csv::Sample;

pub fn cell_of(x: f64, y: f64, z: f64) -> (i64, i64, i64) {
    ((x / 32.0).floor() as i64, ((y + 64.0) / 8.0).floor() as i64, (z / 32.0).floor() as i64)
}

pub fn first_visits(s: &[Sample]) -> Vec<((i64, i64, i64), f64)> {
    let mut seen: std::collections::HashMap<(i64, i64, i64), f64> = std::collections::HashMap::new();
    for r in s {
        seen.entry(cell_of(r.x, r.y, r.z)).or_insert(r.t);
    }
    let mut v: Vec<_> = seen.into_iter().collect();
    v.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    v
}

pub fn report_first(s: &[Sample]) {
    println!("{:>14} {:>10} {:>9} {:>8} {:>9} {:>8}", "cell", "first_s", "x", "y", "z", "kmh");
    let v = first_visits(s);
    for (c, t) in v {
        let r = s.iter().find(|r| r.t == t).unwrap();
        println!(
            "{:>14} {:>10.3} {:>9.2} {:>8.2} {:>9.2} {:>8.1}",
            format!("{},{},{}", c.0, c.1, c.2), t, r.x, r.y, r.z, r.v
        );
    }
}

/// Grade `X,Y,Z=TIME` claims against the path.
pub fn report_check(s: &[Sample], claims: &[String]) {
    let v = first_visits(s);
    let mut fails = 0;
    println!("{:>14} {:>12} {:>12} {:>10}", "cell", "claimed_s", "measured_s", "verdict");
    for c in claims {
        let (cs, ts) = match c.split_once('=') {
            Some(p) => p,
            None => { eprintln!("bad claim {}", c); continue; }
        };
        let p: Vec<i64> = cs.split(',').map(|x| x.trim().parse().expect("cell")).collect();
        let cell = (p[0], p[1], p[2]);
        let claimed: f64 = ts.parse().expect("time");
        match v.iter().find(|(k, _)| *k == cell) {
            Some((_, t)) => {
                let ok = (t - claimed).abs() < 0.5;
                if !ok { fails += 1; }
                println!(
                    "{:>14} {:>12.3} {:>12.3} {:>10}",
                    format!("{},{},{}", cell.0, cell.1, cell.2), claimed, t,
                    if ok { "ok" } else { "MISMATCH" }
                );
            }
            None => {
                fails += 1;
                println!(
                    "{:>14} {:>12.3} {:>12} {:>10}",
                    format!("{},{},{}", cell.0, cell.1, cell.2), claimed, "never", "NEVER ENTERED"
                );
            }
        }
    }
    println!("{} of {} claims fail", fails, claims.len());
}
