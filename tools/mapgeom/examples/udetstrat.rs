//! Stratify U-formula agreement by |det|. Usage: udetstrat HIS.ITEM MINE.ITEM SUBSTR
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
fn qw(v: [f32; 3]) -> u32 {
    let mut o = 0u32;
    for (k, x) in v.iter().enumerate() {
        let q = (x.clamp(-1.0, 1.0) * 511.0) as i32;
        o |= ((q & 0x3FF) as u32) << (10 * k);
    }
    o
}
fn ang(a: [f32; 3], b: [f32; 3]) -> f32 {
    let la = (a[0]*a[0]+a[1]*a[1]+a[2]*a[2]).sqrt().max(1e-30);
    let lb = (b[0]*b[0]+b[1]*b[1]+b[2]*b[2]).sqrt().max(1e-30);
    ((a[0]*b[0]+a[1]*b[1]+a[2]*b[2])/(la*lb)).clamp(-1.0,1.0).acos().to_degrees()
}
struct Corner { p: [f32;3], sn: [f32;3], e1: [f32;3], e2: [f32;3], du1: f32, dv1: f32, du2: f32, dv2: f32 }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut mine: Vec<Corner> = Vec::new();
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut nrm, mut uv) = (Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        _ => {}
                    }
                }
                if pos.len() != nrm.len() || pos.len() != uv.len() { continue; }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    let ps = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    let us = [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]];
                    let ns = [nrm[t[0] as usize], nrm[t[1] as usize], nrm[t[2] as usize]];
                    let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                    let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                    let (du1, dv1, du2, dv2) = (us[1][0]-us[0][0], us[1][1]-us[0][1], us[2][0]-us[0][0], us[2][1]-us[0][1]);
                    for k in 0..3 {
                        mine.push(Corner { p: ps[k], sn: ns[k], e1, e2, du1, dv1, du2, dv2 });
                    }
                }
            }
        }
    }
    let mut his: Vec<([f32;3],[f32;3])> = Vec::new();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut tu) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                for i in 0..pos.len() {
                    if i < tu.len() { his.push((pos[i], tu[i])); }
                }
            }
        }
    }
    let mut grid: BTreeMap<(i64,i64,i64), Vec<usize>> = BTreeMap::new();
    for (j, (p, _)) in his.iter().enumerate() {
        grid.entry(((p[0]*5000.0).floor() as i64, (p[1]*5000.0).floor() as i64, (p[2]*5000.0).floor() as i64)).or_default().push(j);
    }
    let edges = [1e-9f32, 1e-7, 1e-5, 1e-3];
    let mut bins: [(usize, f64, usize, f64, usize); 5] = [(0, 0.0, 0, 0.0, 0); 5];
    let mut skipped = 0;
    for c in &mine {
        let sdet = c.du1*c.dv2-c.du2*c.dv1;
        let det = sdet.abs();
        if det < 1e-12 { skipped += 1; continue; }
        let cc = ((c.p[0]*5000.0).floor() as i64, (c.p[1]*5000.0).floor() as i64, (c.p[2]*5000.0).floor() as i64);
        let mut best = (1e9f32, usize::MAX);
        for dx in -1..=1 { for dy in -1..=1 { for dz in -1..=1 {
            if let Some(js) = grid.get(&(cc.0+dx, cc.1+dy, cc.2+dz)) {
                for &j in js {
                    let d = ((c.p[0]-his[j].0[0]).powi(2)+(c.p[1]-his[j].0[1]).powi(2)+(c.p[2]-his[j].0[2]).powi(2)).sqrt();
                    if d < best.0 { best = (d, j); }
                }
            }
        }}}
        if best.0 > 2e-4 || best.1 == usize::MAX { skipped += 1; continue; }
        let hu = his[best.1].1;
        let r = 1.0/sdet;
        let tx = (c.e1[0]*c.dv2-c.e2[0]*c.dv1)*r;
        let ty = (c.e1[1]*c.dv2-c.e2[1]*c.dv1)*r;
        let tz = (c.e1[2]*c.dv2-c.e2[2]*c.dv1)*r;
        let tl = (tx*tx+ty*ty+tz*tz).sqrt().max(1e-30);
        let (tx, ty, tz) = (tx/tl, ty/tl, tz/tl);
        let dd = tx*c.sn[0]+ty*c.sn[1]+tz*c.sn[2];
        let udu = norm([tx-dd*c.sn[0], ty-dd*c.sn[1], tz-dd*c.sn[2]]);
        let vx = (c.e1[0]*c.du2-c.e2[0]*c.du1)*r;
        let vy = (c.e1[1]*c.du2-c.e2[1]*c.du1)*r;
        let vz = (c.e1[2]*c.du2-c.e2[2]*c.du1)*r;
        let vl = (vx*vx+vy*vy+vz*vz).sqrt().max(1e-30);
        let vg = [vx/vl, vy/vl, vz/vl];
        let udv = norm([c.sn[1]*vg[2]-c.sn[2]*vg[1], c.sn[2]*vg[0]-c.sn[0]*vg[2], c.sn[0]*vg[1]-c.sn[1]*vg[0]]);
        let b = if det < edges[0] {0} else if det < edges[1] {1} else if det < edges[2] {2} else if det < edges[3] {3} else {4};
        bins[b].0 += 1;
        bins[b].1 += ang(udu, hu) as f64;
        if qw(udu) == qw(hu) { bins[b].2 += 1; }
        bins[b].3 += ang(udv, hu) as f64;
        if qw(udv) == qw(hu) { bins[b].4 += 1; }
    }
    let names = ["<1e-9", "1e-9-1e-7", "1e-7-1e-5", "1e-5-1e-3", ">=1e-3"];
    println!("{}: skipped={skipped}", a[3]);
    for b in 0..5 {
        let n = bins[b].0 as f64;
        if n == 0.0 { println!("   {:>10}: n=0", names[b]); continue; }
        println!("   {:>10}: n={} du mean={:.3}deg bit={} ({:.1}%) | dv mean={:.3}deg bit={} ({:.1}%)",
            names[b], bins[b].0, bins[b].1/n, bins[b].2, 100.0*bins[b].2 as f32/n as f32,
            bins[b].3/n, bins[b].4, 100.0*bins[b].4 as f32/n as f32);
    }
}
