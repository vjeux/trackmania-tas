//! An item's waypoint trigger: type (chunk 2E00201F) and the trigger shape
//! (entity model's trigger_shape surface: kind, bounds, tri count).
//! Usage: trigshape ITEM...
fn main() {
    for p in std::env::args().skip(1) {
        let data = std::fs::read(&p).unwrap();
        let f = match mapgeom::static_item::file::parse_file(&data) { Ok(f) => f, Err(e) => { println!("{p}: {e}"); continue; } };
        let wp = f.item.chunks.iter().find_map(|c| match c { mapgeom::static_item::item::ItemChunk::Waypoint { waypoint_type, .. } => Some(*waypoint_type), _ => None });
        let ent = f.item.model().and_then(|mc| mc.entity_model());
        let name = p.rsplit('/').next().unwrap_or(&p);
        match ent {
            None => println!("{name}: waypoint {wp:?}, no entity model"),
            Some(e) => match e.trigger_shape.inline.as_deref() {
                Some(mapgeom::static_item::Node::Surface(s)) => {
                    match &s.surf {
                        mapgeom::static_item::surface::Surf::Mesh { vertices, triangles, .. } => {
                            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                            for v in vertices { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
                            println!("{name}: waypoint {wp:?}, trigger MESH {} verts {} tris bounds x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}; iso {:?}", vertices.len(), triangles.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], &e.iso[9..12]);
                        }
                        other => println!("{name}: waypoint {wp:?}, trigger {:?}", std::mem::discriminant(other)),
                    }
                }
                Some(_) => println!("{name}: waypoint {wp:?}, trigger_shape is not a surface"),
                None => println!("{name}: waypoint {wp:?}, trigger_shape index {} (no inline); iso {:?} iso2 {:?}", e.trigger_shape.index, e.iso, e.iso2),
            },
        }
    }
}
