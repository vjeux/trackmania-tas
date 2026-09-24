//! Neighbor normals + angles for faces. Usage: nbrnorms SRC.ITEM F0 F1 ...
use std::collections::BTreeMap;
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt().max(1e-30);
    [v[0]/l, v[1]/l, v[2]/l]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let want: Vec<usize> = a[2..].iter().map(|x| x.parse().unwrap()).collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    let mut faces: Vec<(Vec<[f32; 3]>, [f32; 3])> = Vec::new();
    for (cr, vis, _col) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).collect();
            let n = pts.len();
            let mut nv = [0f32; 3];
            for i in 0..n {
                let aa = pts[i];
                let b = pts[(i + 1) % n];
                nv[0] += (aa[1] - b[1]) * (aa[2] + b[2]);
                nv[1] += (aa[2] - b[2]) * (aa[0] + b[0]);
                nv[2] += (aa[0] - b[0]) * (aa[1] + b[1]);
            }
            faces.push((pts, norm(nv)));
        }
    }
    let mm = |p: &[f32; 3]| ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32);
    let mut edge2faces: BTreeMap<((i32,i32,i32),(i32,i32,i32)), Vec<usize>> = BTreeMap::new();
    for (fi, (pts, _)) in faces.iter().enumerate() {
        for i in 0..pts.len() {
            let a2 = mm(&pts[i]);
            let b2 = mm(&pts[(i + 1) % pts.len()]);
            let k = if a2 < b2 { (a2, b2) } else { (b2, a2) };
            edge2faces.entry(k).or_default().push(fi);
        }
    }
    for fi in &want {
        let n0 = faces[*fi].1;
        println!("face{fi} newell=({:.4},{:.4},{:.4}) nv={}", n0[0], n0[1], n0[2], faces[*fi].0.len());
        let mut nbrs = BTreeSet2::new();
        for i in 0..faces[*fi].0.len() {
            let a2 = mm(&faces[*fi].0[i]);
            let b2 = mm(&faces[*fi].0[(i + 1) % faces[*fi].0.len()]);
            let k = if a2 < b2 { (a2, b2) } else { (b2, a2) };
            if let Some(fs) = edge2faces.get(&k) {
                for nb in fs { if *nb != *fi { nbrs.insert(*nb); } }
            }
        }
        for nb in &nbrs {
            let n1 = faces[*nb].1;
            let d = (n0[0]*n1[0]+n0[1]*n1[1]+n0[2]*n1[2]).clamp(-1.0,1.0).acos().to_degrees();
            println!("   nbr face{nb} n=({:.4},{:.4},{:.4}) ang={:.2}deg", n1[0], n1[1], n1[2], d);
        }
    }
    use std::collections::BTreeSet;
    type BTreeSet2 = BTreeSet<usize>;
}
