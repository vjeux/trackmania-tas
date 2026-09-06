//! Raw per-face U0 agreement at 1-1 agree-spots. Usage: uagree HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn sub(a: [f32;3], b: [f32;3]) -> [f32;3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn norm(v: [f32;3]) -> [f32;3] {
    let l = (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt().max(1e-30);
    [v[0]/l, v[1]/l, v[2]/l]
}
fn qpack(u: [f32;3]) -> u32 {
    let mut o = 0u32;
    for (k, x) in u.iter().enumerate() {
        let q = (x.clamp(-1.0, 1.0) * 511.0) as i32;
        o |= ((q & 0x3FF) as u32) << (10 * k);
    }
    o
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
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
    // 1-1 positions (mine): need his too; approximate with mine-only 1-vert positions? Use mine verts.
    let data = std::fs::read(&a[2]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut vc: BTreeMap<[u32;3], usize> = BTreeMap::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[3] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 {
                            for q in p { *vc.entry([q[0].to_bits(), q[1].to_bits(), q[2].to_bits()]).or_insert(0) += 1; }
                        }
                    }
                }
            }
        }
    }
    // U0 = du-VPRIM from FACE normal (cross), per tri; at 1-vert mine positions, check quantized agreement of U0 across incident tris
    let (mut agree, mut tot) = (0, 0);
    for (k, c) in &vc {
        if *c != 1 { continue; }
        let p = [f32::from_bits(k[0]), f32::from_bits(k[1]), f32::from_bits(k[2])];
        let mut u0s: Vec<u32> = Vec::new();
        for (ps, us) in &tris {
            let mut hit = false;
            for ci in 0..3 {
                let d = ((ps[ci][0]-p[0]).powi(2)+(ps[ci][1]-p[1]).powi(2)+(ps[ci][2]-p[2]).powi(2)).sqrt();
                if d < 1e-6 { hit = true; break; }
            }
            if !hit { continue; }
            let e1 = sub(ps[1], ps[0]);
            let e2 = sub(ps[2], ps[0]);
            let (du1, dv1, du2, dv2) = (us[1][0]-us[0][0], us[1][1]-us[0][1], us[2][0]-us[0][0], us[2][1]-us[0][1]);
            let det = du1*dv2-du2*dv1;
            if det.abs() < 1e-12 { continue; }
            let r = 1.0/det;
            // face normal
            let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
            let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
            let fn_ = [cr[0]/l, cr[1]/l, cr[2]/l];
            let tx = (e1[0]*dv2-e2[0]*dv1)*r;
            let ty = (e1[1]*dv2-e2[1]*dv1)*r;
            let tz = (e1[2]*dv2-e2[2]*dv1)*r;
            let tl = (tx*tx+ty*ty+tz*tz).sqrt().max(1e-30);
            let tg = [tx/tl, ty/tl, tz/tl];
            let dd = tg[0]*fn_[0]+tg[1]*fn_[1]+tg[2]*fn_[2];
            let ou = norm([tg[0]-dd*fn_[0], tg[1]-dd*fn_[1], tg[2]-dd*fn_[2]]);
            u0s.push(qpack(ou));
        }
        if u0s.len() < 2 { continue; }
        tot += 1;
        if u0s.iter().all(|x| *x == u0s[0]) { agree += 1; }
    }
    println!("{}: 1-vert mine positions with >=2 tris: {tot}, raw-U0-quantized-agree: {agree}", a[3]);
}
