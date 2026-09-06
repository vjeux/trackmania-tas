//! Find coplanar overlapping source faces (double geometry). Usage: dblface SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::geometry_layers;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    // face centroid (unscaled) -> faces
    let mut cmap: BTreeMap<(i64, i64, i64), Vec<(usize, i32, usize)>> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for (fi, f) in cr.faces.iter().enumerate() {
            let n = f.verts.len() as f64;
            let mut cen = [0.0f64; 3];
            for vi in &f.verts {
                let p = cr.positions[*vi as usize];
                cen[0] += p[0] as f64; cen[1] += p[1] as f64; cen[2] += p[2] as f64;
            }
            // 1mm cells
            let k = ((cen[0]/n*1000.0).round() as i64, (cen[1]/n*1000.0).round() as i64, (cen[2]/n*1000.0).round() as i64);
            cmap.entry(k).or_default().push((fi, f.material, f.verts.len()));
        }
    }
    let mut ndouble = 0;
    for (k, v) in &cmap {
        if v.len() > 1 {
            // check if actually overlapping (not just same cell)
            ndouble += 1;
            if ndouble <= 10 {
                println!("cell {k:?}: {v:?}");
            }
        }
    }
    println!("cells_with_2+_faces={ndouble}");
}
