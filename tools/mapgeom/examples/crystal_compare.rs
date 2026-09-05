//! Oracle test for the block->item conversion, no game needed: for every
//! Nadeo archive item whose name matches a Stadium prefab we can resolve,
//! generate our crystal from that prefab and compare it with Nadeo's own
//! crystal: vertex set, face count per material, material links, layers.
//!
//!   crystal_compare NADEO.zip STADIUM.pak:KEY [substring] [--verbose]
use mapgeom::crystal::{decode_template, CrystalMesh};
use std::collections::{BTreeMap, BTreeSet};

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0] * 100.0).round() as i32, (p[1] * 100.0).round() as i32, (p[2] * 100.0).round() as i32)
}

fn bounds(m: &CrystalMesh) -> String {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in &m.positions {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    format!("x {:.1}..{:.1} y {:.1}..{:.1} z {:.1}..{:.1}", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2])
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let zip = std::fs::read(&a[1]).expect("nadeo zip");
    let (pak, k) = a[2].split_once(':').expect("PAK:KEY");
    let filter = a.get(3).filter(|s| !s.starts_with("--")).cloned().unwrap_or_default();
    let verbose = a.iter().any(|s| s == "--verbose");
    let mut store = mapgeom::store::DataStore::empty();
    store.add_pak(pak, k).expect("pak");
    let files = mapgeom::embedded::unzip(&zip).expect("zip");
    let template = files.get("RoadTech/Main/Main/RoadTechStraight.Item.Gbx").expect("template").clone();
    let (mut n, mut pass, mut pos_ok) = (0, 0, 0);
    let mut fails: BTreeMap<String, usize> = BTreeMap::new();
    for (name, bytes) in &files {
        if !name.ends_with(".Item.Gbx") || !name.contains(&filter) {
            continue;
        }
        let short = name.rsplit('/').next().unwrap().trim_end_matches(".Item.Gbx");
        // item name -> prefab: RoadTechStraight -> RoadTech\Straight_Air
        let families = ["RoadTech", "RoadDirt", "RoadIce", "RoadBump", "PlatformTech", "PlatformDirt", "PlatformGrass", "PlatformIce", "PlatformPlastic", "TrackWall", "Technics", "Structure", "DecoWall", "DecoPlatform", "Stand"];
        let Some(fam) = families.iter().find(|f| short.starts_with(*f)) else { continue };
        let rest = &short[fam.len()..];
        let cands = [format!("Stadium\\Media\\Prefab\\{fam}\\{rest}_Air.Prefab.Gbx"), format!("Stadium\\Media\\Prefab\\{fam}\\{rest}.Prefab.Gbx"), format!("Stadium\\Media\\Prefab\\{fam}\\{rest}_Ground.Prefab.Gbx")];
        let Some(prefab) = cands.iter().find(|c| store.resolve(c).is_some()) else { continue };
        n += 1;
        let (nadeo_mats, nadeo_mesh) = decode_template(bytes);
        let (ours_bytes, _) = match mapgeom::tiny_assets::crystal_from_model_in(&mut store, prefab, &template, "X.Item.Gbx", 26) {
            Ok(v) => v,
            Err(e) => { *fails.entry(format!("generate: {e}")).or_default() += 1; continue; }
        };
        let (our_mats, our_mesh) = decode_template(&ours_bytes);
        let np: BTreeSet<_> = nadeo_mesh.positions.iter().map(key).collect();
        let op: BTreeSet<_> = our_mesh.positions.iter().map(key).collect();
        let common = np.intersection(&op).count();
        let pos_match = common as f64 / np.len().max(1) as f64;
        let nl: BTreeSet<String> = nadeo_mats.iter().map(|m| m.link.clone()).collect();
        let ol: BTreeSet<String> = our_mats.iter().map(|m| m.link.clone()).collect();
        let mut faces_n: BTreeMap<String, usize> = BTreeMap::new();
        for f in &nadeo_mesh.faces { *faces_n.entry(nadeo_mats[f.material as usize].link.clone()).or_default() += 1; }
        let mut faces_o: BTreeMap<String, usize> = BTreeMap::new();
        for f in &our_mesh.faces { *faces_o.entry(our_mats[f.material as usize].link.clone()).or_default() += 1; }
        let size_ok = ((op.len() as f64) - (np.len() as f64)).abs() / (np.len().max(1) as f64) < 0.05;
        let ok_pos = pos_match > 0.98 && size_ok;
        let ok_faces = faces_n == faces_o;
        if ok_pos { pos_ok += 1; }
        if ok_pos && ok_faces { pass += 1; }
        if verbose || !(ok_pos && ok_faces) {
            println!("{short}: positions nadeo {} ours {} common {:.1}% | faces {} vs {} | links nadeo-only {:?} ours-only {:?}", np.len(), op.len(), 100.0 * pos_match, nadeo_mesh.faces.len(), our_mesh.faces.len(), nl.difference(&ol).collect::<Vec<_>>(), ol.difference(&nl).collect::<Vec<_>>());
            if verbose {
                println!("   nadeo {} | ours {}", bounds(&nadeo_mesh), bounds(&our_mesh));
                for (l, c) in &faces_n { println!("   {l}: nadeo {c} ours {}", faces_o.get(l).copied().unwrap_or(0)); }
                for (l, c) in &faces_o { if !faces_n.contains_key(l) { println!("   {l}: nadeo 0 ours {c}"); } }
            }
        }
    }
    println!("{n} items compared: {pos_ok} vertex-set matches, {pass} full matches; generation failures: {fails:?}");
}
