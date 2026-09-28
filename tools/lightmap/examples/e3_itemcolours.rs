//! `cargo run --release -p lightmap --example e3_itemcolours -- MAP.Map.Gbx` — the placement COLOUR byte (chunk 0x03043062)
//! histogram per item model: which items a HueMask / PyPxz_Hue recolour touches on a map (E3, 2026-09-28; g23's AI items).
fn main() {
    let path = std::env::args().nth(1).expect("MAP.Map.Gbx");
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(&path));
    let colours = mf.colors();
    let mut hist: std::collections::BTreeMap<(String, u8), usize> = Default::default();
    for (i, it) in mf.items.iter().enumerate() {
        let c = colours.as_ref().map(|c| c.item(i)).unwrap_or(0);
        *hist.entry((it.model.clone(), c)).or_default() += 1;
    }
    println!("{} items, colour chunk {}", mf.items.len(), if colours.is_some() { "present" } else { "absent" });
    for ((m, c), n) in &hist {
        println!("{m}\tcolour {c}\t{n}");
    }
}
