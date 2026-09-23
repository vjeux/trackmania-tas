//! Where the collision of a prefab-form item sits over a column: every hull
//! triangle whose vertices fall inside x0..x1, z0..z1 (item-local, after the
//! entity pose), by entity, with its y range: `item_hull_probe ITEM x0 x1 z0 z1`.
use mapgeom::static_item::{self, Node};
use std::env;

fn main() {
    let a: Vec<String> = env::args().collect();
    let bytes = std::fs::read(&a[1]).unwrap();
    let f = static_item::parse_file(&bytes).unwrap();
    let p: [f32; 4] = [a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap()];
    let Some(Node::Prefab(pf)) = f.item.model().and_then(|mc| mc.entity_model.inline.as_deref()) else {
        // the entity-model form: one static object at identity
        let Some(so) = f.item.static_object() else { eprintln!("no static object"); return };
        let Some(Node::Surface(sf)) = so.shape.inline.as_deref() else { eprintln!("no shape"); return };
        let static_item::surface::Surf::Mesh { vertices, triangles, .. } = &sf.surf else { return };
        let (mut n, mut ymin, mut ymax) = (0usize, f32::MAX, f32::MIN);
        let mut mats: std::collections::BTreeMap<u16, usize> = Default::default();
        for t in triangles {
            let vs = [vertices[t.indices[0] as usize], vertices[t.indices[1] as usize], vertices[t.indices[2] as usize]];
            if vs.iter().any(|v| v[0] >= p[0] && v[0] <= p[1] && v[2] >= p[2] && v[2] <= p[3]) {
                n += 1;
                for v in &vs { ymin = ymin.min(v[1]); ymax = ymax.max(v[1]); }
                *mats.entry((t.material_id as u16) | ((t.gameplay as u16) << 8)).or_default() += 1;
            }
        }
        println!("merged: {n} triangles in the column, y {ymin:.2}..{ymax:.2}, materials {mats:?}");
        return;
    };
    let defined: std::collections::HashMap<i32, &static_item::item::CPlugStaticObjectModel> = pf.ents.iter().filter_map(|e| match e.model.inline.as_deref() { Some(Node::StaticObject(so)) => Some((e.model.index, so)), _ => None }).collect();
    for (k, e) in pf.ents.iter().enumerate() {
        let so = match e.model.inline.as_deref() {
            Some(Node::StaticObject(so)) => so,
            None => match defined.get(&e.model.index) { Some(so) => *so, None => continue },
            _ => continue,
        };
        let at = mapgeom::geom::from_quat(e.rot, e.pos);
        let Some(Node::Surface(sf)) = so.shape.inline.as_deref() else { continue };
        let static_item::surface::Surf::Mesh { vertices, triangles, .. } = &sf.surf else { continue };
        let w: Vec<[f32; 3]> = vertices.iter().map(|v| mapgeom::geom::apply(&at, *v)).collect();
        let (mut n, mut ymin, mut ymax) = (0usize, f32::MAX, f32::MIN);
        let mut mats: std::collections::BTreeMap<u16, usize> = Default::default();
        for t in triangles {
            let vs = [w[t.indices[0] as usize], w[t.indices[1] as usize], w[t.indices[2] as usize]];
            if vs.iter().any(|v| v[0] >= p[0] && v[0] <= p[1] && v[2] >= p[2] && v[2] <= p[3]) {
                n += 1;
                for v in &vs {
                    ymin = ymin.min(v[1]);
                    ymax = ymax.max(v[1]);
                }
                *mats.entry((t.material_id as u16) | ((t.gameplay as u16) << 8)).or_default() += 1;
            }
        }
        if n > 0 {
            println!("entity {k:3} at {:?}: {n} triangles in the column, y {ymin:.2}..{ymax:.2}, materials {mats:?}", e.pos);
        }
    }
}
