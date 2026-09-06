//! Does uv1 constancy predict stored clusters? Usage: uv1clust HIS.ITEM SUBSTR
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
                let (mut pos, mut nrm, mut uv1) = (Vec::new(), Vec::new(), Vec::new());
                let mut has_uv1 = false;
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 11 => { uv1 = u.clone(); has_uv1 = true; }
                        _ => {}
                    }
                }
                if !has_uv1 { println!("{}: no uv1 channel", a[2]); return; }
                if pos.len() != nrm.len() || pos.len() != uv1.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut bypos: BTreeMap<[u32; 3], Vec<([f32; 3], [f32; 2])>> = BTreeMap::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        let p = pos[t[k] as usize];
                        bypos.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().push((nrm[t[k] as usize], uv1[t[k] as usize]));
                    }
                }
                // confusion matrix: (uv1_const?, stored_single?) counts
                let (mut cc_ss, mut cc_sm, mut cs_ss, mut cs_sm) = (0, 0, 0, 0);
                for (_, corners) in &bypos {
                    if corners.len() < 2 { continue; }
                    let mut duv1 = 0;
                    for (i, (_, u)) in corners.iter().enumerate() {
                        if corners[..i].iter().all(|(_, x)| (x[0]-u[0]).abs() > 1e-7 || (x[1]-u[1]).abs() > 1e-7) {
                            duv1 += 1;
                        }
                    }
                    let mut dn = 0;
                    for (i, (n, _)) in corners.iter().enumerate() {
                        if corners[..i].iter().all(|(x, _)| (x[0]-n[0]).abs() > 1e-7 || (x[1]-n[1]).abs() > 1e-7 || (x[2]-n[2]).abs() > 1e-7) {
                            dn += 1;
                        }
                    }
                    match (duv1 == 1, dn == 1) {
                        (true, true) => cc_ss += 1,
                        (true, false) => cc_sm += 1,
                        (false, true) => cs_ss += 1,
                        (false, false) => cs_sm += 1,
                    }
                }
                println!("{}: uv1const+nsingle={cc_ss} uv1const+nmulti={cc_sm} uv1split+nsingle={cs_ss} uv1split+nmulti={cs_sm}", a[2]);
                return;
            }
        }
    }
}
