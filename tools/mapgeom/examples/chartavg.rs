//! Chart-wide face average vs his N at a position. Usage: chartavg HIS.ITEM MINE.ITEM SUBSTR X Y Z
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
    let mut tris: Vec<([usize;3])> = Vec::new();
    let (mut pos, mut uv1) = (Vec::new(), Vec::new());
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    tris.push([t[0] as usize, t[1] as usize, t[2] as usize]);
                }
            }
        }
    }
    // union-find by shared (pos,uv1)
    let mut parent: Vec<usize> = (0..tris.len()).collect();
    fn find(p: &mut Vec<usize>, x: usize) -> usize {
        if p[x] != x { p[x] = find(p, p[x]); }
        p[x]
    }
    let mut cmap: BTreeMap<[u32;5], Vec<usize>> = BTreeMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for k in 0..3 {
            cmap.entry([pos[t[k]][0].to_bits(), pos[t[k]][1].to_bits(), pos[t[k]][2].to_bits(), uv1[t[k]][0].to_bits(), uv1[t[k]][1].to_bits()]).or_default().push(ti);
        }
    }
    for (_, tis) in &cmap {
        for w in tis.windows(2) {
            let x = find(&mut parent, w[0]);
            let y = find(&mut parent, w[1]);
            if x != y { parent[x] = y; }
        }
    }
    // seed chart: tris touching q
    let mut seed_roots = BTreeSet::new();
    for (ti, t) in tris.iter().enumerate() {
        for k in 0..3 {
            let d = ((pos[t[k]][0]-q[0]).powi(2)+(pos[t[k]][1]-q[1]).powi(2)+(pos[t[k]][2]-q[2]).powi(2)).sqrt();
            if d < 1e-6 { seed_roots.insert(find(&mut parent, ti)); break; }
        }
    }
    // face normals + chart averages
    let fnorm = |ti: usize| {
        let ps = [pos[tris[ti][0]], pos[tris[ti][1]], pos[tris[ti][2]]];
        let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
        let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
        let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
        let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
        [cr[0]/l, cr[1]/l, cr[2]/l]
    };
    let ang = |x: [f32;3], y: [f32;3]| (x[0]*y[0]+x[1]*y[1]+x[2]*y[2]).clamp(-1.0,1.0).acos().to_degrees();
    // his N
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
    println!("his=({:.4},{:.4},{:.4}) seed_charts={}", his[0], his[1], his[2], seed_roots.len());
    for r in seed_roots {
        let mut acc = [0.0f64; 3];
        let mut c = 0;
        let mut zmin = 2.0f32;
        for ti in 0..tris.len() {
            if find(&mut parent.clone(), ti) != r { continue; }
            let fn_ = fnorm(ti);
            for d in 0..3 { acc[d] += fn_[d] as f64; }
            zmin = zmin.min(fn_[2]);
            c += 1;
        }
        let avg = norm([(acc[0]/c as f64) as f32, (acc[1]/c as f64) as f32, (acc[2]/c as f64) as f32]);
        println!("chart ntris={c} zmin={zmin:.4} avg=({:.4},{:.4},{:.4}) d_his={:.3}deg", avg[0], avg[1], avg[2], ang(his, avg));
    }
}
