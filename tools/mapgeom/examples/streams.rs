use mapgeom::static_item::vstream::Elem;
fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        println!("== {}", path.rsplit('/').next().unwrap());
        for (gi, g) in s2.shaded_geoms.iter().enumerate() {
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or("?".into());
            let vi = g.visual_index.max(0) as usize;
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let main = vis.main.as_ref().unwrap();
                    let mut ss = Vec::new();
                    for r in &main.vertex_streams {
                        match r.inline.as_deref() {
                            Some(mapgeom::static_item::Node::VertexStream(st)) => {
                                let dd: Vec<String> = st.decls.iter().map(|d| format!("{}:{}", d.name(), d.ty())).collect();
                                ss.push(format!("idx{}[{}]", r.index, dd.join(" ")));
                            }
                            _ => ss.push(format!("idx{}(null)", r.index)),
                        }
                    }
                    println!("  geom{gi} {mat}: {}", ss.join(" | "));
                }
            }
        }
    }
}
