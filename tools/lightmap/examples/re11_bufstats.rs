//! `re11_bufstats BUF.f32 [--w 2048] [--h 2048] [--c 4]` — covered texels (alpha > 0 or any rgb > 0), how many are BLACK
//! (rgb = 0 with alpha > 0), the mean rgb of the coloured ones, and a coarse histogram of the max channel (RE 11).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let w: usize = f("--w").map(|v| v.parse().unwrap()).unwrap_or(2048);
    let h: usize = f("--h").map(|v| v.parse().unwrap()).unwrap_or(2048);
    let c: usize = f("--c").map(|v| v.parse().unwrap()).unwrap_or(4);
    let b = std::fs::read(&a[1]).unwrap();
    let d: Vec<f32> = b.chunks_exact(4).map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]])).collect();
    assert_eq!(d.len(), w * h * c);
    let (mut covered, mut black, mut coloured) = (0usize, 0usize, 0usize);
    let mut sum = [0f64; 3];
    let mut hist = [0usize; 11];
    for i in 0..w * h {
        let p = &d[i * c..i * c + c];
        let alpha = if c > 3 { p[3] } else { 1.0 };
        let mx = p[0].max(p[1]).max(p[2]);
        if alpha > 0.0 || mx > 0.0 {
            covered += 1;
            if mx == 0.0 { black += 1; } else { coloured += 1; for k in 0..3 { sum[k] += p[k] as f64; } hist[((mx * 10.0).min(10.0)) as usize] += 1; }
        }
    }
    println!("{}: {covered} covered texels, {black} black (alpha > 0, rgb = 0), {coloured} coloured; mean rgb of the coloured [{:.4}, {:.4}, {:.4}]; max-channel histogram (0.1 bins) {:?}", a[1], sum[0] / coloured.max(1) as f64, sum[1] / coloured.max(1) as f64, sum[2] / coloured.max(1) as f64, hist);
}
