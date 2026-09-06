//! Vegetation scale control: park everything, then place N copies of one
//! stock vegetation item (default PalmTreeBigB3) side by side at placement
//! scales 1.0, 0.5, 0.25 near the lineup spot -- does the game honour the
//! placement Scale for VegetTreeModel items? (It ignores it for static items.)
//! Usage: vegscale MAP OUT [MODEL] [x y z]
use std::{env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let model = a.get(3).cloned().unwrap_or("PalmTreeBigB3".into());
    let base: [f32; 3] = if a.len() >= 7 { [a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap()] } else { [1344.0, 12.0, 1280.0] };
    let mut m = MapFile::load(Path::new(&a[1]));
    let donors: Vec<usize> = m.items.iter().filter(|it| it.model == model).map(|it| it.index).collect();
    assert!(!donors.is_empty(), "no {model} in the map");
    // clone the first donor so we have 3 placements
    let n = m.items.len();
    m.append_item_clones(n + 3);
    let out0 = Path::new(&a[2]).with_extension("tmp.Map.Gbx");
    m.write_to(&out0).unwrap();
    // renames first (variable-length), then a reload, then the moves
    let mut m = MapFile::load(&out0);
    for k in 0..3 {
        m.set_item_model(n + k, &model);
        m.set_item_author(n + k, "Nadeo");
    }
    m.write_to(&out0).unwrap();
    let mut m = MapFile::load(&out0);
    // uid (a rename) + password (a splice) each in their own pass
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    m.set_map_uid(&format!("VegS{:08X}{:07}{:08X}", nanos % 100_000_000, std::process::id() % 10_000_000, (nanos / 7) % 100_000_000));
    m.write_to(&out0).unwrap();
    let mut m = MapFile::load(&out0);
    m.remove_password();
    for i in 0..m.items.len() {
        m.move_item_pos(i, [16.0, -1000.0, 16.0]);
    }
    for (k, s) in [1.0f32, 0.5, 0.25].iter().enumerate() {
        let i = n + k;
        let pos = [base[0] + 40.0 * k as f32, base[1], base[2]];
        m.move_item(i, pos, 0.0, ((pos[0] / 32.0) as i32, (pos[1] / 8.0) as i32, (pos[2] / 32.0) as i32));
        m.set_item_scale(i, *s);
    }
    m.write_to(Path::new(&a[2])).unwrap();
    let _ = std::fs::remove_file(&out0);
    let c = MapFile::load(Path::new(&a[2]));
    for k in 0..3 {
        let it = &c.items[n + k];
        println!("item {} {} author {:?} scale {} pos {:?}", it.index, it.model, it.author, it.scale, it.pos);
    }
}
