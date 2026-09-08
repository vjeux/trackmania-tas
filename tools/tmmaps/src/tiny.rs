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
    path::PathBuf,
};

pub mod lineup;
pub mod mapping;
pub mod tiles;

pub use lineup::{catalog_cmd, lineup_cmd};
pub use mapping::{read_mapping, Mapping, Mappings};
pub use tiles::{hidden_tiles, replaced_cells, shared_cells_cmd, stands_in_for_tile, HiddenTiles};

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
    /// The waypoint ORDER carried from the source placement (0 on a block-
    /// derived waypoint: a block record has no order field).
    order: u32,
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
pub fn set_ground(collection: u32) {
    GROUND_Y.with(|g| g.set(crate::map::ground_y(collection)));
    println!("  ground: cell row 0 at y {} (collection {collection:#x})", crate::map::ground_y(collection));
}
fn ground() -> f32 {
    GROUND_Y.with(|g| g.get())
}

pub fn block_pos(b: &crate::map::BlockRec) -> [f32; 3] {
    b.free_pos.unwrap_or_else(|| {
        let mut p = census::cell_world(b);
        p[1] = p[1] + 62.0 + ground();
        p
    })
}

/// The world point a block's prefab geometry is authored from: the cell's
/// low corner, shifted by the footprint so a quarter-turned block still covers
/// its own cells (the pairing measured in `mapgeom::place::grid_block`).
pub fn block_origin(b: &crate::map::BlockRec, footprint: (u32, u32)) -> [f32; 3] {
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

pub fn block_yaw(b: &crate::map::BlockRec) -> f32 {
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
/// `skip`: the tree indices of this placement the clearance dropped (the
/// `xv@`/`xvb@`/`xvi@` rows); returns (placed, skipped).
fn push_veget(specs: &mut Vec<Spec>, mapping: &Mappings, alias: &str, origin: [f32; 3], yaw: f32, color: u8, skip: Option<&BTreeSet<usize>>) -> (usize, usize) {
    let Some(rows) = mapping.veget_by_alias.get(alias) else { return (0, 0) };
    let (s, c) = yaw.sin_cos();
    let mut skipped = 0usize;
    for (k, (item, local, tree_yaw, pitch)) in rows.iter().enumerate() {
        if skip.map(|set| set.contains(&k)).unwrap_or(false) {
            skipped += 1;
            continue;
        }
        let pos = [origin[0] + local[0] * c + local[2] * s, origin[1] + local[1], origin[2] - local[0] * s + local[2] * c];
        let y = yaw + tree_yaw;
        specs.push(Spec { model: item.clone(), pos, yaw: y, frame: Some(([y, *pitch, 0.0], [0.0, 0.0, 0.0])), scale: 1.0, tag: None, order: 0, color });
    }
    (rows.len() - skipped, skipped)
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
    // two maps); the TINY_* variables of the environment are passed on. Without it the
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
    // --keep-zone-block: one authored block survives the deletion (the editor's
    // lightmap pass crashes on a map with none; `tinyctl lightmap`)
    let keep_zone_flag = args.iter().any(|a| a == "--keep-zone-block");
    // --name NAME sets the map's name outright; --keep-name leaves the source's
    // (default: "Tiny " + the source name)
    let name_flag: Option<String> = cli::flag(args, "--name").map(String::from);
    let keep_name = args.iter().any(|a| a == "--keep-name");

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
    // the tree clearance: trees left out (tree_clear verdicts in the mapping)
    let mut cleared_trees = 0usize;
    for it in &source.items {
        // a stock tree standing in for the map's own vegetation item, dropped
        // by the clearance: the slot is parked like a dropped item
        if mapping.drop_items.contains(&it.index) {
            cleared_trees += 1;
            specs.push(Spec { model: it.model.clone(), pos: [8.0, -900.0, 8.0], yaw: 0.0, frame: None, scale: 1.0, tag: None, order: 0, color: 0 });
            continue;
        }
        match mapping.items_by_index.get(&it.index) {
            // "-": intentionally gone (procedural vegetation the tiny map
            // cannot shrink); the slot is parked far below the map. A
            // vegetation CLUSTER item leaves its trees behind as stock items
            // (`v@<model>` rows) at the placement's position and yaw.
            Some(map) if map.model == "-" => {
                dropped_items += 1;
                specs.push(Spec { model: it.model.clone(), pos: [8.0, -900.0, 8.0], yaw: 0.0, frame: None, scale: 1.0, tag: None, order: 0, color: 0 });
                let (placed, skipped) = push_veget(&mut cluster_trees, &mapping, &it.model, transform(it.pos, source_anchor, target_anchor, scale), it.yaw, colors.item(it.index), mapping.skip_item_trees.get(&it.index));
                prefab_trees += placed;
                cleared_trees += skipped;
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
                    // A parked placement (the club's custom items: re-pointed
                    // at the first block item and "sunk 1 000 m") stays INSIDE
                    // the map's volume, 4 m above the lowest cell row like the
                    // flag driver. Summer 24 (2026-09-08): the tiny map showed
                    // one red DecoPlatformBase slab — the stand-in — floating
                    // at the water surface at the exact CENTRE of the map,
                    // cell (32,32), where nothing is placed; the 15 stand-ins
                    // at y ≈ −990 are the only placements outside the volume
                    // with that model, and an out-of-volume embedded item is
                    // apparently put back at the map's centre by the game.
                    // (The parked stock trees at (8,−900,8) showed no such
                    // pile — stock items seem exempt — and are left alone.)
                    let floor = crate::map::ground_y(collection) + 4.0;
                    if pos[1] < floor {
                        pos[1] = floor;
                    }
                    sunk_items += 1;
                }
                specs.push(Spec {
                    model: map.model.clone(),
                    pos,
                    yaw: it.yaw,
                    frame,
                    scale: it.scale * scale / map.model_scale,
                    tag: it.waypoint_tag.clone(), order: it.waypoint_order,
                    color: colors.item(it.index),
                });
            }
            None => specs.push(Spec {
                model: it.model.clone(),
                pos: transform(it.pos, source_anchor, target_anchor, scale),
                yaw: it.yaw,
                frame: None,
                scale: it.scale * scale,
                tag: it.waypoint_tag.clone(), order: it.waypoint_order,
                color: colors.item(it.index),
            }),
        }
    }
    let original_items = specs.len();
    specs.extend(cluster_trees);
    // The flag cloth (Flag16m / Flag8m re-pointed at our vertex-tween copies)
    // draws only while a STOCK flag item is DRAWN in the same view: our tween
    // draw borrows the per-material frame state the stock's draw fills each
    // frame, and reads it right only at the SAME detail level (the state is a
    // vertex base into the level's frame table). So (2026-09-08, anim thread,
    // lineups an9–an13 on tiny 18): our cloth keeps the PACK detail ladder
    // (`TINY_FLAG_LADDER=pack`, the builder's default for the tween part) and a
    // stock flag of the same kind hangs UPSIDE DOWN at every converted flag
    // placement — the same distance from the camera as the cloth it drives, so
    // both switch level together; its pole and cloth point into the ground.
    // Measured: proper waving cloth in the placement's own colour at 10–200 m
    // with the driver in view (colour is not borrowed: green/blue/default
    // cloths over red drivers). No stock drawn → bare pole; stock in the map
    // but out of view → nothing or shards. `TINY_FLAG_DRIVER` picks the form:
    //   `twins` (default): the upside-down stock flag at each placement, except
    //     the placements the library marked `xf@` (nothing below to hide it in:
    //     a deck over open air — those cloths stay at frame 0);
    //   `twins:DEPTH`: upright, DEPTH metres under the placement (the 2026-09-08
    //     morning probe form);
    //   `anchor`: one stock flag per kind under the spawn (drives nothing
    //     beyond ~100 m; kept for A/B); `0`: none.
    // ⚠ HACK, named in TINY.md "Animated items" and in the build report: the
    // proper form is a self-contained embedded tween, not found yet.
    let driver = std::env::var("TINY_FLAG_DRIVER").unwrap_or_else(|_| "twins".into());
    let tween_on = std::env::var("TINY_FLAG_TWEEN").as_deref() == Ok("1");
    if driver != "0" && tween_on {
        let is_converted_flag = |it: &crate::map::ItemRec| matches!(it.model.as_str(), "Flag16m" | "Flag8m") && mapping.items_by_index.get(&it.index).map(|m| m.model.ends_with(".Item.Gbx")).unwrap_or(false);
        if driver == "anchor" {
            let converted: BTreeSet<&str> = source.items.iter().filter(|it| is_converted_flag(it)).map(|it| it.model.as_str()).collect();
            for name in converted {
                let pos = [target_anchor[0], crate::map::ground_y(collection) + 4.0, target_anchor[2]];
                specs.push(Spec { model: name.to_string(), pos, yaw: 0.0, frame: None, scale: 1.0, tag: None, order: 0, color: 0 });
                println!("  flag driver: one stock {name} parked at {:.0},{:.0},{:.0} (keeps the tween cloths animating)", pos[0], pos[1], pos[2]);
            }
        } else {
            let depth: Option<f32> = driver.strip_prefix("twins").and_then(|s| s.strip_prefix(':')).and_then(|s| s.parse().ok());
            let (mut n, mut skipped) = (0usize, 0usize);
            for it in source.items.iter().filter(|it| is_converted_flag(it)) {
                if mapping.no_driver.contains(&it.index) {
                    skipped += 1;
                    continue;
                }
                let mut pos = transform(it.pos, source_anchor, target_anchor, scale);
                let frame = match depth {
                    Some(d) => {
                        pos[1] -= d;
                        None
                    }
                    // upside down about the placement point: the pole goes down.
                    // Raised by TINY_FLAG_DRIVER_LIFT metres (default 0.5): a driver
                    // wholly under the ground is occlusion-culled on and off, and
                    // every culled frame is a frame our cloth has no state — the
                    // cloths flickered on tiny 13 (drv13, 2026-09-08). With the
                    // stub of its pole base above the ground its box passes the
                    // test and it is drawn every frame.
                    None => {
                        let lift: f32 = std::env::var("TINY_FLAG_DRIVER_LIFT").ok().and_then(|s| s.parse().ok()).unwrap_or(0.5);
                        pos[1] += lift;
                        Some(([it.yaw, std::f32::consts::PI, 0.0], [0.0, 0.0, 0.0]))
                    }
                };
                specs.push(Spec { model: it.model.clone(), pos, yaw: it.yaw, frame, scale: 1.0, tag: None, order: 0, color: colors.item(it.index) });
                n += 1;
            }
            if n + skipped > 0 {
                match depth {
                    Some(d) => println!("  ⚠ HACK flag driver: {n} stock flag twins {d} m under the converted flags (each drives the tween cloth above it); {skipped} placements without one (xf@ rows)"),
                    None => println!("  ⚠ HACK flag driver: {n} stock flags hung upside down under the converted flag placements (each drives the tween cloth above it, TINY.md \"Animated items\"); {skipped} placements without one (deck over open air — still cloth)"),
                }
            }
        }
    }
    let mut empty_blocks = 0usize;
    // Terrain (zone) tiles under blocks are left out — authored or generated
    // (`hidden_tiles`: the game draws the block and its fillers there, never
    // the tile; drawn as items both were coplanar). Zone names come from the
    // genealogy chunk. The special case this began as (2026-09-06): a
    // `PlatformGrassOnLandHillSlopeBase` sharing its cell with the LandHill1
    // Deadend it stands in for (Summer 03, cells (37,17,22) and (38,17,22)) —
    // both as items, the hill's bumps poked through the deck as a snow blob.
    let zones: BTreeSet<String> = source.genealogy_zones().into_iter().collect();
    let info_of = |b: &BlockRec| -> Option<(String, Vec<[i32; 3]>, Option<(Vec<([i32; 3], String)>, i32)>)> {
        mapping.by_index.get(&b.index).or_else(|| mapping.by_name.get(&b.name)).map(|m| (m.model.clone(), m.units.clone(), m.auto_terrain.clone()))
    };
    let hidden = hidden_tiles(&source, &zones, &info_of);
    let undeclared: Vec<String> = source
        .blocks
        .iter()
        .chain(source.baked.iter())
        .filter(|t| zones.contains(&t.name) && hidden.undeclared(t))
        .map(|t| format!("{} at {:?} under {}", t.name, t.coords(), hidden.occupant(t).and_then(|i| source.blocks.get(i)).map(|b| b.name.as_str()).unwrap_or("?")))
        .collect();
    println!("  terrain tiles under blocks: {} blocks with geometry occupy tile cells, {} declare their auto terrain; {} tiles hidden under a block that did not declare their zone{}", hidden.blocks, hidden.declaring, undeclared.len(), if undeclared.is_empty() { String::new() } else { format!(" ({})", undeclared.iter().take(12).cloned().collect::<Vec<_>>().join("; ")) });
    let mut replaced_terrain = 0usize;
    let mut replaced_baked_terrain = 0usize;
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
        if zones.contains(&b.name) && hidden.hides(b) {
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
            tag: b.waypoint_tag.clone(), order: 0,
            color: colors.block(b.index),
        });
        let (placed, skipped) = push_veget(&mut specs, &mapping, &map.model, pos, rot[0], colors.block(b.index), mapping.skip_block_trees.get(&b.index));
        prefab_trees += placed;
        cleared_trees += skipped;
    }
    // Baked (generated) non-Sea blocks -- the FC clip fillers that finish the
    // authored structures (pillar feet, screen caps, wall faces) -- become
    // items too; the baked chunk itself is rewritten to all-Sea below.
    let mut baked_items = 0usize;
    // the filler colour rule (see the `color` block below): `owner` is what
    // the game draws; `default`, `file` and `inherit` (the behaviour until
    // 2026-09-08) stay for A/Bs
    let filler_color_rule = std::env::var("TINY_FILLER_COLOR").unwrap_or_else(|_| "file".to_string());
    if !matches!(filler_color_rule.as_str(), "owner" | "default" | "file" | "inherit") {
        panic!("TINY_FILLER_COLOR={filler_color_rule}: want owner | default | file | inherit");
    }
    println!("  filler colour rule: {filler_color_rule}");
    // the authored grid blocks per cell, for the `owner` rule
    let occupants = crate::fillers::occupants(&source);
    let mut pillar_fillers = 0usize;
    // cell -> colour of the authored (non-free) block there, for the fillers
    let cell_colors: BTreeMap<(i32, i32, i32), u8> = source
        .blocks
        .iter()
        .filter(|b| b.free_rot.is_none() && colors.block(b.index) != 0)
        .map(|b| ((b.file_cell[0] as i32, b.file_cell[1] as i32, b.file_cell[2] as i32), colors.block(b.index)))
        .collect();
    for b in &source.baked {
        let Some(map) = mapping.baked_by_index.get(&b.index) else { continue };
        if map.model == "-" {
            continue;
        }
        // A GENERATED terrain tile under a block that declares it as its own
        // ground is not drawn by the game either (Summer 04's flat Grass is
        // baked: the start road, the finish platforms and gates stood on 38 of
        // them — see `hidden_tiles`).
        if zones.contains(&b.name) && hidden.hides(b) {
            replaced_baked_terrain += 1;
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
        // The colour byte of a GENERATED filler: the byte the file records for
        // it — the generating block's colour, written by the editor (Summer 20:
        // 4956 of 7287 baked are Red like the 1966 Red blocks; the editor holds
        // the same bytes, /mapblocks2 on the original) — EXCEPT for the walls of
        // a PILLAR, which the game draws untinted. Same-camera frames of the
        // originals (2026-09-08, `tinyctl shoot` own10/col20/col15): a
        // DecoWallBaseVFC panel owned by a colour-2 DecoWallBaseGrass or a
        // PlatformPlasticSlope2LoopStart is GREEN (TrackWall's hue mask covers
        // the whole wall: TrackWallPxz_D_HueMask is (0,f6,06) alpha 0.95
        // everywhere), a VFC between two empty cells too; but every panel of a
        // generated pillar (DecoWallBasePillar, WaterWallPillar — flag 0x4000,
        // byte 4/2/3 in the file) is the untinted TrackWall tan: 20 cp3's tall
        // wall, 10's start pillar and pool walls, 15's pool wall. Recorded in
        // the cell the panel is drawn in, a vertical clip belongs to the block
        // ACROSS its side (fillers.rs); with nothing across, to the block of its
        // own cell. A pillar there -> Default. Until 2026-09-08 a colourless
        // filler even INHERITED a colour from its cell's authored block, else
        // its vertical, else its horizontal neighbours (`inherit`, the wrong
        // way round for the pillars). `TINY_FILLER_COLOR`: `owner` (the rule),
        // `default` (every filler 0), `file` (the byte), `inherit` — A/Bs.
        let color = match filler_color_rule.as_str() {
            "owner" => {
                let own_byte = colors.baked(b.index);
                if b.free_rot.is_some() {
                    own_byte
                } else {
                    let pillar_only = |cell: Option<&Vec<&BlockRec>>| -> bool { cell.map(|v| !v.is_empty() && v.iter().all(|x| x.flags & crate::fillers::FLAG_PILLAR != 0)).unwrap_or(false) };
                    let own = occupants.get(&b.file_cell);
                    let acr = crate::fillers::across(b.file_cell, b.dir).and_then(|k| occupants.get(&k));
                    let owner_is_pillar = match acr {
                        Some(_) => pillar_only(acr),
                        None => pillar_only(own),
                    };
                    if owner_is_pillar {
                        pillar_fillers += 1;
                        0
                    } else {
                        own_byte
                    }
                }
            }
            "default" => 0,
            "file" => colors.baked(b.index),
            _ => {
                let own = colors.baked(b.index);
                let c = (b.file_cell[0] as i32, b.file_cell[1] as i32, b.file_cell[2] as i32);
                let at = |dx: i32, dy: i32, dz: i32| -> Option<u8> { cell_colors.get(&(c.0 + dx, c.1 + dy, c.2 + dz)).copied() };
                let vote = |offsets: &[(i32, i32, i32)]| -> Option<u8> {
                    let mut votes: BTreeMap<u8, usize> = BTreeMap::new();
                    for &(dx, dy, dz) in offsets {
                        if let Some(col) = at(dx, dy, dz) {
                            *votes.entry(col).or_insert(0) += 1;
                        }
                    }
                    votes.into_iter().max_by_key(|(_, n)| *n).map(|(col, _)| col)
                };
                if own != 0 || b.free_rot.is_some() {
                    own
                } else {
                    at(0, 0, 0).or_else(|| vote(&[(0, 1, 0), (0, -1, 0)])).or_else(|| vote(&[(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)])).unwrap_or(0)
                }
            }
        };
        specs.push(Spec {
            model: map.model.clone(),
            pos,
            yaw: rot[0],
            frame: Some((rot, [0.0, 0.0, 0.0])),
            color,
            scale: scale / map.model_scale,
            tag: None,
            order: 0,
        });
        let (placed, skipped) = push_veget(&mut specs, &mapping, &map.model, pos, rot[0], color, mapping.skip_baked_trees.get(&b.index));
        prefab_trees += placed;
        cleared_trees += skipped;
    }
    assert!(specs.iter().any(|s| s.tag.as_deref() == Some("Spawn")));
    assert!(specs.iter().any(|s| s.tag.as_deref() == Some("Goal")));
    // (The engine's START is a fixed item slot per map — a validation record
    // names the start waypoint by index, see collhash — not the last non-Goal
    // record: the record-order rule of 2026-09-07 was refuted within the hour.)

    let tmp0 = out.with_extension(format!("tiny-{}.slots.Map.Gbx", std::process::id()));
    let tmp1 = out.with_extension(format!("tiny-{}.models.Map.Gbx", std::process::id()));
    let tmp2 = out.with_extension(format!("tiny-{}.waypoints.Map.Gbx", std::process::id()));

    // Stage 0: grow the item array before any saved offsets are used.
    let base = host.clone().unwrap_or_else(|| src.clone());
    let mut m = MapFile::load(&base);
    m.append_item_clones(specs.len());
    m.write_to(&tmp0).expect("write item-slot stage");

    // Stage 1: the source's blocks go. DELETED: the authored records and the
    // generated non-foundation fillers are cut out of the block chunks
    // (`remove_blocks` rewrites everything that lists blocks — counts, free
    // positions, colours, lightmap quality, macroblock refs, the items'
    // snapped-on tables), the way the reference tiny maps have ZERO authored
    // blocks. The deletion is variable-length and needs its own reload so the
    // item and block regions cannot shift each other's saved offsets.
    //
    // The old alternative — PARKING: every record kept, moved to cell (0,0,0)
    // and renamed to a stand-in block — is gone (2026-09-07). It was the whole
    // cause of Summer 05's 5.6-minute load: the profile showed the game's
    // lightmapper (`NHmsLightMap::RenderLightBall`) spending 91 % of the load
    // on the 2 116 stand-ins stacked in one cell (05 has no RoadTechStraight,
    // so they were RoadDirtCheckpoint arches — lit blocks). Deleted: 10 s.
    let mut m = MapFile::load(&tmp0);
    let old_uid = m
        .body_ids
        .first()
        .and_then(|f| f.name.clone())
        .expect("map uid");
    let new_uid = format!("Tin2{}", &old_uid[..23]);
    {
        // The foundation records stay: `Sea` (BlueBay's water).
        let keep_baked: BTreeSet<String> = ["Sea".to_string()].into_iter().collect();
        let n_blocks = m.blocks.len();
        let n_baked = m.baked.len();
        // --keep-zone-block: ONE authored block stays — the first one named
        // after the map's ambient terrain zone (GreenCoast `Lake`, RedIsland /
        // WhiteShore `Water`, Stadium `Grass`), in place, full size. The game
        // regenerates exactly that tile from the genealogy at load anyway (a
        // 0-block 09 re-saved by the editor holds 4096 Lake tiles at −30 m), so
        // it is invisible — but the editor's ComputeShadows pass crashes with a
        // STACK_OVERFLOW on a map with NO authored block (3 of 3 on 09, 2026-09-07)
        // and works with one, so `tinyctl lightmap` needs this. Off by default:
        // the published form has zero authored blocks like the reference maps.
        let keep_zone_block: Option<usize> = if keep_zone_flag {
            let zone = source.ambient_zone();
            let pick = m.blocks.iter().find(|b| zone.as_deref() == Some(b.name.as_str())).or_else(|| m.blocks.first()).map(|b| b.index);
            if let Some(i) = pick {
                let b = &m.blocks[i];
                println!("  kept authored block {} `{}` at cell {:?} (--keep-zone-block; zone {:?})", b.index, b.name, b.coords(), zone);
            }
            pick
        } else {
            None
        };
        let r = m.remove_blocks(|b| Some(b.index) != keep_zone_block, |b| !keep_baked.contains(&b.name));
        println!(
            "  deleted {} of {} authored blocks and {} of {} generated (baked) blocks (kept: {}); {} free-block entries, {} snapped-on groups ({} items un-snapped); lookback table {} -> {} strings",
            r.blocks, n_blocks, r.baked, n_baked, keep_baked.iter().cloned().collect::<Vec<_>>().join(","), r.free_entries, r.snap_groups, r.snapped_items_cleared, r.table_before, r.table_after
        );
        m.write_to(&tmp1).expect("write block-deletion stage");
        m = MapFile::load(&tmp1);
        assert_eq!(m.blocks.len(), usize::from(keep_zone_block.is_some()), "{} authored blocks survived the deletion", m.blocks.len());
        assert!(m.baked.iter().all(|b| keep_baked.contains(&b.name)), "a generated block outside the keep set survived the deletion");
    }
    m.set_map_uid(&new_uid);
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
            // … and the map's collection, which is what the manifest row
            // says: a source placement of a club item (a Stadium-collection
            // ident inside a BlueBay map, Summer 21's TME items) keeps its
            // Stadium word through the re-pointing otherwise, and the game,
            // resolving the FULL ident, finds no (AC00000000, Stadium,
            // AC00000000) → "Missing Items" on every load (2026-09-08).
            if s.model.ends_with(".Item.Gbx") {
                m.set_item_collection(i, collection);
            }
        }
        m.move_item(i, s.pos, s.yaw, cell_for(s.pos));
        if let Some((rot, pivot)) = s.frame {
            m.set_item_frame(i, rot, pivot);
        }
        m.set_item_scale(i, s.scale);
        // The variant byte indexes the SOURCE model's variant list: an
        // embedded copy is built for one variant (always 0); a stock stand-in
        // keeps the byte unless the mapping says otherwise (`iv@` row —
        // `Show` variant 28, the fogger rig, re-pointed at `ShowFogger8M`,
        // whose only variant is 0; 2026-09-08).
        if s.model.ends_with(".Item.Gbx") {
            m.clear_item_variant(i);
        } else if let Some(v) = mapping.variant_by_index.get(&i) {
            m.set_item_variant(i, *v);
        }
        m.set_item_color(i, s.color);
    }
    m.write_to(&tmp2).expect("write model stage");

    // Stage 3: variable-length waypoint nodes.
    let mut m = MapFile::load(&tmp2);
    for (i, s) in specs.iter().enumerate() {
        m.set_item_waypoint(i, s.tag.as_deref(), s.order);
    }
    m.write_to(&tmp2).expect("write waypoint stage");

    // Stage 4: embed the converted block models. The source's own archive
    // (custom items: the TME nation items) is replaced; a custom item still
    // placed after the mapping keeps its ORIGINAL file, carried over into the new
    // archive, so the game finds every model the map names (a dangling name is
    // "Missing Items" on load). Anything still placed whose file the source does
    // not carry is a refusal.
    let mut carried: Vec<(String, Vec<u8>)> = Vec::new();
    if let Some((src_zip, names)) = crate::header::embedded_zip_bytes(&source.gbx.body) {
        let files: Vec<String> = names.iter().map(|n| n.replace('/', "\\").to_ascii_lowercase()).collect();
        let still: Vec<String> = specs
            .iter()
            .map(|s| s.model.clone())
            .filter(|m| files.iter().any(|f| f == &format!("items\\{}", m.to_ascii_lowercase())))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if !still.is_empty() {
            let entries = crate::header::zip_entries(&src_zip);
            for model in &still {
                let want = format!("items\\{}", model.to_ascii_lowercase());
                match entries.iter().find(|(n, _)| n.replace('/', "\\").to_ascii_lowercase() == want) {
                    Some((n, bytes)) if !bytes.is_empty() => carried.push((n.replace('\\', "/"), bytes.clone())),
                    _ => panic!("source map embeds custom object {model} still placed after the mapping, and its file cannot be read from the source archive"),
                }
            }
            println!("  source archive ({} files) replaced; {} custom items still placed keep their original files ({})", names.len(), carried.len(), still.join(", "));
        } else {
            println!("  source archive ({} files) replaced: every custom item is re-pointed at a scaled copy", names.len());
        }
    }
    let mut m = MapFile::load(&tmp2);
    m.remove_password();
    // The map's own NAME: "Summer 2026 - 15" becomes "Tiny Summer 2026 - 15",
    // so the editor title bar, the map list and the playground HUD say which
    // one you are looking at (the Nadeo record has said "Tiny …" since map 01;
    // the file kept the original's name, so every screenshot of a tiny map was
    // labelled like the original — vjeux, 2026-09-07). `--name NAME` sets it
    // outright, `--keep-name` leaves the source name.
    if !keep_name {
        let old = crate::header::read(&src.display().to_string()).ok().map(|h| h.name).unwrap_or_default();
        if old.is_empty() || old == "-" {
            println!("  map name: the source declares none; left alone");
        } else {
            let new = name_flag.clone().unwrap_or_else(|| format!("Tiny {old}"));
            let (h, b) = m.set_map_name(&old, &new);
            println!("  map name: {old:?} -> {new:?} ({h} in the header, {b} in the body)");
        }
    }
    // The source's stored lightmap STAYS. It
    // was computed for the full-size layout and the game applies it BY OBJECT
    // INDEX — in PLAY mode a build that kept its blocks (Summer 15, 2026-09-07)
    // drew every converted-block item BLACK, the appended items falling
    // outside the lightmap's tables — but with the blocks DELETED the game
    // rejects a stale lightmap whose block count is 0 on its own (delete-blocks thread: deleted builds render the same with or
    // without it, in play AND in the editor — verified on 09's start deck and
    // cp5 tunnel, 2026-09-07 14:11), while a 0-block map WITHOUT a lightmap
    // makes the EDITOR crash with a STACK_OVERFLOW during or right after the
    // load (3 of 4 opens of the tiny 09; Trackmania.exe+0x96d7e0 recursing
    // under an Openplanet frame — the editor's automatic lightmap pass over a
    // map with no blocks, presumably), which kills every shootset comparison.
    // The game recomputes a default-settings lightmap at every load anyway
    // (0.4 s on a 0-block map, measured 2026-09-07).
    println!("  stored lightmap kept (0 blocks: the game rejects it; without one the editor crashes)");
    // The MediaTracker (chunk 0x03043049, `mediatracker.rs`): the intro, the
    // in-game and the end-race clips fly cameras over FULL-SIZE coordinates
    // and fire from full-size trigger cells; both go through the items'
    // transform (times, angles and fields of view stay: the clip lasts the
    // same seconds over a half-size map). Blocks whose layout the reader does
    // not know are copied verbatim and listed.
    match m.mediatracker() {
        None => println!("  MediaTracker: no chunk 0x03043049 in this map"),
        Some(Err(e)) => eprintln!("  WARNING: MediaTracker left untouched, its cameras fly over the full-size layout: {e}"),
        Some(Ok(mut mt)) => {
            {
                    // The trigger grid is doubled first (3x1x3 -> 6x2x6 cells per
                    // block, every source cell re-expressed as its 2x2x2 finer
                    // cells, exactly): a half-size volume then lands on cells of
                    // its own size instead of the coarse ones, and the tiny
                    // trigger is as tight as the original's (Summer 15's spawn-
                    // ahead test trigger: 10.7 m deep instead of 21.3 m). The game
                    // honours the chunk's trigger size — measured 2026-09-07: the
                    // same clip fired at the same car position with the 3x1x3 and
                    // the 6x2x6 encoding (camera jump 12.96 s / 13.01 s into the
                    // logs, entry 12.97 / 13.03).
                    if let Some(t0) = mt.trigger_size {
                        if let Err(e) = mt.set_trigger_size([t0[0] * 2, t0[1] * 2, t0[2] * 2]) {
                            eprintln!("  WARNING: trigger grid kept at {t0:?}: {e}");
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
    if library.as_os_str() != "-" {
        let mut zip = std::fs::read(&library).unwrap_or_else(|e| panic!("{}: {e}", library.display()));
        assert!(
            zip.starts_with(b"PK\x03\x04"),
            "{} is not a ZIP archive",
            library.display()
        );
        for (name, bytes) in &carried {
            zip = crate::header::zip_add(&zip, name, bytes);
        }
        let mut embedded_names: Vec<String> = specs
            .iter()
            .enumerate()
            .filter(|(i, _)| *i >= original_items || mapping.items_by_index.contains_key(i))
            .filter(|(_, s)| s.model.ends_with(".Item.Gbx"))
            .map(|(_, s)| s.model.clone())
            .collect();
        // the carried custom items are embedded too (their original files)
        for (name, _) in &carried {
            embedded_names.push(name.trim_start_matches("Items/").to_string());
        }
        embedded_names.sort();
        embedded_names.dedup();
        // The archive carries ONLY what the manifest names (plus the non-item
        // support files — the sign-logo DDS). The library builds one item per
        // block recipe and per item variant, and a model whose every placement
        // was dropped (a tree over the track, a parked custom item) used to
        // travel along unlisted: 40 dead entries in 21, 114 in 25
        // (2026-09-08). Every entry counts against the loader's flakiness
        // band (≥ ~757 entries flaky) and the upload cap, so the dead ones go.
        {
            let listed: std::collections::BTreeSet<&str> = embedded_names.iter().map(|s| s.as_str()).collect();
            let entries = crate::header::zip_entries(&zip);
            let before = entries.len();
            let kept: std::collections::BTreeMap<String, Vec<u8>> = entries
                .into_iter()
                .filter(|(name, _)| {
                    let base = name.trim_start_matches("Items/");
                    !name.ends_with(".Item.Gbx") || listed.contains(base)
                })
                .collect();
            if kept.len() != before {
                println!("  archive: {} of {} entries kept ({} unplaced item models pruned)", kept.len(), before, before - kept.len());
                zip = crate::header::deflated_zip(&kept);
            }
        }
        let manifest: Vec<(&str, &str)> = embedded_names
            .iter()
            .map(|name| (name.as_str(), name.as_str()))
            .collect();
        m.replace_embedded_objects(&manifest, &zip);
    }
    // The source's VALIDATION GHOST (chunk 0x0305B00F) is the original's
    // full-size author run: kept, the game replays it over the half-size
    // track as a car driving in the air (vjeux, 2026-09-08: "why is there a
    // car driving on top of me" — every map published before this carried
    // it). It is replaced by the dummy real ghost of `tmmaps stripghost`
    // (the form the player project's author-ghost embed replaces in place)
    // and the header goes unvalidated. `--keep-ghost` keeps the source's.
    if !args.iter().any(|a| a == "--keep-ghost") {
        let removed = m.strip_validation_ghost_to(crate::map::GhostForm::Dummy);
        if removed > 0 {
            println!("  validation ghost: the source's ({removed} bytes) replaced by the dummy ghost, header validated=\"0\"");
        } else {
            println!("  validation ghost: none in the source (the header goes unvalidated)");
        }
    }
    m.write_to(&out).expect("write output");
    for p in [&tmp0, &tmp1, &tmp2] {
        let _ = std::fs::remove_file(p);
    }
    // Genealogies (chunk 0x03043043) are the per-cell terrain zones the game
    // regenerates Land/Beach/Hill/Cliff blocks from at load: with the authored
    // terrain gone they rebuilt the full-size island under the tiny one
    // (2026-09-06). Cleared, the floor cells without a block are plain sea.
    // (Rewriting the baked chunk to one Sea record per cell is NOT needed —
    // fabricated records make the game refuse the map: "Couldn't load map!";
    // that writer is gone.)
    // Stadium keeps it: its zones are the grass floor, full size under the
    // tiny map like the reference maps (and there is no sea to fall into).
    // TINY_GENEALOGY=clear|fill|keep overrides the per-collection policy (a
    // diagnostic: what the game regenerates under the island is only visible
    // in the game). TINY_KEEP_GENEALOGY is the old spelling of `keep`.
    let policy: Option<String> = std::env::var("TINY_GENEALOGY").ok().or_else(|| std::env::var_os("TINY_KEEP_GENEALOGY").map(|_| "keep".to_string()));
    let policy = policy.as_deref().unwrap_or(match collection {
        0x1c => "clear",
        0x10 | 0x1d | 0xf => "fill",
        _ => "keep",
    });
    match policy {
        // BlueBay: the sea around the island is decoration, so no zone
        // at all leaves plain sea under the tiny map.
        "clear" => {
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
        "fill" => {
            let (zone, n) = MapFile::fill_genealogy_file(&out).expect("fill genealogies");
            println!("  genealogy chunk filled: {n} cells of {zone}");
        }
        "keep" => println!("  genealogy chunk kept as the source's"),
        other => panic!("TINY_GENEALOGY={other}: clear, fill or keep"),
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
    println!("  {} existing items re-pointed at scaled copies ({} vegetation stand-ins sunk to half-tree crown height); {} dropped (procedural vegetation); {} blocks intentionally without an item (empty variants); {} terrain tiles replaced by the block standing in for them ({} authored + {} generated); {} prefab trees placed as stock items; {} trees left out by the clearance (overlapping a deck)", repointed_items, sunk_items, dropped_items, empty_blocks, replaced_terrain + replaced_baked_terrain, replaced_terrain, replaced_baked_terrain, prefab_trees, cleared_trees);
    println!(
        "  scaled every authored object: {} blocks + {} items = {} item placements",
        source.blocks.len(),
        source.items.len(),
        specs.len()
    );
    println!(
        "  baked foundation: {} generated blocks in the source ({} re-emitted as items, {} of them pillar walls placed Default)",
        source.baked.len(),
        baked_items,
        pillar_fillers
    );
    println!(
        "  anchor: source {:?} -> target {:?}; scale {:.3}",
        source_anchor, target_anchor, scale
    );
}

