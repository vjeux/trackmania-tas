//! Hex of one item record of a map (its body region), for diffing two
//! placements: `item_record_dump MAP INDEX`.
use std::{env, path::Path};
use tmmaps::map::MapFile;

fn main() {
    let a: Vec<String> = env::args().collect();
    let m = MapFile::load(Path::new(&a[1]));
    let i: usize = a[2].parse().unwrap();
    let it = &m.items[i];
    let (s, e) = it.record_region;
    let b = &m.gbx.body[s..e];
    println!("item {i}: record {s}..{e} ({} bytes) waypoint_region {:?} flags_off {}", e - s, it.waypoint_region, it.waypoint_region.1);
    for (k, chunk) in b.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|x| format!("{x:02x}")).collect();
        let asc: String = chunk.iter().map(|x| if x.is_ascii_graphic() { *x as char } else { '.' }).collect();
        println!("{:04x}: {:<48} {asc}", k * 16, hex.join(" "));
    }
}
