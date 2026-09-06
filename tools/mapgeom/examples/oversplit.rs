//! Find positions where MINE has more verts than HIS (false splits). Usage: oversplit HIS.ITEM MINE.ITEM SUBSTR
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::vstream::Elem;
fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> BTreeMap<[u32; 3], Vec<Vec<u32>>> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out: BTreeMap<[u32; 3], Vec<Vec<u32>>> = BTreeMap::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            if !mat.contains(&a[3]) { continue; }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    let (mut pos, mut nrm, mut uv, mut uv1, mut tu, mut tv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
                    let mut has_uv1 = false;
                    let mut has_tan = false;
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        match e {
                            Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                            Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                            Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                            Elem::Float2(u) if d.name() == 11 => { uv1 = u.clone(); has_uv1 = true; }
                            Elem::Word(w) if d.name() == 18 => { tu = w.iter().map(|v| dec(*v)).collect(); has_tan = true; }
                            Elem::Word(w) if d.name() == 20 => tv = w.iter().map(|v| dec(*v)).collect(),
                            _ => {}
                        }
                    }
                    for i in 0..pos.len() {
                        let mut k: Vec<u32> = vec![nrm[i][0].to_bits(), nrm[i][1].to_bits(), nrm[i][2].to_bits(),
                                            uv[i][0].to_bits(), uv[i][1].to_bits()];
                        if has_uv1 && i < uv1.len() { k.push(uv1[i][0].to_bits()); k.push(uv1[i][1].to_bits()); }
                        if has_tan && i < tu.len() {
                            k.push(tu[i][0].to_bits()); k.push(tu[i][1].to_bits()); k.push(tu[i][2].to_bits());
                            k.push(tv[i][0].to_bits()); k.push(tv[i][1].to_bits()); k.push(tv[i][2].to_bits());
                        }
                        out.entry([pos[i][0].to_bits(), pos[i][1].to_bits(), pos[i][2].to_bits()]).or_default().push(k);
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    let mut shown = 0;
    for (pb, mverts) in &m {
        if let Some(rverts) = r.get(pb) {
            if mverts.len() > rverts.len() {
                let rs: BTreeSet<&Vec<u32>> = rverts.iter().collect();
                let ms: BTreeSet<&Vec<u32>> = mverts.iter().collect();
                let extra: Vec<&&Vec<u32>> = ms.difference(&rs).collect();
                // also need tri det signs; approximate: report full keys
                println!("pos {:x}{:x}{:x}: his={} mine={} extra_mine_keys:", pb[0], pb[1], pb[2], rverts.len(), mverts.len());
                for k in extra {
                    println!("   n=({:x},{:x},{:x}) uv=({:x},{:x}) restlen={}", k[0], k[1], k[2], k[3], k[4], k.len());
                }
                shown += 1;
                if shown > 8 { break; }
            }
        }
    }
    if shown == 0 { println!("{}: no oversplit exact-bit positions", a[3]); }
}
