//! Score U variants vs his U at fuzzy verts. Usage: uscore HIS.ITEM MINE.ITEM SUBSTR
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
    // my corners: pos, uv, storedN(smoothed); per-tri faceN + uvgrad
    struct Corner { p: [f32;3], uv: [f32;2], sn: [f32;3], fn_: [f32;3], e1: [f32;3], e2: [f32;3], du1: f32, dv1: f32, du2: f32, dv2: f32 }
    let mut mine: Vec<Corner> = Vec::new();
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
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let us = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    let ns = [nrm[t[0] as usize], nrm[t[1] as usize], nrm[t[2] as usize]];
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    let fn_ = [cr[0]/l, cr[1]/l, cr[2]/l];
                    let (du1, dv1, du2, dv2) = (us[1][0]-us[0][0], us[1][1]-us[0][1], us[2][0]-us[0][0], us[2][1]-us[0][1]);
                    for k in 0..3 {
                        mine.push(Corner { p: ps[k], uv: us[k], sn: ns[k], fn_, e1, e2, du1, dv1, du2, dv2 });
                    }
                }
            }
        }
    }
    // his verts (pos, U)
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
    // grid his by position
    let mut grid: BTreeMap<(i64,i64,i64), Vec<usize>> = BTreeMap::new();
    for (j, (p, _)) in his.iter().enumerate() {
        grid.entry(((p[0]*5000.0).floor() as i64, (p[1]*5000.0).floor() as i64, (p[2]*5000.0).floor() as i64)).or_default().push(j);
    }
    // variants: (name, use_faceN, du_primary)
    let variants = [("duGS-smoothN", false, true), ("dvNxV-smoothN", false, false), ("duGS-faceN", true, true), ("dvNxV-faceN", true, false)];
    let mut stats: Vec<(f64, usize, usize)> = vec![(0.0, 0, 0); variants.len()]; // (0168sum, n, bitok)
    let mut tot = 0;
    for c in &mine {
        // nearest his vert within 0.2mm
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
        tot += 1;
        let hu = his[best.1].1;
        let det = c.du1*c.dv2-c.du2*c.dv1;
        if det.abs() < 1e-12 { continue; }
        let r = 1.0/det;
        for (vi, (_, use_face, use_du)) in variants.iter().enumerate() {
            let n = if *use_face { c.fn_ } else { c.sn };
            let u = if *use_du {
                let tx = (c.e1[0]*c.dv2-c.e2[0]*c.dv1)*r;
                let ty = (c.e1[1]*c.dv2-c.e2[1]*c.dv1)*r;
                let tz = (c.e1[2]*c.dv2-c.e2[2]*c.dv1)*r;
                let tl = (tx*tx+ty*ty+tz*tz).sqrt().max(1e-30);
                let (tx, ty, tz) = (tx/tl, ty/tl, tz/tl);
                let dd = tx*n[0]+ty*n[1]+tz*n[2];
                norm([tx-dd*n[0], ty-dd*n[1], tz-dd*n[2]])
            } else {
                let vx = (c.e1[0]*c.du2-c.e2[0]*c.du1)*r;
                let vy = (c.e1[1]*c.du2-c.e2[1]*c.du1)*r;
                let vz = (c.e1[2]*c.du2-c.e2[2]*c.du1)*r;
                let vl = (vx*vx+vy*vy+vz*vz).sqrt().max(1e-30);
                let vg = [vx/vl, vy/vl, vz/vl];
                norm([n[1]*vg[2]-n[2]*vg[1], n[2]*vg[0]-n[0]*vg[2], n[0]*vg[1]-n[1]*vg[0]])
            };
            let la = (u[0]*u[0]+u[1]*u[1]+u[2]*u[2]).sqrt().max(1e-30);
            let lb = (hu[0]*hu[0]+hu[1]*hu[1]+hu[2]*hu[2]).sqrt().max(1e-30);
            let ang = ((u[0]*hu[0]+u[1]*hu[1]+u[2]*hu[2])/(la*lb)).clamp(-1.0,1.0).acos().to_degrees();
            stats[vi].0 += ang as f64;
            stats[vi].1 += 1;
            // bit check (trunc words)
            let qw = |v: [f32;3]| { let mut o = 0u32; for (k, x) in v.iter().enumerate() { let q = (x.clamp(-1.0,1.0)*511.0) as i32; o |= ((q & 0x3FF) as u32) << (10*k); } o };
            if qw(u) == qw(hu) { stats[vi].2 += 1; }
        }
    }
    println!("{}: matched_corners={tot}", a[3]);
    for (vi, (name, _, _)) in variants.iter().enumerate() {
        println!("   {name}: meanU={:.4}deg bit={} ({:.1}%)", stats[vi].0/stats[vi].1.max(1) as f64, stats[vi].2, 100.0*stats[vi].2 as f32/stats[vi].1.max(1) as f32);
    }
}
