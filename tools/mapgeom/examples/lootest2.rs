//! Pairs + face identification. Usage: lootest2 HIS.ITEM MINE.ITEM SUBSTR X Y Z
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
    let mut incpos = BTreeSet::new();
    for (i, (ps, _)) in all.iter().enumerate() {
        for c in 0..3 {
            let d = ((ps[c][0]-q[0]).powi(2)+(ps[c][1]-q[1]).powi(2)+(ps[c][2]-q[2]).powi(2)).sqrt();
            if d < 1e-6 { inc.push(i); break; }
        }
    }
    for i in &inc {
        for c in 0..3 { incpos.insert([all[*i].0[c][0].to_bits(), all[*i].0[c][1].to_bits(), all[*i].0[c][2].to_bits()]); }
    }
    println!("incident faces:");
    for (k, i) in inc.iter().enumerate() {
        let (ps, fn_) = &all[*i];
        println!("  inc[{k}] tri{i} fn=({:.4},{:.4},{:.4}) c0=({:.4},{:.4},{:.4})", fn_[0], fn_[1], fn_[2], ps[0][0], ps[0][1], ps[0][2]);
    }
    let mut nbr: Vec<usize> = Vec::new();
    for (i, (ps, _)) in all.iter().enumerate() {
        if inc.contains(&i) { continue; }
        for c in 0..3 {
            if incpos.contains(&[ps[c][0].to_bits(), ps[c][1].to_bits(), ps[c][2].to_bits()]) { nbr.push(i); break; }
        }
    }
    println!("neighbor tri271: fn=({:.4},{:.4},{:.4}) c=({:.4},{:.4},{:.4})({:.4},{:.4},{:.4})({:.4},{:.4},{:.4})",
        all[271].1[0], all[271].1[1], all[271].1[2], all[271].0[0][0], all[271].0[0][1], all[271].0[0][2], all[271].0[1][0], all[271].0[1][1], all[271].0[1][2], all[271].0[2][0], all[271].0[2][1], all[271].0[2][2]);
    let _ = nbr;
    let _ = BTreeMap::<u32,u32>::new();
}
