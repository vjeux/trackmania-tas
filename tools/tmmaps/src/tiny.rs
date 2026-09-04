//! Build miniature maps by replacing authored blocks with Item.Gbx models and
//! scaling every authored item. Baked decoration/terrain is deliberately left
//! alone: it is the map's foundation, just as the U10S Tiny reference maps keep
//! their baked Grass floor at full size.

use crate::{
    census, cli,
    map::{MapFile, FREE_BLOCK_FLAG},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
struct Mapping {
    model: String,
    model_scale: f32,
}

#[derive(Clone, Debug)]
struct Spec {
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

/// `BLOCK<TAB>ITEM[<TAB>MODEL_SCALE]`. MODEL_SCALE is the scale already baked
/// into the item geometry: 1 for full-size block exports, 0.5 for the U10S
/// pre-shrunk library. Placement scale is `requested / MODEL_SCALE`.
fn read_mapping(path: &Path) -> BTreeMap<String, Mapping> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = BTreeMap::new();
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        assert!(
            fields.len() == 2 || fields.len() == 3,
            "{}:{}: expected BLOCK<TAB>ITEM[<TAB>MODEL_SCALE]",
            path.display(),
            line_no + 1
        );
        let model_scale = fields
            .get(2)
            .map_or(1.0, |s| s.parse::<f32>().expect("MODEL_SCALE number"));
        assert!(model_scale.is_finite() && model_scale > 0.0);
        assert!(
            out.insert(
                fields[0].to_string(),
                Mapping {
                    model: fields[1].to_string(),
                    model_scale,
                },
            )
            .is_none(),
            "{}:{}: duplicate block {}",
            path.display(),
            line_no + 1,
            fields[0]
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
                .is_some_and(|n| n.ends_with(".Map.Gbx"))
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
    let library =
        PathBuf::from(cli::flag(args, "--library").expect("tiny needs --library ITEMS.zip"));
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
        .find(|w| w.kind == crate::map::Kind::Block && w.tag == "Spawn")
        .expect("map needs a block-carried Spawn");
    let source_anchor = block_pos(&source.blocks[spawn.index]);

    // ALL authored blocks are required. A missing model is a refusal, never a
    // silently omitted decoration that makes the output look "mostly tiny".
    let missing: BTreeSet<&str> = source
        .blocks
        .iter()
        .filter(|b| !mapping.contains_key(&b.name))
        .map(|b| b.name.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "mapping is missing {} authored block model(s): {}",
        missing.len(),
        missing.into_iter().collect::<Vec<_>>().join(", ")
    );

    let mut specs = Vec::with_capacity(source.blocks.len() + source.items.len());
    for b in &source.blocks {
        let map = &mapping[&b.name];
        specs.push(Spec {
            model: map.model.clone(),
            pos: transform(block_pos(b), source_anchor, target_anchor, scale),
            yaw: block_yaw(b),
            scale: scale / map.model_scale,
            tag: b.waypoint_tag.clone(),
        });
    }
    // Existing items need no replacement geometry: scale and position are
    // native placement fields, so every decoration and item waypoint survives.
    for it in &source.items {
        specs.push(Spec {
            model: it.model.clone(),
            pos: transform(it.pos, source_anchor, target_anchor, scale),
            yaw: it.yaw,
            scale: it.scale * scale,
            tag: it.waypoint_tag.clone(),
        });
    }
    assert!(specs.iter().any(|s| s.tag.as_deref() == Some("Spawn")));
    assert!(specs.iter().any(|s| s.tag.as_deref() == Some("Goal")));

    let tmp0 = out.with_extension(format!("tiny-{}.slots.Map.Gbx", std::process::id()));
    let tmp1 = out.with_extension(format!("tiny-{}.models.Map.Gbx", std::process::id()));
    let tmp2 = out.with_extension(format!("tiny-{}.waypoints.Map.Gbx", std::process::id()));

    // Stage 0: grow the item array before any saved offsets are used.
    let mut m = MapFile::load(&src);
    m.append_item_clones(specs.len());
    m.write_to(&tmp0).expect("write item-slot stage");

    // Stage 1: fixed-size placement edits and Id-table model changes.
    let mut m = MapFile::load(&tmp0);
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
    for (i, s) in specs.iter().enumerate() {
        m.set_item_model(i, &s.model);
        m.move_item(i, s.pos, s.yaw, cell_for(s.pos));
        m.set_item_scale(i, s.scale);
    }
    m.write_to(&tmp1).expect("write model stage");

    // Stage 2: variable-length waypoint nodes.
    let mut m = MapFile::load(&tmp1);
    for (i, s) in specs.iter().enumerate() {
        m.set_item_waypoint_tag(i, s.tag.as_deref());
    }
    m.write_to(&tmp2).expect("write waypoint stage");

    // Stage 3: embed the converted block models. A non-empty source archive
    // would also need merging; replacing it would silently drop custom items.
    assert!(
        crate::header::embedded_zip_data(&source.gbx.body).is_none(),
        "source map already embeds custom objects; merge them into --library before converting"
    );
    let zip = std::fs::read(&library).unwrap_or_else(|e| panic!("{}: {e}", library.display()));
    assert!(
        zip.starts_with(b"PK\x03\x04"),
        "{} is not a ZIP archive",
        library.display()
    );
    let mut m = MapFile::load(&tmp2);
    m.replace_embedded_zip(&zip);
    m.write_to(&out).expect("write output");
    for p in [&tmp0, &tmp1, &tmp2] {
        let _ = std::fs::remove_file(p);
    }

    let check = MapFile::load(&out);
    assert_eq!(check.items.len(), specs.len(), "item count changed");
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
        "  scaled every authored object: {} blocks + {} items = {} item placements",
        source.blocks.len(),
        source.items.len(),
        specs.len()
    );
    println!(
        "  baked foundation unchanged: {} generated terrain/decoration blocks",
        source.baked.len()
    );
    println!(
        "  anchor: source {:?} -> target {:?}; scale {:.3}",
        source_anchor, target_anchor, scale
    );
}
