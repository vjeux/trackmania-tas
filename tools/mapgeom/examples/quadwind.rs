//! Quad winding distribution: are flip quads wound opposite? Usage: quadwind SRCFILE
use std::collections::BTreeSet;
use mapgeom::static_item::bake::geometry_layers;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    // Newell Y sign per quad, per material
    let mut plus: BTreeSet<(i32, usize)> = BTreeSet::new();
    let mut minus: BTreeSet<(i32, usize)> = BTreeSet::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for (fi, f) in cr.faces.iter().enumerate() {
            if f.verts.len() != 4 { continue; }
            let pts: Vec<[f64; 3]> = f.verts.iter().map(|i| {
                let p = cr.positions[*i as usize];
                [p[0] as f64, p[1] as f64, p[2] as f64]
            }).collect();
            let mut ny = 0.0;
            for i in 0..4 {
                let a2 = pts[i];
                let b = pts[(i+1)%4];
                ny += (a2[2]-b[2])*(a2[0]+b[0]);
            }
            if ny >= 0.0 { plus.insert((f.material, fi)); }
            else { minus.insert((f.material, fi)); }
        }
    }
    let mut plusm: BTreeMap2 = BTreeMap::new();
    let mut minusm: BTreeMap2 = BTreeMap::new();
    for (m, _) in &plus { *plusm.entry(*m).or_insert(0) += 1; }
    for (m, _) in &minus { *minusm.entry(*m).or_insert(0) += 1; }
    println!("plus(ny>=0) by mat: {plusm:?}");
    println!("minus(ny<0) by mat: {minusm:?}");
}
use std::collections::BTreeMap;
type BTreeMap2 = BTreeMap<i32, usize>;
