//! Tri counts per material. Usage: tricount FILE
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|mm| mm.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let n = vis.index_buffer.as_ref().map(|b| b.indices.len()).unwrap_or(0) / 3;
                *m.entry(stem).or_insert(0) += n;
            }
        }
    }
    for (k, v) in &m { println!("{k} {v}"); }
}
