//! `re17_grazing [Q]` — the sweep-0 direction set in the game's ISSUE ORDER (dome::sweep_directions, RE child 5's order) with each
//! direction's elevation, and the indices of the most grazing ones (|D.y| smallest) — for baker-7's capture window: with two frames
//! per direction (env block + item layers/accumulate) the k-th issued direction's frames sit at first_compute_frame + 2k (env) and + 2k + 1.
//! RE 17 2026-09-30 10:55Z (g23: frame 557 = direction 0's env block, 558 its layers, 559 direction 1's env block).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let q: u32 = a.get(1).map(|s| s.parse().unwrap()).unwrap_or(4);
    let ps = lightmap::dome::PointSets::load(&lightmap::dome::default_path()).expect("point sets");
    let dirs = lightmap::dome::sweep_directions(&ps, q, 0, false).expect("sweep 0");
    println!("quality {q}: {} directions in issue order", dirs.len());
    let mut rows: Vec<(usize, [f32; 3], f32)> = dirs.iter().enumerate().map(|(i, d)| (i, *d, d[1].asin().to_degrees())).collect();
    for (i, d, el) in rows.iter().take(4) { println!("  k {i:3}: D ({:+.4}, {:+.4}, {:+.4}) el {el:5.1}°", d[0], d[1], d[2]); }
    rows.sort_by(|x, y| x.2.abs().partial_cmp(&y.2.abs()).unwrap());
    println!("the 12 most grazing (|el| smallest):");
    for (i, d, el) in rows.iter().take(12) { println!("  k {i:3}: D ({:+.4}, {:+.4}, {:+.4}) el {el:5.2}°  → frames first+{} (env) / first+{} (layers)", d[0], d[1], d[2], 2 * i, 2 * i + 1); }
    let mut low: Vec<&(usize, [f32; 3], f32)> = rows.iter().filter(|r| r.2 >= 3.0 && r.2 <= 8.0).collect();
    low.sort_by_key(|r| r.0);
    println!("directions with elevation 3–8° (the low band), earliest first:");
    for (i, d, el) in low.iter().take(6) { println!("  k {i:3}: D ({:+.4}, {:+.4}, {:+.4}) el {el:5.2}°  → frames first+{} / first+{}", d[0], d[1], d[2], 2 * i, 2 * i + 1); }
    let mut mid: Vec<&(usize, [f32; 3], f32)> = rows.iter().filter(|r| r.2 >= 25.0 && r.2 <= 35.0).collect();
    mid.sort_by_key(|r| r.0);
    println!("directions with elevation 25–35° (the sun's band), earliest first:");
    for (i, d, el) in mid.iter().take(6) { println!("  k {i:3}: D ({:+.4}, {:+.4}, {:+.4}) el {el:5.2}°  → frames first+{} / first+{}", d[0], d[1], d[2], 2 * i, 2 * i + 1); }
}
// (appended) the 3–8° band, earliest first — the low band a vertical face sees, before the zero-elevation ones
#[allow(dead_code)]
fn unused() {}
