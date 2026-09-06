//! Dump a prefab's first static object: solid visuals/geoms/materials,
//! surface materials + triangle phys histogram, externals.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    store.add_pak("/tmp/BlueBay.pak", "660C4C156B80337E296A1034B0AA05B8").unwrap();
    store.add_pak("/tmp/current-Stadium.pak", "B773D73047A4104857722366D78D28A6").unwrap();
    let model = store.load_model(&a[1]).unwrap();
    let ext = |i: i32| model.externals.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone()).unwrap_or("?inline?".into());
    let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&model).unwrap();
    for (i, e) in pf.ents.iter().enumerate() {
        if i > 0 {
            break;
        }
        if let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() {
            let s2 = so.solid2().unwrap();
            println!("geoms: {:?}", s2.shaded_geoms.iter().map(|g| (g.visual_index, g.material_index)).collect::<Vec<_>>());
            println!("custom_materials:");
            for (mi, m) in s2.custom_materials.iter().enumerate() {
                let node = match m.node.as_ref().and_then(|r| r.inline.as_deref()) {
                    Some(mapgeom::static_item::Node::OldMaterial(om)) => format!("OldMaterial phys={} refs={:?}", om.physics, om.refs.iter().map(|r| ext(*r)).collect::<Vec<_>>()),
                    Some(n) => format!("other class 0x{:08X}", n.class_id()),
                    None => "external".into(),
                };
                println!("  mat{mi} name={:?} {node}", m.name);
            }
            println!("materials (deprec): {:?}", s2.materials.iter().map(|r| r.index).collect::<Vec<_>>());
            if let Some(sf) = so.surface() {
                println!("surface materials:");
                for sm in &sf.materials {
                    match sm {
                        mapgeom::static_item::surface::SurfMaterial::Node(r) => println!("  node {} -> {}", r.index, ext(r.index)),
                        mapgeom::static_item::surface::SurfMaterial::Id(id) => println!("  id {id}"),
                    }
                }
                println!("surface ids: {:?}", sf.material_ids);
            }
        }
    }
}
