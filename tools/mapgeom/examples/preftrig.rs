//! Waypoint triggers inside a prefab: every entity whose model is
//! NPlugTrigger_SWaypoint (0x09178000): its raw body bytes, the referenced
//! trigger shape (inline surface bounds or external path), entity position.
//! Usage: preftrig --pak F:KEY [--pak ...] PREFAB...
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut paths = Vec::new();
    let mut i = 0;
    while i < a.len() {
        if a[i] == "--pak" { let (p, k) = a[i + 1].rsplit_once(':').unwrap(); store.add_pak(p, k).unwrap(); i += 2; } else { paths.push(a[i].clone()); i += 1; }
    }
    for p in paths {
        let model = match store.load_model(&p) { Ok(m) => m, Err(e) => { println!("{p}: {e}"); continue; } };
        let prefab = match mapgeom::static_item::prefab::CPlugPrefab::from_model(&model) { Ok(x) => x, Err(e) => { println!("{p}: {e}"); continue; } };
        println!("== {} ({} entities)", p.rsplit('\\').next().unwrap(), prefab.ents.len());
        for (ei, e) in prefab.ents.iter().enumerate() {
            let Some(mapgeom::static_item::Node::Opaque(o)) = e.model.inline.as_deref() else { continue };
            if o.class_id != 0x09178000 && o.class_id != 0x0917A000 && o.class_id != 0x0917B000 { continue; }
            let hex: Vec<String> = o.raw.iter().take(48).map(|b| format!("{b:02x}")).collect();
            println!("  ent {ei}: class 0x{:08X} pos {:?} rot {:?} raw[{}] {}", o.class_id, e.pos, e.rot, o.raw.len(), hex.join(""));
            if o.class_id == 0x09178000 {
                // version u32, then a node ref: index i32 (+ inline body if new)
                let idx = i32::from_le_bytes(o.raw[4..8].try_into().unwrap());
                let ext = model.externals.iter().find(|(k, _)| *k as i32 == idx).map(|(_, p)| p.clone());
                println!("     trigger shape node {idx} -> {}", ext.unwrap_or_else(|| "inline/defined earlier".into()));
            }
        }
        // surfaces named in externals
        for (k, x) in &model.externals { if x.to_ascii_lowercase().ends_with(".shape.gbx") { println!("  external shape node {k}: {x}"); } }
    }
}
