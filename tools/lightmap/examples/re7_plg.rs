//! `re7_plg ITEM…` — the PreLightGen fields of items (RE 7).
fn main() {
    for p in std::env::args().skip(1) {
        let b = std::fs::read(&p).unwrap();
        let f = mapgeom::static_item::file::parse_file(&b).unwrap();
        let s2 = f.item.static_object().unwrap().solid2().unwrap();
        println!("{}: plg {:?}; materials_folder {:?}; visuals {}", p.rsplit('/').next().unwrap(), s2.pre_light_gen, s2.materials_folder, s2.visuals.len());
    }
}
