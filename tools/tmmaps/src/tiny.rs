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
    /// Footprint in cells (x, z) of the block's selected variant. The prefab
    /// geometry is authored from the block's local corner, so a rotated block
    /// must be shifted by its footprint to stay on its own cells.
    footprint: Option<(u32, u32)>,
}

#[derive(Clone, Debug)]
struct Spec {
    model: String,
    pos: [f32; 3],
    yaw: f32,
    /// `Some` re-bases the whole placement frame (yaw, pitch, roll, pivot).
    /// Original items keep their own frame and use `None`.
    frame: Option<([f32; 3], [f32; 3])>,
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

#[derive(Default)]
struct Mappings {
    by_name: BTreeMap<String, Mapping>,
    by_index: BTreeMap<usize, Mapping>,
    /// Original ITEM placements re-pointed at an embedded copy of their own
    /// model (`i@INDEX` rows). Items without a row keep their model.
    items_by_index: BTreeMap<usize, Mapping>,
}

/// `BLOCK<TAB>ITEM[<TAB>MODEL_SCALE]`, or `@INDEX<TAB>...` for an exact block
/// placement, or `i@INDEX<TAB>...` for an existing item placement. Index rows
/// win over block-name rows. MODEL_SCALE is the scale already baked into the
/// item geometry.
fn read_mapping(path: &Path) -> Mappings {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = Mappings::default();
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        assert!(
            (2..=5).contains(&fields.len()) && fields.len() != 4,
            "{}:{}: expected BLOCK<TAB>ITEM[<TAB>MODEL_SCALE[<TAB>SX<TAB>SZ]]",
            path.display(),
            line_no + 1
        );
        let model_scale = fields
            .get(2)
            .map_or(1.0, |s| s.parse::<f32>().expect("MODEL_SCALE number"));
        assert!(model_scale.is_finite() && model_scale > 0.0);
        let footprint = if fields.len() == 5 {
            let sx: u32 = fields[3].parse().expect("SX cells");
            let sz: u32 = fields[4].parse().expect("SZ cells");
            assert!(sx >= 1 && sz >= 1, "footprint must be at least 1x1");
            Some((sx, sz))
        } else {
            None
        };
        let mapping = Mapping {
            model: fields[1].to_string(),
            model_scale,
            footprint,
        };
        let prev = if let Some(index) = fields[0].strip_prefix("i@") {
            out.items_by_index
                .insert(index.parse().expect("i@INDEX number"), mapping)
        } else if let Some(index) = fields[0].strip_prefix('@') {
            out.by_index
                .insert(index.parse().expect("@INDEX number"), mapping)
        } else {
            out.by_name.insert(fields[0].to_string(), mapping)
        };
        assert!(
            prev.is_none(),
            "{}:{}: duplicate mapping {}",
            path.display(),
            line_no + 1,
            fields[0]
        );
    }
    out
}

thread_local! {
    /// World y of cell row 0 for the map being converted (see `map::ground_y`).
    static GROUND_Y: std::cell::Cell<f32> = const { std::cell::Cell::new(-62.0) };
}
fn set_ground(collection: u32) {
    GROUND_Y.with(|g| g.set(crate::map::ground_y(collection)));
    println!("  ground: cell row 0 at y {} (collection {collection:#x})", crate::map::ground_y(collection));
}
fn ground() -> f32 {
    GROUND_Y.with(|g| g.get())
}

fn block_pos(b: &crate::map::BlockRec) -> [f32; 3] {
    b.free_pos.unwrap_or_else(|| {
        let mut p = census::cell_world(b);
        p[1] = p[1] + 62.0 + ground();
        p
    })
}

