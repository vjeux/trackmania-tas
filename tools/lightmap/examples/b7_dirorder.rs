//! `b7_dirorder [--quality Q] [--sweep S] [--points FILE] [--max-elev DEG] [--match FILE [--all-sweeps]] [--list A-B]` — the game's sky-sweep directions in ISSUE
//! ORDER (dome::sweep_directions: the rotated Std.PointsInSphere set, grouped by the 9 raster jitters) with the
//! elevation of each, and the list of the LOWEST directions (|elevation| ≤ --max-elev, default 6°) with their issue
//! index — where in the compute a grazing direction sits, for a timed RenderDoc burst (baker-7, 2026-09-30; the g23
//! capture g23cf2 holds direction 0 = (0.3313, 0.9146, 0.2317), 66° — item 1(g) wants a grazing one).
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let q: u32 = f("--quality").map(|v| v.parse().unwrap()).unwrap_or(4);
    let sweep: usize = f("--sweep").map(|v| v.parse().unwrap()).unwrap_or(0);
    let max_elev: f32 = f("--max-elev").map(|v| v.parse().unwrap()).unwrap_or(6.0);
    let ps = lightmap::dome::PointSets::load(&f("--points").unwrap_or_else(lightmap::dome::default_path)).expect("point sets");
    let counts = lightmap::dome::sweep_counts(q);
    println!("quality {q}: sweeps {counts:?}");
    let dirs = lightmap::dome::sweep_directions(&ps, q, sweep, false).expect("sweep directions");
    println!("sweep {sweep}: {} directions in issue order; first {:?}", dirs.len(), dirs[0]);
    let mut low: Vec<(usize, f32, [f32; 3])> = dirs.iter().enumerate().map(|(i, d)| (i, d[1].asin().to_degrees(), *d)).filter(|(_, e, _)| e.abs() <= max_elev).collect();
    low.sort_by(|x, y| x.1.abs().partial_cmp(&y.1.abs()).unwrap());
    println!("{} directions with |elevation| <= {max_elev}° (issue index, elevation, dir):", low.len());
    for (i, e, d) in &low { println!("  issue {i:4}  elev {e:6.2}°  ({:.4}, {:.4}, {:.4})", d[0], d[1], d[2]); }
    let mut mid: Vec<(usize, f32, [f32; 3])> = dirs.iter().enumerate().map(|(i, d)| (i, d[1].asin().to_degrees(), *d)).filter(|(_, e, _)| (25.0..=35.0).contains(e)).collect();
    mid.sort_by_key(|x| x.0);
    println!("{} directions with elevation 25–35° (first 12 by issue index):", mid.len());
    for (i, e, d) in mid.iter().take(12) { println!("  issue {i:4}  elev {e:6.2}°  ({:.4}, {:.4}, {:.4})", d[0], d[1], d[2]); }
    let down = dirs.iter().filter(|d| d[1] < 0.0).count();
    println!("{down} of {} directions look down (y < 0)", dirs.len());
    // `--list A-B`: the issue indices A..=B of this sweep with their elevations (the k → direction table for a capture plan)
    if let Some(r) = f("--list") {
        let (a0, b0) = r.split_once('-').map(|(x, y)| (x.parse::<usize>().unwrap_or(0), y.parse::<usize>().unwrap_or(0))).unwrap_or((0, 0));
        for (i, d) in dirs.iter().enumerate().skip(a0).take(b0.saturating_sub(a0) + 1) {
            println!("  k{i:<5} elev {:6.2}°  ({:.4}, {:.4}, {:.4})", d[1].clamp(-1.0, 1.0).asin().to_degrees(), d[0], d[1], d[2]);
        }
    }
    // `--match FILE`: lines "label x y z" (a capture's PeelDirInW per frame) → the nearest issue index of THIS sweep (and of
    // every sweep of the quality when --all-sweeps), with the angle — the frame → k table of a captured run
    if let Some(mf) = f("--match") {
        let all = a.iter().any(|x| x == "--all-sweeps");
        let sweeps: Vec<usize> = if all { (0..counts.len()).collect() } else { vec![sweep] };
        let tables: Vec<(usize, Vec<[f32; 3]>)> = sweeps.iter().map(|&s| (s, lightmap::dome::sweep_directions(&ps, q, s, false).expect("sweep"))).collect();
        println!("matching {mf} against sweep(s) {sweeps:?}:");
        for line in std::fs::read_to_string(&mf).expect("match file").lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            if t.len() < 4 { continue; }
            let v: Vec<f32> = t[1..4].iter().map(|x| x.parse().unwrap_or(0.0)).collect();
            let mut best: Option<(usize, usize, f32)> = None;
            for (s, tbl) in &tables {
                for (i, d) in tbl.iter().enumerate() {
                    let dot = (v[0] * d[0] + v[1] * d[1] + v[2] * d[2]).clamp(-1.0, 1.0);
                    let ang = dot.acos().to_degrees();
                    if best.map_or(true, |b| ang < b.2) { best = Some((*s, i, ang)); }
                }
            }
            let (s, i, ang) = best.unwrap();
            println!("  {:>8}  sweep {s} issue k{i:<5} ({:.1}° off)  elev {:6.2}°  ({:.4}, {:.4}, {:.4})", t[0], ang, v[1].clamp(-1.0, 1.0).asin().to_degrees(), v[0], v[1], v[2]);
        }
    }
}
