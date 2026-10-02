//! `re18_variants PAK:KEY[,…] ITEM_PATH…` — an NPlugItem::SVariantList's variants (tags, model path) and, for each VegetTreeModel,
//! whether it carries a PreLightGen (legacy_plg) and its PLG numbers — the group key of the kind-0 records (RE 18, 2026-10-01).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    for pk in a[1].split(',') { let (pp, key) = pk.rsplit_once(':').expect("PAK:KEY"); store.add_pak(pp, key).expect("pak"); }
    for gp in &a[2..] {
        let m = match store.load_model(gp) { Ok(m) => m, Err(e) => { println!("{gp}: {e}"); continue } };
        let mut lb = mapgeom::static_item::LookbackState::default();
        lb.defined_nodes.extend(m.external_indices().iter().copied());
        let mut r = mapgeom::static_item::Rd::new(&m.body, 0, lb);
        let item = match mapgeom::static_item::item::CGameItemModel::parse(&mut r) { Ok(i) => i, Err(e) => { println!("{gp}: parse: {e}"); continue } };
        let Some(mc) = item.model() else { println!("{gp}: no model"); continue };
        match mc.entity_model.inline.as_deref() {
            Some(mapgeom::static_item::Node::VariantList(vl)) => {
                println!("{gp}: {} variants", vl.variants.len());
                for (i, v) in vl.variants.iter().enumerate() {
                    let model = m.externals.iter().find(|(k, _)| *k as i32 == v.model.index).map(|(_, p)| p.clone()).unwrap_or_else(|| format!("node {}", v.model.index));
                    let plg = match store.load_model(&model) { Ok(vm) => match mapgeom::static_item::legacy_plg::veget_tree_prelight(&vm.body) { Ok(Some(p)) => format!("PLG yes: u01 {} u02 {} u04 {:?}", p.u01, p.u02, p.u04), Ok(None) => "PLG none".into(), Err(e) => format!("plg err {e}") }, Err(e) => format!("load err {e}") };
                    println!("  variant {i}: tags {:?} model {model} — {plg}", v.tags);
                }
            }
            other => println!("{gp}: entity model is {:?}", other.map(|n| std::mem::discriminant(n))),
        }
    }
}
