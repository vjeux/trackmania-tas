//! `e3_nearfir FIRS.txt ITEMS.txt [--radius R]` — the items (rows `idx id x y z`) whose placement lies within R metres (xz) of any
//! fir placement (rows `x y z`), with the distance to the nearest fir and its position. E3 2026-09-28 (the tiny03 card cell's
//! near-set: V4's AC03234041 within 8 m of the AV03234* firs).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let r: f32 = f("--radius").map(|s| s.parse().expect("R")).unwrap_or(8.0);
    let firs: Vec<[f32; 3]> = std::fs::read_to_string(&a[1]).expect("firs").lines().filter_map(|l| { let v: Vec<f32> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect(); (v.len() >= 3).then(|| [v[0], v[1], v[2]]) }).collect();
    for l in std::fs::read_to_string(&a[2]).expect("items").lines() {
        let cols: Vec<&str> = l.split_whitespace().collect();
        if cols.len() < 5 { continue; }
        let p: Vec<f32> = cols[2..5].iter().filter_map(|x| x.parse().ok()).collect();
        if p.len() < 3 { continue; }
        let mut best: Option<(f32, [f32; 3])> = None;
        for fp in &firs {
            let d = ((fp[0] - p[0]).powi(2) + (fp[2] - p[2]).powi(2)).sqrt();
            if best.map_or(true, |(b, _)| d < b) { best = Some((d, *fp)); }
        }
        if let Some((d, fp)) = best {
            if d <= r { println!("{} {} at ({:.1},{:.1},{:.1}) nearest fir {:.2} m at ({:.1},{:.1},{:.1})", cols[0], cols[1], p[0], p[1], p[2], d, fp[0], fp[1], fp[2]); }
        }
    }
}
