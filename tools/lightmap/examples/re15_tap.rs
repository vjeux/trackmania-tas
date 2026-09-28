//! `re15_tap FILE.dds u,v [u,v …] [--address wrap|clamp] [--flip]` — the WRAP/CLAMP bilinear mip-0 tap of a pack texture at fixed uvs
//! (zero derivatives = the pre-pass's zero-matrix constant), stored and sRGB-decoded; `--flip` samples at (u, 1 − v) (the GPU upload
//! reverses the DDS rows, RE 8). RE 15: the PyPxz constants Pxz(0, −ty) for stpad's wall materials (NOTES 05:30Z).
use lightmap::texsample::{load_dds, sample, Address, Bc1Decode, Sampler};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: re15_tap FILE.dds u,v [u,v …] [--address wrap|clamp] [--flip]"); std::process::exit(2); }
    let addr = if a.iter().any(|x| x == "clamp") { Address::Clamp } else { Address::Wrap };
    let flip = a.iter().any(|x| x == "--flip");
    let tex = load_dds(std::path::Path::new(&a[1]), Bc1Decode::Ideal).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1) });
    let s = Sampler::bilinear_no_mip(addr);
    let lin = |c: f32| -> f32 { if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) } };
    println!("{}: {:?} {}×{} mips {}; address {:?}{}", a[1], tex.fmt, tex.w, tex.h, tex.mips, addr, if flip { ", v flipped" } else { "" });
    for uv in a.iter().skip(2).filter(|x| x.contains(',')) {
        let p: Vec<f32> = uv.split(',').map(|x| x.parse().unwrap()).collect();
        let v = if flip { 1.0 - p[1] } else { p[1] };
        let c = sample(&tex, 0, &s, [p[0], v], [0.0, 0.0], [0.0, 0.0]);
        println!("  uv ({}, {}) → stored ({:.4}, {:.4}, {:.4}, a {:.4}); sRGB→linear ({:.4}, {:.4}, {:.4})", p[0], p[1], c[0], c[1], c[2], c[3], lin(c[0]), lin(c[1]), lin(c[2]));
    }
}
