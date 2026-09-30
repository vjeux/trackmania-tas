//! `e7_huetargets --pak FILE:KEY … LINK …` — the HueMask colour-table targets the port derives for a material link (the six
//! placement colours), beside the game's RgbBaseColorTarget read from a capture (E7, 2026-09-30: g23's Colorize trims carry
//! MapElemColor byte 1 and the game shades them toward (0, 0.0319, 0.2016)).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut links = Vec::new();
    let mut i = 1;
    while i < a.len() {
        if a[i] == "--pak" { if let Some((p, k)) = a.get(i + 1).and_then(|s| s.rsplit_once(':')) { store.add_pak(p, k).unwrap_or_else(|e| panic!("--pak {p}: {e}")); } i += 2; continue; }
        links.push(a[i].clone());
        i += 1;
    }
    for l in &links {
        match lightmap::setupmap::hue_texture(&mut store, l) {
            Ok(Some((mask, tx, targets))) => {
                println!("{l}: HueMask {mask} ({}×{})", tx.w, tx.h);
                for c in 1..6 { println!("   colour {c}: target ({:.5}, {:.5}, {:.5})", targets[c][0], targets[c][1], targets[c][2]); }
                let mat = if l.to_ascii_uppercase().ends_with(".MATERIAL.GBX") { l.clone() } else { format!("{l}.Material.Gbx") };
                if let Ok(m) = store.load_model(&mat) { for (_, p) in &m.externals { if p.to_ascii_lowercase().contains("colortable") { println!("   table {p}"); if let Ok(b) = store.read(p) { println!("   {}", String::from_utf8_lossy(&b).chars().take(1500).collect::<String>().replace('\n', " ")); } } } }
            }
            Ok(None) => println!("{l}: no HueMask slot"),
            Err(e) => println!("{l}: {e}"),
        }
    }
}
