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
    let plan = read_plan(&plan_path);
    let source = MapFile::load(&src);
    // the plan must describe THIS map: every row names the record it was read from
    for r in &plan {
        let b = source.blocks.get(r.index).unwrap_or_else(|| panic!("plan row #{} but the map has {} authored blocks", r.index, source.blocks.len()));
        assert_eq!(b.name, r.name, "plan row #{}: the map has `{}` there", r.index, b.name);
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
    let baked_indices: BTreeSet<usize> = item_specs.iter().map(|(i, _)| *i).collect();
    let deletions: BTreeSet<usize> = plan.iter().filter(|r| r.delete).map(|r| r.index).chain(baked_indices.iter().copied()).collect();
    let renames: Vec<&PlanRec> = plan.iter().filter(|r| r.action == "rename" && !r.delete).collect();
    let moves: Vec<&PlanRec> = renames.iter().copied().filter(|r| if r.free { r.new_pos.is_some() } else { r.new_cell != r.cell }).collect();
    println!("surface: {} authored blocks; plan: {} renames ({} moved a cell), {} baked as items, {} deletions, {} left as they are", source.blocks.len(), renames.len(), moves.len(), item_specs.len(), deletions.len() - item_specs.len(), source.blocks.len() - renames.len() - deletions.len());
    let old_uid = source.body_ids.first().and_then(|f| f.name.clone()).expect("map uid");
    assert!(old_uid.len() > 4, "source uid `{old_uid}` too short");
    let new_uid = format!("{uid_prefix}{}", &old_uid[4..]);
    let tmp1 = out.with_extension("s1.Map.Gbx");
    let tmp2 = out.with_extension("s2.Map.Gbx");
    let tmp3 = out.with_extension("s3.Map.Gbx");

    // 1. deletions
    let mut m = MapFile::load(&src);
    if !deletions.is_empty() {
        let r = m.remove_blocks(|b| deletions.contains(&b.index), |_| false);
        println!("  stage 1: deleted {} blocks ({} free entries, {} snap groups)", r.blocks, r.free_entries, r.snap_groups);
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
    m.write_to(&tmp2).expect("write stage 2");
    for (k, n) in &by_target {
        println!("  {n:5}  {k}");
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
    m.set_map_uid(&new_uid);
    m.write_to(&tmp3).expect("write stage 3");
    println!("  stage 3: {moved} records moved; uid {old_uid} -> {new_uid}");

    // 3b. items: the baked blocks' item placements appended (clones of an
    // existing record, then model / author / collection / position / frame /
    // scale / colour in one write, the variable-length waypoint nodes in the
    // next, the archive + manifest in the last — the `tmmaps tiny` stages)
    if !item_specs.is_empty() {
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
            m.set_item_author(i, &s.model);
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
        let mut names: Vec<String> = item_specs.iter().map(|(_, s)| s.model.clone()).collect();
        names.sort();
        names.dedup();
        let listed: BTreeSet<&str> = names.iter().map(|s| s.as_str()).collect();
        let entries = crate::header::zip_entries(&zip);
        let before = entries.len();
        let kept: BTreeMap<String, Vec<u8>> = entries.into_iter().filter(|(name, _)| !name.ends_with(".Item.Gbx") || listed.contains(name.trim_start_matches("Items/"))).collect();
        for n in &names {
            assert!(kept.contains_key(&format!("Items/{n}")), "{}: the library has no Items/{n}", library.display());
        }
        let zip = crate::header::deflated_zip(&kept);
        let manifest: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), n.as_str())).collect();
        m.replace_embedded_objects(&manifest, &zip);
        m.write_to(&tmp3).expect("write archive stage");
        let mut tally: BTreeMap<String, usize> = BTreeMap::new();
        for (i, _) in &item_specs {
            *tally.entry(source.blocks[*i].name.clone()).or_default() += 1;
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
        wet_pads.push(crate::map::FreeBlockSpec { name: "PlatformWaterRampBase".into(), author: None, flags: 0x0020_0000, pos, rot: [yaw, 0.0, 0.0], grid: None });
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
    for r in plan.iter().filter(|r| r.action == "bake" && !r.delete) {
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
        specs.push((r.index, ItemSpec { model: map.model.clone(), pos, rot, scale: 1.0 / map.model_scale, tag: b.waypoint_tag.clone(), color: colors.block(b.index) }));
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
