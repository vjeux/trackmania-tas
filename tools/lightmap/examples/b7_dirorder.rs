//! `b7_dirorder [--quality Q] [--sweep S] [--points FILE] [--max-elev DEG]` — the game's sky-sweep directions in ISSUE
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
}
