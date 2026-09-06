//! Check (N,U,V) orthogonality. Usage: tanbasis FILE SUBSTR
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
                let (mut d_nu, mut d_nv, mut d_uv, mut mx_nu, mut mx_nv, mut mx_uv) = (0.0f64, 0.0f64, 0.0f64, 0.0f32, 0.0f32, 0.0f32);
                for i in 0..nrm.len() {
                    let nu = (nrm[i][0]*tu[i][0]+nrm[i][1]*tu[i][1]+nrm[i][2]*tu[i][2]).abs();
                    let nv = (nrm[i][0]*tv[i][0]+nrm[i][1]*tv[i][1]+nrm[i][2]*tv[i][2]).abs();
                    let uv = (tu[i][0]*tv[i][0]+tu[i][1]*tv[i][1]+tu[i][2]*tv[i][2]).abs();
                    d_nu += nu as f64; d_nv += nv as f64; d_uv += uv as f64;
                    mx_nu = mx_nu.max(nu); mx_nv = mx_nv.max(nv); mx_uv = mx_uv.max(uv);
                }
                let n = nrm.len() as f64;
                println!("{}: |N.U| avg={:.4} max={:.3} |N.V| avg={:.4} max={:.3} |U.V| avg={:.4} max={:.3}",
                    a[2], d_nu/n, mx_nu, d_nv/n, mx_nv, d_uv/n, mx_uv);
                return;
            }
        }
    }
}
