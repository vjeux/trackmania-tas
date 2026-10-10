//! `tmmaps surface SRC.Map.Gbx --plan RECORDS.tsv --out OUT.Map.Gbx [--name NAME]
//! [--uid-prefix Ice2] [--keep-ghost] [--strip-lightmap]` — apply a surface-swap
//! plan (`mapgeom surface-plan MAP --surface ice --records RECORDS.tsv`): every
//! `rename` row's block model becomes its same-footprint twin of the other
//! surface (`PlatformTechBase` -> `PlatformIceBase`), a `delete` row's record
//! goes (a pillar swallowed by a twin's extra cells), a row whose new cell
//! differs moves there (a twin whose deck sits a unit higher in its own frame
//! is placed a cell lower). Everything else in the file — items, times,
//! MediaTracker, thumbnail — stays as the source has it.
//!
//! Stages (each variable-length edit gets its own write + reload, like `tiny`):
//!
//! 1. deletions (`remove_blocks`: counts, free positions, colours, snap tables
//!    all move together);
//! 2. renames (the Id table re-encoded — the body's LENGTH changes, so the
//!    whole body is re-emitted; the game accepts a re-emitted body, MAPS.md §3);
//! 3. cell moves (in-place byte patches) + the new uid (same byte length);
//! 4. password off, the map's name, the source's validation ghost removed
//!    (an author ghost recorded on another surface would slide off), the
//!    stored lightmap optionally stripped.
//!
//! The plan's record indices are the SOURCE's; after stage 1 the surviving
//! blocks are renumbered, and the tool maps old -> new by counting deletions.

use crate::cli;
use crate::map::MapFile;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// One record row of the plan (the columns `mapgeom surface-plan --records` writes).
#[derive(Clone, Debug)]
pub struct PlanRec {
    pub index: usize,
    pub name: String,
    pub action: String,
    pub target: String,
    pub cell: [i32; 3],
    pub new_cell: [i32; 3],
    pub new_pos: Option<[f32; 3]>,
    pub delete: bool,
    pub free: bool,
    /// the planner's free-text note (`wetpad deck=<h>` on a wet plan's start block)
    pub detail: String,
    /// every cell the block covers (the `cells` column of records.tsv; empty for an older file)
    pub cells: Vec<[i32; 3]>,
    /// for a generated face: the index of the block record that owns it (`owner` column)
    pub owner: Option<usize>,
    /// the flags word the renamed record takes (`new_flags` column; `-` = unchanged)
    pub new_flags: Option<u32>,
}

pub fn read_plan(path: &std::path::Path) -> Vec<PlanRec> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut head: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if i == 0 {
            head = f.iter().map(|s| s.to_string()).collect();
            continue;
        }
        let col = |n: &str| -> &str {
            let k = head.iter().position(|h| h == n).unwrap_or_else(|| panic!("{}: no column `{n}`", path.display()));
            f.get(k).copied().unwrap_or("")
        };
        let int = |n: &str| -> i32 { col(n).parse().unwrap_or_else(|_| panic!("{}:{}: `{n}` = {:?}", path.display(), i + 1, col(n))) };
        let new_pos = match col("new_pos") {
            "-" | "" => None,
            s => {
                let v: Vec<f32> = s.split(',').map(|x| x.parse().unwrap_or_else(|_| panic!("{}:{}: new_pos {s:?}", path.display(), i + 1))).collect();
                Some([v[0], v[1], v[2]])
            }
        };
        out.push(PlanRec {
            index: int("index") as usize,
            name: col("name").to_string(),
            action: col("action").to_string(),
            target: col("target").to_string(),
            cell: [int("cell_x"), int("cell_y"), int("cell_z")],
            new_cell: [int("new_x"), int("new_y"), int("new_z")],
            new_pos,
            delete: col("delete") == "1",
            free: col("free") == "1",
            detail: head.iter().position(|h| h == "detail").and_then(|k| f.get(k)).unwrap_or(&"").to_string(),
            cells: head
                .iter()
                .position(|h| h == "cells")
                .and_then(|k| f.get(k))
                .map(|v| {
                    v.split(';')
                        .filter_map(|c| {
                            let n: Vec<i32> = c.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                            (n.len() == 3).then(|| [n[0], n[1], n[2]])
                        })
                        .collect()
                })
                .unwrap_or_default(),
            owner: head.iter().position(|h| h == "owner").and_then(|k| f.get(k)).and_then(|v| v.trim().parse().ok()),
            new_flags: head.iter().position(|h| h == "new_flags").and_then(|k| f.get(k)).filter(|v| **v != "-" && !v.is_empty()).map(|v| u32::from_str_radix(v.trim(), 16).unwrap_or_else(|_| panic!("{}:{}: new_flags {v:?}", path.display(), i + 1))),
        });
    }
    out
}

