//! U average weight laws at 1-1 spots. Usage: uweight HIS.ITEM MINE.ITEM SUBSTR
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
fn tang(a: [f32;3], b: [f32;3]) -> f32 {
    let la = (a[0]*a[0]+a[1]*a[1]+a[2]*a[2]).sqrt().max(1e-30);
    let lb = (b[0]*b[0]+b[1]*b[1]+b[2]*b[2]).sqrt().max(1e-30);
    ((a[0]*b[0]+a[1]*b[1]+a[2]*b[2])/(la*lb)).clamp(-1.0,1.0).acos().to_degrees()
}
fn qw(v: [f32; 3]) -> u32 {
    let mut o = 0u32;
    for (k, x) in v.iter().enumerate() {
        let q = (x.clamp(-1.0, 1.0) * 511.0) as i32;
        o |= ((q & 0x3FF) as u32) << (10 * k);
    }
    o
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // my smoothed N + stored U by position (for cluster context not needed; we recompute candidates)
    let loadn = |path: &str| -> BTreeMap<[u32;3], Vec<([f32;3],[f32;3])>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<[u32;3], Vec<([f32;3],[f32;3])>> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if stem != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrm, mut tu) = (Vec::new(), Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                            Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                            _ => {}
                        }
                    }
                    for i in 0..pos.len() {
                        let u = if i < tu.len() { tu[i] } else { [0.0;3] };
                        out.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push((nrm[i], u));
                    }
                }
            }
        }
        out
    };
    // my tris (positions, uvs) for per-face du-grads
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut tris: Vec<([[f32;3];3],[[f32;2];3])> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut uv) = (Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    tris.push(([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]],
                               [uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]]));
                }
            }
        }
    }
    let r = loadn(&a[1]);
    let m = loadn(&a[2]);
    // candidates: uniform-avg, angle-avg, mag-avg, dominant-face; scored vs his U
    let (mut n, mut uok, mut gok, mut mok, mut dok, mut eok) = (0, 0, 0, 0, 0, 0);
    let (mut usum, mut gsum, mut msum, mut dsum, mut esum) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for (k, rvs) in &r {
        if rvs.len() != 1 { continue; }
        if m.get(k).map(|v| v.len()).unwrap_or(0) != 1 { continue; }
        let (hn, hu) = (rvs[0].0, rvs[0].1);
        let mn = m.get(k).unwrap()[0].0;
        // use MY smoothed N as the GS target (his N unknown to the rule; mine is close)
        let p = [f32::from_bits(k[0]), f32::from_bits(k[1]), f32::from_bits(k[2])];
        // per-face (du-grad unit GS mn, |du-grad|, corner angle)
        let mut faces: Vec<([f32;3], f64, f32)> = Vec::new();
        for (ps, us) in &tris {
            let mut ci = None;
            for c in 0..3 {
                let d = ((ps[c][0]-p[0]).powi(2)+(ps[c][1]-p[1]).powi(2)+(ps[c][2]-p[2]).powi(2)).sqrt();
                if d < 1e-6 { ci = Some(c); break; }
            }
            let ci = match ci { Some(v) => v, None => continue };
            let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
            let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
            let (du1, dv1, du2, dv2) = (us[1][0]-us[0][0], us[1][1]-us[0][1], us[2][0]-us[0][0], us[2][1]-us[0][1]);
            let det = du1*dv2-du2*dv1;
            if det.abs() < 1e-12 { continue; }
            let rr = 1.0/det;
            let tx = (e1[0]*dv2-e2[0]*dv1)*rr;
            let ty = (e1[1]*dv2-e2[1]*dv1)*rr;
            let tz = (e1[2]*dv2-e2[2]*dv1)*rr;
            let mag = ((tx*tx+ty*ty+tz*tz) as f64).sqrt().max(1e-30);
            let (tx, ty, tz) = (tx/mag as f32, ty/mag as f32, tz/mag as f32);
            let dd = tx*mn[0]+ty*mn[1]+tz*mn[2];
            let uu = norm([tx-dd*mn[0], ty-dd*mn[1], tz-dd*mn[2]]);
            // corner angle
            let o1 = (ci+1)%3;
            let o2 = (ci+2)%3;
            let v1 = [ps[o1][0]-p[0], ps[o1][1]-p[1], ps[o1][2]-p[2]];
            let v2 = [ps[o2][0]-p[0], ps[o2][1]-p[1], ps[o2][2]-p[2]];
            let l1 = (v1[0]*v1[0]+v1[1]*v1[1]+v1[2]*v1[2]).sqrt().max(1e-30);
            let l2 = (v2[0]*v2[0]+v2[1]*v2[1]+v2[2]*v2[2]).sqrt().max(1e-30);
            let an = ((v1[0]*v2[0]+v1[1]*v2[1]+v1[2]*v2[2])/(l1*l2)).clamp(-1.0,1.0).acos();
            faces.push((uu, mag, an));
        }
        if faces.len() < 2 { continue; }
        let avgw = |w: &dyn Fn(f64, f32) -> f64| {
            let mut acc = [0.0f64; 3];
            let mut s = 0.0f64;
            for (uu, mag, an) in &faces {
                let ww = w(*mag, *an);
                for d in 0..3 { acc[d] += uu[d] as f64 * ww; }
                s += ww;
            }
            norm([(acc[0]/s) as f32, (acc[1]/s) as f32, (acc[2]/s) as f32])
        };
        let au = avgw(&|_, _| 1.0);
        let gu = avgw(&|_, an| an as f64);
        let mu = avgw(&|mag, _| mag);
        // (e) raw-avg: average UNNORMALIZED du-grads, normalize, then GS vs N
        let eu = {
            let mut acc = [0.0f64; 3];
            for (ps, us) in &tris {
                let mut ci = None;
                for c in 0..3 {
                    let d = ((ps[c][0]-p[0]).powi(2)+(ps[c][1]-p[1]).powi(2)+(ps[c][2]-p[2]).powi(2)).sqrt();
                    if d < 1e-6 { ci = Some(c); break; }
                }
                if ci.is_none() { continue; }
                let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
                let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
                let (du1, dv1, du2, dv2) = (us[1][0]-us[0][0], us[1][1]-us[0][1], us[2][0]-us[0][0], us[2][1]-us[0][1]);
                let det = du1*dv2-du2*dv1;
                if det.abs() < 1e-12 { continue; }
                let rr = 1.0/det;
                acc[0] += ((e1[0]*dv2-e2[0]*dv1)*rr) as f64;
                acc[1] += ((e1[1]*dv2-e2[1]*dv1)*rr) as f64;
                acc[2] += ((e1[2]*dv2-e2[2]*dv1)*rr) as f64;
            }
            let l = (acc[0]*acc[0]+acc[1]*acc[1]+acc[2]*acc[2]).sqrt().max(1e-30);
            let (tx, ty, tz) = ((acc[0]/l) as f32, (acc[1]/l) as f32, (acc[2]/l) as f32);
            let dd = tx*mn[0]+ty*mn[1]+tz*mn[2];
            norm([tx-dd*mn[0], ty-dd*mn[1], tz-dd*mn[2]])
        };
        let (a1, a2, a3) = (tang(hu, au), tang(hu, gu), tang(hu, mu));
        let a5 = tang(hu, eu);
        // dominant face (max mag)
        let mut di = 0;
        for i in 1..faces.len() { if faces[i].1 > faces[di].1 { di = i; } }
        let duu = faces[di].0;
        let a4 = tang(hu, duu);
        // (also verify his N close to mine so GS target is fair; skip if N off >2deg)
        if tang(hn, mn) > 2.0 { continue; }
        n += 1;
        if a1 < 0.5 { uok += 1; }
        if a2 < 0.5 { gok += 1; }
        if a3 < 0.5 { mok += 1; }
        if a4 < 0.5 { dok += 1; }
        if a5 < 0.5 { eok += 1; }
        usum += a1 as f64; gsum += a2 as f64; msum += a3 as f64; dsum += a4 as f64; esum += a5 as f64;
        let _ = qw;
    }
    println!("{}: 1-1(N-ok)={n} uniform<0.5={uok} ({:.1}%) mean={:.3}deg | angle<0.5={gok} ({:.1}%) mean={:.3}deg | mag<0.5={mok} ({:.1}%) mean={:.3}deg | dominant<0.5={dok} ({:.1}%) mean={:.3}deg | rawavg<0.5={eok} ({:.1}%) mean={:.3}deg",
        a[3], 100.0*uok as f32/n.max(1) as f32, usum/n.max(1) as f64,
        100.0*gok as f32/n.max(1) as f32, gsum/n.max(1) as f64,
        100.0*mok as f32/n.max(1) as f32, msum/n.max(1) as f64,
        100.0*dok as f32/n.max(1) as f32, dsum/n.max(1) as f64,
        100.0*eok as f32/n.max(1) as f32, esum/n.max(1) as f64);
}
