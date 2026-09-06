//! Strip continuity: shared verts between consecutive tris. Usage: stripcont FILE SUBSTR
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
                let (mut share2, mut share1, mut share0) = (0, 0, 0);
                for w in tris.windows(2) {
                    let mut sh = 0;
                    for a2 in w[0] {
                        for b in w[1] {
                            if a2 == b { sh += 1; }
                        }
                    }
                    match sh {
                        2 => share2 += 1,
                        1 => share1 += 1,
                        _ => share0 += 1,
                    }
                }
                println!("{}: ntri={} share2(edge)={} share1={} share0={}",
                    mat.rsplit('\\').next().unwrap_or(&mat), tris.len(), share2, share1, share0);
                return;
            }
        }
    }
}
