//! Per-tri uvgrad + tanrule U at a position. Usage: ucalc FILE SUBSTR X Y Z
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
    let q: [f32;3] = [a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[2] { continue; }
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
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    // does this tri touch q?
                    if !ps.iter().any(|p| ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt() < 2e-4) { continue; }
                    let us = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    let ns = [nrm[t[0] as usize], nrm[t[1] as usize], nrm[t[2] as usize]];
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let (du1, dv1, du2, dv2) = (us[1][0]-us[0][0], us[1][1]-us[0][1], us[2][0]-us[0][0], us[2][1]-us[0][1]);
                    let det = du1*dv2-du2*dv1;
                    // tanrule U per corner (uvgrad + smoothed N + GS)
                    // AND V-primary U (N x Vgrad) for formula self-consistency
                    for k in 0..3 {
                        let n = ns[k];
                        let uu = if det.abs() < 1e-12 { [0.0,0.0,0.0] } else {
                            let r = 1.0/det;
                            let tx = (e1[0]*dv2-e2[0]*dv1)*r;
                            let ty = (e1[1]*dv2-e2[1]*dv1)*r;
                            let tz = (e1[2]*dv2-e2[2]*dv1)*r;
                            let l = (tx*tx+ty*ty+tz*tz).sqrt().max(1e-30);
                            let (tx, ty, tz) = (tx/l, ty/l, tz/l);
                            let d = tx*n[0]+ty*n[1]+tz*n[2];
                            let ox = tx-d*n[0];
                            let oy = ty-d*n[1];
                            let oz = tz-d*n[2];
                            let l2 = (ox*ox+oy*oy+oz*oz).sqrt().max(1e-30);
                            [ox/l2, oy/l2, oz/l2]
                        };
                        // V-primary: Vgrad then U=NxVg
                        let up = if det.abs() < 1e-12 { [0.0,0.0,0.0] } else {
                            let r = 1.0/det;
                            let vx = (e1[0]*du2-e2[0]*du1)*r;
                            let vy = (e1[1]*du2-e2[1]*du1)*r;
                            let vz = (e1[2]*du2-e2[2]*du1)*r;
                            let vl = (vx*vx+vy*vy+vz*vz).sqrt().max(1e-30);
                            let (vx, vy, vz) = (vx/vl, vy/vl, vz/vl);
                            let ux = n[1]*vz-n[2]*vy;
                            let uy = n[2]*vx-n[0]*vz;
                            let uz = n[0]*vy-n[1]*vx;
                            let ul = (ux*ux+uy*uy+uz*uz).sqrt().max(1e-30);
                            [ux/ul, uy/ul, uz/ul]
                        };
                        println!("tri corner{k} uv=({:.4},{:.4}) N=({:.4},{:.4},{:.4}) det={:.4e} U_tanrule=({:.4},{:.4},{:.4}) U_vprim=({:.4},{:.4},{:.4})",
                            us[k][0], us[k][1], n[0], n[1], n[2], det, uu[0], uu[1], uu[2], up[0], up[1], up[2]);
                    }
                }
            }
        }
    }
}
