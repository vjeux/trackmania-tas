//! A test map for LOOSE items (files under Documents/Trackmania/Items, not
//! embedded): every item record of the host map parked far away, then one
//! record per placement spec re-pointed at an item of the set — the way a
//! mapper's saved map references a set item, and the in-game check that a
//! `TinyBlocks\…\X.Item.Gbx` file resolves by its path, draws where its
//! pivot says, and tiles against its neighbours.
//!
//! usage: item_set_map HOST.Map.Gbx OUT.Map.Gbx AUTHOR SPEC...
//!   SPEC = IDENT@x,y,z[/yaw_deg[,pitch_deg[,roll_deg]]][/px,py,pz][#Spawn|Checkpoint|Goal]   (pivot default 8,0,8: a 1x1 tiny block's centre;
//!          the #tag is the placement's waypoint property — a start/CP/finish item needs one)
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
    let mut tags: Vec<(usize, String)> = Vec::new();
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
        let (spec, tag) = match spec.split_once('#') {
            Some((s, t)) => (s, Some(t)),
            None => (spec.as_str(), None),
        };
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
        // yaw[,pitch[,roll]] in degrees
        let angles: Vec<f32> = parts.next().map(|y| y.split(',').filter_map(|v| v.parse().ok()).collect()).unwrap_or_default();
        let yaw_deg: f32 = angles.first().copied().unwrap_or(0.0);
        let pitch_deg: f32 = angles.get(1).copied().unwrap_or(0.0);
        let roll_deg: f32 = angles.get(2).copied().unwrap_or(0.0);
        let pivot: Vec<f32> = parts.next().map(|p| p.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_else(|| vec![8.0, 0.0, 8.0]);
        let pos = [pos[0], pos[1], pos[2]];
        m.move_item(i, pos, yaw_deg.to_radians(), cell(pos));
        m.set_item_frame(i, [yaw_deg.to_radians(), pitch_deg.to_radians(), roll_deg.to_radians()], [pivot[0], pivot[1], pivot[2]]);
        m.set_item_scale(i, 1.0);
        m.set_item_model(i, ident);
        m.set_item_author(i, author);
        if let Some(t) = tag {
            tags.push((i, t.to_string()));
        }
        println!("item {i}: {ident} at {pos:?} yaw {yaw_deg} pivot {pivot:?}{}", tag.map(|t| format!(" waypoint {t}")).unwrap_or_default());
    }
    let stage = Path::new(out).with_extension("stage.Map.Gbx");
    m.write_to(&stage).unwrap();
    // the waypoint properties are variable-length splices: a second pass over
    // the written file, after the model renames
    let mut m2 = MapFile::load(&stage);
    for (i, t) in &tags {
        m2.set_item_waypoint(*i, Some(t), 0);
    }
    m2.remove_password();
    m2.write_to(Path::new(out)).unwrap();
    let _ = std::fs::remove_file(stage);
    println!("test map: {out} with {} loose items (author {author})", specs.len());
}
