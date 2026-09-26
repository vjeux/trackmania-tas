//! `re7_decls ITEM.Item.Gbx` — every visual's vertex stream declarations (RE 7).
use mapgeom::static_item::Node;
fn main() {
    let b = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    let f = mapgeom::static_item::file::parse_file(&b).unwrap();
    let s2 = f.item.static_object().unwrap().solid2().unwrap();
    for sg in &s2.shaded_geoms {
        let Some(Node::Visual(v)) = s2.visuals.get(sg.visual_index as usize).and_then(|r| r.inline.as_deref()) else { continue };
        let Some(st) = v.stream() else { continue };
        println!("visual {} material {} lod {}: {} verts, decls {:?}; elems {:?}; tex_coord_sets {}", sg.visual_index, sg.material_index, sg.lod_mask, st.decls.len(), st.decls.iter().map(|d| format!("name {} ty {} space {} off {} stride_w {}", d.name(), d.ty(), d.space(), d.offset(), (d.flags1 >> 20) & 0xff)).collect::<Vec<_>>(), st.elems.iter().map(|e| match e { mapgeom::static_item::vstream::Elem::Float3(v) => format!("f3×{}", v.len()), mapgeom::static_item::vstream::Elem::Float2(v) => format!("f2×{}", v.len()), mapgeom::static_item::vstream::Elem::Word(v) => format!("w×{}", v.len()), _ => "other".to_string() }).collect::<Vec<_>>(), v.main.as_ref().map(|m| m.tex_coord_sets.len()).unwrap_or(0));
    }
}
