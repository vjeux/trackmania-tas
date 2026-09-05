use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = MapFile::load(std::path::Path::new(&a[1]));
    let b = &m.gbx.body;
    let want = &a[2];
    for it in &m.items {
        if !it.model.contains(want.as_str()) { continue; }
        // record spans from the model id word to the end of the v8 tail: dump yaw..end
        let start = it.yaw_off - 4 * 0; // yaw is the first fixed field after ids
        let end = it.waypoint_region.1 + 2 + 12 + 4 + 24;
        let end = end.min(b.len());
        println!("{} @{} author={:?} coll={:#x} pos={:?} yaw={:.4} pivot={:?} scale={} wp={:?}", it.model, it.index, it.author, it.collection_raw, it.pos, it.yaw, it.pivot, it.scale, it.waypoint_tag);
        println!("  fixed fields from yaw: {}", b[start..end].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "));
        break;
    }
}
