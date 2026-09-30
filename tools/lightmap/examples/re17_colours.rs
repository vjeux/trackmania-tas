//! `re17_colours MAP.Gbx` — the colour-byte census of chunk 0x03043062 (blocks / baked / items) with the model names of the coloured
//! items; the 2026 palettes carry more than the 0–5 enum (RE 17 2026-09-30 17:35Z, g23's RgbBaseColorTarget #00327c = Default.ColorTable "Blue"[0]).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
    let Some(c) = m.colors() else { println!("no colour chunk"); return };
    println!("colour bytes: {} = {} blocks + {} baked + {} items (chunk holds {})", c.bytes.len(), c.n_blocks, c.n_baked, m.items.len(), c.bytes.len());
    let mut hb = std::collections::BTreeMap::new(); let mut hi = std::collections::BTreeMap::new();
    for i in 0..c.n_blocks { *hb.entry(c.block(i)).or_insert(0usize) += 1; }
    for i in 0..m.items.len() { *hi.entry(c.item(i)).or_insert(0usize) += 1; }
    println!("block colour histogram: {hb:?}");
    println!("item colour histogram: {hi:?}");
    let mut per_model: std::collections::BTreeMap<(u8, String), usize> = std::collections::BTreeMap::new();
    for (i, it) in m.items.iter().enumerate() { let col = c.item(i); if col != 0 { *per_model.entry((col, it.model.clone())).or_insert(0) += 1; } }
    for ((col, model), n) in per_model.iter().take(40) { println!("  colour {col:3}  {n:4} × {model}"); }
    let mut per_block: std::collections::BTreeMap<(u8, String), usize> = std::collections::BTreeMap::new();
    for (i, b) in m.blocks.iter().enumerate() { let col = c.block(i); if col != 0 { *per_block.entry((col, b.name.clone())).or_insert(0) += 1; } }
    for ((col, name), n) in per_block.iter().take(30) { println!("  block colour {col:3}  {n:4} × {name}"); }
}
