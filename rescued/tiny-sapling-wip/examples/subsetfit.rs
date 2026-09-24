//! Exact word-match subset search for his N at a position. Usage: subsetfit HIS.ITEM MINE.ITEM SUBSTR X Y Z
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
fn qw(v: [f32; 3]) -> [i32; 3] {
    [(v[0].clamp(-1.0, 1.0) * 511.0) as i32, (v[1].clamp(-1.0, 1.0) * 511.0) as i32, (v[2].clamp(-1.0, 1.0) * 511.0) as i32]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let q: [f32;3] = [a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap()];
    // his verts at q: N words
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut hisv: Vec<[i32; 3]> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrmw) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrmw = w.clone(),
                        _ => {}
                    }
                }
                for i in 0..pos.len() {
                    let d = ((pos[i][0]-q[0]).powi(2)+(pos[i][1]-q[1]).powi(2)+(pos[i][2]-q[2]).powi(2)).sqrt();
                    if d < 2e-4 {
                        let w = nrmw[i];
                        hisv.push([((w & 0x3FF) as i32) << 22 >> 22, ((w >> 10 & 0x3FF) as i32) << 22 >> 22, ((w >> 20 & 0x3FF) as i32) << 22 >> 22]);
                    }
                }
            }
        }
    }
    // my incident faces: (face normal, corner angle at q)
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut faces: Vec<([f32; 3], f32)> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos = Vec::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 { pos = p.clone(); }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let mut ci = None;
                    for c in 0..3 {
                        let d = ((ps[c][0]-q[0]).powi(2)+(ps[c][1]-q[1]).powi(2)+(ps[c][2]-q[2]).powi(2)).sqrt();
                        if d < 1e-6 { ci = Some(c); break; }
                    }
                    let ci = match ci { Some(v) => v, None => continue };
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                    let o1 = (ci+1)%3;
                    let o2 = (ci+2)%3;
                    let v1 = [ps[o1][0]-ps[ci][0], ps[o1][1]-ps[ci][1], ps[o1][2]-ps[ci][2]];
                    let v2 = [ps[o2][0]-ps[ci][0], ps[o2][1]-ps[ci][1], ps[o2][2]-ps[ci][2]];
                    let l1 = (v1[0]*v1[0]+v1[1]*v1[1]+v1[2]*v1[2]).sqrt().max(1e-30);
                    let l2 = (v2[0]*v2[0]+v2[1]*v2[1]+v2[2]*v2[2]).sqrt().max(1e-30);
                    let an = ((v1[0]*v2[0]+v1[1]*v2[1]+v1[2]*v2[2])/(l1*l2)).clamp(-1.0,1.0).acos();
                    faces.push(([cr[0]/l, cr[1]/l, cr[2]/l], an));
                }
            }
        }
    }
    println!("his verts={} incident faces={}", hisv.len(), faces.len());
    for (i, f) in faces.iter().enumerate() {
        println!("  face{i} n=({:.4},{:.4},{:.4}) corner_ang={:.2}deg", f.0[0], f.0[1], f.0[2], f.1.to_degrees());
    }
    if faces.len() > 24 {
        println!("too many faces for exhaustive search");
        return;
    }
    let nf = faces.len();
    // per his vert: subsets x {uniform, angle} exact word hits
    for (vi, hv) in hisv.iter().enumerate() {
        let mut hits: Vec<(String, Vec<usize>)> = Vec::new();
        for mask in 1..(1u32 << nf) {
            let set: Vec<usize> = (0..nf).filter(|k| mask & (1 << k) != 0).collect();
            for law in 0..2 {
                let mut acc = [0.0f64; 3];
                let mut s = 0.0f64;
                for k in &set {
                    let w = if law == 0 { 1.0 } else { faces[*k].1 as f64 };
                    for d in 0..3 { acc[d] += faces[*k].0[d] as f64 * w; }
                    s += w;
                }
                let cand = norm([(acc[0]/s) as f32, (acc[1]/s) as f32, (acc[2]/s) as f32]);
                if qw(cand) == *hv {
                    hits.push(((if law == 0 { "uni" } else { "ang" }).to_string(), set));
                    break;
                }
            }
        }
        println!("his vert{vi} words={hv:?}: {} exact subsets", hits.len());
        for (law, s) in hits.iter().take(8) {
            println!("   {law} {s:?}");
        }
    }
    let _ = BTreeMap::<u32,u32>::new();
    let _ = dec;
}