/// The world point a block's prefab geometry is authored from: the cell's
/// low corner, shifted by the footprint so a quarter-turned block still covers
/// its own cells (the pairing measured in `mapgeom::place::grid_block`).
fn block_origin(b: &crate::map::BlockRec, footprint: (u32, u32)) -> [f32; 3] {
    if let Some(p) = b.free_pos {
        return p;
    }
    let (cx, cy, cz) = b.coords();
    let sx = footprint.0 as f32 * crate::map::CELL_XZ;
    let sz = footprint.1 as f32 * crate::map::CELL_XZ;
    let shift = match b.dir & 3 {
        0 => [0.0, 0.0],
        1 => [sz, 0.0],
        2 => [sx, sz],
        _ => [0.0, sx],
    };
    [
        cx as f32 * crate::map::CELL_XZ + shift[0],
        cy as f32 * crate::map::CELL_Y + ground(),
        cz as f32 * crate::map::CELL_XZ + shift[1],
    ]
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
        c(p[1] - ground(), 8.0),
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
    // --host HOST.Map.Gbx: build the copy INTO another map (e.g. an empty
    // Stadium map, where real Stadium materials are accepted) instead of into
    // the parked source. Placements still come from the source.
    let host: Option<PathBuf> = cli::flag(args, "--host").map(PathBuf::from);

    let source = MapFile::load(&src);
    set_ground(source.items.first().map(|it| it.collection_raw).unwrap_or(26));
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
        .filter(|b| {
            !mapping.by_index.contains_key(&b.index) && !mapping.by_name.contains_key(&b.name)
        })
        .map(|b| b.name.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "mapping is missing {} authored block model(s): {}",
        missing.len(),
        missing.into_iter().collect::<Vec<_>>().join(", ")
    );

    let mut specs = Vec::with_capacity(source.blocks.len() + source.items.len());
    // Preserve original item records and their lookback IDs in place. They only
    // need fixed-size placement edits.
    let mut repointed_items = 0usize;
    for it in &source.items {
        match mapping.items_by_index.get(&it.index) {
            // Re-pointed at an embedded copy whose geometry already carries
            // the scale: the placement stays where it is at scale 1.
            Some(map) => {
                repointed_items += 1;
                specs.push(Spec {
                    model: map.model.clone(),
                    pos: transform(it.pos, source_anchor, target_anchor, scale),
                    yaw: it.yaw,
                    frame: None,
                    scale: it.scale * scale / map.model_scale,
                    tag: it.waypoint_tag.clone(),
                });
            }
            None => specs.push(Spec {
                model: it.model.clone(),
                pos: transform(it.pos, source_anchor, target_anchor, scale),
                yaw: it.yaw,
                frame: None,
                scale: it.scale * scale,
                tag: it.waypoint_tag.clone(),
            }),
        }
    }
    let original_items = specs.len();
    // Authored blocks occupy appended clones. Each mapped model name is exactly
    // ten bytes, matching the clone donor `PalmForest`, so changing it does not
    // rebuild or renumber the lookback table.
    for b in &source.blocks {
        let map = mapping
            .by_index
            .get(&b.index)
            .or_else(|| mapping.by_name.get(&b.name))
            .expect("mapping checked above");
        let rot = b.free_rot.unwrap_or([block_yaw(b), 0.0, 0.0]);
        let origin = match map.footprint {
            Some(fp) => block_origin(b, fp),
            None => block_pos(b),
        };
        specs.push(Spec {
            model: map.model.clone(),
            pos: transform(origin, source_anchor, target_anchor, scale),
            yaw: rot[0],
            frame: Some((rot, [0.0, 0.0, 0.0])),
            scale: scale / map.model_scale,
            tag: b.waypoint_tag.clone(),
        });
    }
    assert!(specs.iter().any(|s| s.tag.as_deref() == Some("Spawn")));
    assert!(specs.iter().any(|s| s.tag.as_deref() == Some("Goal")));

    let tmp0 = out.with_extension(format!("tiny-{}.slots.Map.Gbx", std::process::id()));
    let tmp1 = out.with_extension(format!("tiny-{}.models.Map.Gbx", std::process::id()));
    let tmp2 = out.with_extension(format!("tiny-{}.waypoints.Map.Gbx", std::process::id()));

    // Stage 0: grow the item array before any saved offsets are used.
    let base = host.clone().unwrap_or_else(|| src.clone());
    let mut m = MapFile::load(&base);
    m.append_item_clones(specs.len());
    m.write_to(&tmp0).expect("write item-slot stage");

    // Stage 1: park blocks using fixed-size patches only. Rename lookback tables
    // in a separate reload so the item and block regions cannot shift each
    // other's saved offsets.
    let mut m = MapFile::load(&tmp0);
    let old_uid = m
        .body_ids
        .first()
        .and_then(|f| f.name.clone())
        .expect("map uid");
    let new_uid = format!("Tin2{}", &old_uid[..23]);
    m.set_map_uid(&new_uid);
    // A parked start block would still be THE start (the car spawned in the
    // map corner), and parked checkpoints would still count: every waypoint
    // block becomes a plain road piece, and the race runs on the items.
    let neutral = source
        .blocks
        .iter()
        .map(|b| b.name.as_str())
        .find(|n| *n == "RoadTechStraight")
        .unwrap_or_else(|| source.blocks[0].name.as_str())
        .to_string();
    let mut neutralised = 0;
    for i in 0..m.blocks.len() {
        let b = m.blocks[i].clone();
        if b.flags & FREE_BLOCK_FLAG != 0 {
            m.move_block_free(i, [16.0, -1000.0, 16.0]);
        } else {
            m.move_block_cell(i, (0, 0, 0));
        }
        if b.waypoint_tag.is_some() || b.name.contains("Start") || b.name.contains("Finish") || b.name.contains("Checkpoint") || b.name.contains("Multilap") {
            m.set_block_name(i, &neutral);
            neutralised += 1;
        }
    }
    println!("  {neutralised} parked waypoint blocks renamed to {neutral}");
    m.write_to(&tmp1).expect("write parked-block stage");

    // Stage 2: append new model slots while preserving every original slot.
    let mut m = MapFile::load(&tmp1);
    for (i, s) in specs.iter().enumerate() {
        if host.is_some() || i >= original_items || mapping.items_by_index.contains_key(&i) {
            // Embedded items carry their ident as their author too (their
            // body ident has no room for a second string, see mapgeom
            // `set_body_ident_nameless`); the placement must say the same.
            m.set_item_model(i, &s.model);
            // Nadeo models kept as-is (vegetation, gates) keep author Nadeo.
            m.set_item_author(i, if s.model.starts_with("AC0") { &s.model } else { "Nadeo" });
        }
        m.move_item(i, s.pos, s.yaw, cell_for(s.pos));
        if let Some((rot, pivot)) = s.frame {
            m.set_item_frame(i, rot, pivot);
        }
        m.set_item_scale(i, s.scale);
        if s.model.starts_with("AC0") {
            m.clear_item_variant(i);
        }
    }
    m.write_to(&tmp2).expect("write model stage");

    // Stage 3: variable-length waypoint nodes.
    let mut m = MapFile::load(&tmp2);
    for (i, s) in specs.iter().enumerate() {
        m.set_item_waypoint_tag(i, s.tag.as_deref());
    }
    m.write_to(&tmp2).expect("write waypoint stage");

    // Stage 4: embed the converted block models. A non-empty source archive
    // would also need merging; replacing it would silently drop custom items.
    assert!(
        crate::header::embedded_zip(&source.gbx.body).is_none(),
        "source map already embeds custom objects; merge them into --library before converting"
    );
    let mut m = MapFile::load(&tmp2);
    m.remove_password();
    if library.as_os_str() != "-" {
        let zip = std::fs::read(&library).unwrap_or_else(|e| panic!("{}: {e}", library.display()));
        assert!(
            zip.starts_with(b"PK\x03\x04"),
            "{} is not a ZIP archive",
            library.display()
        );
        let mut embedded_names: Vec<String> = specs
            .iter()
            .enumerate()
            .filter(|(i, _)| *i >= original_items || mapping.items_by_index.contains_key(i))
            .filter(|(_, s)| s.model.starts_with("AC0"))
            .map(|(_, s)| s.model.clone())
            .collect();
        embedded_names.sort();
        embedded_names.dedup();
        let manifest: Vec<(&str, &str)> = embedded_names
            .iter()
            .map(|name| (name.as_str(), name.as_str()))
            .collect();
        m.replace_embedded_objects(&manifest, &zip);
    }
    m.write_to(&out).expect("write output");
    for p in [&tmp0, &tmp1, &tmp2] {
        let _ = std::fs::remove_file(p);
    }

    let check = MapFile::load(&out);
    assert!(
        !crate::map::skip_chunks(&check.gbx.body)
            .iter()
            .any(|(cid, ..)| *cid == 0x0304_3029),
        "generated map retained its editor password"
    );
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
        if let Some((rot, pivot)) = s.frame {
            assert!((got.pitch - rot[1]).abs() < 0.0001, "item#{i} pitch");
            assert!((got.roll - rot[2]).abs() < 0.0001, "item#{i} roll");
            assert!(
                (0..3).all(|k| (got.pivot[k] - pivot[k]).abs() < 0.0001),
                "item#{i} pivot"
            );
        }
    }
    println!("wrote {}", out.display());
    println!("  uid: {}", new_uid);
    println!("  {} existing items re-pointed at scaled copies", repointed_items);
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

