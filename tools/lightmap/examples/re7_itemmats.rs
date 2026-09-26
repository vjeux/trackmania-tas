//! `re7_itemmats ITEM.Item.Gbx…` — every material of an embedded item: name, shading model, link, textures, csts (RE 7).
fn main() {
    for p in std::env::args().skip(1) {
        let b = std::fs::read(&p).expect("read");
        let f = mapgeom::static_item::file::parse_file(&b).expect("parse");
        let Some(so) = f.item.static_object() else { println!("{p}: no static object"); continue };
        let Some(s2) = so.solid2() else { continue };
        println!("{}: {} materials, {} shaded geoms", p.rsplit('/').next().unwrap_or(&p), s2.materials.len(), s2.shaded_geoms.len());
        for (i, mr) in s2.materials.iter().enumerate() {
            match mr.inline.as_deref() {
                Some(mapgeom::static_item::Node::Material(m)) => {
                    if let Some(mm) = &m.main {
                        println!("  [{i}] name {:?} model {:?} game_material {} link {:?} base_texture {:?} physics {} gameplay {} csts {:?} color {:?} textures {:?} u01 {:?} hiding {:?}", mm.material_name, mm.model, mm.is_using_game_material, mm.link, mm.base_texture, mm.surface_physic_id, mm.surface_gameplay_id, mm.csts.iter().map(|c| format!("{:?}", c)).collect::<Vec<_>>(), mm.color, mm.user_textures.iter().map(|t| format!("{:?}", t)).collect::<Vec<_>>(), mm.u01, mm.hiding_group);
                    } else { println!("  [{i}] material without main chunk: {:?}", m.chunks); }
                }
                Some(n) => println!("  [{i}] other node {:?}", std::mem::discriminant(n)),
                None => println!("  [{i}] external index {}", mr.index),
            }
        }
        for (i, cm) in s2.custom_materials.iter().enumerate() {
            let inst = cm.inst();
            let main = inst.and_then(|m| m.main.as_ref());
            println!("  custom[{i}] {:?}: {}", cm.name, match main { Some(mm) => format!("name {:?} model {:?} game_material {} link {:?} base_texture {:?} physics {} gameplay {} csts {:?} color {:?} textures {:?} u01 {:?} hiding {:?}", mm.material_name, mm.model, mm.is_using_game_material, mm.link, mm.base_texture, mm.surface_physic_id, mm.surface_gameplay_id, mm.csts.iter().map(|c| format!("{:?}", c)).collect::<Vec<_>>(), mm.color, mm.user_textures.iter().map(|t| format!("{:?}", t)).collect::<Vec<_>>(), mm.u01, mm.hiding_group), None => "no main".into() });
        }
        for (gi, sg) in s2.shaded_geoms.iter().enumerate() {
            println!("  geom {gi}: visual {} material {} lod_mask {}", sg.visual_index, sg.material_index, sg.lod_mask);
        }
    }
}
