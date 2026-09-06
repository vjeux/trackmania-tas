//! One Laplacian pass over smoothed normals: does it move toward his? Usage: laptest HIS.ITEM MINE.ITEM SUBSTR [N]
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
    let n: usize = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(6);
    // my verts: pos -> normal (need 1-1 only? use all; adjacency from tris)
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // per material: tris (positions), vert normals by position (average if multiple? use 1-1 only for test)
    let mut mypos: BTreeMap<[u32;3], [f32;3]> = BTreeMap::new();
    let mut multicount = 0;
    let mut tris: Vec<[[u32;3];3]> = Vec::new();
    let mut posf: BTreeMap<[u32;3],[f32;3]> = BTreeMap::new();
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
                    let k = [pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()];
                    posf.insert(k, pos[i]);
                    if mypos.insert(k, nrm[i]).is_some() { multicount += 1; }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let k = [0,1,2].map(|j| { let p = pos[t[j] as usize]; [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()] });
                    tris.push(k);
                }
            }
        }
    }
    // adjacency: position -> set of neighbor positions (share a tri)
    let mut adj: BTreeMap<[u32;3], BTreeSet<[u32;3]>> = BTreeMap::new();
    for t in &tris {
        for i in 0..3 {
            for j in 0..3 {
                if i != j { adj.entry(t[i]).or_default().insert(t[j]); }
            }
        }
    }
    // his normals
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
    let mut shown = 0;
    let (mut d0, mut d1, mut cnt) = (0.0f64, 0.0f64, 0usize);
    for (k, hv) in &his {
        if hv.len() != 1 { continue; }
        let mn = match mypos.get(k) { Some(v) => *v, None => continue };
        // laplacian: avg of my smoothed normals at neighbor positions (+self?)
        let nbrs = match adj.get(k) { Some(s) => s, None => continue };
        let mut acc = [0.0f64; 3];
        let mut c = 0;
        for nb in nbrs {
            if let Some(nn) = mypos.get(nb) { for d in 0..3 { acc[d] += nn[d] as f64; } c += 1; }
        }
        if c == 0 { continue; }
        let lap = norm([(acc[0]/c as f64) as f32, (acc[1]/c as f64) as f32, (acc[2]/c as f64) as f32]);
        // avg including self
        let mut acc2 = [mn[0] as f64, mn[1] as f64, mn[2] as f64];
        for nb in nbrs {
            if let Some(nn) = mypos.get(nb) { for d in 0..3 { acc2[d] += nn[d] as f64; } }
        }
        let lapself = norm([(acc2[0]/(c+1) as f64) as f32, (acc2[1]/(c+1) as f64) as f32, (acc2[2]/(c+1) as f64) as f32]);
        d0 += ang(hv[0], mn) as f64; d1 += ang(hv[0], lapself) as f64; cnt += 1;
        if ang(hv[0], mn) > 0.5 && shown < n {
            let p = posf[k];
            println!("pos=({:.5},{:.5},{:.5}) his=({:.4},{:.4},{:.4}) mine=({:.4},{:.4},{:.4}) lapself=({:.4},{:.4},{:.4}) d_mine={:.3} d_lap={:.3}",
                p[0], p[1], p[2], hv[0][0], hv[0][1], hv[0][2], mn[0], mn[1], mn[2], lapself[0], lapself[1], lapself[2], ang(hv[0], mn), ang(hv[0], lapself));
            shown += 1;
        }
    }
    println!("{}: 1-1 n={cnt} multi={multicount} mean_d_mine={:.4}deg mean_d_lapself={:.4}deg", a[3], d0/cnt.max(1) as f64, d1/cnt.max(1) as f64);
}
