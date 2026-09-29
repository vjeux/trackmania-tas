//! Check one flipkey. Usage: onecheck HIS.ITEM MINE.ITEM
use std::collections::BTreeMap;
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[2]]
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let fk: Vec<&str> = std::fs::read_to_string("/tmp/flip17b.txt").unwrap().trim().split('|').collect();
    println!("nkeys={}", fk.len());
    // parse first key
    let v: Vec<&str> = fk[0].split(';').collect();
    let p = |i: usize| {
        let c: Vec<&str> = v[i].split(',').collect();
        (c[0].parse().unwrap(), c[1].parse().unwrap(), c[2].parse().unwrap())
    };
    let mut k = [p(0), p(1), p(2)];
    k.sort();
    println!("first key={k:?}");
    let _ = (BTreeMap::<u8, u8>::new(), mk, sub, cross, Elem::Float3(vec![[0.0; 3]]), a);
}
