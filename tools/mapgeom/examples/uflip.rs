//! 1-1 spots with big U angles. Usage: uflip HIS.ITEM MINE.ITEM SUBSTR [THRESH]
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
    let th: f32 = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(30.0);
    let load = |path: &str| -> BTreeMap<[u32;3], Vec<([f32;3],[f32;3],[f32;3])>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<[u32;3], Vec<([f32;3],[f32;3],[f32;3])>> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
            if stem != a[3] { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrm, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                            Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                            Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                            _ => {}
                        }
                    }
                    for i in 0..pos.len() {
                        let u = if i < tu.len() { tu[i] } else { [0.0;3] };
                        let v = if i < tv.len() { tv[i] } else { [0.0;3] };
                        out.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push((nrm[i], u, v));
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let tang = |x: [f32;3], y: [f32;3]| {
        let la = (x[0]*x[0]+x[1]*x[1]+x[2]*x[2]).sqrt().max(1e-30);
        let lb = (y[0]*y[0]+y[1]*y[1]+y[2]*y[2]).sqrt().max(1e-30);
        ((x[0]*y[0]+x[1]*y[1]+x[2]*y[2])/(la*lb)).clamp(-1.0,1.0).acos().to_degrees()
    };
    for (k, rvs) in &r {
        if rvs.len() != 1 { continue; }
        if let Some(mvs) = m.get(k) {
            if mvs.len() != 1 { continue; }
            let du = tang(rvs[0].1, mvs[0].1);
            if du > th {
                let p = [f32::from_bits(k[0]), f32::from_bits(k[1]), f32::from_bits(k[2])];
                println!("pos=({:.5},{:.5},{:.5}) dU={:.1} hisU=({:.3},{:.3},{:.3}) myU=({:.3},{:.3},{:.3}) hisN=({:.3},{:.3},{:.3}) myN=({:.3},{:.3},{:.3})",
                    p[0], p[1], p[2], du, rvs[0].1[0], rvs[0].1[1], rvs[0].1[2], mvs[0].1[0], mvs[0].1[1], mvs[0].1[2], rvs[0].0[0], rvs[0].0[1], rvs[0].0[2], mvs[0].0[0], mvs[0].0[1], mvs[0].0[2]);
            }
        }
    }
}
