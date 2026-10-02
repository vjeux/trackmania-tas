//! `e8_sprobe` — which f32 arithmetic of the scale search yields the game's probe s = 0x3f8c3476 at bisection iteration 3 on the
//! RI x2 bake copy (ours: 0x3f8c3471 from s = sqrt(mid·D), D = W·H/Σ_f32 = 0x3faf0332, Σ_f32 = 3 067 615, mid = 0.877499938)?
//! (E8, 2026-10-01)
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let sum_f32: f32 = a.get(1).and_then(|v| v.parse().ok()).unwrap_or(3067615.0);
    let sum_f64: f64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(3067612.4691908453);
    let target: u32 = a.get(3).and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(0x3f8c3476);
    let wh: f32 = 2048.0 * 2048.0;
    let d_f32 = wh / sum_f32;
    let d_f64 = (wh as f64) / sum_f64;
    let d_f64_from_f32sum = (wh as f64) / (sum_f32 as f64);
    println!("D f32 {d_f32} ({:#010x}); D f64 {d_f64} → f32 {:#010x}; D from f32 Σ in f64 {d_f64_from_f32sum} → {:#010x}", d_f32.to_bits(), (d_f64 as f32).to_bits(), (d_f64_from_f32sum as f32).to_bits());
    // the ladder: scale_hi 1.0 → ×0.9 (f32) twice (shrink iter 1 fails, iter 2 at 0.81 succeeds), then mids
    let nine: f32 = 0.9;
    let lo1 = 1.0f32 * nine; // 0.9
    let lo2 = lo1 * nine; // 0.81
    let mid2 = (lo2 + lo1) / 2.0; // 0.855
    let mid3 = (mid2 + lo1) / 2.0; // 0.8775
    println!("mids f32: lo1 {lo1} lo2 {lo2} mid2 {mid2} mid3 {mid3} ({:#010x})", mid3.to_bits());
    let mid3_f64 = ((0.81f64 + 0.9) / 2.0 + 0.9) / 2.0;
    let cands: Vec<(&str, f32)> = vec![
        ("sqrt(mid·D) f32 (ours)", (mid3 * d_f32).sqrt()),
        ("sqrt(mid)·sqrt(D) f32", mid3.sqrt() * d_f32.sqrt()),
        ("sqrt(D·mid) f32", (d_f32 * mid3).sqrt()),
        ("f64 sqrt(mid·D_f32)", ((mid3 as f64) * (d_f32 as f64)).sqrt() as f32),
        ("f64 sqrt(mid·D_f64)", ((mid3 as f64) * d_f64).sqrt() as f32),
        ("f64 sqrt(mid_f64·D_f64)", (mid3_f64 * d_f64).sqrt() as f32),
        ("f64 sqrt(mid_f64·D_f32)", (mid3_f64 * (d_f32 as f64)).sqrt() as f32),
        ("sqrt(mid·D) with D = f32(f64 W·H/Σf32)", (mid3 * (d_f64_from_f32sum as f32)).sqrt()),
        ("sqrt(mid·D) with D = f32(f64 W·H/Σf64)", (mid3 * (d_f64 as f32)).sqrt()),
        ("sqrt(mid · W·H / Σ) f32 (no D)", (mid3 * wh / sum_f32).sqrt()),
        ("sqrt((mid · W·H) / Σ) f32", ((mid3 * wh) / sum_f32).sqrt()),
        ("sqrt(W·H / (Σ / mid)) f32", (wh / (sum_f32 / mid3)).sqrt()),
        ("sqrt(mid/Σ)·2048 f32", (mid3 / sum_f32).sqrt() * 2048.0),
        ("2048·sqrt(mid/Σ) f32", 2048.0 * (mid3 / sum_f32).sqrt()),
        ("sqrt(mid·D) f32, D = 1/(Σ/W·H)", (mid3 * (1.0 / (sum_f32 / wh))).sqrt()),
        ("rsqrt-style: 1/sqrt(Σ/(mid·W·H)) f32", 1.0 / (sum_f32 / (mid3 * wh)).sqrt()),
        ("rsqrt-style: 1/sqrt(1/(mid·D)) f32", 1.0 / (1.0 / (mid3 * d_f32)).sqrt()),
    ];
    for (name, s) in &cands {
        let bits = s.to_bits();
        println!("{:<44} s {s:.8} {bits:#010x} {}", name, if bits == target { "← TARGET" } else { "" });
    }
    // which Σ_f32 values make sqrt(mid·W·H/Σ) hit the target (Σ scan ± 64 ulps)
    let mut hits = Vec::new();
    for k in -2000i32..=2000 {
        let sigma = f32::from_bits((sum_f32.to_bits() as i64 + k as i64) as u32);
        let d = wh / sigma;
        let s = (mid3 * d).sqrt();
        if s.to_bits() == target { hits.push((k, sigma)); }
    }
    println!("Σ_f32 values (ulp offsets from {sum_f32}) giving the target with ours' formula: {} hits, first/last {:?} {:?}", hits.len(), hits.first(), hits.last());
    // which mid values make it (mid scan ± 64 ulps)
    let mut mh = Vec::new();
    for k in -200i32..=200 { let m = f32::from_bits((mid3.to_bits() as i64 + k as i64) as u32); if (m * d_f32).sqrt().to_bits() == target { mh.push((k, m)); } }
    println!("mid values giving the target: {} hits, first/last {:?} {:?}", mh.len(), mh.first(), mh.last());
}
