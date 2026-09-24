//! Shared-edge endpoints + q membership for adjacent pairs. Usage: edgemember FILE SUBSTR HEXHEXHEX
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let qb = [u32::from_str_radix(a[3].trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(a[4].trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(a[5].trim_start_matches("0x"), 16).unwrap()];
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
    let mm = |p: &[f32;3]| (p[0].to_bits(), p[1].to_bits(), p[2].to_bits());
    // q-touching tris
    let mut qt: Vec<usize> = Vec::new();
    for (i, t) in tris.iter().enumerate() {
        for c in 0..3 {
            if mm(&t[c]) == (qb[0], qb[1], qb[2]) { qt.push(i); break; }
        }
    }
    println!("q-touching tris: {qt:?}");
    // shared edges among q-touching tris: endpoints + q membership
    for i in 0..qt.len() {
        for j in (i+1)..qt.len() {
            let (a2, b2) = (qt[i], qt[j]);
            // shared corners (exact bits)
            let mut shared: Vec<[f32;3]> = Vec::new();
            for c in 0..3 {
                for d in 0..3 {
                    if mm(&tris[a2][c]) == mm(&tris[b2][d]) { shared.push(tris[a2][c]); break; }
                }
            }
            if shared.len() >= 2 {
                let hasq = shared.iter().any(|p| mm(p) == (qb[0], qb[1], qb[2]));
                println!("tri{a2}-tri{b2}: share edge, endpoints_equal_q={hasq}");
            } else if shared.len() == 1 {
                let isq = mm(&shared[0]) == (qb[0], qb[1], qb[2]);
                println!("tri{a2}-tri{b2}: share corner only (is_q={isq})");
            }
        }
    }
    let _ = BTreeMap::<u32,u32>::new();
}
