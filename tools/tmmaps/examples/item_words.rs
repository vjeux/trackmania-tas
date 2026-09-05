use std::{env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let m = MapFile::load(Path::new(&a[1]));
    let want: Vec<usize> = a[2..].iter().map(|s| s.parse().unwrap()).collect();
    for it in &m.items {
        if !want.contains(&it.index) { continue; }
        let mf = &m.item_ids[it.model_field];
        let cf = &m.item_ids[it.model_field + 1];
        let af = &m.item_ids[it.author_field];
        println!("#{:<5} model raw={:08X} def={} slot={:?} len={} | coll raw={:08X} | author raw={:08X} def={} slot={:?} {:?} | off {}",
            it.index, mf.raw, mf.is_def, mf.slot, mf.len, cf.raw, af.raw, af.is_def, af.slot, it.author, mf.off);
    }
}
