//! Per-tri det + flat-U vs stored-U at an exact position. Usage: uprobedet FILE SUBSTR HX HY HZ
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt().max(1e-30);
    [v[0]/l, v[1]/l, v[2]/l]
}
fn tang(a: [f32;3], b: [f32;3]) -> f32 {
    let la = (a[0]*a[0]+a[1]*a[1]+a[2]*a[2]).sqrt().max(1e-30);
    let lb = (b[0]*b[0]+b[1]*b[1]+b[2]*b[2]).sqrt().max(1e-30);
    ((a[0]*b[0]+a[1]*b[1]+a[2]*b[2])/(la*lb)).clamp(-1.0,1.0).acos().to_degrees()
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let qb = [u32::from_str_radix(a[3].trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(a[4].trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(a[5].trim_start_matches("0x"), 16).unwrap()];
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
                let (mut pos, mut nrm, mut uv, mut tu, mut tv, mut uv1v): (Vec<[f32;3]>, Vec<[f32;3]>, Vec<[f32;2]>, Vec<[f32;3]>, Vec<[f32;3]>, Vec<[f32;2]>) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Float2(u) if d.name() == 11 => uv1v = u.clone(),
                        Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut flats: Vec<[f32; 3]> = Vec::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let mut hit = false;
                    for c in 0..3 {
                        if [ps[c][0].to_bits(), ps[c][1].to_bits(), ps[c][2].to_bits()] == qb { hit = true; break; }
                    }
                    if !hit { continue; }
                    let us = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    if std::env::var("UPROBE_FULLTRI").is_ok() {
                        println!("    fulltri idx[BITS {:08x},{:08x},{:08x}] posbits=({:08x},{:08x},{:08x})({:08x},{:08x},{:08x})({:08x},{:08x},{:08x}) uvbits=({:08x},{:08x})({:08x},{:08x})({:08x},{:08x})",
                            t[0], t[1], t[2],
                            ps[0][0].to_bits(), ps[0][1].to_bits(), ps[0][2].to_bits(), ps[1][0].to_bits(), ps[1][1].to_bits(), ps[1][2].to_bits(), ps[2][0].to_bits(), ps[2][1].to_bits(), ps[2][2].to_bits(),
                            us[0][0].to_bits(), us[0][1].to_bits(), us[1][0].to_bits(), us[1][1].to_bits(), us[2][0].to_bits(), us[2][1].to_bits());
                    }
                    let ns = [nrm[t[0] as usize], nrm[t[1] as usize], nrm[t[2] as usize]];
                    let sus = [tu[t[0] as usize], tu[t[1] as usize], tu[t[2] as usize]];
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let (du1, dv1, du2, dv2) = (us[1][0]-us[0][0], us[1][1]-us[0][1], us[2][0]-us[0][0], us[2][1]-us[0][1]);
                    let det = du1*dv2-du2*dv1;
                    // flat du-GS vs each corner's stored N (use corner0's N? print per corner)
                    for k in 0..3 {
                        if [ps[k][0].to_bits(), ps[k][1].to_bits(), ps[k][2].to_bits()] != qb { continue; }
                        let n = ns[k];
                        let fu = if det.abs() < 1e-12 {
                            [999.0, 999.0, 999.0]
                        } else {
                            let r = 1.0/det;
                            let tx = (e1[0]*dv2-e2[0]*dv1)*r;
                            let ty = (e1[1]*dv2-e2[1]*dv1)*r;
                            let tz = (e1[2]*dv2-e2[2]*dv1)*r;
                            let tl = (tx*tx+ty*ty+tz*tz).sqrt().max(1e-30);
                            let (tx, ty, tz) = (tx/tl, ty/tl, tz/tl);
                            let dd = tx*n[0]+ty*n[1]+tz*n[2];
                            norm([tx-dd*n[0], ty-dd*n[1], tz-dd*n[2]])
                        };
                        println!("tri corner{k} detbits={:08x} uv=({:08x},{:08x}) uv1=({:08x},{:08x}) flatU=({:.3},{:.3},{:.3}) storedU=({:08x},{:08x},{:08x}) storedV=({:08x},{:08x},{:08x}) N=({:08x},{:08x},{:08x}) dFlatStored={:.1}deg",
                            det.to_bits(), us[k][0].to_bits(), us[k][1].to_bits(), uv1v[t[k] as usize][0].to_bits(), uv1v[t[k] as usize][1].to_bits(), fu[0], fu[1], fu[2], sus[k][0].to_bits(), sus[k][1].to_bits(), sus[k][2].to_bits(), tv[t[k] as usize][0].to_bits(), tv[t[k] as usize][1].to_bits(), tv[t[k] as usize][2].to_bits(), ns[k][0].to_bits(), ns[k][1].to_bits(), ns[k][2].to_bits(),
                            if fu[0] > 900.0 { -1.0 } else { tang(fu, sus[k]) });
                        if fu[0] <= 900.0 { flats.push(fu); }
                    }
                }
                // pairwise flat angles
                for i in 0..flats.len() {
                    for j in (i+1)..flats.len() {
                        println!("   flatU{i}-flatU{j} angle={:.1}deg", tang(flats[i], flats[j]));
                    }
                }
            }
        }
    }
    let _ = BTreeMap::<u32,u32>::new();
}
