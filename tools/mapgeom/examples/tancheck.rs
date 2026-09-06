use mapgeom::static_item::vstream::Elem;
fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index.max(0) as usize;
            let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or("?".into());
            if !mat.contains("Road") && !mat.contains("SpecialFX") && !mat.contains("Technics") {
                continue;
            }
            if let Some(vref) = s2.visuals.get(vi) {
                if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                    let st = vis.stream().unwrap();
                    for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                        if let Elem::Word(w) = e {
                            if d.name() == 18 || d.name() == 20 {
                                let zero = w.iter().filter(|v| **v == 0).count();
                                let total = w.len();
                                // length distribution of decoded vectors
                                println!("{} {mat} decl{}: n={total} zero={zero}", path.rsplit('/').next().unwrap(), d.name());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}
