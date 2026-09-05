//! Three-way control for embedded copies of a Nadeo item, beside the stock
//! item: T1 = the item's own file with only its Ident renamed (ref table
//! untouched, still names the pack prefab); T2 = Ident renamed AND the ref
//! table rebuilt from Items/ but still naming the pack prefab; T3 = as T2 but
//! naming a half-scaled copy embedded in the ZIP. Which of the three the game
//! keeps says where the rejection is.
use mapgeom::{container, names, rescale::{self, Rescale}, store::DataStore, tiny_assets};
use std::{collections::BTreeMap, env, path::Path};
use tmmaps::{gbx::Gbx, map::MapFile};
fn ident(bytes: &[u8], stem: &str, alias: &str) -> Gbx {
    let mut g = Gbx::parse(bytes);
    g.user_data = tiny_assets::replace_lp(g.user_data, stem, alias);
    g.body = tiny_assets::replace_lp(g.body, stem, alias);
    g.user_data = tiny_assets::replace_lp(g.user_data, "Nadeo", tiny_assets::AUTHOR);
    g.body = tiny_assets::replace_lp(g.body, "Nadeo", tiny_assets::AUTHOR);
    g
}
fn main() {
    let a: Vec<String> = env::args().collect();
    let (src, out, stadium) = (&a[1], &a[2], &a[3]);
    let mut store = DataStore::empty();
    store.add_pak(stadium, tiny_assets::STADIUM_KEY).unwrap();
    let item = "Stadium\\Items\\TunnelSupportArch16m.Item.Gbx";
    let stem = "TunnelSupportArch16m";
    let orig = store.read(item).unwrap();
    let mut files = BTreeMap::new();
    // T1
    let g = ident(&orig, stem, "AC00000220.Item.Gbx");
    let b = g.body.clone();
    files.insert("C:/Users/vjeux/Documents/Trackmania/Items/AC00000220.Item.Gbx".to_string(), g.write_body_recompressed(&b));
    // T2: rebuilt table, pack paths
    let cg = container::Gbx::parse(&orig).unwrap();
    let folder = "Stadium\\Items";
    let entries: Vec<(u32, String, bool)> = cg.refs.iter().map(|e| (e.node_index, names::join(folder, &cg.ref_path(e)), e.use_file)).collect();
    let mut g = ident(&orig, stem, "AC00000221.Item.Gbx");
    g.ref_table = tiny_assets::ref_table(&entries);
    let b = g.body.clone();
    files.insert("C:/Users/vjeux/Documents/Trackmania/Items/AC00000221.Item.Gbx".to_string(), g.write_body_recompressed(&b));
    // T3: rebuilt table, half copies in the ZIP
    let mut rs = Rescale::new(0.5, "_half");
    let entries3: Vec<(u32, String, bool)> = entries.iter().map(|(n, p, u)| {
        let p2 = if rescale::is_geometry(p) { rs.file(&mut store, p).unwrap() } else { p.clone() };
        (*n, p2, *u)
    }).collect();
    let mut g = ident(&orig, stem, "AC00000222.Item.Gbx");
    g.ref_table = tiny_assets::ref_table(&entries3);
    let b = g.body.clone();
    files.insert("C:/Users/vjeux/Documents/Trackmania/Items/AC00000222.Item.Gbx".to_string(), g.write_body_recompressed(&b));
    for (name, bytes) in &rs.files {
        files.insert(format!("C:/Users/vjeux/Documents/Trackmania/{}", name.replace('\\', "/")), bytes.clone());
    }
    let zip = tiny_assets::zip(&files);

    let mut m = MapFile::load(Path::new(src));
    m.set_map_uid("ScaleEmbed20260904ABCDEFGHK");
    let arch: Vec<usize> = m.items.iter().filter(|it| it.model == stem).map(|it| it.index).collect();
    assert!(arch.len() >= 4);
    for i in 0..m.items.len() { m.move_item_pos(i, [16.0, -1000.0, 16.0]); }
    for (k, x) in [64.0f32, 128.0, 192.0, 256.0].iter().enumerate() {
        m.move_item(arch[k], [*x, 16.0, 128.0], 0.0, ((x / 32.0) as i32, 9, 4));
        m.set_item_scale(arch[k], 1.0);
    }
    for (k, al) in ["AC00000220.Item.Gbx", "AC00000221.Item.Gbx", "AC00000222.Item.Gbx"].iter().enumerate() {
        m.set_item_model(arch[k + 1], al);
        m.set_item_author(arch[k + 1], tiny_assets::AUTHOR);
    }
    let stage = Path::new(out).with_extension("stage.Map.Gbx");
    m.write_to(&stage).unwrap();
    let mut m2 = MapFile::load(&stage);
    m2.remove_password();
    m2.replace_embedded_objects(&[("AC00000220.Item.Gbx", tiny_assets::AUTHOR), ("AC00000221.Item.Gbx", tiny_assets::AUTHOR), ("AC00000222.Item.Gbx", tiny_assets::AUTHOR)], &zip);
    m2.write_to(Path::new(out)).unwrap();
    let _ = std::fs::remove_file(stage);
    let c = MapFile::load(Path::new(out));
    for i in &arch[..4] { println!("item {} model {} author {:?} pos {:?}", i, c.items[*i].model, c.items[*i].author, c.items[*i].pos); }
    println!("zip {} bytes, {} files", zip.len(), files.len());
}
