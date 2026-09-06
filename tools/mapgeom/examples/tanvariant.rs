//! U formula variants (flat %). Usage: tanvariant FILE SUBSTR
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
                // Variants: GS target = smoothed N (per-corner) vs face N. Count flat % (match within 0.01).
                let (mut gs_smooth, mut gs_face, mut tot) = (0, 0, 0);
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
                    if det.abs() < 1e-12 { continue; }
                    let r = 1.0/det;
                    let raw = norm([(e1[0]*dv2-e2[0]*dv1)*r, (e1[1]*dv2-e2[1]*dv1)*r, (e1[2]*dv2-e2[2]*dv1)*r]);
                    // face normal
                    let cr = cross(e1, e2);
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    let fn_ = [cr[0]/l, cr[1]/l, cr[2]/l];
                    for k in 0..3 {
                        tot += 1;
                        let su = tu[t[k] as usize];
                        // GS vs smoothed N
                        let n = nrm[t[k] as usize];
                        let d = raw[0]*n[0]+raw[1]*n[1]+raw[2]*n[2];
                        let gs = norm([raw[0]-d*n[0], raw[1]-d*n[1], raw[2]-d*n[2]]);
                        if (gs[0]-su[0]).abs() < 0.01 && (gs[1]-su[1]).abs() < 0.01 && (gs[2]-su[2]).abs() < 0.01 { gs_smooth += 1; }
                        // GS vs face N
                        let d2 = raw[0]*fn_[0]+raw[1]*fn_[1]+raw[2]*fn_[2];
                        let gf = norm([raw[0]-d2*fn_[0], raw[1]-d2*fn_[1], raw[2]-d2*fn_[2]]);
                        if (gf[0]-su[0]).abs() < 0.01 && (gf[1]-su[1]).abs() < 0.01 && (gf[2]-su[2]).abs() < 0.01 { gs_face += 1; }
                    }
                }
                println!("{}: GS-vs-smoothedN {gs_smooth}/{tot} GS-vs-faceN {gs_face}/{tot}", a[2]);
                return;
            }
        }
    }
}
