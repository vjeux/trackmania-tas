//! Face-normal averages over k-rings around a position vs his N. Usage: ringavg HIS.ITEM MINE.ITEM SUBSTR X Y Z
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
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let q: [f32;3] = [a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap()];
    // his N at q
    let load_n = |path: &str| -> Option<[f32;3]> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if stem != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrm) = (Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                            _ => {}
                        }
                    }
                    for i in 0..pos.len() {
                        let d = ((pos[i][0]-q[0]).powi(2)+(pos[i][1]-q[1]).powi(2)+(pos[i][2]-q[2]).powi(2)).sqrt();
                        if d < 2e-4 { return Some(nrm[i]); }
                    }
                }
            }
        }
        None
    };
    let his = load_n(&a[1]).unwrap();
    println!("his=({:.4},{:.4},{:.4})", his[0], his[1], his[2]);
    // my tris with face normals; positions exact-bit keyed
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut trif: Vec<([u32;3],[f32;3])> = Vec::new(); // (pos keys, face normal)
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
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    let k = [ps[0][0].to_bits(), ps[0][1].to_bits(), ps[0][2].to_bits()];
                    // key by all three corners? store per-tri with corner keys
                    let _ = k;
                    trif.push(([ps[0][0].to_bits(),ps[0][1].to_bits(),ps[0][2].to_bits()], [cr[0]/l, cr[1]/l, cr[2]/l]));
                    // (store one entry per tri; ring expansion via shared positions below needs full tri keys)
                }
            }
        }
    }
    // rebuild with full tri keys
    let _ = trif;
    let mut trifs: Vec<([[u32;3];3],[f32;3])> = Vec::new();
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
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    let kk = [0,1,2].map(|j| [ps[j][0].to_bits(), ps[j][1].to_bits(), ps[j][2].to_bits()]);
                    trifs.push((kk, [cr[0]/l, cr[1]/l, cr[2]/l]));
                }
            }
        }
    }
    // seed: tris touching q (any corner within 1um)
    let mut seed: BTreeSet<usize> = BTreeSet::new();
    for (i, (kk, _)) in trifs.iter().enumerate() {
        for c in 0..3 {
            let p = [f32::from_bits(kk[c][0]), f32::from_bits(kk[c][1]), f32::from_bits(kk[c][2])];
            let d = ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt();
            if d < 1e-6 { seed.insert(i); break; }
        }
    }
    let ang = |x: [f32;3], y: [f32;3]| (x[0]*y[0]+x[1]*y[1]+x[2]*y[2]).clamp(-1.0,1.0).acos().to_degrees();
    let avg = |set: &BTreeSet<usize>| {
        let mut acc = [0.0f64; 3];
        for i in set { for d in 0..3 { acc[d] += trifs[*i].1[d] as f64; } }
        let n = set.len().max(1) as f64;
        norm([(acc[0]/n) as f32, (acc[1]/n) as f32, (acc[2]/n) as f32])
    };
    // position set of a tri set
    let posset = |set: &BTreeSet<usize>| {
        let mut s = BTreeSet::new();
        for i in set { for c in 0..3 { s.insert(trifs[*i].0[c]); } }
        s
    };
    let mut ring = seed.clone();
    for k in 0..3 {
        let a = avg(&ring);
        println!("ring{k}: ntris={} avg=({:.4},{:.4},{:.4}) d_his={:.3}deg", ring.len(), a[0], a[1], a[2], ang(his, a));
        // expand: tris sharing any position with ring
        let ps = posset(&ring);
        let mut next = ring.clone();
        for (i, (kk, _)) in trifs.iter().enumerate() {
            for c in 0..3 { if ps.contains(&kk[c]) { next.insert(i); break; } }
        }
        if next.len() == ring.len() { break; }
        ring = next;
    }
}
