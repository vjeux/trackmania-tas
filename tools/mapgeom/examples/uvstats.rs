use mapgeom::crystal::decode_template;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (mats, mesh) = decode_template(&std::fs::read(&a[1]).unwrap());
    for (mi, m) in mats.iter().enumerate() {
        let mut lo = [f32::INFINITY; 2]; let mut hi = [f32::NEG_INFINITY; 2]; let mut n = 0;
        for f in mesh.faces.iter().filter(|f| f.material as usize == mi) {
            for uv in &f.uvs { for k in 0..2 { lo[k] = lo[k].min(uv[k]); hi[k] = hi[k].max(uv[k]); } n += 1; }
        }
        println!("mat {mi} {:<48} phys {:>2} corners {:>5} u {:.3}..{:.3} v {:.3}..{:.3}", m.link, m.physics, n, lo[0], hi[0], lo[1], hi[1]);
    }
}
