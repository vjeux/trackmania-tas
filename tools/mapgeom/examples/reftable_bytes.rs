use mapgeom::{container::Gbx, store::DataStore, tiny_assets, rescale::Rescale};
fn show(tag: &str, b: &[u8]) {
    let g = Gbx::parse(b).unwrap();
    println!("### {tag}: version {} class {:08X} nodes {} ancestor {} body {} header {}", g.version, g.class_id, g.num_nodes, g.ancestor_level, g.body.len(), g.user_data.len());
    println!("  folders: {:?}", g.folders);
    for r in &g.refs { println!("  ref node {} folder {:?} use_file {} name {}", r.node_index, r.folder, r.use_file, r.name); }
}
fn main() {
    let mut store = DataStore::empty();
    store.add_pak("/tmp/current-Stadium.pak", tiny_assets::STADIUM_KEY).unwrap();
    let item = "Stadium\\Items\\TunnelSupportArch16m.Item.Gbx";
    let orig = store.read(item).unwrap();
    show("original item", &orig);
    let mut rs = Rescale::new(0.5, "_half");
    let copy = tiny_assets::item_copy(&mut store, &mut rs, item, "TunnelSupportArch16m", "AC00000220.Item.Gbx").unwrap().unwrap();
    show("item copy", &copy);
    let template = store.read("Stadium\\Items\\GateSupport.Item.Gbx").unwrap();
    show("GateSupport template", &template);
    let wrap = tiny_assets::wrapper(&template, "AC00000221.Item.Gbx", "Stadium\\Media\\Prefab\\Items\\TunnelSupport\\TunnelSupportArch16m.Prefab.Gbx");
    show("wrapper", &wrap);
}
