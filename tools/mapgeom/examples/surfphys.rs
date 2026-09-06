fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let sf = so.surface().unwrap();
    let mut h: std::collections::BTreeMap<u8, usize> = std::collections::BTreeMap::new();
    match &sf.surf {
        mapgeom::static_item::surface::Surf::Mesh { triangles, .. } => {
            for t in triangles { *h.entry(t.material_id).or_default() += 1; }
        }
        _ => {}
    }
    println!("{a:?} -> {h:?}");
}
