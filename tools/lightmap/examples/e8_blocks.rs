//! `e8_blocks MAP.Map.Gbx` — a map's block list census: count, per (name, flags & ground/ghost bits) histogram with the cell rows,
//! the item count and the item rows (E8, 2026-10-01: what the x2 unlit file carries that the game turned into 270 records).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
    println!("{}: size {:?}, {} blocks, {} baked, {} items", a[1], mf.size, mf.blocks.len(), mf.baked.len(), mf.items.len());
    let mut h: std::collections::BTreeMap<(String, bool, bool, u32), (usize, std::collections::BTreeMap<i32, usize>)> = Default::default();
    for b in &mf.blocks {
        let (_, y, _) = b.coords();
        let e = h.entry((b.name.clone(), b.flags & 0x1000 != 0, b.flags & 0x1000_0000 != 0, b.flags & 0xFFF)).or_default();
        e.0 += 1; *e.1.entry(y).or_default() += 1;
    }
    for ((name, ground, ghost, lo), (n, rows)) in &h { println!("  block {name}: {n} (ground {ground}, ghost {ghost}, flags&0xfff {lo:#x}); rows {rows:?}"); }
    let mut rows: std::collections::BTreeMap<i32, usize> = Default::default();
    for it in &mf.items { *rows.entry(it.file_cell[1] as i32).or_default() += 1; }
    println!("  item file-cell rows: {rows:?}");
    let mut names: std::collections::BTreeMap<String, usize> = Default::default();
    for it in &mf.items { *names.entry(it.model.clone()).or_default() += 1; }
    let mut v: Vec<_> = names.into_iter().collect(); v.sort_by(|x, y| y.1.cmp(&x.1));
    println!("  top item models: {:?}", v.iter().take(8).collect::<Vec<_>>());
    if let Some(b) = mf.blocks.first() { println!("  first block: {} flags {:#x} coords {:?} dir {}", b.name, b.flags, b.coords(), b.dir); }
    // --row R: the items of file-cell row R — model histogram, the position lattice (multiples of 64 / 32 m), the first few
    if let Some(i) = a.iter().position(|x| x == "--row") {
        let r: i32 = a[i + 1].parse().unwrap();
        let its: Vec<_> = mf.items.iter().enumerate().filter(|(_, it)| it.file_cell[1] as i32 == r).collect();
        let mut names: std::collections::BTreeMap<String, usize> = Default::default();
        let (mut m64, mut m32) = (0usize, 0usize);
        for (_, it) in &its { *names.entry(it.model.clone()).or_default() += 1; if it.pos[0] % 64.0 == 0.0 && it.pos[2] % 64.0 == 0.0 { m64 += 1; } else if it.pos[0] % 32.0 == 0.0 && it.pos[2] % 32.0 == 0.0 { m32 += 1; } }
        println!("  row {r}: {} items; on the 64-m lattice {m64}, on the 32-m lattice {m32}; models {:?}", its.len(), names);
        for (ii, it) in its.iter().take(8) { println!("    item {ii} {} pos ({:.1}, {:.1}, {:.1}) yaw {:.3} variant {} flags {:#x}", it.model, it.pos[0], it.pos[1], it.pos[2], it.yaw, it.variant(), it.flags); }
    }
}
