fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let g = tmmaps::gbx::Gbx::parse(&data);
        let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
        let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
        let layers = mapgeom::static_item::bake::geometry_layers(&c);
        let mut hist = std::collections::BTreeMap::new();
        for (cr, visible, collidable) in &layers {
            for f in &cr.faces {
                hist.entry((f.group, f.material, *visible, *collidable)).or_insert(0);
                *hist.get_mut(&(f.group, f.material, *visible, *collidable)).unwrap() += 1;
            }
        }
        println!("== {}", path.rsplit('/').next().unwrap());
        for ((grp, mat, vis, col), n) in hist {
            println!("  group={grp} mat={mat} vis={vis} col={col} faces={n}");
        }
    }
}
