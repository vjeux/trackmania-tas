//! Compare the crystal byte spans of two items (first differing offset).
use mapgeom::crystal::ItemCrystal;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let x = ItemCrystal::open(&std::fs::read(&a[0]).unwrap()).unwrap();
    let y = ItemCrystal::open(&std::fs::read(&a[1]).unwrap()).unwrap();
    let sx = &x.body[x.loc.at..x.end];
    let sy = &y.body[y.loc.at..y.end];
    let first = sx.iter().zip(sy.iter()).position(|(p, q)| p != q);
    println!("spans {} and {} bytes; first diff {:?}; nodes {} vs {}; suffix equal: {}", sx.len(), sy.len(), first, x.gbx.num_nodes, y.gbx.num_nodes, x.body[x.end..] == y.body[y.end..]);
}
