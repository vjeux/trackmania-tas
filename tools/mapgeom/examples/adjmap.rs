//! Shared edges among tri indices. Usage: adjmap FILE SUBSTR I0 I1 ...
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let want: Vec<usize> = a[3..].iter().map(|x| x.parse().unwrap()).collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut tris: Vec<[[f32;3];3]> = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[2] { continue; }
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
    let ek = |p: [f32;3]| (p[0].to_bits(), p[1].to_bits(), p[2].to_bits());
    for (ii, i) in want.iter().enumerate() {
        for j in &want[ii+1..] {
            // shared corners
            let mut shared = Vec::new();
            for c in 0..3 {
                for d in 0..3 {
                    if ek(tris[*i][c]) == ek(tris[*j][d]) { shared.push(c); break; }
                }
            }
            if shared.len() >= 2 {
                println!("tri{i}-tri{j}: share edge ({shared:?})");
            } else if shared.len() == 1 {
                println!("tri{i}-tri{j}: share corner only");
            }
        }
    }
    let _ = BTreeMap::<u32,u32>::new();
}
