//! Are stored tangents per-face-flat? Usage: tanflat FILE SUBSTR
use mapgeom::static_item::vstream::Elem;
use mapgeom::static_item::bake::tangent;
use mapgeom::static_item::bake::Corner;
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
                let (mut pos, mut nrm, mut uv, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                if pos.len() != nrm.len() || pos.is_empty() || tu.is_empty() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let (mut flat_u, mut flat_v, mut tot) = (0, 0, 0);
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let c = [Corner { pos: pos[t[0] as usize], normal: nrm[t[0] as usize], uv: uv[t[0] as usize], uv1: uv[t[0] as usize] },
                             Corner { pos: pos[t[1] as usize], normal: nrm[t[1] as usize], uv: uv[t[1] as usize], uv1: uv[t[1] as usize] },
                             Corner { pos: pos[t[2] as usize], normal: nrm[t[2] as usize], uv: uv[t[2] as usize], uv1: uv[t[2] as usize] }];
                    let (fu, fv) = tangent(&c);
                    for k in 0..3 {
                        tot += 1;
                        let su = tu[t[k] as usize];
                        let sv = tv[t[k] as usize];
                        if (fu[0]-su[0]).abs() < 0.01 && (fu[1]-su[1]).abs() < 0.01 && (fu[2]-su[2]).abs() < 0.01 { flat_u += 1; }
                        if (fv[0]-sv[0]).abs() < 0.01 && (fv[1]-sv[1]).abs() < 0.01 && (fv[2]-sv[2]).abs() < 0.01 { flat_v += 1; }
                    }
                }
                println!("{}: tanU_flat={flat_u}/{tot} ({:.1}%) tanV_flat={flat_v}/{tot}", a[2], 100.0*flat_u as f32/tot as f32);
                return;
            }
        }
    }
}
