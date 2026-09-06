//! Cross-layer face/material order analysis. Usage: layerord SRC
use std::collections::BTreeMap;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let it = mapgeom::crystal::ItemCrystal::open(&data).unwrap();
    let c = &it.model;
    if let Some((_, cr)) = &c.single_layer {
        one(cr, "single", &it);
    }
    for (i, l) in c.layers.iter().enumerate() {
        if let mapgeom::crystal_model::LayerKind::Geometry { crystal, is_visible, collidable, .. } = &l.kind {
            one(crystal, &format!("layer{i} vis={is_visible} col={collidable} enabled={}", l.base.is_enabled), &it);
        }
    }
}

fn one(cr: &mapgeom::crystal_model::Crystal, name: &str, it: &mapgeom::crystal::ItemCrystal) {
    let link = |m: i32| {
        it.model.materials.get(m.max(0) as usize).map(|x| x.inst().map(|i| i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or(x.name.clone())).unwrap_or("?".into())
    };
    let mut first = Vec::new();
    let mut counts: BTreeMap<i32, usize> = BTreeMap::new();
    for fa in &cr.faces {
        if !first.contains(&fa.material) {
            first.push(fa.material);
        }
        *counts.entry(fa.material).or_default() += if fa.verts.len() <= 3 { 1 } else { fa.verts.len() - 2 };
    }
    let fl: Vec<String> = first.iter().map(|m| format!("{m}:{}", link(*m))).collect();
    println!("{name}: nfaces={} first-appear={fl:?} tri-counts={counts:?}", cr.faces.len());
}
