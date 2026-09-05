//! Test smooth-normal schemes on the wall visual (identical welding both sides).
//! Usage: wallnorm REF SRC
//! Takes ref vis1 (wall, 24 verts) tris+normals; builds same tris from source
//! crystal exact-half; tests simple/angle/area/Newell schemes per-vert.
use mapgeom::static_item::vstream::Elem;
use std::collections::BTreeMap;

fn dec3n_pack(n: [f32; 3]) -> u32 {
    let q = |x: f32| ((x * 511.0).round() as i32).clamp(-511, 511) as u32 & 0x3FF;
    q(n[0]) | (q(n[1]) << 10) | (q(n[2]) << 20)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt();
    if l < 1e-12 { [0.0, 1.0, 0.0] } else { [v[0]/l, v[1]/l, v[2]/l] }
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0]*b[0]+a[1]*b[1]+a[2]*b[2] }
fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, ((p[1]*1000.0).round() as i32), ((p[2]*1000.0).round() as i32))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    // ref vis1: index buffer + positions + normals
    let refdata = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&refdata).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // wall = material index 1 in ref
    let vi = s2.shaded_geoms.iter().find(|g| g.material_index == 1).unwrap().visual_index as usize;
    let (mut rpos, mut rnrm, mut ridx) = (Vec::new(), Vec::new(), Vec::new());
    if let Some(mapgeom::static_item::Node::Visual(vis)) = s2.visuals[vi].inline.as_deref() {
        let st = vis.stream().unwrap();
        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
            match e {
                Elem::Float3(p) if d.name() == 0 => rpos = p.clone(),
                Elem::Word(w) if d.name() == 5 => rnrm = w.clone(),
                _ => {}
            }
        }
        ridx = vis.index_buffer.as_ref().unwrap().indices.clone();
    }
    println!("ref wall: {} verts {} idx", rpos.len(), ridx.len());
    // source crystal wall faces (mat0=TrackWall), fan-triangulated exact half
    let srcdata = std::fs::read(&a[2]).unwrap();
    let it = mapgeom::crystal::ItemCrystal::open(&srcdata).unwrap();
    let layer = it.model.first_geometry().unwrap();
    let c = layer.kind.crystal().unwrap();
    let mut tris: Vec<[[f32; 3]; 3]> = Vec::new();
    for fa in &c.faces {
        if fa.material != 0 { continue; }
        let pts: Vec<[f32; 3]> = fa.verts.iter().map(|i| { let p = c.positions[*i as usize]; [p[0]*0.5, p[1]*0.5, p[2]*0.5] }).collect();
        if pts.len() == 3 {
            tris.push([pts[0], pts[1], pts[2]]);
        } else {
            for i in 2..pts.len() {
                tris.push([pts[1], pts[i], pts[(i + 1) % pts.len()]]);
            }
        }
    }
    println!("src wall tris: {}", tris.len());
    // ref triangle set (mm keys) vs src triangle set
    let trikey = |t: &[[f32; 3]; 3]| { let mut v = [key(&t[0]), key(&t[1]), key(&t[2])]; v.sort(); v };
    let rtris: BTreeMap<_, usize> = {
        let mut m = BTreeMap::new();
        for t in ridx.chunks(3) {
            *m.entry(trikey(&[rpos[t[0] as usize], rpos[t[1] as usize], rpos[t[2] as usize]])).or_default() += 1;
        }
        m
    };
    let stris: BTreeMap<_, usize> = {
        let mut m = BTreeMap::new();
        for t in &tris {
            *m.entry(trikey(t)).or_default() += 1;
        }
        m
    };
    println!("ref tri-set == src tri-set: {}", rtris == stris);
    if rtris != stris {
        println!("--- ref tris (mm, sorted corners):");
        for (t, c) in rtris.iter().take(16) { println!("  {t:?} x{c}"); }
        println!("--- src tris (mm, sorted corners):");
        for (t, c) in stris.iter().take(16) { println!("  {t:?} x{c}"); }
    }
    // per ref vert: adjacent src faces (by mm), schemes
    let mut adj: BTreeMap<(i32,i32,i32), Vec<[f32; 3]>> = BTreeMap::new();
    for t in &tris {
        let e1 = sub(t[1], t[0]); let e2 = sub(t[2], t[0]);
        let fn_ = norm(cross(e1, e2));
        let area = (dot(cross(e1, e2), cross(e1, e2))).sqrt();
        let ang = |u: [f32; 3], v: [f32; 3]| dot(norm(u), norm(v)).clamp(-1.0, 1.0).acos();
        let angs = [ang(e1, e2), ang(sub(t[0], t[1]), sub(t[2], t[1])), ang(sub(t[0], t[2]), sub(t[1], t[2]))];
        for k in 0..3 {
            adj.entry(key(&t[k])).or_default().push([fn_[0], fn_[1], fn_[2]]);
            let _ = angs;
        }
    }
    // need angles+areas too; redo with full info
    let mut adj2: BTreeMap<(i32,i32,i32), Vec<([f32; 3], f32, f32)>> = BTreeMap::new();
    for t in &tris {
        let e1 = sub(t[1], t[0]); let e2 = sub(t[2], t[0]); let e3 = sub(t[2], t[1]);
        let fn_ = norm(cross(e1, e2));
        let area = (dot(cross(e1, e2), cross(e1, e2))).sqrt() / 2.0;
        let ang = |u: [f32; 3], v: [f32; 3]| dot(norm(u), norm(v)).clamp(-1.0, 1.0).acos();
        let angs = [ang(e1, e2), ang(sub(t[0], t[1]), e3), ang(sub(t[0], t[2]), sub(t[1], t[2]))];
        for k in 0..3 {
            adj2.entry(key(&t[k])).or_default().push((fn_, angs[k], area));
        }
    }
    let mut score = [0, 0, 0, 0];
    let mut total = 0;
    for (p, n) in rpos.iter().zip(rnrm.iter()) {
        if let Some(ad) = adj2.get(&key(p)) {
            let cands = [
                norm([ad.iter().map(|(f, _, _)| f[0]).sum::<f32>(), ad.iter().map(|(f, _, _)| f[1]).sum::<f32>(), ad.iter().map(|(f, _, _)| f[2]).sum::<f32>()]),
                norm([ad.iter().map(|(f, w, _)| f[0]*w).sum::<f32>(), ad.iter().map(|(f, w, _)| f[1]*w).sum::<f32>(), ad.iter().map(|(f, w, _)| f[2]*w).sum::<f32>()]),
                norm([ad.iter().map(|(f, _, r)| f[0]*r).sum::<f32>(), ad.iter().map(|(f, _, r)| f[1]*r).sum::<f32>(), ad.iter().map(|(f, _, r)| f[2]*r).sum::<f32>()]),
                ad[0].0,
            ];
            total += 1;
            for (s, cn) in cands.iter().enumerate() {
                if dec3n_pack(*cn) == *n {
                    score[s] += 1;
                }
            }
        }
    }
    println!("verts tested {total}: simple={} angle={} area={} flat(first-face)={}", score[0], score[1], score[2], score[3]);
    let _ = adj;
}
