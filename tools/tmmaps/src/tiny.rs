//! Build a miniature, route-only copy of a map from half-scale block items.
//!
//! The reference U10S Tiny campaign does not scale native map blocks in-place:
//! it replaces the authored route with custom Item.Gbx versions whose geometry
//! is half size, spaces their placements at half distance, and leaves the
//! stadium decoration behind. `tiny` applies the same recipe repeatably.

use crate::{
    census, cli,
    map::{Kind, MapFile, FREE_BLOCK_FLAG},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

const LIBRARY_SCALE: f32 = 0.5;

#[derive(Clone, Debug)]
struct Spec {
    source: String,
    model: String,
    pos: [f32; 3],
    yaw: f32,
    scale: f32,
    tag: Option<String>,
}

fn vec3(s: &str, label: &str) -> [f32; 3] {
    let v: Vec<f32> = s
        .split(',')
        .map(|x| {
            x.trim()
                .parse()
                .unwrap_or_else(|_| panic!("{label} wants x,y,z"))
        })
        .collect();
    assert_eq!(v.len(), 3, "{label} wants x,y,z");
    [v[0], v[1], v[2]]
}

fn read_mapping(path: &Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = BTreeMap::new();
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (block, item) = line.split_once('\t').unwrap_or_else(|| {
            panic!(
                "{}:{}: expected BLOCK<TAB>ITEM",
                path.display(),
                line_no + 1
            )
        });
        assert!(
            !block.is_empty() && !item.is_empty(),
            "{}:{}: empty mapping field",
            path.display(),
            line_no + 1
        );
        assert!(
            out.insert(block.to_string(), item.to_string()).is_none(),
            "{}:{}: duplicate block {block}",
            path.display(),
            line_no + 1
        );
    }
    out
}

fn block_pos(b: &crate::map::BlockRec) -> [f32; 3] {
    b.free_pos.unwrap_or_else(|| census::cell_world(b))
}

fn block_yaw(b: &crate::map::BlockRec) -> f32 {
    if let Some(rot) = b.free_rot {
        return rot[0];
    }
    match b.dir & 3 {
        0 => 0.0,
        1 => -std::f32::consts::FRAC_PI_2,
        2 => std::f32::consts::PI,
        _ => std::f32::consts::FRAC_PI_2,
    }
}

fn transform(
    p: [f32; 3],
    source_anchor: [f32; 3],
    target_anchor: [f32; 3],
    scale: f32,
) -> [f32; 3] {
    [
        target_anchor[0] + (p[0] - source_anchor[0]) * scale,
        target_anchor[1] + (p[1] - source_anchor[1]) * scale,
        target_anchor[2] + (p[2] - source_anchor[2]) * scale,
    ]
}

fn cell_for(p: [f32; 3]) -> (i32, i32, i32) {
    let c = |v: f32, divisor: f32| (v / divisor).floor().clamp(0.0, 255.0) as i32;
    (
        c(p[0], 32.0).min(254),
        c(p[1] + 62.0, 8.0),
        c(p[2], 32.0).min(254),
    )
}

pub fn cmd_batch(args: &[String]) {
    let input = PathBuf::from(&args[2]);
    let output = PathBuf::from(cli::flag(args, "--out").expect("tiny-batch needs --out DIR"));
    std::fs::create_dir_all(&output).expect("create output directory");
    let mut maps: Vec<PathBuf> = std::fs::read_dir(&input)
        .unwrap_or_else(|e| panic!("{}: {e}", input.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.ends_with(".Map.Gbx"))
        })
        .collect();
    maps.sort();
    assert!(
        !maps.is_empty(),
        "{} contains no .Map.Gbx files",
        input.display()
    );
    for source in maps {
        let target = output.join(source.file_name().unwrap());
        let mut one = vec![
            "tmmaps".to_string(),
            "tiny".to_string(),
            source.display().to_string(),
            "--out".to_string(),
            target.display().to_string(),
        ];
        for flag in ["--mapping", "--library", "--scale", "--anchor"] {
            if let Some(value) = cli::flag(args, flag) {
                one.push(flag.to_string());
                one.push(value.to_string());
            }
        }
        cmd(&one);
    }
}

