//! Per-material layer/position map. Usage: orderdeep SRC.Item.Gbx
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let mats: Vec<String> = c.materials.iter().map(|m| m.inst().map(|x| x.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or_else(|| m.name.clone())).collect();
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    // global face sequence with layer info
    let mut info: std::collections::BTreeMap<usize, Vec<(usize, usize, bool, bool)>> = Default::default();
    for (li, (cr, vis, col)) in layers.iter().enumerate() {
        for (fi, f) in cr.faces.iter().enumerate() {
            info.entry(f.material.max(0) as usize).or_default().push((li, fi, *vis, *col));
        }
    }
    for (mi, v) in &info {
        let first = v.first().unwrap();
        let last = v.last().unwrap();
        println!("mat{} {}: nfaces={} first=(L{},F{},v{},c{}) last=(L{},F{},v{},c{}) nlayers={}", mi, mats.get(*mi).cloned().unwrap_or("?".into()), v.len(), first.0, first.1, first.2, first.3, last.0, last.1, last.2, last.3, v.iter().map(|x| x.0).collect::<Vec<_>>().into_iter().collect::<std::collections::BTreeSet<_>>().len());
    }
}
