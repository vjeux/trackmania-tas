//! `re17_modcensus --pak F:K … --mod DIR MATERIALS.txt` — which of a map's item materials bind a texture the MOD replaces:
//! per material link (rows `count material <link>` as `sort | uniq -c` prints them) the bitmap slots through
//! `mapgeom::envblock::material_chain` (the material's own CPlugMaterialCustom and its parents), each slot's `.Texture.gbx`
//! stem matched against the mod's `Image/<stem>.dds` files (the game's mod rule: a mod's Image/<name>.dds replaces the
//! collection texture whose image file has that name). RE 17 2026-09-29 22:45Z — g23's NationsNORWAY mod (33 textures).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut i = 1;
    let mut mod_dir = String::new();
    let mut list = String::new();
    while i < a.len() {
        match a[i].as_str() {
            "--pak" => { let (p, k) = a[i + 1].rsplit_once(':').expect("FILE:KEY"); store.add_pak(p, k).unwrap_or_else(|e| panic!("{p}: {e}")); i += 2; }
            "--mod" => { mod_dir = a[i + 1].clone(); i += 2; }
            x => { list = x.to_string(); i += 1; }
        }
    }
    let mut mod_names: std::collections::BTreeSet<String> = Default::default();
    for e in std::fs::read_dir(format!("{mod_dir}/Image")).expect("mod Image dir") {
        let n = e.unwrap().file_name().to_string_lossy().to_string();
        if let Some(stem) = n.strip_suffix(".dds") { mod_names.insert(stem.to_ascii_lowercase()); }
    }
    eprintln!("mod: {} textures", mod_names.len());
    let text = std::fs::read_to_string(&list).expect("materials list");
    let mut hit_total = 0usize;
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 3 || cols[1] != "material" { continue; }
        let count: usize = cols[0].parse().unwrap_or(0);
        let link = cols[2..].join(" ");
        if !link.contains('\\') { continue; }
        let mat = if link.to_ascii_uppercase().ends_with(".MATERIAL.GBX") { link.clone() } else { format!("{link}.Material.Gbx") };
        let chain = mapgeom::envblock::material_chain(&mut store, &mat);
        let mut slots: Vec<String> = Vec::new();
        let mut hits: Vec<String> = Vec::new();
        for (slot, tex) in &chain.bitmaps {
            if tex.is_empty() { continue; }
            let stem = tex.rsplit(['\\', '/']).next().unwrap_or(tex);
            let stem = stem.strip_suffix(".Texture.gbx").or_else(|| stem.strip_suffix(".Texture.Gbx")).unwrap_or(stem);
            slots.push(format!("{slot}={stem}"));
            if mod_names.contains(&stem.to_ascii_lowercase()) { hits.push(format!("{slot}={stem}")); }
        }
        if !hits.is_empty() { hit_total += count; }
        println!("{}\t{count}\t{link}\tparent {}\t{}", if hits.is_empty() { "  -  " } else { "MOD  " }, chain.parent_material, if hits.is_empty() { slots.join(" ") } else { format!("HITS {}", hits.join(" ")) });
    }
    eprintln!("{hit_total} material rows (geoms) bind a mod-replaced texture");
}
