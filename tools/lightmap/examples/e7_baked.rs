//! `e7_baked MAP.Map.Gbx …` — the map's baked-block list: entries, distinct (x, z) columns, the stacked columns (several baked blocks at one
//! (x, z)) and the grid — for the record-scene rule on stacked baked blocks (E7, 2026-09-30: does the game emit one tile record per baked
//! BLOCK or per CELL?).
fn main() {
    for p in std::env::args().skip(1) {
        let mf = tmmaps::map::MapFile::load(std::path::Path::new(&p));
        let mut by_col: std::collections::BTreeMap<(i32, i32), Vec<(i32, String)>> = Default::default();
        for b in &mf.baked { let (x, y, z) = b.coords(); by_col.entry((x, z)).or_default().push((y, b.name.clone())); }
        let stacked: Vec<_> = by_col.iter().filter(|(_, v)| v.len() > 1).collect();
        println!("{}\tsize {:?}\tbaked {}\tdistinct columns {}\tstacked columns {} (extra entries {})", p.rsplit('/').next().unwrap_or(&p), mf.size, mf.baked.len(), by_col.len(), stacked.len(), stacked.iter().map(|(_, v)| v.len() - 1).sum::<usize>());
        for (c, v) in stacked.iter().take(4) { println!("  column {:?}: {:?}", c, v); }
    }
}
