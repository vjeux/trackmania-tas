//! All tri corners within EPS of a position, with face normals. Usage: epsnear FILE SUBSTR X Y Z EPS
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let q: [f32;3] = [a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let eps: f32 = a[6].parse().unwrap();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[2] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos = Vec::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 { pos = p.clone(); }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    for k in 0..3 {
                        let d = ((ps[k][0]-q[0]).powi(2)+(ps[k][1]-q[1]).powi(2)+(ps[k][2]-q[2]).powi(2)).sqrt();
                        if d < eps {
                            println!("d={:.1e} p=({:.6},{:.6},{:.6}) face=({:.4},{:.4},{:.4}) area={:.6}",
                                d, ps[k][0], ps[k][1], ps[k][2], cr[0]/l, cr[1]/l, cr[2]/l, l/2.0);
                        }
                    }
                }
            }
        }
    }
}
