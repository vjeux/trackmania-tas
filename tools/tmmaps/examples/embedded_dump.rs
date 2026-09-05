use std::{env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let m = MapFile::load(Path::new(&a[1]));
    let body = &m.gbx.body;
    for (cid, _hdr, off, len) in tmmaps::map::skip_chunks(body) {
        if cid == 0x03043054 {
            let d = &body[off..off + len];
            println!("chunk 0x03043054 payload at {off} len {len}");
            let n = d.len().min(176);
            for row in (0..n).step_by(16) {
                let end = (row + 16).min(n);
                let hex: Vec<String> = d[row..end].iter().map(|b| format!("{b:02x}")).collect();
                let asc: String = d[row..end].iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
                println!("  {row:5}: {:<48} {asc}", hex.join(" "));
            }
            // find PK header
            if let Some(p) = d.windows(4).position(|w| w == b"PK\x03\x04") {
                println!("  zip starts at payload+{p}; 8 bytes before: {:02x?}", &d[p.saturating_sub(8)..p]);
                let zip = &d[p..];
                let mut i = 0usize;
                while i + 30 <= zip.len() && &zip[i..i + 4] == b"PK\x03\x04" {
                    let comp = u32::from_le_bytes(zip[i + 18..i + 22].try_into().unwrap()) as usize;
                    let nlen = u16::from_le_bytes(zip[i + 26..i + 28].try_into().unwrap()) as usize;
                    let elen = u16::from_le_bytes(zip[i + 28..i + 30].try_into().unwrap()) as usize;
                    let name = String::from_utf8_lossy(&zip[i + 30..i + 30 + nlen]).to_string();
                    let method = u16::from_le_bytes(zip[i + 8..i + 10].try_into().unwrap());
                    let flags = u16::from_le_bytes(zip[i + 6..i + 8].try_into().unwrap());
                    let ver = u16::from_le_bytes(zip[i + 4..i + 6].try_into().unwrap());
                    println!("    {name}  (ver {ver} method {method} flags {flags:#x} {comp} bytes, extra {elen})");
                    i += 30 + nlen + elen + comp;
                }
                println!("  tail after entries: {:02x?}", &zip[i..(i + 24).min(zip.len())]);
            }
        }
    }
}
