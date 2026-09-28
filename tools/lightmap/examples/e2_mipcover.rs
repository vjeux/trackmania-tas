// E2: per-mip alpha coverage of a card cut-out (the fraction of texels with alpha ≥ 128/255, and the mean alpha) — boosted mips
// keep the coverage up at coarse levels; unboosted ones fall toward the leaf density
fn main() {
    for p in std::env::args().skip(1) {
        let bytes = std::fs::read(&p).unwrap();
        match lightmap::shadowmap::AlphaTexture::from_dds(&bytes) {
            Err(e) => println!("{p}: {e}"),
            Ok(t) => {
                let name = p.rsplit('/').next().unwrap();
                print!("{name} {}×{} {} mips: coverage(≥128/255) / mean alpha per mip:", t.w, t.h, t.mips.len());
                for (w, h, a) in &t.mips {
                    let n = (*w as usize) * (*h as usize);
                    let cov = a.iter().filter(|&&v| v >= 128.0 / 255.0).count() as f32 / n.max(1) as f32;
                    let mean = a.iter().sum::<f32>() / n.max(1) as f32;
                    print!("  {w}²: {:.3}/{:.3}", cov, mean);
                }
                println!();
            }
        }
    }
}
