//! Quad aspect ratio distribution. Usage: quadar SRCFILE
use mapgeom::static_item::bake::geometry_layers;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = geometry_layers(&c);
    let mut ars: Vec<f32> = Vec::new();
    for (cr, vis, _) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            if f.verts.len() != 4 { continue; }
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            // edge lengths
            let mut es = Vec::new();
            for i in 0..4 {
                let a2 = pts[i];
                let b = pts[(i+1)%4];
                es.push(((a2[0]-b[0]).powi(2)+(a2[1]-b[1]).powi(2)+(a2[2]-b[2]).powi(2)).sqrt());
            }
            es.sort_by(|x, y| x.partial_cmp(y).unwrap());
            if es[0] > 1e-9 {
                ars.push(es[3]/es[0]);
            }
        }
    }
    ars.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!("quads={} ar p50={:.1} p90={:.1} p99={:.1} max={:.1} count_ar_gt7={} gt14={}",
        ars.len(), ars[ars.len()/2], ars[ars.len()*9/10], ars[ars.len()*99/100], ars[ars.len()-1],
        ars.iter().filter(|x| **x > 7.0).count(), ars.iter().filter(|x| **x > 14.0).count());
}
