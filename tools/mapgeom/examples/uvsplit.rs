//! Do "other" splits differ in U or V? Usage: uvsplit FILE SUBSTR
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
                // group by (pos,n,uv,uv1); for groups with 2+ verts (differ in U/V), check U vs V
                let mut bybase: BTreeMap<Vec<u32>, Vec<usize>> = BTreeMap::new();
                for i in 0..pos.len() {
                    let mut k = vec![pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits(),
                                     nrm[i][0].to_bits(), nrm[i][1].to_bits(), nrm[i][2].to_bits(),
                                     uv[i][0].to_bits(), uv[i][1].to_bits(), uv1[i][0].to_bits(), uv1[i][1].to_bits()];
                    let _ = k;
                    bybase.entry(vec![pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits(), nrm[i][0].to_bits(), nrm[i][1].to_bits(), nrm[i][2].to_bits(), uv[i][0].to_bits(), uv[i][1].to_bits(), uv1[i][0].to_bits(), uv1[i][1].to_bits()]).or_default().push(i);
                }
                let (mut u_diff, mut v_diff, mut both, mut groups) = (0, 0, 0, 0);
                for (_, idxs) in &bybase {
                    if idxs.len() < 2 { continue; }
                    groups += 1;
                }
                // Redo properly with decoded-bit comparison
                let (mut u_only, mut v_only, mut uv_both) = (0, 0, 0);
                for (_, idxs) in &bybase {
                    if idxs.len() < 2 { continue; }
                    let mut du_set: Vec<[u32; 3]> = Vec::new();
                    let mut dv_set: Vec<[u32; 3]> = Vec::new();
                    // (tu/tv decoded; need raw words for exact. Approximate with float bits of decoded.)
                    for i in idxs {
                        let ub = [tu[*i][0].to_bits(), tu[*i][1].to_bits(), tu[*i][2].to_bits()];
                        let vb = [tv[*i][0].to_bits(), tv[*i][1].to_bits(), tv[*i][2].to_bits()];
                        if !du_set.contains(&ub) { du_set.push(ub); }
                        if !dv_set.contains(&vb) { dv_set.push(vb); }
                    }
                    if du_set.len() > 1 && dv_set.len() > 1 { uv_both += 1; }
                    else if du_set.len() > 1 { u_only += 1; }
                    else if dv_set.len() > 1 { v_only += 1; }
                }
                println!("{}: base-groups-split={} U-only={u_only} V-only={v_only} both={uv_both}", a[2], u_only+v_only+uv_both);
                return;
            }
        }
    }
    let _ = (u_only_init(), v_only_init());
}
fn u_only_init() -> usize { 0 }
fn v_only_init() -> usize { 0 }
