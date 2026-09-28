//! v4_skirtoverlay — RE 16's skirt-height CSV (256×256 at 32 m, row = z index, col = x index, `nan` = the footprint) joined with
//! V4-11's editor tile-floor grid (32×32 at 254 m: `texeldelta --grid 32`'s "EDITOR Σ HDR … mean per chart ×1000" block) → per
//! 254-m cell the floor brightness and the mean skirt height; then the ONE-THRESHOLD test: is there a T such that
//! bright ⇔ skirt y < T? Prints the best T (fewest misfits), the misfit cells, and the brightness-by-height table.
//!
//!   cargo run --release -p lightmap --example v4_skirtoverlay -- SKIRT.csv EDITOR-GRID32.txt [--bright 150] [--cell 254]
use std::env;

fn main() {
    let a: Vec<String> = env::args().collect();
    if a.len() < 3 { eprintln!("usage: v4_skirtoverlay SKIRT.csv EDITOR-GRID32.txt [--bright B] [--cell M]"); std::process::exit(2); }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1).cloned());
    let bright_cut: f64 = f("--bright").map(|v| v.parse().unwrap()).unwrap_or(150.0);
    let cell_m: f64 = f("--cell").map(|v| v.parse().unwrap()).unwrap_or(254.0);
    // the skirt CSV
    let csv = std::fs::read_to_string(&a[1]).expect("csv");
    let skirt: Vec<Vec<f64>> = csv.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).map(|l| l.split(',').map(|v| v.trim().parse::<f64>().unwrap_or(f64::NAN)).collect()).collect();
    let (nz, nx) = (skirt.len(), skirt.iter().map(|r| r.len()).max().unwrap_or(0));
    // the editor grid
    let g = std::fs::read_to_string(&a[2]).expect("grid");
    let grid: Vec<Vec<f64>> = g.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split_whitespace().map(|v| v.parse::<f64>().unwrap_or(f64::NAN)).collect()).collect();
    let n = grid.len();
    eprintln!("skirt {nz}×{nx} at 32 m; editor grid {n}×{} at {cell_m} m", grid.first().map_or(0, |r| r.len()));
    // per grid cell: mean skirt y over the 32-m cells whose centre falls in it (skirt col j → x = 32 j + 16)
    let mut rows: Vec<(usize, usize, f64, f64, usize)> = Vec::new(); // (gz, gx, floor, mean skirt y, skirt cells)
    for gz in 0..n {
        for gx in 0..grid[gz].len() {
            let (x0, x1) = (16.0 + gx as f64 * cell_m, 16.0 + (gx + 1) as f64 * cell_m);
            let (z0, z1) = (16.0 + gz as f64 * cell_m, 16.0 + (gz + 1) as f64 * cell_m);
            let (mut s, mut k) = (0.0, 0usize);
            for (j, r) in skirt.iter().enumerate() {
                let z = 32.0 * j as f64 + 16.0;
                if z < z0 || z >= z1 { continue; }
                for (i, v) in r.iter().enumerate() {
                    let x = 32.0 * i as f64 + 16.0;
                    if x < x0 || x >= x1 || !v.is_finite() { continue; }
                    s += v; k += 1;
                }
            }
            if k > 0 { rows.push((gz, gx, grid[gz][gx], s / k as f64, k)); }
        }
    }
    eprintln!("{} outside cells with a skirt height (the footprint's nan cells drop out)", rows.len());
    // the one-threshold scan: T over the observed heights; misfits = bright with y ≥ T + dark with y < T
    let mut ys: Vec<f64> = rows.iter().map(|r| r.3).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.dedup();
    let nb = rows.iter().filter(|r| r.2 >= bright_cut).count();
    let mut best: Option<(f64, usize, usize, usize)> = None; // T, misfits, bright-above, dark-below
    for w in ys.windows(2).map(|w| 0.5 * (w[0] + w[1])).chain(std::iter::once(ys.last().copied().unwrap_or(0.0) + 0.01)) {
        let ba = rows.iter().filter(|r| r.2 >= bright_cut && r.3 >= w).count();
        let db = rows.iter().filter(|r| r.2 < bright_cut && r.3 < w).count();
        let m = ba + db;
        if best.map_or(true, |b| m < b.1) { best = Some((w, m, ba, db)); }
    }
    let (t, m, ba, db) = best.unwrap_or((0.0, 0, 0, 0));
    println!("bright cut {bright_cut} (×0.001 Σrgb): {nb} bright / {} dark outside cells", rows.len() - nb);
    println!("best single threshold: bright ⇔ skirt y < {t:.2} m — misfits {m} of {} ({} bright cells with y ≥ T, {} dark cells with y < T)", rows.len(), ba, db);
    // brightness by skirt-height band
    println!("\nskirt y band\tcells\tmean floor\tbright %\tmin floor\tmax floor");
    let bands = [(-99.0, -17.0), (-17.0, -16.0), (-16.0, -15.0), (-15.0, -14.5), (-14.5, -14.0), (-14.0, -12.0), (-12.0, -9.0), (-9.0, -6.0), (-6.0, 99.0)];
    for (lo, hi) in bands {
        let v: Vec<&(usize, usize, f64, f64, usize)> = rows.iter().filter(|r| r.3 >= lo && r.3 < hi).collect();
        if v.is_empty() { continue; }
        let mean = v.iter().map(|r| r.2).sum::<f64>() / v.len() as f64;
        let br = v.iter().filter(|r| r.2 >= bright_cut).count();
        let mn = v.iter().map(|r| r.2).fold(f64::INFINITY, f64::min);
        let mx = v.iter().map(|r| r.2).fold(f64::NEG_INFINITY, f64::max);
        println!("[{lo:>6.1}, {hi:>6.1})\t{}\t{mean:.0}\t{:.0}\t{mn:.0}\t{mx:.0}", v.len(), 100.0 * br as f64 / v.len() as f64);
    }
    println!("\nmisfit cells (gz, gx, floor, mean skirt y):");
    for r in rows.iter().filter(|r| (r.2 >= bright_cut) != (r.3 < t)) { println!("  ({}, {})\t{:.0}\t{:.2}", r.0, r.1, r.2, r.3); }
    println!("\nthe brightest 12 outside cells with their skirt height:");
    let mut byb: Vec<&(usize, usize, f64, f64, usize)> = rows.iter().collect();
    byb.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    for r in byb.iter().take(12) { println!("  ({}, {})\tfloor {:.0}\tskirt y {:.2}", r.0, r.1, r.2, r.3); }
}
