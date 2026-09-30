//! `e7_palette MAP.Map.Gbx …` — each map's item-colour palette byte (chunk 0x0304306C) and name, plus the MapElemColor byte census of
//! its items (E7, 2026-09-30: the per-map attribution of the palette rule).
fn main() {
    for p in std::env::args().skip(1) {
        let mf = tmmaps::map::MapFile::load(std::path::Path::new(&p));
        let byte = mf.color_palette();
        let name = byte.and_then(tmmaps::map::MapFile::color_palette_name).unwrap_or("?");
        let mut census: std::collections::BTreeMap<u8, usize> = Default::default();
        if let Some(c) = mf.colors() { for i in 0..mf.items.len() { *census.entry(c.item(i)).or_default() += 1; } }
        println!("{}\tpalette {}\t{}\titems {} MapElemColor {:?}", p.rsplit('/').next().unwrap_or(&p), byte.map(|b| b.to_string()).unwrap_or("-".into()), name, mf.items.len(), census);
    }
}
