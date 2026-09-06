//! Are flip quads wound inward in the crystal? Usage: windcheck SRCFILE
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::bake::geometry_layers;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*50.0).round() as i32, (p[1]*50.0).round() as i32, (p[2]*50.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    // all quad faces: Newell normal vs outward (center - blockcenter)
    let mut inw = 0;
    let mut tot = 0;
    let mut inw_mats: BTreeMap<i32, usize> = BTreeMap::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.verts.len() != 4 { continue; }
            tot += 1;
            let pts: Vec<[f64; 3]> = f.verts.iter().map(|i| {
                let p = cr.positions[*i as usize];
                [p[0] as f64, p[1] as f64, p[2] as f64]
            }).collect();
            let mut n = [0.0f64; 3];
            for i in 0..4 {
                let a2 = pts[i];
                let b = pts[(i+1)%4];
                n[0] += (a2[1]-b[1])*(a2[2]+b[2]);
                n[1] += (a2[2]-b[2])*(a2[0]+b[0]);
                n[2] += (a2[0]-b[0])*(a2[1]+b[1]);
            }
            let cen = [(pts[0][0]+pts[1][0]+pts[2][0]+pts[3][0])/4.0, (pts[0][1]+pts[1][1]+pts[2][1]+pts[3][1])/4.0, (pts[0][2]+pts[1][2]+pts[3][2]+pts[3][2])/4.0];
            // block center approx (16, ?, 16)? use mesh bbox center
            let out = [cen[0]-16.0, 0.0, cen[2]-16.0];
            let dot = n[0]*out[0] + n[2]*out[2];
            if dot < 0.0 {
                inw += 1;
                *inw_mats.entry(f.material).or_insert(0) += 1;
            }
            let _ = mk(&[0.0; 3]);
        }
    }
    println!("quads={tot} inward={inw} bymat={inw_mats:?}");
    let _ = BTreeSet::<u8>::new();
}
