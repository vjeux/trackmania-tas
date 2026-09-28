// E2: a pak material's chain (parent, shader, flags, bitmaps, params, texcoord transforms) — paks as PATH:KEY args, then material links
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut links = Vec::new();
    for x in &a[1..] { if let Some((p, k)) = x.rsplit_once(':') { if std::path::Path::new(p).exists() { store.add_pak(p, k).expect("pak"); continue; } } links.push(x.clone()); }
    for l in links {
        let mat = if l.to_ascii_uppercase().ends_with(".MATERIAL.GBX") { l.clone() } else { format!("{l}.Material.Gbx") };
        let c = mapgeom::envblock::material_chain(&mut store, &mat);
        println!("{l}: parent {:?} shader {:?} flags {:?} custom {}", c.parent_material, c.shader, c.flags, c.has_custom);
        for (n, p) in &c.bitmaps { println!("  bitmap {n} = {p}"); }
        for (n, v) in &c.params { println!("  param {n} = {v:?}"); }
        for (n, t) in &c.texcoord { println!("  texcoord {n} = {t:?}"); }
    }
    // the decoration layout's groups (material names) for the collection
}
