//! Dump crystal face triangles (unscaled source positions + uvs). Usage: srcpos SRCFILE
use mapgeom::static_item::bake::{geometry_layers, face_triangles};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let mut n = 0;
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            for t in face_triangles(cr, f, 1.0) {
                n += 1;
                let _ = t;
            }
        }
    }
    println!("faces tris={n}");
}
