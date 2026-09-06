//! Compare averaging rules at 1-1 positions. Usage: normavg HIS.ITEM MINE.ITEM SUBSTR [N]
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
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(5);
    // my 1-1 positions + my N + his N
    let load = |path: &str| -> BTreeMap<[u32;3], Vec<[f32;3]>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<[u32;3], Vec<[f32;3]>> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if stem != a[3] { continue; }
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
                    for i in 0..pos.len() {
                        out.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push(nrm[i]);
                    }
                }
            }
        }
        out
    };
    // my tris (positions only) for face normals + corner angles
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
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut shown = 0;
    for (k, rvs) in &r {
        if rvs.len() != 1 { continue; }
        let mvs = match m.get(k) { Some(v) if v.len() == 1 => v, _ => continue };
        // incident tris (any corner within 1um)
        let p = [f32::from_bits(k[0]), f32::from_bits(k[1]), f32::from_bits(k[2])];
        let mut faces: Vec<([f32;3], f32, f32)> = Vec::new(); // (face normal, area, corner angle at p)
        for t in &tris {
            for ci in 0..3 {
                let d = ((t[ci][0]-p[0]).powi(2)+(t[ci][1]-p[1]).powi(2)+(t[ci][2]-p[2]).powi(2)).sqrt();
                if d > 1e-6 { continue; }
                let e1 = [t[1][0]-t[0][0], t[1][1]-t[0][1], t[1][2]-t[0][2]];
                let e2 = [t[2][0]-t[0][0], t[2][1]-t[0][1], t[2][2]-t[0][2]];
                let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                // corner angle at ci
                let a1 = [t[(ci+1)%3][0]-t[ci][0], t[(ci+1)%3][1]-t[ci][1], t[(ci+1)%3][2]-t[ci][2]];
                let a2 = [t[(ci+2)%3][0]-t[ci][0], t[(ci+2)%3][1]-t[ci][1], t[(ci+2)%3][2]-t[ci][2]];
                let l1 = (a1[0]*a1[0]+a1[1]*a1[1]+a1[2]*a1[2]).sqrt().max(1e-30);
                let l2 = (a2[0]*a2[0]+a2[1]*a2[1]+a2[2]*a2[2]).sqrt().max(1e-30);
                let ang = ((a1[0]*a2[0]+a1[1]*a2[1]+a1[2]*a2[2])/(l1*l2)).clamp(-1.0,1.0).acos();
                faces.push(([cr[0]/l, cr[1]/l, cr[2]/l], l/2.0, ang));
                break;
            }
        }
        if faces.len() < 2 { continue; }
        // averages
        let mut acc_u = [0.0f64; 3];
        let mut acc_a = [0.0f64; 3];
        let mut acc_w = [0.0f64; 3];
        let (mut sa, mut sw) = (0.0f64, 0.0f64);
        for (fn_, ar, an) in &faces {
            for d in 0..3 { acc_u[d] += fn_[d] as f64; acc_a[d] += fn_[d] as f64 * *ar as f64; acc_w[d] += fn_[d] as f64 * *an as f64; }
            sa += *ar as f64; sw += *an as f64;
        }
        let nu = norm([acc_u[0] as f32, acc_u[1] as f32, acc_u[2] as f32]);
        let na = norm([(acc_a[0]/sa) as f32, (acc_a[1]/sa) as f32, (acc_a[2]/sa) as f32]);
        let nw = norm([(acc_w[0]/sw) as f32, (acc_w[1]/sw) as f32, (acc_w[2]/sw) as f32]);
        let (rn, mn) = (rvs[0], mvs[0]);
        let ang = |x: [f32;3], y: [f32;3]| (x[0]*y[0]+x[1]*y[1]+x[2]*y[2]).clamp(-1.0,1.0).acos().to_degrees();
        // only show where mine is off (>0.5deg from his)
        if ang(rn, mn) < 0.5 { continue; }
        println!("pos=({:.5},{:.5},{:.5}) nfaces={} his=({:.4},{:.4},{:.4}) mine=({:.4},{:.4},{:.4})",
            p[0], p[1], p[2], faces.len(), rn[0], rn[1], rn[2], mn[0], mn[1], mn[2]);
        println!("   d_his_uniform={:.3}deg d_his_area={:.3}deg d_his_angle={:.3}deg | d_mine_uniform={:.3}deg", ang(rn, nu), ang(rn, na), ang(rn, nw), ang(mn, nu));
        shown += 1;
        if shown >= n { break; }
    }
    if shown == 0 { println!("{}: no off-1-1 spots (all within 0.5deg)", a[3]); }
}
