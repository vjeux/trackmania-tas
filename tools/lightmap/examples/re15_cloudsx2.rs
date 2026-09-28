//! `re15_cloudsx2 Clouds.tga minR,minG,minB maxR,maxG,maxB` — the CloudsX2 modulation of the decoration terrain in the LM peel
//! (PS 16752 l.56–64): per texel c = tex.x (R8), k = lerp(min, 0.5, sat(2c)) then lerp(k, max, sat(2c − 1)); prints the mean of c,
//! the mean of k and of 2k (the ×2 of the shader), and the k histogram. The cloud field's translation is the wind's (not a
//! function of the map), so a port can only use the statistics (RE 15, NOTES 08:55Z).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 { eprintln!("usage: re15_cloudsx2 Clouds.tga minR,minG,minB maxR,maxG,maxB"); std::process::exit(2); }
    let v3 = |s: &str| -> [f32; 3] { let p: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect(); [p[0], p[1], p[2]] };
    let (mn, mx) = (v3(&a[2]), v3(&a[3]));
    let bytes = std::fs::read(&a[1]).expect("tga");
    // TGA header: 18 bytes (+ id field); image type 3 = grey, 2 = truecolour
    let id_len = bytes[0] as usize; let ty = bytes[2]; let w = u16::from_le_bytes([bytes[12], bytes[13]]) as usize; let h = u16::from_le_bytes([bytes[14], bytes[15]]) as usize; let bpp = bytes[16] as usize / 8;
    let data = &bytes[18 + id_len..];
    println!("{}: {}×{} type {ty} {bpp} B/px", a[1], w, h);
    let n = w * h;
    let (mut sc, mut sk, mut hist) = (0f64, [0f64; 3], [0u64; 10]);
    for i in 0..n {
        let c = data[i * bpp] as f32 / 255.0; // grey or the first (blue in BGR) channel — a grey map either way
        let t = (2.0 * c).clamp(0.0, 1.0); let u = (2.0 * c - 1.0).clamp(0.0, 1.0);
        let mut k = [0f32; 3];
        for ch in 0..3 { let a1 = mn[ch] + t * (0.5 - mn[ch]); k[ch] = a1 + u * (mx[ch] - a1); sk[ch] += k[ch] as f64; }
        sc += c as f64; hist[((c * 9.999) as usize).min(9)] += 1;
    }
    let m = n as f64;
    println!("mean c {:.4}; mean k ({:.4}, {:.4}, {:.4}); mean 2k ({:.4}, {:.4}, {:.4})", sc / m, sk[0] / m, sk[1] / m, sk[2] / m, 2.0 * sk[0] / m, 2.0 * sk[1] / m, 2.0 * sk[2] / m);
    println!("c histogram (10 bins): {:?}", hist.iter().map(|x| format!("{:.3}", *x as f64 / m)).collect::<Vec<_>>());
}
