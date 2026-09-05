//! Tiny test map for ONE item: every item of the host map parked far away,
//! one arch replaced by the item under test at scale S next to a stock arch.
//! Optionally embeds the item file (ident, author) instead of relying on a
//! local copy in Documents/Trackmania/Items.
//! usage: single_item_map HOST OUT IDENT AUTHOR SCALE [ITEM_FILE_TO_EMBED]
use mapgeom::tiny_assets;
use std::{collections::BTreeMap, env, path::Path};
use tmmaps::map::MapFile;
fn main() {
    let a: Vec<String> = env::args().collect();
    let (host, out, ident, author, scale) = (&a[1], &a[2], &a[3], &a[4], a[5].parse::<f32>().unwrap());
    let embed = a.get(6).map(|p| std::fs::read(p).unwrap());
    let mut m = MapFile::load(Path::new(host));
    m.set_map_uid(&format!("One{:024}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() % 10u128.pow(24)));
    let mut arch: Vec<usize> = m.items.iter().filter(|it| it.model == "TunnelSupportArch16m").map(|it| it.index).collect();
    if arch.len() < 3 {
        arch = (0..3).collect(); // a host without three arches: borrow any placements
    }
    let anchor = std::env::var("HOST_ANCHOR").ok().and_then(|s| { let v: Vec<f32> = s.split(',').filter_map(|x| x.parse().ok()).collect(); if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None } }).unwrap_or([96.0, 16.0, 128.0]);
    let cell = |p: [f32; 3]| ((p[0] / 32.0) as i32, ((p[1] + 64.0) / 8.0) as i32, (p[2] / 32.0) as i32);
    for i in 0..m.items.len() { m.move_item_pos(i, [16.0, -1000.0, 16.0]); }
    m.move_item(arch[0], anchor, 0.0, cell(anchor));
    m.set_item_scale(arch[0], 1.0);
    // Two copies of the item under test: scale 1 beside the reference, scale S further on.
    let p1 = [anchor[0] + 64.0, anchor[1], anchor[2]];
    let p2 = [anchor[0] + 128.0, anchor[1], anchor[2]];
    m.move_item(arch[1], p1, 0.0, cell(p1));
    m.set_item_scale(arch[1], 1.0);
    m.set_item_model(arch[1], ident);
    m.set_item_author(arch[1], author);
    m.move_item(arch[2], p2, 0.0, cell(p2));
    m.set_item_scale(arch[2], scale);
    m.set_item_model(arch[2], ident);
    m.set_item_author(arch[2], author);
    // COPIES=N: N more placements of the item in a grid behind the pair, so a
    // per-instance-count failure (a crash that needs hundreds of copies)
    // reproduces in a one-item map.
    if let Ok(n) = std::env::var("COPIES") {
        let n: usize = n.parse().unwrap();
        let used: std::collections::HashSet<usize> = arch.iter().copied().collect();
        let mut free: Vec<usize> = (0..m.items.len()).filter(|i| !used.contains(i)).collect();
        free.truncate(n);
        for (k, &i) in free.iter().enumerate() {
            let p = [anchor[0] + (k % 20) as f32 * 40.0, anchor[1], anchor[2] + 64.0 + (k / 20) as f32 * 40.0];
            m.move_item(i, p, 0.0, cell(p));
            m.set_item_scale(i, scale);
            m.set_item_model(i, ident);
            m.set_item_author(i, author);
        }
        eprintln!("{} extra copies placed", free.len());
    }
    let stage = Path::new(out).with_extension("stage.Map.Gbx");
    m.write_to(&stage).unwrap();
    let mut m2 = MapFile::load(&stage);
    m2.remove_password();
    if let Some(bytes) = embed {
        let mut files = BTreeMap::new();
        files.insert(format!("Items/{}", ident.replace('\\', "/")), bytes);
        m2.replace_embedded_objects(&[(ident.as_str(), author.as_str())], &tiny_assets::zip(&files));
    }
    m2.write_to(Path::new(out)).unwrap();
    let _ = std::fs::remove_file(stage);
    println!("test map: {} at x=160 (scale 1) and x=224 (scale {}) {}", ident, scale, if a.get(6).is_some() { "EMBEDDED" } else { "LOCAL" });
}
