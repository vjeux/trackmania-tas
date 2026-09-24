//! Dump all crystal layers with kinds. Usage: layerdump FILE
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    for (i, l) in c.layers.iter().enumerate() {
        let kind = match &l.kind {
            mapgeom::crystal_model::LayerKind::Geometry { crystal, is_visible, collidable, .. } => {
                format!("Geometry vis={is_visible} col={collidable} nfaces={}", crystal.faces.len())
            }
            k => format!("{k:?}"),
        };
        println!("layer{i} enabled={} id={:?} name={:?} kind={kind}", l.base.is_enabled, l.base.layer_id, l.base.layer_name);
    }
    if let Some((_, cr)) = &c.single_layer {
        println!("single_layer nfaces={}", cr.faces.len());
    }
}
