//! Are flip quads winding-consistent with neighbors? Usage: quadconsist SRCFILE
use std::collections::BTreeMap;
use mapgeom::static_item::bake::geometry_layers;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    // quad Newell + neighbor agreement (share an edge, compare Newell dots)
    // Build edge->quads map (visible quads, Technics/TSpecials mats 5,11? use all)
    let mut edge_map: BTreeMap<(u32, u32), Vec<(usize, [f64; 3])>> = BTreeMap::new();
    let mut qid = 0;
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.verts.len() != 4 { continue; }
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
            let l = (n[0]*n[0]+n[1]*n[1]+n[2]*n[2]).sqrt().max(1e-30);
            let nn = [n[0]/l, n[1]/l, n[2]/l];
            for i in 0..4 {
                let a2 = f.verts[i].min(f.verts[(i+1)%4]);
                let b = f.verts[i].max(f.verts[(i+1)%4]);
                edge_map.entry((a2, b)).or_default().push((qid, nn));
            }
            qid += 1;
        }
    }
    // quads sharing an edge with OPPOSITE Newell (inconsistent winding)
    let mut inconsist = 0;
    let mut consist = 0;
    for (_, qs) in &edge_map {
        if qs.len() < 2 { continue; }
        for x in 0..qs.len() {
            for y in (x+1)..qs.len() {
                let dot = qs[x].1[0]*qs[y].1[0]+qs[x].1[1]*qs[y].1[1]+qs[x].1[2]*qs[y].1[2];
                if dot < 0.0 { inconsist += 1; } else { consist += 1; }
            }
        }
    }
    println!("quads={qid} edge-pairs consistent={consist} inconsistent={inconsist}");
}
