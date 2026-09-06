//! Minimal: faces at q, uniform avg, his/mine N, angles. Usage: minavg HIS.ITEM MINE.ITEM SUBSTR X Y Z
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
    let q: [f32;3] = [a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap()];
    // faces from MINE
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut fns: Vec<[f32;3]> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if mat.rsplit('\\').next().unwrap_or(&mat) != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos = Vec::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e { if d.name() == 0 { pos = p.clone(); } }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let mut hit = false;
                    for ci in 0..3 {
                        let d = ((ps[ci][0]-q[0]).powi(2)+(ps[ci][1]-q[1]).powi(2)+(ps[ci][2]-q[2]).powi(2)).sqrt();
                        if d < 1e-6 { hit = true; break; }
                    }
                    if !hit { continue; }
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                    let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt();
                    println!("face fn=({:.6},{:.6},{:.6})", cr[0]/l, cr[1]/l, cr[2]/l);
                    fns.push([cr[0]/l, cr[1]/l, cr[2]/l]);
                }
            }
        }
    }
    let n = fns.len() as f64;
    let mut acc = [0.0f64; 3];
    for f in &fns { for d in 0..3 { acc[d] += f[d] as f64; } }
    let l = (acc[0]*acc[0]+acc[1]*acc[1]+acc[2]*acc[2]).sqrt();
    let un = [(acc[0]/l) as f32, (acc[1]/l) as f32, (acc[2]/l) as f32];
    println!("nfaces={} uniform=({:.6},{:.6},{:.6})", fns.len(), un[0], un[1], un[2]);
    // his + mine stored
    for (path, tag) in [(&a[1], "HIS"), (&a[2], "MINE")] {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        'outer: for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            if mat.rsplit('\\').next().unwrap_or(&mat) != a[3] { continue; }
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
                        if d < 2e-4 {
                            let nn = n2[i];
                            let la = (nn[0]*nn[0]+nn[1]*nn[1]+nn[2]*nn[2]).sqrt().max(1e-30);
                            let lb = (un[0]*un[0]+un[1]*un[1]+un[2]*un[2]).sqrt().max(1e-30);
                            let dot = ((nn[0]*un[0]+nn[1]*un[1]+nn[2]*un[2])/(la*lb)).clamp(-1.0,1.0);
                            println!("{tag} n=({:.6},{:.6},{:.6}) |n|={:.5} d_uniform={:.4}deg", nn[0], nn[1], nn[2], la, dot.acos().to_degrees());
                        }
                    }
                    break 'outer;
                }
            }
        }
    }
    let _ = BTreeMap::<u32,u32>::new();
}
