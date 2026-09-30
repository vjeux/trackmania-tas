//! Does U follow N clusters? Average per-face U within (pos,N) groups, compare to stored U.
//! Usage: uclustertest HIS.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
use mapgeom::static_item::bake::{tangent, Corner};
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
                let (mut pos, mut nrm, mut uv, mut tu) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                if pos.len() != nrm.len() || pos.is_empty() || tu.is_empty() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                // per-face U (using FACE normal for GS? or stored? Use face normal (from positions) for per-face U.)
                // Group corners by (pos, storedN). Average per-face U within group. Compare avg to stored U.
                let mut groups: BTreeMap<(String, String), Vec<[f32; 3]>> = BTreeMap::new(); // (posbits, nbits) -> per-face U list
                let mut stored: BTreeMap<(String, String), Vec<[f32; 3]>> = BTreeMap::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let u = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    // face normal
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    let fn_ = [cr[0]/l, cr[1]/l, cr[2]/l];
                    let c = [Corner { pos: p[0], normal: fn_, uv: u[0], uv1: u[0] },
                             Corner { pos: p[1], normal: fn_, uv: u[1], uv1: u[1] },
                             Corner { pos: p[2], normal: fn_, uv: u[2], uv1: u[2] }];
                    let (fu, _) = tangent(&c);
                    for k in 0..3 {
                        let pb = format!("{:x}{:x}{:x}", p[k][0].to_bits(), p[k][1].to_bits(), p[k][2].to_bits());
                        let nb = format!("{:x}{:x}{:x}", nrm[t[k] as usize][0].to_bits(), nrm[t[k] as usize][1].to_bits(), nrm[t[k] as usize][2].to_bits());
                        groups.entry((pb.clone(), nb.clone())).or_default().push(fu);
                        stored.entry((pb, nb)).or_default().push(tu[t[k] as usize]);
                    }
                }
                // compare group-average U to stored U (should be constant per group if U follows N)
                let (mut agree, mut tot) = (0, 0);
                for (key, us) in &groups {
                    // average us
                    let n = us.len() as f32;
                    let avg = [us.iter().map(|x| x[0]).sum::<f32>()/n, us.iter().map(|x| x[1]).sum::<f32>()/n, us.iter().map(|x| x[2]).sum::<f32>()/n];
                    let l = (avg[0]*avg[0]+avg[1]*avg[1]+avg[2]*avg[2]).sqrt().max(1e-30);
                    let avg = [avg[0]/l, avg[1]/l, avg[2]/l];
                    if let Some(ss) = stored.get(key) {
                        for s in ss {
                            tot += 1;
                            if (avg[0]-s[0]).abs() < 0.01 && (avg[1]-s[1]).abs() < 0.01 && (avg[2]-s[2]).abs() < 0.01 {
                                agree += 1;
                            }
                        }
                    }
                }
                println!("{}: U avg-within-Ncluster matches stored {agree}/{tot} ({:.1}%)", a[2], 100.0*agree as f32/tot as f32);
                return;
            }
        }
    }
}
