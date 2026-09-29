//! `e6_atlashist FILE.f32 [--w 2048] [--ch 4] [--levels 16] [--top 40] [--rect x0,y0,w,h] [--linear]` — the colour histogram of a
//! from-map stage dump (`--chain-final-dir`: frommap-mdiffuse8.f32 = RGBA f32 2048², sRGB bytes as 0..1, alpha = coverage;
//! frommap-ilightinput.f32 = RGB f32 linear): the texels with any non-zero channel, quantised to `levels` per channel, the most
//! populous colours with their mean value (E6, 2026-09-29 — g23's hills: which pre-pass colours the items carry).
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let path = a.iter().find(|x| !x.starts_with("--") && x.ends_with(".f32")).expect("FILE.f32");
    let w: usize = f("--w").map(|v| v.parse().unwrap()).unwrap_or(2048);
    let ch: usize = f("--ch").map(|v| v.parse().unwrap()).unwrap_or(4);
    let levels: usize = f("--levels").map(|v| v.parse().unwrap()).unwrap_or(16);
    let top: usize = f("--top").map(|v| v.parse().unwrap()).unwrap_or(40);
    let rect: Option<[usize; 4]> = f("--rect").map(|v| { let p: Vec<usize> = v.split(',').map(|t| t.trim().parse().unwrap()).collect(); [p[0], p[1], p[2], p[3]] });
    let bytes = std::fs::read(path).expect("read");
    let n = bytes.len() / 4;
    let h = n / (w * ch);
    let data: Vec<f32> = bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let lin = |c: f32| -> f32 { if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) } };
    let mut hist: std::collections::HashMap<[u16; 3], (usize, [f64; 3])> = Default::default();
    let mut covered = 0usize;
    let [x0, y0, rw, rh] = rect.unwrap_or([0, 0, w, h]);
    let mut sum = [0f64; 3];
    let mut sum_lin = [0f64; 3];
    for y in y0..(y0 + rh).min(h) {
        for x in x0..(x0 + rw).min(w) {
            let i = (y * w + x) * ch;
            let px = &data[i..i + ch];
            let cov = if ch == 4 { px[3] > 0.0 } else { px[0] != 0.0 || px[1] != 0.0 || px[2] != 0.0 };
            if !cov { continue; }
            covered += 1;
            let q = [(px[0].clamp(0.0, 1.0) * levels as f32) as u16, (px[1].clamp(0.0, 1.0) * levels as f32) as u16, (px[2].clamp(0.0, 1.0) * levels as f32) as u16];
            let e = hist.entry(q).or_insert((0, [0.0; 3]));
            e.0 += 1;
            for c in 0..3 { e.1[c] += px[c] as f64; sum[c] += px[c] as f64; sum_lin[c] += lin(px[c]) as f64; }
        }
    }
    println!("{path}: {w}×{h}×{ch}; rect {:?}; covered {covered} texels; mean stored ({:.4}, {:.4}, {:.4}); mean sRGB→linear ({:.4}, {:.4}, {:.4})", [x0, y0, rw, rh], sum[0] / covered.max(1) as f64, sum[1] / covered.max(1) as f64, sum[2] / covered.max(1) as f64, sum_lin[0] / covered.max(1) as f64, sum_lin[1] / covered.max(1) as f64, sum_lin[2] / covered.max(1) as f64);
    let mut v: Vec<(&[u16; 3], &(usize, [f64; 3]))> = hist.iter().collect();
    v.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    println!("{:>9} {:>6}  mean stored rgb           G/R    B/R   (sRGB→linear rgb)", "texels", "%");
    for (_, (k, s)) in v.iter().take(top) {
        let m = [s[0] / *k as f64, s[1] / *k as f64, s[2] / *k as f64];
        let ml = [lin(m[0] as f32), lin(m[1] as f32), lin(m[2] as f32)];
        println!("{:>9} {:>6.2}  ({:.3}, {:.3}, {:.3})   {:.2}   {:.2}   ({:.4}, {:.4}, {:.4})", k, 100.0 * *k as f64 / covered.max(1) as f64, m[0], m[1], m[2], m[1] / m[0].max(1e-6), m[2] / m[0].max(1e-6), ml[0], ml[1], ml[2]);
    }
}
