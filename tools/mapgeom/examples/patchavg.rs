//! Flood fill by dihedral<THETA from seed, avg patch faces vs his. Usage: patchavg HIS.ITEM MINE.ITEM SUBSTR X Y Z THETA
use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let q: [f32;3] = [a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap()];
    let theta: f32 = a[7].parse().unwrap();
    let cos_t = theta.to_radians().cos();
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut tris: Vec<[[f32;3];3]> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos = Vec::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 { pos = p.clone(); }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    tris.push([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
                }
            }
        }
    }
    let fnorm = |t: &[[f32;3];3]| {
        let e1 = [t[1][0]-t[0][0], t[1][1]-t[0][1], t[1][2]-t[0][2]];
        let e2 = [t[2][0]-t[0][0], t[2][1]-t[0][1], t[2][2]-t[0][2]];
        let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
        let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
        [cr[0]/l, cr[1]/l, cr[2]/l]
    };
    let fns: Vec<[f32;3]> = tris.iter().map(fnorm).collect();
    // position -> tris
    let mut p2t: BTreeMap<[u32;3], Vec<usize>> = BTreeMap::new();
    for (i, t) in tris.iter().enumerate() {
        for c in 0..3 { p2t.entry([t[c][0].to_bits(), t[c][1].to_bits(), t[c][2].to_bits()]).or_default().push(i); }
    }
    // seed tris
    let mut seed = BTreeSet::new();
    for (i, t) in tris.iter().enumerate() {
        for c in 0..3 {
            let d = ((t[c][0]-q[0]).powi(2)+(t[c][1]-q[1]).powi(2)+(t[c][2]-q[2]).powi(2)).sqrt();
            if d < 1e-6 { seed.insert(i); break; }
        }
    }
    // flood fill: BFS from seed across shared EDGE (2 shared corners), dihedral < theta vs the tri we come from
    let mut patch = seed.clone();
    let mut qd = VecDeque::new();
    for i in &seed { qd.push_back(*i); }
    // edge key
    let ekey = |a: [f32;3], b: [f32;3]| {
        let ka = [a[0].to_bits(), a[1].to_bits(), a[2].to_bits()];
        let kb = [b[0].to_bits(), b[1].to_bits(), b[2].to_bits()];
        if ka < kb { (ka, kb) } else { (kb, ka) }
    };
    let mut e2t: BTreeMap<([u32;3],[u32;3]), Vec<usize>> = BTreeMap::new();
    for (i, t) in tris.iter().enumerate() {
        for e in 0..3 {
            e2t.entry(ekey(t[e], t[(e+1)%3])).or_default().push(i);
        }
    }
    while let Some(i) = qd.pop_front() {
        for e in 0..3 {
            let k = ekey(tris[i][e], tris[i][(e+1)%3]);
            if let Some(nbrs) = e2t.get(&k) {
                for j in nbrs {
                    if patch.contains(j) { continue; }
                    if fns[i][0]*fns[*j][0]+fns[i][1]*fns[*j][1]+fns[i][2]*fns[*j][2] >= cos_t {
                        patch.insert(*j);
                        qd.push_back(*j);
                    }
                }
            }
        }
    }
    let avg = {
        let mut acc = [0.0f64; 3];
        for i in &patch { for d in 0..3 { acc[d] += fns[*i][d] as f64; } }
        norm([(acc[0]/patch.len() as f64) as f32, (acc[1]/patch.len() as f64) as f32, (acc[2]/patch.len() as f64) as f32])
    };
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut his = [0.0; 3];
    'outer: for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut p2, mut n2) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => p2 = p.clone(),
                        Elem::Word(w) if d.name() == 5 => n2 = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                for i in 0..p2.len() {
                    let d = ((p2[i][0]-q[0]).powi(2)+(p2[i][1]-q[1]).powi(2)+(p2[i][2]-q[2]).powi(2)).sqrt();
                    if d < 2e-4 { his = n2[i]; break 'outer; }
                }
            }
        }
    }
    let ang = |x: [f32;3], y: [f32;3]| (x[0]*y[0]+x[1]*y[1]+x[2]*y[2]).clamp(-1.0,1.0).acos().to_degrees();
    println!("seed={} patch={} patchavg=({:.4},{:.4},{:.4}) his=({:.4},{:.4},{:.4}) d={:.3}deg", seed.len(), patch.len(), avg[0], avg[1], avg[2], his[0], his[1], his[2], ang(his, avg));
}
