fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let g = tmmaps::gbx::Gbx::parse(&data);
        let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
        let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
        let mats: Vec<String> = c.materials.iter().map(|m| m.inst().map(|x| x.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or_else(|| m.name.clone())).collect();
        let layers = mapgeom::static_item::bake::geometry_layers(&c);
        let mut first: Vec<usize> = Vec::new();
        let mut counts: std::collections::BTreeMap<usize, usize> = Default::default();
        for (cr, visible, _) in &layers {
            if !visible { continue; }
            for f in &cr.faces {
                let mi = f.material.max(0) as usize;
                if !first.contains(&mi) { first.push(mi); }
                *counts.entry(mi).or_default() += 1;
            }
        }
        println!("== {}", path.rsplit('/').next().unwrap());
        println!("  first-appearance: {:?}", first.iter().map(|i| format!("{i}:{}", mats.get(*i).cloned().unwrap_or("?".into()))).collect::<Vec<_>>());
        let mut bycount: Vec<(usize, usize)> = counts.into_iter().collect();
        bycount.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        println!("  by-face-count: {:?}", bycount.iter().map(|(i, n)| format!("{i}:{}x{n}", mats.get(*i).cloned().unwrap_or("?".into()))).collect::<Vec<_>>());
    }
}
