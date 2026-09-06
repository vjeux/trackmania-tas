fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s2 = so.solid2().unwrap();
        let mut v = Vec::new();
        for vr in &s2.visuals {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() {
                let n = vis.main.as_ref().map(|m| m.vertex_streams.len()).unwrap_or(0);
                let idx: Vec<i32> = vis.main.as_ref().map(|m| m.vertex_streams.iter().map(|r| r.index).collect()).unwrap_or_default();
                v.push(format!("{}(vidx{}:{:?})", n, vr.index, idx));
            }
        }
        println!("{} nodes={} visuals={} streams=[{}]", path.rsplit('/').next().unwrap(), f.num_nodes, s2.visuals.len(), v.join(" "));
    }
}
