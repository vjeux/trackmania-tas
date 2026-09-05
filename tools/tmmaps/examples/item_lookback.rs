use std::{collections::BTreeMap, env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let m = MapFile::load(Path::new(&a[1]));
    // model name -> (first item index, first is_def, raw of first, defs, refs, distinct slots)
    let mut by: BTreeMap<String, (usize, bool, u32, usize, usize, Vec<Option<usize>>)> = BTreeMap::new();
    for it in &m.items {
        let f = &m.item_ids[it.model_field];
        let e = by.entry(it.model.clone()).or_insert((it.index, f.is_def, f.raw, 0, 0, Vec::new()));
        if f.is_def { e.3 += 1 } else { e.4 += 1 }
        if !e.5.contains(&f.slot) { e.5.push(f.slot); }
    }
    for (k, (first, def, raw, defs, refs, slots)) in by {
        println!("{k:22} first=#{first:<5} first_is_def={def:<5} raw={raw:08X} defs={defs} refs={refs} slots={slots:?}");
    }
    println!("item lookback fields: {}", m.item_ids.len());
}