pub fn cmd(args: &[String]) {
    let src = PathBuf::from(&args[2]);
    let out = PathBuf::from(cli::flag(args, "--out").expect("surface needs --out MAP"));
    let plan_path = PathBuf::from(cli::flag(args, "--plan").expect("surface needs --plan RECORDS.tsv (mapgeom surface-plan --records)"));
    let uid_prefix = cli::flag(args, "--uid-prefix").unwrap_or("Ice2").to_string();
    assert!(uid_prefix.len() == 4, "--uid-prefix is 4 characters (it replaces the first 4 of the source uid)");
    let mut plan = read_plan(&plan_path);
    let source = MapFile::load(&src);
    // the plan must describe THIS map: every row names the record it was read from
    for r in &plan {
        let b = source.blocks.get(r.index).unwrap_or_else(|| panic!("plan row #{} but the map has {} authored blocks", r.index, source.blocks.len()));
        assert_eq!(b.name, r.name, "plan row #{}: the map has `{}` there", r.index, b.name);
    }
    // ⛔ NEVER BAKE A WAYPOINT RECORD. A baked block becomes an ITEM, and an item with
    // a Goal/Start/Checkpoint tag does not act as one: U10S 354's finish is a custom
    // block (`PlatformWoodFinish.Block.Gbx_CustomBlock`) and every set that baked it
    // (Dirt, Grass, Icy, Magnet, Plastic, Tiny) is unfinishable, while Wood and Wet
    // Wood, which kept it a block, work (leod 2026-10-07). The planner only knows the
    // block's NAME; the tag is on the map record, so the demotion lives here. The
    // block keeps its own look — a dead finish is never worth a dress.
    // TINY_BAKE_WAYPOINTS=1 restores the old behaviour.
    if std::env::var("TINY_BAKE_WAYPOINTS").map(|v| v != "1").unwrap_or(true) {
        let mut demoted = 0;
        for r in plan.iter_mut().filter(|r| r.action == "bake" && !r.delete) {
            let tag = source.blocks[r.index].waypoint_tag.as_deref().unwrap_or("");
            // CUSTOM blocks only (`…_CustomBlock`, or an author path with a backslash): a
            // STOCK waypoint block baked as a re-dressed copy keeps working — the whole
            // September run shipped RoadTechFinish → dirt that way — and demoting it left
            // U10S 58's finish plastic (Everios96 2026-10-09, picture 3). Why the custom
            // one dies and the stock one does not is in the item's waypoint node, which
            // the stock bake path writes and the custom path did not.
            let custom = r.name.ends_with("_CustomBlock") || r.name.contains('\\');
            if custom && matches!(tag, "Goal" | "Start" | "StartFinish" | "Checkpoint") {
                r.action = "keep".to_string();
                r.detail = format!("waypoint {tag}: kept as a BLOCK (a baked item would not count) — was: {}", r.detail);
                demoted += 1;
            }
        }
        if demoted > 0 {
            println!("  {demoted} waypoint record(s) kept as blocks instead of baked (TINY_BAKE_WAYPOINTS=1 to override)");
        }
    }
    // A COVERED black block stays the game's own. Everios96 2026-10-09: "when you place
    // down a dirt block it generates a pillar below it, and the black blocks are
    // technically the top part of the pillar, and then there is a layer of dirt placed
    // above it. But you can also place down the black roads without a surface on top,
    // which is what I want to convert… only change them if there is air above". Of the
    // 36,618 black blocks in the 975 originals 20,676 have a block directly on top. A
    // one-cell black block (`DecoWall*`, `Platform*Wall*`; not a pillar, not water)
    // whose top cell is occupied by another block is demoted to `keep`, sides and all.
    // Bigger black pieces (curves, loops, slopes: several cells) convert as before —
    // "partly covered counts as open". TINY_COVERED_BLACK=convert restores.
    if std::env::var("TINY_COVERED_BLACK").map(|v| v != "convert").unwrap_or(true) {
        let cell_of = |p: [f32; 3]| -> (i32, i32, i32) {
            ((p[0] / crate::map::CELL_XZ).floor() as i32, ((p[1] + 62.0) / 8.0).round() as i32, (p[2] / crate::map::CELL_XZ).floor() as i32)
        };
        let occupied: std::collections::HashSet<(i32, i32, i32)> = source.blocks.iter().map(|b| cell_of(crate::tiny::block_pos(b))).collect();
        // The DecoWall FULL blocks only — the kind nothing had converted before 2026-10-09.
        // The platform-wall ROADS keep converting as every September set did: the live
        // sets already carry their covered ones as wood/dirt items (1,831 blocks on 153
        // wet wood maps), and the rule exists to avoid needless NEW conversions, not to
        // turn converted blocks black again.
        let is_black = |n: &str| n.starts_with("DecoWall") && !n.contains("Pillar") && !n.contains("Water");
        let one_cell = |n: &str| !["Curve", "Loop", "Slope", "Diag", "Corner", "x2", "x3", "x4", "x6", "2In", "2Out"].iter().any(|k| n.contains(k));
        let mut covered = 0;
        for r in plan.iter_mut().filter(|r| (r.action == "bake" || r.action == "rename") && !r.delete) {
            if !is_black(&r.name) || !one_cell(&r.name) {
                continue;
            }
            let b = &source.blocks[r.index];
            let (cx, cy, cz) = cell_of(crate::tiny::block_pos(b));
            if occupied.contains(&(cx, cy + 1, cz)) {
                r.action = "keep".to_string();
                r.detail = format!("covered black block (a block sits on its top): kept as the game's own — was: {}", r.detail);
                covered += 1;
            }
        }
        if covered > 0 {
            println!("  {covered} covered black block(s) kept as they are (TINY_COVERED_BLACK=convert to override)");
        }
    }
    // --library ZIP --mapping TSV: the `bake` rows become ITEMS (mapgeom tiny-library
    // --scale 1 --only <names>, re-dressed through TINY_REDRESS / TINY_FORCE_MODIFIER)
    let library = cli::flag(args, "--library").map(PathBuf::from);
    let mapping = cli::flag(args, "--mapping").map(|p| crate::tiny::read_mapping(std::path::Path::new(p)));
    let colors = source.colors().unwrap_or(crate::map::Colors { bytes: Vec::new(), n_blocks: 0, n_baked: 0 });
    crate::tiny::set_ground(source.items.first().map(|it| it.collection_raw).unwrap_or(26));
    let collection = source.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let (item_specs, unbaked) = match &mapping {
        Some(mp) => item_specs(&source, &plan, mp, &colors),
        None => (Vec::new(), plan.iter().filter(|r| r.action == "bake" && !r.delete).map(|r| format!("#{} {} (no --mapping)", r.index, r.name)).collect()),
    };
    if !unbaked.is_empty() {
        println!("  {} bake rows stay blocks: {}", unbaked.len(), unbaked.join(", "));
    }
    // an `overlay` row's block stays (the water volume lives in the block; the item is look only)
    let overlay_indices: BTreeSet<usize> = plan.iter().filter(|r| r.action == "overlay" && !r.delete).map(|r| r.index).collect();
    let baked_indices: BTreeSet<usize> = item_specs.iter().map(|(i, _)| *i).filter(|i| !overlay_indices.contains(i)).collect();
    // A bake row with no library model is normally left as a block; a PREFAB-LESS wall of
    // a face-drawn family has no model BY NATURE (its body is the generated faces, which
    // bake separately) — leaving the block would keep the game drawing them black
    // (Everios96 2026-10-05). `TINY_FACE_BLOCK_DELETE=0` keeps the old behaviour.
    let face_block_delete = std::env::var("TINY_FACE_BLOCK_DELETE").map(|v| v != "0").unwrap_or(true);
    let face_families: Vec<String> = std::env::var("TINY_BAKE_FAMILY").unwrap_or_default().split(',').map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
    let face_only: BTreeSet<usize> = if face_block_delete {
        plan.iter().filter(|r| r.action == "bake" && !baked_indices.contains(&r.index) && face_families.iter().any(|f| r.name.starts_with(f.as_str()))).map(|r| r.index).collect()
    } else {
        BTreeSet::new()
    };
    if !face_only.is_empty() {
        println!("  {} face-drawn block(s) removed: their generated faces bake in their place", face_only.len());
    }
    // The generated `<Road>PenaltyHFC*` filler records of every penalty frame that becomes
    // an item: the game draws the frame's whole FLOOR from them (deck at the ends, penalty
    // between the posts, kerbs — the vanilla Penalty_Air prefab is posts and kerbs only), so
    // they bake too, re-dressed like the frame (the library maps them under `b@INDEX`), and
    // the records go (else the plate stays under the copies: z-fight, and the wheels touch
    // IT — wet wood 37, 2026-09-24). Fillers sit in the frame's cell and its 4 neighbours.
    let is_frame = |n: &str| n.starts_with("Road") && (n.ends_with("Penalty") || n.ends_with("PenaltyIce") || n.ends_with("PenaltyDirt"));
    let frame_cells: Vec<[i32; 3]> = plan.iter().filter(|r| baked_indices.contains(&r.index) && is_frame(&r.name)).map(|r| r.cell).collect();
    // … and the gameplay PADS' generated side skirts (`*Special*FC*`: LED-sign panels dressed
    // by the owner pad's effect — orphaned records draw a wrong effect or direction, Everios96
    // 2026-09-24): baked with the pad, records dropped. A pad's skirts sit in its 4 neighbours.
    let is_pad = |n: &str| n.contains("Special") && !n.contains("FC");
    let pad_cells: Vec<[i32; 3]> = plan.iter().filter(|r| baked_indices.contains(&r.index) && is_pad(&r.name)).map(|r| r.cell).collect();
    // A skirt record belongs to ONE pad: the block across the skirt's side `dir` (a side
    // clip finishes the block across), else its own cell, else a row above or below (a
    // Slope2/Tilt2 pad's skirts stand a row off its origin). Only a skirt whose OWNER bakes
    // is baked and dropped — a "reach" rule (±2 cells, 2026-09-24 morning) stole the wedge
    // records of a kept neighbour pad (the catalog's icy Tilt2 pad lost its base).
    // Owner cells: every authored pad by cell (baked or not), so a kept pad's skirt is seen as
    // owned-by-kept and left alone.
    let pads_by_cell: std::collections::HashMap<[i32; 3], (usize, String)> = plan.iter().filter(|r| is_pad(&r.name) && !r.free).map(|r| (r.cell, (r.index, r.name.clone()))).collect();
    let free_pads: Vec<(usize, String, [f32; 3])> = plan.iter().filter(|r| is_pad(&r.name) && r.free).filter_map(|r| source.blocks.get(r.index).and_then(|b| b.free_pos).map(|p| (r.index, r.name.clone(), p))).collect();
    let skirt_owner = |b: &crate::map::BlockRec| -> Option<usize> {
        if b.free_pos.is_some() {
            // a free skirt: the nearest free pad centre within 100 m (one-cell footprints)
            let p = b.free_pos.unwrap();
            let (s, c) = b.free_rot.map(|r| r[0]).unwrap_or(0.0).sin_cos();
            let centre = [p[0] + 16.0 * c + 16.0 * s, p[1], p[2] - 16.0 * s + 16.0 * c];
            return free_pads.iter().filter_map(|(i, _, q)| {
                let d = ((q[0] + 16.0 - centre[0]).powi(2) + (q[1] - centre[1]).powi(2) + (q[2] + 16.0 - centre[2]).powi(2)).sqrt();
                (d <= 100.0).then_some((d as i64, *i))
            }).min().map(|(_, i)| i);
        }
        let (x, y, z) = b.coords();
        let mut ranked: Vec<[i32; 3]> = Vec::new();
        for dy in [0i32, -1, 1] {
            if let Some(a) = crate::fillers::across(b.file_cell, b.dir) {
                ranked.push([a[0] as i32, y + dy, a[2] as i32]);
            }
            ranked.push([x, y + dy, z]);
        }
        ranked.iter().find_map(|c| pads_by_cell.get(c).map(|(i, _)| *i))
    };
    // The BLACK WALL family's generated faces (`TrackWall*VFC*`, `*FCT`, `*FCB`; not the
    // water walls' nor the pillars'): every non-water, non-pillar TrackWall block bakes
    // (the family has no surface variants), so every such record is owned by a baked
    // wall → baked as a re-dressed item with it and the black record dropped (the wall
    // is a thin sheet without them — the ribbon walls of 2026-10-03/04).
    // … or a face-drawn wall that goes (`face_only`): a map whose walls are ALL prefab-less
    // (five adjacent `DecoWallBase`, the cap-physics test of 2026-10-06) converted NOTHING
    // because no wall had a model of its own and this gate stayed shut.
    let wall_fillers = std::env::var("TINY_WALL_FILLERS").map(|v| v != "0").unwrap_or(true)
        && (!face_only.is_empty() || plan.iter().any(|r| baked_indices.contains(&r.index) && (r.name.starts_with("TrackWall") || r.name.starts_with("DecoWall"))));
    // A wall face belongs to the wall whose NAME it extends (`DecoWallArchSlope2End2FCT` →
    // `DecoWallArchSlope2End2`): bake it only when that wall itself baked. The engine
    // REGENERATES a clip for every authored block at load (crate::bake) — the file's record
    // is only attributes — so a face baked beside a wall that STAYED is drawn twice:
    // the game's own black face and our re-dressed copy, z-fighting (Everios96 2026-10-05
    // "the full blocks look like they are flickering (2 textures overlapping)"; 38 of the
    // 95 DecoWall kinds have no mesh of their own and could not bake).
    // The owner is found by GEOMETRY, never by name: a generated face carries a generic
    // name (`DecoWallSimpleVFC`, `DecoWallBaseVFC` — one panel kind serves many blocks), so
    // a name match loses most of them and the faces of a removed block go missing
    // (Everios96 2026-10-05: "many blocks still have invisible faces"). A side clip
    // finishes the block ACROSS its side `dir`; a top cap (FCT) sits in the cell above its
    // owner, a bottom cap (FCB) in the cell below; a panel may also sit in its owner's own
    // cell.
    let mut blocks_by_cell: std::collections::HashMap<[i32; 3], Vec<usize>> = std::collections::HashMap::new();
    for r in plan.iter().filter(|r| !r.free && !r.name.contains("FC")) {
        for c in if r.cells.is_empty() { vec![r.cell] } else { r.cells.clone() } {
            blocks_by_cell.entry(c).or_default().push(r.index);
        }
    }
    let converted = |i: &usize| baked_indices.contains(i) || face_only.contains(i);
    // the exact answer first: `faces.tsv` beside the plan carries, for every GENERATED
    // FACE record, the index of the block that owns it — resolved by the game's own rule
    // (the owner's face list names the clip). A face's own name is generic, so nothing
    // else is exact (2026-10-06).
    let owner_of: std::collections::HashMap<usize, usize> = {
        let fpath = plan_path.with_file_name("faces.tsv");
        let mut m = std::collections::HashMap::new();
        if let Ok(text) = std::fs::read_to_string(&fpath) {
            let mut lines = text.lines();
            let head: Vec<&str> = lines.next().unwrap_or_default().split('\t').collect();
            let (bi, oi) = (head.iter().position(|h| *h == "baked"), head.iter().position(|h| *h == "owner"));
            if let (Some(bi), Some(oi)) = (bi, oi) {
                for l in lines {
                    let f: Vec<&str> = l.split('\t').collect();
                    if let (Some(b), Some(o)) = (f.get(bi).and_then(|v| v.parse::<usize>().ok()), f.get(oi).and_then(|v| v.parse::<usize>().ok())) {
                        m.insert(b, o);
                    }
                }
            }
        }
        m
    };
    if !owner_of.is_empty() {
        println!("  face owners: {} generated records mapped to their block", owner_of.len());
    }
    let water_faces = std::env::var("TINY_WATER_FACES").map(|v| v != "0").unwrap_or(true);
    // A water block RENAMED to the game's own dirt twin keeps drawing BLUE: the twin has
    // no prefab either, and the floor comes from these stored face records, which the
    // rename does not touch (map 734, verified in game 2026-10-07). So a renamed owner
    // counts exactly like an overlaid one — its faces are re-dressed and replaced.
    let rename_indices: std::collections::BTreeSet<usize> = plan.iter().filter(|r| r.action == "rename" && !r.delete).map(|r| r.index).collect();
    let keep_indices: std::collections::BTreeSet<usize> = plan.iter().filter(|r| r.action == "keep" && !r.delete && r.name.contains("Water")).map(|r| r.index).collect();
    // ⛔ NEVER decide this by the face's NAME. `DecoWallWaterFCBInside` is a BOTTOM cap —
    // the pool floor under a deco wall — and `RoadWaterVFC` is part of a water road's
    // drivable channel; excluding them as "walls" left map 263's pool centre and map 23's
    // channel blue (Everios96 2026-10-08). The OWNER decides: a face whose block is a kept
    // wall never passes `wall_owner_baked`, so it stays by itself.
    // A water block's VERTICAL face clip (`…VFC…`: `DecoWallWaterBaseVFC`, `WaterWallVFC`)
    // is the pool's SIDE WALL — the blue face Everios96 wants kept (2026-10-09, picture 4:
    // "you deleted the side face of the deeper pool blocks… the side of the pool that
    // should keep the blue material"). Only the horizontal clips (`HFC`, `FC`, `FCT`,
    // `FCB`) are floor. `WaterWallVFC` escaped so far only because its prefab is empty.
    let is_water_face = |b: &crate::map::BlockRec| water_faces && b.name.contains("Water") && b.name.contains("FC") && !b.name.contains("Pillar") && !b.name.contains("VFC");
    // A FREE-placed map has no grid cells at all (map 960's records all read cell -1,0,-1),
    // so neither the owner table nor the cell search can find a water face's block. Match
    // by WORLD POSITION there: a face sits on the block it finishes (2026-10-07).
    let overlay_pos: Vec<[f32; 3]> = overlay_indices.iter().filter_map(|i| source.blocks.get(*i)).map(crate::tiny::block_pos).collect();
    let keep_pos: Vec<[f32; 3]> = keep_indices.iter().filter_map(|i| source.blocks.get(*i)).map(crate::tiny::block_pos).collect();
    let rename_pos: Vec<[f32; 3]> = rename_indices.iter().filter(|i| source.blocks.get(**i).map(|b| b.name.contains("Water")).unwrap_or(false)).filter_map(|i| source.blocks.get(*i)).map(crate::tiny::block_pos).collect();
    // 160 m: map 960's free-placed floors sit far from the block origin the plan records,
    // and 48 m left 8 of them behind. The swap stays 1:1 — a face is only removed when a
    // re-dressed copy of ITS OWN geometry was placed at ITS OWN position.
    let near_r: f32 = std::env::var("TINY_WATER_FACE_RADIUS").ok().and_then(|v| v.parse().ok()).unwrap_or(160.0);
    let wall_owner_baked = |b: &crate::map::BlockRec| -> bool {
        // A WATER face's owner is KEPT (`overlay`) — the game's block stays and only its
        // floor is re-dressed — so `converted` is the wrong question for it: the face
        // itself is what we replace, and leaving the original beside our copy means the
        // game keeps drawing its blue floor on top (263, 2026-10-07). This holds for the
        // geometric fallback too: 62 of map 960's water faces resolve to no owner at all.
        // …and a KEPT water WALL block still owns FLOOR pieces (`WaterFCCenter` and friends
        // sit inside the wall ring): the block's own wall geometry stays the game's because
        // the block is kept, but the floor faces it owns are the pool bottom and convert.
        // Without this, 853 floor pieces on U10S 72 and 179 on map 10 stayed blue
        // (Everios96 2026-10-08).
        let ok = |i: &usize| converted(i) || (is_water_face(b) && (overlay_indices.contains(i) || rename_indices.contains(i) || keep_indices.contains(i)));
        if let Some(o) = owner_of.get(&b.index) {
            return ok(o);
        }
        let (x, y, z) = b.coords();
        let mut cells: Vec<[i32; 3]> = Vec::new();
        if let Some(a) = crate::fillers::across(b.file_cell, b.dir) {
            cells.push([a[0] as i32, y, a[2] as i32]);
        }
        cells.push([x, y, z]);
        if b.name.ends_with("FCT") || b.name.contains("FCT") {
            cells.push([x, y - 1, z]);
        }
        if b.name.ends_with("FCB") || b.name.contains("FCB") {
            cells.push([x, y + 1, z]);
        }
        for dy in [0i32, -1, 1] {
            cells.push([x, y + dy, z]);
        }
        if cells.iter().filter_map(|c| blocks_by_cell.get(c)).flatten().any(ok) {
            return true;
        }
        if is_water_face(b) && !overlay_pos.is_empty() {
            let p = crate::tiny::block_pos(b);
            let r2 = near_r * near_r;
            return overlay_pos.iter().chain(rename_pos.iter()).chain(keep_pos.iter()).any(|q| {
                let (dx, dy, dz) = (p[0] - q[0], p[1] - q[1], p[2] - q[2]);
                dx * dx + dy * dy + dz * dz <= r2
            });
        }
        false
    };
    // The WATER faces the game generates per configuration (`PlatformWaterFCSmall`,
    // `PlatformWaterHFCInsideNoOut`, `DecoWallWaterFCBInside`, …) ARE the floor the player
    // sees and drives — a water block's own prefab has none, and its shape depends on what
    // it connects to, so nothing can be generated to fit (Everios96 2026-10-07: "water
    // blocks change their geometry when connected to other blocks… the dirt layer is
    // placed as a flat geometry, so it doesnt fit"). They re-dress exactly like a wall's
    // faces. `TINY_WATER_FACES=0` leaves them to the game.
    let is_frame_filler = |b: &crate::map::BlockRec| -> bool {
        if water_faces && b.name.contains("Water") && b.name.contains("FC") && !b.name.contains("Pillar") && !b.name.contains("VFC") {
            return wall_owner_baked(b);
        }
        if wall_fillers && (b.name.starts_with("TrackWall") || b.name.starts_with("DecoWall") || b.name.starts_with("Platform")) && b.name.contains("FC") && !b.name.contains("Water") && !b.name.contains("Pillar") {
            return wall_owner_baked(b);
        }
        if b.name.contains("PenaltyHFC") {
            let (x, y, z) = b.coords();
            return frame_cells.iter().any(|c| (c[1] - y).abs() <= 1 && (c[0] - x).abs() <= 1 && (c[2] - z).abs() <= 1);
        }
        // (a WATER pad stays a block — its clips stay the game's records)
        if b.name.contains("Special") && b.name.contains("FC") && !b.name.contains("Water") {
            return skirt_owner(b).map(|i| baked_indices.contains(&i)).unwrap_or(false);
        }
        false
    };
    let mut filler_specs: Vec<ItemSpec> = Vec::new();
    let mut filler_placed: BTreeSet<usize> = BTreeSet::new();
    let mut filler_unmapped = 0usize;
    if let Some(mp) = &mapping {
        for b in source.baked.iter().filter(|b| is_frame_filler(b)) {
            match mp.baked_by_index.get(&b.index).filter(|m| m.model != "-" && m.model.ends_with(".Item.Gbx")) {
                Some(map) => {
                    let rot = b.free_rot.unwrap_or([crate::tiny::block_yaw(b), 0.0, 0.0]);
                    let pos = match map.footprint {
                        Some(fp) => crate::tiny::block_origin(b, fp),
                        None => crate::tiny::block_pos(b),
                    };
                    filler_specs.push(ItemSpec { model: map.model.clone(), pos, rot, scale: 1.0 / map.model_scale, tag: None, color: 0 });
                    filler_placed.insert(b.index);
                }
                None => filler_unmapped += 1,
            }
        }
    }
    if !frame_cells.is_empty() || !pad_cells.is_empty() || wall_fillers {
        println!("  penalty frames: {} → items, pads: {} → items; {} owned filler records baked as items, {} without a library row", frame_cells.len(), pad_cells.len(), filler_specs.len(), filler_unmapped);
    }
    // the fillers ride with the authored bakes (index usize::MAX: no authored block behind them)
    let mut item_specs = item_specs;
    item_specs.extend(filler_specs.into_iter().map(|sp| (usize::MAX, sp)));
    let deletions: BTreeSet<usize> = plan.iter().filter(|r| r.delete).map(|r| r.index).chain(baked_indices.iter().copied()).chain(face_only.iter().copied()).collect();
    let renames: Vec<&PlanRec> = plan.iter().filter(|r| r.action == "rename" && !r.delete).collect();
    let moves: Vec<&PlanRec> = renames.iter().copied().filter(|r| if r.free { r.new_pos.is_some() } else { r.new_cell != r.cell }).collect();
    if !overlay_indices.is_empty() {
        println!("  water overlay: {} blocks kept with a visual-only item beside them", overlay_indices.len());
    }
    println!("surface: {} authored blocks; plan: {} renames ({} moved a cell), {} baked as items, {} deletions, {} left as they are", source.blocks.len(), renames.len(), moves.len(), item_specs.len(), deletions.len().saturating_sub(baked_indices.len()), source.blocks.len() - renames.len() - deletions.len());
    let old_uid = source.body_ids.first().and_then(|f| f.name.clone()).expect("map uid");
    assert!(old_uid.len() > 4, "source uid `{old_uid}` too short");
    let new_uid = format!("{uid_prefix}{}", &old_uid[4..]);
    let tmp1 = out.with_extension("s1.Map.Gbx");
    let tmp2 = out.with_extension("s2.Map.Gbx");
    let tmp3 = out.with_extension("s3.Map.Gbx");

    // 1. deletions — with the generated `<Road>PenaltyHFC*` filler records of every penalty
    // frame that becomes an item: the game draws the frame's whole floor from them (road
    // deck at the ends, penalty surface between the posts; the vanilla prefab is kerbs and
    // posts only), and under the item's own slabs that plate z-fights and is what the wheels
    // touch (wet wood 37, 2026-09-24). The fillers sit in the frame's cell and its four
    // neighbours (the "Ends" pieces one cell along the road, the side piece one cell across).
    let mut m = MapFile::load(&src);
    // A water face goes ONLY when its re-dressed replacement was actually placed: removing
    // one without a model leaves a hole in the floor (the invisible faces of 2026-10-05).
    // `TINY_WATER_FACE_DELETE=0` keeps the game's water face clips under their re-dressed
    // floor copies (tried 2026-10-10 for the missing pool walls of wet wood 55 — no visible
    // difference at the spots checked; the default stays "delete", which the 10-07 double
    // draw argued for).
    let water_face_delete = std::env::var("TINY_WATER_FACE_DELETE").map(|v| v != "0").unwrap_or(true);
    let drop_baked = |b: &crate::map::BlockRec| -> bool {
        if is_water_face(b) {
            return water_face_delete && is_frame_filler(b) && filler_placed.contains(&b.index);
        }
        is_frame_filler(b)
    };
    let water_left = source.baked.iter().filter(|b| is_water_face(b) && is_frame_filler(b) && !filler_placed.contains(&b.index)).count();
    if water_left > 0 {
        println!("  ⛔ {water_left} water floor record(s) without a re-dressed model: the game's floor shows there");
    }
    if !water_face_delete {
        let kept = source.baked.iter().filter(|b| is_water_face(b) && is_frame_filler(b) && filler_placed.contains(&b.index)).count();
        if kept > 0 {
            println!("  water faces: {kept} game clip(s) kept under their re-dressed floor copies (sides and water stay the game's)");
        }
    }
    // …and the stage RUNS whenever anything goes, not only when an authored block does: a
    // pure-water map deletes no block at all, so its floors were never removed and the game
    // drew them over ours (960/734/10/72/748, 2026-10-07).
    if !deletions.is_empty() || source.baked.iter().any(|b| drop_baked(b)) {
        let r = m.remove_blocks(|b| deletions.contains(&b.index), |b| drop_baked(b));
        println!("  stage 1: deleted {} blocks ({} free entries, {} snap groups), {} generated filler records", r.blocks, r.free_entries, r.snap_groups, r.baked);
    }
    m.write_to(&tmp1).expect("write stage 1");
    // old index -> new index
    let mut new_index: BTreeMap<usize, usize> = BTreeMap::new();
    let mut k = 0usize;
    for i in 0..source.blocks.len() {
        if deletions.contains(&i) {
            continue;
        }
        new_index.insert(i, k);
        k += 1;
    }

    // 2. renames (blocks, and the vegetation items' icy stand-ins)
    let mut m = MapFile::load(&tmp1);
    assert_eq!(m.blocks.len(), k, "stage 1 left {} blocks, expected {k}", m.blocks.len());
    let mut by_target: BTreeMap<String, usize> = BTreeMap::new();
    for r in &renames {
        let ni = new_index[&r.index];
        assert_eq!(m.blocks[ni].name, r.name, "renumbering: #{} -> #{ni} is `{}`", r.index, m.blocks[ni].name);
        m.set_block_name(ni, &r.target);
        *by_target.entry(format!("{} -> {}", r.name, r.target)).or_default() += 1;
    }
    // --keep-trees leaves the vegetation; by default every tree item becomes
    // its winter stand-in (`icy_tree`: same size, frozen or snow-fir), the
    // variant byte re-pointed into the stand-in's own species list
    let mut trees: BTreeMap<String, usize> = BTreeMap::new();
    let mut trees_kept: BTreeMap<String, usize> = BTreeMap::new();
    if !cli::has(args, "--keep-trees") {
        let items: Vec<(usize, String, u8)> = m.items.iter().map(|it| (it.index, it.model.clone(), it.variant())).collect();
        for (i, model, variant) in items {
            match icy_tree(&model, variant) {
                Some((to, v)) => {
                    m.set_item_model(i, to);
                    if v != variant {
                        m.set_item_variant(i, v);
                    }
                    *trees.entry(format!("{model}:{variant} -> {to}:{v}")).or_default() += 1;
                }
                None if model.starts_with("Summer") || model.starts_with("Spring") || model.starts_with("Fall") || model.starts_with("Winter") || model.contains("Tree") || model.starts_with("Fir") || model.starts_with("Cypress") || model.starts_with("Palm") => {
                    *trees_kept.entry(format!("{model}:{variant}")).or_default() += 1;
                }
                None => {}
            }
        }
    }
    // the ITEMS the swap dressed (the pipes: `i@INDEX<TAB>ALIAS` mapping rows
    // from tiny-library): the record points at the embedded re-dressed copy,
    // built for one variant
    let mut repointed: BTreeMap<String, usize> = BTreeMap::new();
    let mut repointed_aliases: BTreeSet<String> = BTreeSet::new();
    if let Some(mp) = &mapping {
        let items: Vec<(usize, String, u8)> = m.items.iter().map(|it| (it.index, it.model.clone(), it.variant())).collect();
        for (i, model, variant) in items {
            if let Some(row) = mp.items_by_index.get(&i) {
                if row.model != "-" && row.model != model {
                    m.set_item_model(i, &row.model);
                    if variant != 0 {
                        m.set_item_variant(i, 0);
                    }
                    *repointed.entry(format!("{model} -> {}", row.model)).or_default() += 1;
                    repointed_aliases.insert(row.model.clone());
                }
            }
        }
    }
    m.write_to(&tmp2).expect("write stage 2");
    for (k, n) in &by_target {
        println!("  {n:5}  {k}");
    }
    if !repointed.is_empty() {
        println!("  items re-dressed: {} placements ({})", repointed.values().sum::<usize>(), repointed.iter().map(|(k, v)| format!("{k} ×{v}")).collect::<Vec<_>>().join(", "));
    }
    if !trees.is_empty() || !trees_kept.is_empty() {
        let n: usize = trees.values().sum();
        let kept: usize = trees_kept.values().sum();
        println!("  trees: {n} placements -> icy ({}); {kept} kept as they are ({})", trees.iter().map(|(k, v)| format!("{k} ×{v}")).collect::<Vec<_>>().join(", "), trees_kept.iter().map(|(k, v)| format!("{k} ×{v}")).collect::<Vec<_>>().join(", "));
    }

    // 3. cell / position moves + uid
    let mut m = MapFile::load(&tmp2);
    for r in &renames {
        let ni = new_index[&r.index];
        assert_eq!(m.blocks[ni].name, r.target, "stage 2: #{ni} is `{}`, wanted `{}`", m.blocks[ni].name, r.target);
    }
    let mut moved = 0usize;
    for r in &moves {
        let ni = new_index[&r.index];
        if r.free {
            if let Some(p) = r.new_pos {
                m.move_block_free(ni, p);
                moved += 1;
            }
        } else {
            m.move_block_cell(ni, (r.new_cell[0], r.new_cell[1], r.new_cell[2]));
            moved += 1;
        }
    }
    // an additional variant the twin lacks -> the twin's base variant (the special pads)
    let mut reflagged = 0usize;
    for r in &renames {
        if let Some(nf) = r.new_flags {
            let ni = new_index[&r.index];
            m.set_block_flags(ni, nf);
            reflagged += 1;
        }
    }
    m.set_map_uid(&new_uid);
    m.write_to(&tmp3).expect("write stage 3");
    println!("  stage 3: {moved} records moved, {reflagged} re-flagged to the twin's base variant; uid {old_uid} -> {new_uid}");

    // 3b. items: the baked blocks' item placements appended (clones of an
    // existing record, then model / author / collection / position / frame /
    // scale / colour in one write, the variable-length waypoint nodes in the
    // next, the archive + manifest in the last — the `tmmaps tiny` stages)
    if !item_specs.is_empty() || !repointed_aliases.is_empty() {
        let library = library.as_ref().expect("--mapping without --library");
        // a source with NO items (U10S_113) gets one template record first — the
        // clone donor (`seed_item_record`, the tiny writer's way); it is parked far
        // below the map like a dropped tiny item
        if MapFile::load(&tmp3).items.is_empty() {
            let g = crate::gbx::Gbx::parse(&std::fs::read(&tmp3).expect("read stage 3"));
            let body = crate::map::seed_item_record(&g.body, collection).expect("seed an item record into a 0-item map");
            std::fs::write(&tmp3, g.write_body_recompressed(&body)).expect("write seeded stage");
            let mut m = MapFile::load(&tmp3);
            m.move_item(0, [8.0, -900.0, 8.0], 0.0, crate::tiny::cell_for([8.0, -900.0, 8.0]));
            m.write_to(&tmp3).expect("write seeded stage");
            println!("  0 items in the source: one template item record seeded (the clone donor), parked at (8,-900,8)");
        }
        let mut m = MapFile::load(&tmp3);
        let first = m.items.len();
        m.append_item_clones(first + item_specs.len());
        m.write_to(&tmp3).expect("write item-slot stage");
        let mut m = MapFile::load(&tmp3);
        assert_eq!(m.items.len(), first + item_specs.len(), "item clones did not come back");
        for (k, (_, s)) in item_specs.iter().enumerate() {
            let i = first + k;
            m.set_item_model(i, &s.model);
            m.set_item_author(i, &item_author(&s.model));
            m.set_item_collection(i, collection);
            m.move_item(i, s.pos, s.rot[0], crate::tiny::cell_for(s.pos));
            m.set_item_frame(i, s.rot, [0.0, 0.0, 0.0]);
            m.set_item_scale(i, s.scale);
            m.clear_item_variant(i);
            m.set_item_color(i, s.color);
        }
        m.write_to(&tmp3).expect("write item stage");
        let mut m = MapFile::load(&tmp3);
        assert_eq!(m.items.len(), first + item_specs.len(), "item clones did not come back");
        for (k, (_, s)) in item_specs.iter().enumerate() {
            m.set_item_waypoint(first + k, s.tag.as_deref(), 0);
        }
        m.write_to(&tmp3).expect("write item waypoint stage");
        let mut m = MapFile::load(&tmp3);
        // the archive: the library's item files the placements name (+ the
        // non-item support files), one manifest row per model
        let zip = std::fs::read(library).unwrap_or_else(|e| panic!("{}: {e}", library.display()));
        assert!(zip.starts_with(b"PK\x03\x04"), "{} is not a ZIP archive", library.display());
        let mut names: Vec<String> = item_specs.iter().map(|(_, s)| s.model.clone()).chain(repointed_aliases.iter().cloned()).collect();
        names.sort();
        names.dedup();
        let listed: BTreeSet<&str> = names.iter().map(|s| s.as_str()).collect();
        let entries = crate::header::zip_entries(&zip);
        let before = entries.len();
        let mut kept: BTreeMap<String, Vec<u8>> = entries.into_iter().filter(|(name, _)| !name.ends_with(".Item.Gbx") || listed.contains(name.trim_start_matches("Items/"))).collect();
        for n in &names {
            assert!(kept.contains_key(&format!("Items/{n}")), "{}: the library has no Items/{n}", library.display());
        }
        // The SOURCE's own embedded objects ride along: the author's custom blocks
        // and items (`Blocks/WoodRally/…/RallyRoadWoodHighCurve1.Block.Gbx`) that
        // the kept records still name. Dropped until 2026-09-14 — every alteration
        // of a map with custom objects failed to load (plastic 630/640/641).
        let mut manifest_rows: Vec<(String, String)> = names.iter().map(|n| (n.clone(), item_author(n))).collect();
        let mut carried = 0usize;
        if let Some((rows, src_zip)) = source.embedded_objects() {
            for (name, bytes) in crate::header::zip_entries(&src_zip) {
                if !kept.contains_key(&name) {
                    kept.insert(name, bytes);
                    carried += 1;
                }
            }
            for (ident, author) in rows {
                if !manifest_rows.iter().any(|(i, _)| *i == ident) {
                    manifest_rows.push((ident, author));
                }
            }
        }
        if carried > 0 {
            println!("  archive: {carried} of the source's own embedded files carried over (custom blocks/items)");
        }
        let zip = crate::header::deflated_zip(&kept);
        let manifest: Vec<(&str, &str)> = manifest_rows.iter().map(|(n, a)| (n.as_str(), a.as_str())).collect();
        m.replace_embedded_objects(&manifest, &zip);
        m.write_to(&tmp3).expect("write archive stage");
        let mut tally: BTreeMap<String, usize> = BTreeMap::new();
        for (i, sp) in &item_specs {
            *tally.entry(source.blocks.get(*i).map(|b| b.name.clone()).unwrap_or_else(|| format!("filler {}", sp.model))).or_default() += 1;
        }
        println!("  stage 3b: {} blocks became items ({} models, archive {} of {} entries, {} bytes): {}", item_specs.len(), names.len(), kept.len(), before, zip.len(), tally.iter().map(|(k, v)| format!("{k}×{v}")).collect::<Vec<_>>().join(" "));
    }

    // 3c. the WET pad: a free `PlatformWaterRampBase` under every START block the
    // plan marked `wetpad deck=<h>` — the block's own cell and quarter turn, its
    // origin 1.6 m under the start's driving deck, so the shallow water (a 32×2×32
    // volume, sheet at +2.0) stands 0.4 m over the deck the car spawns on: wet
    // tyres within 1.5 s, for the whole run (TM2020 tyres do not dry). Flags
    // 0x200000 (air, additional variant 0: the `NoOut` edge clips) are the
    // record the Altered Nadeo "Pure Wet Icy Wood" maps carry under their spawn.
    let mut wet_pads: Vec<crate::map::FreeBlockSpec> = Vec::new();
    for r in plan.iter().filter(|r| r.detail.contains("wetpad deck=")) {
        let b = &source.blocks[r.index];
        let deck: f32 = r.detail.split("wetpad deck=").nth(1).and_then(|t| t.split(|c: char| c == ';' || c.is_whitespace()).next()).and_then(|v| v.parse().ok()).unwrap_or(2.0);
        let origin = crate::tiny::block_origin(b, (1, 1));
        let yaw = crate::tiny::block_yaw(b);
        let pos = [origin[0], origin[1] + deck - 1.6, origin[2]];
        println!("  stage 3c: wet pad under #{} {} — PlatformWaterRampBase (free) at ({:.1}, {:.2}, {:.1}) yaw {:.3}, deck {deck} (water 0.4 m over it)", r.index, r.name, pos[0], pos[1], pos[2], yaw);
        wet_pads.push(crate::map::FreeBlockSpec { name: "PlatformWaterRampBase".into(), author: None, flags: 0x0020_0000, pos, rot: [yaw, 0.0, 0.0], grid: None, dir: 0 });
    }
    // a wet plan whose start is an ITEM (a start gate item, a custom start: no START
    // block row to mark) gets the pad under the Spawn item: the item's own cell,
    // its origin 1.6 m under the item's floor, its yaw — unless the start is a water block
    if cli::has(args, "--wet") && wet_pads.is_empty() && !plan.iter().any(|r| r.detail.contains("wet already")) {
        for it in source.items.iter().filter(|it| it.waypoint_tag.as_deref() == Some("Spawn")) {
            let cell = |v: f32| (v / crate::map::CELL_XZ).floor() * crate::map::CELL_XZ;
            let pos = [cell(it.pos[0]), it.pos[1] - 1.6, cell(it.pos[2])];
            println!("  stage 3c: wet pad under the start ITEM {} at ({:.1}, {:.2}, {:.1}) — PlatformWaterRampBase (free) at ({:.1}, {:.2}, {:.1}), unturned (a quarter turn would shift the free block out of the cell)", it.model, it.pos[0], it.pos[1], it.pos[2], pos[0], pos[1], pos[2]);
            wet_pads.push(crate::map::FreeBlockSpec { name: "PlatformWaterRampBase".into(), author: None, flags: 0x0020_0000, pos, rot: [0.0, 0.0, 0.0], grid: None, dir: 0 });
        }
        if wet_pads.is_empty() {
            println!("  stage 3c: WET: no start block and no Spawn item found — the map stays dry");
        }
    }
    if !wet_pads.is_empty() {
        let mut m = MapFile::load(&tmp3);
        let r = m.remove_and_add_blocks(|_| false, |_| false, &wet_pads);
        m.write_to(&tmp3).expect("write wet pad stage");
        println!("  stage 3c: {} free water block(s) added (Id table {} -> {})", wet_pads.len(), r.table_before, r.table_after);
    }

    // 4. password, name, ghost, lightmap
    let mut m = MapFile::load(&tmp3);
    m.remove_password();
    let old_name = crate::header::read(&src.display().to_string()).ok().map(|h| h.name).unwrap_or_default();
    if let Some(name) = cli::flag(args, "--name") {
        if old_name.is_empty() || old_name == "-" {
            println!("  map name: the source declares none; --name ignored");
        } else {
            let (h, b) = m.set_map_name(&old_name, name);
            println!("  map name: {old_name:?} -> {name:?} ({h} in the header, {b} in the body)");
            m.write_to(&tmp3).expect("write name stage");
            m = MapFile::load(&tmp3);
        }
    }
    if !cli::has(args, "--keep-ghost") {
        let removed = m.strip_validation_ghost_to(crate::map::GhostForm::Remove);
        println!("  validation ghost: {}", if removed > 0 { format!("the source's ({removed} bytes) removed, header validated=\"0\"") } else { "none in the source".into() });
        if removed > 0 {
            m.write_to(&tmp3).expect("write ghost stage");
            m = MapFile::load(&tmp3);
        }
    }
    // The source's STORED lightmap indexes objects by their position in the block
    // list: once records are deleted (the baked blocks) every later entry lands
    // on the wrong block — Ice U10S_03 / 04 (vjeux, 2026-09-13: "the shadows and
    // colors are looking really weird"). With deletions the chunk goes (the game
    // computes a lightmap at load); a rename-only map keeps its bake (same mesh,
    // same uv). `--strip-lightmap` forces, `--keep-lightmap` keeps it regardless.
    let strip = cli::has(args, "--strip-lightmap") || ((!deletions.is_empty() || !wet_pads.is_empty()) && !cli::has(args, "--keep-lightmap"));
    if strip {
        let n = m.strip_lightmap();
        println!("  lightmap: {n} bytes stripped ({}; the game computes one at load)", if deletions.is_empty() && wet_pads.is_empty() { "--strip-lightmap" } else { "block records were deleted or added: the stored entries would shift" });
    }
    m.write_to(&out).expect("write output");
    for p in [&tmp1, &tmp2, &tmp3] {
        let _ = std::fs::remove_file(p);
    }

    // verify: reload, every planned record has its target name and cell
    let check = MapFile::load(&out);
    let mut bad = 0usize;
    for r in &renames {
        let ni = new_index[&r.index];
        let b = &check.blocks[ni];
        let c = b.coords();
        if b.name != r.target || (!r.free && [c.0, c.1, c.2] != r.new_cell) {
            bad += 1;
            eprintln!("  VERIFY #{} -> #{ni}: `{}` at {:?}, wanted `{}` at {:?}", r.index, b.name, c, r.target, r.new_cell);
        }
    }
    let hs = crate::header::read(&src.display().to_string()).ok();
    let ho = crate::header::read(&out.display().to_string()).ok();
    if let (Some(a), Some(b)) = (hs, ho) {
        assert_eq!((a.authortime.as_str(), a.gold.as_str(), a.silver.as_str(), a.bronze.as_str()), (b.authortime.as_str(), b.gold.as_str(), b.silver.as_str(), b.bronze.as_str()), "the medal times changed");
        println!("  times unchanged: author {} gold {} silver {} bronze {}; uid {}; name {:?}", b.authortime, b.gold, b.silver, b.bronze, b.uid, b.name);
    }
    assert_eq!(bad, 0, "{bad} planned records did not come out as planned");
    println!("wrote {} ({} blocks, {} baked, {} items, {} bytes)", out.display(), check.blocks.len(), check.baked.len(), check.items.len(), std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0));
}

