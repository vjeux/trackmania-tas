//! `re7_decls_pak --pak F:K PREFAB` — the vertex declarations of a pack prefab's static-object visuals (RE 7).
use mapgeom::static_item::Node;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let pak = f("--pak").unwrap(); let (pp, key) = pak.rsplit_once(':').unwrap();
    let mut store = mapgeom::store::DataStore::empty(); store.add_pak(pp, key).unwrap();
    let path = a.iter().skip(1).find(|x| !x.starts_with("--") && !x.contains(':')).unwrap();
    let pm = store.load_model(path).unwrap();
    let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm).unwrap();
    for (ei, e) in pf.ents.iter().enumerate() {
        let Some(Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
        let Some(s2) = so.solid2() else { continue };
        for sg in &s2.shaded_geoms {
            let Some(Node::Visual(v)) = s2.visuals.get(sg.visual_index as usize).and_then(|r| r.inline.as_deref()) else { continue };
            let Some(st) = v.stream() else { continue };
            println!("ent {ei} visual {} lod {}: {} idx, decls {:?}", sg.visual_index, sg.lod_mask, v.index_buffer.as_ref().map(|b| b.indices.len()).unwrap_or(0), st.decls.iter().map(|d| format!("name {} ty {} space {} off {} stride_w {}", d.name(), d.ty(), d.space(), d.offset(), (d.flags1 >> 20) & 0xff)).collect::<Vec<_>>());
        }
    }
}
