//! `tinyctl material-census --build DIR --set Dirt|Ice|Grass|Plastic` — the look gate over a
//! rollout build: every baked item of every map is read for the material links it carries,
//! and a wall that must stay WOOD but carries the set's `Modifier\Platform<S>\TrackWall`
//! dress is a violation. The rule is Everios96's test-map rule (2026-10-05/08, restated
//! 2026-10-23 on Dirt 26: "again you didn't properly convert the black blocks based on the
//! test maps we created, can you always check that please?"):
//!
//! - `TrackWall*` (the wall-ride family: walls, arches, their generated caps and panels)
//!   keeps its wood; only the metal rim (`TrackWallClips`, another stem) and the halfpipe
//!   insides (`*SlopeU*`, `*SlopeUp*`, `*SlopeDown*`, `TrackWallArchSlope*U*`) take the set;
//! - the full-block family's side panels (`DecoWall*VFC*`) and undersides (`*FCB`) keep
//!   their pillar look — never the set's TrackWall dress.
//!
//! Each map dir (`<build>/<set>/pNN/<set>NN/`) carries `report.tsv` (item id ↔ block kind)
//! and `libx/Items/<id>.Item.Gbx`; the link is a plain string in the item file, so a byte
//! search is the whole parser. Exit 1 on any violation — a build that fails this gate is
//! never published (vjeux: what a pipeline cannot produce FAILS the build).

use std::path::{Path, PathBuf};

fn ride_face(name: &str) -> bool {
    name.contains("SlopeU") || name.contains("SlopeUp") || name.contains("SlopeDown") || (name.contains("ArchSlope") && name.contains('U'))
}

/// Does an item made from block kind `name` have to stay clear of the set's TrackWall dress?
fn must_stay_wood(name: &str) -> bool {
    if name.starts_with("TrackWall") {
        return !ride_face(name);
    }
    if name.starts_with("DecoWall") {
        return name.contains("VFC") || name.ends_with("FCB");
    }
    false
}

fn map_dirs(build: &Path, set: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let root = build.join(set);
    for part in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        if !part.path().is_dir() {
            continue;
        }
        for map in std::fs::read_dir(part.path()).into_iter().flatten().flatten() {
            if map.path().join("report.tsv").is_file() {
                out.push(map.path());
            }
        }
    }
    out.sort();
    out
}

