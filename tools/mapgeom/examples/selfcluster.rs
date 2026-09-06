//! Cluster HIS face normals; compare to HIS stored clusters. Usage: selfcluster HIS.ITEM
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
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or("?".into());
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
                if pos.len() != nrm.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                // stored clusters per position
                let mut bypos: BTreeMap<[u32; 3], Vec<[f32; 3]>> = BTreeMap::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        let p = pos[t[k] as usize];
                        bypos.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().push(nrm[t[k] as usize]);
                    }
                }
                let mut stored_clusters = 0;
                for (_, ns) in &bypos {
                    let mut dist = 0;
                    for n in ns {
                        if dist == 0 { dist = 1; continue; }
                        // count distinct (simplified: compare to first of each)
                    }
                    // proper: distinct count
                    let mut d2: Vec<[f32; 3]> = Vec::new();
                    for n in ns {
                        if !d2.iter().any(|x| (x[0]-n[0]).abs() < 1e-7 && (x[1]-n[1]).abs() < 1e-7 && (x[2]-n[2]).abs() < 1e-7) {
                            d2.push(*n);
                        }
                    }
                    stored_clusters += d2.len().max(1);
                }
                // face-normal clusters (seed-largest 44°)
                let cos_max = 44.0f32.to_radians().cos();
                // need per-corner face normal + area: recompute per tri
                let mut fbypos: BTreeMap<[u32; 3], Vec<([f32; 3], f32)>> = BTreeMap::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt();
                    if l < 1e-15 { continue; }
                    let nn = [cr[0]/l, cr[1]/l, cr[2]/l];
                    let area = l/2.0;
                    for k in 0..3 {
                        fbypos.entry([p[k][0].to_bits(), p[k][1].to_bits(), p[k][2].to_bits()]).or_default().push((nn, area));
                    }
                }
                let mut face_clusters = 0;
                for (_, corners) in &fbypos {
                    if corners.len() < 2 { face_clusters += 1; continue; }
                    let mut ord: Vec<usize> = (0..corners.len()).collect();
                    ord.sort_by(|a, b| corners[*b].1.partial_cmp(&corners[*a].1).unwrap());
                    let mut seeds: Vec<[f32; 3]> = Vec::new();
                    for gi in ord {
                        let n = corners[gi].0;
                        if !seeds.iter().any(|s| s[0]*n[0]+s[1]*n[1]+s[2]*n[2] >= cos_max) {
                            seeds.push(n);
                        }
                    }
                    face_clusters += seeds.len().max(1);
                }
                println!("{mat}: stored_clusters={stored_clusters} face44_clusters={face_clusters} (verts={})", pos.len());
            }
        }
    }
}
