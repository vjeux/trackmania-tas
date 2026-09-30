//! `re17_chunks MAP.Gbx [MAP2.Gbx…]` — the body chunk ids of a map (tmmaps::gbx::all_skip_chunks) with payload sizes and, for
//! payloads ≤ 32 bytes, the bytes — to locate the CGameCtnChallenge ColorPalette word (RE 17 2026-09-30 17:40Z).
fn main() {
    for p in std::env::args().skip(1) {
        let m = tmmaps::map::MapFile::load(std::path::Path::new(&p));
        let chunks = tmmaps::gbx::all_skip_chunks(&m.gbx.body);
        println!("== {p}: {} skippable chunks", chunks.len());
        for (id, off, payload, size) in &chunks {
            if (*id & 0xFFFFF000) == 0x03043000 || (*id & 0xFFFFF000) == 0x0304B000 {
                let small = if *size <= 32 { m.gbx.body[*payload..*payload + *size].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ") } else { String::new() };
                println!("  chunk {id:08x} at {off} payload {size} B {small}");
            }
        }
    }
}
