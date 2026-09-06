//! Full CPlugSurface dump. Usage: surffull FILE...
fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let s = match so.shape.inline.as_deref() {
            Some(mapgeom::static_item::Node::Surface(s)) => s,
            _ => { println!("{}: no surface", path.rsplit('/').next().unwrap()); continue; }
        };
        println!("== {} version={} surfv={} dir={:?} nmats={} u05={:?} u01len={} matids={:?} skel={} u06len={}", path.rsplit('/').next().unwrap(), s.version, s.surf_version, s.gameplay_main_dir, s.materials.len(), s.u05, s.u01.len(), s.material_ids, s.skel.index, s.u06.len());
        for (i, m) in s.materials.iter().enumerate() {
            println!("  smat{i}: {m:?}");
        }
    }
}