/// One block turned into an item: what the placement needs.
struct ItemSpec {
    model: String,
    pos: [f32; 3],
    rot: [f32; 3],
    scale: f32,
    tag: Option<String>,
    color: u8,
}

/// The `bake` rows of the plan as item placements: the library's mapping
/// (`@index` / block-name rows from `mapgeom tiny-library --scale 1 --only …`)
/// names the item file each block became; the placement is the block's own
/// origin corner and quarter turn (a free block: its position and full
/// rotation), scale 1 in the item's frame, the block's waypoint tag and colour.
/// A `bake` row the library has no model for stays a block (reported).
fn item_specs(source: &MapFile, plan: &[PlanRec], mapping: &crate::tiny::Mappings, colors: &crate::map::Colors) -> (Vec<(usize, ItemSpec)>, Vec<String>) {
    let mut specs = Vec::new();
    let mut unbaked = Vec::new();
    for r in plan.iter().filter(|r| (r.action == "bake" || r.action == "overlay") && !r.delete) {
        let b = &source.blocks[r.index];
        let Some(map) = mapping.by_index.get(&r.index).or_else(|| mapping.by_name.get(&r.name)) else {
            unbaked.push(format!("#{} {} (no library row)", r.index, r.name));
            continue;
        };
        if map.model == "-" || !map.model.ends_with(".Item.Gbx") {
            unbaked.push(format!("#{} {} (library says {:?})", r.index, r.name, map.model));
            continue;
        }
        let rot = b.free_rot.unwrap_or([crate::tiny::block_yaw(b), 0.0, 0.0]);
        let pos = match map.footprint {
            Some(fp) => crate::tiny::block_origin(b, fp),
            None => crate::tiny::block_pos(b),
        };
        // an overlay item carries no waypoint (the kept block has it)
        let tag = if r.action == "overlay" { None } else { b.waypoint_tag.clone() };
        specs.push((r.index, ItemSpec { model: map.model.clone(), pos, rot, scale: 1.0 / map.model_scale, tag, color: colors.block(b.index) }));
    }
    (specs, unbaked)
}

