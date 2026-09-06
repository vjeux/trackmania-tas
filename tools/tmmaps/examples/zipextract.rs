//! Extract the embedded-items zip from a map and verify entries against files.
//! Usage: zipextract MAP OUTDIR [NAME=PATH ...]
//! Prints per-entry sizes + md5-ish (FNV) of stored data vs expected.
use std::collections::HashMap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
    let data = m.gbx.body.clone();
    // find 0x03043054 chunk payload: search for "Items/" inside zip locals
    // simpler: find first PK\x03\x04, treat rest-of-parse from there; the zip
    // is the last blob. Walk all PK locals in the FILE.
    let mut expect: HashMap<String, Vec<u8>> = HashMap::new();
    for kv in a.iter().skip(3) {
        let (n, p) = kv.split_once('=').unwrap();
        expect.insert(n.to_string(), std::fs::read(p).unwrap());
    }
    // decode the embedded-object manifest (0x03043054 payload head)
    {
        let body = &m.gbx.body;
        if let Some((_, _, payload, _)) = tmmaps::gbx::all_skip_chunks(body).iter().find(|(c, ..)| *c == 0x03043054) {
            let b = body;
            let u32at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
            let nitems = u32at(payload + 12);
            println!("manifest items={nitems}");
            let mut o = payload + 16;
            let _ver = u32at(o); o += 4;
            let mut table: Vec<String> = Vec::new();
            let mut get = |o: &mut usize| -> String {
                let w = u32at(*o); *o += 4;
                if w == 0xFFFF_FFFF { return String::new(); }
                if (w >> 30) == 0 { return format!("#{w:x}"); }
                let idx = (w & 0x3FFF_FFFF) as usize;
                if idx == 0 {
                    let l = u32at(*o); *o += 4;
                    let s = String::from_utf8_lossy(&b[*o..*o + l]).to_string(); *o += l;
                    table.push(s.clone());
                    s
                } else if idx == 0x3FFF_FFFF { format!("#{w:x}") } else { table.get(idx - 1).cloned().unwrap_or(format!("#{w:x}")) }
            };
            for _ in 0..nitems {
                let name = get(&mut o);
                let coll = u32at(o); o += 4;
                let author = get(&mut o);
                println!("  manifest: name={name} coll={coll} author={author}");
            }
        }
    }
    let mut i = 0;
    let mut n = 0;
    std::fs::create_dir_all(&a[2]).unwrap();
    while i + 30 <= data.len() {
        if &data[i..i + 4] != b"PK\x03\x04" {
            i += 1;
            continue;
        }
        let method = u16::from_le_bytes(data[i + 8..i + 10].try_into().unwrap());
        let csize = u32::from_le_bytes(data[i + 18..i + 22].try_into().unwrap()) as usize;
        let usize_ = u32::from_le_bytes(data[i + 22..i + 26].try_into().unwrap()) as usize;
        let nlen = u16::from_le_bytes(data[i + 26..i + 28].try_into().unwrap()) as usize;
        let xlen = u16::from_le_bytes(data[i + 28..i + 30].try_into().unwrap()) as usize;
        if i + 30 + nlen + xlen + csize > data.len() {
            break;
        }
        let fname = String::from_utf8_lossy(&data[i + 30..i + 30 + nlen]).to_string();
        let start = i + 30 + nlen + xlen;
        let raw = &data[start..start + csize];
        let inflated = if method == 8 {
            miniz_oxide::inflate::decompress_to_vec(raw).unwrap_or_default()
        } else {
            raw.to_vec()
        };
        let fnv = |b: &[u8]| b.iter().fold(0xcbf29ce484222325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100000001b3));
        let note = match expect.get(&fname) {
            Some(e) => {
                if *e == inflated {
                    "MATCH".to_string()
                } else {
                    format!("MISMATCH explen={} gotlen={}", e.len(), inflated.len())
                }
            }
            None => format!("(no expect, usize={usize_})"),
        };
        println!("entry {n}: {fname} method={method} csize={csize} fnv={:x} {note}", fnv(&inflated));
        std::fs::write(format!("{}/{n}-{}.gbx", &a[2], fname.replace(['/', '\\'], "_")), &inflated).unwrap();
        n += 1;
        i = start + csize;
    }
}
