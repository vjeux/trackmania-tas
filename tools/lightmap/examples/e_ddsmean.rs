//! Per-mip means of a DDS inside a pak, stored bytes/255 and sRGB-decoded: `e_ddsmean --pak FILE:KEY … PATH` (E, 2026-09-26 18:50Z —
//! the grass mip question: are the DDS mips linear-correct or byte-averaged?).
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut path = String::new();
    let mut i = 0;
    while i < a.len() {
        if a[i] == "--pak" { let (p, k) = a[i + 1].rsplit_once(':').unwrap(); store.add_pak(p, k).unwrap(); i += 2; } else { path = a[i].clone(); i += 1; }
    }
    let bytes = store.read(&path).unwrap();
    let raw = lightmap::texsample::parse_dds(&bytes, lightmap::texsample::Bc1Decode::Expand8Round).unwrap();
    let mut lin = lightmap::texsample::parse_dds(&bytes, lightmap::texsample::Bc1Decode::Expand8Round).unwrap();
    lin.decode_srgb();
    println!("{path}: {}×{}, {} mips", raw.w, raw.h, raw.mips);
    for l in 0..raw.levels[0].len() {
        let (lr, ll) = (&raw.levels[0][l], &lin.levels[0][l]);
        let n = (lr.w * lr.h) as f64;
        let mut sr = [0.0f64; 3]; let mut sl = [0.0f64; 3];
        for y in 0..lr.h { for x in 0..lr.w { let a = lr.get(x, y); let b = ll.get(x, y); for c in 0..3 { sr[c] += a[c] as f64; sl[c] += b[c] as f64; } } }
        println!("  mip {l:2} {:4}×{:<4} stored/255 ({:.4}, {:.4}, {:.4})  sRGB-decoded ({:.4}, {:.4}, {:.4})", lr.w, lr.h, sr[0] / n, sr[1] / n, sr[2] / n, sl[0] / n, sl[1] / n, sl[2] / n);
    }
}
