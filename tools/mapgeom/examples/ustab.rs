//! Single-face 1-1 spots: which (+du,-du,+dv,-dv) matches his U? Usage: ustab HIS.ITEM MINE.ITEM SUBSTR
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
    // his pos -> (N, U)
    let loadh = |path: &str| -> BTreeMap<[u32;3], Vec<([f32;3],[f32;3])>> {
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
    // my tris
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
    let r = loadh(&a[1]);
    let m = loadh(&a[2]);
    // wins per candidate, split by det sign; bit-exact counts
    let mut wbit = [[0usize; 4]; 2];
    let mut wang = [[0.0f64; 4]; 2];
    let mut wn = [0usize; 2];
    for (k, rvs) in &r {
        if rvs.len() != 1 { continue; }
        if m.get(k).map(|v| v.len()).unwrap_or(0) != 1 { continue; }
        let p = [f32::from_bits(k[0]), f32::from_bits(k[1]), f32::from_bits(k[2])];
        // incident tris (expect exactly 1 tri touching, else skip)
        let mut inc: Vec<usize> = Vec::new();
        for (i, (ps, _)) in tris.iter().enumerate() {
            for c in 0..3 {
                let d = ((ps[c][0]-p[0]).powi(2)+(ps[c][1]-p[1]).powi(2)+(ps[c][2]-p[2]).powi(2)).sqrt();
                if d < 1e-6 { inc.push(i); break; }
            }
        }
        if inc.len() != 1 { continue; }
        let (ps, us) = &tris[inc[0]];
        let e1 = [ps[1][0]-ps[0][0], ps[1][1]-ps[0][1], ps[1][2]-ps[0][2]];
        let e2 = [ps[2][0]-ps[0][0], ps[2][1]-ps[0][1], ps[2][2]-ps[0][2]];
        let (du1, dv1, du2, dv2) = (us[1][0]-us[0][0], us[1][1]-us[0][1], us[2][0]-us[0][0], us[2][1]-us[0][1]);
        let det = du1*dv2-du2*dv1;
        if det.abs() < 1e-12 { continue; }
        let rr = 1.0/det;
        let n = rvs[0].0; // HIS N as GS target
        let hu = rvs[0].1;
        // du candidate
        let tx = (e1[0]*dv2-e2[0]*dv1)*rr;
        let ty = (e1[1]*dv2-e2[1]*dv1)*rr;
        let tz = (e1[2]*dv2-e2[2]*dv1)*rr;
        let tl = (tx*tx+ty*ty+tz*tz).sqrt().max(1e-30);
        let (tx, ty, tz) = (tx/tl, ty/tl, tz/tl);
        let dd = tx*n[0]+ty*n[1]+tz*n[2];
        let udu = norm([tx-dd*n[0], ty-dd*n[1], tz-dd*n[2]]);
        // dv candidate
        let vx = (e1[0]*du2-e2[0]*du1)*rr;
        let vy = (e1[1]*du2-e2[1]*du1)*rr;
        let vz = (e1[2]*du2-e2[2]*du1)*rr;
        let vl = (vx*vx+vy*vy+vz*vz).sqrt().max(1e-30);
        let vg = [vx/vl, vy/vl, vz/vl];
        let udv = norm([n[1]*vg[2]-n[2]*vg[1], n[2]*vg[0]-n[0]*vg[2], n[0]*vg[1]-n[1]*vg[0]]);
        let cands = [udu, [-udu[0],-udu[1],-udu[2]], udv, [-udv[0],-udv[1],-udv[2]]];
        let s = if det < 0.0 { 0 } else { 1 };
        wn[s] += 1;
        for (ci, c) in cands.iter().enumerate() {
            let d = tang(hu, *c);
            wang[s][ci] += d as f64;
            if qw(*c) == qw(hu) { wbit[s][ci] += 1; }
        }
    }
    let names = ["+du", "-du", "+dv", "-dv"];
    for s in 0..2 {
        println!("{} det{}: n={} {}", a[3], if s == 0 { "<0" } else { ">0" }, wn[s],
            names.iter().enumerate().map(|(ci, nm)| format!("{nm} bit={} ({:.1}%) mean={:.3}deg", wbit[s][ci], 100.0*wbit[s][ci] as f32/wn[s].max(1) as f32, wang[s][ci]/wn[s].max(1) as f64)).collect::<Vec<_>>().join(" | "));
    }
}
