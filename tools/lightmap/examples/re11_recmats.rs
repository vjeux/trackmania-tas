//! `re11_recmats MAP --pak F:K… [--collection C]` — per distinct (prefab, entity) of the map's records: the geoms' material links and the water test (RE 11).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let mut store = mapgeom::store::DataStore::empty();
    let mut it = a.iter();
    while let Some(x) = it.next() {
        if x == "--pak" {
            let spec = it.next().expect("--pak FILE:KEY");
            let (pp, key) = spec.rsplit_once(':').expect("--pak FILE:KEY");
            store.add_pak(pp, key).unwrap_or_else(|e| panic!("{pp}: {e}"));
        }
    }
    let coll = f("--collection").unwrap_or_else(|| "Stadium".into());
    let opts = lightmap::records::BuildOpts { collection: coll.clone(), zone: None, kept: None, tile_level: None, yoff: None, grid: None, items_3d: false, ghost_marks: false, no_block_cells: false, clip_order_sim: false, face_order: None, one_class: Vec::new() };
    let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
    let mr = lightmap::records::build_map_records(&a[1], &scene, &mut store, &opts).expect("records");
    let mut seen: std::collections::BTreeMap<(String, usize), (usize, &'static str, [f32; 3])> = Default::default();
    let mut no_mesh: std::collections::BTreeMap<&'static str, usize> = Default::default();
    for r in &mr.recs {
        match &r.mesh {
            Some(m) => { let e = seen.entry((m.prefab.clone(), m.entity)).or_insert((0, r.class, [m.xf[9], m.xf[10], m.xf[11]])); e.0 += 1; }
            None => *no_mesh.entry(r.class).or_default() += 1,
        }
    }
    println!("records without a mesh source: {no_mesh:?}");
    for ((prefab, ent), (n, class, t)) in &seen {
        let pm = store.load_model(prefab).unwrap();
        let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm).unwrap();
        let Some(e) = pf.ents.get(*ent) else { println!("{prefab} ent {ent}: no entity"); continue };
        let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() else { println!("{prefab} ent {ent} ×{n} ({class}): not a static object"); continue };
        let Some(s2) = so.solid2() else { println!("{prefab} ent {ent} ×{n} ({class}): no solid2"); continue };
        let geoms: Vec<String> = s2.shaded_geoms.iter().map(|sg| { let l = lightmap::waterid::geom_material_link(s2, sg.material_index, &pm.externals); format!("v{} lod {} mat {} {:?} water {}", sg.visual_index, sg.lod_mask, sg.material_index, l, lightmap::waterid::is_water_material(&mut store, &l)) }).collect();
        println!("{prefab} ent {ent} ×{n} ({class}, first at {t:?}): materials {} custom {} ids {}: {}", s2.materials.len(), s2.custom_materials.len(), s2.material_ids.len(), geoms.join(" | "));
    }
}