pub fn cmd(args: &[String]) {
    let src = PathBuf::from(&args[2]);
    let out = PathBuf::from(cli::flag(args, "--out").expect("tiny needs --out MAP"));
    let mapping_path =
        PathBuf::from(cli::flag(args, "--mapping").expect("tiny needs --mapping FILE.tsv"));
    let library = cli::flag(args, "--library").map(PathBuf::from);
    let scale: f32 = cli::flag(args, "--scale")
        .unwrap_or("0.5")
        .parse()
        .expect("--scale number");
    assert!(
        scale.is_finite() && scale > 0.0,
        "--scale must be positive and finite"
    );
    let target_anchor = vec3(
        cli::flag(args, "--anchor").unwrap_or("1024,300,1024"),
        "--anchor",
    );
    let mapping = read_mapping(&mapping_path);

    let source = MapFile::load(&src);
    let spawn = source
        .waypoints()
        .into_iter()
        .find(|w| w.kind == Kind::Block && w.tag == "Spawn")
        .expect("map needs a block-carried Spawn");
    let source_anchor = block_pos(&source.blocks[spawn.index]);

    let mut specs = Vec::new();
    let mut skipped = BTreeSet::new();
    for b in &source.blocks {
        if let Some(model) = mapping.get(&b.name) {
            specs.push(Spec {
                source: format!("block#{} {}", b.index, b.name),
                model: model.clone(),
                pos: transform(block_pos(b), source_anchor, target_anchor, scale),
                yaw: block_yaw(b),
                scale: scale / LIBRARY_SCALE,
                tag: b.waypoint_tag.clone(),
            });
        } else if b.waypoint_tag.is_some() {
            // Waypoints may never disappear silently. A built-in gate is a safe
            // fallback when the custom library lacks a checkpoint-shaped block.
            let model = match b.waypoint_tag.as_deref() {
                Some("Spawn") => "GateStart32m",
                Some("Goal") => "GateFinish32m",
                _ => "GateCheckpointLeft32m",
            };
            specs.push(Spec {
                source: format!("block#{} {} (builtin fallback)", b.index, b.name),
                model: model.to_string(),
                pos: transform(block_pos(b), source_anchor, target_anchor, scale),
                yaw: block_yaw(b),
                scale,
                tag: b.waypoint_tag.clone(),
            });
        } else if !b.name.starts_with("Land") && b.name != "Beach" {
            skipped.insert(b.name.clone());
        }
    }
    for it in &source.items {
        if let Some(tag) = &it.waypoint_tag {
            specs.push(Spec {
                source: format!("item#{} {}", it.index, it.model),
                model: it.model.clone(),
                pos: transform(it.pos, source_anchor, target_anchor, scale),
                yaw: it.yaw,
                scale: it.scale * scale,
                tag: Some(tag.clone()),
            });
        }
    }
    assert!(
        specs.len() <= source.items.len(),
        "need {} donor item slots, map has {}",
        specs.len(),
        source.items.len()
    );
    assert!(
        specs.iter().any(|s| s.tag.as_deref() == Some("Spawn")),
        "no Spawn in output"
    );
    assert!(
        specs.iter().any(|s| s.tag.as_deref() == Some("Goal")),
        "no Goal in output"
    );

    let tmp1 = out.with_extension(format!("tiny-{}.stage1.Map.Gbx", std::process::id()));
    let tmp2 = out.with_extension(format!("tiny-{}.stage2.Map.Gbx", std::process::id()));

    let mut m = MapFile::load(&src);
    let old_uid = m
        .body_ids
        .first()
        .and_then(|f| f.name.clone())
        .expect("map uid");
    let new_uid = format!("Tiny{}", &old_uid[..23]);
    m.set_map_uid(&new_uid);
    for i in 0..m.blocks.len() {
        let b = m.blocks[i].clone();
        if b.flags & FREE_BLOCK_FLAG != 0 {
            m.move_block_free(i, [16.0, -1000.0, 16.0]);
        } else {
            m.move_block_cell(i, (0, 0, 0));
        }
        if b.waypoint_tag.is_some() {
            m.set_block_name(i, "RoadTechStraight");
        }
    }
    for i in 0..m.items.len() {
        m.move_item_pos(i, [16.0, -1000.0, 16.0]);
        m.set_item_scale(i, 0.001);
    }
    for (i, s) in specs.iter().enumerate() {
        m.set_item_model(i, &s.model);
        m.move_item(i, s.pos, s.yaw, cell_for(s.pos));
        m.set_item_scale(i, s.scale);
    }
    m.write_to(&tmp1).expect("write stage 1");

    let mut m = MapFile::load(&tmp1);
    for i in 0..m.items.len() {
        m.set_item_waypoint_tag(i, specs.get(i).and_then(|s| s.tag.as_deref()));
    }
    m.write_to(&tmp2).expect("write stage 2");

    let mut m = MapFile::load(&tmp2);
    if let Some(library) = library {
        let zip = std::fs::read(&library).unwrap_or_else(|e| panic!("{}: {e}", library.display()));
        assert!(
            zip.starts_with(b"PK\x03\x04"),
            "{} is not a ZIP archive",
            library.display()
        );
        m.replace_embedded_zip(&zip);
    }
    m.write_to(&out).expect("write output");
    let _ = std::fs::remove_file(&tmp1);
    let _ = std::fs::remove_file(&tmp2);

    let check = MapFile::load(&out);
    assert_eq!(check.items.len(), source.items.len(), "item count changed");
    for (i, s) in specs.iter().enumerate() {
        let got = &check.items[i];
        assert_eq!(got.model, s.model, "item#{i} model");
        assert_eq!(got.waypoint_tag, s.tag, "item#{i} waypoint tag");
        assert!(
            (0..3).all(|k| (got.pos[k] - s.pos[k]).abs() < 0.001),
            "item#{i} position"
        );
        assert!((got.scale - s.scale).abs() < 0.0001, "item#{i} scale");
    }
    println!("wrote {}", out.display());
    println!("  uid: {}", new_uid);
    println!(
        "  route items: {} (from {} mapped blocks)",
        specs.len(),
        specs
            .iter()
            .filter(|s| s.source.starts_with("block#") && !s.source.contains("fallback"))
            .count()
    );
    println!(
        "  anchor: source {:?} -> target {:?}; scale {:.3}",
        source_anchor, target_anchor, scale
    );
    println!(
        "  parked: {} blocks and {} unused items",
        source.blocks.len(),
        source.items.len() - specs.len()
    );
    if !skipped.is_empty() {
        println!(
            "  skipped unmapped block models ({}): {}",
            skipped.len(),
            skipped.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    for (i, s) in specs.iter().enumerate() {
        println!(
            "  item#{i}: {} -> {} {:?} tag={}",
            s.source,
            s.model,
            s.pos,
            s.tag.as_deref().unwrap_or("-")
        );
    }
}