// ------------------------------------------------------------------ trees

/// The icy form of a vegetation ITEM placement: `(model, variant)` of the
/// source placement → `(model, variant)` of the winter stand-in (vjeux,
/// 2026-09-13: "the trees should be changed to their icy version as well").
///
/// Nadeo's Stadium vegetation comes as cluster items whose VARIANT picks a
/// species (the variant list is the item's external model list, in order):
/// `Spring` 0 Medium 1 Big 2 VerySmall 3 Tall 4 Small 5 CypressTall;
/// `Fall` 0 Medium 1 Big 2 VerySmall 3 Small 4 Tall; `Summer` 0–4 the cacti
/// A–E (static objects) 5 FirMedium 6 FirTall 7 CypressDirtTall;
/// `SummerPalmTree` 0 CactusE 1 PalmTreeDirtMedium 2 PalmTreeDirtSmall;
/// `SpringPalmTree` 0 PalmTreeMedium 1 PalmTreeSmall; `SpringCherryTree`
/// 0 CherryTreeMedium 1 SpringTreeSmall 2 SpringTreeVerySmall — and the two
/// winter sets: `Winter` 0 FirSnowMedium 1 FirSnowTall, `WinterFrozenTree`
/// 0 Medium 1 Big 2 VerySmall 3 Tall 4 Small (frozen bare trees). Plus the
/// single-species items (`FallTreeTall`, `FrozenTreeTall`, `FirSnowTall`, …).
///
/// Rule: a deciduous tree becomes the frozen tree of the same size, a conifer
/// (fir, cypress) the snow fir of the same size, a palm the frozen tree of its
/// size (no icy palm exists). Cacti are not trees and have no winter form:
/// they stay (`None`). Already-winter models stay too.
pub fn icy_tree(model: &str, variant: u8) -> Option<(&'static str, u8)> {
    const WFT: &str = "WinterFrozenTree";
    const WIN: &str = "Winter";
    Some(match (model, variant) {
        // cluster items, per species
        ("Spring", 0) => (WFT, 0),
        ("Spring", 1) => (WFT, 1),
        ("Spring", 2) => (WFT, 2),
        ("Spring", 3) => (WFT, 3),
        ("Spring", 4) => (WFT, 4),
        ("Spring", 5) => (WIN, 1),
        ("Fall", 0) => (WFT, 0),
        ("Fall", 1) => (WFT, 1),
        ("Fall", 2) => (WFT, 2),
        ("Fall", 3) => (WFT, 4),
        ("Fall", 4) => (WFT, 3),
        ("Summer", 5) => (WIN, 0),
        ("Summer", 6) => (WIN, 1),
        ("Summer", 7) => (WIN, 1),
        ("SummerPalmTree", 1) => (WFT, 0),
        ("SummerPalmTree", 2) => (WFT, 4),
        ("SpringPalmTree", 0) => (WFT, 0),
        ("SpringPalmTree", 1) => (WFT, 4),
        ("SpringCherryTree", 0) => (WFT, 0),
        ("SpringCherryTree", 1) => (WFT, 4),
        ("SpringCherryTree", 2) => (WFT, 2),
        // the CLUSTER variants (a prefab grouping several trees: `<Item>.Prefab`, `B`, `C`
        // = the last three variants of every set) → the winter set's three clusters
        ("Spring", 6) | ("Fall", 5) | ("SpringPalmTree", 2) | ("SummerPalmTree", 3) | ("SpringCherryTree", 3) => (WFT, 5),
        ("Spring", 7) | ("Fall", 6) | ("SpringPalmTree", 3) | ("SummerPalmTree", 4) | ("SpringCherryTree", 4) => (WFT, 6),
        ("Spring", 8) | ("Fall", 7) | ("SpringPalmTree", 4) | ("SummerPalmTree", 5) | ("SpringCherryTree", 5) => (WFT, 7),
        ("Summer", 8) => (WIN, 2),
        ("Summer", 9) => (WIN, 3),
        ("Summer", 10) => (WIN, 4),
        // single-species items
        ("FallTreeTall", v) | ("SpringTreeTall", v) => ("FrozenTreeTall", v),
        ("FallTreeBig", v) | ("SpringTreeBig", v) => ("FrozenTreeBig", v),
        ("FallTreeMedium", v) | ("SpringTreeMedium", v) | ("CherryTreeMedium", v) | ("PalmTreeMedium", v) | ("PalmTreeDirtMedium", v) => ("FrozenTreeMedium", v),
        ("FallTreeSmall", v) | ("SpringTreeSmall", v) | ("PalmTreeSmall", v) | ("PalmTreeDirtSmall", v) => ("FrozenTreeSmall", v),
        ("FallTreeVerySmall", v) | ("SpringTreeVerySmall", v) => ("FrozenTreeVerySmall", v),
        ("FirMedium", v) => ("FirSnowMedium", v),
        ("FirTall", v) | ("CypressTall", v) | ("CypressDirtTall", v) => ("FirSnowTall", v),
        _ => return None,
    })
}

#[cfg(test)]
mod tree_tests {
    use super::icy_tree;
    #[test]
    fn sizes_line_up() {
        assert_eq!(icy_tree("Fall", 3), Some(("WinterFrozenTree", 4))); // Small
        assert_eq!(icy_tree("Fall", 4), Some(("WinterFrozenTree", 3))); // Tall
        assert_eq!(icy_tree("Summer", 6), Some(("Winter", 1)));
        assert_eq!(icy_tree("Summer", 2), None); // a cactus
        assert_eq!(icy_tree("Winter", 1), None);
        assert_eq!(icy_tree("FallTreeTall", 0), Some(("FrozenTreeTall", 0)));
        assert_eq!(icy_tree("SummerPalmTree", 3), Some(("WinterFrozenTree", 5))); // a palm cluster -> a frozen cluster
    }
}


/// `TINY_ITEM_AUTHOR`: the author id the generated items were written with (mapgeom
/// `tiny_library::item_author`); the placements and the manifest say the same.
pub fn item_author(model: &str) -> String {
    std::env::var("TINY_ITEM_AUTHOR").ok().filter(|a| !a.is_empty()).unwrap_or_else(|| model.to_string())
}
