//! Measure his weld rule: flat-face dihedral angles tolerated vs refused.
//! Usage: weldrule REF.HIS [SUBSTR] [CELLMM]
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;

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
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt();
    if l < 1e-12 { [0.0, 1.0, 0.0] } else { [v[0]/l, v[1]/l, v[2]/l] }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let substr = a.get(2).map(|s| s.as_str()).unwrap_or("");
    let cell: f32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.25);
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut tol: BTreeMap<String, f32> = BTreeMap::new();
    let mut refus: BTreeMap<String, f32> = BTreeMap::new();
    let mut ncells: BTreeMap<String, usize> = BTreeMap::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or("?".into());
        if !substr.is_empty() && !mat.contains(substr) { continue; }
        let short = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
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
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut cells: BTreeMap<(i32, i32, i32), Vec<([f32; 3], [f32; 3])>> = BTreeMap::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let p = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let fn_ = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
                    for k in 0..3 {
                        let ck = ((p[k][0]/cell).round() as i32, (p[k][1]/cell).round() as i32, (p[k][2]/cell).round() as i32);
                        cells.entry(ck).or_default().push((fn_, nrm[t[k] as usize]));
                    }
                }
                for (_, corners) in &cells {
                    if corners.len() < 2 { continue; }
                    *ncells.entry(short.clone()).or_insert(0) += 1;
                    // cluster by stored normal
                    let mut cl: Vec<Vec<[f32; 3]>> = Vec::new(); // flat normals
                    let mut sn: Vec<[f32; 3]> = Vec::new(); // representative stored
                    for (fn_, s) in corners {
                        let mut done = false;
                        for (ci, r) in sn.iter().enumerate() {
                            if dot(*r, *s) > 0.9999 {
                                cl[ci].push(*fn_);
                                done = true;
                                break;
                            }
                        }
                        if !done {
                            sn.push(*s);
                            cl.push(vec![*fn_]);
                        }
                    }
                    // tolerated: max flat-angle within a cluster
                    for c in &cl {
                        for x in 0..c.len() {
                            for y in (x+1)..c.len() {
                                let ang = dot(c[x], c[y]).clamp(-1.0, 1.0).acos().to_degrees();
                                let e = tol.entry(short.clone()).or_insert(0.0);
                                if ang > *e { *e = ang; }
                            }
                        }
                    }
                    // refused: min flat-angle across clusters
                    for x in 0..cl.len() {
                        for y in (x+1)..cl.len() {
                            let mut m = 180.0f32;
                            for fx in &cl[x] {
                                for fy in &cl[y] {
                                    m = m.min(dot(*fx, *fy).clamp(-1.0, 1.0).acos().to_degrees());
                                }
                            }
                            let e = refus.entry(short.clone()).or_insert(180.0);
                            if m < *e { *e = m; }
                        }
                    }
                }
            }
        }
    }
    for (m, _) in &ncells {
        println!("{m}: cells={} max_tolerated={:.1} min_refused={:.1}", ncells[m], tol.get(m).unwrap_or(&-1.0), refus.get(m).unwrap_or(&-1.0));
    }
}
