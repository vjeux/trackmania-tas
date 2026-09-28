// E2 scratch: a captured R11G11B10 peel-colour plane's colour histogram (quantised to 1/64) — what the game's world peel layer 0 holds
// where it is not the dome (the sea floor / terrain colours), top 16 colours by count
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = lightmap::texsample::load_dds(std::path::Path::new(&a[1]), lightmap::texsample::Bc1Decode::Ideal).expect("dds");
    let lv = &t.levels[0][0];
    let mut h: std::collections::HashMap<(i32, i32, i32), u64> = std::collections::HashMap::new();
    for y in 0..lv.h { for x in 0..lv.w { let p = lv.get(x, y); let k = ((p[0] * 64.0).round() as i32, (p[1] * 64.0).round() as i32, (p[2] * 64.0).round() as i32); *h.entry(k).or_default() += 1; } }
    let mut v: Vec<_> = h.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    println!("{}×{} {:?}: top colours (r,g,b in 1/64 steps → value): ", lv.w, lv.h, t.fmt);
    for (k, n) in v.iter().take(16) { println!("  ({:.3},{:.3},{:.3}) × {n}", k.0 as f32 / 64.0, k.1 as f32 / 64.0, k.2 as f32 / 64.0); }
}
