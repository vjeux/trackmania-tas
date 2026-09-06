//! his-coll ∩ my-coll bit-exact. Usage: collinter HIS.ITEM MINE.ITEM
use std::collections::BTreeSet;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut sets = Vec::new();
    for path in [&a[1], &a[2]] {
        let data = std::fs::read(path).unwrap();
        let f = mapgeom::static_item::file::parse_file(&data).unwrap();
        let so = f.item.static_object().unwrap();
        let mut s = BTreeSet::new();
        if let Some(surf) = so.surface() {
            if let mapgeom::static_item::surface::Surf::Mesh { vertices, triangles: _, version: _ } = &surf.surf {
                for v in vertices {
                    s.insert([v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]);
                }
            }
        }
        sets.push(s);
    }
    let inter = sets[0].intersection(&sets[1]).count();
    println!("his={} mine={} intersect={} his_only={} mine_only={}",
        sets[0].len(), sets[1].len(), inter, sets[0].len()-inter, sets[1].len()-inter);
}
