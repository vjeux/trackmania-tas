//! Do "other" splits coincide with det-sign changes? Usage: detsplit HIS.ITEM SUBSTR
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
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(&a[2]) { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrm, mut uv, mut uv1, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(),
                        Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                // per tri det sign
                let mut tridet: Vec<f32> = Vec::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { tridet.push(0.0); continue; }
                    let u = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    let du1 = u[1][0]-u[0][0];
                    let dv1 = u[1][1]-u[0][1];
                    let du2 = u[2][0]-u[0][0];
                    let dv2 = u[2][1]-u[0][1];
                    tridet.push(du1*dv2-du2*dv1);
                }
                // group by (pos,n,uv,uv1); for split groups (2+ verts), check det signs of contributing tris
                let mut bybase: BTreeMap<Vec<u32>, Vec<usize>> = BTreeMap::new(); // base key -> vert indices
                // (need tri per vert; verts are per-index, tris per-index/3. Map vert idx -> tri idx.)
                for (vi2, _) in pos.iter().enumerate() {
                    let k = vec![pos[vi2][0].to_bits(), pos[vi2][1].to_bits(), pos[vi2][2].to_bits(), nrm[vi2][0].to_bits(), nrm[vi2][1].to_bits(), nrm[vi2][2].to_bits(), uv[vi2][0].to_bits(), uv[vi2][1].to_bits(), uv1[vi2][0].to_bits(), uv1[vi2][1].to_bits()];
                    bybase.entry(k).or_default().push(vi2);
                }
                // vert idx -> tri idx (via index buffer position? verts are deduplicated, tris reference them. Need reverse map.)
                let mut v2t: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
                for (ti, t) in idx.chunks(3).enumerate() {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        v2t.entry(t[k] as usize).or_default().push(ti);
                    }
                }
                let (mut detmix, mut detsame, mut groups) = (0, 0, 0);
                for (_, verts) in &bybase {
                    if verts.len() < 2 { continue; }
                    groups += 1;
                    // det signs across contributing tris
                    let mut signs: Vec<bool> = Vec::new();
                    for v in verts {
                        if let Some(tis) = v2t.get(v) {
                            for ti in tis {
                                signs.push(tridet[*ti] >= 0.0);
                            }
                        }
                    }
                    let has_pos = signs.iter().any(|x| *x);
                    let has_neg = signs.iter().any(|x| !*x);
                    if has_pos && has_neg { detmix += 1; } else { detsame += 1; }
                }
                println!("{}: split-base-groups={groups} det_mixed={detmix} det_same={detsame}", a[2]);
                return;
            }
        }
    }
}
