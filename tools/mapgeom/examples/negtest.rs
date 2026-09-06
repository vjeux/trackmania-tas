//! Try negating each incident face: does any negation match his? Usage: negtest HIS.ITEM MINE.ITEM SUBSTR
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
    let load = |path: &str| -> (BTreeMap<[u32;3], Vec<[f32;3]>>, Vec<[[f32;3];3]>) {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut map: BTreeMap<[u32;3], Vec<[f32;3]>> = BTreeMap::new();
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
                    let (mut pos, mut nrm) = (Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                            _ => {}
                        }
                    }
                    for i in 0..pos.len() {
                        map.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push(nrm[i]);
                    }
                    let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                    for t in idx.chunks(3) {
                        if t.len() < 3 { continue; }
                        tris.push([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
                    }
                }
            }
        }
        (map, tris)
    };
    let (r, _) = load(&a[1]);
    let (m, tris) = load(&a[2]);
    let ang = |x: [f32;3], y: [f32;3]| (x[0]*y[0]+x[1]*y[1]+x[2]*y[2]).clamp(-1.0,1.0).acos().to_degrees();
    let (mut tot, mut plain_ok, mut neg_ok, mut bothbad) = (0, 0, 0, 0);
    for (k, rvs) in &r {
        if rvs.len() != 1 { continue; }
        if m.get(k).map(|v| v.len()).unwrap_or(0) != 1 { continue; }
        let p = [f32::from_bits(k[0]), f32::from_bits(k[1]), f32::from_bits(k[2])];
        let mut fns: Vec<[f32;3]> = Vec::new();
        for t in &tris {
            for ci in 0..3 {
                let d = ((t[ci][0]-p[0]).powi(2)+(t[ci][1]-p[1]).powi(2)+(t[ci][2]-p[2]).powi(2)).sqrt();
                if d > 1e-6 { continue; }
                let e1 = [t[1][0]-t[0][0], t[1][1]-t[0][1], t[1][2]-t[0][2]];
                let e2 = [t[2][0]-t[0][0], t[2][1]-t[0][1], t[2][2]-t[0][2]];
                let cr = [e1[1]*e2[2]-e1[2]*e2[1], e1[2]*e2[0]-e1[0]*e2[2], e1[0]*e2[1]-e1[1]*e2[0]];
                let l = (cr[0]*cr[0]+cr[1]*cr[1]+cr[2]*cr[2]).sqrt().max(1e-30);
                fns.push([cr[0]/l, cr[1]/l, cr[2]/l]);
                break;
            }
        }
        if fns.len() < 2 { continue; }
        tot += 1;
        let plain = {
            let mut acc = [0.0f64; 3];
            for f in &fns { for d in 0..3 { acc[d] += f[d] as f64; } }
            norm([(acc[0]/fns.len() as f64) as f32, (acc[1]/fns.len() as f64) as f32, (acc[2]/fns.len() as f64) as f32])
        };
        if ang(rvs[0], plain) < 0.3 { plain_ok += 1; continue; }
        // try single negations
        let mut hit = false;
        for j in 0..fns.len() {
            let mut acc = [0.0f64; 3];
            for (i, f) in fns.iter().enumerate() {
                let s = if i == j { -1.0 } else { 1.0 };
                for d in 0..3 { acc[d] += s * f[d] as f64; }
            }
            let cand = norm([(acc[0]/fns.len() as f64) as f32, (acc[1]/fns.len() as f64) as f32, (acc[2]/fns.len() as f64) as f32]);
            if ang(rvs[0], cand) < 0.3 { hit = true; break; }
        }
        if hit { neg_ok += 1; } else { bothbad += 1; }
    }
    println!("{}: 1-1={tot} plain_ok={plain_ok} neg1_ok={neg_ok} neither={bothbad}", a[3]);
}
