//! Per-visual and collision bounds of a static item. Usage: itembounds ITEM...
use mapgeom::static_item::vstream::Elem;
fn main() {
    for p in std::env::args().skip(1) {
        let data = std::fs::read(&p).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        println!("== {}", p.rsplit('/').next().unwrap());
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").rsplit('\\').next().unwrap_or("").to_string()).unwrap_or("?".into());
            let Some(vref) = s2.visuals.get(vi) else { continue };
            let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() else { continue };
            let st = vis.stream().unwrap();
            for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                if let Elem::Float3(pos) = e { if d.name() == 0 {
                    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for v in pos { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
                    println!("  {:<24} {:>5} verts  x {:7.2}..{:7.2}  y {:7.2}..{:7.2}  z {:7.2}..{:7.2}", mat, pos.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
                } }
            }
        }
        if let Some(sf) = so.surface() {
            if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles, .. } = &sf.surf {
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for v in vertices { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
                println!("  collision {} verts {} tris  x {:7.2}..{:7.2}  y {:7.2}..{:7.2}  z {:7.2}..{:7.2}", vertices.len(), triangles.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
            }
        }
    }
}
