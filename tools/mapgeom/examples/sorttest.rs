//! Stable-sort my tris by max vertex; compare order to his. Usage: sorttest HIS.ITEM MINE.ITEM SUBSTR
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let load = |path: &str| -> (Vec<[(i32, i32, i32); 3]>, Vec<[u32; 3]>) {
        // (tri keys in order, tri max-vertex (mine only))
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut keys = Vec::new();
        let mut maxv = Vec::new();
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
                        keys.push(k);
                        maxv.push([t[0], t[1], t[2]]);
                    }
                }
            }
        }
        (keys, maxv)
    };
    let (rk, _) = load(&a[1]);
    let (mk2, mv) = load(&a[2]);
    // stable sort my tris by max vertex
    let mut order: Vec<usize> = (0..mk2.len()).collect();
    order.sort_by_key(|&i| mv[i][0].max(mv[i][1]).max(mv[i][2]));
    let sorted: Vec<[(i32, i32, i32); 3]> = order.iter().map(|&i| mk2[i]).collect();
    // positional exact matches (handle duplicates loosely: exact sequence match)
    let mut exact = 0;
    for (i, k) in rk.iter().enumerate() {
        if i < sorted.len() && sorted[i] == *k {
            exact += 1;
        }
    }
    println!("{}: ntri={} sorted-by-max positional_exact={} ({:.1}%)", a[3], rk.len(), exact, 100.0*exact as f32/rk.len() as f32);
}
