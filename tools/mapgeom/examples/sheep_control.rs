//! Placement-rename control on the Summer map: the untouched Sheep item from
//! a community map, embedded as Items/Sheep.Item.Gbx, with ten palms
//! re-pointed at it through the same rename path the converter uses.
use mapgeom::tiny_assets;
use std::{collections::BTreeMap, env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let sheep = std::fs::read(&a[3]).unwrap();
    let mut files = BTreeMap::new();
    files.insert("Items/Sheep.Item.Gbx".to_string(), sheep);
    let zip = tiny_assets::zip(&files);
    let author = "_u37dSLySJ-yOSlMoNO6vQ";
    let mut m = MapFile::load(Path::new(&a[1]));
    m.set_map_uid(&format!("Sheep{:022}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()));
    let palms: Vec<usize> = m.items.iter().filter(|it| it.model == "PalmForest").map(|it| it.index).take(10).collect();
    for &i in &palms { m.set_item_model(i, "Sheep.Item.Gbx"); m.set_item_author(i, author); m.set_item_scale(i, 1.0); }
    let stage = Path::new(&a[2]).with_extension("stage.Map.Gbx");
    m.write_to(&stage).unwrap();
    let mut m2 = MapFile::load(&stage);
    m2.remove_password();
    m2.replace_embedded_objects(&[("Sheep.Item.Gbx", author)], &zip);
    m2.write_to(Path::new(&a[2])).unwrap();
    let _ = std::fs::remove_file(stage);
    let c = MapFile::load(Path::new(&a[2]));
    println!("sheep placements {}", c.items.iter().filter(|it| it.model == "Sheep.Item.Gbx").count());
}
