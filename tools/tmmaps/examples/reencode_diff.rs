use std::{env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let m0 = MapFile::load(Path::new(&a[1]));
    let mut m = MapFile::load(Path::new(&a[1]));
    let old = m.body_ids.first().and_then(|f| f.name.clone()).unwrap();
    let new: String = old.chars().rev().collect();
    m.set_map_uid(&new);
    m.write_to(Path::new(&a[2])).unwrap();
    let m1 = MapFile::load(Path::new(&a[2]));
    let (b0, b1) = (&m0.gbx.body, &m1.gbx.body);
    let (_, _, o0, l0) = tmmaps::map::skip_chunks(b0).into_iter().find(|c| c.0 == 0x0304301F).unwrap_or_else(|| (0, 0, m0.body_regions[0].0, m0.body_regions[0].1 - m0.body_regions[0].0));
    println!("regions: {:?} vs {:?}", &m0.body_regions[..], &m1.body_regions[..]);
    println!("blocks chunk {} @{} vs len {}", l0, o0, b1.len());
    // find first differing byte within the blocks region
    let (s0, e0) = m0.body_regions[0]; let (s1, e1) = m1.body_regions[0];
    let r0 = &b0[s0..e0]; let r1 = &b1[s1..e1];
    println!("block region len {} vs {}", r0.len(), r1.len());
    let mut i = 0; while i < r0.len().min(r1.len()) && r0[i] == r1[i] { i += 1; }
    println!("first diff at region+{i}");
    let lo = i.saturating_sub(24);
    println!(" orig: {:02x?}", &r0[lo..(i + 40).min(r0.len())]);
    println!(" new : {:02x?}", &r1[lo..(i + 40).min(r1.len())]);
    println!(" orig ascii: {}", String::from_utf8_lossy(&r0[lo..(i + 60).min(r0.len())]).replace(|c: char| !c.is_ascii_graphic() && c != ' ', "."));
    println!(" new  ascii: {}", String::from_utf8_lossy(&r1[lo..(i + 60).min(r1.len())]).replace(|c: char| !c.is_ascii_graphic() && c != ' ', "."));
    println!("blocks {} vs {}; first block {:?} vs {:?}", m0.blocks.len(), m1.blocks.len(), m0.blocks[0].name, m1.blocks[0].name);
    // block-chunk lookback stats
    let defs0 = m0.body_ids.iter().filter(|f| f.is_def).count();
    let defs1 = m1.body_ids.iter().filter(|f| f.is_def).count();
    println!("body_ids {} (defs {}) vs {} (defs {})", m0.body_ids.len(), defs0, m1.body_ids.len(), defs1);
    let mut shown = 0;
    for (k, (f0, f1)) in m0.body_ids.iter().zip(m1.body_ids.iter()).enumerate() {
        if f0.is_def != f1.is_def || f0.name != f1.name || (f0.raw & 0xC000_0000) != (f1.raw & 0xC000_0000) {
            println!("#{k}: orig raw={:08X} def={} slot={:?} name={:?} | new raw={:08X} def={} slot={:?} name={:?}", f0.raw, f0.is_def, f0.slot, f0.name, f1.raw, f1.is_def, f1.slot, f1.name);
            shown += 1;
            if shown > 12 { break; }
        }
    }
    // also print the first 12 fields of each for context
    for k in 0..12 { let f0 = &m0.body_ids[k]; let f1 = &m1.body_ids[k]; println!("  ctx #{k}: {:08X} {:?} | {:08X} {:?}", f0.raw, f0.name, f1.raw, f1.name); }
}
#[allow(dead_code)]
fn unused() {}
