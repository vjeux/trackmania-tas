//! Embedding control 3: the original map, with two of its item files passed
//! through our GBX writer — Sheep parse+re-write only, HayRollSmall through
//! rewrite_ident with its own name (a no-op rename).
use mapgeom::{embedded, tiny_assets};
use std::{collections::BTreeMap, env, path::Path};
use tmmaps::{gbx::Gbx, map::MapFile};
fn main() {
    let a: Vec<String> = env::args().collect();
    let m0 = MapFile::load(Path::new(&a[1]));
    let body = &m0.gbx.body;
    let (_, _, off, len) = tmmaps::map::skip_chunks(body).into_iter().find(|c| c.0 == 0x03043054).unwrap();
    let d = &body[off..off + len];
    let p = d.windows(4).position(|w| w == b"PK\x03\x04").unwrap();
    let mut files: BTreeMap<String, Vec<u8>> = embedded::unzip(&d[p..len - 4]).unwrap().into_iter().collect();
    let sheep = files.get_mut("Items/Catan/Sheep.Item.Gbx").unwrap();
    let g = Gbx::parse(sheep);
    println!("sheep: version {} body {} bytes, header {} bytes, ref table {} bytes", g.version, g.body.len(), g.user_data.len(), g.ref_table.len());
    let b = g.body.clone();
    *sheep = g.write_body_recompressed(&b);
    let hay = files.get_mut("Items/Catan/HayRollSmall.Item.Gbx").unwrap();
    *hay = tiny_assets::rewrite_ident(hay, "HayRollSmall", "HayRollSmall", "vq_Y1MZ0RDKJJSro2EEtLQ");
    let zip = tiny_assets::zip(&files);
    let mut names: Vec<(String, String)> = m0.items.iter().filter(|it| it.author.as_deref() != Some("Nadeo")).map(|it| (it.model.clone(), it.author.clone().unwrap())).collect();
    names.sort(); names.dedup();
    let refs: Vec<(&str, &str)> = names.iter().map(|(n, a)| (n.as_str(), a.as_str())).collect();
    let mut m = MapFile::load(Path::new(&a[1]));
    m.remove_password();
    m.replace_embedded_objects(&refs, &zip);
    m.write_to(Path::new(&a[2])).unwrap();
    println!("wrote");
}
