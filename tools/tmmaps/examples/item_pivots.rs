use std::{collections::BTreeMap, env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let m = MapFile::load(Path::new(&a[1]));
    let mut by: BTreeMap<String, (usize, Vec<[f32; 3]>, Vec<f32>, Vec<(f32, f32)>)> = BTreeMap::new();
    for it in &m.items {
        let e = by.entry(it.model.clone()).or_default();
        e.0 += 1;
        if !e.1.contains(&it.pivot) { e.1.push(it.pivot); }
        if !e.2.contains(&it.scale) { e.2.push(it.scale); }
        if !e.3.contains(&(it.pitch, it.roll)) && e.3.len() < 3 { e.3.push((it.pitch, it.roll)); }
    }
    for (k, (n, piv, sc, pr)) in by {
        println!("{k:24} n={n:5} pivots={piv:?} scales={sc:?} pitch/roll={pr:?}");
    }
}
