//! Per-triangle normal comparison on identical tri-sets. Usage: trinorm REF MINE
//! For each ref tri, finds the same tri (mm keys) in mine, compares ref
//! stored vert normals vs both sides' geometric face normals.
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt();
    if l < 1e-12 { [0.0, 1.0, 0.0] } else { [v[0]/l, v[1]/l, v[2]/l] }
}

struct Tri { v: [[f32; 3]; 3], n: Vec<[f32; 3]> }

fn load(path: &str) -> Vec<Tri> {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut out = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrm) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => {
                            nrm = w.iter().map(|v| {
                                let x = ((*v & 0x3FF) as i32) << 22 >> 22;
                                let y = ((*v >> 10 & 0x3FF) as i32) << 22 >> 22;
                                let z = ((*v >> 20 & 0x3FF) as i32) << 22 >> 22;
                                [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
                            }).collect();
                        }
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 {
                        continue;
                    }
                    out.push(Tri { v: [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]], n: vec![nrm[t[0] as usize], nrm[t[1] as usize], nrm[t[2] as usize]] });
                }
            }
        }
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut mmap: BTreeMap<[(i32, i32, i32); 3], Vec<usize>> = BTreeMap::new();
    for (i, t) in m.iter().enumerate() {
        let mut k = [key(&t.v[0]), key(&t.v[1]), key(&t.v[2])];
        k.sort();
        mmap.entry(k).or_default().push(i);
    }
    let (mut same, mut flip, mut other, mut miss) = (0, 0, 0, 0);
    let mut ex = Vec::new();
    for t in &r {
        let mut k = [key(&t.v[0]), key(&t.v[1]), key(&t.v[2])];
        k.sort();
        match mmap.get(&k) {
            None => miss += 1,
            Some(v) => {
                let u = &m[v[0]];
                let fn_r = norm(cross(sub(t.v[1], t.v[0]), sub(t.v[2], t.v[0])));
                let fn_m = norm(cross(sub(u.v[1], u.v[0]), sub(u.v[2], u.v[0])));
                let dot = fn_r[0]*fn_m[0] + fn_r[1]*fn_m[1] + fn_r[2]*fn_m[2];
                // ref stored normals vs ref face normal
                let d = t.n[0][0]*fn_r[0] + t.n[0][1]*fn_r[1] + t.n[0][2]*fn_r[2];
                if dot > 0.99 {
                    same += 1;
                } else if dot < -0.99 {
                    flip += 1;
                } else {
                    other += 1;
                }
                if ex.len() < 3 {
                    ex.push((t.v[0], fn_r, t.n[0], fn_m, d));
                }
            }
        }
    }
    println!("ref tris {}: same-winding={} flipped={} other={} missing={}", r.len(), same, flip, other, miss);
    for (p, fr, nr, fm, d) in ex {
        println!("  pos {p:?} ref-face {fr:?} ref-stored {nr:?} (align {d:.2}) mine-face {fm:?}");
    }
}
