//! Full material instance dump. Usage: matdump FILE [SUBSTR]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    for (i, m) in s2.custom_materials.iter().enumerate() {
        if let Some(inst) = m.inst() {
            let link = inst.link().unwrap_or("?").to_string();
            if let Some(f) = a.get(2) {
                if !link.contains(f.as_str()) {
                    continue;
                }
            }
            println!("mat{i} {link} phys={} inst={inst:?}", inst.physics());
        }
    }
}
