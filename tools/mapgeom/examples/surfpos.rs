//! Collision position analysis. Usage: surfpos FILE
use std::collections::BTreeSet;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // surface shape verts (CPlugSurface mesh)
    // access via solid2? surface is in static_object.shape. Use debug print of counts.
    // Instead: compare surf_vertices distinct vs stored count via surfdump (already have counts).
    // Here: check if collision positions are a subset of visual positions (welded same?).
    println!("use surfdump counts: implement subset check via visual pos set");
    let _ = (s2, BTreeSet::<u8>::new());
}
