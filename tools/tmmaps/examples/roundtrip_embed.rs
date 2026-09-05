//! Embedding control: the original map with only two raw splices — password
//! removed and the embedded-objects chunk rebuilt by our writer around the
//! map's own ZIP and manifest. Every other byte of the body is untouched.
use std::{env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let m0 = MapFile::load(Path::new(&a[1]));
    let body = &m0.gbx.body;
    let (_, _, off, len) = tmmaps::map::skip_chunks(body).into_iter().find(|c| c.0 == 0x03043054).unwrap();
    let d = &body[off..off + len];
    let p = d.windows(4).position(|w| w == b"PK\x03\x04").unwrap();
    let zip = d[p..len - 4].to_vec();
    let mut names: Vec<(String, String)> = m0.items.iter().filter(|it| it.author.as_deref() != Some("Nadeo")).map(|it| (it.model.clone(), it.author.clone().unwrap())).collect();
    names.sort(); names.dedup();
    let refs: Vec<(&str, &str)> = names.iter().map(|(n, a)| (n.as_str(), a.as_str())).collect();
    let mut m = MapFile::load(Path::new(&a[1]));
    m.remove_password();
    m.replace_embedded_objects(&refs, &zip);
    m.write_to(Path::new(&a[2])).unwrap();
    let c = MapFile::load(Path::new(&a[2]));
    println!("items {} manifest {}", c.items.len(), refs.len());
}
