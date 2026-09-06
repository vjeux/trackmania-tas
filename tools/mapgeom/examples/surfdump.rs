fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s = match so.shape.inline.as_deref() {
            Some(mapgeom::static_item::Node::Surface(s)) => s,
            _ => { println!("{}: no surface", path.rsplit('/').next().unwrap()); continue; }
        };
        println!("== {}", path.rsplit('/').next().unwrap());
        // surf_ids list lives in CPlugSurface.material_ids? and triangles?
        match &s.surf {
            mapgeom::static_item::surface::Surf::Mesh { vertices, triangles, .. } => {
                println!("  verts={} tris={}", vertices.len(), triangles.len());
                let mut hist = std::collections::BTreeMap::new();
                for t in triangles {
                    *hist.entry((t.material_id, t.u03, t.surface_index)).or_insert(0) += 1;
                }
                for ((m, u, si), n) in hist {
                    println!("    mat={m} u03={u} si={si} tris={n}");
                }
            }
            _ => println!("  not a mesh"),
        }
    }
}
