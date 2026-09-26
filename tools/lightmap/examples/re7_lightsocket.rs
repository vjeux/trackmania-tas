//! `re7_lightsocket --pak F:K PREFAB_PATH…` — the raw light SOCKET fields of a pack prefab's static-object solids (RE 7).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let pak = f("--pak").expect("--pak");
    let (pp, key) = pak.rsplit_once(':').unwrap();
    let mut store = mapgeom::store::DataStore::empty();
    store.add_pak(pp, key).expect("pak");
    for p in a.iter().skip(1).filter(|x| !x.starts_with("--") && !x.contains(':')) {
        let pm = store.load_model(p).expect("prefab");
        let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm).expect("parse");
        for (ei, e) in pf.ents.iter().enumerate() {
            if let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() {
                if let Some(s2) = so.solid2() {
                    for (li, l) in s2.lights.iter().enumerate() {
                        println!("{p} ent {ei} light {li}: u01 {:?} u02 {} node idx {} u04 {:?} u05 {:?} ints {:?} u15 {} u16 {:?}", l.u01, l.u02, l.node.index, l.u04, l.u05, l.ints, l.u15, l.u16);
                    }
                }
            }
        }
    }
}