pub fn cmd(args: &[String]) -> i32 {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let Some(build) = f("--build") else {
        eprintln!("material-census needs --build DIR --set Dirt|Ice|Grass|Plastic");
        return 2;
    };
    let Some(set) = f("--set") else {
        eprintln!("material-census needs --set Dirt|Ice|Grass|Plastic");
        return 2;
    };
    let folder = match set.as_str() {
        "Dirt" | "dirt" => "PlatformDirt",
        "Ice" | "Icy" | "ice" | "icy" => "PlatformIce",
        "Grass" | "grass" => "PlatformGrass",
        "Plastic" | "plastic" => "PlatformPlastic",
        other => {
            eprintln!("material-census: no forced modifier folder for set {other:?} — nothing to check");
            return 0;
        }
    };
    let set_dir = match set.to_ascii_lowercase().as_str() {
        "ice" => "icy".to_string(),
        s => s.to_string(),
    };
    // `--any-modifier`: flag ANY modifier-folder TrackWall dress on a wood-rule item (a
    // neighbour's hill dress counts too), not just the set's own folder
    let any = args.iter().any(|a| a == "--any-modifier");
    let needle = if any { "\\TrackWall".to_string() } else { format!("Modifier\\{folder}\\TrackWall") };
    let needle_b = needle.as_bytes();
    let modifier_b = b"Modifier\\";
    let dirs = map_dirs(Path::new(&build), &set_dir);
    if dirs.is_empty() {
        eprintln!("material-census: no map dirs under {build}/{set_dir}");
        return 2;
    }
    let mut maps = 0usize;
    let mut items = 0usize;
    let mut missing = 0usize;
    let mut violations: Vec<String> = Vec::new();
    for d in &dirs {
        maps += 1;
        let report = std::fs::read_to_string(d.join("report.tsv")).unwrap_or_default();
        // a GENERATED FACE item is judged by its OWNER (faces.tsv: baked index → owner name,
        // placements.tsv: b@index → item): a pillar's faces and a hill's or platform's side
        // panels carry the parent's dress by the game's own rule (Dirt 3: pillars under dirt
        // hills are orange in the original too); only a black block's own panels (owner
        // `DecoWall*`, not a pillar) must stay wood
        let faces = std::fs::read_to_string(d.join("faces.tsv")).unwrap_or_default();
        let owner_of: std::collections::HashMap<String, String> = faces
            .lines()
            .skip(1)
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                (c.len() >= 4).then(|| (c[0].to_string(), c[3].to_string()))
            })
            .collect();
        let placements = std::fs::read_to_string(d.join("placements.tsv")).unwrap_or_default();
        let mut face_owners: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for l in placements.lines() {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() >= 2 && c[0].starts_with("b@") && c[1].ends_with(".Item.Gbx") {
                if let Some(o) = owner_of.get(&c[0][2..]) {
                    face_owners.entry(c[1].trim_end_matches(".Item.Gbx").to_string()).or_default().push(o.clone());
                }
            }
        }
        let mut wanted: Vec<(String, String)> = Vec::new();
        for line in report.lines() {
            let c: Vec<&str> = line.split('\t').collect();
            // block <id> <count> OK <Name flags [variant] prefab> …
            if c.len() < 5 || c[0] != "block" || c[1] == "-" {
                continue;
            }
            let name = c[4].split(' ').next().unwrap_or("");
            if !must_stay_wood(name) {
                continue;
            }
            if let Some(owners) = face_owners.get(c[1]) {
                // a face item: wood-rule only when a black block (not a pillar) owns it
                let black_owner = owners.iter().any(|o| o.starts_with("DecoWall") && !o.contains("Pillar") && !o.contains("Water"));
                let wall_owner = owners.iter().any(|o| o.starts_with("TrackWall") && !o.contains("Pillar") && !ride_face(o));
                if !black_owner && !wall_owner {
                    continue;
                }
            }
            wanted.push((c[1].to_string(), name.to_string()));
        }
        if wanted.is_empty() {
            continue;
        }
        // the items live in the map's embedded zip (a rollout build keeps no libx/ dir);
        // `libx/Items/<id>.Item.Gbx` is the fallback for a hand build
        let map_file = std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).find(|p| p.extension().map(|x| x == "Gbx").unwrap_or(false) && p.to_string_lossy().ends_with(".Map.Gbx"));
        let zipped: std::collections::HashMap<String, Vec<u8>> = match &map_file {
            Some(mf) => {
                let m = tmmaps::map::MapFile::load(mf);
                match m.embedded_objects() {
                    Some((_, zip)) => tmmaps::header::zip_entries(&zip).into_iter().map(|(n, b)| (n.rsplit('/').next().unwrap_or(&n).rsplit('\\').next().unwrap_or(&n).to_string(), b)).collect(),
                    None => Default::default(),
                }
            }
            None => Default::default(),
        };
        for (id, name) in wanted {
            let fname = format!("{id}.Item.Gbx");
            let bytes: Vec<u8> = match zipped.get(&fname) {
                Some(b) => b.clone(),
                None => match std::fs::read(d.join("libx").join("Items").join(&fname)) {
                    Ok(b) => b,
                    Err(_) => {
                        missing += 1;
                        continue;
                    }
                },
            };
            items += 1;
            let hit = if any {
                // a `Modifier\<Folder>\TrackWall` link of any folder
                bytes.windows(needle_b.len()).enumerate().any(|(i, w)| w == needle_b && bytes[..i].ends_with(&[]) && bytes[i.saturating_sub(64)..i].windows(modifier_b.len()).any(|m| m == modifier_b))
            } else {
                bytes.windows(needle_b.len()).any(|w| w == needle_b)
            };
            if hit {
                violations.push(format!("{}\t{}\t{}", d.display(), id, name));
            }
        }
    }
    println!("material-census {set}: {maps} maps, {items} wood-rule items checked ({missing} item files not found), {} violation(s)", violations.len());
    if missing > 0 && items == 0 {
        println!("  no item could be read — the census saw nothing; treated as a FAILURE");
        return 1;
    }
    for v in violations.iter().take(40) {
        println!("  VIOLATION\t{v}");
    }
    if violations.len() > 40 {
        println!("  … {} more", violations.len() - 40);
    }
    if violations.is_empty() {
        0
    } else {
        1
    }
}
