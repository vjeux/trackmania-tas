//! Minimal deflated library zip from listed files. Usage: single_zip OUT.zip NAME=PATH ...
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut files = std::collections::BTreeMap::new();
    for kv in a.iter().skip(2) {
        let (name, path) = kv.split_once('=').unwrap();
        files.insert(name.to_string(), std::fs::read(path).unwrap());
    }
    let zip = mapgeom::tiny_assets::zip(&files);
    std::fs::write(&a[1], &zip).unwrap();
    println!("wrote {} ({} bytes, {} files)", &a[1], zip.len(), files.len());
}
