use std::{env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    for f in env::args().skip(1) {
        let m = MapFile::load(Path::new(&f));
        let (s, _) = m.body_regions[0];
        let b = &m.gbx.body;
        println!("{f}: block region @{s}: {:02x?}", &b[s..s + 16]);
        // scan body before the block region for 0x0304300D chunk id and lookback version word
        let mut hits = Vec::new();
        for i in 0..s.saturating_sub(4) {
            let w = u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
            if w == 0x0304300D || w == 0x03043011 || w == 0x0301B000 { hits.push((i, format!("{w:08X}"))); }
        }
        println!("  early chunk ids: {:?}", hits);
        if let Some((i, _)) = hits.first() { println!("  bytes after first: {:02x?} {}", &b[*i..*i + 24], String::from_utf8_lossy(&b[*i + 16..*i + 44]).replace(|c: char| !c.is_ascii_graphic(), ".")); }
    }
}
