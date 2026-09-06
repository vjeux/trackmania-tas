//! Crystal material names. Usage: cmatnames SRCFILE
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    for (i, m) in c.materials.iter().enumerate() {
        println!("{i}: name={} inst={}", m.name, m.inst().map(|x| x.link().unwrap_or("?").to_string()).unwrap_or("?".into()));
    }
}
