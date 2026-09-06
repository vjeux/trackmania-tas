//! Trigger + waypoint dump. Usage: trigdump FILE...
fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let m = f.item.model().unwrap();
        let em = m.entity_model().unwrap();
        let has_trig = em.trigger_shape.inline.is_some();
        let mut wp = "?".to_string();
        for c in &f.item.chunks {
            if let mapgeom::static_item::item::ItemChunk::Waypoint { version, waypoint_type, .. } = c {
                wp = format!("v{version} type={waypoint_type}");
            }
        }
        let mut tb = String::new();
        if let Some(mapgeom::static_item::Node::Surface(s)) = em.trigger_shape.inline.as_deref() {
            match &s.surf {
                mapgeom::static_item::surface::Surf::Mesh { vertices, triangles, .. } => {
                    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for v in vertices {
                        for k in 0..3 {
                            lo[k] = lo[k].min(v[k]);
                            hi[k] = hi[k].max(v[k]);
                        }
                    }
                    tb = format!(" meshbox=[{:.1},{:.1},{:.1}]-[{:.1},{:.1},{:.1}] ntri={}", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], triangles.len());
                }
                other => tb = format!(" {other:?}").chars().take(120).collect(),
            }
        }
        println!("{} trigger={has_trig} waypoint={wp}{tb}", path.rsplit('/').next().unwrap());
    }
}
