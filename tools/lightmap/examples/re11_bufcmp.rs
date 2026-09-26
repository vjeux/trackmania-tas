//! `re11_bufcmp A.f32 B.f32 [--w 2048] [--h 2048] [--c 4] [--png OUT.png]` — two dumped f32 buffers (the from-map setup's
//! `--chain-final-dir` files: frommap-attr.f32 RGBA16F values, frommap-mdiffuse8.f32 UNORM8 values, frommap-ilightinput.f32
//! R11G11B10), texel by texel: how many differ, the per-channel mean ratio B/A over the differing texels, a few samples, and
//! (--png) a mask image (white = differs, grey = covered in both, black = empty). RE 11's check that the water tint engages on
//! exactly the submerged charts of a Stadium bake.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let w: usize = f("--w").map(|v| v.parse().unwrap()).unwrap_or(2048);
    let h: usize = f("--h").map(|v| v.parse().unwrap()).unwrap_or(2048);
    let c: usize = f("--c").map(|v| v.parse().unwrap()).unwrap_or(4);
    let read = |p: &str| -> Vec<f32> { let b = std::fs::read(p).unwrap_or_else(|e| panic!("{p}: {e}")); b.chunks_exact(4).map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]])).collect() };
    let (ba, bb) = (read(&a[1]), read(&a[2]));
    assert_eq!(ba.len(), w * h * c, "{}: {} floats ≠ {w}×{h}×{c}", a[1], ba.len());
    assert_eq!(bb.len(), ba.len());
    let mut n_diff = 0usize;
    let mut n_cov = 0usize;
    let mut sum_ratio = vec![0f64; c];
    let mut n_ratio = vec![0usize; c];
    let mut mask = vec![0u8; w * h * 3];
    let mut shown = 0;
    let (mut lo, mut hi) = ([usize::MAX; 2], [0usize; 2]);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * c;
            let pa = &ba[i..i + c];
            let pb = &bb[i..i + c];
            let covered = pa.iter().any(|v| *v != 0.0) || pb.iter().any(|v| *v != 0.0);
            let differs = pa != pb;
            if covered { n_cov += 1; mask[(y * w + x) * 3] = 64; mask[(y * w + x) * 3 + 1] = 64; mask[(y * w + x) * 3 + 2] = 64; }
            if differs {
                n_diff += 1;
                lo = [lo[0].min(x), lo[1].min(y)];
                hi = [hi[0].max(x), hi[1].max(y)];
                for k in 0..c { if pa[k] > 1e-6 { sum_ratio[k] += (pb[k] / pa[k]) as f64; n_ratio[k] += 1; } }
                mask[(y * w + x) * 3] = 255; mask[(y * w + x) * 3 + 1] = 255; mask[(y * w + x) * 3 + 2] = 255;
                if shown < 8 && (x + y) % 97 == 0 { println!("  ({x}, {y}): A {:?} → B {:?}", pa, pb); shown += 1; }
            }
        }
    }
    println!("{} vs {}: {n_cov} covered texels, {n_diff} differ; bbox x {}..{} y {}..{}", a[1], a[2], lo[0], hi[0], lo[1], hi[1]);
    for k in 0..c { if n_ratio[k] > 0 { println!("  channel {k}: mean B/A over the differing texels {:.4} ({} texels)", sum_ratio[k] / n_ratio[k] as f64, n_ratio[k]); } }
    if let Some(p) = f("--png") { lightmap::png::write_rgb(&p, w as u32, h as u32, &mask).unwrap(); println!("  wrote {p}"); }
}
