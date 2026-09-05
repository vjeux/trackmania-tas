use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = MapFile::load(std::path::Path::new(&a[1]));
    let b = &m.gbx.body;
    let mut counts: std::collections::BTreeMap<(String, u16), usize> = Default::default();
    for it in &m.items {
        let f = u16::from_le_bytes(b[it.waypoint_region.1..it.waypoint_region.1 + 2].try_into().unwrap());
        *counts.entry((it.model.clone(), f)).or_default() += 1;
    }
    for ((model, f), n) in counts {
        println!("{n:5} {model:<28} flags {f:#06x}");
    }
}
