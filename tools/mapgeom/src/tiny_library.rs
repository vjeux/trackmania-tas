//! Build the complete tiny library of a map through the STATIC-ITEM path (the
//! item kind Granady's tiny blocks are). Driven by the game's own block infos:
//! for every authored block (name + flags) the picked variant gives the
//! prefab(s), the unit footprint, the waypoint kind and the spawn point; each
//! becomes a half-scale `CPlugStaticObjectModel` item with the stage-1
//! mechanisms (`static_item/bake.rs`; recipe env vars apply), under the map's
//! own collection. Every item model of the map is baked from its pack file
//! (external prefab / static object) or, when it is procedural vegetation,
//! re-pointed at a smaller stock species (`--veget substitute|keep|drop`).
//! Legacy prefab-less blocks come from the converted Nadeo item archive
//! (`--legacy-zip`). A variant with no geometry (an intentionally empty
//! pillar) maps to `-`: no item, on purpose.
//!
//! Writes the library zip, the `tmmaps tiny` mapping, and a report of every
//! model with its outcome, so a gap is explicit, never silent.
//!
//! Usage: mapgeom tiny-library MAP.Map.Gbx --library-out ITEMS.zip --mapping-out placements.tsv
//!        [--report REPORT.tsv] [--scale 0.5] [--legacy-zip Nadeo.zip] [--items-dir DIR]
//!        [--veget substitute|keep|drop] [--collection BlueBay] [--only NAME[,NAME]]
//! Needs the client packs (`--pak FILE:KEY` for BlueBay.pak and the Stadium pak).

use crate::static_item::surface::{CPlugSurface, Triangle};
use crate::store::DataStore;
use std::collections::BTreeMap;
use std::path::Path;
use tmmaps::map::MapFile;

pub struct Outcome {
    pub alias: String,
    pub kind: &'static str,
    pub source: String,
    pub placements: usize,
    pub result: Result<String, String>,
}

/// The report's word on an item's detail ladder: `", 3 lod levels [32, 64]"`
/// (switch distances in metres, already scaled), nothing for a one-level item.
fn lod_summary(m: &crate::static_item::build::Merged) -> String {
    if m.lod_max_dist.is_empty() || crate::static_item::build::lod0_only() {
        return String::new();
    }
    format!(", {} lod levels [{}]", m.lod_max_dist.len() + 1, m.lod_max_dist.iter().map(|d| format!("{d:.0}")).collect::<Vec<_>>().join(", "))
}

/// Block models whose picked variant has neither a prefab nor a solid draw
/// NOTHING of their own: the DecoWall / Platform family is rendered entirely
/// by the generated face-clip fillers (`DecoWallSlope2StraightFCT`,
/// `PlatformBaseFCB`, `DecoWallBaseVFC`... -- baked blocks, re-emitted here as
/// items). An archive item in their place drew a wall that is not there
/// (2026-09-06: brown TrackWall slabs under the stands). Optional overrides
/// from a converted-item archive, keyed by block name, for a model that
/// really has legacy CPlugSolid geometry:
const LEGACY: &[(&str, &str)] = &[];

/// Smaller stock species for the procedural vegetation the map places (the
/// game ignores placement scale for VegetTreeModel items, measured
/// 2026-09-06: PalmTreeBigB3 at 1.0/0.5/0.25 rendered one size).
fn veget_substitute(collection: u32, model: &str) -> Option<&'static str> {
    // WhiteShore (0x1d) ships its own ladder of species (Summer 03: 244
    // Forest + 351 Grove procedural patches, 34 TreeFirBig, 28 Medium,
    // 6 Small, 18 BushMediumA, 15 BushSmallB, 1 TreePineDeadMediumA):
    // firs come in Big / Medium / Small / VerySmall, bushes in Bush /
    // MediumA / SmallA / SmallB, dead pines in Big / Medium / Small /
    // VerySmall — no Plant* or Palm* here, so the generic ladder below
    // would point at species the pack does not have.
    // GreenCoast (0xf) species (Summer 04: 2821 Forest + 1472 Grove + 1249
    // Ecotone patches, no individual trees): Tree/TreeThin/TreeBushy in
    // Big/Medium/Small, Bush in Big/Medium/Small, Flower*. A Forest patch
    // becomes one thin small tree (the original forest is tall thin trunks
    // with light canopies), a Grove one small bushy tree, an Ecotone (the
    // forest edge) one medium bush.
    // Stadium (0x1a) "Japan" set (Summer 05: 384 `Spring` clusters, 69
    // `SpringCherryTree`, 18 CypressTall): SpringTree in Big / Tall / Medium /
    // Small / VerySmall, CherryTreeMedium, CypressTall, SpringPalmTree.
    // Palms (Summer 15: 1593 `SummerPalmTree` = CactusE static object /
    // PalmTreeDirtMedium / PalmTreeDirtSmall per variant, 14 `SpringPalmTree`
    // = PalmTreeMedium / PalmTreeSmall): the pack has Medium and Small of
    // each family and nothing smaller, so Medium -> Small and Small stays.
    if collection == 0x1a {
        return Some(match model {
            "SpringTreeBig" | "SpringTreeTall" | "SpringTreeMedium" => "SpringTreeSmall",
            "SpringTreeSmall" => "SpringTreeVerySmall",
            "Spring" | "SpringCherryTree" => "CherryTreeMedium",
            "PalmTreeMedium" => "PalmTreeSmall",
            "PalmTreeDirtMedium" => "PalmTreeDirtSmall",
            _ => return None,
        });
    }
    if collection == 0xf {
        return Some(match model {
            "Forest" => "TreeThinSmallA",
            "Grove" => "TreeBushySmallA",
            "Ecotone" => "BushMediumA",
            m if m.starts_with("TreeThinBushyBig") => "TreeThinBushySmallA",
            m if m.starts_with("TreeThinBig") || m.starts_with("TreeThinMedium") => "TreeThinSmallA",
            m if m.starts_with("TreeBushyMedium") => "TreeBushySmallA",
            m if m.starts_with("TreeBig") || m.starts_with("TreeMedium") => "TreeSmallA",
            m if m.starts_with("BushBig") => "BushMediumA",
            m if m.starts_with("BushMedium") => "BushSmallA",
            _ => return None,
        });
    }
    if collection == 0x1d {
        return Some(match model {
            "Forest" | "Grove" => "TreeFirSmall",
            m if m.starts_with("TreeFirBig") || m.starts_with("TreeFirMedium") => "TreeFirSmall",
            m if m.starts_with("TreeFirSmall") => "TreeFirVerySmall",
            m if m.starts_with("TreePineDeadBig") || m.starts_with("TreePineDeadMedium") => "TreePineDeadSmallA",
            m if m.starts_with("TreePineDeadSmall") => "TreePineDeadVerySmallA",
            "Bush" => "BushMediumA",
            m if m.starts_with("BushMedium") => "BushSmallA",
            _ => return None,
        });
    }
    Some(match model {
        "PalmForest" | "PalmGrove" | "PalmEcotone" => "PalmTreeSmallA",
        m if m.starts_with("PalmTreeBig") => "PalmTreeSmallB",
        m if m.starts_with("PalmTreeSugarBig") => "PalmTreeSugarSmallA",
        m if m.starts_with("PalmTreeSugarMedium") => "PalmTreeSugarSmallB",
        "TreeMediumA" => "BushBigA",
        // RedIsland pines (Summer 02: 113 TreePineBig, 49 TreePine, 13 Small)
        m if m.starts_with("TreePineBig") => "TreePineMedium",
        "TreePine" | "TreePineMedium" => "TreePineSmall",
        m if m.starts_with("TreePineMedium") => "TreePineSmall",
        "Bush" => "BushMedium",
        m if m.starts_with("TreePineSmall") => return None,
        m if m.starts_with("BushBig") => "BushMediumA",
        m if m.starts_with("BushMedium") => "BushSmallA",
        m if m.starts_with("BushSmall") => "PlantSmallA",
        m if m.starts_with("PlantSmall") => "PlantSmallB",
        m if m.starts_with("PalmTreeSmall") || m.starts_with("PalmTreeSugarSmall") => return None,
        _ => return None,
    })
}

