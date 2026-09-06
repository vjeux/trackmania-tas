//! Sliver U rule test: longest-edge-projected vs his U. Usage: sliverule HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
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
    // my corners: pos, smoothed N, tri geometry + du/dv (for det + du-grad hemisphere)
    struct C { p: [f32;3], n: [f32;3], ps: [[f32;3];3], du1: f32, dv1: f32, du2: f32, dv2: f32, e1: [f32;3], e2: [f32;3] }
    let mut mine: Vec<C> = Vec::new();
    let data = std::fs::read(&a[2]).unwrap();
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
                let (mut pos, mut nrm, mut uv) = (Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        _ => {}
                    }
                }
                if pos.len() != nrm.len() || pos.len() != uv.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let us = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    let ns = [nrm[t[0] as usize], nrm[t[1] as usize], nrm[t[2] as usize]];
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let (du1, dv1, du2, dv2) = (us[1][0]-us[0][0], us[1][1]-us[0][1], us[2][0]-us[0][0], us[2][1]-us[0][1]);
                    for k in 0..3 {
                        mine.push(C { p: ps[k], n: ns[k], ps, du1, dv1, du2, dv2, e1, e2 });
                    }
                }
            }
        }
    }
    let mut his: Vec<([f32;3],[f32;3])> = Vec::new();
    let data = std::fs::read(&a[1]).unwrap();
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
                let (mut pos, mut tu) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                for i in 0..pos.len() {
                    if i < tu.len() { his.push((pos[i], tu[i])); }
                }
            }
        }
    }
    let mut grid: BTreeMap<(i64,i64,i64), Vec<usize>> = BTreeMap::new();
    for (j, (p, _)) in his.iter().enumerate() {
        grid.entry(((p[0]*5000.0).floor() as i64, (p[1]*5000.0).floor() as i64, (p[2]*5000.0).floor() as i64)).or_default().push(j);
    }
    // sliver corners only (|det| in [1e-12, 1e-5])
    let (mut n, mut par, mut dotsum) = (0, 0, 0.0f64);
    let (mut sdet_pos, mut sdet_neg) = (0, 0); // sign(dot(edge U, his U)) vs sign(det)
    for c in &mine {
        let sdet = c.du1*c.dv2-c.du2*c.dv1;
        let det = sdet.abs();
        if det < 1e-12 || det > 1e-5 { continue; }
        let cc = ((c.p[0]*5000.0).floor() as i64, (c.p[1]*5000.0).floor() as i64, (c.p[2]*5000.0).floor() as i64);
        let mut best = (1e9f32, usize::MAX);
        for dx in -1..=1 { for dy in -1..=1 { for dz in -1..=1 {
            if let Some(js) = grid.get(&(cc.0+dx, cc.1+dy, cc.2+dz)) {
                for &j in js {
                    let d = ((c.p[0]-his[j].0[0]).powi(2)+(c.p[1]-his[j].0[1]).powi(2)+(c.p[2]-his[j].0[2]).powi(2)).sqrt();
                    if d < best.0 { best = (d, j); }
                }
            }
        }}}
        if best.0 > 2e-4 || best.1 == usize::MAX { continue; }
        let hu = his[best.1].1;
        // longest edge (3D)
        let e3 = [c.ps[2][0]-c.ps[1][0], c.ps[2][1]-c.ps[1][1], c.ps[2][2]-c.ps[1][2]];
        let (l1, l2, l3) = ((c.e1[0]*c.e1[0]+c.e1[1]*c.e1[1]+c.e1[2]*c.e1[2]).sqrt(), (c.e2[0]*c.e2[0]+c.e2[1]*c.e2[1]+c.e2[2]*c.e2[2]).sqrt(), (e3[0]*e3[0]+e3[1]*e3[1]+e3[2]*e3[2]).sqrt());
        let le = if l1 >= l2 && l1 >= l3 { c.e1 } else if l2 >= l3 { c.e2 } else { e3 };
        let d = le[0]*c.n[0]+le[1]*c.n[1]+le[2]*c.n[2];
        let eu = norm([le[0]-d*c.n[0], le[1]-d*c.n[1], le[2]-d*c.n[2]]);
        let dot = (eu[0]*hu[0]+eu[1]*hu[1]+eu[2]*hu[2]).clamp(-1.0,1.0);
        n += 1;
        dotsum += dot.abs() as f64;
        if dot.abs() > 0.999 { par += 1; }
        // sign test: sign(dot) vs sign(sdet)
        if dot >= 0.0 { if sdet >= 0.0 { sdet_pos += 1; } } else { if sdet < 0.0 { sdet_neg += 1; } }
    }
    println!("{}: sliver corners n={} |dot| mean={:.4} parallel(>0.999)={} ({:.1}%) sdet-sign-agree={}+{} ({:.1}%)",
        a[3], n, dotsum/n.max(1) as f64, par, 100.0*par as f32/n.max(1) as f32,
        sdet_pos, sdet_neg, 100.0*(sdet_pos+sdet_neg) as f32/n.max(1) as f32);
}
