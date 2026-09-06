//! Full weld key test. Usage: fullkey FILE
use std::collections::BTreeSet;
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
                let (mut pos, mut nrm, mut uv, mut uv1, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                let mut has_uv1 = false;
                let mut has_tan = false;
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Float2(u) if d.name() == 11 => { uv1 = u.clone(); has_uv1 = true; }
                        Elem::Word(w) if d.name() == 18 => { tu = w.iter().map(|v| dec(*v)).collect(); has_tan = true; }
                        Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut s_full: BTreeSet<Vec<u32>> = BTreeSet::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        let vi2 = t[k] as usize;
                        let mut key = vec![pos[vi2][0].to_bits(), pos[vi2][1].to_bits(), pos[vi2][2].to_bits(),
                                           nrm[vi2][0].to_bits(), nrm[vi2][1].to_bits(), nrm[vi2][2].to_bits(),
                                           uv[vi2][0].to_bits(), uv[vi2][1].to_bits()];
                        if has_uv1 && vi2 < uv1.len() {
                            key.push(uv1[vi2][0].to_bits());
                            key.push(uv1[vi2][1].to_bits());
                        }
                        if has_tan && vi2 < tu.len() {
                            key.push(tu[vi2][0].to_bits());
                            key.push(tu[vi2][1].to_bits());
                            key.push(tu[vi2][2].to_bits());
                            key.push(tv[vi2][0].to_bits());
                            key.push(tv[vi2][1].to_bits());
                            key.push(tv[vi2][2].to_bits());
                        }
                        s_full.insert(key);
                    }
                }
                println!("{}: stream={} |fullkey|={} diff={}", mat, pos.len(), s_full.len(), pos.len() as i64 - s_full.len() as i64);
            }
        }
    }
}
