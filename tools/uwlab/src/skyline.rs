//! `uwlab skyline` — the map's high-altitude skeleton, as a picture.
//!
//! A census row is a name and a cell. The question a route needs answered is
//! "what is the highest thing I could stand on over there, and is there a
//! chain of them" — which is a picture, not a list. This prints one character
//! per cell: the highest STRUCTURAL block in that column (water and scenery
//! filtered out by default), bucketed by height.
//!
//! It is a census view and not a measurement: the census names a block, it
//! does not say the block is solid or where its drivable face is. Use it to
//! choose where to point a probe, never as evidence that something is
//! reachable.

use std::collections::HashMap;

pub fn cmd_skyline(a: &[String]) -> i32 {
    let pos = |n: &str| a.iter().position(|s| s == n).and_then(|i| a.get(i + 1));
    let Some(path) = a.iter().find(|s| !s.starts_with("--") && s.ends_with(".tsv")) else {
        eprintln!("uwlab skyline: need CENSUS.tsv");
        return 2;
    };
    let cell: f64 = pos("--cell").and_then(|s| s.parse().ok()).unwrap_or(32.0);
    let ymin: f64 = pos("--ymin").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let filter = pos("--filter").cloned();
    let drop = pos("--drop")
        .cloned()
        .unwrap_or_else(|| "Water|Spring|Tree|Lamp|Light|Sign|Screen|Flag|Grass".into());
    let drops: Vec<&str> = drop.split('|').filter(|s| !s.is_empty()).collect();
    let (x0, x1) = pos("--x")
        .and_then(|s| s.split_once(':'))
        .map(|(a, b)| (a.parse().unwrap_or(0.0), b.parse().unwrap_or(0.0)))
        .unwrap_or((0.0, 2048.0));
    let (z0, z1) = pos("--z")
        .and_then(|s| s.split_once(':'))
        .map(|(a, b)| (a.parse().unwrap_or(0.0), b.parse().unwrap_or(0.0)))
        .unwrap_or((0.0, 2048.0));

    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("uwlab skyline: cannot read {path}");
        return 2;
    };
    let mut top: HashMap<(i64, i64), (f64, String)> = HashMap::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 12 {
            continue;
        }
        let name = f[2];
        if drops.iter().any(|d| name.contains(d)) {
            continue;
        }
        if let Some(pat) = &filter {
            if !name.contains(pat.as_str()) {
                continue;
            }
        }
        let (x, y, z) = (
            f[8].parse::<f64>().unwrap_or(-1e9),
            f[9].parse::<f64>().unwrap_or(-1e9),
            f[10].parse::<f64>().unwrap_or(-1e9),
        );
        if y < ymin || x < x0 || x > x1 || z < z0 || z > z1 {
            continue;
        }
        let k = ((x / cell).floor() as i64, (z / cell).floor() as i64);
        let e = top.entry(k).or_insert((f64::MIN, String::new()));
        if y > e.0 {
            *e = (y, name.to_string());
        }
    }
    if top.is_empty() {
        println!("(nothing)");
        return 0;
    }
    let cx0 = (x0 / cell).floor() as i64;
    let cx1 = (x1 / cell).floor() as i64;
    let cz0 = (z0 / cell).floor() as i64;
    let cz1 = (z1 / cell).floor() as i64;
    // legend: one char per 16 m of height
    let ch = |y: f64| -> char {
        let b = ((y - ymin) / 16.0).floor() as i64;
        match b {
            b if b < 0 => '.',
            0..=9 => (b'0' + b as u8) as char,
            10..=35 => (b'a' + (b - 10) as u8) as char,
            _ => '#',
        }
    };
    println!("skyline of {path}: cell {cell} m, ymin {ymin}, char = (y-ymin)/16 in 0-9a-z");
    print!("      ");
    for cx in cx0..=cx1 {
        print!("{}", if (cx * cell as i64) % 128 == 0 { '|' } else { ' ' });
    }
    println!();
    for cz in (cz0..=cz1).rev() {
        print!("z{:5.0}", cz as f64 * cell);
        for cx in cx0..=cx1 {
            match top.get(&(cx, cz)) {
                Some((y, _)) => print!("{}", ch(*y)),
                None => print!(" "),
            }
        }
        println!();
    }
    print!("      ");
    for cx in cx0..=cx1 {
        print!("{}", if (cx * cell as i64) % 128 == 0 { '|' } else { ' ' });
    }
    println!();
    println!("x from {} to {} (| every 128 m)", cx0 as f64 * cell, cx1 as f64 * cell);
    0
}

/// `uwlab lattice` — the MEASURED surface map, as a picture.
///
/// Reads a `uwlab sweep` plumb run (one spawn per 32 m cell, a no-input tape)
/// and prints where a dropped car came to rest in each column. This is the
/// census's blind spot made visible: terraformed ground carries no block row
/// at all, so the biggest hill on this map reads as `Grass@10` in the census
/// and as 162 m here.
pub fn cmd_lattice(a: &[String]) -> i32 {
    let pos = |n: &str| a.iter().position(|s| s == n).and_then(|i| a.get(i + 1));
    let Some(path) = a.iter().find(|s| !s.starts_with("--") && s.ends_with(".tsv")) else {
        eprintln!("uwlab lattice: need SWEEP.tsv");
        return 2;
    };
    let ymin: f64 = pos("--ymin").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let step: f64 = pos("--step").and_then(|s| s.parse().ok()).unwrap_or(16.0);
    let col: usize = pos("--col").and_then(|s| s.parse().ok()).unwrap_or(5); // yend
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("uwlab lattice: cannot read {path}");
        return 2;
    };
    let mut g: std::collections::HashMap<(i64, i64), f64> = std::collections::HashMap::new();
    let (mut x0, mut x1, mut z0, mut z1) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() <= col || f[1] == "TRACE_FAILED" {
            continue;
        }
        // tag: s<cx>_<cy>_<cz>_d<dir>__<tape>
        let tag = f[0].split("__").next().unwrap_or("");
        let p: Vec<&str> = tag.trim_start_matches('s').split('_').collect();
        if p.len() < 4 {
            continue;
        }
        let (cx, cz) = (p[0].parse::<i64>().unwrap_or(-999), p[2].parse::<i64>().unwrap_or(-999));
        let y: f64 = f[col].parse().unwrap_or(f64::NAN);
        if cx < 0 || cz < 0 || y.is_nan() {
            continue;
        }
        let e = g.entry((cx, cz)).or_insert(f64::MIN);
        if y > *e {
            *e = y;
        }
        x0 = x0.min(cx);
        x1 = x1.max(cx);
        z0 = z0.min(cz);
        z1 = z1.max(cz);
    }
    println!("measured lattice {path}: char = (y-{ymin})/{step} in 0-9a-z, '.' below, ' ' no reading");
    for cz in (z0..=z1).rev() {
        print!("z{:5}", cz * 32);
        for cx in x0..=x1 {
            match g.get(&(cx, cz)) {
                Some(&y) => {
                    let b = ((y - ymin) / step).floor() as i64;
                    print!(
                        "{}",
                        match b {
                            b if b < 0 => '.',
                            0..=9 => (b'0' + b as u8) as char,
                            10..=35 => (b'a' + (b - 10) as u8) as char,
                            _ => '#',
                        }
                    );
                }
                None => print!(" "),
            }
        }
        println!();
    }
    print!("      ");
    for cx in x0..=x1 {
        print!("{}", if (cx * 32) % 128 == 0 { '|' } else { ' ' });
    }
    println!("\nx {} .. {}", x0 * 32, x1 * 32);
    0
}
