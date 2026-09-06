//! Show source positions near a his-position. Usage: nearby HIS.ITEM SRCFILE hx hy hz
use mapgeom::static_item::bake::{geometry_layers, face_triangles};
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let h: Vec<f32> = vec![a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let data = std::fs::read(&a[2]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    // all crystal positions, halved
    let mut pts: Vec<[f32; 3]> = Vec::new();
    for (cr, _, _) in &layers {
        for p in &cr.positions {
            pts.push([p[0]*0.5, p[1]*0.5, p[2]*0.5]);
        }
    }
    pts.sort_by(|x, y| x[0].partial_cmp(&y[0]).unwrap());
    let mut near: Vec<([f32; 3], f32)> = Vec::new();
    for p in &pts {
        let dd = ((p[0]-h[0]).powi(2)+(p[1]-h[1]).powi(2)+(p[2]-h[2]).powi(2)).sqrt();
        if dd < 0.050 {
            near.push((*p, dd*1000.0));
        }
    }
    near.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
    println!("his=[{:.6},{:.6},{:.6}] nnear={}", h[0], h[1], h[2], near.len());
    for (p, dd) in near.iter().take(12) {
        println!("  src_half=[{:.6},{:.6},{:.6}] d={:.3}mm", p[0], p[1], p[2], dd);
    }
    let _ = face_triangles;
}
