//! U vs V clusters. Usage: uvclusters HIS.ITEM SUBSTR
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
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        if !mat.contains(&a[2]) { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let (mut pos, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new());
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Word(w) if d.name() == 18 => tu = w.iter().map(|v| dec(*v)).collect(),
                        Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                        _ => {}
                    }
                }
                if pos.len() != tu.len() || pos.is_empty() || tv.is_empty() { continue; }
                let mut bu: BTreeMap<[u32; 3], Vec<[f32; 3]>> = BTreeMap::new();
                let mut bv: BTreeMap<[u32; 3], Vec<[f32; 3]>> = BTreeMap::new();
                for i in 0..pos.len() {
                    bu.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push(tu[i]);
                    bv.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push(tv[i]);
                }
                let count = |b: &BTreeMap<[u32; 3], Vec<[f32; 3]>>| {
                    let mut tot = 0;
                    for (_, vs) in b {
                        let mut d: Vec<[f32; 3]> = Vec::new();
                        for v in vs {
                            if !d.iter().any(|x| (x[0]-v[0]).abs() < 1e-7 && (x[1]-v[1]).abs() < 1e-7 && (x[2]-v[2]).abs() < 1e-7) {
                                d.push(*v);
                            }
                        }
                        tot += d.len().max(1);
                    }
                    tot
                };
                println!("{}: |pos+U|={} |pos+V|={} stream={}", a[2], count(&bu), count(&bv), pos.len());
                return;
            }
        }
    }
}