/// Block-by-block verification map. For every distinct authored block model of
/// the source map, one ORIGINAL block is kept and moved onto a grid cell; its
/// generated item is placed beside it at scale 1 (+64 m in x) and again at the
/// requested scale (+112 m in x). Every other block is parked, every original
/// item is moved out of sight. A TSV of the grid (name, alias, cell, positions)
/// is written next to the map for camera planning and for reading the shots.
///
///   tmmaps tiny-catalog SRC.Map.Gbx --mapping placements.tsv --library ITEMS.zip
///       --out CATALOG.Map.Gbx [--scale 0.5] [--cols 7] [--host HOST.Map.Gbx]
pub fn catalog_cmd(args: &[String]) {
    let src = PathBuf::from(&args[2]);
    let out = PathBuf::from(cli::flag(args, "--out").expect("tiny-catalog needs --out MAP"));
    let mapping = read_mapping(&PathBuf::from(cli::flag(args, "--mapping").expect("--mapping FILE.tsv")));
    let library = PathBuf::from(cli::flag(args, "--library").expect("--library ITEMS.zip"));
    let scale: f32 = cli::flag(args, "--scale").unwrap_or("0.5").parse().expect("--scale");
    let cols: i32 = cli::flag(args, "--cols").unwrap_or("7").parse().expect("--cols");
    let host: Option<PathBuf> = cli::flag(args, "--host").map(PathBuf::from);
    let only: Option<String> = cli::flag(args, "--only").map(String::from);
    let source = MapFile::load(&src);
    set_ground(source.items.first().map(|it| it.collection_raw).unwrap_or(26));

    // one representative per block name, grid-placed blocks only
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut reps: Vec<usize> = Vec::new();
    for b in &source.blocks {
        if b.flags & FREE_BLOCK_FLAG != 0 {
            continue;
        }
        if !mapping.by_index.contains_key(&b.index) && !mapping.by_name.contains_key(&b.name) {
            continue;
        }
        if let Some(o) = &only {
            if !o.split(',').any(|n| n == b.name) {
                continue;
            }
        }
        if seen.insert(b.name.clone()) {
            reps.push(b.index);
        }
    }
    println!("  {} block models to verify", reps.len());

    // grid: 6 cells per column (192 m), 3 cells per row (96 m)
    let col_cells = 6;
    let row_cells = 3;
    let mut grid: Vec<(usize, (i32, i32, i32))> = Vec::new();
    for (k, &bi) in reps.iter().enumerate() {
        let col = k as i32 % cols;
        let row = k as i32 / cols;
        let (_, cy, _) = source.blocks[bi].coords();
        // start well inside the grid: the map edge is the decoration's scenery
        let origin: i32 = cli::flag(args, "--origin-cell").unwrap_or("20").parse().expect("--origin-cell");
        let raise: i32 = cli::flag(args, "--raise").unwrap_or("0").parse().expect("--raise cells");
        grid.push((bi, (origin + col * col_cells, cy + raise, origin + row * row_cells)));
    }

    let mut specs: Vec<Spec> = Vec::new();
    let mut tsv = String::from("name\talias\tcell_x\tcell_y\tcell_z\tblock_x\tblock_y\tblock_z\titem1_x\titem1_z\titem_scaled_x\titem_scaled_z\n");
    for &(bi, cell) in &grid {
        let b = &source.blocks[bi];
        let map = mapping.by_index.get(&bi).or_else(|| mapping.by_name.get(&b.name)).unwrap();
        // the block's origin once moved: recompute from the new cell
        let mut moved = b.clone();
        moved.raw_coords = [(cell.0 + 1) as u8, cell.1 as u8, (cell.2 + 1) as u8];
        let origin = match map.footprint {
            Some(fp) => block_origin(&moved, fp),
            None => block_pos(&moved),
        };
        let rot = [block_yaw(b), 0.0, 0.0];
        // --geometry-scaled: slot C uses the AS-prefixed library twin whose
        // mesh already carries the scale (the game ignores placement scale).
        let geom_scaled = args.iter().any(|a| a == "--geometry-scaled");
        if args.iter().any(|a| a == "--overlay") {
            // the scale-1 item exactly on the block: mismatches peek out.
            // --yaw-offset DEG turns the item relative to the block's yaw.
            let off: f32 = cli::flag(args, "--yaw-offset").unwrap_or("0").parse::<f32>().expect("--yaw-offset deg").to_radians();
            let rot = [rot[0] + off, rot[1], rot[2]];
            // --overlay-lift M raises the item a little so a coplanar match
            // reads as covered instead of z-fighting with the block.
            let lift: f32 = cli::flag(args, "--overlay-lift").unwrap_or("0").parse().expect("--overlay-lift m");
            let pos = [origin[0], origin[1] + lift, origin[2]];
            specs.push(Spec { model: map.model.clone(), pos, yaw: rot[0], frame: Some((rot, [0.0, 0.0, 0.0])), scale: 1.0 / map.model_scale, tag: None });
            let bp = block_pos(&moved);
            tsv.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\toverlay\n", b.name, map.model, cell.0, cell.1, cell.2, bp[0], bp[1], bp[2]));
            continue;
        }
        if args.iter().any(|a| a == "--yaw-sweep") {
            // four copies of the scale-1 item at yaw 0, 90, 180, 270 degrees
            for k in 0..4 {
                let pos = [origin[0] + 64.0 + 48.0 * k as f32, origin[1], origin[2]];
                let yaw = k as f32 * std::f32::consts::FRAC_PI_2;
                specs.push(Spec { model: map.model.clone(), pos, yaw, frame: Some(([yaw, 0.0, 0.0], [0.0, 0.0, 0.0])), scale: 1.0 / map.model_scale, tag: None });
            }
            let bp = block_pos(&moved);
            tsv.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\tsweep yaw 0/90/180/270 at x+64/+112/+160/+208\n", b.name, map.model, cell.0, cell.1, cell.2, bp[0], bp[1], bp[2]));
            continue;
        }
        for (dx, s) in [(64.0f32, 1.0f32), (112.0, scale)] {
            let pos = [origin[0] + dx, origin[1], origin[2]];
            let (model, s) = if geom_scaled && s != 1.0 {
                (map.model.replacen("AC", "AS", 1), 1.0)
            } else {
                (map.model.clone(), s / map.model_scale)
            };
            specs.push(Spec {
                model,
                pos,
                yaw: rot[0],
                frame: Some((rot, [0.0, 0.0, 0.0])),
                scale: s,
                tag: None,
            });
        }
        let bp = block_pos(&moved);
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\n",
            b.name, map.model, cell.0, cell.1, cell.2, bp[0], bp[1], bp[2], origin[0] + 64.0, origin[2], origin[0] + 112.0, origin[2]
        ));
    }

    let base = host.clone().unwrap_or_else(|| src.clone());
    let tmp0 = out.with_extension("cat0.Map.Gbx");
    let tmp1 = out.with_extension("cat1.Map.Gbx");
    let tmp2 = out.with_extension("cat2.Map.Gbx");
    let mut m = MapFile::load(&base);
    let n_existing = m.items.len();
    m.append_item_clones(n_existing + specs.len());
    m.write_to(&tmp0).expect("write slots");

    let mut m = MapFile::load(&tmp0);
    let old_uid = m.body_ids.first().and_then(|f| f.name.clone()).expect("map uid");
    m.set_map_uid(&format!("Cat1{}", &old_uid[..23]));
    if host.is_none() {
        let keep: BTreeSet<usize> = grid.iter().map(|g| g.0).collect();
        let neutral = "RoadTechStraight".to_string();
        for i in 0..m.blocks.len() {
            let b = m.blocks[i].clone();
            if let Some(&(_, cell)) = grid.iter().find(|g| g.0 == i) {
                m.move_block_cell(i, cell);
                continue;
            }
            if b.flags & FREE_BLOCK_FLAG != 0 {
                m.move_block_free(i, [16.0, -1000.0, 16.0]);
            } else {
                m.move_block_cell(i, (0, 0, 0));
            }
            if !keep.contains(&i) && (b.waypoint_tag.is_some() || b.name.contains("Start") || b.name.contains("Finish") || b.name.contains("Checkpoint")) {
                m.set_block_name(i, &neutral);
            }
        }
    }
    // existing items out of sight
    for i in 0..n_existing {
        m.move_item(i, [8.0, -900.0, 8.0], 0.0, (0, 0, 0));
    }
    m.write_to(&tmp1).expect("write parked");

    let mut m = MapFile::load(&tmp1);
    for (k, s) in specs.iter().enumerate() {
        let i = n_existing + k;
        m.set_item_model(i, &s.model);
        m.set_item_author(i, &s.model);
        m.move_item(i, s.pos, s.yaw, cell_for(s.pos));
        if let Some((rot, pivot)) = s.frame {
            m.set_item_frame(i, rot, pivot);
        }
        m.set_item_scale(i, s.scale);
        m.clear_item_variant(i);
    }
    m.write_to(&tmp2).expect("write models");

    let mut m = MapFile::load(&tmp2);
    m.remove_password();
    let zip = std::fs::read(&library).unwrap_or_else(|e| panic!("{}: {e}", library.display()));
    let mut names: Vec<String> = specs.iter().map(|s| s.model.clone()).collect();
    names.sort();
    names.dedup();
    let manifest: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), n.as_str())).collect();
    m.replace_embedded_objects(&manifest, &zip);
    m.write_to(&out).expect("write output");
    for p in [&tmp0, &tmp1, &tmp2] {
        let _ = std::fs::remove_file(p);
    }
    if host.is_none() {
        // no regenerated island under the grid
        let _ = MapFile::clear_genealogy_file(&out);
    }
    let tsv_path = out.with_extension("grid.tsv");
    std::fs::write(&tsv_path, tsv).unwrap();
    println!("wrote {} ({} blocks x [original, item x1, item x{}]); grid {}", out.display(), grid.len(), scale, tsv_path.display());
}
