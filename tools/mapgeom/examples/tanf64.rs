//! f64 tangent vs f32. Usage: tanf64 FILE SUBSTR
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[2]]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt().max(1e-30);
    [v[0]/l, v[1]/l, v[2]/l]
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
                let (mut f32ok, mut f64ok, mut tot) = (0, 0, 0);
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let u = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    let e1 = sub(p[1], p[0]);
                    let e2 = sub(p[2], p[0]);
                    let du1 = u[1][0]-u[0][0];
                    let dv1 = u[1][1]-u[0][1];
                    let du2 = u[2][0]-u[0][0];
                    let dv2 = u[2][1]-u[0][1];
                    let det = du1*dv2-du2*dv1;
                    for k in 0..3 {
                        tot += 1;
                        let n = nrm[t[k] as usize];
                        // f32 path
                        let fu32 = if det.abs() < 1e-12 {
                            let up = if n[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
                            norm(cross(up, n))
                        } else {
                            let r = 1.0/det;
                            let tu2 = norm([(e1[0]*dv2-e2[0]*dv1)*r, (e1[1]*dv2-e2[1]*dv1)*r, (e1[2]*dv2-e2[2]*dv1)*r]);
                            let d = tu2[0]*n[0]+tu2[1]*n[1]+tu2[2]*n[2];
                            norm([tu2[0]-d*n[0], tu2[1]-d*n[1], tu2[2]-d*n[2]])
                        };
                        // f64 path
                        let e1d = [[e1[0] as f64, e1[1] as f64, e1[2] as f64]];
                        let e2d = [e2[0] as f64, e2[1] as f64, e2[2] as f64];
                        let dud = [[du1 as f64, dv1 as f64], [du2 as f64, dv2 as f64]];
                        let detd = dud[0][0]*dud[1][1]-dud[1][0]*dud[0][1];
                        let nd = [n[0] as f64, n[1] as f64, n[2] as f64];
                        let fu64 = if detd.abs() < 1e-12 {
                            let up = if nd[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
                            let c = [up[1]*nd[2]-up[2]*nd[1], up[2]*nd[0]-up[0]*nd[2], up[0]*nd[1]-up[1]*nd[0]];
                            let l: f64 = (c[0]*c[0]+c[1]*c[1]+c[2]*c[2]).sqrt().max(1e-30);
                            [(c[0]/l) as f32, (c[1]/l) as f32, (c[2]/l) as f32]
                        } else {
                            let r = 1.0/detd;
                            let tu2 = [(e1d[0][0]*dud[1][1]-e2d[0]*dud[0][1])*r, (e1d[0][1]*dud[1][1]-e2d[1]*dud[0][1])*r, (e1d[0][2]*dud[1][1]-e2d[2]*dud[0][1])*r];
                            let l: f64 = (tu2[0]*tu2[0]+tu2[1]*tu2[1]+tu2[2]*tu2[2]).sqrt().max(1e-30);
                            let tu2 = [tu2[0]/l, tu2[1]/l, tu2[2]/l];
                            let d = tu2[0]*nd[0]+tu2[1]*nd[1]+tu2[2]*nd[2];
                            let o = [tu2[0]-d*nd[0], tu2[1]-d*nd[1], tu2[2]-d*nd[2]];
                            let l2: f64 = (o[0]*o[0]+o[1]*o[1]+o[2]*o[2]).sqrt().max(1e-30);
                            [(o[0]/l2) as f32, (o[1]/l2) as f32, (o[2]/l2) as f32]
                        };
                        let su = tu[t[k] as usize];
                        if (fu32[0]-su[0]).abs() < 0.01 && (fu32[1]-su[1]).abs() < 0.01 && (fu32[2]-su[2]).abs() < 0.01 { f32ok += 1; }
                        if (fu64[0]-su[0]).abs() < 0.01 && (fu64[1]-su[1]).abs() < 0.01 && (fu64[2]-su[2]).abs() < 0.01 { f64ok += 1; }
                    }
                }
                println!("{}: f32tan_match={f32ok}/{tot} f64tan_match={f64ok}/{tot}", a[2]);
                return;
            }
        }
    }
}
