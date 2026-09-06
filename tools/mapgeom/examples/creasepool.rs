//! Avg over incident + crease-filtered edge-neighbors vs his N. Usage: creasepool HIS.ITEM MINE.ITEM SUBSTR X Y Z THETA
use std::collections::{BTreeMap, BTreeSet};
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
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let q: [f32;3] = [a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap()];
    let theta: f32 = a[7].parse().unwrap();
    let cos_t = theta.to_radians().cos();
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut tris: Vec<[[f32;3];3]> = Vec::new();
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
                    tris.push([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
                }
            }
        }
    }
    let fnorm = |t: &[[f32;3];3]| {
        let e1 = [t[1][0]-t[0][0], t[1][1]-t[0][1], t[1][2]-t[0][2]];
        let e2 = [t[2][0]-t[0][0], t[2][1]-t[0][1], t[2][2]-t[0][2]];
        let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
        let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
        [cr[0]/l, cr[1]/l, cr[2]/l]
    };
    // seed: tris with a corner within 1um of q
    let mut seed = BTreeSet::new();
    for (i, t) in tris.iter().enumerate() {
        for c in 0..3 {
            let d = ((t[c][0]-q[0]).powi(2)+(t[c][1]-q[1]).powi(2)+(t[c][2]-q[2]).powi(2)).sqrt();
            if d < 1e-6 { seed.insert(i); break; }
        }
    }
    let seedavg = {
        let mut acc = [0.0f64; 3];
        for i in &seed { let f = fnorm(&tris[*i]); for d in 0..3 { acc[d] += f[d] as f64; } }
        norm([(acc[0]/seed.len() as f64) as f32, (acc[1]/seed.len() as f64) as f32, (acc[2]/seed.len() as f64) as f32])
    };
    // neighbors sharing a position with seed, filtered by dihedral vs seedavg
    let mut seedpos = BTreeSet::new();
    for i in &seed { for c in 0..3 { seedpos.insert([tris[*i][c][0].to_bits(), tris[*i][c][1].to_bits(), tris[*i][c][2].to_bits()]); } }
    let mut pool = seed.clone();
    let mut kept = 0;
    for (i, t) in tris.iter().enumerate() {
        if pool.contains(&i) { continue; }
        let mut shares = false;
        for c in 0..3 {
            if seedpos.contains(&[t[c][0].to_bits(), t[c][1].to_bits(), t[c][2].to_bits()]) { shares = true; break; }
        }
        if !shares { continue; }
        let f = fnorm(t);
        if f[0]*seedavg[0]+f[1]*seedavg[1]+f[2]*seedavg[2] >= cos_t { pool.insert(i); kept += 1; }
    }
    let avg = {
        let mut acc = [0.0f64; 3];
        for i in &pool { let f = fnorm(&tris[*i]); for d in 0..3 { acc[d] += f[d] as f64; } }
        norm([(acc[0]/pool.len() as f64) as f32, (acc[1]/pool.len() as f64) as f32, (acc[2]/pool.len() as f64) as f32])
    };
    // his
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut his = [0.0; 3];
    'outer: for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut p2, mut n2) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => p2 = p.clone(),
                        Elem::Word(w) if d.name() == 5 => n2 = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                for i in 0..p2.len() {
                    let d = ((p2[i][0]-q[0]).powi(2)+(p2[i][1]-q[1]).powi(2)+(p2[i][2]-q[2]).powi(2)).sqrt();
                    if d < 2e-4 { his = n2[i]; break 'outer; }
                }
            }
        }
    }
    let ang = |x: [f32;3], y: [f32;3]| (x[0]*y[0]+x[1]*y[1]+x[2]*y[2]).clamp(-1.0,1.0).acos().to_degrees();
    println!("seed={} kept={} pool={} poolavg=({:.4},{:.4},{:.4}) his=({:.4},{:.4},{:.4}) d_seed={:.3}deg d_pool={:.3}deg",
        seed.len(), kept, pool.len(), avg[0], avg[1], avg[2], his[0], his[1], his[2], ang(his, seedavg), ang(his, avg));
    let _ = BTreeMap::<u32,u32>::new();
}
