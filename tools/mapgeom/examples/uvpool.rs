//! Group corners by exact uv bits, avg face normals, compare to his at 1-1 spots. Usage: uvpool HIS.ITEM MINE.ITEM SUBSTR
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
    // my corners: (poskey, uvkey) -> face normal of its tri
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // uvkey -> list of face normals (one per corner occurrence)
    let mut uvgroups: BTreeMap<[u32;2], Vec<[f32;3]>> = BTreeMap::new();
    // poskey -> list of (uvkey, facenormal)
    let mut poscorners: BTreeMap<[u32;3], Vec<([u32;2],[f32;3])>> = BTreeMap::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut uv) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let us = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    let fn_ = [cr[0]/l, cr[1]/l, cr[2]/l];
                    for k in 0..3 {
                        let uk = [us[k][0].to_bits(), us[k][1].to_bits()];
                        let pk = [ps[k][0].to_bits(), ps[k][1].to_bits(), ps[k][2].to_bits()];
                        uvgroups.entry(uk).or_default().push(fn_);
                        poscorners.entry(pk).or_default().push((uk, fn_));
                    }
                }
            }
        }
    }
    // his normals by pos
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut his: BTreeMap<[u32;3], Vec<[f32;3]>> = BTreeMap::new();
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
                    his.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push(nrm[i]);
                }
            }
        }
    }
    let ang = |x: [f32;3], y: [f32;3]| (x[0]*y[0]+x[1]*y[1]+x[2]*y[2]).clamp(-1.0,1.0).acos().to_degrees();
    // uv-group stats
    let mut multisize = 0;
    let mut multis = 0;
    for (_, v) in &uvgroups {
        if v.len() > 3 { multis += 1; multisize += v.len(); }
    }
    println!("{}: uvgroups={} multi(>3)={multis} multicorners={multisize}", a[3], uvgroups.len());
    // at 1-1 spots: compare his vs avg-over-union-of-uvgroups-of-its-corners
    let (mut dpos, mut duv, mut cnt) = (0.0f64, 0.0f64, 0usize);
    for (k, hv) in &his {
        if hv.len() != 1 { continue; }
        let pc = match poscorners.get(k) { Some(v) => v, None => continue };
        // position avg
        let mut acc = [0.0f64; 3];
        for (_, fn_) in pc { for d in 0..3 { acc[d] += fn_[d] as f64; } }
        let n = pc.len() as f64;
        let pa = norm([(acc[0]/n) as f32, (acc[1]/n) as f32, (acc[2]/n) as f32]);
        // union over uv groups of its corners
        let mut acc2 = [0.0f64; 3];
        let mut n2 = 0;
        let mut seen = std::collections::BTreeSet::new();
        for (uk, _) in pc {
            if !seen.insert(*uk) { continue; }
            if let Some(fs) = uvgroups.get(uk) {
                for fn_ in fs { for d in 0..3 { acc2[d] += fn_[d] as f64; } n2 += 1; }
            }
        }
        if n2 == 0 { continue; }
        let ua = norm([(acc2[0]/n2 as f64) as f32, (acc2[1]/n2 as f64) as f32, (acc2[2]/n2 as f64) as f32]);
        dpos += ang(hv[0], pa) as f64; duv += ang(hv[0], ua) as f64; cnt += 1;
    }
    println!("1-1 n={cnt} mean_d_posavg={:.4}deg mean_d_uvavg={:.4}deg", dpos/cnt.max(1) as f64, duv/cnt.max(1) as f64);
}
