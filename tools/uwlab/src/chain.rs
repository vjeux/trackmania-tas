//! `uwlab chain` — can the car GET there? A reachability solver over the map.
//!
//! Underwater on this map the car cannot gain height: gravity is 2.2 m/s²,
//! terminal sink 2.68 m/s, horizontal drag linear with k ≈ 0.55, and the best
//! ramp on the map is worth about +3 m of apex. So the reachable set is
//! "everything at or below a surface you can already stand on, within one
//! drag-limited glide of it", closed transitively. That is a graph search, and
//! nobody on this map had run it: every previous arm searched TAPES from a
//! spawn, which answers "does this input stream work" and never "is there a
//! path at all".
//!
//! The surface grid comes from the block census, so this is a CENSUS VIEW and
//! not a measurement — it says where to point the engine, and every edge it
//! proposes has to be driven before it is believed. `--calibrate` scores the
//! grid against a measured plumb lattice so the size of that lie is a number.

use std::collections::HashMap;

const K_H: f64 = 0.55; // horizontal linear drag, fitted 0.489..0.602
const VT: f64 = 2.68; // terminal sink
const G: f64 = 2.2; // effective gravity in water
const K_V: f64 = G / VT;

/// Horizontal distance covered in t seconds from launch speed v0.
fn dist(v0: f64, t: f64) -> f64 {
    (v0 / K_H) * (1.0 - (-K_H * t).exp())
}
/// Vertical displacement in t seconds from vertical launch speed vy0 (up +).
fn drop_at(vy0: f64, t: f64) -> f64 {
    -VT * t + ((vy0 + VT) / K_V) * (1.0 - (-K_V * t).exp())
}
/// Time to cover horizontal distance d at launch speed v0; None past the
/// asymptote v0/k, which is the whole point of this map.
fn time_for(v0: f64, d: f64) -> Option<f64> {
    let a = v0 / K_H;
    if d >= a * 0.999 {
        return None;
    }
    Some(-(1.0 - d / a).ln() / K_H)
}
/// The height LOST getting to horizontal distance d. None if out of reach.
pub fn glide_drop(v0: f64, vy0: f64, d: f64) -> Option<f64> {
    let t = time_for(v0, d)?;
    Some(-drop_at(vy0, t))
}

#[derive(Clone, Copy, PartialEq)]
pub struct Cell {
    pub y: f64,
}

