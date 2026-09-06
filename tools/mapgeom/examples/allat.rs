//! All incident faces at a position: every visual + collision. Usage: allat FILE X Y Z
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
    let q: [f32;3] = [a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap()];
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrm, mut uv) = (Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        _ => {}
                    }
                }
                for (i, p) in pos.iter().enumerate() {
                    let d = ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt();
                    if d < 2e-4 {
                        println!("vis mat={stem} p=({:.6},{:.6},{:.6}) n=({:.4},{:.4},{:.4}) uv=({:.4},{:.4})", p[0],p[1],p[2], nrm[i][0],nrm[i][1],nrm[i][2], uv[i][0],uv[i][1]);
                    }
                }
            }
        }
    }
    // surface mesh faces at q
    if let Some(mapgeom::static_item::Node::Surface(s)) = so.shape.inline.as_deref() {
        if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles, .. } = &s.surf {
            for t in triangles {
                let ps = [vertices[t.indices[0] as usize], vertices[t.indices[1] as usize], vertices[t.indices[2] as usize]];
                for c in 0..3 {
                    let d = ((ps[c][0]-q[0]).powi(2)+(ps[c][1]-q[1]).powi(2)+(ps[c][2]-q[2]).powi(2)).sqrt();
                    if d < 2e-4 {
                        let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                        let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                        let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                        let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                        println!("surf mat={} fn=({:.4},{:.4},{:.4})", t.material_id, cr[0]/l, cr[1]/l, cr[2]/l);
                        break;
                    }
                }
            }
        }
    }
}
