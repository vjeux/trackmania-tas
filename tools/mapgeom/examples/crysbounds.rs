fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let g = tmmaps::gbx::Gbx::parse(&data);
        let loc = match mapgeom::crystal_model::locate(&g.body) {
            Ok(l) => l,
            Err(e) => { println!("{}: {e}", path.rsplit('/').next().unwrap()); continue; }
        };
        let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
        let layers = mapgeom::static_item::bake::geometry_layers(&c);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for (cr, _, _) in &layers {
            for p in &cr.positions {
                for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); }
            }
        }
        println!("{} xyz=[{:.1},{:.1},{:.1}]-[{:.1},{:.1},{:.1}]", path.rsplit('/').next().unwrap(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
    }
}
