//! Faces+verts near a decimal position, both files. Usage: nearfaces HIS.ITEM MINE.ITEM SUBSTR X Y Z
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn dump(path: &str, sub: &str, q: [f32;3], tag: &str) {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    println!("== {tag} near ({},{},{}) ==", q[0], q[1], q[2]);
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != sub { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrm, mut uv, mut uv1, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(),
                        Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                for (i, p) in pos.iter().enumerate() {
                    let d = ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt();
                    if d < 2e-4 && i < nrm.len() {
                        let u1 = if i < uv1.len() { format!("({:.4},{:.4})", uv1[i][0], uv1[i][1]) } else { "(--)".to_string() };
                        let uu = if i < tu.len() { format!("({:.3},{:.3},{:.3})", tu[i][0], tu[i][1], tu[i][2]) } else { "(--)".to_string() };
                        let vv = if i < tv.len() { format!("({:.3},{:.3},{:.3})", tv[i][0], tv[i][1], tv[i][2]) } else { "(--)".to_string() };
                        println!("  v p=({:.6},{:.6},{:.6}) n=({:.4},{:.4},{:.4}) uv=({:.4},{:.4}) uv1={} U={} V={}", p[0],p[1],p[2], nrm[i][0],nrm[i][1],nrm[i][2], uv[i][0],uv[i][1], u1, uu, vv);
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    if ps.iter().all(|p| ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt() > 5e-4) { continue; }
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    println!("  tri fn=({:.4},{:.4},{:.4}) area={:.6}", cr[0]/l, cr[1]/l, cr[2]/l, l/2.0);
                }
            }
        }
    }
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let q = [a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap()];
    dump(&a[1], &a[3], q, "HIS");
    dump(&a[2], &a[3], q, "MINE");
}
