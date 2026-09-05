//! Bisect the map side: take Granady's map, rewrite ONLY its embedded-objects
//! chunk through our writer (same items, same bytes), and see if it still loads.
//!   swap_item MAP.Map.Gbx OUT.Map.Gbx [--zip-paths items|orig]
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut m = MapFile::load(std::path::Path::new(&a[1]));
    let body = m.gbx.body.clone();
    let (off, names) = tmmaps::header::embedded_zip(&body).expect("embedded zip");
    let (_, _, payload, size) = *tmmaps::gbx::all_skip_chunks(&body).iter().find(|(c, ..)| *c == 0x03043054).unwrap();
    // the zip bytes: from `off` (relative to chunk payload end? embedded_zip returns len-z) -- recompute: find PK
    let zip_start = (payload..payload + size).find(|&i| &body[i..i + 4] == b"PK\x03\x04").unwrap();
    let zip_len = u32::from_le_bytes(body[zip_start - 4..zip_start].try_into().unwrap()) as usize;
    let zip = body[zip_start..zip_start + zip_len].to_vec();
    let _ = off;
    // authors: read from the item headers inside the zip
    let files = zip_files(&zip);
    let mut manifest: Vec<(String, String)> = Vec::new();
    for (path, bytes) in &files {
        if let Some((ident, author)) = tmmaps::header::item_ident_author(bytes) {
            manifest.push((ident, author));
        }
    }
    let mode = a.get(3).map(|s| s.as_str()).unwrap_or("orig");
    let newzip = if mode == "deflorig" {
        let mut map = std::collections::BTreeMap::new();
        for (path, bytes) in &files { map.insert(path.clone(), bytes.clone()); }
        tmmaps::header::deflated_zip(&map)
    } else if mode == "deflitems" {
        let mut map = std::collections::BTreeMap::new();
        for (path, bytes) in &files { let short = path.rsplit('/').next().unwrap(); map.insert(format!("Items/{short}"), bytes.clone()); }
        tmmaps::header::deflated_zip(&map)
    } else if mode == "storedorig" {
        // stored zip, his original long paths
        let mut map = std::collections::BTreeMap::new();
        for (path, bytes) in &files { map.insert(path.clone(), bytes.clone()); }
        tmmaps::header::stored_zip(&map)
    } else if mode == "items" {
        let mut map = std::collections::BTreeMap::new();
        for (path, bytes) in &files { let short = path.rsplit('/').next().unwrap(); map.insert(format!("Items/{short}"), bytes.clone()); }
        tmmaps::header::stored_zip(&map)
    } else { zip.clone() };
    let mref: Vec<(&str, &str)> = manifest.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    println!("{} declared, {} files, mode {mode}", names.len(), files.len());
    m.replace_embedded_objects(&mref, &newzip);
    m.write_to(std::path::Path::new(&a[2])).unwrap();
}
fn zip_files(zip: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 30 <= zip.len() && &zip[i..i + 4] == b"PK\x03\x04" {
        let method = u16::from_le_bytes(zip[i + 8..i + 10].try_into().unwrap());
        let csize = u32::from_le_bytes(zip[i + 18..i + 22].try_into().unwrap()) as usize;
        let nlen = u16::from_le_bytes(zip[i + 26..i + 28].try_into().unwrap()) as usize;
        let xlen = u16::from_le_bytes(zip[i + 28..i + 30].try_into().unwrap()) as usize;
        let name = String::from_utf8_lossy(&zip[i + 30..i + 30 + nlen]).to_string();
        let start = i + 30 + nlen + xlen;
        let data = zip[start..start + csize].to_vec();
        let data = match method {
            0 => data,
            8 => miniz_oxide::inflate::decompress_to_vec(&data).expect("inflate"),
            _ => panic!("zip method {method}"),
        };
        out.push((name, data));
        i = start + csize;
    }
    out
}
