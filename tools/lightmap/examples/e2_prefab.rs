// E2 scratch: a prefab item's entities (rot, pos, params id, model kind) and each static object's light sockets (the 12 floats), leniently
fn main() {
    std::env::set_var("MAPGEOM_PREFAB_LENIENT", "1");
    std::env::set_var("MAPGEOM_PREFAB_TRACE", "1");
    for p in std::env::args().skip(1) {
        let bytes = std::fs::read(&p).expect("read");
        let f = match mapgeom::static_item::file::parse_file(&bytes) { Ok(f) => f, Err(e) => { println!("{p}: parse error: {e}"); continue } };
        let name = p.rsplit('/').next().unwrap();
        match f.item.prefab() {
            None => { println!("{name}: not a prefab item (static object: {})", f.item.static_object().is_some()); }
            Some(pf) => {
                println!("{name}: prefab v{} url {:?} {} entities (truncated {})", pf.version, pf.url, pf.ents.len(), pf.truncated);
                for (i, e) in pf.ents.iter().enumerate() {
                    let kind = match e.model.inline.as_deref() { Some(mapgeom::static_item::Node::StaticObject(_)) => "StaticObject".to_string(), Some(mapgeom::static_item::Node::Prefab(_)) => "Prefab".into(), Some(n) => format!("{:?}", std::mem::discriminant(n)), None => format!("ref index {}", e.model.index) };
                    println!("  entity {i}: model {kind}, rot (xyzw) {:?}, pos {:?}, params id {:#010x} ({} B), u01 {} B", e.rot, e.pos, e.params_id, e.params.len(), e.u01.len());
                    if let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() {
                        if let Some(s2) = so.solid2() {
                            println!("    solid2: {} geoms, {} lights", s2.shaded_geoms.len(), s2.lights.len());
                            for (li, l) in s2.lights.iter().enumerate() {
                                let t = &l.u05;
                                println!("    light {li}: u01 {:?} u02 {} u04 {:?} rows X ({:+.4},{:+.4},{:+.4}) Y ({:+.4},{:+.4},{:+.4}) Z ({:+.4},{:+.4},{:+.4}) T ({:+.3},{:+.3},{:+.3}) → column 2 ({:+.4},{:+.4},{:+.4})", l.u01, l.u02, l.u04, t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8], t[9], t[10], t[11], t[2], t[5], t[8]);
                            }
                        }
                    }
                }
            }
        }
    }
}
