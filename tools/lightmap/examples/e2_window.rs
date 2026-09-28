// E2 scratch: a window of one raw sweep plane — alpha and own-mean (max channel) per texel
use lightmap::passdiff::load_file;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (dir, x0, y0, r): (&str, i64, i64, i64) = (&a[1], a[2].parse().unwrap(), a[3].parse().unwrap(), a.get(4).map(|s| s.parse().unwrap()).unwrap_or(3));
    for s in 0..8 {
        let f = format!("sweep{s}_mrt0.f32");
        let p = std::path::Path::new(dir).join(&f);
        if !p.exists() { break; }
        let n = std::fs::metadata(&p).unwrap().len() / 16; let side = (n as f64).sqrt() as u32;
        let b = load_file(std::path::Path::new(dir), &f, "R32G32B32A32_FLOAT", side, side, 0).unwrap();
        println!("sweep {s}: alpha×128 / own-mean G (rgb/alpha) — rows {}..{} cols {}..{}", y0 - r, y0 + r, x0 - r, x0 + r);
        for y in (y0 - r)..=(y0 + r) {
            let mut l1 = format!("{y:5}:"); let mut l2 = String::from("      ");
            for x in (x0 - r)..=(x0 + r) {
                let al = b.get(x as u32, y as u32, 3); let g = b.get(x as u32, y as u32, 1);
                l1 += &format!("{:6.0}", al * 128.0);
                l2 += &format!("{:6.2}", if al > 0.0 { g / al } else { 0.0 });
            }
            println!("{l1}\n{l2}");
        }
    }
}
