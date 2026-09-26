//! Per-mip means of a DDS inside a pak, stored bytes/255 and sRGB-decoded: `e_ddsmean --pak FILE:KEY … PATH` (E, 2026-09-26 18:50Z —
//! the grass mip question: are the DDS mips linear-correct or byte-averaged?).
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut path = String::new();
    let mut i = 0;
    while i < a.len() {
        if a[i] == "--pak" { let (p, k) = a[i + 1].rsplit_once(':').unwrap(); store.add_pak(p, k).unwrap(); i += 2; } else if a[i] == "--uv" || a[i] == "--mip" { i += 2; } else { path = a[i].clone(); i += 1; }
    }
    let bytes = store.read(&path).unwrap();
    let raw = lightmap::texsample::parse_dds(&bytes, lightmap::texsample::Bc1Decode::Expand8Round).unwrap();
    let mut lin = lightmap::texsample::parse_dds(&bytes, lightmap::texsample::Bc1Decode::Expand8Round).unwrap();
    lin.decode_srgb();
    println!("{path}: {}×{}, {} mips", raw.w, raw.h, raw.mips);
    // --uv u,v: the bilinear sample of mip 0 at (u, v) with WRAP (the pre-pass sample at a ZERO world position: uv = the translation column)
    if let Some(i) = a.iter().position(|x| x == "--uv") {
        let t: Vec<f32> = a[i + 1].split(',').map(|v| v.trim().parse().unwrap()).collect();
        let mip: usize = a.iter().position(|x| x == "--mip").map(|i| a[i + 1].parse().unwrap()).unwrap_or(0);
        for (label, tex) in [("stored/255", &raw), ("sRGB-decoded", &lin)] {
            let l0 = &tex.levels[0][mip.min(tex.levels[0].len() - 1)];
            let (w, h) = (l0.w as f32, l0.h as f32);
            let fx = t[0].rem_euclid(1.0) * w - 0.5; let fy = t[1].rem_euclid(1.0) * h - 0.5;
            let (x0, y0) = (fx.floor(), fy.floor()); let (ax, ay) = (fx - x0, fy - y0);
            let px = |x: f32, y: f32| l0.get((x.rem_euclid(w)) as u32, (y.rem_euclid(h)) as u32);
            let (c00, c10, c01, c11) = (px(x0, y0), px(x0 + 1.0, y0), px(x0, y0 + 1.0), px(x0 + 1.0, y0 + 1.0));
            let mut o = [0f32; 4];
            for c in 0..4 { o[c] = (c00[c] * (1.0 - ax) + c10[c] * ax) * (1.0 - ay) + (c01[c] * (1.0 - ax) + c11[c] * ax) * ay; }
            println!("  mip {mip} bilinear at uv ({}, {}) {label}: ({:.5}, {:.5}, {:.5}, a {:.4})", t[0], t[1], o[0], o[1], o[2], o[3]);
        }
        return;
    }
    for l in 0..raw.levels[0].len() {
        let (lr, ll) = (&raw.levels[0][l], &lin.levels[0][l]);
        let n = (lr.w * lr.h) as f64;
        let mut sr = [0.0f64; 3]; let mut sl = [0.0f64; 3];
        for y in 0..lr.h { for x in 0..lr.w { let a = lr.get(x, y); let b = ll.get(x, y); for c in 0..3 { sr[c] += a[c] as f64; sl[c] += b[c] as f64; } } }
        println!("  mip {l:2} {:4}×{:<4} stored/255 ({:.4}, {:.4}, {:.4})  sRGB-decoded ({:.4}, {:.4}, {:.4})", lr.w, lr.h, sr[0] / n, sr[1] / n, sr[2] / n, sl[0] / n, sl[1] / n, sl[2] / n);
    }
}