/// Light-carrying items (Lamp*, Light*, the ShowLights rigs) are baked like
/// everything else since 2026-09-07: the bake embeds their `CPlugLight`s
/// (scaled) in the static item — `static_item::build::MergedLight`. Before
/// that a `light_substitute` ladder kept them as STOCK items one size down
/// (lit, but twice the relative size); it is gone.
fn find_item_file(store: &DataStore, model: &str) -> Option<String> {
    let want = format!("\\{}.ITEM.GBX", model.to_uppercase());
    let mut hits: Vec<String> = store.entries().map(|e| e.path()).filter(|p| p.to_uppercase().ends_with(&want)).collect();
    hits.sort_by_key(|p| (!p.to_uppercase().contains("\\ITEMS\\"), p.len()));
    hits.into_iter().next()
}

/// The stock item that is exactly the HALF of `model` — Nadeo's `Small`
/// screens (measured 2026-09-07 on the pack: every pair halves both width and
/// height; the frame depth stays 0.86 m on all of them). `TINY_STOCK_HALF=0`
/// turns the table off; `=gates` adds the gate families' half-WIDTH member
/// (same 11 m posts — twice the tiny height; the 24 m gates have no 12 m twin
/// and the `Special`/`Gameplay` 4 m ends have nothing below them).
pub fn stock_half_variant(model: &str) -> Option<&'static str> {
    let mode = std::env::var("TINY_STOCK_HALF").unwrap_or_default();
    if mode == "0" {
        return None;
    }
    const SCREENS: &[(&str, &str)] = &[
        ("RaceScreen6x1", "RaceScreen6x1Small"),
        ("Screen16x9", "Screen16x9Small"),
        ("Screen2x3Big", "Screen2x3"),
        ("Screen2x3", "Screen2x3Small"),
        ("Screen4x1", "Screen4x1Small"),
        ("Screen2x1", "Screen2x1Small"),
        ("Screen1x1", "Screen1x1Small"),
        ("Screen155", "Screen155Small"),
    ];
    if let Some((_, small)) = SCREENS.iter().find(|(big, _)| *big == model) {
        return Some(small);
    }
    if mode != "gates" {
        return None;
    }
    // gates: <Family><Size>m[<Suffix>] -> the same family one size down
    for (from, to) in [("32m", "16m"), ("16m", "8m"), ("8m", "4m")] {
        if let Some(i) = model.find(from) {
            let (head, tail) = (&model[..i], &model[i + from.len()..]);
            if head.starts_with("Gate") && head.chars().last().map(|c| !c.is_ascii_digit()).unwrap_or(false) {
                // 4 m exists only for the Special / Gameplay families
                if to == "4m" && !(head.starts_with("GateSpecial") || head.starts_with("GateGameplay")) {
                    return None;
                }
                let name = format!("{head}{to}{tail}");
                return GATE_NAMES.iter().find(|g| **g == name).copied();
            }
        }
    }
    None
}

/// Every gate item of the Stadium pack (the target of the gate ladder must be
/// a real file; `find_item_file` confirms it again against the store).
const GATE_NAMES: &[&str] = &[
    "GateCheckpointCenter16m", "GateCheckpointCenter16mv2", "GateCheckpointCenter8m", "GateCheckpointCenter8mv2",
    "GateCheckpointLeft16m", "GateCheckpointLeft8m", "GateCheckpointRight16m", "GateCheckpointRight8m",
    "GateFinish16m", "GateFinish8m", "GateFinishCenter16m", "GateFinishCenter16mv2", "GateFinishCenter8m", "GateFinishCenter8mv2",
    "GateMultilapCenter16m", "GateMultilapCenter8m", "GateMultilapLeft16m", "GateMultilapLeft8m", "GateMultilapRight16m", "GateMultilapRight8m",
    "GateStartCenter16m", "GateStartCenter8m", "GateStartLeft16m", "GateStartLeft8m", "GateStartRight16m", "GateStartRight8m",
    "GateSpecial16mBoost", "GateSpecial16mBoost2", "GateSpecial16mCruise", "GateSpecial16mFragile", "GateSpecial16mNoBrake", "GateSpecial16mNoEngine",
    "GateSpecial16mNoSteering", "GateSpecial16mReset", "GateSpecial16mSlowMotion", "GateSpecial16mTurbo", "GateSpecial16mTurbo2", "GateSpecial16mTurboRoulette",
    "GateSpecial8mBoost", "GateSpecial8mBoost2", "GateSpecial8mCruise", "GateSpecial8mFragile", "GateSpecial8mNoBrake", "GateSpecial8mNoEngine",
    "GateSpecial8mNoSteering", "GateSpecial8mReset", "GateSpecial8mSlowMotion", "GateSpecial8mTurbo", "GateSpecial8mTurbo2", "GateSpecial8mTurboRoulette",
    "GateSpecial4mBoost", "GateSpecial4mBoost2", "GateSpecial4mCruise", "GateSpecial4mFragile", "GateSpecial4mNoBrake", "GateSpecial4mNoEngine",
    "GateSpecial4mNoSteering", "GateSpecial4mReset", "GateSpecial4mSlowMotion", "GateSpecial4mTurbo", "GateSpecial4mTurbo2", "GateSpecial4mTurboRoulette",
    "GateGameplayDesert16m", "GateGameplayDesert8m", "GateGameplayDesert4m", "GateGameplayRally16m", "GateGameplayRally8m", "GateGameplayRally4m",
    "GateGameplaySnow16m", "GateGameplaySnow8m", "GateGameplaySnow4m", "GateGameplayStadium16m", "GateGameplayStadium8m", "GateGameplayStadium4m",
];

