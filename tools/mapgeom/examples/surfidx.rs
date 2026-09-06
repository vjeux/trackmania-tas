//! Per surface-material entry: path, u16 id, tri count, tri u8-phys histogram.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    store.add_pak("/tmp/BlueBay.pak", "660C4C156B80337E296A1034B0AA05B8").unwrap();
    store.add_pak("/tmp/current-Stadium.pak", "B773D73047A4104857722366D78D28A6").unwrap();
    let model = store.load_model(&a[1]).unwrap();
    let ext = |i: i32| model.externals.iter().find(|(k, _)| *k as i32 == i).map(|(_, p)| p.clone()).unwrap_or("?inline?".into());
    let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&model).unwrap();
    for e in pf.ents.iter().take(1) {
        if let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() {
            if let Some(sf) = so.surface() {
                match &sf.surf {
                    mapgeom::static_item::surface::Surf::Mesh { triangles, .. } => {
                        use std::collections::BTreeMap;
                        let mut per: BTreeMap<i16, (usize, BTreeMap<u8, usize>)> = BTreeMap::new();
                        for t in triangles {
                            let e = per.entry(t.surface_index).or_default();
                            e.0 += 1;
                            *e.1.entry(t.material_id).or_default() += 1;
                        }
                        for (si, (n, h)) in &per {
                            let sii = (*si).max(0) as usize;
                            let mat = sf.materials.get(sii);
                            let path = match mat {
                                Some(mapgeom::static_item::surface::SurfMaterial::Node(r)) => ext(r.index),
                                _ => "?".into(),
                            };
                            let uid = sf.material_ids.get(sii).copied().unwrap_or(0);
                            println!("surfidx={si} n={n} u8phys={h:?} u16id={uid} path={path}");
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
