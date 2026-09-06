//! Static-library mapping for tiny-catalog: alias i = prefabs.txt line i
//! (the order /tmp/static-lib was baked in). Writes placements TSV +
//! deflated library zip from a directory of AC########.Item.Gbx files.
//! Usage: static_catalog_map RESOLVED.TSV PREFABS.TXT FOOTPRINTS.TSV SUMMER.Map.Gbx LIBDIR OUTBASE [--scale S]
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scale: f32 = a.iter().position(|x| x == "--scale").and_then(|i| a.get(i + 1)).map(|s| s.parse().unwrap()).unwrap_or(0.5);
    let prefabs: Vec<String> = std::fs::read_to_string(&a[2]).unwrap().lines().map(|s| s.to_string()).filter(|s| !s.is_empty()).collect();
    let alias_of: BTreeMap<&str, usize> = prefabs.iter().enumerate().map(|(i, p)| (p.as_str(), i)).collect();
    let mut catalog = BTreeMap::new();
    for line in std::fs::read_to_string(&a[1]).unwrap().lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split('\t').collect();
        catalog.insert((f[0].to_string(), f[1].to_string()), f[2].to_string());
    }
    let mut fp = BTreeMap::new();
    for line in std::fs::read_to_string(&a[3]).unwrap().lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split('\t').collect();
        fp.insert(f[0].to_string(), (f[1].to_string(), f[2].to_string()));
    }
    let source = tmmaps::map::MapFile::load(Path::new(&a[4]));
    let libdir = &a[5];
    let outbase = &a[6];
    let mut mapping = String::new();
    let mut used = BTreeSet::new();
    // Blocks without a prefab bake (legacy walls/pillars) keep the crystal
    // pipeline's fallback aliases; the catalog parks them un-itemized under
    // --only, so they need no row here.
    let fallback = |name: &str, flags: u32| match name {
        "PlatformBase" => Some("AC00000101"),
        "DecoWallSlope2Straight" => Some("AC00000102"),
        "DecoWallBasePillar" => Some("AC00000103"),
        "StructurePillar" if flags == 0x0000_4001 => Some("AC00000104"),
        _ => None,
    };
    for b in &source.blocks {
        if fallback(&b.name, b.flags).is_some() {
            continue;
        }
        let prefab = catalog.get(&(b.name.clone(), format!("{:08X}", b.flags))).unwrap_or_else(|| panic!("no prefab for {} {:08X}", b.name, b.flags));
        let i = alias_of.get(prefab.as_str()).unwrap_or_else(|| panic!("prefab not in prefabs.txt: {prefab}"));
        let alias = format!("AC{i:08}");
        used.insert(alias.clone());
        let (sx, sz) = &fp[&b.name];
        mapping.push_str(&format!("@{}\t{alias}.Item.Gbx\t{scale}\t{sx}\t{sz}\n", b.index));
    }
    std::fs::write(format!("{outbase}.placements.tsv"), &mapping).unwrap();
    let mut files = BTreeMap::new();
    for alias in &used {
        let bytes = std::fs::read(format!("{libdir}/{alias}.Item.Gbx")).unwrap_or_else(|_| panic!("missing {libdir}/{alias}.Item.Gbx"));
        files.insert(format!("Items/{alias}.Item.Gbx"), bytes);
    }
    let zip = mapgeom::tiny_assets::zip(&files);
    std::fs::write(format!("{outbase}.zip"), &zip).unwrap();
    println!("static library: {} items, {} bytes; mapping for {} blocks", files.len(), zip.len(), source.blocks.len());
}
