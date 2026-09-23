//! A test map for LOOSE items (files under Documents/Trackmania/Items, not
//! embedded): every item record of the host map parked far away, then one
//! record per placement spec re-pointed at an item of the set — the way a
//! mapper's saved map references a set item, and the in-game check that a
//! `TinyBlocks\…\X.Item.Gbx` file resolves by its path, draws where its
//! pivot says, and tiles against its neighbours.
//!
//! usage: item_set_map HOST.Map.Gbx OUT.Map.Gbx AUTHOR SPEC...
//!   SPEC = IDENT@x,y,z[/yaw_deg][/px,py,pz]   (pivot default 8,0,8: a 1x1 tiny block's centre)
//!   e.g. 'TinyBlocks\Roads\RoadTech\Main\Main\RoadTechStraight.Item.Gbx@408,8,408'
//! The host must have at least as many item records as specs.
use std::{env, path::Path};
use tmmaps::map::MapFile;

fn main() {
    let a: Vec<String> = env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: item_set_map HOST OUT AUTHOR SPEC...");
        std::process::exit(2);
    }
    let (host, out, author) = (&a[1], &a[2], &a[3]);
    let specs = &a[4..];
    let mut m = MapFile::load(Path::new(host));
    if m.items.len() < specs.len() {
        eprintln!("host has {} item records, {} specs", m.items.len(), specs.len());
        std::process::exit(1);
    }
    m.set_map_uid(&format!("Set{:024}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() % 10u128.pow(24)));
    let cell = |p: [f32; 3]| ((p[0] / 32.0) as i32, ((p[1] + 64.0) / 8.0) as i32, (p[2] / 32.0) as i32);
    for i in 0..m.items.len() {
        m.move_item_pos(i, [16.0, -1000.0, 16.0]);
    }
    for (i, spec) in specs.iter().enumerate() {
        let (ident, rest) = spec.split_once('@').unwrap_or_else(|| {
            eprintln!("spec {spec}: no '@'");
            std::process::exit(2);
        });
        let mut parts = rest.split('/');
        let pos: Vec<f32> = parts.next().unwrap_or("").split(',').filter_map(|x| x.parse().ok()).collect();
        if pos.len() != 3 {
            eprintln!("spec {spec}: position needs x,y,z");
            std::process::exit(2);
        }
        let yaw_deg: f32 = parts.next().and_then(|y| y.parse().ok()).unwrap_or(0.0);
        let pivot: Vec<f32> = parts.next().map(|p| p.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_else(|| vec![8.0, 0.0, 8.0]);
        let pos = [pos[0], pos[1], pos[2]];
        m.move_item(i, pos, yaw_deg.to_radians(), cell(pos));
        m.set_item_frame(i, [yaw_deg.to_radians(), 0.0, 0.0], [pivot[0], pivot[1], pivot[2]]);
        m.set_item_scale(i, 1.0);
        m.set_item_model(i, ident);
        m.set_item_author(i, author);
        println!("item {i}: {ident} at {pos:?} yaw {yaw_deg} pivot {pivot:?}");
    }
    let stage = Path::new(out).with_extension("stage.Map.Gbx");
    m.write_to(&stage).unwrap();
    let mut m2 = MapFile::load(&stage);
    m2.remove_password();
    m2.write_to(Path::new(out)).unwrap();
    let _ = std::fs::remove_file(stage);
    println!("test map: {out} with {} loose items (author {author})", specs.len());
}
