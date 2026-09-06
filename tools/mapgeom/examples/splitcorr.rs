//! Correlate his splits with edge length / face area. Usage: splitcorr HIS.ITEM SUBSTR
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
    // per position (bits): corners (tri, face normal, stored normal, edge lens to centroid?)
    // simpler: for each tri edge (shared by 2 tris at same positions?), compare stored normals
    // simplest robust: group corners by position; for each pair in group, record (edge_len?, angle_between_face_normals, welded?)
    // edge length proxy: distance between the two OTHER corners? no. Use face area.
    let mut weld_area: Vec<f32> = Vec::new();
    let mut split_area: Vec<f32> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(&a[2]) { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrm) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                if pos.len() != nrm.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                // corners by position
                let mut bypos: BTreeMap<[u32; 3], Vec<(usize, usize)>> = BTreeMap::new();
                for (ti, t) in idx.chunks(3).enumerate() {
                    if t.len() < 3 { continue; }
                    for k in 0..3 {
                        let p = pos[t[k] as usize];
                        bypos.entry([p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]).or_default().push((ti, t[k] as usize));
                    }
                }
                // tri areas
                let mut areas: Vec<f32> = Vec::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { areas.push(0.0); continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let e1 = [p[1][0]-p[0][0], p[1][1]-p[0][1], p[1][2]-p[0][2]];
                    let e2 = [p[2][0]-p[0][0], p[2][1]-p[0][1], p[2][2]-p[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    areas.push(((cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt()/2.0));
                }
                for (_, corners) in &bypos {
                    if corners.len() < 2 { continue; }
                    // cluster by stored normal (0.9999)
                    let mut cl: Vec<Vec<(usize, usize)>> = Vec::new();
                    for c in corners {
                        let mut done = false;
                        for cc in cl.iter_mut() {
                            let (ti0, vi0) = cc[0];
                            let _ = ti0;
                            // compare stored normals
                            let n0 = nrm[vi0];
                            let n1 = nrm[c.1];
                            let dot = n0[0]*n1[0]+n0[1]*n1[1]+n0[2]*n1[2];
                            if dot > 0.9999 {
                                cc.push(*c);
                                done = true;
                                break;
                            }
                        }
                        if !done { cl.push(vec![*c]); }
                    }
                    if cl.len() == 1 {
                        // welded: record min area
                        let a2 = cl[0].iter().map(|(ti, _)| areas[*ti]).fold(f32::MAX, |x, y| x.min(y));
                        weld_area.push(a2);
                    } else {
                        // split: record min area
                        let a2 = corners.iter().map(|(ti, _)| areas[*ti]).fold(f32::MAX, |x, y| x.min(y));
                        split_area.push(a2);
                    }
                }
            }
        }
    }
    weld_area.sort_by(|x, y| x.partial_cmp(y).unwrap());
    split_area.sort_by(|x, y| x.partial_cmp(y).unwrap());
    if !weld_area.is_empty() && !split_area.is_empty() {
        println!("{}: welded min-area p50={:.2e} | split min-area p50={:.2e}",
            a[2], weld_area[weld_area.len()/2], split_area[split_area.len()/2]);
    }
}
