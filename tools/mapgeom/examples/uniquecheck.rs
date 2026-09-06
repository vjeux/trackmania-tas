//! Count subsets fitting each his vert <0.3deg. Usage: uniquecheck HIS.ITEM MINE.ITEM SUBSTR X Y Z
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
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut all: Vec<([[f32;3];3],[f32;3])> = Vec::new();
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
                    all.push((ps, [cr[0]/l, cr[1]/l, cr[2]/l]));
                }
            }
        }
    }
    let mut inc: Vec<usize> = Vec::new();
    for (i, (ps, _)) in all.iter().enumerate() {
        for c in 0..3 {
            let d = ((ps[c][0]-q[0]).powi(2)+(ps[c][1]-q[1]).powi(2)+(ps[c][2]-q[2]).powi(2)).sqrt();
            if d < 1e-6 { inc.push(i); break; }
        }
    }
    let tang = |x: [f32;3], y: [f32;3]| {
        let la = (x[0]*x[0]+x[1]*x[1]+x[2]*x[2]).sqrt().max(1e-30);
        let lb = (y[0]*y[0]+y[1]*y[1]+y[2]*y[2]).sqrt().max(1e-30);
        ((x[0]*y[0]+x[1]*y[1]+x[2]*y[2])/(la*lb)).clamp(-1.0,1.0).acos().to_degrees()
    };
    let avg = |set: &[usize]| {
        let mut acc = [0.0f64; 3];
        for i in set { for d in 0..3 { acc[d] += all[*i].1[d] as f64; } }
        let n = set.len().max(1) as f64;
        norm([(acc[0]/n) as f32, (acc[1]/n) as f32, (acc[2]/n) as f32])
    };
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut hisv: Vec<[f32;3]> = Vec::new();
    for g in &s2.shaded_geoms {
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
                    if d < 2e-4 { hisv.push(n2[i]); }
                }
            }
        }
    }
    // exhaustive over incident subsets (2^5)
    for (vi, hv) in hisv.iter().enumerate() {
        let mut fits: Vec<(Vec<usize>, f32)> = Vec::new();
        for mask in 1..(1u32 << inc.len()) {
            let set: Vec<usize> = inc.iter().enumerate().filter(|(k, _)| mask & (1 << k) != 0).map(|(_, v)| *v).collect();
            let d = tang(*hv, avg(&set));
            if d < 0.3 { fits.push((set, d)); }
        }
        println!("his vert{vi}: {} incident-subsets fit <0.3deg", fits.len());
        fits.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
        for (s, d) in fits.iter().take(5) {
            println!("   d={d:.3} set={s:?}");
        }
    }
    let _ = BTreeMap::<u32,u32>::new();
    let _ = BTreeSet::<u32>::new();
}
