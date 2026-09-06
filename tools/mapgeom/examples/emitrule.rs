//! Test tri emission rules. Usage: emitrule HIS.ITEM MINE.BAKED SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i64, i64, i64) {
    ((p[0]*100.0).round() as i64, (p[1]*100.0).round() as i64, (p[2]*100.0).round() as i64)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // his tri order (keys)
    let load = |path: &str| -> Vec<[(i64, i64, i64); 3]> {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut out = Vec::new();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
            if !mat.contains(&a[3]) { continue; }
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
                        let mut k = [mk(&pos[t[0] as usize]), mk(&pos[t[1] as usize]), mk(&pos[t[2] as usize])];
                        k.sort();
                        out.push(k);
                    }
                }
            }
        }
        out
    };
    let r = load(&a[1]);
    let m = load(&a[2]);
    println!("{}: his_ntri={} my_ntri={}", a[3], r.len(), m.len());
    // Candidate (a): stable sort my tris by ... need my vertex indices (don't have; use position rank?).
    // (My tris are in face order; his in max-vertex order. To test rules I need vertex numbering.
    // Approximate: rank my positions by first-appearance (face order), sort my tris by max rank, compare.)
    // Build my position ranks (face order)
    // (Need my corners in face order; m is already face-ordered tris (bake emits face order). Reconstruct positions? Only have keys (10cm cells, coarse). Too coarse for ranks.)
    println!("emitrule: needs finer keys; use mm (1mm) instead");
}
