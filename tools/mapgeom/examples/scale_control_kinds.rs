//! Which self-contained item kinds honour the placement scale? Beside a stock
//! arch: a game-made block-item (CGameBlockItem, DecoPlatformBase) at scale 1
//! and 0.5, and an archive crystal item (RoadTechStraight) at 0.5 and 1.
use mapgeom::{embedded, tiny_assets};
use std::{collections::BTreeMap, env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let (src, out, blockitem, nadeo_zip) = (&a[1], &a[2], &a[3], &a[4]);
    let bi = std::fs::read(blockitem).unwrap();
    let legacy = embedded::unzip(&std::fs::read(nadeo_zip).unwrap()).unwrap();
    let crystal = legacy.get("RoadTech/Main/Main/RoadTechStraight.Item.Gbx").expect("archive item");
    let mut files = BTreeMap::new();
    files.insert("C:/Users/vjeux/Documents/Trackmania/Items/AC00000230.Item.Gbx".to_string(), tiny_assets::rewrite_ident(&bi, "New Item", "AC00000230.Item.Gbx", tiny_assets::AUTHOR));
    files.insert("C:/Users/vjeux/Documents/Trackmania/Items/AC00000231.Item.Gbx".to_string(), tiny_assets::rewrite_ident(crystal, "New Item", "AC00000231.Item.Gbx", "UVkE4dP3TEmNl3yiI40gVg"));
    let zip = tiny_assets::zip(&files);
    let mut m = MapFile::load(Path::new(src));
    m.set_map_uid(&format!("ScaleKinds{:017}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()));
    let arch: Vec<usize> = m.items.iter().filter(|it| it.model == "TunnelSupportArch16m").map(|it| it.index).collect();
    assert!(arch.len() >= 5);
    for i in 0..m.items.len() { m.move_item_pos(i, [16.0, -1000.0, 16.0]); }
    let plan: [(&str, f32); 5] = [("TunnelSupportArch16m", 1.0), ("AC00000230.Item.Gbx", 1.0), ("AC00000230.Item.Gbx", 0.5), ("AC00000231.Item.Gbx", 0.5), ("AC00000231.Item.Gbx", 1.0)];
    for (k, (model, sc)) in plan.iter().enumerate() {
        let x = 64.0 + 64.0 * k as f32;
        m.move_item(arch[k], [x, 16.0, 128.0], 0.0, ((x / 32.0) as i32, 9, 4));
        m.set_item_scale(arch[k], *sc);
        if k > 0 { m.set_item_model(arch[k], model); m.set_item_author(arch[k], tiny_assets::AUTHOR); }
    }
    let stage = Path::new(out).with_extension("stage.Map.Gbx");
    m.write_to(&stage).unwrap();
    let mut m2 = MapFile::load(&stage);
    m2.remove_password();
    m2.replace_embedded_objects(&[("AC00000230.Item.Gbx", tiny_assets::AUTHOR), ("AC00000231.Item.Gbx", tiny_assets::AUTHOR)], &zip);
    m2.write_to(Path::new(out)).unwrap();
    let _ = std::fs::remove_file(stage);
    let c = MapFile::load(Path::new(out));
    for i in &arch[..5] { println!("item {} model {} scale {} pos {:?}", i, c.items[*i].model, c.items[*i].scale, c.items[*i].pos); }
}
