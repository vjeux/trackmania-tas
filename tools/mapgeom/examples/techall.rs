//! All Technics stored=1/face44=2+ positions: dihedral + area + location stats.
//! Usage: techall HIS.ITEM
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
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.ends_with("\\Technics") { continue; }
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
                if pos.len() != nrm.len() || pos.len() != uv.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut bypos: BTreeMap<[u32; 3], Vec<(usize, [f32; 3], [f32; 2])>> = BTreeMap::new();
                // also need face normals per tri
                let mut tri_n: Vec<[f32; 3]> = Vec::new();
                let mut tri_a: Vec<f32> = Vec::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { tri_n.push([0.0; 3]); tri_a.push(0.0); continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt();
                    if l < 1e-15 { tri_n.push([0.0; 3]); tri_a.push(0.0); continue; }
                    tri_n.push([cr[0]/l, cr[1]/l, cr[2]/l]);
                    tri_a.push(l/2.0);
                    for k in 0..3 {
                        bypos.entry([p[k][0].to_bits(), p[k][1].to_bits(), p[k][2].to_bits()]).or_default().push((tri_n.len()-1, nrm[t[k] as usize], uv[t[k] as usize]));
                    }
                }
                // classify
                let cos_max = 44.0f32.to_radians().cos();
                let (mut weld_dih, mut weld_n) = (Vec::new(), 0);
                for (_, corners) in &bypos {
                    if corners.len() < 2 { continue; }
                    let mut sd: Vec<[f32; 3]> = Vec::new();
                    for (_, sn, _) in corners {
                        if !sd.iter().any(|x| (x[0]-sn[0]).abs() < 1e-7 && (x[1]-sn[1]).abs() < 1e-7 && (x[2]-sn[2]).abs() < 1e-7) {
                            sd.push(*sn);
                        }
                    }
                    if sd.len() != 1 { continue; }
                    // stored=1: max face dihedral + uv diversity
                    let mut maxdih = 0.0f32;
                    for x in 0..corners.len() {
                        for y in (x+1)..corners.len() {
                            let n0 = tri_n[corners[x].0];
                            let n1 = tri_n[corners[y].0];
                            let dot = (n0[0]*n1[0]+n0[1]*n1[1]+n0[2]*n1[2]).clamp(-1.0, 1.0);
                            maxdih = maxdih.max(dot.acos().to_degrees());
                        }
                    }
                    // uv diversity
                    let mut duv = 0;
                    for (i, (_, _, u)) in corners.iter().enumerate() {
                        if corners[..i].iter().all(|(_, _, x)| (x[0]-u[0]).abs() > 1e-9 || (x[1]-u[1]).abs() > 1e-9) {
                            duv += 1;
                        }
                    }
                    weld_n += 1;
                    if maxdih > 44.0 {
                        weld_dih.push((maxdih, corners.len(), duv));
                    }
                }
                println!("Technics stored=1 positions={weld_n}, of_which_dihedral_gt44={}", weld_dih.len());
                weld_dih.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
                for (dih, nc, duv) in weld_dih.iter().take(15) {
                    println!("  dihedral={dih:.1} ncorners={nc} distinct_uv={duv}");
                }
            }
        }
    }
}
