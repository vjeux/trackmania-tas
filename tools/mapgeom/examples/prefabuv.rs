//! uv0/uv1/position ranges of every visual of a pack prefab's static objects,
//! straight from the pack (no bake). Usage: prefabuv --pak F:KEY [...] PREFAB
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut path = String::new();
    let mut i = 0;
    while i < a.len() {
        if a[i] == "--pak" { let (p, k) = a[i + 1].rsplit_once(':').unwrap(); store.add_pak(p, k).unwrap(); i += 2; } else { path = a[i].clone(); i += 1; }
    }
    let model = store.load_model(&path).unwrap();
    let prefab = mapgeom::static_item::prefab::CPlugPrefab::from_model(&model).unwrap();
    for (ei, e) in prefab.ents.iter().enumerate() {
        let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
        let Some(s2) = so.solid2() else { println!("entity {ei}: static object without inline solid2"); continue };
        println!("entity {ei} pos {:?}: {} geoms", e.pos, s2.shaded_geoms.len());
        for g in &s2.shaded_geoms {
            let vi = g.visual_index.max(0) as usize;
            let mi = g.material_index as usize;
            let mat = s2.materials.get(mi).and_then(|r| model.externals.iter().find(|(k, _)| *k as i32 == r.index).map(|(_, p)| p.rsplit('\\').next().unwrap_or(p).to_string())).unwrap_or_else(|| format!("mat{mi}"));
            let Some(vref) = s2.visuals.get(vi) else { continue };
            let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() else { continue };
            let Some(st) = vis.stream() else { continue };
            println!("  visual {vi} {mat}");
            for (d, el) in st.decls.iter().zip(st.elems.iter()) {
                match el {
                    Elem::Float3(pos) => { let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]); for v in pos { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } } println!("    decl {:<3} x {:8.3}..{:8.3}  y {:8.3}..{:8.3}  z {:8.3}..{:8.3}", d.name(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]); }
                    Elem::Float2(uv) => { let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]); for v in uv { for k in 0..2 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } } println!("    decl {:<3} u {:8.4}..{:8.4}  v {:8.4}..{:8.4}  ({} verts)", d.name(), lo[0], hi[0], lo[1], hi[1], uv.len()); }
                    _ => {}
                }
            }
        }
    }
}
