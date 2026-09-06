//! Match tris by diffuse UV (position-independent), report position residuals.
//! Usage: uvmatch REF.HIS MINE.BAKED dx dy dz [SUBSTR]
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;

fn ukey(u: &[f32; 2]) -> (u32, u32) { (u[0].to_bits(), u[1].to_bits()) }
fn load(path: &str) -> Vec<(Vec<[f32; 3]>, Vec<[f32; 2]>, String)> {
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
                let (mut pos, mut uv) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        _ => {}
                    }
                }
                if pos.len() != uv.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    out.push((vec![pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]],
                              vec![uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]], mat.clone()));
                }
            }
        }
    }
    out
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d: Vec<f32> = vec![a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let substr = a.get(6).map(|s| s.as_str()).unwrap_or("");
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[((u32, u32)); 3], Vec<usize>> = BTreeMap::new();
    for (i, (_, u, mat)) in m.iter().enumerate() {
        if !substr.is_empty() && !mat.contains(substr) { continue; }
        let mut k = [ukey(&u[0]), ukey(&u[1]), ukey(&u[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    // per material: matched count, residual histogram (mm)
    let mut stat: BTreeMap<String, Vec<f32>> = BTreeMap::new();
    let mut unmatched: BTreeMap<String, usize> = BTreeMap::new();
    for (p, u, mat) in &r {
        if !substr.is_empty() && !mat.contains(substr) { continue; }
        let short = mat.rsplit('\\').next().unwrap_or(mat).to_string();
        let mut k = [ukey(&u[0]), ukey(&u[1]), ukey(&u[2])];
        k.sort();
        match mmap.get(&k) {
            Some(v) => {
                let (_, _, _) = &m[v[0]];
                let mp = &m[v[0]].0;
                // best corner assignment by min total distance
                let mut best = f32::MAX;
                for perm in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                    let mut tot = 0.0f32;
                    let mut mx = 0.0f32;
                    for c in 0..3 {
                        let dx = mp[perm[c]][0]+d[0]-p[c][0];
                        let dy = mp[perm[c]][1]+d[1]-p[c][1];
                        let dz = mp[perm[c]][2]+d[2]-p[c][2];
                        let d2 = (dx*dx+dy*dy+dz*dz).sqrt();
                        tot += d2;
                        mx = mx.max(d2);
                    }
                    if tot < best { best = mx; }
                }
                stat.entry(short).or_default().push(best * 1000.0);
            }
            None => { *unmatched.entry(short).or_insert(0) += 1; }
        }
    }
    for (mat, v) in &stat {
        let mut s = v.clone();
        s.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let un = unmatched.get(mat).unwrap_or(&0);
        println!("{mat}: uvmatched={} uvunmatched={} resid_mm min={:.3} p50={:.3} p90={:.3} max={:.3}",
            s.len(), un, s[0], s[s.len()/2], s[(s.len()*9)/10], s[s.len()-1]);
    }
    for (mat, un) in &unmatched {
        if !stat.contains_key(mat) {
            println!("{mat}: uvmatched=0 uvunmatched={un}");
        }
    }
}
