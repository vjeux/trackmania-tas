//! Distinct vert indices at exact positions. Usage: vcount FILE SUBSTR [HX HY HZ]...
use std::collections::BTreeSet;
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut queries: Vec<[u32; 3]> = Vec::new();
    let mut i = 3;
    while i + 2 < a.len() {
        queries.push([u32::from_str_radix(a[i].trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(a[i+1].trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(a[i+2].trim_start_matches("0x"), 16).unwrap()]);
        i += 3;
    }
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if stem != a[2] { continue; }
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let st = vis.stream().unwrap();
                let mut pos: Vec<[f32; 3]> = Vec::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e { if d.name() == 0 { pos = p.clone(); } }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut hits: BTreeSet<u32> = BTreeSet::new();
                let mut nhit = 0;
                for t in idx.chunks(3) {
                    if t.len() < 3 { continue; }
                    for c in 0..3 {
                        let p = pos[t[c] as usize];
                        let bits = [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
                        if queries.is_empty() || queries.contains(&bits) { hits.insert(t[c]); nhit += 1; }
                    }
                }
                println!("visual vstream verts={} idxrefs={} distinct_idx_at_query={} refs={}", pos.len(), idx.len(), hits.len(), nhit);
            }
        }
    }
}
