use std::{collections::BTreeMap, env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let m = MapFile::load(Path::new(&a[1]));
    let mut by: BTreeMap<String, (usize, Vec<Option<String>>, Vec<u32>, Vec<Option<String>>)> = BTreeMap::new();
    for it in &m.items {
        let e = by.entry(it.model.clone()).or_default();
        e.0 += 1;
        if !e.1.contains(&it.author) { e.1.push(it.author.clone()); }
        if !e.2.contains(&it.collection_raw) { e.2.push(it.collection_raw); }
        if !e.3.contains(&it.waypoint_tag) { e.3.push(it.waypoint_tag.clone()); }
    }
    for (k, (n, au, col, wp)) in by {
        println!("{k:24} n={n:5} authors={au:?} collection={col:08X?} wp={wp:?}");
    }
}
