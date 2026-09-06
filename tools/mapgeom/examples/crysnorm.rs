//! Crystal stored normals census. Usage: crysnorm FILE...
fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let g = tmmaps::gbx::Gbx::parse(&data);
        let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
        let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
        let layers = mapgeom::static_item::bake::geometry_layers(&c);
        let (mut faces, mut with_n, mut with_uv) = (0, 0, 0);
        for (cr, _, _) in &layers {
            for f in &cr.faces {
                faces += 1;
                if f.u01.is_some() { with_n += 1; }
                if !f.uv_index.is_empty() || !f.uvs.is_empty() { with_uv += 1; }
            }
        }
        println!("{} faces={faces} with_u01={with_n} with_uv={with_uv} version_note=", path.rsplit('/').next().unwrap());
    }
}
