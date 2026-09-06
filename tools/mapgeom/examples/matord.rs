fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let it = mapgeom::crystal::ItemCrystal::open(&data).unwrap();
    let layer = it.model.first_geometry().unwrap();
    let c = layer.kind.crystal().unwrap();
    let mut seen = Vec::new();
    for fa in &c.faces {
        if !seen.contains(&fa.material) { seen.push(fa.material); }
    }
    println!("first-appearance: {seen:?}");
    let mut counts: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
    for fa in &c.faces { *counts.entry(fa.material).or_default() += if fa.verts.len() <= 3 { 1 } else { fa.verts.len() - 2 }; }
    println!("tri counts: {counts:?}");
}
