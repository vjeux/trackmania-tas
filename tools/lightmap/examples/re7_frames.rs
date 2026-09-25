//! `re7_frames MAP…` — the three SFrame records of a saved map's lightmap cache mapping head, decoded (RE 7).
fn main() {
    for p in std::env::args().skip(1) {
        let m = match lightmap::mapio::load(&p) { Ok(m) => m, Err(e) => { println!("{p}: {e}"); continue } };
        let Some(d) = m.chunk.data.as_ref() else { println!("{p}: no lightmap data"); continue };
        let mp = match d.cache.mapping() { Some(mp) => mp, None => { println!("{p}: no mapping"); continue } };
        let h = &mp.head;
        println!("== {p}: head {} B; frames {} images each {:?} B", h.len(), d.frames.len(), d.frames.iter().map(|f| f.images.iter().map(|i| i.len()).collect::<Vec<_>>()).collect::<Vec<_>>());
        let u32at = |o: usize| u32::from_le_bytes(h[o..o + 4].try_into().unwrap());
        let f32at = |o: usize| f32::from_le_bytes(h[o..o + 4].try_into().unwrap());
        let f16at = |o: usize| { let b = u16::from_le_bytes([h[o], h[o + 1]]); half_to_f32(b) };
        println!("  head words: {:?}", (0..15).map(|i| u32at(i * 4)).collect::<Vec<_>>());
        if let Some(out) = std::env::var_os("RE7_FRAMES_OUT") {
            for (fi, f) in d.frames.iter().enumerate() {
                for (ii, im) in f.images.iter().enumerate() {
                    if im.is_empty() { continue; }
                    let path = format!("{}/frame{fi}-image{ii}.webp", out.to_string_lossy());
                    std::fs::write(&path, im).unwrap();
                    // decode stats: per RIFF part the mean and max of the RGB bytes
                    let mut off = 0usize;
                    let mut k = 0;
                    while off + 12 <= im.len() && &im[off..off + 4] == b"RIFF" {
                        let sz = u32::from_le_bytes([im[off + 4], im[off + 5], im[off + 6], im[off + 7]]) as usize + 8;
                        let part = &im[off..(off + sz).min(im.len())];
                        match lightmap::img::decode_webp(part) {
                            Ok(d) => { let n = d.px.len().max(1); let mean = d.px.iter().map(|&v| v as u64).sum::<u64>() as f64 / n as f64; let mx = d.px.iter().copied().max().unwrap_or(0); let nz = d.px.iter().filter(|&&v| v != 0).count(); println!("    frame {fi} image {ii} part {k}: {}×{} mean {mean:.2} max {mx} nonzero {nz}/{n}", d.w, d.h); }
                            Err(e) => println!("    frame {fi} image {ii} part {k}: decode error {e}"),
                        }
                        off += sz;
                        k += 1;
                    }
                }
            }
        }
        for k in 0..3 {
            let o = 60 + 66 * k;
            if o + 66 > h.len() { break; }
            println!("  frame {k}: bump {} z {} daytime {:#x} replay {} MaxHDR_Mood {} MaxHDR {} bounce {} sky {} clouds {} hbasis234 ({}, {}, {}) storeLAmbient {} LocalLight_Storage {} LocalLight_Switch {} LAmbient ({}, {}, {})",
                u32at(o), u32at(o + 4), u32at(o + 8), f32at(o + 12), f32at(o + 16), f32at(o + 20), f32at(o + 24), f32at(o + 28), u32at(o + 32), f16at(o + 36), f16at(o + 38), f16at(o + 40), u32at(o + 42), u32at(o + 46), u32at(o + 50), f32at(o + 54), f32at(o + 58), f32at(o + 62));
        }
    }
}
fn half_to_f32(h: u16) -> f32 {
    let s = ((h >> 15) & 1) as u32; let e = ((h >> 10) & 0x1f) as i32; let f = (h & 0x3ff) as u32;
    let v = if e == 0 { (f as f32) * 2f32.powi(-24) } else if e == 31 { f32::INFINITY } else { (1.0 + f as f32 / 1024.0) * 2f32.powi(e - 15) };
    if s == 1 { -v } else { v }
}
