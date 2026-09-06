fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let g = tmmaps::gbx::Gbx::parse(&data);
        let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
        let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
        println!("== {} smooth_ver={} ngroups={} nfaceints={}", path.rsplit('/').next().unwrap(), c.smoothing_version, c.smoothing_groups.len(), c.per_face_ints.len());
        println!("  groups={:?}", &c.smoothing_groups[..c.smoothing_groups.len().min(8)]);
        let mut hist = std::collections::BTreeMap::new();
        for v in &c.per_face_ints { *hist.entry(*v).or_insert(0) += 1; }
        println!("  face_ints hist (val:count) {:?}", hist.into_iter().take(12).collect::<Vec<_>>());
    }
}
