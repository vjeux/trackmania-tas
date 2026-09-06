//! Max welded dihedral vs min split dihedral (within (pos,uv,uv1) groups).
//! Usage: weldsplitbounds HIS.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut max_weld = 0.0f32;
    let mut min_split = 180.0f32;
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrm, mut uv, mut uv1) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                let mut has_uv1 = false;
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Float2(u) if d.name() == 11 => { uv1 = u.clone(); has_uv1 = true; }
                        _ => {}
                    }
                }
                if pos.len() != nrm.len() || pos.len() != uv.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                // per tri face normal
                let mut trin: Vec<[f32; 3]> = Vec::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { trin.push([0.0; 3]); continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    trin.push([cr[0]/l, cr[1]/l, cr[2]/l]);
                }
                // group by (pos,uv,uv1)
                let mut bygrp: BTreeMap<Vec<u32>, Vec<(usize, [f32; 3])>> = BTreeMap::new();
                for (ti, t) in idx.chunks(3).enumerate() {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        let vi2 = t[k] as usize;
                        let mut key = vec![pos[vi2][0].to_bits(), pos[vi2][1].to_bits(), pos[vi2][2].to_bits(),
                                           uv[vi2][0].to_bits(), uv[vi2][1].to_bits()];
                        if has_uv1 && vi2 < uv1.len() {
                            key.push(uv1[vi2][0].to_bits());
                            key.push(uv1[vi2][1].to_bits());
                        }
                        bygrp.entry(key).or_default().push((ti, nrm[vi2]));
                    }
                }
                for (_, corners) in &bygrp {
                    if corners.len() < 2 { continue; }
                    // stored distinct?
                    let mut dn = 0;
                    for (i, (_, n)) in corners.iter().enumerate() {
                        if corners[..i].iter().all(|(_, x)| (x[0]-n[0]).abs() > 1e-7 || (x[1]-n[1]).abs() > 1e-7 || (x[2]-n[2]).abs() > 1e-7) {
                            dn += 1;
                        }
                    }
                    // max face dihedral in group
                    let mut maxdih = 0.0f32;
                    for x in 0..corners.len() {
                        for y in (x+1)..corners.len() {
                            let n0 = trin[corners[x].0];
                            let n1 = trin[corners[y].0];
                            if n0 == [0.0; 3] || n1 == [0.0; 3] { continue; }
                            let d = (n0[0]*n1[0]+n0[1]*n1[1]+n0[2]*n1[2]).clamp(-1.0, 1.0).acos().to_degrees();
                            maxdih = maxdih.max(d);
                        }
                    }
                    if dn == 1 {
                        max_weld = max_weld.max(maxdih);
                    } else {
                        min_split = min_split.min(maxdih);
                    }
                }
            }
        }
    }
    println!("max welded dihedral={max_weld:.2} min split dihedral={min_split:.2}");
}
