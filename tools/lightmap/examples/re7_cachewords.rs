//! `re7_cachewords MAP…` — the raw words of the CHmsLightMapCache chunks 0x06022014 (…, LDirQ, LPntQ) / 0x0602200F / 0x06022013.
fn main() {
    for p in std::env::args().skip(1) {
        let m = match lightmap::mapio::load(&p) { Ok(m) => m, Err(e) => { println!("{p}: {e}"); continue } };
        let Some(d) = m.chunk.data.as_ref() else { println!("{p}: no data"); continue };
        print!("{}:", p.rsplit('/').next().unwrap_or(&p));
        for c in &d.cache.chunks {
            if let lightmap::format::ChunkBody::Raw(b) = &c.body {
                if true {
                    let words: Vec<String> = b.chunks(4).map(|w| if w.len() == 4 { let u = u32::from_le_bytes([w[0], w[1], w[2], w[3]]); if u < 0x10000 { format!("{u}") } else { format!("{:.4}", f32::from_bits(u)) } } else { "?".into() }).collect();
                    print!("  {:#010x} [{}]", c.id, words.join(", "));
                }
            }
        }
        println!();
    }
}