pub fn cmd_chain(a: &[String]) -> i32 {
    let pos = |n: &str| a.iter().position(|s| s == n).and_then(|i| a.get(i + 1));
    let Some(path) = a.iter().find(|s| !s.starts_with("--") && s.ends_with(".tsv")) else {
        eprintln!("uwlab chain: need CENSUS.tsv");
        return 2;
    };
    let v0: f64 = pos("--v").and_then(|s| s.parse().ok()).unwrap_or(28.6);
    let vy0: f64 = pos("--vy").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let climb: f64 = pos("--climb").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let cell: f64 = 32.0;
    let drops = "Water|Spring|Tree|Lamp|Light|Sign|Screen|Flag";
    let dropv: Vec<&str> = drops.split('|').collect();

    // ---- surface grid: the top of each 32 m column.
    // A block's row carries the cell's BASE height. A stack of fillers
    // (pillars, walls) is solid to the top of the highest one, so its surface
    // is y+8; a flat platform's own surface is y+0.16. Both are guesses about
    // geometry the census does not carry, which is what --calibrate is for.
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("uwlab chain: cannot read {path}");
        return 2;
    };
    let mut top: HashMap<(i64, i64), (f64, String)> = HashMap::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 12 {
            continue;
        }
        let name = f[2];
        if dropv.iter().any(|d| name.contains(d)) {
            continue;
        }
        let (x, y, z) = (
            f[8].parse::<f64>().unwrap_or(-1e9),
            f[9].parse::<f64>().unwrap_or(-1e9),
            f[10].parse::<f64>().unwrap_or(-1e9),
        );
        if x < -1e8 {
            continue;
        }
        // HFC ("hole") variants of the canopy are NOT solid — measured: a
        // flight passes straight through the ring one cell nearer than the
        // deck, and the gate slot is exactly the HFC cell of the upper canopy.
        let surface = if name.contains("HFC") && name.contains("Canopy") {
            continue;
        } else if name.contains("Pillar")
            || name.contains("Structure")
            || name.contains("Stand")
            || name.contains("Wall")
            || name.contains("Gate")
        {
            y + 8.0
        } else {
            y + 0.16
        };
        let k = ((x / cell).floor() as i64, (z / cell).floor() as i64);
        let e = top.entry(k).or_insert((f64::MIN, String::new()));
        if surface > e.0 {
            *e = (surface, name.to_string());
        }
    }

    // Terraformed ground carries NO block row, so the census alone puts the
    // map's biggest hill at 10 m. Merge in a measured plumb lattice: a
    // column the engine says is higher than the census wins.
    if let Some(lat) = pos("--lattice") {
        let Ok(t) = std::fs::read_to_string(lat) else {
            eprintln!("uwlab chain: cannot read {lat}");
            return 2;
        };
        let mut n = 0;
        for line in t.lines().skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 6 || f[1] == "TRACE_FAILED" { continue; }
            let tag = f[0].split("__").next().unwrap_or("");
            let p: Vec<&str> = tag.trim_start_matches('s').split('_').collect();
            if p.len() < 4 { continue; }
            let (cx, cz) = (p[0].parse::<i64>().unwrap_or(-1), p[2].parse::<i64>().unwrap_or(-1));
            let y: f64 = f[4].parse().unwrap_or(f64::NAN);
            if cx < 0 || cz < 0 || y.is_nan() || y > 190.0 { continue; }
            let e = top.entry((cx, cz)).or_insert((f64::MIN, "measured".into()));
            if y > e.0 { *e = (y, "measured-terrain".into()); n += 1; }
        }
        eprintln!("chain: lattice raised {n} columns above their census height");
    }

    if let Some(cal) = pos("--calibrate") {
        let Ok(t) = std::fs::read_to_string(cal) else {
            eprintln!("uwlab chain: cannot read {cal}");
            return 2;
        };
        let (mut n, mut agree) = (0, 0);
        let mut worst: Vec<(f64, String)> = Vec::new();
        for line in t.lines() {
            // "(cx,cz) x NNNN z NNNN  CONTACT y  NNN.NNN at ( x, z) ..."
            if !line.contains("CONTACT y") {
                continue;
            }
            let cut = line.split("CONTACT y").nth(1).unwrap_or("");
            let meas: f64 = cut.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(f64::NAN);
            let head = line.split(')').next().unwrap_or("").trim_start_matches('(');
            let p: Vec<&str> = head.split(',').collect();
            if p.len() != 2 || meas.is_nan() {
                continue;
            }
            let (cx, cz): (i64, i64) = (
                p[0].trim().parse().unwrap_or(0),
                p[1].trim().parse().unwrap_or(0),
            );
            n += 1;
            let got = top.get(&(cx, cz)).map(|v| v.0).unwrap_or(f64::MIN);
            if (got - meas).abs() < 2.0 {
                agree += 1;
            } else {
                worst.push((got - meas, format!("({cx},{cz}) census {got:.2} vs plumb {meas:.2}  {}", top.get(&(cx,cz)).map(|v|v.1.clone()).unwrap_or_default())));
            }
        }
        worst.sort_by(|a, b| b.0.abs().partial_cmp(&a.0.abs()).unwrap());
        println!("calibrate: {agree}/{n} columns within 2 m of the plumb probe");
        for (_, w) in worst.iter().take(25) {
            println!("  {w}");
        }
        return 0;
    }

    // ---- the graph
    let start: Vec<(i64, i64)> = pos("--from")
        .map(|s| {
            s.split(';')
                .filter_map(|p| {
                    let (a, b) = p.split_once(',')?;
                    Some(((a.trim().parse::<f64>().ok()? / cell).floor() as i64, (b.trim().parse::<f64>().ok()? / cell).floor() as i64))
                })
                .collect()
        })
        .unwrap_or_default();
    let goal: Vec<(i64, i64)> = pos("--to")
        .map(|s| {
            s.split(';')
                .filter_map(|p| {
                    let (a, b) = p.split_once(',')?;
                    Some(((a.trim().parse::<f64>().ok()? / cell).floor() as i64, (b.trim().parse::<f64>().ok()? / cell).floor() as i64))
                })
                .collect()
        })
        .unwrap_or_default();
    if start.is_empty() || goal.is_empty() {
        eprintln!("uwlab chain: --from x,z[;x,z...] --to x,z[;x,z...]");
        return 2;
    }
    let keys: Vec<(i64, i64)> = top.keys().copied().collect();
    let reach_cells = ((v0 / K_H) / cell).ceil() as i64 + 1;
    let mut prev: HashMap<(i64, i64), (i64, i64)> = HashMap::new();
    let mut seen: Vec<(i64, i64)> = Vec::new();
    let mut q: std::collections::VecDeque<(i64, i64)> = std::collections::VecDeque::new();
    for s in &start {
        if top.contains_key(s) {
            q.push_back(*s);
            seen.push(*s);
        } else {
            eprintln!("chain: start cell {s:?} has no surface in the census");
        }
    }
    let mut hit: Option<(i64, i64)> = None;
    while let Some(u) = q.pop_front() {
        if goal.contains(&u) {
            hit = Some(u);
            break;
        }
        let yu = top[&u].0;
        for dx in -reach_cells..=reach_cells {
            for dz in -reach_cells..=reach_cells {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let v = (u.0 + dx, u.1 + dz);
                if seen.contains(&v) {
                    continue;
                }
                let Some(&(yv, _)) = top.get(&v) else { continue };
                // centre-to-centre, minus half a cell each side: the nearest
                // approach two 32 m cells can have.
                let d = (((dx * dx + dz * dz) as f64).sqrt() * cell - cell).max(1.0);
                let ok = if yv <= yu + climb {
                    match glide_drop(v0, vy0, d) {
                        Some(dr) => yu - dr >= yv,
                        None => false,
                    }
                } else {
                    false
                };
                if ok {
                    seen.push(v);
                    prev.insert(v, u);
                    q.push_back(v);
                }
            }
        }
    }
    let _ = keys;
    println!(
        "chain: v0 {v0} m/s, vy0 {vy0}, climb allowance {climb} m — reach {:.1} m, {} cells reachable",
        v0 / K_H,
        seen.len()
    );
    match hit {
        Some(g) => {
            let mut path = vec![g];
            let mut c = g;
            while let Some(&p) = prev.get(&c) {
                path.push(p);
                c = p;
            }
            path.reverse();
            println!("REACHED {:?}", g);
            for w in path.windows(2) {
                let (u, v) = (w[0], w[1]);
                let d = ((((v.0 - u.0).pow(2) + (v.1 - u.1).pow(2)) as f64).sqrt() * cell - cell).max(1.0);
                println!(
                    "  ({:5.0},{:5.0}) y {:7.2} {:28} --{:5.1} m-->  ({:5.0},{:5.0}) y {:7.2} {}",
                    u.0 as f64 * cell, u.1 as f64 * cell, top[&u].0, top[&u].1,
                    d,
                    v.0 as f64 * cell, v.1 as f64 * cell, top[&v].0, top[&v].1
                );
            }
        }
        None => {
            println!("NOT REACHED. The frontier's closest approach to each goal:");
            for g in &goal {
                let Some(&(gy, _)) = top.get(g) else {
                    println!("  goal {g:?}: no surface in the census");
                    continue;
                };
                let mut best = (f64::MAX, (0i64, 0i64), 0.0);
                for s in &seen {
                    let d = ((((g.0 - s.0).pow(2) + (g.1 - s.1).pow(2)) as f64).sqrt() * cell - cell).max(1.0);
                    if d < best.0 {
                        best = (d, *s, top[s].0);
                    }
                }
                println!(
                    "  goal ({:.0},{:.0}) y {:.2}: nearest reachable ({:.0},{:.0}) y {:.2} at {:.0} m — needs {:.1} m of glide at {:.1} m/s (have {:.1})",
                    g.0 as f64 * cell, g.1 as f64 * cell, gy,
                    best.1 .0 as f64 * cell, best.1 .1 as f64 * cell, best.2, best.0,
                    best.0, v0, v0 / K_H
                );
            }
        }
    }
    0
}
