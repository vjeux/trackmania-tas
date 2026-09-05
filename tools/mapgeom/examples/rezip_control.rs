//! Embedding control 2: the original map, its embedded ZIP unpacked and
//! re-packed by our own ZIP writer. Items surviving here clears the writer.
use mapgeom::{embedded, tiny_assets};
use std::{collections::BTreeMap, env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let m0 = MapFile::load(Path::new(&a[1]));
    let body = &m0.gbx.body;
    let (_, _, off, len) = tmmaps::map::skip_chunks(body).into_iter().find(|c| c.0 == 0x03043054).unwrap();
    let d = &body[off..off + len];
    let p = d.windows(4).position(|w| w == b"PK\x03\x04").unwrap();
    let zip0 = &d[p..len - 4];
    let files0 = embedded::unzip(zip0).expect("unzip original");
    let files: BTreeMap<String, Vec<u8>> = files0.into_iter().collect();
    println!("{} files re-packed: {:?}", files.len(), files.keys().take(3).collect::<Vec<_>>());
    let zip = tiny_assets::zip(&files);
    let mut names: Vec<(String, String)> = m0.items.iter().filter(|it| it.author.as_deref() != Some("Nadeo")).map(|it| (it.model.clone(), it.author.clone().unwrap())).collect();
    names.sort(); names.dedup();
    let refs: Vec<(&str, &str)> = names.iter().map(|(n, a)| (n.as_str(), a.as_str())).collect();
    let mut m = MapFile::load(Path::new(&a[1]));
    m.remove_password();
    m.replace_embedded_objects(&refs, &zip);
    m.write_to(Path::new(&a[2])).unwrap();
    println!("zip {} -> {} bytes", zip0.len(), zip.len());
}
