//! `re16_warpcover SCENE3D.obj [--map-size 8192] [--cell 32] [--y-plane -6] [--group WarpGround]` — which cells of a
//! map's tile plane are NOT covered (in XZ) by any triangle of one group of a decoration Scene3d export (mapgeom
//! `scene3d --out`): the holes in the WarpGround skirt's ring triangulation, printed as runs per row plus a summary,
//! so V4's bright tile bands (the editor's outside floor) can be compared with the mesh's own gaps box-free
//! (RE 16, 2026-09-28 22:40Z: the Square64Water solid's 4 quadrant leaves each carry their share of the rings out to
//! 97 km, so a per-leaf frustum cull cannot drop the far rings — a bright band under the skirt is a hole or nothing).
//! Also prints, per uncovered cell run, the y of the nearest covering triangle to show where the skirt sits.
use std::fs;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 2 {
        eprintln!("usage: re16_warpcover SCENE3D.obj [--map-size M] [--cell C] [--group NAME] [--origin X Z]");
        std::process::exit(2);
    }
    let mut map_size = 8192.0f64;
    let mut cell = 32.0f64;
    let mut group = "WarpGround".to_string();
    let mut origin = (0.0f64, 0.0f64);
    let mut i = 2;
    while i < a.len() {
        match a[i].as_str() {
            "--map-size" => { map_size = a[i + 1].parse().unwrap(); i += 2; }
            "--cell" => { cell = a[i + 1].parse().unwrap(); i += 2; }
            "--group" => { group = a[i + 1].clone(); i += 2; }
            "--origin" => { origin = (a[i + 1].parse().unwrap(), a[i + 2].parse().unwrap()); i += 3; }
            _ => { i += 1; }
        }
    }
    let text = fs::read_to_string(&a[1]).expect("obj");
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut tris: Vec<[usize; 3]> = Vec::new();
    let mut in_group = false;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let x: f64 = it.next().unwrap().parse().unwrap();
                let y: f64 = it.next().unwrap().parse().unwrap();
                let z: f64 = it.next().unwrap().parse().unwrap();
                verts.push([x, y, z]);
            }
            Some("o") | Some("g") => { in_group = it.next().map(|n| n == group).unwrap_or(false); }
            Some("f") if in_group => {
                let idx: Vec<usize> = it.map(|t| t.split('/').next().unwrap().parse::<usize>().unwrap() - 1).collect();
                for k in 1..idx.len() - 1 {
                    tris.push([idx[0], idx[k], idx[k + 1]]);
                }
            }
            _ => {}
        }
    }
    println!("group {group}: {} triangles, {} vertices in file; map {map_size} m, cell {cell} m, origin ({}, {})", tris.len(), verts.len(), origin.0, origin.1);
    let n = (map_size / cell).round() as usize;
    let mut uncovered = 0usize;
    let mut rows: Vec<String> = Vec::new();
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for iz in 0..n {
        let cz = origin.1 + (iz as f64 + 0.5) * cell;
        let mut run_start: Option<usize> = None;
        let mut row_runs: Vec<(usize, usize)> = Vec::new();
        for ix in 0..n {
            let cx = origin.0 + (ix as f64 + 0.5) * cell;
            let mut covered = false;
            for t in &tris {
                let p0 = verts[t[0]];
                let p1 = verts[t[1]];
                let p2 = verts[t[2]];
                // 2D (x, z) point-in-triangle with edge inclusivity
                let d1 = (cx - p1[0]) * (p0[2] - p1[2]) - (p0[0] - p1[0]) * (cz - p1[2]);
                let d2 = (cx - p2[0]) * (p1[2] - p2[2]) - (p1[0] - p2[0]) * (cz - p2[2]);
                let d3 = (cx - p0[0]) * (p2[2] - p0[2]) - (p2[0] - p0[0]) * (cz - p0[2]);
                let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
                let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
                if !(has_neg && has_pos) {
                    covered = true;
                    // the plane height at the cell centre (barycentric)
                    let det = (p1[2] - p2[2]) * (p0[0] - p2[0]) + (p2[0] - p1[0]) * (p0[2] - p2[2]);
                    if det.abs() > 1e-12 {
                        let l0 = ((p1[2] - p2[2]) * (cx - p2[0]) + (p2[0] - p1[0]) * (cz - p2[2])) / det;
                        let l1 = ((p2[2] - p0[2]) * (cx - p2[0]) + (p0[0] - p2[0]) * (cz - p2[2])) / det;
                        let y = l0 * p0[1] + l1 * p1[1] + (1.0 - l0 - l1) * p2[1];
                        ymin = ymin.min(y);
                        ymax = ymax.max(y);
                    }
                    break;
                }
            }
            if !covered {
                uncovered += 1;
                if run_start.is_none() { run_start = Some(ix); }
            } else if let Some(s) = run_start.take() {
                row_runs.push((s, ix - 1));
            }
        }
        if let Some(s) = run_start.take() { row_runs.push((s, n - 1)); }
        if !row_runs.is_empty() {
            let runs: Vec<String> = row_runs.iter().map(|(s, e)| format!("x {:.0}..{:.0}", origin.0 + *s as f64 * cell, origin.0 + (*e as f64 + 1.0) * cell)).collect();
            rows.push(format!("z {:.0}..{:.0}: {}", cz - cell / 2.0, cz + cell / 2.0, runs.join(", ")));
        }
    }
    if let Some(pos) = a.iter().position(|x| x == "--line") { let zl: f64 = a[pos + 1].parse().unwrap(); let mut s = String::new(); let mut x = 0.0; while x <= map_size { let mut yy = f64::NAN; for t in &tris { let (p0, p1, p2) = (verts[t[0]], verts[t[1]], verts[t[2]]); let d1 = (x - p1[0]) * (p0[2] - p1[2]) - (p0[0] - p1[0]) * (zl - p1[2]); let d2 = (x - p2[0]) * (p1[2] - p2[2]) - (p1[0] - p2[0]) * (zl - p2[2]); let d3 = (x - p0[0]) * (p2[2] - p0[2]) - (p2[0] - p0[0]) * (zl - p0[2]); if !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0)) { let det = (p1[2] - p2[2]) * (p0[0] - p2[0]) + (p2[0] - p1[0]) * (p0[2] - p2[2]); if det.abs() > 1e-12 { let l0 = ((p1[2] - p2[2]) * (x - p2[0]) + (p2[0] - p1[0]) * (zl - p2[2])) / det; let l1 = ((p2[2] - p0[2]) * (x - p2[0]) + (p0[0] - p2[0]) * (zl - p2[2])) / det; yy = l0 * p0[1] + l1 * p1[1] + (1.0 - l0 - l1) * p2[1]; } break; } } s.push_str(&format!("x{:.0}:{:.2} ", x, yy)); x += 256.0; } println!("skirt y along z = {zl}: {s}"); }
    if let Some(pos) = a.iter().position(|x| x == "--grid-out") { let path = &a[pos + 1]; let mut out = String::new(); for iz in 0..n { let cz = origin.1 + (iz as f64 + 0.5) * cell; let mut row: Vec<String> = Vec::new(); for ix in 0..n { let cx = origin.0 + (ix as f64 + 0.5) * cell; let mut yy = f64::NAN; for t in &tris { let (p0, p1, p2) = (verts[t[0]], verts[t[1]], verts[t[2]]); let d1 = (cx - p1[0]) * (p0[2] - p1[2]) - (p0[0] - p1[0]) * (cz - p1[2]); let d2 = (cx - p2[0]) * (p1[2] - p2[2]) - (p1[0] - p2[0]) * (cz - p2[2]); let d3 = (cx - p0[0]) * (p2[2] - p0[2]) - (p2[0] - p0[0]) * (cz - p0[2]); if !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0)) { let det = (p1[2] - p2[2]) * (p0[0] - p2[0]) + (p2[0] - p1[0]) * (p0[2] - p2[2]); if det.abs() > 1e-12 { let l0 = ((p1[2] - p2[2]) * (cx - p2[0]) + (p2[0] - p1[0]) * (cz - p2[2])) / det; let l1 = ((p2[2] - p0[2]) * (cx - p2[0]) + (p0[0] - p2[0]) * (cz - p2[2])) / det; yy = l0 * p0[1] + l1 * p1[1] + (1.0 - l0 - l1) * p2[1]; } break; } } row.push(if yy.is_nan() { "nan".to_string() } else { format!("{yy:.2}") }); } out.push_str(&row.join(",")); out.push('\n'); } fs::write(path, out).expect("grid csv"); println!("wrote {path}: {n}x{n} cells (row = z index, col = x index), the covering {group} y per cell centre, nan = uncovered"); }
    println!("uncovered cells: {uncovered} of {} ({:.2} %); covering-plane y over covered cells: {:.3}..{:.3}", n * n, 100.0 * uncovered as f64 / (n * n) as f64, ymin, ymax);
    for r in rows.iter().take(80) {
        println!("  {r}");
    }
    if rows.len() > 80 {
        println!("  … {} more rows with holes", rows.len() - 80);
    }
}

/// `--line Z` prints the covering triangle's y along the row z = Z at 256-m steps (the skirt's slope under the tiles).
#[allow(dead_code)]
fn unused() {}
