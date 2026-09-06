//! Is V = cross(N,U)? Usage: vcross FILE SUBSTR
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
                let (mut nrm, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                if nrm.is_empty() || tu.is_empty() || tv.is_empty() { continue; }
                let (mut m1, mut m2, mut tot) = (0, 0, 0);
                for i in 0..nrm.len() {
                    tot += 1;
                    let n = nrm[i];
                    let u = tu[i];
                    let v = tv[i];
                    // V = N x U?
                    let c1 = [n[1]*u[2]-n[2]*u[1], n[2]*u[0]-n[0]*u[2], n[0]*u[1]-n[1]*u[0]];
                    // V = U x N?
                    let c2 = [u[1]*n[2]-u[2]*n[1], u[2]*n[0]-u[0]*n[2], u[0]*n[1]-u[1]*n[0]];
                    if (c1[0]-v[0]).abs() < 0.01 && (c1[1]-v[1]).abs() < 0.01 && (c1[2]-v[2]).abs() < 0.01 { m1 += 1; }
                    if (c2[0]-v[0]).abs() < 0.01 && (c2[1]-v[1]).abs() < 0.01 && (c2[2]-v[2]).abs() < 0.01 { m2 += 1; }
                }
                println!("{}: V=NxU {m1}/{tot} V=UxN {m2}/{tot}", a[2]);
                return;
            }
        }
    }
}
