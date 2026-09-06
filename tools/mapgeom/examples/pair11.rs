//! Dump 1-1 pairs (his vs my U/V/N). Usage: pair11 HIS.ITEM MINE.ITEM SUBSTR [N]
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
    let n: usize = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(6);
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
    let mut shown = 0;
    for (k, rvs) in &r {
        if rvs.len() != 1 { continue; }
        if let Some(mvs) = m.get(k) {
            if mvs.len() != 1 { continue; }
            let ((rn, ru, rv), (mn, mu, mv)) = (rvs[0], mvs[0]);
            println!("HIS N=({:.3},{:.3},{:.3}) U=({:.3},{:.3},{:.3}) V=({:.3},{:.3},{:.3})",
                rn[0],rn[1],rn[2], ru[0],ru[1],ru[2], rv[0],rv[1],rv[2]);
            println!("MYN N=({:.3},{:.3},{:.3}) U=({:.3},{:.3},{:.3}) V=({:.3},{:.3},{:.3})",
                mn[0],mn[1],mn[2], mu[0],mu[1],mu[2], mv[0],mv[1],mv[2]);
            shown += 1;
            if shown >= n { break; }
        }
    }
}
