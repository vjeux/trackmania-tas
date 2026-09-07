//! Build miniature maps by replacing authored blocks with Item.Gbx models and
//! scaling every authored item. Baked decoration/terrain is deliberately left
//! alone: it is the map's foundation, just as the U10S Tiny reference maps keep
//! their baked Grass floor at full size.

use crate::{
    census, cli,
    map::{BlockRec, MapFile, FREE_BLOCK_FLAG},
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
    /// Placement colour (chunk 0x03043062: 0 Default, 1 White, 2 Green, 3 Blue,
    /// 4 Red, 5 Black). Carried over from the source block or item: Default
    /// on a BlueBay item paints the colourisable parts (TrackBorders shoulders)
    /// green where the source blocks, all White, show white (2026-09-06).
    color: u8,
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
    /// `b@index`: a BAKED (generated) block -- the FC clip fillers -- that the
    /// tiny build re-emits as an item like an authored block.
    baked_by_index: BTreeMap<usize, Mapping>,
    by_name: BTreeMap<String, Mapping>,
    by_index: BTreeMap<usize, Mapping>,
    /// Original ITEM placements re-pointed at an embedded copy of their own
    /// model (`i@INDEX` rows). Items without a row keep their model.
    items_by_index: BTreeMap<usize, Mapping>,
    /// `v@ALIAS` rows: the vegetation a block's prefab carried, as stock
    /// items to place with every placement of that alias — (item, position
    /// in the item's scaled frame, yaw). A DecoLake shore carries hundreds of
    /// trees the static item cannot bake (VegetTreeModel: no mesh).
    veget_by_alias: BTreeMap<String, Vec<(String, [f32; 3], f32)>>,
    /// `y@INDEX` rows: metres an existing item placement is LOWERED by after the
    /// transform — a full-size stock tree standing in for a species the game
    /// cannot scale, sunk so its crown top sits where the original's would.
    sink_by_index: BTreeMap<usize, f32>,
}

