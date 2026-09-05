//! Scale control: two copies of one stock item, at 1.0 and 0.5, side by side.
//! If both render the same size, placement scale is ignored for that item
//! class and no positional fix will ever produce a tiny map.
use std::{env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let mut m = MapFile::load(Path::new(&a[1]));
    m.set_map_uid("ScaleCtrl20260904ABCDEFGHIJ");
    m.remove_password();
    let arch: Vec<usize> = m
        .items
        .iter()
        .filter(|it| it.model == "TunnelSupportArch16m")
        .map(|it| it.index)
        .collect();
    assert!(arch.len() >= 2, "need two TunnelSupportArch16m donors");
    for i in 0..m.items.len() {
        m.move_item_pos(i, [16.0, -1000.0, 16.0]);
    }
    m.move_item(arch[0], [96.0, 16.0, 128.0], 0.0, (3, 9, 4));
    m.set_item_scale(arch[0], 1.0);
    m.move_item(arch[1], [160.0, 16.0, 128.0], 0.0, (5, 9, 4));
    m.set_item_scale(arch[1], 0.5);
    m.write_to(Path::new(&a[2])).unwrap();
    let c = MapFile::load(Path::new(&a[2]));
    for i in arch {
        println!("item {} scale {} pos {:?}", i, c.items[i].scale, c.items[i].pos);
    }
}
