use mapgeom::{container::Gbx, store::DataStore, tiny_assets};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = DataStore::empty();
    store.add_pak("/tmp/BlueBay.pak", tiny_assets::BLUEBAY_KEY).unwrap();
    store.add_pak("/tmp/current-Stadium.pak", tiny_assets::STADIUM_KEY).unwrap();
    for p in &a[1..] {
        let b = store.read(p).unwrap();
        let g = Gbx::parse(&b).unwrap();
        println!("### {p}\n  version {} class {:08X} nodes {} ancestor {} body {} bytes header {} bytes", g.version, g.class_id, g.num_nodes, g.ancestor_level, g.body.len(), g.user_data.len());
        println!("  folders: {:?}", g.folders);
        for r in &g.refs {
            println!("  ref node {} folder {:?} use_file {} name {}", r.node_index, r.folder, r.use_file, r.name);
        }
    }
}
