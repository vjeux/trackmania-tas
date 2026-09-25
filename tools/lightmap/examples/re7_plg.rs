//! `re7_plg --pak PAK:KEY PATH...` — the PreLightGen of stand-alone CPlugSolid2Model (.Mesh.Gbx) files: the
//! decoration meshes' chart factors (RE 7).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut i = 1;
    let mut pak: Option<String> = None;
    while i < a.len() && a[i].starts_with("--") {
        if a[i] == "--pak" { pak = Some(a[i + 1].clone()); i += 2; } else { i += 1; }
    }
    let pak = pak.expect("--pak PAK:KEY");
    let (pp, key) = pak.split_once(':').expect("PAK:KEY");
    let mut store = mapgeom::store::DataStore::empty();
    store.add_pak(pp, key).expect("pak");
    for p in &a[i..] {
        let m = match store.load_model(p) { Ok(m) => m, Err(e) => { println!("{p}: {e}"); continue } };
        if m.class_id != 0x090BB000 { println!("{p}: class {:#010x} (not a Solid2Model)", m.class_id); continue; }
        let mut lb = mapgeom::static_item::LookbackState::default();
        lb.defined_nodes.extend(m.external_indices().iter().copied());
        let mut r = mapgeom::static_item::Rd::new(&m.body, 0, lb);
        match mapgeom::static_item::solid2::CPlugSolid2Model::parse(&mut r) {
            Ok(s2) => {
                let plg = s2.pre_light_gen.as_ref();
                println!("{p}: v{} visuals {} geoms {} lights {} lodMaxDist {:?} PLG {}", s2.version, s2.visuals.len(), s2.shaded_geoms.len(), s2.lights.len(), s2.lod_max_dist,
                    plg.map(|g| format!("v{} u01 {} MeterByUv {} uv0 {:?} uv1 {:?} sprite {:?} boxes {} uvGroups {} {:?}", g.version, g.u01, g.u02, &g.u04[..4], &g.u04[4..], g.sprite_count, g.boxes.len(), g.uv_groups.len(), g.uv_groups.iter().take(6).collect::<Vec<_>>())).unwrap_or("none".into()));
            }
            Err(e) => println!("{p}: parse error {e}"),
        }
    }
}
