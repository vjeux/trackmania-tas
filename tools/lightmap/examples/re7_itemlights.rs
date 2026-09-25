//! `re7_itemlights ITEM.Item.Gbx…` — the lights `geometry::load_model` sees in an embedded item file (RE 7).
fn main() {
    for p in std::env::args().skip(1) {
        let b = std::fs::read(&p).expect("read");
        match lightmap::geometry::load_model(&b) {
            Ok(g) => println!("{}: {} tris, {} lights: {:?}", p.rsplit('/').next().unwrap_or(&p), g.tris.len(), g.lights.len(), g.lights.iter().map(|l| (l.pos, l.color, l.intensity, l.radius, l.cone)).collect::<Vec<_>>()),
            Err(e) => println!("{p}: {e}"),
        }
        let f = mapgeom::static_item::file::parse_file(&b).expect("parse");
        if let Some(so) = f.item.static_object() { if let Some(s2) = so.solid2() { println!("  static object: {} light sockets: {:?}", s2.lights.len(), s2.lights.iter().map(|l| (l.u02, l.node.index, l.node.inline.as_deref().map(|n| format!("{:?}", std::mem::discriminant(n))))).collect::<Vec<_>>()); } }
        else if let Some(pf) = f.item.prefab() { println!("  prefab: {} ents: {:?}", pf.ents.len(), pf.ents.iter().map(|e| (e.model.index, e.model.inline.as_deref().map(|n| format!("{:?}", std::mem::discriminant(n))))).collect::<Vec<_>>()); }
        else { println!("  neither static object nor prefab"); }
    }
}
