//! Tri match by sorted mmkey + winding parity check. Usage: trimatch HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> Vec<([(i32,i32,i32);3], [[f32;3];3])> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out = Vec::new();
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
                        let mk = |p: &[f32;3]| ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32);
                        let mut k = [mk(&ps[0]), mk(&ps[1]), mk(&ps[2])];
                        k.sort();
                        out.push((k, ps));
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32,i32,i32);3], Vec<[[f32;3];3]>> = BTreeMap::new();
    for (k, ps) in &m { mmap.entry(*k).or_default().push(*ps); }
    let (mut hit, mut miss, mut flip) = (0, 0, 0);
    for (k, rps) in &r {
        match mmap.get_mut(k) {
            Some(v) if !v.is_empty() => {
                hit += 1;
                let mps = v.pop().unwrap();
                // winding parity: permutation mapping rps order to mps order (match by nearest, positions ~equal post-transplant)
                // find perm p with mps[p[i]] ≈ rps[i]
                let mut perm = [0; 3];
                let mut used = [false; 3];
                let mut ok = true;
                for i in 0..3 {
                    let mut best = (1e9f32, 0);
                    for j in 0..3 {
                        if used[j] { continue; }
                        let d = ((rps[i][0]-mps[j][0]).powi(2)+(rps[i][1]-mps[j][1]).powi(2)+(rps[i][2]-mps[j][2]).powi(2)).sqrt();
                        if d < best.0 { best = (d, j); }
                    }
                    if best.0 > 1e-4 { ok = false; break; }
                    used[best.1] = true;
                    perm[i] = best.1;
                }
                if ok {
                    // parity of perm: even = same winding, odd = flipped
                    let inv = (if perm[0] > perm[1] { 1 } else { 0 }) + (if perm[0] > perm[2] { 1 } else { 0 }) + (if perm[1] > perm[2] { 1 } else { 0 });
                    if inv % 2 == 1 { flip += 1; }
                }
            }
            _ => { miss += 1; }
        }
    }
    let mleft: usize = mmap.values().map(|v| v.len()).sum();
    println!("{}: his={} mine={} matched={} his_miss={} mine_left={} flipped_winding={flip}", a[3], r.len(), m.len(), hit, miss, mleft);
}
