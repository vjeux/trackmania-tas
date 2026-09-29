//! Rewrite an item's WAYPOINT TYPE in place. Usage: wptype IN OUT TYPE
//! (TM2020 CGameItemModel::EWaypointType: 0 Start, 1 Finish, 2 Checkpoint,
//! 3 None, 4 StartFinish). Everything else in the file is carried through
//! mapgeom's own item reader/writer; the read-back is printed so the caller can
//! see the type actually landed.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 4 {
        eprintln!("usage: wptype IN.Item.Gbx OUT.Item.Gbx TYPE");
        std::process::exit(2);
    }
    let want: i32 = a[3].parse().expect("TYPE is an integer 0..5");
    let data = std::fs::read(&a[1]).expect("read IN");
    let mut f = mapgeom::static_item::file::parse_file(&data).expect("parse item");
    let mut seen = 0usize;
    for c in f.item.chunks.iter_mut() {
        if let mapgeom::static_item::item::ItemChunk::Waypoint { waypoint_type, .. } = c {
            println!("waypoint type {} -> {}", waypoint_type, want);
            *waypoint_type = want;
            seen += 1;
        }
    }
    assert_eq!(seen, 1, "expected exactly one Waypoint chunk, found {seen}");
    let out = mapgeom::static_item::file::write_file(&f);
    std::fs::write(&a[2], &out).expect("write OUT");
    let back = mapgeom::static_item::file::parse_file(&out).expect("re-parse OUT");
    let got = back.item.chunks.iter().find_map(|c| match c {
        mapgeom::static_item::item::ItemChunk::Waypoint { waypoint_type, .. } => Some(*waypoint_type),
        _ => None,
    });
    assert_eq!(got, Some(want), "read-back type differs");
    println!("wrote {} ({} bytes, was {}), read-back type {:?}", a[2], out.len(), data.len(), got);
}
