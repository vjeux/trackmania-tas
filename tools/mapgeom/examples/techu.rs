//! Technics 1-1 spots: his U/V/N + per-tri det. Usage: techu HIS.ITEM MINE.ITEM
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
    // his Technics verts at 1-1 positions (need mine for 1-1; approximate: positions with exactly 1 vert in HIS file? No (twins are his-side too). Use mine's 1-1 set? Simplify: dump all his Technics verts with tri dets.)
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != "Technics" { continue; }
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
                // per-tri det + corner U for first 12 tris
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut n = 0;
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let u = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    let du1 = u[1][0]-u[0][0];
                    let dv1 = u[1][1]-u[0][1];
                    let du2 = u[2][0]-u[0][0];
                    let dv2 = u[2][1]-u[0][1];
                    let det = du1*dv2-du2*dv1;
                    let uu = tu[t[0] as usize];
                    let vv = tv[t[0] as usize];
                    let nn = nrm[t[0] as usize];
                    println!("det={:.3e} U=({:.3},{:.3},{:.3}) V=({:.3},{:.3},{:.3}) N=({:.3},{:.3},{:.3}) U.N={:.3}",
                        det, uu[0],uu[1],uu[2], vv[0],vv[1],vv[2], nn[0],nn[1],nn[2], uu[0]*nn[0]+uu[1]*nn[1]+uu[2]*nn[2]);
                    n += 1;
                    if n > 14 { break; }
                }
            }
        }
    }
    let _ = BTreeMap::<u32,u32>::new();
}
