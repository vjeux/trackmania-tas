//! Source verts near a given unscaled position. Usage: srcnear SRCFILE x y z
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let q: Vec<f32> = vec![a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap()];
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    let mut near: Vec<([f32; 3], f32)> = Vec::new();
    for (cr, _, _) in &layers {
        for p in &cr.positions {
            let dd = ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt();
            if dd < 0.002 {
                near.push((*p, dd*1000.0));
            }
        }
    }
    near.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
    let mut seen = std::collections::BTreeSet::new();
    println!("nnear={}", near.len());
    for (p, dd) in &near {
        if seen.insert([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]) {
            println!("  src=[{:.7},{:.7},{:.7}] d={:.2}um", p[0], p[1], p[2], dd*1000.0);
        }
        if seen.len() >= 10 { break; }
    }
}