/// The stock vegetation item standing in for a prefab's `.VegetTreeModel.Gbx`
/// entity: `…\TreeBigA1.VegetTreeModel.Gbx` -> the pack item `TreeBigA` (an
/// item carries its A1/A2/A3 variants; `PalmTreeBigB3` -> `PalmTreeBigB`),
/// then the collection's smaller species for it. None when no item matches.
fn veget_item(store: &DataStore, collection: u32, model_path: &str, cache: &mut BTreeMap<String, Option<String>>) -> Option<String> {
    let file = model_path.rsplit('\\').next().unwrap_or(model_path);
    let low = file.to_ascii_lowercase();
    let stem = if low.ends_with(".vegettreemodel.gbx") { &file[..file.len() - ".vegettreemodel.gbx".len()] } else { file };
    if let Some(c) = cache.get(stem) {
        return c.clone();
    }
    let mut cands = vec![stem.to_string()];
    let no_digits = stem.trim_end_matches(|c: char| c.is_ascii_digit());
    if no_digits != stem {
        cands.push(no_digits.to_string());
    }
    let no_letter = no_digits.trim_end_matches(|c: char| c.is_ascii_uppercase());
    if no_letter != no_digits && !no_letter.is_empty() {
        cands.push(no_letter.to_string());
    }
    let found = cands.iter().find(|c| find_item_file(store, c).is_some()).cloned();
    // the smaller species must EXIST in this collection's packs: the generic
    // ladder's `BushSmall* -> PlantSmallA` is BlueBay's plant, and RedIsland
    // (FlowerSmall*/Grass* instead) has none — the editor still loaded Summer
    // 12 with 67 of them, play mode refused the stored map ("Missing Items:
    // PlantSmallA"). No smaller species in the packs: the species itself stays.
    let out = found.map(|item| match veget_substitute(collection, &item) {
        Some(sub) if find_item_file(store, sub).is_some() => sub.to_string(),
        _ => item,
    });
    cache.insert(stem.to_string(), out.clone());
    out
}