/// `BLOCK<TAB>ITEM[<TAB>MODEL_SCALE]`, or `@INDEX<TAB>...` for an exact block
/// placement, or `i@INDEX<TAB>...` for an existing item placement. Index rows
/// win over block-name rows. MODEL_SCALE is the scale already baked into the
/// item geometry. `v@ALIAS<TAB>ITEM<TAB>X<TAB>Y<TAB>Z<TAB>YAW` adds a stock
/// vegetation item to every placement of ALIAS.
fn read_mapping(path: &Path) -> Mappings {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = Mappings::default();
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if let Some(index) = fields[0].strip_prefix("y@") {
            assert!(fields.len() == 2, "{}:{}: expected y@INDEX<TAB>DY", path.display(), line_no + 1);
            let idx: usize = index.parse().unwrap_or_else(|_| panic!("{}:{}: item index expected", path.display(), line_no + 1));
            let dy: f32 = fields[1].parse().unwrap_or_else(|_| panic!("{}:{}: number expected", path.display(), line_no + 1));
            out.sink_by_index.insert(idx, dy);
            continue;
        }
        if let Some(alias) = fields[0].strip_prefix("v@") {
            assert!(fields.len() == 6, "{}:{}: expected v@ALIAS<TAB>ITEM<TAB>X<TAB>Y<TAB>Z<TAB>YAW", path.display(), line_no + 1);
            let f = |i: usize| fields[i].parse::<f32>().unwrap_or_else(|_| panic!("{}:{}: number expected", path.display(), line_no + 1));
            out.veget_by_alias.entry(alias.to_string()).or_default().push((fields[1].to_string(), [f(2), f(3), f(4)], f(5)));
            continue;
        }
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
        } else if let Some(index) = fields[0].strip_prefix("b@") {
            out.baked_by_index
                .insert(index.parse().expect("b@INDEX number"), mapping)
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

/// World y the tiny transform keeps fixed by default, per collection: the
/// BlueBay sea surface (7: the land plane, block top y 10, halves to 8.5
/// above it — the verified Summer 01 mapping, spawn 16 -> 11.5); the Stadium
/// grass (10: the full-size floor the genealogy regenerates stays the floor).
/// A block that stands in for the terrain tile of its cell: the game draws
/// only the replacement. Two families: a block named after the zone it
/// replaces (`PlatformGrassOnLandHillSlopeBase`, `RoadTechStraightOnBeach` —
/// `On<zone>` with or without the zone's trailing digit, OnLandHill covers
/// LandHill1/2) and a GROUND-variant block (flags bit 12) whose family covers
/// its whole cell with a deck: platforms, roads, stands, deco platforms
/// (Summer 04's PlatformDirtCheckpoint shared its cell with a Grass tile whose
/// grass-blade shader poked tufts up through the dirt deck). Pillars,
/// DecoTerrainHD detail meshes and wall pillars do not cover the cell and keep
/// their tile.
pub fn stands_in_for_tile(name: &str, flags: u32, zones: &BTreeSet<String>) -> bool {
    if zones.contains(name) {
        return false;
    }
    let replaces = |zone: &str| {
        let stem = zone.trim_end_matches(|c: char| c.is_ascii_digit());
        name.contains(&format!("On{zone}")) || (!stem.is_empty() && name.contains(&format!("On{stem}")))
    };
    let covers_cell = flags & (1 << 12) != 0 && ["Platform", "Road", "OpenTechRoad", "DecoPlatform", "Stand"].iter().any(|p| name.starts_with(p));
    zones.iter().any(|z| replaces(z)) || covers_cell
}

/// The cells whose terrain tile is hidden by a stand-in block (see
/// `stands_in_for_tile`), keyed by the raw file cell.
pub fn replaced_cells(source: &MapFile, zones: &BTreeSet<String>) -> BTreeSet<[u8; 3]> {
    source.blocks.iter().filter(|b| stands_in_for_tile(&b.name, b.flags, zones)).map(|b| b.raw_coords).collect()
}

/// `tmmaps shared-cells MAP [--all]`: every cell where a terrain tile shares
/// its cell with another authored block, and whether the tile survives the
/// tiny transform. The tiles that survive beside a block are the coplanar
/// pairs the terrain drop was invented for (2026-09-06: Land plane and road
/// deck both at +2 z-fought as two items); once every deck hides its tile the
/// remaining pairs decide whether the drop is still needed at all. Without
/// `--all` only the surviving pairs are listed.
pub fn shared_cells_cmd(args: &[String]) {
    let source = MapFile::load(Path::new(&args[2]));
    let all = args.iter().any(|a| a == "--all");
    let zones: BTreeSet<String> = source.genealogy_zones().into_iter().collect();
    let replaced = replaced_cells(&source, &zones);
    let mut by_cell: BTreeMap<[u8; 3], (Vec<&BlockRec>, Vec<&BlockRec>)> = BTreeMap::new();
    for b in &source.blocks {
        let e = by_cell.entry(b.raw_coords).or_default();
        if zones.contains(&b.name) {
            e.0.push(b);
        } else {
            e.1.push(b);
        }
    }
    let mut pairs: BTreeMap<(String, String, &str), usize> = BTreeMap::new();
    let mut listed = 0usize;
    println!("cell\ttile\tstatus\tother blocks (flags)");
    for (cell, (tiles, others)) in &by_cell {
        if tiles.is_empty() || others.is_empty() {
            continue;
        }
        let status = if replaced.contains(cell) { "hidden" } else { "kept" };
        if !all && status == "hidden" {
            for t in tiles {
                for o in others {
                    *pairs.entry((t.name.clone(), o.name.clone(), status)).or_default() += 1;
                }
            }
            continue;
        }
        listed += 1;
        let (x, y, z) = tiles[0].coords();
        let tile_names: Vec<&str> = tiles.iter().map(|t| t.name.as_str()).collect();
        let other_names: Vec<String> = others.iter().map(|o| format!("{} ({:08X}{})", o.name, o.flags, if o.flags & (1 << 12) != 0 { " ground" } else { "" })).collect();
        println!("{x},{y},{z}\t{}\t{status}\t{}", tile_names.join("+"), other_names.join(", "));
        for t in tiles {
            for o in others {
                *pairs.entry((t.name.clone(), o.name.clone(), status)).or_default() += 1;
            }
        }
    }
    eprintln!("{listed} cells listed; tile/block pairs:");
    for ((t, o, status), n) in &pairs {
        eprintln!("  {n:4} × {t} + {o}  [{status}]");
    }
}

pub fn fixed_plane(collection: u32) -> f32 {
    match collection {
        0x1c => 7.0,
        0x10 => -0.5, // RedIsland lake surface (Water prefab local +7.5 at cell 14)
        // WhiteShore sea surface: `Zone\Water\Base.Prefab` water quad at local
        // +7 (its bottom at +2), the Water zone at cell 14 -> 14*8 - 120 + 7
        0x1d => -1.0,
        // GreenCoast lake surface: `Zone\Lake\Base.Prefab` water quad at local
        // +7.2 (bottom +2), the Lake zone at cell 4 -> 4*8 - 40 + 7.2
        0xf => -0.8,
        _ => 10.0,
    }
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

/// The stock vegetation items a block's prefab carried (`v@ALIAS` rows),
/// placed with the block: the row's position is in the item's scaled frame,
/// so it turns with the block's yaw the way `block_origin` turns footprints
/// (dir 1 = yaw -pi/2 maps local +x onto world +z, local +z onto world -x).
/// Returns how many were added.
fn push_veget(specs: &mut Vec<Spec>, mapping: &Mappings, alias: &str, origin: [f32; 3], yaw: f32, color: u8) -> usize {
    let Some(rows) = mapping.veget_by_alias.get(alias) else { return 0 };
    let (s, c) = yaw.sin_cos();
    for (item, local, tree_yaw) in rows {
        let pos = [origin[0] + local[0] * c + local[2] * s, origin[1] + local[1], origin[2] - local[0] * s + local[2] * c];
        let y = yaw + tree_yaw;
        specs.push(Spec { model: item.clone(), pos, yaw: y, frame: Some(([y, 0.0, 0.0], [0.0, 0.0, 0.0])), scale: 1.0, tag: None, color });
    }
    rows.len()
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

/// One MediaTracker trigger cell of the source (the trigger grid divides a
/// block cell into `ts` = 3×1×3 boxes of 32/3 × 8 × 32/3 m; row 0 at the
/// collection's ground) -> the trigger cells of the target its transformed
/// box covers. The box comes out half-size, so per axis it lies in one target
/// cell or straddles two; a straddled cell counts when the box covers at
/// least a fifth of it. Covering every cell merely TOUCHED (a sliver of a
/// metre) would double the volume's footprint — a 16 m tiny road would fire
/// its camera 8 m off each side, onto a neighbouring road — while the cell
/// centre alone can leave a one-row-tall trigger a row below the car; the
/// fifth keeps the car's band (deck +1..+3 m) inside on every collection's
/// row phase (Stadium boxes sit at row offsets 1..5 / 5..9, BlueBay's at
/// 3.5..7.5 / 7.5..11.5).
fn trigger_cells(c: [i32; 3], ts: [i32; 3], point: &dyn Fn([f32; 3]) -> [f32; 3]) -> Vec<[i32; 3]> {
    let unit = [32.0 / ts[0].max(1) as f32, 8.0 / ts[1].max(1) as f32, 32.0 / ts[2].max(1) as f32];
    let origin = [0.0, ground(), 0.0];
    let lo = [origin[0] + c[0] as f32 * unit[0], origin[1] + c[1] as f32 * unit[1], origin[2] + c[2] as f32 * unit[2]];
    let hi = [lo[0] + unit[0], lo[1] + unit[1], lo[2] + unit[2]];
    let a = point(lo);
    let b = point(hi);
    let cells_on = |k: usize| -> Vec<i32> {
        let (x0, x1) = (a[k].min(b[k]), a[k].max(b[k]));
        let first = ((x0 - origin[k]) / unit[k]).floor() as i32;
        let last = ((x1 - origin[k]) / unit[k]).floor() as i32;
        let mut out = Vec::new();
        for i in first.max(0)..=last.max(0) {
            let (c0, c1) = (origin[k] + i as f32 * unit[k], origin[k] + (i + 1) as f32 * unit[k]);
            let overlap = x1.min(c1) - x0.max(c0);
            if overlap >= 0.2 * unit[k] - 1e-4 {
                out.push(i);
            }
        }
        if out.is_empty() {
            // a degenerate box (scale ~0): the cell under its centre
            out.push((((x0 + x1) * 0.5 - origin[k]) / unit[k]).floor().max(0.0) as i32);
        }
        out
    };
    let (xs, ys, zs) = (cells_on(0), cells_on(1), cells_on(2));
    let mut out = Vec::with_capacity(xs.len() * ys.len() * zs.len());
    for x in &xs {
        for y in &ys {
            for z in &zs {
                out.push([*x, *y, *z]);
            }
        }
    }
    out
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
    // --mapgeom BIN --paks "--pak A:HASH --pak B:HASH": build EVERY map its own
    // item library first (`mapgeom tiny-library`, a subprocess: the mapping is
    // indexed by the map's own block indices, so one library cannot serve
    // two maps); the TINY_* recipe comes from the environment. Without it the
    // old form applies: one --mapping/--library shared by every map.
    let mapgeom = cli::flag(args, "--mapgeom").map(PathBuf::from);
    let paks: Vec<String> = cli::flag(args, "--paks").map(|p| p.split_whitespace().map(str::to_string).collect()).unwrap_or_default();
    // The CLI's panic hook exits the process on the first refusal; a batch
    // wants the refusal printed and the next map tried.
    std::panic::set_hook(Box::new(|info| {
        let msg = info.payload().downcast_ref::<String>().cloned().or_else(|| info.payload().downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "internal error".to_string());
        eprintln!("tmmaps: {msg}");
    }));
    let mut failed: Vec<String> = Vec::new();
    for source in maps {
        let name = source.file_name().unwrap().to_string_lossy().to_string();
        let stem = name.trim_end_matches(".Map.Gbx").to_string();
        let target = output.join(&name);
        let mut one = vec![
            "tmmaps".to_string(),
            "tiny".to_string(),
            source.display().to_string(),
            "--out".to_string(),
            target.display().to_string(),
        ];
        if let Some(bin) = &mapgeom {
            let lib = output.join(format!("{stem}.lib.zip"));
            let mapping = output.join(format!("{stem}.placements.tsv"));
            let report = output.join(format!("{stem}.report.tsv"));
            println!("== {name}: library");
            let status = std::process::Command::new(bin)
                .args(&paks)
                .arg("tiny-library")
                .arg(&source)
                .arg("--library-out").arg(&lib)
                .arg("--mapping-out").arg(&mapping)
                .arg("--report").arg(&report)
                .status()
                .unwrap_or_else(|e| panic!("{}: {e}", bin.display()));
            if !status.success() {
                eprintln!("== {name}: tiny-library failed ({status}); skipped");
                failed.push(name);
                continue;
            }
            one.extend(["--mapping".to_string(), mapping.display().to_string(), "--library".to_string(), lib.display().to_string()]);
        }
        for flag in ["--mapping", "--library", "--scale", "--anchor"] {
            if let Some(value) = cli::flag(args, flag) {
                one.push(flag.to_string());
                one.push(value.to_string());
            }
        }
        println!("== {name}: tiny");
        match std::panic::catch_unwind(|| cmd(&one)) {
            Ok(()) => {}
            Err(_) => {
                eprintln!("== {name}: tiny failed; skipped");
                failed.push(name);
            }
        }
    }
    if !failed.is_empty() {
        eprintln!("tiny-batch: {} map(s) failed: {}", failed.len(), failed.join(", "));
        std::process::exit(1);
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
    let anchor_flag = cli::flag(args, "--anchor").map(|a| vec3(a, "--anchor"));
    let mapping = read_mapping(&mapping_path);
    // --host HOST.Map.Gbx: build the copy INTO another map (e.g. an empty
    // Stadium map, where real Stadium materials are accepted) instead of into
    // the parked source. Placements still come from the source.
    let host: Option<PathBuf> = cli::flag(args, "--host").map(PathBuf::from);

    let source = MapFile::load(&src);
    let colors = source.colors().unwrap_or(crate::map::Colors { bytes: Vec::new(), n_blocks: 0, n_baked: 0 });
    set_ground(source.items.first().map(|it| it.collection_raw).unwrap_or(26));
    // The source anchor is the Spawn: a start block's cell corner, or (the
    // Stadium maps 15/20/25: GateStart items) the start item's position.
    let spawns = source.waypoints();
    let source_anchor = match spawns.iter().find(|w| w.kind == crate::map::Kind::Block && w.tag == "Spawn") {
        Some(w) => block_pos(&source.blocks[w.index]),
        None => {
            let w = spawns.iter().find(|w| w.kind == crate::map::Kind::Item && w.tag == "Spawn").expect("map needs a Spawn (block or item)");
            source.items[w.index].pos
        }
    };
    // Default target anchor: the start keeps its x,z and the sea surface
    // stays where it is, so y' = 7 + (y - 7) * scale. Summer 01: spawn 1584,16,784 -> 1584,11.5,784.
    let collection = source.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let plane = fixed_plane(collection);
    let target_anchor = anchor_flag.unwrap_or([source_anchor[0], plane + (source_anchor[1] - plane) * scale, source_anchor[2]]);
    println!("  anchor: spawn {:?} -> {:?} (scale {scale})", source_anchor, target_anchor);

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
    let mut prefab_trees = 0usize;
    // Trees of dropped vegetation-cluster items, appended AFTER the original
    // item slots (spec i is item slot i up to original_items).
    let mut cluster_trees: Vec<Spec> = Vec::new();
    let mut repointed_items = 0usize;
    let mut dropped_items = 0usize;
    let mut sunk_items = 0usize;
    for it in &source.items {
        match mapping.items_by_index.get(&it.index) {
            // "-": intentionally gone (procedural vegetation the tiny map
            // cannot shrink); the slot is parked far below the map. A
            // vegetation CLUSTER item leaves its trees behind as stock items
            // (`v@<model>` rows) at the placement's position and yaw.
            Some(map) if map.model == "-" => {
                dropped_items += 1;
                specs.push(Spec { model: it.model.clone(), pos: [8.0, -900.0, 8.0], yaw: 0.0, frame: None, scale: 1.0, tag: None, color: 0 });
                prefab_trees += push_veget(&mut cluster_trees, &mapping, &it.model, transform(it.pos, source_anchor, target_anchor, scale), it.yaw, colors.item(it.index));
            }
            // Re-pointed at an embedded copy whose geometry already carries
            // the scale: the placement stays where it is at scale 1. Its PIVOT
            // is in model metres and must shrink with the model: the game puts
            // the pivot point at `pos` and rotates about it, so a full-size
            // pivot on a half-size copy shifts the piece by half the pivot
            // (Summer 15's inflatable loop — 4 m mats with pivot (-4,0,-4)
            // rotated by pitch/roll — fell apart into a straight tube).
            Some(map) => {
                repointed_items += 1;
                // a stock half-size variant (RaceScreen6x1Small) is a scaled
                // copy too: its mapping row says model_scale 0.5
                let scaled_copy = (map.model_scale - 1.0).abs() > 1e-6;
                let frame = if scaled_copy && it.pivot.iter().any(|v| v.abs() > 1e-6) {
                    Some(([it.yaw, it.pitch, it.roll], [it.pivot[0] * map.model_scale, it.pivot[1] * map.model_scale, it.pivot[2] * map.model_scale]))
                } else {
                    None
                };
                // a vegetation stand-in is sunk (`y@` row): its crown top
                // where the original's would be at the tiny scale
                let mut pos = transform(it.pos, source_anchor, target_anchor, scale);
                if let Some(dy) = mapping.sink_by_index.get(&it.index) {
                    pos[1] -= dy;
                    sunk_items += 1;
                }
                specs.push(Spec {
                    model: map.model.clone(),
                    pos,
                    yaw: it.yaw,
                    frame,
                    scale: it.scale * scale / map.model_scale,
                    tag: it.waypoint_tag.clone(),
                    color: colors.item(it.index),
                });
            }
            None => specs.push(Spec {
                model: it.model.clone(),
                pos: transform(it.pos, source_anchor, target_anchor, scale),
                yaw: it.yaw,
                frame: None,
                scale: it.scale * scale,
                tag: it.waypoint_tag.clone(),
                color: colors.item(it.index),
            }),
        }
    }
    let original_items = specs.len();
    specs.extend(cluster_trees);
    // The flag cloth (Flag16m / Flag8m re-pointed at our vertex-tween copies)
    // animates only while a STOCK flag item is loaded in the map: without one
    // the tween frame index runs unbounded — the cloths are garbage or gone,
    // alive for one ~45 s window after the editor opens (Summer 15 lineups,
    // 2026-09-07: 6 and 24 copies alike; 11 copies next to one stock Flag16m
    // waved for the whole 100 s watched, hidden 50 m underground too). So one
    // stock flag of each kind the map converts rides along, parked under the
    // lowest row at the spawn. `TINY_FLAG_DRIVER=0` leaves it out.
    if std::env::var("TINY_FLAG_DRIVER").map(|v| v != "0").unwrap_or(true) {
        let converted: BTreeSet<&str> = source
            .items
            .iter()
            .filter(|it| matches!(it.model.as_str(), "Flag16m" | "Flag8m"))
            .filter(|it| mapping.items_by_index.get(&it.index).map(|m| m.model.ends_with(".Item.Gbx")).unwrap_or(false))
            .map(|it| it.model.as_str())
            .collect();
        for name in converted {
            let pos = [target_anchor[0], crate::map::ground_y(collection) + 4.0, target_anchor[2]];
            specs.push(Spec { model: name.to_string(), pos, yaw: 0.0, frame: None, scale: 1.0, tag: None, color: 0 });
            println!("  flag driver: one stock {name} parked at {:.0},{:.0},{:.0} (keeps the tween cloths animating)", pos[0], pos[1], pos[2]);
        }
    }
    let mut empty_blocks = 0usize;
    // Zone (terrain) blocks REPLACED by a block designed for that terrain: a
    // `PlatformGrassOnLandHillSlopeBase` / `PlatformGrassBaseOnLandHill2` /
    // `RoadTechStraightOnWaterShore1` shares its cell with the LandHill /
    // WaterShore tile it stands in for, and the game draws only the
    // replacement. Both as items (Summer 03, cells (37,17,22) and (38,17,22):
    // LandHill1 Deadend under the OnLandHill slope base) the hill's bumps poke
    // through the deck — a snow blob in the grass. Zone names come from the
    // genealogy chunk; a replacement block names its zone after "On", with or
    // without the zone's trailing digit (OnLandHill covers LandHill1/2).
    let zones: BTreeSet<String> = source.genealogy_zones().into_iter().collect();
    let replaced_cells = replaced_cells(&source, &zones);
    let mut replaced_terrain = 0usize;
    // Authored blocks occupy appended clones.
    for b in &source.blocks {
        let map = mapping
            .by_index
            .get(&b.index)
            .or_else(|| mapping.by_name.get(&b.name))
            .expect("mapping checked above");
        // "-": the picked variant has no geometry (an intentionally empty
        // pillar mobil): no item, on purpose.
        if map.model == "-" {
            empty_blocks += 1;
            continue;
        }
        if zones.contains(&b.name) && replaced_cells.contains(&b.raw_coords) {
            replaced_terrain += 1;
            continue;
        }
        let rot = b.free_rot.unwrap_or([block_yaw(b), 0.0, 0.0]);
        let origin = match map.footprint {
            Some(fp) => block_origin(b, fp),
            None => block_pos(b),
        };
        let pos = transform(origin, source_anchor, target_anchor, scale);
        specs.push(Spec {
            model: map.model.clone(),
            pos,
            yaw: rot[0],
            frame: Some((rot, [0.0, 0.0, 0.0])),
            scale: scale / map.model_scale,
            tag: b.waypoint_tag.clone(),
            color: colors.block(b.index),
        });
        prefab_trees += push_veget(&mut specs, &mapping, &map.model, pos, rot[0], colors.block(b.index));
    }
    // Baked (generated) non-Sea blocks -- the FC clip fillers that finish the
    // authored structures (pillar feet, screen caps, wall faces) -- become
    // items too; the baked chunk itself is rewritten to all-Sea below.
    let mut baked_items = 0usize;
    for b in &source.baked {
        let Some(map) = mapping.baked_by_index.get(&b.index) else { continue };
        if map.model == "-" {
            continue;
        }
        // A generated filler of a FREE block is free too, with the parent's
        // full rotation (Summer 11's inverted ramp: 57 TrackWallSlopeStraightFCB
        // bottom plates pitched by 178° under upside-down roads; yaw alone
        // stood them up as thin bars and the ramp vanished).
        let rot = b.free_rot.unwrap_or([block_yaw(b), 0.0, 0.0]);
        let origin = match map.footprint {
            Some(fp) => block_origin(b, fp),
            None => block_pos(b),
        };
        baked_items += 1;
        let pos = transform(origin, source_anchor, target_anchor, scale);
        let color = std::env::var("TINY_BAKED_COLOR").ok().and_then(|s| s.parse().ok()).unwrap_or(colors.baked(b.index));
        specs.push(Spec {
            model: map.model.clone(),
            pos,
            yaw: rot[0],
            frame: Some((rot, [0.0, 0.0, 0.0])),
            color,
            scale: scale / map.model_scale,
            tag: None,
        });
        prefab_trees += push_veget(&mut specs, &mapping, &map.model, pos, rot[0], color);
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

    // Stage 1: the source's blocks go. DELETED (default, 2026-09-07): the
    // authored records and the generated non-foundation fillers are cut out of
    // the block chunks (`remove_blocks` rewrites everything that lists blocks
    // — counts, free positions, colours, lightmap quality, macroblock refs,
    // the items' snapped-on tables), the way the reference tiny maps have
    // ZERO authored blocks. `TINY_PARK_BLOCKS=1` keeps the old way for A/B:
    // every record stays and is MOVED to cell (0,0,0) by fixed-size patches
    // (free blocks to y -1000) and renamed to a neutral road — ~5 700 dead
    // records per map stacked in the corner, and a suspect for Summer 05's
    // 5-7 minute load. The lookback-table renames of the parking path need
    // their own reload so the item and block regions cannot shift each
    // other's saved offsets; the deletion is variable-length and needs the
    // same.
    let park = std::env::var_os("TINY_PARK_BLOCKS").is_some();
    let mut m = MapFile::load(&tmp0);
    let old_uid = m
        .body_ids
        .first()
        .and_then(|f| f.name.clone())
        .expect("map uid");
    let new_uid = format!("Tin2{}", &old_uid[..23]);
    if !park {
        // The foundation records stay: `Sea` (BlueBay's water, the only baked
        // name the parking path left in place). TINY_KEEP_BAKED=Sea,Grass
        // widens it (a Stadium map's baked Grass tiles in place instead of
        // regenerated from the kept genealogy).
        let keep_baked: BTreeSet<String> = std::env::var("TINY_KEEP_BAKED")
            .unwrap_or_else(|_| "Sea".to_string())
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        let n_blocks = m.blocks.len();
        let n_baked = m.baked.len();
        let r = m.remove_blocks(|_| true, |b| !keep_baked.contains(&b.name));
        println!(
            "  deleted {} of {} authored blocks and {} of {} generated (baked) blocks (kept: {}); {} free-block entries, {} snapped-on groups ({} items un-snapped); lookback table {} -> {} strings",
            r.blocks, n_blocks, r.baked, n_baked, keep_baked.iter().cloned().collect::<Vec<_>>().join(","), r.free_entries, r.snap_groups, r.snapped_items_cleared, r.table_before, r.table_after
        );
        m.write_to(&tmp1).expect("write block-deletion stage");
        m = MapFile::load(&tmp1);
        assert!(m.blocks.is_empty(), "{} authored blocks survived the deletion", m.blocks.len());
        assert!(m.baked.iter().all(|b| keep_baked.contains(&b.name)), "a generated block outside the keep set survived the deletion");
    }
    m.set_map_uid(&new_uid);
    if park {
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
            if std::env::var_os("TINY_NO_PARK_BLOCKS").is_some() && b.waypoint_tag.is_none() {
                continue; // debug knob: only the waypoint blocks are parked
            }
            if b.flags & FREE_BLOCK_FLAG != 0 {
                m.move_block_free(i, [16.0, -1000.0, 16.0]);
            } else {
                m.move_block_cell(i, (0, 0, 0));
            }
            // EVERY parked block is renamed to the neutral road, not just the
            // waypoints: with the genealogy chunk cleared, a parked terrain block
            // (Summer 06: Land0_Land1_Land2 / DecoTreeBeach…) at cell 0,0,0 makes
            // the client dereference a missing zone (0x140d2b0bc, 2026-09-06);
            // the same map loads with the blocks left in place or the genealogy
            // kept. A road needs no zone.
            if b.name != neutral {
                m.set_block_name(i, &neutral);
                neutralised += 1;
            }
        }
        println!("  {neutralised} parked blocks renamed to {neutral}");
        // the generated non-Sea fillers are parked too (re-emitted as items above);
        // the Sea records stay: they are the water
        let mut parked_baked = 0usize;
        for i in 0..m.baked.len() {
            if std::env::var_os("TINY_NO_PARK_BAKED").is_some() {
                break; // debug knob: leave the generated blocks where they are
            }
            if m.baked[i].name != "Sea" {
                m.move_baked_cell(i, (0, 0, 0));
                parked_baked += 1;
            }
        }
        println!("  {parked_baked} generated (baked) non-Sea blocks parked");
    }
    m.write_to(&tmp1).expect("write block stage");

    // Stage 2: append new model slots while preserving every original slot.
    let mut m = MapFile::load(&tmp1);
    for (i, s) in specs.iter().enumerate() {
        if host.is_some() || i >= original_items || mapping.items_by_index.contains_key(&i) {
            // Embedded items carry their ident as their author too (their
            // body ident has no room for a second string, see mapgeom
            // `set_body_ident_nameless`); the placement must say the same.
            m.set_item_model(i, &s.model);
            // Nadeo models kept as-is (vegetation, gates) keep author Nadeo.
            // an embedded library item (any *.Item.Gbx) is its own author; a
            // stock model (vegetation substitute) is Nadeo's
            m.set_item_author(i, if s.model.ends_with(".Item.Gbx") { &s.model } else { "Nadeo" });
        }
        m.move_item(i, s.pos, s.yaw, cell_for(s.pos));
        if let Some((rot, pivot)) = s.frame {
            m.set_item_frame(i, rot, pivot);
        }
        m.set_item_scale(i, s.scale);
        if s.model.ends_with(".Item.Gbx") {
            m.clear_item_variant(i);
        }
        m.set_item_color(i, s.color);
    }
    m.write_to(&tmp2).expect("write model stage");

    // Stage 3: variable-length waypoint nodes.
    let mut m = MapFile::load(&tmp2);
    for (i, s) in specs.iter().enumerate() {
        m.set_item_waypoint_tag(i, s.tag.as_deref());
    }
    m.write_to(&tmp2).expect("write waypoint stage");

    // Stage 4: embed the converted block models. The source's own archive
    // (custom items: the TME nation items) is replaced, which is only right
    // when no placement still points at one of its files — the library
    // builder bakes them into half-scale copies; anything left over would be
    // silently dropped, so it is a refusal.
    if let Some((_, names)) = crate::header::embedded_zip(&source.gbx.body) {
        let files: Vec<String> = names.iter().map(|n| n.replace('/', "\\").to_ascii_lowercase()).collect();
        let still: Vec<String> = specs
            .iter()
            .map(|s| s.model.clone())
            .filter(|m| files.iter().any(|f| f == &format!("items\\{}", m.to_ascii_lowercase())))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert!(
            still.is_empty(),
            "source map embeds custom objects still placed after the mapping ({}): merge them into --library before converting",
            still.join(", ")
        );
        println!("  source archive ({} files) replaced: every custom item is re-pointed at a scaled copy", names.len());
    }
    let mut m = MapFile::load(&tmp2);
    m.remove_password();
    // The map's own NAME: "Summer 2026 - 15" becomes "Tiny Summer 2026 - 15",
    // so the editor title bar, the map list and the playground HUD say which
    // one you are looking at (the Nadeo record has said "Tiny …" since map 01;
    // the file kept the original's name, so every screenshot of a tiny map was
    // labelled like the original — vjeux, 2026-09-07). `TINY_MAP_NAME=<name>`
    // sets it outright, `=keep` leaves the source name.
    let name_mode = std::env::var("TINY_MAP_NAME").unwrap_or_default();
    if name_mode != "keep" {
        let old = crate::header::read(&src.display().to_string()).ok().map(|h| h.name).unwrap_or_default();
        if old.is_empty() || old == "-" {
            println!("  map name: the source declares none; left alone");
        } else {
            let new = if name_mode.is_empty() { format!("Tiny {old}") } else { name_mode.clone() };
            let (h, b) = m.set_map_name(&old, &new);
            println!("  map name: {old:?} -> {new:?} ({h} in the header, {b} in the body)");
        }
    }
    // The source's stored lightmap goes (TINY_LIGHTMAP=keep keeps it). It was
    // computed for the full-size layout and the game applies it BY OBJECT
    // INDEX: in PLAY mode a parked build of Summer 15 (2026-09-07) drew every
    // converted-block item BLACK — the appended items fall outside the
    // lightmap's tables — while the original items (screens, palms, flags)
    // were lit; the same build with the lightmap stripped is lit throughout,
    // pixel-identical to the deleted builds. Every map published before this
    // (parked, lightmap kept) has that defect in play. In the editor the
    // stale map showed "VALIDATED" and the original's shading on 05's deck.
    //
    // BUT with the blocks DELETED (the default since 4081dde) the lightmap
    // STAYS: the game rejects a stale lightmap whose block count is 0 on its
    // own (delete-blocks thread: deleted builds render the same with or
    // without it, in play AND in the editor — verified on 09's start deck and
    // cp5 tunnel, 2026-09-07 14:11), while a 0-block map WITHOUT a lightmap
    // makes the EDITOR crash with a STACK_OVERFLOW during or right after the
    // load (3 of 4 opens of the tiny 09; Trackmania.exe+0x96d7e0 recursing
    // under an Openplanet frame — the editor's automatic lightmap pass over a
    // map with no blocks, presumably), which kills every shootset comparison.
    // The parked build (blocks kept) still needs the strip. TINY_LIGHTMAP=
    // keep|strip forces either way.
    let strip = match std::env::var("TINY_LIGHTMAP").as_deref() {
        Ok("keep") => false,
        Ok("strip") => true,
        _ => park,
    };
    if strip {
        let n = m.strip_lightmap();
        println!("  stored lightmap stripped ({n} bytes)");
    } else {
        println!("  stored lightmap kept (0 blocks: the game rejects it; without one the editor crashes)");
    }
    // The MediaTracker (chunk 0x03043049, `mediatracker.rs`): the intro, the
    // in-game and the end-race clips fly cameras over FULL-SIZE coordinates
    // and fire from full-size trigger cells; both go through the items'
    // transform (times, angles and fields of view stay: the clip lasts the
    // same seconds over a half-size map). Blocks whose layout the reader does
    // not know are copied verbatim and listed. TINY_MEDIATRACKER=keep leaves
    // the chunk alone, =strip drops every clip (A/B).
    let mt_mode = std::env::var("TINY_MEDIATRACKER").unwrap_or_default();
    if mt_mode != "keep" {
        match m.mediatracker() {
            None => println!("  MediaTracker: no chunk 0x03043049 in this map"),
            Some(Err(e)) => eprintln!("  WARNING: MediaTracker left untouched, its cameras fly over the full-size layout: {e}"),
            Some(Ok(mut mt)) => {
                if mt_mode == "strip" {
                    mt.strip = true;
                    println!("  MediaTracker: every clip dropped (TINY_MEDIATRACKER=strip)");
                } else {
                    // The trigger grid is doubled first (3x1x3 -> 6x2x6 cells per
                    // block, every source cell re-expressed as its 2x2x2 finer
                    // cells, exactly): a half-size volume then lands on cells of
                    // its own size instead of the coarse ones, and the tiny
                    // trigger is as tight as the original's (Summer 15's spawn-
                    // ahead test trigger: 10.7 m deep instead of 21.3 m). The game
                    // honours the chunk's trigger size — measured 2026-09-07: the
                    // same clip fired at the same car position with the 3x1x3 and
                    // the 6x2x6 encoding (camera jump 12.96 s / 13.01 s into the
                    // logs, entry 12.97 / 13.03). TINY_TRIGGER_SIZE=keep leaves the
                    // source grid.
                    if std::env::var("TINY_TRIGGER_SIZE").map(|v| v != "keep").unwrap_or(true) {
                        if let Some(t0) = mt.trigger_size {
                            if let Err(e) = mt.set_trigger_size([t0[0] * 2, t0[1] * 2, t0[2] * 2]) {
                                eprintln!("  WARNING: trigger grid kept at {t0:?}: {e}");
                            }
                        }
                    }
                    let ts = mt.trigger_size.unwrap_or([3, 1, 3]);
                    let point = |p: [f32; 3]| transform(p, source_anchor, target_anchor, scale);
                    let cell = |c: [i32; 3]| trigger_cells(c, ts, &point);
                    let (keys, verts, (c0, c1), left) = mt.transform(&point, scale, &cell);
                    let opaque = mt.opaque_blocks();
                    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
                    for (class, _) in &opaque {
                        *kinds.entry(crate::mediatracker::class_name(*class)).or_default() += 1;
                    }
                    println!(
                        "  MediaTracker: {} clips; {keys} camera keys and {verts} triangle vertices moved through the transform, trigger cells {c0} -> {c1} (grid {}x{}x{} per block), {left} blocks left alone ({} kept verbatim: {})",
                        mt.clips().len(),
                        ts[0], ts[1], ts[2],
                        opaque.len(),
                        kinds.iter().map(|(k, v)| format!("{k}×{v}")).collect::<Vec<_>>().join(" ")
                    );
                }
                m.set_mediatracker(&mt);
            }
        }
    } else {
        println!("  MediaTracker kept untouched (TINY_MEDIATRACKER=keep)");
    }
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
            .filter(|(_, s)| s.model.ends_with(".Item.Gbx"))
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
    // Genealogies (chunk 0x03043043) are the per-cell terrain zones the game
    // regenerates Land/Beach/Hill/Cliff blocks from at load: with the authored
    // terrain parked they rebuilt the full-size island under the tiny one
    // (2026-09-06). Cleared, the floor cells without a block are plain sea.
    // (Rewriting the baked chunk to all-Sea -- `all_sea_file` -- is NOT
    // needed and makes the game refuse the map: "Couldn't load map!".)
    // Stadium keeps it: its zones are the grass floor, full size under the
    // tiny map like the reference maps (and there is no sea to fall into).
    if std::env::var_os("TINY_KEEP_GENEALOGY").is_none() {
        match collection {
            // BlueBay: the sea around the island is decoration, so no zone
            // at all leaves plain sea under the tiny map.
            0x1c => {
                let zones = MapFile::clear_genealogy_file(&out).expect("clear genealogies");
                println!("  genealogy chunk cleared: {zones} terrain zone records dropped");
            }
            // RedIsland: the ambient terrain is a zone BLOCK (Water, 2568 of
            // the 4096 cells of Summer 02); every cell gets it, so the game
            // regenerates the lake full size around and under the tiny
            // island, whose water items sit on the same surface (-0.5).
            // WhiteShore likewise: Water is a zone block (3148 of the 4096
            // cells of Summer 03), the sea the island sits in, surface -1.
            // GreenCoast: Lake (2418 of 4096 cells of Summer 04), the same way.
            0x10 | 0x1d | 0xf => {
                let (zone, n) = MapFile::fill_genealogy_file(&out).expect("fill genealogies");
                println!("  genealogy chunk filled: {n} cells of {zone}");
            }
            _ => {}
        }
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
    println!("  {} existing items re-pointed at scaled copies ({} vegetation stand-ins sunk to half-tree crown height); {} dropped (procedural vegetation); {} blocks intentionally without an item (empty variants); {} terrain tiles replaced by the block standing in for them; {} prefab trees placed as stock items", repointed_items, sunk_items, dropped_items, empty_blocks, replaced_terrain, prefab_trees);
    println!(
        "  scaled every authored object: {} blocks + {} items = {} item placements",
        source.blocks.len(),
        source.items.len(),
        specs.len()
    );
    println!(
        "  baked foundation: {} generated blocks in the source ({} re-emitted as items)",
        source.baked.len(),
        baked_items
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
    let mut ref_authors: BTreeMap<String, String> = BTreeMap::new();
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
        // --ref-item F[,G...]: foreign item files, under their OWN idents
        // and authors, placed at +160 m, +208 m, ... Falls through to the
        // normal slots below (side-by-side needs both).
        if let Some(refitems) = cli::flag(args, "--ref-item") {
            for (ri, refitem) in refitems.split(',').enumerate() {
                let bytes = std::fs::read(refitem).unwrap_or_else(|e| panic!("--ref-item {refitem}: {e}"));
                let (ident, author) = crate::header::item_ident_author(&bytes).expect("item header ident");
                let pos = [origin[0] + 160.0 + 48.0 * ri as f32, origin[1], origin[2]];
                specs.push(Spec { model: ident.clone(), pos, yaw: rot[0], frame: Some((rot, [0.0, 0.0, 0.0])), scale: 1.0, tag: None, color: 1 });
                ref_authors.insert(ident, author);
            }
        }
        // --lineup F[,G...]: the block alone, then each listed item file
        // under its OWN ident and author at +64 m, +112 m, ... (48 m pitch),
        // no library slots: "original block, our tiny, his tiny" in one frame.
        if let Some(files) = cli::flag(args, "--lineup") {
            for (ri, file) in files.split(',').enumerate() {
                let bytes = std::fs::read(file).unwrap_or_else(|e| panic!("--lineup {file}: {e}"));
                let (ident, author) = crate::header::item_ident_author(&bytes).expect("item header ident");
                let pos = [origin[0] + 64.0 + 48.0 * ri as f32, origin[1], origin[2]];
                specs.push(Spec { model: ident.clone(), pos, yaw: rot[0], frame: Some((rot, [0.0, 0.0, 0.0])), scale: 1.0, tag: None, color: 1 });
                ref_authors.insert(ident, author);
            }
            let bp = block_pos(&moved);
            tsv.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\tlineup {} at x+64/+112/...\n", b.name, map.model, cell.0, cell.1, cell.2, bp[0], bp[1], bp[2], files));
            continue;
        }
        if args.iter().any(|a| a == "--overlay") {
            // the scale-1 item exactly on the block: mismatches peek out.
            // --yaw-offset DEG turns the item relative to the block's yaw.
            let off: f32 = cli::flag(args, "--yaw-offset").unwrap_or("0").parse::<f32>().expect("--yaw-offset deg").to_radians();
            let rot = [rot[0] + off, rot[1], rot[2]];
            // --overlay-lift M raises the item a little so a coplanar match
            // reads as covered instead of z-fighting with the block.
            let lift: f32 = cli::flag(args, "--overlay-lift").unwrap_or("0").parse().expect("--overlay-lift m");
            let pos = [origin[0], origin[1] + lift, origin[2]];
            specs.push(Spec { model: map.model.clone(), pos, yaw: rot[0], frame: Some((rot, [0.0, 0.0, 0.0])), scale: 1.0 / map.model_scale, tag: None, color: 1 });
            let bp = block_pos(&moved);
            tsv.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\toverlay\n", b.name, map.model, cell.0, cell.1, cell.2, bp[0], bp[1], bp[2]));
            continue;
        }
        if args.iter().any(|a| a == "--yaw-sweep") {
            // four copies of the scale-1 item at yaw 0, 90, 180, 270 degrees
            for k in 0..4 {
                let pos = [origin[0] + 64.0 + 48.0 * k as f32, origin[1], origin[2]];
                let yaw = k as f32 * std::f32::consts::FRAC_PI_2;
                specs.push(Spec { model: map.model.clone(), pos, yaw, frame: Some(([yaw, 0.0, 0.0], [0.0, 0.0, 0.0])), scale: 1.0 / map.model_scale, tag: None, color: 1 });
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
                color: 1,
            });
        }
        let bp = block_pos(&moved);
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\n",
            b.name, map.model, cell.0, cell.1, cell.2, bp[0], bp[1], bp[2], origin[0] + 64.0, origin[2], origin[0] + 112.0, origin[2]
        ));
    }

    let base = host.clone().unwrap_or_else(|| src.clone());
    // --stock A,B,C: stock (pack) items by name — vegetation species — in a
    // row 40 m in front of the first block, 16 m apart, standing on the
    // block's deck level, under author Nadeo: a size-and-colour survey of a
    // collection's trees in one frame (GreenCoast has 45 species).
    if let Some(list) = cli::flag(args, "--stock") {
        let (bx, by, bz) = grid.first().map(|g| g.1).unwrap_or((20, 5, 20));
        let base_pos = [bx as f32 * crate::map::CELL_XZ, by as f32 * crate::map::CELL_Y + ground() + 2.0, bz as f32 * crate::map::CELL_XZ - 40.0];
        for (k, name) in list.split(',').filter(|s| !s.is_empty()).enumerate() {
            let pos = [base_pos[0] + 16.0 * k as f32, base_pos[1], base_pos[2]];
            specs.push(Spec { model: name.to_string(), pos, yaw: 0.0, frame: Some(([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])), scale: 1.0, tag: None, color: 0 });
            ref_authors.insert(name.to_string(), "Nadeo".to_string());
            tsv.push_str(&format!("stock\t{name}\t\t\t\t{:.0}\t{:.0}\t{:.0}\n", pos[0], pos[1], pos[2]));
        }
    }
    let tmp0 = out.with_extension("cat0.Map.Gbx");
    let tmp1 = out.with_extension("cat1.Map.Gbx");
    let tmp2 = out.with_extension("cat2.Map.Gbx");
    let mut m = MapFile::load(&base);
    let n_existing = m.items.len();
    m.append_item_clones(n_existing + specs.len());
    m.write_to(&tmp0).expect("write slots");

    let mut m = MapFile::load(&tmp0);
    // Fresh UID per build: the game caches embedded items and lightmaps by
    // map UID, so reusing the host's UID shows stale items (empty grass
    // where new items should be -- graft tests 2026-09-05).
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    m.set_map_uid(&format!("Cat1{:08X}{:07}{:08X}", nanos % 100_000_000, std::process::id() % 10_000_000, (nanos / 7) % 100_000_000));
    if host.is_none() {
        let keep: BTreeSet<usize> = grid.iter().map(|g| g.0).collect();
        let neutral = "RoadTechStraight".to_string();
        // Zone (terrain) blocks stay where they are: 2936 Lake/LakeShore
        // tiles stacked in one cell left the GreenCoast editor view solid
        // white (Summer 04, 2026-09-06); the island is scenery for the survey.
        let zones: BTreeSet<String> = source.genealogy_zones().into_iter().collect();
        for i in 0..m.blocks.len() {
            let b = m.blocks[i].clone();
            if let Some(&(_, cell)) = grid.iter().find(|g| g.0 == i) {
                m.move_block_cell(i, cell);
                continue;
            }
            if zones.contains(&b.name) {
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
        m.set_item_author(i, ref_authors.get(&s.model).map(|a| a.as_str()).unwrap_or(&s.model));
        m.move_item(i, s.pos, s.yaw, cell_for(s.pos));
        if let Some((rot, pivot)) = s.frame {
            m.set_item_frame(i, rot, pivot);
        }
        m.set_item_scale(i, s.scale);
        m.clear_item_variant(i);
        m.set_item_color(i, s.color);
        if let Ok(f) = std::env::var("TINY_ITEM_FLAGS") {
            m.set_item_flags(i, u16::from_str_radix(f.trim_start_matches("0x"), 16).expect("TINY_ITEM_FLAGS hex"));
        }
    }
    m.write_to(&tmp2).expect("write models");

    let mut m = MapFile::load(&tmp2);
    m.remove_password();
    let mut zip = std::fs::read(&library).unwrap_or_else(|e| panic!("{}: {e}", library.display()));
    // Foreign items claim the MAP's collection inside (header + body idents):
    // a BlueBay map drops a Stadium-collection item silently (Lineup7,
    // 2026-09-06: both tiny items absent, no dialog, probe listed none).
    let map_collection = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
    for flag in ["--ref-item", "--lineup"] {
        if let Some(refitems) = cli::flag(args, flag) {
            for refitem in refitems.split(',') {
                let bytes = std::fs::read(refitem).unwrap();
                let (ident, _) = crate::header::item_ident_author(&bytes).unwrap();
                let bytes = crate::header::set_ident_collection(&bytes, map_collection);
                zip = crate::header::zip_add(&zip, &format!("Items/{ident}"), &bytes); // zip_add re-emits deflated
            }
        }
    }
    let mut names: Vec<String> = specs.iter().map(|s| s.model.clone()).filter(|n| n.ends_with(".Item.Gbx")).collect();
    names.sort();
    names.dedup();
    let manifest: Vec<(&str, &str)> = names
        .iter()
        .map(|n| (n.as_str(), ref_authors.get(n).map(|a| a.as_str()).unwrap_or(n.as_str())))
        .collect();
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

/// `tmmaps lineup MAP --out F --stock A,B,C --at X,Y,Z [--pitch M]
/// [--items F.Item.Gbx,G.Item.Gbx]`: the map unchanged plus a row of STOCK
/// (pack) items by name — vegetation species — starting at X,Y,Z, `pitch`
/// metres apart along +x, under author Nadeo. A species survey in one frame
/// (which SpringTree is green, which is pink), on the real map so the editor
/// renders it (a parked-block catalog map came out blank in GreenCoast).
/// `--items` continues the row with EMBEDDED item files under their own ident
/// and author (re-stamped to the map's collection): a stock `Lamp` next to
/// our baked lamp on a night map is the oracle for the lights work.
pub fn lineup_cmd(args: &[String]) {
    let src = PathBuf::from(&args[2]);
    let out = PathBuf::from(cli::flag(args, "--out").expect("lineup needs --out MAP"));
    let list = cli::flag(args, "--stock").unwrap_or("");
    let at = vec3(&cli::flag(args, "--at").expect("lineup needs --at X,Y,Z"), "--at");
    let pitch: f32 = cli::flag(args, "--pitch").unwrap_or("16").parse().expect("--pitch metres");
    let step: [f32; 3] = cli::flag(args, "--step").map(|s| vec3(s, "--step")).unwrap_or([pitch, 0.0, 0.0]);
    // --yaw R turns every item of the row (radians): a pusher's piston runs
    // along its local z, so pi/2 makes it run along the row, visible from the north
    let yaw: f32 = cli::flag(args, "--yaw").unwrap_or("0").parse().expect("--yaw radians");
    // a stock name may carry a variant: `ShowLights@23` = the Light4Spots entry
    let mut variants: Vec<u8> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for s in list.split(',').filter(|s| !s.is_empty()) {
        let (name, v) = match s.split_once('@') {
            Some((n, v)) => (n.to_string(), v.parse::<u8>().unwrap_or_else(|_| panic!("--stock {s}: variant is not a byte"))),
            None => (s.to_string(), 0),
        };
        names.push(name);
        variants.push(v);
    }
    // --place "x,y,z,yaw;x,y,z,yaw;…": one pose per item instead of the row
    // (the play-mode collision test: pushers on four sides of the spawn)
    let places: Vec<[f32; 4]> = cli::flag(args, "--place")
        .map(|s| {
            s.split(';')
                .filter(|p| !p.is_empty())
                .map(|p| {
                    let v: Vec<f32> = p.split(',').map(|x| x.trim().parse().expect("--place x,y,z,yaw")).collect();
                    assert_eq!(v.len(), 4, "--place wants x,y,z,yaw per item");
                    [v[0], v[1], v[2], v[3]]
                })
                .collect()
        })
        .unwrap_or_default();
    // --colors 2,3,5,…: the placement colour byte of each item in row order
    // (0 Default 1 White 2 Green 3 Blue 4 Red 5 Black); an item past the list
    // keeps the default (stock 0, embedded 1). A stock flag at Green next to
    // ours at Green is the hue-mask oracle.
    // --scales 1,1.001,…: one placement scale per item of the row (1 past the
    // list) — does a scale of its own keep an item out of the game's
    // instanced draw of identical placements? (2026-09-07)
    let scales: Vec<f32> = cli::flag(args, "--scales")
        .map(|s| s.split(',').filter(|c| !c.is_empty()).map(|c| c.trim().parse::<f32>().expect("--scales wants floats")).collect())
        .unwrap_or_default();
    let colors: Vec<u8> = cli::flag(args, "--colors")
        .map(|s| s.split(',').filter(|c| !c.is_empty()).map(|c| c.trim().parse::<u8>().expect("--colors wants bytes 0..5")).collect())
        .unwrap_or_default();
    let n_stock = names.len();
    // embedded item files: (ident, author, bytes)
    let mut embedded: Vec<(String, String, Vec<u8>)> = Vec::new();
    if let Some(files) = cli::flag(args, "--items") {
        for file in files.split(',').filter(|s| !s.is_empty()) {
            // `F.Item.Gbx*8`: the same embedded item eight times in the row
            // (one pose each with --place) — a distance ladder of one model
            let (file, copies) = match file.rsplit_once('*') {
                Some((f, n)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => (f, n.parse::<usize>().unwrap().max(1)),
                _ => (file, 1),
            };
            let bytes = std::fs::read(file).unwrap_or_else(|e| panic!("--items {file}: {e}"));
            let (ident, author) = crate::header::item_ident_author(&bytes).unwrap_or_else(|| panic!("--items {file}: no item header ident"));
            for _ in 0..copies {
                names.push(ident.clone());
            }
            embedded.push((ident, author, bytes));
        }
    }
    if names.is_empty() {
        panic!("lineup needs --stock A,B,C and/or --items F.Item.Gbx");
    }
    let source = MapFile::load(&src);
    set_ground(source.items.first().map(|it| it.collection_raw).unwrap_or(26));
    let map_collection = source.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let n = source.items.len();
    let tmp0 = out.with_extension("lineup0.Map.Gbx");
    let mut m = MapFile::load(&src);
    m.append_item_clones(n + names.len());
    m.write_to(&tmp0).expect("write slots");
    let mut m = MapFile::load(&tmp0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    m.set_map_uid(&format!("Lin1{:08X}{:07}{:08X}", nanos % 100_000_000, std::process::id() % 10_000_000, (nanos / 7) % 100_000_000));
    for (k, name) in names.iter().enumerate() {
        let i = n + k;
        // --step X,Y,Z: the offset between two items (default pitch along +x)
        let (pos, yaw) = match places.get(k) {
            Some(p) => ([p[0], p[1], p[2]], p[3]),
            None => ([at[0] + step[0] * k as f32, at[1] + step[1] * k as f32, at[2] + step[2] * k as f32], yaw),
        };
        m.set_item_model(i, name);
        let author = embedded.iter().find(|(id, _, _)| id == name).map(|(_, a, _)| a.as_str()).unwrap_or("Nadeo");
        m.set_item_author(i, author);
        m.move_item(i, pos, yaw, cell_for(pos));
        // the appended record is a byte copy of a donor item: its pivot (and
        // pitch/roll) come along, and the game puts the PIVOT at `pos` —
        // a donor pivot of a few metres turned every yawed pusher of the
        // play-mode tests 3-4 m sideways of the car (2026-09-07)
        m.set_item_frame(i, [yaw, 0.0, 0.0], [0.0; 3]);
        m.set_item_scale(i, scales.get(k).copied().unwrap_or(1.0));
        m.set_item_variant(i, variants.get(k).copied().unwrap_or(0));
        let color = colors.get(k).copied().unwrap_or(if k < n_stock { 0 } else { 1 });
        m.set_item_color(i, color);
        println!("  {name} ({author}) at {:.0},{:.0},{:.0} colour {color}", pos[0], pos[1], pos[2]);
    }
    let tmp1 = out.with_extension("lineup1.Map.Gbx");
    m.write_to(&tmp1).expect("write models");
    // variable-length splices (the password chunk) only after a write+reload
    let mut m = MapFile::load(&tmp1);
    m.remove_password();
    // --skin K=PATH,…: the K-th item of the row (0-based) gets a placement skin
    // (`Skins\Any\Advertisement6x1\X.png` — a file put into the archive with
    // --extra, or one of the game's). The 2026-09-07 probe: does an explicit
    // skin reach an embedded item's screen where the default advertisement
    // did not?
    if let Some(list) = cli::flag(args, "--skin") {
        for entry in list.split(',').filter(|s| !s.is_empty()) {
            let (k, path) = entry.split_once('=').unwrap_or_else(|| panic!("--skin wants K=Skins\\…, got {entry:?}"));
            let k: usize = k.parse().unwrap_or_else(|_| panic!("--skin: {k:?} is not a row index"));
            assert!(k < names.len(), "--skin {k}: the row has {} items", names.len());
            let mut checksum = [0u8; 32];
            checksum[0] = 2; // what the game writes for its own skins (Summer 15: every LightColors ref)
            let f = crate::header::FileRef { version: 3, checksum, path: path.to_string(), url: String::new() };
            m.set_item_skin(n + k, Some(&f));
            println!("  skin on {} ({}): {path}", names[k], n + k);
        }
    }
    if !embedded.is_empty() {
        // the same file placed several times is ONE archive entry / manifest row
        let mut seen: Vec<&str> = Vec::new();
        let unique: Vec<&(String, String, Vec<u8>)> = embedded.iter().filter(|(id, _, _)| if seen.contains(&id.as_str()) { false } else { seen.push(id.as_str()); true }).collect();
        // a map that already embeds items (a TINY map) keeps them: its archive is
        // the base the new items are added to, its entries stay in the manifest
        // (their author is their ident, as tiny-library writes them)
        let (mut zip, existing): (Vec<u8>, Vec<String>) = crate::header::embedded_zip_bytes(&m.gbx.body).unwrap_or_default();
        for (ident, _, bytes) in &unique {
            let bytes = crate::header::set_ident_collection(bytes, map_collection);
            zip = crate::header::zip_add(&zip, &format!("Items/{ident}"), &bytes);
        }
        // --extra ARCHIVE/PATH=LOCAL,…: more files into the map's archive next
        // to the items (a texture an item names by path, a skin zip a
        // placement points at) — the 2026-09-07 probe of what an embedded
        // item can reach inside its own archive.
        if let Some(list) = cli::flag(args, "--extra") {
            for entry in list.split(',').filter(|s| !s.is_empty()) {
                let (name, local) = entry.split_once('=').unwrap_or_else(|| panic!("--extra wants ARCHIVE/PATH=LOCALFILE, got {entry:?}"));
                let bytes = std::fs::read(local).unwrap_or_else(|e| panic!("--extra {local}: {e}"));
                zip = crate::header::zip_add(&zip, name, &bytes);
                println!("  archive file {name} ({} bytes)", bytes.len());
            }
        }
        let kept: Vec<String> = existing.iter().filter(|n| n.to_ascii_lowercase().ends_with(".item.gbx")).map(|n| n.rsplit(['/', '\\']).next().unwrap_or(n).to_string()).collect();
        let mut manifest: Vec<(&str, &str)> = kept.iter().map(|n| (n.as_str(), n.as_str())).collect();
        manifest.extend(unique.iter().map(|(id, a, _)| (id.as_str(), a.as_str())));
        if !kept.is_empty() {
            println!("  kept {} embedded items of the source map", kept.len());
        }
        m.replace_embedded_objects(&manifest, &zip);
    }
    m.write_to(&out).expect("write output");
    let _ = std::fs::remove_file(&tmp0);
    let _ = std::fs::remove_file(&tmp1);
    println!("wrote {} ({} stock + {} embedded items in a row)", out.display(), n_stock, embedded.len());
}
