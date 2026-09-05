use std::{env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let m = MapFile::load(Path::new(&a[1]));
    let body = &m.gbx.body;
    let (_, _, off, len) = tmmaps::map::skip_chunks(body).into_iter().find(|c| c.0 == 0x03043054).unwrap();
    let d = &body[off..off + len];
    let p = d.windows(4).position(|w| w == b"PK\x03\x04").unwrap();
    let zip = &d[p..];
    std::fs::write("/tmp/game-embedded.zip", zip).unwrap();
    // print first dir entry local header + first file local header fully
    let mut i = 0; let mut shown = 0;
    while shown < 8 {
        let nlen = u16::from_le_bytes(zip[i + 26..i + 28].try_into().unwrap()) as usize;
        let elen = u16::from_le_bytes(zip[i + 28..i + 30].try_into().unwrap()) as usize;
        let comp = u32::from_le_bytes(zip[i + 18..i + 22].try_into().unwrap()) as usize;
        println!("local hdr @{i}: {:02x?}", &zip[i..i + 30]);
        println!("  name {:?}", String::from_utf8_lossy(&zip[i + 30..i + 30 + nlen]));
        println!("  extra {:02x?}", &zip[i + 30 + nlen..i + 30 + nlen + elen]);
        i += 30 + nlen + elen + comp; shown += 1;
    }
    // central directory: find PK\x01\x02
    let c = zip.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    println!("central @{c}: {:02x?}", &zip[c..c + 46]);
    let nlen = u16::from_le_bytes(zip[c + 28..c + 30].try_into().unwrap()) as usize;
    let elen = u16::from_le_bytes(zip[c + 30..c + 32].try_into().unwrap()) as usize;
    println!("  name {:?} extra {:02x?}", String::from_utf8_lossy(&zip[c + 46..c + 46 + nlen]), &zip[c + 46 + nlen..c + 46 + nlen + elen]);
    let e = zip.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
    println!("eocd @{e}: {:02x?}", &zip[e..]);
}