/// Closed box mesh over the block's units (each 32 x 8 x 32 m in block
/// space), scaled: the waypoint trigger, as the game triggers blocks on their
/// whole unit volume.
fn unit_box_trigger(units: &[[i32; 3]], scale: f32) -> CPlugSurface {
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut tris: Vec<Triangle> = Vec::new();
    for u in units {
        let (x0, y0, z0) = (u[0] as f32 * 32.0 * scale, u[1] as f32 * 8.0 * scale, u[2] as f32 * 32.0 * scale);
        let (x1, y1, z1) = (x0 + 32.0 * scale, y0 + 8.0 * scale, z0 + 32.0 * scale);
        let b = verts.len() as u32;
        verts.extend([[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1], [x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]]);
        let faces: [[u32; 3]; 12] = [[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
        for f in faces {
            tris.push(Triangle { indices: [b + f[0], b + f[1], b + f[2]], material_id: 0, u03: 0, surface_index: 0 });
        }
    }
    CPlugSurface::mesh(verts, tris, vec![0], [0.0, 0.0, 1.0])
}

#[allow(clippy::too_many_arguments)]
pub fn build(store: &mut DataStore, map: &Path, out_zip: &Path, out_mapping: &Path, report: Option<&Path>, scale: f32, legacy_zip: Option<&Path>, items_dir: Option<&Path>, veget_mode: &str, collection_name: &str, only: Option<&str>) {
    let source = MapFile::load(map);
    let collection = source.items.first().map(|it| it.collection_raw).unwrap_or(26);
    println!("  map collection {collection:#x}; {} blocks, {} items", source.blocks.len(), source.items.len());
    // block infos are looked up under the map's own collection first
    // (BlueBay\GameCtnBlockInfo\…\Stadium\X carries the terrain modifiers a
    // Stadium block gets on BlueBay; a Stadium map wants the plain files)
    let collection_name = if collection_name.is_empty() { crate::static_item::build::env_name(collection) } else { collection_name };
    let wanted = |name: &str| only.map(|o| o.split(',').any(|n| n == name)).unwrap_or(true);
    let legacy: BTreeMap<String, Vec<u8>> = match legacy_zip {
        Some(p) => crate::embedded::unzip(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).expect("legacy zip"),
        None => BTreeMap::new(),
    };
    let mut idx = crate::blockmap::BlockInfoIndex::build(store, collection_name);

    // A generated filler takes the MATERIAL MODIFIER of the authored block it
    // finishes: the game grows the FC clips of a PlatformDirt platform in the
    // neighbouring cells and dresses them like the platform (Summer 15: the
    // `OpenTechZoneFC*` skirts beside the dirt platforms and the fillers
    // under the `WaterWallDirt` pool walls are sand-orange in the original,
    // and came out plain blue from the unmodified block infos). Per baked
    // block: the authored block of its own cell decides (its modifier, or
    // none); a cell without an authored block asks its four horizontal
    // neighbours and takes the modifier they agree on most.
    let terrain_mods = |bi: &crate::blockinfo::BlockInfo| -> Vec<String> { bi.material_modifier.iter().filter(|r| r.ends_with(".TerrainModifier.Gbx")).cloned().collect() };
    let mut cell_mod: std::collections::HashMap<(u8, u8, u8), Vec<String>> = std::collections::HashMap::new();
    // (x, z) column -> [(y, is_pillar, mods)] for the pillar rule below
    let mut columns: std::collections::HashMap<(u8, u8), Vec<(u8, bool, Vec<String>)>> = std::collections::HashMap::new();
    for b in source.blocks.iter().filter(|b| b.flags & crate::blockmap::FLAG_FREE == 0) {
        let Some(path) = idx.path_for(&b.name) else { continue };
        let mods = match idx.load(store, &path) {
            Ok(bi) => terrain_mods(bi),
            Err(_) => Vec::new(),
        };
        let c = (b.raw_coords[0], b.raw_coords[1], b.raw_coords[2]);
        let pillar = b.flags & crate::blockmap::FLAG_PILLAR != 0;
        columns.entry((c.0, c.2)).or_default().push((c.1, pillar, mods.clone()));
        // several authored blocks in one cell (a pillar under a deck): a
        // modifier wins over none
        let e = cell_mod.entry(c).or_default();
        if e.is_empty() {
            *e = mods;
        }
    }
    // A generated PILLAR (flag 0x4000, usually with 0x8000 "skinnable") under
    // a block is dressed like the block it supports: the DecoWallBasePillar
    // stacks under Summer 15's dirt hill draw the sand cliff texture down to
    // the grass, while their block infos carry no modifier of their own. A
    // pillar cell without a modifier takes the modifier of the nearest
    // non-pillar authored block above it in its column.
    for ((x, z), col) in &columns {
        for (y, pillar, mods) in col {
            if !*pillar || !mods.is_empty() {
                continue;
            }
            let above = col.iter().filter(|(yy, p, m)| !*p && *yy > *y && !m.is_empty()).min_by_key(|(yy, _, _)| *yy);
            if let Some((_, _, m)) = above {
                let e = cell_mod.entry((*x, *y, *z)).or_default();
                if e.is_empty() {
                    *e = m.clone();
                }
            }
        }
    }
    let inherited_mod = |b: &tmmaps::map::BlockRec| -> Vec<String> {
        if b.flags & crate::blockmap::FLAG_FREE != 0 {
            return Vec::new();
        }
        let c = (b.raw_coords[0], b.raw_coords[1], b.raw_coords[2]);
        // the cell's own authored block, when it has a modifier
        if let Some(m) = cell_mod.get(&c).filter(|m| !m.is_empty()) {
            return m.clone();
        }
        // else the neighbours: the vertical pair first (a VFC filler sits at
        // the interface of two stacked blocks and is recorded in the cell of
        // the plain one — Summer 15's dirt hill faces), then the horizontal four
        let vote = |offsets: &[(i32, i32, i32)]| -> Vec<String> {
            let mut votes: BTreeMap<Vec<String>, usize> = BTreeMap::new();
            for (dx, dy, dz) in offsets {
                let (x, y, z) = (c.0 as i32 + dx, c.1 as i32 + dy, c.2 as i32 + dz);
                if !(0..=255).contains(&x) || !(0..=255).contains(&y) || !(0..=255).contains(&z) {
                    continue;
                }
                if let Some(m) = cell_mod.get(&(x as u8, y as u8, z as u8)) {
                    if !m.is_empty() {
                        *votes.entry(m.clone()).or_insert(0) += 1;
                    }
                }
            }
            votes.into_iter().max_by_key(|(_, n)| *n).map(|(m, _)| m).unwrap_or_default()
        };
        let v = vote(&[(0, 1, 0), (0, -1, 0)]);
        if !v.is_empty() {
            return v;
        }
        vote(&[(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)])
    };
    let mod_key = |mods: &[String]| -> String { mods.join("|") };

    // distinct (name, flags, inherited modifier) among authored grid blocks AND
    // the generated (baked) non-Sea blocks -- the FC clip fillers that finish
    // the authored structures; the Sea itself stays the full-size foundation
    let mut keys: BTreeMap<(String, u32, String), usize> = BTreeMap::new();
    // Free-placed blocks (flag 0x20000000) are keyed like the rest: their
    // variant bits are the same, `tmmaps tiny` places them from free_pos /
    // free_rot (Summer 11 has 87 of them; skipping them refused the map).
    for b in source.blocks.iter() {
        *keys.entry((b.name.clone(), b.flags, String::new())).or_insert(0) += 1;
    }
    let mut baked_key: BTreeMap<usize, String> = BTreeMap::new();
    for b in source.baked.iter().filter(|b| b.name != "Sea") {
        let mk = mod_key(&inherited_mod(b));
        baked_key.insert(b.index, mk.clone());
        *keys.entry((b.name.clone(), b.flags, mk)).or_insert(0) += 1;
    }
    let inherited_fillers = baked_key.values().filter(|k| !k.is_empty()).count();
    if inherited_fillers > 0 {
        println!("  {inherited_fillers} generated fillers inherit a material modifier from the authored block they finish");
    }
    // Cell rows per key: a shore tile whose every placement sits at the water
    // row gets its sea floor back at source depth (`restore_depth`).
    let mut rows_by_key: BTreeMap<(String, u32), std::collections::BTreeSet<u8>> = BTreeMap::new();
    for b in source.blocks.iter().chain(source.baked.iter()) {
        rows_by_key.entry((b.name.clone(), b.flags)).or_default().insert(b.raw_coords[1]);
    }
    // The water row and the surface's height above that row's floor, from the
    // collection's fixed plane: the zone prefabs put their water quad about
    // local +7 (BlueBay 7, RedIsland 7.5, WhiteShore 7, GreenCoast 7.2).
    let water = if matches!(collection, 0x1c | 0x10 | 0x1d | 0xf) {
        let ground_y = tmmaps::map::ground_y(collection);
        let plane = tmmaps::tiny::fixed_plane(collection);
        let row = ((plane - 7.0 - ground_y) / 8.0).round();
        Some((row as u8, plane - (ground_y + 8.0 * row)))
    } else {
        None
    };
    // TINY_DEEPEN=0 leaves the halved sea floor; TINY_DEPTH_KEEP=m (tiny
    // metres below the surface that keep the tile's scale, default 0: the
    // Beach apron is only 0.8..3 m deep in the source and the sea over it
    // reads as open sea from 3 m down, so every underwater vertex takes its
    // source depth)
    let deepen = std::env::var("TINY_DEEPEN").map(|v| v != "0").unwrap_or(true);
    let depth_keep: f32 = std::env::var("TINY_DEPTH_KEEP").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let mut deepened: Vec<String> = Vec::new();
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    // picture files (gate sign logos) the items name: added after the ident pass below (they are not GBX)
    let mut pictures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut outcomes: Vec<Outcome> = Vec::new();
    // key -> (alias or "-", footprint sx, sz)
    let mut block_map: BTreeMap<(String, u32, String), (String, u32, u32)> = BTreeMap::new();
    let mut alias_of_recipe: BTreeMap<String, String> = BTreeMap::new();
    let mut next_alias = 0usize;
    // `v@ALIAS` rows (the prefabs' vegetation as stock items) and the
    // VegetTreeModel stem -> stock item cache behind them.
    let mut veget_rows = String::new();
    let mut veget_cache: BTreeMap<String, Option<String>> = BTreeMap::new();
    let ambient = source.ambient_zone().unwrap_or_default();
    for ((name, flags, modk), n) in &keys {
        if !wanted(name) {
            continue;
        }
        let ground = flags & crate::blockmap::FLAG_GROUND != 0;
        let vindex = (flags & crate::blockmap::FLAG_VARIANT_MASK) as usize;
        let sub = ((flags >> crate::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
        let Some(path) = idx.path_for(name) else {
            if std::env::var_os("TINY_DEBUG_LOOKUP").is_some() {
                eprintln!("  lookup {name:?}: no path; index knows {} stems; store has {} entries", idx.stem_count(), store.entries().count());
            }
            outcomes.push(Outcome { alias: String::new(), kind: "block", source: format!("{name} {flags:08X}"), placements: *n, result: Err("no block info file with this name".into()) });
            continue;
        };
        let bi = match idx.load(store, &path) {
            Ok(b) => b.clone(),
            Err(e) => {
                outcomes.push(Outcome { alias: String::new(), kind: "block", source: format!("{name} {flags:08X}"), placements: *n, result: Err(format!("block info: {e}")) });
                continue;
            }
        };
        // Stadium's Grass floor is the one terrain the tiny map keeps FULL
        // size: `tmmaps tiny` leaves the Stadium genealogy in place, so the
        // game regenerates the floor under the tiny map (the reference
        // maps' foundation); a half-scale copy would only z-fight it.
        // RedIsland's Water is the lake the island sits in: `tmmaps tiny` fills
        // the genealogy with it, so the game regenerates it full size under
        // and around the tiny map — its surface (-0.5) is the tiny map's
        // fixed plane, so a half-scale copy would only z-fight it.
        // WhiteShore's Water is the sea around its island, the same way
        // (Summer 03: 3148 of 4096 cells; surface -1 = the fixed plane), and
        // GreenCoast's Lake (Summer 04: 2418 cells). The block is the map's
        // most common genealogy zone — what `tmmaps tiny` fills with.
        if matches!(collection, 0x10 | 0x1d | 0xf) && !ambient.is_empty() && *name == ambient {
            block_map.insert((name.clone(), *flags, modk.clone()), ("-".into(), 1, 1));
            outcomes.push(Outcome { alias: "-".into(), kind: "block", source: format!("{name} {flags:08X}"), placements: *n, result: Ok(format!("{} ambient {name}: regenerated full size by the genealogy, no item", crate::static_item::build::env_name(collection))) });
            continue;
        }
        if collection == 0x1a && name == "Grass" {
            block_map.insert((name.clone(), *flags, modk.clone()), ("-".into(), 1, 1));
            outcomes.push(Outcome { alias: "-".into(), kind: "block", source: format!("{name} {flags:08X}"), placements: *n, result: Ok("Stadium grass floor: regenerated full size by the genealogy, no item".into()) });
            continue;
        }
        // TINY_DROP_BLOCKS=sub,sub: block models whose name contains a substring
        // get no item (experiments: which generated fillers the game does not
        // draw — Summer 15's `*Inside*` water-wall fillers).
        if let Ok(list) = std::env::var("TINY_DROP_BLOCKS") {
            if list.split(',').any(|s| !s.is_empty() && name.contains(s)) {
                block_map.insert((name.clone(), *flags, modk.clone()), ("-".into(), 1, 1));
                outcomes.push(Outcome { alias: "-".into(), kind: "block", source: format!("{name} {flags:08X}"), placements: *n, result: Ok("dropped by TINY_DROP_BLOCKS".into()) });
                continue;
            }
        }
        let addv = ((flags >> crate::blockmap::FLAG_ADDITIONAL_SHIFT) & 0x7F) as usize;
        let Some(pk) = bi.pick_placement_add(ground, vindex, sub, addv) else {
            outcomes.push(Outcome { alias: String::new(), kind: "block", source: format!("{name} {flags:08X}"), placements: *n, result: Err("block info has no variant with units or mobils".into()) });
            continue;
        };
        let units: Vec<[i32; 3]> = pk.variant.block_units.iter().map(|u| u.offset).collect();
        let (sx, sz) = units.iter().fold((1u32, 1u32), |(sx, sz), u| (sx.max(u[0] as u32 + 1), sz.max(u[2] as u32 + 1)));
        let prefabs: Vec<(String, Option<[f32; 3]>, Option<[f32; 3]>)> = pk.mobils.iter().filter_map(|mb| mb.prefab.clone().map(|p| (p, mb.translation, mb.rotation))).collect();
        let solids: Vec<String> = pk.mobils.iter().filter_map(|mb| mb.solid.clone()).collect();
        let legacy_item = LEGACY.iter().find(|(n, _)| n == name).map(|(_, p)| *p);
        // recipe key: what gets baked (prefab set or legacy item) + waypoint
        // + the block's material modifier — PlatformGrass*/PlatformDirt*/
        // PlatformIce* share the PlatformTech prefabs and differ ONLY by the
        // modifier folder their materials are taken from (Summer 03: the tech
        // slopes next to the first checkpoint came out grass, keyed to the
        // PlatformGrassSlope2Straight item built first).
        // the block's own modifier plus the one a filler inherits from the
        // authored block it finishes (`modk`, see `inherited_mod`)
        let effective_mods: Vec<String> = bi.material_modifier.iter().cloned().chain(modk.split('|').filter(|s| !s.is_empty()).map(String::from)).collect();
        let recipe = if let Some(l) = legacy_item { format!("legacy:{l}") } else { format!("{}|wp{:?}|units{:?}|mod{:?}", prefabs.iter().map(|p| format!("{}@{:?}/{:?}", p.0, p.1, p.2)).collect::<Vec<_>>().join(","), bi.waypoint_type, units, effective_mods) };
        if prefabs.is_empty() && legacy_item.is_none() {
            if solids.is_empty() {
                // intentionally empty variant (e.g. the hidden pillar)
                block_map.insert((name.clone(), *flags, modk.clone()), ("-".into(), sx, sz));
                outcomes.push(Outcome { alias: "-".into(), kind: "block", source: format!("{name} {flags:08X} [{}]", pk.label), placements: *n, result: Ok("no geometry in this variant: intentionally no item".into()) });
            } else {
                outcomes.push(Outcome { alias: String::new(), kind: "block", source: format!("{name} {flags:08X} [{}] solids {:?}", pk.label, solids), placements: *n, result: Err("legacy CPlugSolid model without a converted archive item".into()) });
            }
            continue;
        }
        if let Some(alias) = alias_of_recipe.get(&recipe) {
            block_map.insert((name.clone(), *flags, modk.clone()), (alias.clone(), sx, sz));
            continue;
        }
        let alias = format!("AC{next_alias:08}");
        next_alias += 1;
        let ident = format!("{alias}.Item.Gbx");
        let res: Result<(Vec<u8>, crate::static_item::build::Merged), String> = if let Some(l) = legacy_item {
            match legacy.get(l) {
                Some(bytes) => crate::static_item::build::static_item_from_item_report(bytes, &ident, &ident, scale, collection),
                None => Err(format!("{l} absent from the legacy archive (pass --legacy-zip)")),
            }
        } else {
            let mut m = crate::static_item::build::Merged::default();
            m.editors = std::env::var_os("TINY_EDITORS").is_some();
            m.keep_water = crate::static_item::build::keep_water_for(collection);
            m.modifier = modifier_links(store, &effective_mods);
            // TINY_NO_SPLIT_FOR=name,name (default DecoBeachMangrove): models baked
            // without the per-layer split (the Mangrove split crashes the client;
            // minimal repro var-m1, open bug).
            let no_split_for = std::env::var("TINY_NO_SPLIT_FOR").unwrap_or_default();
            m.no_split = no_split_for.split(',').any(|s| !s.is_empty() && s == name);
            let mut err = None;
            // Terrain (Flat/Frontier/Transition zone blocks) may be lowered by
            // TINY_TERRAIN_DROP (full-scale metres; default 0). The drop was
            // 0.2 while a road deck and the Land plane of the same cell were
            // both items (coplanar at +2 in the source: grass stripes across
            // the road, 2026-09-06); since `tmmaps tiny` hides the tile under
            // every deck (`stands_in_for_tile`) the drop only opened a 0.1 m
            // step between a deck and its lowered neighbours — a black line
            // across the sand in front of Summer 06's start (the tile meshes
            // have no skirts). The pairs that still share a cell (pillar feet,
            // DecoTerrainHD, deco shores: `tmmaps shared-cells`) coexist with
            // their tile in the original too, so they are not coplanar.
            let terrain = matches!(bi.kind, crate::blockinfo::Kind::Flat | crate::blockinfo::Kind::Frontier | crate::blockinfo::Kind::Transition);
            let drop: f32 = std::env::var("TINY_TERRAIN_DROP").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            for (p, tr, rot) in &prefabs {
                let mut at = crate::geom::IDENTITY;
                if let Some(t) = tr {
                    at[9] = t[0];
                    at[10] = t[1];
                    at[11] = t[2];
                }
                if terrain {
                    at[10] -= drop;
                }
                if rot.map(|r| r.iter().any(|v| v.abs() > 1e-6)).unwrap_or(false) {
                    m.notes.push(format!("mobil rotation {:?} ignored for {p}", rot));
                }
                if let Err(e) = crate::static_item::build::add_prefab(store, p, &at, scale, &mut m, 0) {
                    err = Some(e);
                    break;
                }
            }
            match err {
                Some(e) => Err(e),
                None => {
                    // A terrain tile at the water row: the sea floor regains
                    // its depth (the apron under the water would otherwise
                    // sit at half depth and shade the sea a cell wide).
                    if let (true, true, Some((wrow, wlocal))) = (deepen, terrain, water) {
                        if rows_by_key.get(&(name.clone(), *flags)).map(|r| r.len() == 1 && r.contains(&wrow)).unwrap_or(false) {
                            crate::static_item::build::restore_depth(&mut m, wlocal * scale, depth_keep, scale);
                            deepened.push(format!("{name} {flags:08X}"));
                        }
                    }
                    // waypoint: type from the block info; the trigger is the
                    // variant's own `*_Trigger.Shape.Gbx` scaled (for the road
                    // checkpoints a 0.1 m plane across the middle of the block,
                    // deck to ~8 m — where the original fires), the unit-box
                    // volume only for a block without one; spawn = variant
                    // spawn_loc scaled (Granady's items)
                    if let Some(wt) = bi.waypoint_type.filter(|t| (0..=2).contains(t) || *t == 4) {
                        m.waypoint_type = Some(wt);
                        if wt != 0 {
                            let mut trig = None;
                            for sp in &pk.variant.trigger_shapes {
                                match crate::static_item::build::trigger_from_shape_file(store, sp, &crate::geom::IDENTITY, scale) {
                                    Ok(t) => {
                                        m.notes.push(format!("waypoint trigger from {}", sp.rsplit('\\').next().unwrap_or(sp)));
                                        trig = Some(t);
                                        break;
                                    }
                                    Err(e) => m.notes.push(format!("trigger shape {sp}: {e}; next")),
                                }
                            }
                            m.trigger = Some(trig.unwrap_or_else(|| {
                                m.notes.push("waypoint trigger: no shape in the block info, unit box".to_string());
                                unit_box_trigger(&units, scale)
                            }));
                        }
                        let sl = pk.variant.spawn_loc;
                        m.spawn = [sl[0] * scale, sl[1] * scale, sl[2] * scale];
                    }
                    // the block info's skin declaration (the screen blocks'
                    // `Any\Advertisement16x9\`) travels into the item header
                    if let Some(chunk) = store.read(&path).ok().and_then(|b| tmmaps::header::game_skin_chunk(&b)) {
                        if let Some(s) = tmmaps::header::GameSkin::decode(&chunk) {
                            m.notes.push(format!("skin {} ({} slots)", s.dir, s.fids.len()));
                        }
                        m.skin = Some(chunk);
                    }
                    let opts = crate::static_item::build::BuildOpts { ident: ident.clone(), author: ident.clone(), scale, collection, editors: m.editors, skin: m.skin.clone() };
                    crate::static_item::build::assemble(&m, &opts).map(|f| (crate::static_item::file::write_file(&f), m))
                }
            }
        };
        match res {
            Ok((bytes, m)) => {
                let nv = m.visuals.len();
                let veget = m.notes.iter().filter(|n| n.to_ascii_lowercase().contains(".vegettreemodel.gbx")).count();
                let other_skips = m.notes.iter().filter(|n| (n.contains("skipped") || n.contains("failed") || n.contains("unnamed")) && !n.to_ascii_lowercase().contains(".vegettreemodel.gbx")).count();
                let wp = match m.waypoint_type { Some(t) => format!(", waypoint {t} spawn {:?} trigger {}", m.spawn, m.trigger.is_some()), None => String::new() };
                // The prefab's vegetation entities become stock tree items placed
                // with the block (`v@ALIAS` rows: item, position in the item's
                // scaled frame, yaw), the species one step smaller like the
                // map's own vegetation. Only in `substitute` mode.
                let mut re_emitted = 0usize;
                if veget_mode == "substitute" {
                    for (p, iso) in &m.veget {
                        let Some(item) = veget_item(store, collection, p, &mut veget_cache) else { continue };
                        let yaw = (-iso[2]).atan2(iso[0]);
                        veget_rows.push_str(&format!("v@{ident}\t{item}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\n", iso[9] * scale, iso[10] * scale, iso[11] * scale, yaw));
                        re_emitted += 1;
                    }
                }
                let summary = format!("{} bytes, {} visuals, {} collision tris, {} vegetation entities ({} re-emitted as items), {} other skips{wp}{}", bytes.len(), nv, m.surf_triangles.len(), veget, re_emitted, other_skips, lod_summary(&m));
                if nv == 0 {
                    outcomes.push(Outcome { alias: alias.clone(), kind: "block", source: format!("{name} {flags:08X} [{}] {}", pk.label, recipe), placements: *n, result: Err(format!("no visuals ({summary}); notes: {}", m.notes.iter().take(3).cloned().collect::<Vec<_>>().join(" | "))) });
                    continue;
                }
                outcomes.push(Outcome { alias: alias.clone(), kind: "block", source: format!("{name} {flags:08X} [{}] {}", pk.label, prefabs.iter().map(|p| p.0.rsplit('\\').next().unwrap_or(&p.0).to_string()).collect::<Vec<_>>().join("+")), placements: *n, result: Ok(summary) });
                files.insert(format!("Items/{ident}"), bytes);
                alias_of_recipe.insert(recipe, alias.clone());
                block_map.insert((name.clone(), *flags, modk.clone()), (alias, sx, sz));
            }
            // a prefab with no entities at all (Stadium\Structure\PillarToFlat_ACB
            // is one): the game draws nothing there either
            Err(e) if e.starts_with("no visuals: nothing to build") => {
                next_alias -= 1;
                block_map.insert((name.clone(), *flags, modk.clone()), ("-".into(), sx, sz));
                alias_of_recipe.insert(recipe.clone(), "-".into());
                outcomes.push(Outcome { alias: "-".into(), kind: "block", source: format!("{name} {flags:08X} [{}] {}", pk.label, recipe), placements: *n, result: Ok("empty prefab (no entities): intentionally no item".into()) });
            }
            Err(e) => outcomes.push(Outcome { alias: alias.clone(), kind: "block", source: format!("{name} {flags:08X} [{}] {}", pk.label, recipe), placements: *n, result: Err(e) }),
        }
    }
    // item models — one library entry per (model, VARIANT): the placement's
    // variant byte picks which external of a variant-list item it shows
    // (Summer 11's `Show` rigs are 2 m stubs, 32 m beams, spot bars, speakers
    // and foggers of ONE item; a `PalmForest` placement's variant is its palm
    // species). Items whose file has no variant list share one entry.
    let mut item_counts: BTreeMap<(String, u8), usize> = BTreeMap::new();
    for it in &source.items {
        *item_counts.entry((it.model.clone(), it.variant())).or_insert(0) += 1;
    }
    // (model, variant) -> new model name: an embedded alias (AI...Item.Gbx) or a stock species
    let mut item_map: BTreeMap<(String, u8), String> = BTreeMap::new();
    // the map's own embedded files (custom items live under Items\…)
    let embedded: BTreeMap<String, Vec<u8>> = crate::embedded::files(&source).unwrap_or_default();
    let mut item_alias_n = 0usize;
    // a model without a variant list is built once; later variants reuse it
    let mut single_variant: BTreeMap<String, String> = BTreeMap::new();
    // stock half-size variants used as targets: their mapping rows carry
    // model_scale = scale like an embedded half-size copy
    let mut half_stock: std::collections::BTreeSet<String> = Default::default();
    for ((model, variant), n) in &item_counts {
        if model.is_empty() || !wanted(model) {
            continue;
        }
        if let Some(target) = single_variant.get(model) {
            item_map.insert((model.clone(), *variant), target.clone());
            continue;
        }
        // Stock HALF-SIZE variants (2026-09-07): Nadeo ships every screen in a
        // `Small` version that is exactly half in both dimensions
        // (RaceScreen6x1 24×4 m → RaceScreen6x1Small 12×2 m; Screen2x3Big →
        // Screen2x3 → Screen2x3Small). A stock item keeps the game's own
        // behaviour — the live in-game advertisement on its panel — which no
        // baked copy can have (the skin remap needs the model's own texture
        // file). The mapping row carries model_scale = scale so the placement
        // is treated like a half-size copy (scale 1, pivot halved).
        // TINY_STOCK_HALF=0 bakes them instead; TINY_STOCK_HALF=gates also
        // maps the gates to their half-WIDTH family member (32 m → 16 m: the
        // same 11 m posts, so twice the tiny height — a visible mismatch).
        if let Some(small) = stock_half_variant(model) {
            if find_item_file(store, small).is_some() {
                let key = (model.clone(), *variant);
                single_variant.insert(model.clone(), small.to_string());
                item_map.insert(key, small.to_string());
                half_stock.insert(small.to_string());
                outcomes.push(Outcome { alias: small.to_string(), kind: "item", source: model.clone(), placements: *n, result: Ok(format!("stock half-size variant {small}: the game's own item, its screen keeps the live advertisement")) });
                continue;
            }
        }
        let alias = format!("AI{item_alias_n:08}");
        let ident = format!("{alias}.Item.Gbx");
        let local = items_dir.map(|d| d.join(format!("{model}.Item.Gbx"))).filter(|p| p.is_file());
        // how many variants the item's file lists (pack items only)
        let mut variants: Vec<String> = Vec::new();
        let res = match &local {
            Some(p) => crate::static_item::build::static_item_from_item_report(&std::fs::read(p).unwrap(), &ident, &ident, scale, collection),
            None => match embedded.iter().find(|(k, _)| k.replace('/', "\\").eq_ignore_ascii_case(&format!("Items\\{model}"))).map(|(_, v)| v) {
                // a custom item the MAP embeds (the TME_* nation items)
                Some(bytes) => crate::static_item::build::static_item_from_item_report(bytes, &ident, &ident, scale, collection),
                None => match find_item_file(store, model) {
                    Some(logical) => {
                        variants = crate::static_item::build::pack_item_variants(store, &logical).unwrap_or_default();
                        crate::static_item::build::static_item_from_pack_item_report(store, &logical, &ident, &ident, scale, collection, *variant as usize)
                    }
                    None => Err("no .Item.Gbx in the client packs, the map's embedded files, or --items-dir".into()),
                },
            },
        };
        let multi = variants.len() > 1;
        let source_name = if multi {
            let picked = variants.get(*variant as usize).or(variants.first()).map(|p| p.rsplit('\\').next().unwrap_or(p).to_string()).unwrap_or_default();
            format!("{model} v{variant} ({picked})")
        } else {
            model.clone()
        };
        let key = (model.clone(), *variant);
        let mut remember = |target: &str| {
            if !multi {
                single_variant.insert(model.clone(), target.to_string());
            }
        };
        match res {
            // a moving item (rotor, tube) may have NO static visuals: all of
            // its geometry rides on the dyna parts
            Ok((out, m)) if !m.visuals.is_empty() || !m.dyna.is_empty() => {
                item_alias_n += 1;
                let lights = if m.lights_out.is_empty() { String::new() } else { format!(", {} light(s) embedded", m.lights_out.len()) };
                let moving = if m.dyna.is_empty() { String::new() } else { format!(", {} moving part(s)", m.dyna.len()) };
                let summary = format!("{} bytes, {} visuals, {} collision tris{lights}{moving}{}{}", out.len(), m.visuals.len(), m.surf_triangles.len(), lod_summary(&m), match m.waypoint_type { Some(t) => format!(", waypoint {t} trigger {} spawn {:?}", m.trigger.is_some(), m.spawn), None => String::new() });
                files.insert(format!("Items/{ident}"), out);
                for (file, dds) in &m.pictures {
                    pictures.entry(format!("Items/{file}")).or_insert_with(|| dds.clone());
                }
                remember(&ident);
                item_map.insert(key, ident.clone());
                outcomes.push(Outcome { alias: ident, kind: "item", source: source_name, placements: *n, result: Ok(summary) });
            }
            // A vegetation cluster (a prefab of tree entities, no mesh): the
            // placement is dropped and its trees placed as stock items
            // (`v@<model>` rows, keyed by the ITEM model name), each one step
            // smaller, at the item's position and yaw — Stadium's `Spring`
            // (384 in Summer 05) is 3-6 spring trees and a cypress.
            Ok((_, m)) if !m.veget.is_empty() && veget_mode == "substitute" => {
                let mut placed = 0usize;
                for (p, iso) in &m.veget {
                    let Some(item) = veget_item(store, collection, p, &mut veget_cache) else { continue };
                    let yaw = (-iso[2]).atan2(iso[0]);
                    veget_rows.push_str(&format!("v@{model}\t{item}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\n", iso[9] * scale, iso[10] * scale, iso[11] * scale, yaw));
                    placed += 1;
                }
                remember("-");
                item_map.insert(key, "-".into());
                outcomes.push(Outcome { alias: "-".into(), kind: "item", source: source_name, placements: *n, result: Ok(format!("vegetation cluster: {} of {} trees re-emitted as stock items per placement", placed, m.veget.len())) });
            }
            Ok((_, m)) => outcomes.push(Outcome { alias: String::new(), kind: "item", source: source_name, placements: *n, result: Err(format!("no visuals; notes: {}", m.notes.iter().take(3).cloned().collect::<Vec<_>>().join(" | "))) }),
            Err(e) if e.contains("procedural vegetation") => match veget_mode {
                "substitute" => {
                    // the variant names the SPECIES (`…\PalmTreeBigB1.VegetTreeModel.Gbx`):
                    // the stock item of that species one size down; else the
                    // collection ladder for the item's own name
                    let species = e.split_once("procedural vegetation: ").and_then(|(_, rest)| rest.split(" (").next()).filter(|p| p.to_ascii_lowercase().ends_with(".vegettreemodel.gbx")).map(|s| s.to_string());
                    let by_species = species.as_deref().and_then(|p| veget_item(store, collection, p, &mut veget_cache));
                    let by_name = || veget_substitute(collection, model).filter(|s| find_item_file(store, s).is_some()).map(|s| s.to_string());
                    match by_species.or_else(by_name) {
                        Some(sub) => {
                            let how = if species.is_some() { "species of this variant" } else { "the item's own name" };
                            remember(&sub);
                            item_map.insert(key, sub.clone());
                            outcomes.push(Outcome { alias: sub.to_string(), kind: "item", source: source_name, placements: *n, result: Ok(format!("vegetation: re-pointed at stock {sub} by {how} (placement scale is ignored by the game)")) });
                        }
                        None => {
                            remember(model);
                            item_map.insert(key, model.clone());
                            outcomes.push(Outcome { alias: model.clone(), kind: "item", source: source_name, placements: *n, result: Ok("vegetation: already a small species, kept".into()) });
                        }
                    }
                }
                "drop" => {
                    remember("-");
                    item_map.insert(key, "-".into());
                    outcomes.push(Outcome { alias: "-".into(), kind: "item", source: source_name, placements: *n, result: Ok("vegetation: dropped".into()) });
                }
                _ => {
                    remember(model);
                    outcomes.push(Outcome { alias: model.clone(), kind: "item", source: source_name, placements: *n, result: Ok("vegetation: kept full size".into()) });
                }
            },
            Err(e) => outcomes.push(Outcome { alias: String::new(), kind: "item", source: source_name, placements: *n, result: Err(e) }),
        }
    }
    // every embedded item claims the map's collection (header + body idents)
    for bytes in files.values_mut() {
        *bytes = crate::tiny_assets::set_ident_collection(bytes, collection);
    }
    // the pictures the items' custom-texture materials name (gate sign logos,
    // signlogo.rs) ride NEXT TO THE ITEMS — the one place the game resolves an
    // item's texture file name from
    if !pictures.is_empty() {
        println!("  pictures: {} sign logo DDS into Items/ ({})", pictures.len(), pictures.keys().map(|k| k.rsplit('/').next().unwrap_or(k)).collect::<Vec<_>>().join(" "));
        files.extend(pictures);
    }
    // TINY_PICTURES=DIR: the pictures the screen/gate materials were re-pointed
    // at (`custom_texture_material`) ride in the archive NEXT TO THE ITEMS —
    // the one place the game resolves an item's texture file name from.
    if let Some(dir) = std::env::var_os("TINY_PICTURES") {
        let mut n = 0;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut names: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map(|x| x.eq_ignore_ascii_case("dds")).unwrap_or(false)).collect();
            names.sort();
            for p in names {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                files.insert(format!("Items/{name}"), std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())));
                n += 1;
            }
        }
        println!("  pictures: {n} DDS from {} into Items/", std::path::Path::new(&dir).display());
    }
    let archive = crate::tiny_assets::zip(&files);
    std::fs::write(out_zip, &archive).unwrap();
    // mapping: @index rows for blocks (alias or "-" = intentionally nothing), i@ rows for items
    let mut mapping = String::from("# tiny-library mapping: @block_index<TAB>ITEM|-<TAB>model_scale<TAB>sx<TAB>sz ; i@item_index<TAB>ITEM|stock model|-\n");
    let mut missing_blocks: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows = 0usize;
    for (prefix, b) in source.blocks.iter().map(|b| ("@", b)).chain(source.baked.iter().filter(|b| b.name != "Sea").map(|b| ("b@", b))) {
        let modk = if prefix == "b@" { baked_key.get(&b.index).cloned().unwrap_or_default() } else { String::new() };
        match block_map.get(&(b.name.clone(), b.flags, modk)) {
            Some((alias, sx, sz)) => {
                let model = if alias == "-" { "-".to_string() } else { format!("{alias}.Item.Gbx") };
                mapping.push_str(&format!("{prefix}{}\t{}\t{}\t{}\t{}\n", b.index, model, scale, sx, sz));
                rows += 1;
            }
            None => *missing_blocks.entry(format!("{} {:08X}", b.name, b.flags)).or_insert(0) += 1,
        }
    }
    let mut missing_items: BTreeMap<String, usize> = BTreeMap::new();
    for it in &source.items {
        match item_map.get(&(it.model.clone(), it.variant())) {
            Some(target) => {
                let ms = if target.ends_with(".Item.Gbx") || half_stock.contains(target) { scale } else { 1.0 };
                mapping.push_str(&format!("i@{}\t{}\t{}\n", it.index, target, ms));
                rows += 1;
            }
            None => *missing_items.entry(it.model.clone()).or_insert(0) += 1,
        }
    }
    mapping.push_str(&veget_rows);
    std::fs::write(out_mapping, &mapping).unwrap();
    // report
    let mut rep = String::from("kind\talias\tplacements\tstatus\tsource\tdetail\n");
    let (mut ok, mut bad) = (0, 0);
    for o in &outcomes {
        match &o.result {
            Ok(s) => {
                ok += 1;
                rep.push_str(&format!("{}\t{}\t{}\tOK\t{}\t{}\n", o.kind, o.alias, o.placements, o.source, s));
            }
            Err(e) => {
                bad += 1;
                rep.push_str(&format!("{}\t{}\t{}\tFAIL\t{}\t{}\n", o.kind, o.alias, o.placements, o.source, e.replace('\n', " ")));
            }
        }
    }
    if let Some(r) = report {
        std::fs::write(r, &rep).unwrap();
    }
    println!("  library: {} embedded items; {} models ok, {} failed -> {}", files.len(), ok, bad, out_zip.display());
    if !deepened.is_empty() {
        println!("  sea floor at source depth under {} shore tile models at the water row (TINY_DEEPEN=0 to keep it halved): {}", deepened.len(), deepened.join(", "));
    }
    println!("  mapping: {} rows -> {}", rows, out_mapping.display());
    if !missing_blocks.is_empty() {
        println!("  BLOCK PLACEMENTS WITHOUT A MODEL:");
        for (k, n) in &missing_blocks {
            println!("    {n:>5} x {k}");
        }
    }
    if !missing_items.is_empty() {
        println!("  ITEM PLACEMENTS WITHOUT A MODEL (kept as they are):");
        for (k, n) in &missing_items {
            println!("    {n:>5} x {k}");
        }
    }
    for o in &outcomes {
        if let Err(e) = &o.result {
            println!("  FAIL {} {} ({} placements): {}", o.kind, o.source, o.placements, e);
        }
    }
    // Every STOCK model name the mapping points at (light substitutes, tree
    // species, `v@` rows) must be an item of the packs this map is built
    // with: the editor loads a foreign name from any installed pack, play mode
    // refuses the stored map ("Error while retrieving map! Missing Items:
    // PlantSmallA" — Summer 12, a BlueBay plant in a RedIsland map).
    let mut stock: BTreeMap<String, usize> = BTreeMap::new();
    for line in mapping.lines() {
        let mut f = line.split('\t');
        let (Some(head), Some(model)) = (f.next(), f.next()) else { continue };
        if head.starts_with('#') || model == "-" || model.ends_with(".Item.Gbx") {
            continue;
        }
        *stock.entry(model.to_string()).or_insert(0) += 1;
    }
    let missing: Vec<(String, usize)> = stock.into_iter().filter(|(name, _)| find_item_file(store, name).is_none()).collect();
    if !missing.is_empty() {
        println!("  STOCK ITEMS NOT IN THESE PACKS (play mode will refuse the map):");
        for (k, n) in &missing {
            println!("    {n:>5} x {k}");
        }
        std::process::exit(2);
    }
}

/// The material links a block's modifier folder provides: for each
/// `…\Modifier\X.TerrainModifier.Gbx` among the block info's material
/// modifier refs, every `…\Modifier\X\S.Material.Gbx` in the packs, as the
/// link `…\Modifier\X\S`. (The modifier file itself only names that folder.)
pub fn modifier_links(store: &DataStore, refs: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for r in refs {
        let Some(base) = r.strip_suffix(".TerrainModifier.Gbx") else { continue };
        let prefix = format!("{base}\\").to_uppercase();
        for e in store.entries() {
            let p = e.path();
            if p.to_uppercase().starts_with(&prefix) {
                if let Some(link) = p.strip_suffix(".Material.Gbx") {
                    out.push(link.to_string());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
