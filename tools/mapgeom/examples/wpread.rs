//! Print an item's waypoint chunk (type) without needing its static model. Usage: wpread FILE...
fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).unwrap();
        match mapgeom::static_item::file::parse_file(&data) {
            Ok(f) => {
                let wp = f.item.chunks.iter().find_map(|c| match c {
                    mapgeom::static_item::item::ItemChunk::Waypoint { version, waypoint_type, .. } => Some(format!("v{version} type={waypoint_type}")),
                    _ => None,
                });
                println!("{} waypoint={}", path, wp.unwrap_or_else(|| "none".into()));
            }
            Err(e) => println!("{} PARSE ERROR {e}", path),
        }
    }
}
