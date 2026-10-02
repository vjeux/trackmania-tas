//! `re18_species PAK:KEY[,…] ITEM_PATH…` — the species list of a vegetation item (mapgeom::veget::item_species) and each species'
//! PreLightGen words (the kind-0 group key) (RE 18, 2026-10-01).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    for pk in a[1].split(',') { let (pp, key) = pk.rsplit_once(':').expect("PAK:KEY"); store.add_pak(pp, key).expect("pak"); }
    for p in &a[2..] {
        match mapgeom::veget::item_species(&mut store, p) {
            Err(e) => println!("{p}: {e}"),
            Ok(list) => {
                println!("{p}: {} species", list.len());
                for s in &list {
                    let plg = store.read(s).ok().and_then(|b| mapgeom::static_item::legacy_plg::veget_tree_prelight(&b).ok().flatten());
                    let tm = mapgeom::veget::parse_tree_model(&mut store, s).ok();
                    println!("  {s}: PLG {}{}", plg.as_ref().map(|p| format!("u01 {} MeterByUv {} uv {:?}", p.u01, p.u02, p.u04)).unwrap_or_else(|| "none".into()), tm.map(|m| format!("; scale_var01 {} rotxz {} randY {} box {:?}", m.scale_var01, m.angle_max_rot_xz_deg, m.enable_random_rotation_y, m.lightmap_record_box())).unwrap_or_default());
                }
            }
        }
    }
}
