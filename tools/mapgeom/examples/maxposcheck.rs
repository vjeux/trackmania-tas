//! Is max-corner-position sorted? Usage: maxposcheck FILE SUBSTR
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
                let st = vis.stream().unwrap();
                let mut pos = Vec::new();
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    if let Elem::Float3(p) = e {
                        if d.name() == 0 { pos = p.clone(); }
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let tris: Vec<[usize; 3]> = idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0] as usize, t[1] as usize, t[2] as usize]).collect();
                for ax in 0..3 {
                    let mut sorted = true;
                    for w in tris.windows(2) {
                        let m0 = pos[w[0][0]][ax].max(pos[w[0][1]][ax]).max(pos[w[0][2]][ax]);
                        let m1 = pos[w[1][0]][ax].max(pos[w[1][1]][ax]).max(pos[w[1][2]][ax]);
                        if m1 < m0 { sorted = false; break; }
                    }
                    println!("{} ax{ax} maxpos_sorted={sorted}", mat.rsplit('\\').next().unwrap_or(&mat));
                }
                return;
            }
        }
    }
    let _ = Elem::Float3(vec![[0.0; 3]]);
}
