// E2 scratch: the Stadium grass pre-pass constant 2·D_lin(0,0)·X2_raw(0.25,0.25) under every BC1 palette decode × sRGB model — the spread, in % per channel
use lightmap::texsample::{self, Bc1Decode, Sampler};
fn main() {
    let a: Vec<String> = std::env::args().collect(); // Grass_D.dds Grass_X2.dds
    let d_bytes = std::fs::read(&a[1]).unwrap(); let x_bytes = std::fs::read(&a[2]).unwrap();
    let modes = [("Ideal", Bc1Decode::Ideal), ("Expand8Trunc", Bc1Decode::Expand8Trunc), ("Expand8Round", Bc1Decode::Expand8Round)];
    let base = {
        let mut d = texsample::parse_dds(&d_bytes, Bc1Decode::Expand8Round).unwrap(); d.decode_srgb();
        let x = texsample::parse_dds(&x_bytes, Bc1Decode::Expand8Round).unwrap();
        let s = Sampler::trilinear(texsample::Address::Wrap);
        let cd = texsample::sample(&d, 0, &s, [0.0, 0.0], [0.0; 2], [0.0; 2]); let cx = texsample::sample(&x, 0, &s, [0.25, 0.25], [0.0; 2], [0.0; 2]);
        [2.0 * cd[0] * cx[0], 2.0 * cd[1] * cx[1], 2.0 * cd[2] * cx[2]]
    };
    println!("default (Expand8Round, IEC sRGB): D_lin(0,0)·X2·2 = ({:.5},{:.5},{:.5})", base[0], base[1], base[2]);
    // the four corner texels of Grass_D mip 0 as stored bytes (before sRGB), per decode
    for (dn, dm) in &modes {
        let d_raw = texsample::parse_dds(&d_bytes, *dm).unwrap();
        let s = Sampler::trilinear(texsample::Address::Wrap);
        let cd_raw = texsample::sample(&d_raw, 0, &s, [0.0, 0.0], [0.0; 2], [0.0; 2]);
        let corners: Vec<[f32; 3]> = [(0u32, 0u32), (d_raw.w - 1, 0), (0, d_raw.h - 1), (d_raw.w - 1, d_raw.h - 1)].iter().map(|&(x, y)| { let t = d_raw.levels[0][0].get(x, y); [t[0], t[1], t[2]] }).collect();
        println!("Grass_D {dn}: corner texels (stored, ×255) {:?}; bilinear(0,0) stored ({:.4},{:.4},{:.4})", corners.iter().map(|c| [(c[0] * 255.0).round(), (c[1] * 255.0).round(), (c[2] * 255.0).round()]).collect::<Vec<_>>(), cd_raw[0], cd_raw[1], cd_raw[2]);
        for (xn, xm) in &modes {
            let mut d = texsample::parse_dds(&d_bytes, *dm).unwrap(); d.decode_srgb();
            let x = texsample::parse_dds(&x_bytes, *xm).unwrap();
            let cd = texsample::sample(&d, 0, &s, [0.0, 0.0], [0.0; 2], [0.0; 2]); let cx = texsample::sample(&x, 0, &s, [0.25, 0.25], [0.0; 2], [0.0; 2]);
            let k = [2.0 * cd[0] * cx[0], 2.0 * cd[1] * cx[1], 2.0 * cd[2] * cx[2]];
            println!("  D {dn:<13} X2 {xn:<13}: D_lin ({:.5},{:.5},{:.5}) X2 ({:.5},{:.5},{:.5}) → ({:.5},{:.5},{:.5})  vs default {:+.2} % {:+.2} % {:+.2} %", cd[0], cd[1], cd[2], cx[0], cx[1], cx[2], k[0], k[1], k[2], 100.0 * (k[0] / base[0] - 1.0), 100.0 * (k[1] / base[1] - 1.0), 100.0 * (k[2] / base[2] - 1.0));
        }
    }
    // the sRGB decode: decoded-per-texel THEN bilinear (the view decodes texels before filtering) vs bilinear on bytes then decode
    let d_raw = texsample::parse_dds(&d_bytes, Bc1Decode::Expand8Round).unwrap(); let s = Sampler::trilinear(texsample::Address::Wrap);
    let cd_raw = texsample::sample(&d_raw, 0, &s, [0.0, 0.0], [0.0; 2], [0.0; 2]);
    let after = [lightmap::gpufmt::srgb_to_linear(cd_raw[0]), lightmap::gpufmt::srgb_to_linear(cd_raw[1]), lightmap::gpufmt::srgb_to_linear(cd_raw[2])];
    println!("sRGB AFTER the bilinear (the wrong order): D_lin ({:.5},{:.5},{:.5}) → constant × {:.4} {:.4} {:.4} of default", after[0], after[1], after[2], after[0] / (base[0] / 2.0 / 0.4775), after[1] / (base[1] / 2.0 / 0.5020), after[2] / (base[2] / 2.0 / 0.3088));
}
