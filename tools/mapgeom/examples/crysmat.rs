fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let g = tmmaps::gbx::Gbx::parse(&data);
        let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
        let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
        println!("== {}", path.rsplit('/').next().unwrap());
        for (i, m) in c.materials.iter().enumerate() {
            let link = m.inst().map(|x| x.link().unwrap_or("?").to_string()).unwrap_or_else(|| m.name.clone());
            println!("  mat{i}: {link}");
        }
    }
}
