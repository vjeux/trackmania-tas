//! Find double tris (same positions, opposite normals). Usage: dbltri FILE SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
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
                // tri key -> list of (avg stored normal)
                let mut tmap: BTreeMap<[(i32, i32, i32); 3], Vec<[f32; 3]>> = BTreeMap::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let mut k = [mk(&pos[t[0] as usize]), mk(&pos[t[1] as usize]), mk(&pos[t[2] as usize])];
                    k.sort();
                    let avg = [(nrm[t[0] as usize][0]+nrm[t[1] as usize][0]+nrm[t[2] as usize][0])/3.0,
                               (nrm[t[0] as usize][1]+nrm[t[1] as usize][1]+nrm[t[2] as usize][1])/3.0,
                               (nrm[t[0] as usize][2]+nrm[t[1] as usize][2]+nrm[t[2] as usize][2])/3.0];
                    tmap.entry(k).or_default().push(avg);
                }
                let mut dbl = 0;
                let mut dbl_opp = 0;
                for (_, v) in &tmap {
                    if v.len() > 1 {
                        dbl += 1;
                        // opposite?
                        for x in 0..v.len() {
                            for y in (x+1)..v.len() {
                                let dot = v[x][0]*v[y][0]+v[x][1]*v[y][1]+v[x][2]*v[y][2];
                                if dot < -0.9 {
                                    dbl_opp += 1;
                                    break;
                                }
                            }
                        }
                    }
                }
                println!("{}: distinct_trikeys={} doubled={} doubled_opposite={}", a[2], tmap.len(), dbl, dbl_opp);
                return;
            }
        }
    }
}
