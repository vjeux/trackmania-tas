//! Max-sort violations. Usage: maxviol FILE SUBSTR
use mapgeom::static_item::vstream::Elem;
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
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let tris: Vec<[u32; 3]> = idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0], t[1], t[2]]).collect();
                let mut viol = 0;
                for (i, w) in tris.windows(2).enumerate() {
                    let x0 = w[0][0].max(w[0][1]).max(w[0][2]);
                    let x1 = w[1][0].max(w[1][1]).max(w[1][2]);
                    if x1 < x0 {
                        viol += 1;
                        if viol <= 8 {
                            println!("  viol@{i}: max {x0} -> {x1} tris=({},{},{})->({},{},{})",
                                w[0][0], w[0][1], w[0][2], w[1][0], w[1][1], w[1][2]);
                        }
                    }
                }
                println!("{}: ntri={} maxviol={viol}", mat.rsplit('\\').next().unwrap_or(&mat), tris.len());
                return;
            }
        }
    }
    let _ = Elem::Float3(vec![[0.0; 3]]);
}
