//! `e7_texprobe FILE.dds[.gz] [--fmt DXGI_NAME] [--rect x0,y0,x1,y1] x,y …` — pixel values of a captured texture export (RGBA16F /
//! R11G11B10 / R16 / R32 …) at the given pixels, and the mean + non-zero count over a rect (E7, 2026-09-30: the game's direct-sun
//! atlas at a hill's LM texels).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 2 { eprintln!("usage: e7_texprobe FILE.dds[.gz] [--fmt F] [--rect x0,y0,x1,y1] x,y …"); std::process::exit(2); }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let p = &a[1];
    let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("{p}: {e}"));
    let bytes = if p.ends_with(".gz") { lightmap::passdiff::gunzip(&bytes).expect("gunzip") } else { bytes };
    // a DDS export, or a raw plane: --fmt names the format (R32_FLOAT, R11G11B10_FLOAT, …) with --size W,H (default 2048²; a raw f32 plane
    // without --fmt takes its channel count from the byte length)
    let buf = if bytes.len() >= 4 && &bytes[..4] == b"DDS " { lightmap::passdiff::load_dds_bytes(&bytes, &f("--fmt").unwrap_or_default(), 0, 0).unwrap_or_else(|e| panic!("{p}: {e}")) } else {
        let (w, h) = f("--size").map(|s| { let v: Vec<u32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect(); (v[0], v[1]) }).unwrap_or((2048, 2048));
        let fmt = f("--fmt").unwrap_or_else(|| match (bytes.len() / 4) / (w as usize * h as usize) { 1 => "R32_FLOAT".into(), 3 => "R32G32B32_FLOAT".into(), _ => "R32G32B32A32_FLOAT".into() });
        lightmap::passdiff::decode_raw(&bytes, lightmap::passdiff::parse_format(&fmt), w, h, 0).unwrap_or_else(|e| panic!("{p}: {e}"))
    };
    eprintln!("{p}: {}×{} × {} channels", buf.w, buf.h, buf.channels);
    let px = |x: u32, y: u32| -> Vec<f32> { (0..buf.channels).map(|c| buf.get(x, y, c)).collect() };
    for arg in a.iter().skip(2).filter(|s| s.contains(',') && !s.starts_with("--")) {
        let v: Vec<u32> = arg.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        if v.len() == 2 { println!("({}, {}) = {:?}", v[0], v[1], px(v[0], v[1])); }
    }
    if let Some(r) = f("--rect") {
        let v: Vec<u32> = r.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        if v.len() == 4 {
            let mut sum = vec![0f64; buf.channels as usize];
            let (mut n, mut nz) = (0usize, 0usize);
            for y in v[1]..v[3].min(buf.h) { for x in v[0]..v[2].min(buf.w) { let q = px(x, y); n += 1; if q.iter().take(3).any(|c| *c != 0.0) { nz += 1; } for (c, s) in q.iter().zip(sum.iter_mut()) { *s += *c as f64; } } }
            println!("rect [{},{})×[{},{}): {n} pixels, {nz} non-zero (rgb); mean {:?}", v[0], v[2], v[1], v[3], sum.iter().map(|s| (*s / n.max(1) as f64) as f32).collect::<Vec<_>>());
            for y in v[1]..v[3].min(buf.h) { let row: Vec<String> = (v[0]..v[2].min(buf.w)).map(|x| { let q = px(x, y); format!("{:.2}/{:.2}/{:.2}", q[0], q[1], q[2]) }).collect(); println!("  y {y}: {}", row.join(" ")); }
        }
    }
}
