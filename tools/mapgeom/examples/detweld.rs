//! Do welded groups have mixed det? Usage: detweld HIS.ITEM SUBSTR
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
                let (mut pos, mut nrm, mut uv, mut uv1) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut tridet: Vec<bool> = Vec::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { tridet.push(true); continue; }
                    let u = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    let du1 = u[1][0]-u[0][0];
                    let dv1 = u[1][1]-u[0][1];
                    let du2 = u[2][0]-u[0][0];
                    let dv2 = u[2][1]-u[0][1];
                    tridet.push(du1*dv2-du2*dv1 >= 0.0);
                }
                // vert idx -> tris
                let mut v2t: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
                for (ti, t) in idx.chunks(3).enumerate() {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        v2t.entry(t[k] as usize).or_default().push(ti);
                    }
                }
                // group verts by (pos,n,uv,uv1); check det signs (via contributing tris)
                let mut bybase: BTreeMap<Vec<u32>, Vec<usize>> = BTreeMap::new();
                for (vi2, _) in pos.iter().enumerate() {
                    let k = vec![pos[vi2][0].to_bits(), pos[vi2][1].to_bits(), pos[vi2][2].to_bits(), nrm[vi2][0].to_bits(), nrm[vi2][1].to_bits(), nrm[vi2][2].to_bits(), uv[vi2][0].to_bits(), uv[vi2][1].to_bits(), uv1[vi2][0].to_bits(), uv1[vi2][1].to_bits()];
                    bybase.entry(k).or_default().push(vi2);
                }
                // welded = groups with 1 vert? No (verts already welded). Instead: positions with 1 vert (fully welded) vs positions with 2+ verts (split).
                // For positions with 1 vert: check if contributing tris have mixed det (if mixed but welded, det-sign key would over-split!).
                let mut bypos: BTreeMap<[u32; 3], Vec<usize>> = BTreeMap::new();
                for (vi2, _) in pos.iter().enumerate() {
                    bypos.entry([pos[vi2][0].to_bits(), pos[vi2][1].to_bits(), pos[vi2][2].to_bits()]).or_default().push(vi2);
                }
                let (mut weld_mixed, mut weld_same, mut split_mixed, mut split_same) = (0, 0, 0, 0);
                for (_, verts) in &bypos {
                    // det signs across all contributing tris
                    let mut pos_sign = false;
                    let mut neg_sign = false;
                    for v in verts {
                        if let Some(tis) = v2t.get(v) {
                            for ti in tis {
                                if tridet[*ti] { pos_sign = true; } else { neg_sign = true; }
                            }
                        }
                    }
                    let mixed = pos_sign && neg_sign;
                    if verts.len() == 1 {
                        if mixed { weld_mixed += 1; } else { weld_same += 1; }
                    } else {
                        if mixed { split_mixed += 1; } else { split_same += 1; }
                    }
                }
                println!("{}: welded_pos mixed_det={weld_mixed} same_det={weld_same} | split_pos mixed={split_mixed} same={split_same}", a[2]);
                return;
            }
        }
    }
}
