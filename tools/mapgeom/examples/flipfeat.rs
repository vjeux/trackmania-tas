//! Features of flipped vs unflipped tris. Usage: flipfeat HIS.ITEM MINE.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[2]]
}
fn len(v: [f32; 3]) -> f32 { (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt() }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> Vec<(Vec<[f32; 3]>, String)> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out = Vec::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
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
                        out.push((vec![pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]], mat.clone()));
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, (t, _)) in m.iter().enumerate() {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    // per material: flipped tri aspect ratios + normal dirs
    for (t, mat) in &r {
        let mut k = [mk(&t[0]), mk(&t[1]), mk(&t[2])];
        k.sort();
        if let Some(v) = mmap.get(&k) {
            let (u, _) = &m[v[0]];
            let mut perm = [0, 1, 2];
            'outer: for cand in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                let mut ok = true;
                for cc in 0..3 {
                    if (u[cand[cc]][0]-t[cc][0]).abs() > 0.002 || (u[cand[cc]][1]-t[cc][1]).abs() > 0.002 || (u[cand[cc]][2]-t[cc][2]).abs() > 0.002 {
                        ok = false; break;
                    }
                }
                if ok { perm = cand; break 'outer; }
            }
            let fnr = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            let fnm = cross(sub(u[perm[1]], u[perm[0]]), sub(u[perm[2]], u[perm[0]]));
            let lr = len(fnr);
            let lm = len(fnm);
            if lr < 1e-12 || lm < 1e-12 { continue; }
            let dot = (fnr[0]*fnm[0]+fnr[1]*fnm[1]+fnr[2]*fnm[2])/(lr*lm);
            if dot < 0.0 {
                // features (use MY tri = crystal fan tri)
                let e0 = len(sub(u[0], u[1]));
                let e1 = len(sub(u[1], u[2]));
                let e2 = len(sub(u[2], u[0]));
                let mx = e0.max(e1).max(e2);
                let mn = e0.min(e1).min(e2);
                let n = [fnm[0]/lm, fnm[1]/lm, fnm[2]/lm];
                println!("FLIP {} ar={:.1} n=[{:.2},{:.2},{:.2}] cen=[{:.2},{:.2},{:.2}]",
                    mat.rsplit('\\').next().unwrap_or(mat), mx/mn.max(1e-9), n[0], n[1], n[2],
                    (u[0][0]+u[1][0]+u[2][0])/3.0, (u[0][1]+u[1][1]+u[2][1])/3.0, (u[0][2]+u[1][2]+u[2][2])/3.0);
            }
        }
    }
}
