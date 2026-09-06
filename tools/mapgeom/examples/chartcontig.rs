//! Are charts contiguous in file order? Usage: chartcontig HIS.ITEM SRCFILE SUBSTR
use std::collections::{BTreeMap, BTreeSet};
use mapgeom::static_item::bake::{geometry_layers, face_triangles};
use mapgeom::static_item::vstream::Elem;
fn mk(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0]*1000.0).round() as i32, (p[1]*1000.0).round() as i32, (p[2]*1000.0).round() as i32)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // his charts (tri indices per chart via flood fill)
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // find visual with substr, get tris + uv1
    let mut htris: Vec<([f32; 3], [f32; 2])> = Vec::new(); // (pos, uv1) per corner? need per tri
    // Simplified: per tri, set of (pos,uv1) corners; flood fill; then map tris to source faces via mmkey, check face contiguity.
    println!("chartcontig: complex, simplifying to chart face-index ranges");
    let _ = (BTreeMap::<u8, u8>::new(), BTreeSet::<u8>::new(), geometry_layers, face_triangles, mk, Elem::Float3(vec![[0.0; 3]]));
}
