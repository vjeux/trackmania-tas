//! `f_zonemesh --pak F:K COLLECTION ZONE` — every mobil / prefab / entity of a zone block's ground variant with its solid's
//! geoms (vertices, triangles, lightmap uv) and the LM mesh the builder makes of it (engineer F: which mesh is the zone
//! tile's LM record mesh — the stpad capture draws the Grass tiles with 9 vertices / 24 indices).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let pak = f("--pak").expect("--pak");
    let (pp, key) = pak.rsplit_once(':').unwrap();
    let mut store = mapgeom::store::DataStore::empty();
    store.add_pak(pp, key).expect("pak");
    let rest: Vec<&String> = a.iter().skip(1).filter(|x| !x.starts_with("--") && !x.contains(':')).collect();
    let (coll, zone) = (rest[0].as_str(), rest[1].as_str());
    for (fam, ext) in [("GameCtnBlockInfoFlat", "EDFlat"), ("GameCtnBlockInfoFrontier", "EDFrontier"), ("GameCtnBlockInfoTransition", "EDTransition"), ("GameCtnBlockInfoClassic", "EDClassic")] {
        let path = format!("{coll}\\GameCtnBlockInfo\\{fam}\\{zone}.{ext}.Gbx");
        let Ok(bi) = mapgeom::blockinfo::load(&mut store, &path) else { continue };
        println!("{path}: ground variant {}", bi.variant_base_ground.is_some());
        let Some(v) = bi.variant_base_ground.as_ref() else { continue };
        for (li, layer) in v.mobils.iter().enumerate() {
            for (mi, m) in layer.iter().enumerate() {
                println!("  mobil layer {li} #{mi}: prefab {:?}", m.prefab);
                let Some(pp) = m.prefab.clone() else { continue };
                let pm = match store.load_model(&pp) { Ok(p) => p, Err(e) => { println!("    load: {e}"); continue } };
                let pf = match mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm) { Ok(p) => p, Err(e) => { println!("    prefab: {e}"); continue } };
                for (ei, e) in pf.ents.iter().enumerate() {
                    match e.model.inline.as_deref() {
                        Some(mapgeom::static_item::Node::StaticObject(so)) => {
                            match so.solid2() {
                                Some(s2) => {
                                    let mesh = lightmap::lmmesh::lm_mesh_of_solid(s2);
                                    println!("    ent {ei}: static object, {} shaded geoms; LM mesh {}", s2.shaded_geoms.len(), mesh.as_ref().map(|m| format!("{} verts / {} indices", m.verts.len(), m.indices.len())).unwrap_or("none".into()));
                                    println!("      material ids {:?}; materials {}; material_insts {}; custom {}; pre_light_gen {:?}", s2.material_ids, s2.materials.len(), s2.material_insts.len(), s2.custom_materials.len(), s2.pre_light_gen.as_ref().map(|p| format!("{p:?}")).unwrap_or_default());
                                    for (mi, mr) in s2.materials.iter().enumerate() { println!("      material {mi}: index {} inline {:?}", mr.index, mr.inline.as_deref().map(|n| format!("{n:?}").chars().take(300).collect::<String>())); }
                                    for (mi, mr) in s2.material_insts.iter().enumerate() { println!("      material_inst {mi}: index {} inline {:?}", mr.index, mr.inline.as_deref().map(|n| format!("{n:?}").chars().take(300).collect::<String>())); }
                                    for (gi, sg) in s2.shaded_geoms.iter().enumerate() {
                                        let mut uvr = String::new();
                                        if let Some(mapgeom::static_item::Node::Visual(vis)) = s2.visuals.get(sg.visual_index as usize).and_then(|r| r.inline.as_deref()) {
                                            if let Some(st) = vis.stream() {
                                                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                                                    if let mapgeom::static_item::vstream::Elem::Float2(u) = e { let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]); for p in u { for k in 0..2 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } } uvr.push_str(&format!(" decl {} (type {}) uv range {lo:?}..{hi:?};", d.name(), d.ty())); } else { uvr.push_str(&format!(" decl {} (type {});", d.name(), d.ty())); }
                                                }
                                            }
                                            uvr.push_str(&format!(" main flags {:?}", vis.main.as_ref().map(|m| m.flags())));
                                        }
                                        println!("      shaded geom {gi}: {sg:?}{uvr}");
                                    }
                                    for (gi, sg) in s2.shaded_geoms.iter().enumerate() {
                                        println!("      geom {gi}: {}", lightmap::lmmesh::geom_summary(s2).get(gi).cloned().unwrap_or_default());
                                        let _ = sg;
                                    }
                                }
                                None => println!("    ent {ei}: static object without solid2"),
                            }
                        }
                        Some(other) => println!("    ent {ei}: {:?}", std::mem::discriminant(other)),
                        None => println!("    ent {ei}: external model index {}", e.model.index),
                    }
                }
            }
        }
    }
}
