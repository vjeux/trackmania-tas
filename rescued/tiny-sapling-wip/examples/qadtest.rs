//! Quad-neighborhood smoothing test at a position. Usage: qadtest SRC.ITEM HIS.ITEM SUBSTR X Y Z
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt().max(1e-30);
    [v[0]/l, v[1]/l, v[2]/l]
}
fn qw(v: [f32; 3]) -> [i32; 3] {
    [(v[0].clamp(-1.0, 1.0) * 511.0) as i32, (v[1].clamp(-1.0, 1.0) * 511.0) as i32, (v[2].clamp(-1.0, 1.0) * 511.0) as i32]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let q: [f32;3] = [a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap()];
    // source faces (visible layers): half-size positions, Newell per face
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let t = [f32::from_bits(0xbb018000), f32::from_bits(0xb7000000), f32::from_bits(0xbcbe2c00)];
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    // global face list: (corners half-size, newell)
    let mut faces: Vec<(Vec<[f32; 3]>, [f32; 3])> = Vec::new();
    for (cr, vis, _col) in &layers {
        if !vis { continue; }
        for f in &cr.faces {
            let pts: Vec<[f32; 3]> = f.verts.iter().map(|i| cr.positions[*i as usize]).map(|p| [p[0]*0.5+t[0], p[1]*0.5+t[1], p[2]*0.5+t[2]]).collect();
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
    // edge key: sorted pair of mmkeys (position-based)
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
    // adjacency: faces sharing an edge
    let mut adj: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); faces.len()];
    for (_, fs) in &edge2faces {
        for i in 0..fs.len() {
            for j in 0..fs.len() {
                if i != j { adj[fs[i]].insert(fs[j]); }
            }
        }
    }
    // faces touching q (any corner within 1um)
    let mut inc: Vec<usize> = Vec::new();
    for (fi, (pts, _)) in faces.iter().enumerate() {
        for p in pts {
            let d = ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt();
            if d < 1e-6 { inc.push(fi); break; }
        }
    }
    println!("source faces touching q: {}", inc.len());
    for fi in &inc {
        let (pts, n) = &faces[*fi];
        println!("  face{fi} nv={} newell=({:.4},{:.4},{:.4}) neighbors={:?}", pts.len(), n[0], n[1], n[2], adj[*fi]);
    }
    // smoothed value per incident face = uniform avg(self + neighbors)
    // his verts at q (words)
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut hisv: Vec<[i32; 3]> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrmw) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrmw = w.clone(),
                        _ => {}
                    }
                }
                for i in 0..pos.len() {
                    let d = ((pos[i][0]-q[0]).powi(2)+(pos[i][1]-q[1]).powi(2)+(pos[i][2]-q[2]).powi(2)).sqrt();
                    if d < 2e-4 {
                        let w = nrmw[i];
                        hisv.push([((w & 0x3FF) as i32) << 22 >> 22, ((w >> 10 & 0x3FF) as i32) << 22 >> 22, ((w >> 20 & 0x3FF) as i32) << 22 >> 22]);
                    }
                }
            }
        }
    }
    println!("his verts at q: {}", hisv.len());
    for (fi, (pts, n)) in faces.iter().enumerate() {
        if !inc.contains(&fi) { continue; }
        let mut acc = [n[0] as f64, n[1] as f64, n[2] as f64];
        let mut set = vec![fi];
        for nb in &adj[fi] { acc[0] += faces[*nb].1[0] as f64; acc[1] += faces[*nb].1[1] as f64; acc[2] += faces[*nb].1[2] as f64; set.push(*nb); }
        let nn = set.len() as f64;
        let cand = norm([(acc[0]/nn) as f32, (acc[1]/nn) as f32, (acc[2]/nn) as f32]);
        let w = qw(cand);
        let hit = hisv.iter().position(|h| *h == w).map(|i| format!("HISv{i}")).unwrap_or("none".to_string());
        println!("  V(face{fi}) nv={} words={w:?} -> {hit}", pts.len());
    }
    let _ = BTreeMap::<u32,u32>::new();
    let _ = dec;
}
