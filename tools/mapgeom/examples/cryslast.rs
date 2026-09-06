fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let g = tmmaps::gbx::Gbx::parse(&data);
        let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
        let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
        let mats: Vec<String> = c.materials.iter().map(|m| m.inst().map(|x| x.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or_else(|| m.name.clone())).collect();
        let layers = mapgeom::static_item::bake::geometry_layers(&c);
        let mut last: std::collections::BTreeMap<usize, usize> = Default::default();
        let mut seq = 0usize;
        for (cr, visible, _) in &layers {
            if !visible { continue; }
            for f in &cr.faces {
                let mi = f.material.max(0) as usize;
                last.insert(mi, seq);
                seq += 1;
            }
        }
        let mut v: Vec<(usize, usize)> = last.into_iter().collect();
        v.sort_by_key(|(_, s)| *s);
        println!("== {}", path.rsplit('/').next().unwrap());
        println!("  last-appearance: {:?}", v.iter().map(|(i, _)| format!("{i}:{}", mats.get(*i).cloned().unwrap_or("?".into()))).collect::<Vec<_>>());
    }
}
