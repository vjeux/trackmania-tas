fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let g = tmmaps::gbx::Gbx::parse(&data);
        let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
        let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
        let mats: Vec<String> = c.materials.iter().map(|m| m.inst().map(|x| x.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or_else(|| m.name.clone())).collect();
        // geometry layers
        let mut order: Vec<usize> = Vec::new();
        let mut counts = std::collections::BTreeMap::new();
        // use bake's geometry_layers? replicate: find layers with faces
        for layer in &c.layers {
            // Layer enum? try debug
            let _ = layer;
        }
        println!("== {} mats={:?}", path.rsplit('/').next().unwrap(), mats);
        let _ = (order, counts);
    }
}
