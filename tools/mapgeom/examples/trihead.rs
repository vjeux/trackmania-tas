//! First N tris of a visual. Usage: trihead FILE SUBSTR N
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a[3].parse().unwrap();
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
                println!("== {} ntri={}", mat.rsplit('\\').next().unwrap_or(&mat), idx.len()/3);
                for (ti, t) in idx.chunks(3).enumerate().take(n) {
                    if t.len() < 3 { continue; }
                    println!("  tri{ti}: idx=({},{},{}) cen=[{:.3},{:.3},{:.3}]",
                        t[0], t[1], t[2],
                        (pos[t[0] as usize][0]+pos[t[1] as usize][0]+pos[t[2] as usize][0])/3.0,
                        (pos[t[0] as usize][1]+pos[t[1] as usize][1]+pos[t[2] as usize][1])/3.0,
                        (pos[t[0] as usize][2]+pos[t[1] as usize][2]+pos[t[2] as usize][2])/3.0);
                }
                return;
            }
        }
    }
}
