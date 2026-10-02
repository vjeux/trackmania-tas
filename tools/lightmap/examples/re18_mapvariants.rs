//! `re18_mapvariants MAP.Gbx [MODEL_SUBSTR]` — the placements' variant byte (flags >> 8) per item model (RE 18, 2026-10-01: the
//! kind-0 vegetation records' grouping — does the map carry a variant pick per bush?).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
    let want = a.get(2).cloned().unwrap_or_default();
    let mut h: std::collections::BTreeMap<(String, u8), usize> = Default::default();
    for it in &mf.items { if !want.is_empty() && !it.model.contains(&want) { continue; } *h.entry((it.model.clone(), it.variant())).or_default() += 1; }
    for ((m, v), n) in &h { println!("{m}\tvariant {v}\t{n}"); }
    if !want.is_empty() { for it in mf.items.iter().filter(|i| i.model.contains(&want)).take(40) { println!("  {} v{} flags {:#x} pos {:?} yaw {:.3} pitch {:.3} roll {:.3} pivot {:?}", it.model, it.variant(), it.flags, it.pos, it.yaw, it.pitch, it.roll, it.pivot); } }
}
