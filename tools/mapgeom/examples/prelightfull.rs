//! Full prelight dump. Usage: prelightfull FILE
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    if let Some(p) = &s2.pre_light_gen {
        println!("version={} u01={} u02={:.6} u03={} u04={:.6?} sprites={:?} boxes={} uvgroups={}",
            p.version, p.u01, p.u02, p.u03, p.u04, p.sprite_count, p.boxes.len(), p.uv_groups.len());
    }
    println!("filetime={}", m_filetime(&f));
}
fn m_filetime(f: &mapgeom::static_item::file::StaticItemFile) -> u64 {
    f.item.static_object().and_then(|so| so.solid2()).map(|s2| s2.file_write_time).unwrap_or(0)
}
