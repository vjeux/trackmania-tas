use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for (gi, g) in s2.shaded_geoms.iter().enumerate() {
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or("?".into());
        let vi = g.visual_index.max(0) as usize;
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let m = vis.main.as_ref().unwrap();
                let st = vis.stream().unwrap();
                let decls: Vec<String> = st.decls.iter().map(|d| format!("{}:{}", d.name(), d.ty())).collect();
                let counts: Vec<String> = st.elems.iter().map(|e| match e { Elem::Float3(p) => p.len().to_string(), Elem::Float2(u) => u.len().to_string(), Elem::Word(w) => w.len().to_string(), _ => "?".into() }).collect();
                println!("geom{gi} mat={mat} flags=0x{:x} count={} tangents={:?} decls=[{}] lens=[{}]", m.chunk_flags, m.count, vis.tangents.as_ref().map(|(x, y)| (x.len(), y.len())), decls.join(" "), counts.join(" "));
            }
        }
    }
}
