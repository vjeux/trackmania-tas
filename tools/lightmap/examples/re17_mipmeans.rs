//! `re17_mipmeans FILE.dds…` — per mip level: size, mean linear rgb over ALL texels, over alpha ≥ 0.5 texels (count), and the raw
//! (non-linearised) mean — the streaming-stub question of E7's 18:37Z (which 16×16 the LM pre-pass sampled for the fir cards).
//! RE 17 2026-09-30 18:40Z.
fn main() {
    for p in std::env::args().skip(1) {
        let bytes = match std::fs::read(&p) { Ok(b) => b, Err(e) => { println!("{p}: {e}"); continue; } };
        let t = match lightmap::texsample::parse_dds(&bytes, lightmap::texsample::Bc1Decode::Ideal) { Ok(t) => t, Err(e) => { println!("{p}: {e}"); continue; } };
        println!("{}: {:?} {}×{} {} mips {} slices complete {}", p.rsplit('/').next().unwrap(), t.fmt, t.w, t.h, t.mips, t.slices, t.complete);
        for (mi, lvl) in t.levels[0].iter().enumerate() {
            let tx = lvl.texels();
            let n = tx.len().max(1) as f64;
            let mut s = [0f64; 4]; let mut sp = [0f64; 3]; let mut np = 0usize;
            for c in &tx { for k in 0..4 { s[k] += c[k] as f64; } if c[3] >= 0.5 { np += 1; for k in 0..3 { sp[k] += c[k] as f64; } } }
            println!("  mip {mi:2} {:4}×{:<4} mean lin ({:.4}, {:.4}, {:.4}) alpha {:.3}; alpha≥0.5: {np:5} texels ({:.1} %) mean ({:.4}, {:.4}, {:.4})", lvl.w, lvl.h, s[0] / n, s[1] / n, s[2] / n, s[3] / n, 100.0 * np as f64 / n, sp[0] / np.max(1) as f64, sp[1] / np.max(1) as f64, sp[2] / np.max(1) as f64);
        }
    }
}
