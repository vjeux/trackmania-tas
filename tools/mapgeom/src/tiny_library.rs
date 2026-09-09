//! Build the complete tiny library of a map through the STATIC-ITEM path (the
//! item kind Granady' tiny blocks are). Driven by the game's own block infos:
//! for every authored block (name + flags) the picked variant gives the
//! prefab(s), the unit footprint, the waypoint kind and the spawn point; each
//! becomes a half-scale `CPlugStaticObjectModel` item with the stage-1
//! mechanisms (`static_item/bake.rs`; recipe env vars apply), under the map's
//! own collection. Every item model of the map is baked from its pack file
//! (external prefab / static object) or, when it is procedural vegetation,
//! BAKED from its `.VegetTreeModel.Gbx` as a half-size static item too
//! (`--veget bake`, the default; species under 2 m stay stock items) — or,
//! with `--veget substitute|keep|drop`, re-pointed at a smaller stock
//! species / kept / dropped (the 2026-09-05..07 stand-in path).
//! Legacy prefab-less blocks come from the converted Nadeo item archive
//! (`--legacy-zip`). A variant with no geometry (an intentionally empty
//! pillar) maps to `-`: no item, on purpose.
//!
//! Writes the library zip, the `tmmaps tiny` mapping, and a report of every
//! model with its outcome, so a gap is explicit, never silent.
//!
//! Usage: mapgeom tiny-library MAP.Map.Gbx --library-out ITEMS.zip --mapping-out placements.tsv
//!        [--report REPORT.tsv] [--scale 0.5] [--legacy-zip Nadeo.zip] [--items-dir DIR]
//!        [--veget bake|substitute|keep|drop] [--collection BlueBay] [--only NAME[,NAME]]
//! Needs the client packs (`--pak FILE:KEY` for BlueBay.pak and the Stadium pak).

use crate::static_item::build::{Redress, RedressKey};
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
pub fn find_item_file(store: &DataStore, model: &str) -> Option<String> {
    let want = format!("\\{}.ITEM.GBX", model.to_uppercase());
    let mut hits: Vec<String> = store.entries().map(|e| e.path()).filter(|p| p.to_uppercase().ends_with(&want)).collect();
    hits.sort_by_key(|p| (!p.to_uppercase().contains("\\ITEMS\\"), p.len()));
    hits.into_iter().next()
}

/// The stock item that is exactly the HALF of `model` — Nadeo's `Small`
/// screens (measured 2026-09-07 on the pack: every pair halves both width and
/// height; the frame depth stays 0.86 m on all of them). The gates have no
/// such twin: the half-WIDTH family member (32 m → 16 m) keeps the same 11 m
/// posts — twice the tiny height, a visible mismatch — so gates are baked.
/// The pseudo light-skin key of a converted flag placement baked with the STILL
/// cloth (no stock driver can hide under it, see `covered_cells` and the guard
/// in `build`).
pub const STILL_FLAG_KEY: &str = "still";

pub fn stock_half_variant(model: &str, variant: u8) -> Option<&'static str> {
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
    // The flag (2026-09-08): `Flag8m` IS the half-size `Flag16m` — its cloth
    // (`FlagSmall.Mesh.Gbx`) is the 16 m flag's cloth at exactly x0.5, frame for
    // frame, the same 5 detail levels; only its pole is 5 m where a strict half
    // would be 6.5 m (the cloth rides 1.5 m lower). As a stock item its cloth is
    // driven by the game's own vertex-tween machinery, which no embedded copy
    // gets right: an embedded tween cloth draws only while a stock flag is
    // loaded and near, and is garbage (crumpled shards, giant sails) as soon as
    // the flags sit at different detail levels — the state of every tiny map
    // with converted flags until today. TINY_FLAG_STOCK=0 bakes the flag instead
    // (still cloth under ItemFlagNoAnim, or the tween part with TINY_FLAG_TWEEN=1).
    if model == "Flag16m" && std::env::var("TINY_FLAG_STOCK").as_deref() != Ok("0") {
        return Some("Flag8m");
    }
    // The particle items (2026-09-08): an embedded item cannot carry a live
    // emitter in this build — the game silently DROPS any item whose prefab
    // has an FxSystem entity with a model (sixteen one-item probes, FX thread),
    // so the baked fogger/sparkler/torch stood in the map with no smoke or
    // sparks. Nadeo ships each of them in a smaller version whose NAME is the
    // effect's reach, not the machine's size: `ShowFogger8M` is the same
    // 0.6 m box as `ShowFogger16M` with a plume that carries half as far,
    // `Sparkler8m` the same for the sparks, `ShowTorchSmall` a 1.6 m torch for
    // the 2.8 m one (measured on the pack prefabs). Placed as STOCK items the
    // game runs its own particle systems for them — the half-reach effect on
    // a half-size map, for free (vjeux: "can we use a smaller version of the
    // fog machine so we still get the effect?"). TINY_FX_STOCK=0 bakes them
    // (static, no effect) as before.
    if std::env::var("TINY_FX_STOCK").as_deref() != Ok("0") {
        const FX: &[(&str, &str)] = &[
            ("ShowFogger16m", "ShowFogger8m"),
            ("ShowFoggerWithLight16m", "ShowFoggerWithLight8m"),
            ("Sparkler16m", "Sparkler8m"),
            ("ShowTorch", "ShowTorchSmall"),
        ];
        if let Some((_, small)) = FX.iter().find(|(big, _)| *big == model) {
            return Some(small);
        }
        // The generic `Show` rig is one item in 37 variants (its prefab list:
        // rigs, supports, `Light4Spots` 23, `LightRamp4m` 24, `LightRamp8m` 25,
        // speakers 26/27, `Fogger16M` at 28, stage supports, front, up); the
        // fogger variant is the same prefab `ShowFogger16M` wraps — 60
        // placements over ten maps (02 ×4, 03 ×10, 04 ×14, 07 ×6, 08 ×2,
        // 09 ×4, 10 ×2, 11 ×6, 18 ×4, 21 ×8), smokeless as baked copies. The
        // other variants bake as before; the placement's variant byte is
        // rewritten to 0 for the stand-in (`iv@` mapping row).
        if model == "Show" && variant == 28 {
            return Some("ShowFogger8m");
        }
        // The maps' own 8 m sparklers (168 placements: 03 ×15, 06 ×25, 10 ×8,
        // 19 ×29, 20 ×55, 21 ×28, 24 ×8) have no 4 m stock sibling. A/B knob
        // (2026-09-08, this thread): TINY_FX_SPARK8=stock keeps them as the
        // stock item — live sparks, at the FULL 8 m reach in a half-size
        // world (the 0.5 m box is what every stock sparkler is anyway);
        // unset/bake = the static half-size copy, no sparks (as before).
        if model == "Sparkler8m" && std::env::var("TINY_FX_SPARK8").as_deref() == Ok("stock") {
            return Some("Sparkler8m");
        }
    }
    None
}

/// The stock vegetation item standing in for a prefab's `.VegetTreeModel.Gbx`
/// entity — `…\TreeBigA1.VegetTreeModel.Gbx` -> the pack item `TreeBigA` (an
/// item carries its A1/A2/A3 variants; `PalmTreeBigB3` -> `PalmTreeBigB`) —
/// with the collection's smaller species for it: (original item, substitute
/// item), the substitute being the original when no smaller species exists in
/// the packs. None when no item matches.
fn veget_item_pair(store: &DataStore, collection: u32, model_path: &str, cache: &mut BTreeMap<String, Option<String>>) -> Option<(String, String)> {
    let file = model_path.rsplit('\\').next().unwrap_or(model_path);
    let low = file.to_ascii_lowercase();
    let stem = if low.ends_with(".vegettreemodel.gbx") { &file[..file.len() - ".vegettreemodel.gbx".len()] } else { file };
    if let Some(c) = cache.get(stem) {
        return c.clone().and_then(|s| s.split_once('\t').map(|(a, b)| (a.to_string(), b.to_string())));
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
    let out = found.map(|item| {
        let sub = match veget_substitute(collection, &item) {
            Some(sub) if find_item_file(store, sub).is_some() => sub.to_string(),
            _ => item.clone(),
        };
        (item, sub)
    });
    cache.insert(stem.to_string(), out.as_ref().map(|(a, b)| format!("{a}\t{b}")));
    out
}

/// How far a stock tree standing in for another species is SUNK into the
/// ground (metres) so its crown top sits where the original's would at the
/// tiny scale: `top(substitute) - top(original) * scale`, never negative.
/// The game ignores placement scale for VegetTreeModel items, so a species
/// one size down is still taller than half the original — its trunk poked
/// through the roads above (vjeux, Summer 20: "trees overlapping with the
/// road and making the map impossible to play"). Heights come from the tree
/// models' own visual boxes (`veget::tree_model_stats`; the trunk mesh — the
/// procedural crown is not in the file, so the reference is the trunk top,
/// which is where a palm's crown sits). A species whose model does not read
/// sinks 0 (noted once).
fn veget_sink(store: &mut DataStore, orig: &str, sub: &str, scale: f32, cache: &mut BTreeMap<String, Option<f32>>) -> f32 {
    // the model's own height: a trunk-only mesh (the Stadium palms: radius
    // under a metre, the crown is procedural) gets a crown allowance on top —
    // `tree_clear::CROWN_ALLOWANCE` metres: the fronds a half tree would
    // carry are what the roads must clear
    fn measured(store: &mut DataStore, name: &str) -> Result<f32, String> {
        let path = find_item_file(store, name).ok_or_else(|| format!("no item file for {name}"))?;
        let s = crate::veget::tree_model_stats(store, &path)?;
        Ok(if s.radius < 1.0 { s.top + crate::tree_clear::CROWN_ALLOWANCE } else { s.top })
    }
    // a species whose model does not read borrows a sibling's height
    // (`species_siblings`); a species with no sibling either sinks 0 (noted once)
    let siblings = species_siblings;
    let top = |name: &str, store: &mut DataStore, cache: &mut BTreeMap<String, Option<f32>>| -> Option<f32> {
        if let Some(t) = cache.get(name) {
            return *t;
        }
        let t = match measured(store, name) {
            Ok(t) => Some(t),
            Err(e) => {
                let mut borrowed = None;
                for sib in siblings(name) {
                    if let Ok(t) = measured(store, &sib) {
                        eprintln!("  vegetation: {name}: no height of its own ({e}); {sib}'s {t:.2} m used");
                        borrowed = Some(t);
                        break;
                    }
                }
                if borrowed.is_none() {
                    eprintln!("  vegetation: {name}: no height ({e}) and no sibling with one; not sunk");
                }
                borrowed
            }
        };
        cache.insert(name.to_string(), t);
        t
    };
    let (Some(t_orig), Some(t_sub)) = (top(orig, store, cache), top(sub, store, cache)) else { return 0.0 };
    (t_sub - t_orig * scale).max(0.0)
}

/// A species' siblings, for borrowing a height or a crown from a model that
/// reads: the name without its terrain word (PalmTreeDirtSmall ->
/// PalmTreeSmall), then without its variant letter/digit (PalmTreeSmallB ->
/// PalmTreeSmallA -> PalmTreeSmall).
pub fn species_siblings(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in ["Dirt", "Grass", "Snow", "Sand", "Rock", "Water", "Ice"] {
        if name.contains(word) {
            out.push(name.replacen(word, "", 1));
        }
    }
    let no_digits = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if no_digits != name {
        out.push(no_digits.to_string());
    }
    if let Some(last) = no_digits.chars().last() {
        if last.is_ascii_uppercase() && no_digits.len() > 1 {
            let stem = &no_digits[..no_digits.len() - 1];
            if last != 'A' {
                out.push(format!("{stem}A"));
            }
            out.push(stem.to_string());
        }
    }
    out
}


/// The tree bake of `tiny-library --veget bake` (the default since
/// 2026-09-08): every vegetation SPECIES the map places — the map's own
/// vegetation items, the trees inside the prefabs, the cluster items' trees —
/// becomes ONE half-size static item (`AV00000000.Item.Gbx`…, built by
/// `static_item::build::add_veget_tree_model` from the pack's
/// `.VegetTreeModel.Gbx`), placed at the source position × scale with the
/// source yaw, exactly like a block. No stand-in species, no sink, no
/// clearance drop: a half-size tree at a half-size position meets a road
/// exactly when the original did (vjeux, 2026-09-08: "the trees look really
/// weird at double the size, can we make a static model half the size?").
/// Species under [`TreeBaker::MIN_HEIGHT`] metres (grass, flowers, the small
/// bushes) keep the stock-item path — the game's own vegetation renderer
/// draws thousands of those cheaply and their size hardly reads.
/// `--veget substitute` is the old stand-in path everywhere.
struct TreeBaker {
    enabled: bool,
    min_height: f32,
    bake_hullless: bool,
    /// species model path (lower-cased) -> the baked ident, or None when the
    /// species stays a stock item (too small, or its bake failed)
    baked: BTreeMap<String, Option<String>>,
    /// baked ident -> (radius, height) in the SCALED frame, for the
    /// clearance census
    dims: BTreeMap<String, (f32, f32)>,
    next: usize,
    /// bytes of the items and their textures added to the library
    item_bytes: usize,
    texture_bytes: usize,
}

impl TreeBaker {
    /// Species shorter than this (metres, unscaled) stay stock items.
    const MIN_HEIGHT: f32 = 2.0;

    fn new(mode: &str) -> TreeBaker {
        let enabled = mode == "bake";
        let min_height = Self::MIN_HEIGHT;
        let bake_hullless = std::env::var("TINY_TREE_BAKE_HULLLESS").map(|v| v == "1").unwrap_or(false);
        TreeBaker { enabled, min_height, bake_hullless, baked: BTreeMap::new(), dims: BTreeMap::new(), next: 0, item_bytes: 0, texture_bytes: 0 }
    }

    /// The baked item ident for a species model path (an `.Item.Gbx` of the
    /// packs is followed to its model), building it on first sight.
    #[allow(clippy::too_many_arguments)]
    fn ident_for(&mut self, store: &mut DataStore, species: &str, scale: f32, collection: u32, files: &mut BTreeMap<String, Vec<u8>>, pictures: &mut BTreeMap<String, Vec<u8>>, outcomes: &mut Vec<Outcome>) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let model_path = match crate::veget::tree_model_path(store, species) {
            Ok(p) => p,
            Err(e) => {
                outcomes.push(Outcome { alias: String::new(), kind: "tree", source: species.to_string(), placements: 0, result: Err(format!("no VegetTreeModel: {e}")) });
                return None;
            }
        };
        let key = model_path.to_ascii_lowercase();
        if let Some(v) = self.baked.get(&key) {
            return v.clone();
        }
        let stem = model_path.rsplit('\\').next().unwrap_or(&model_path).trim_end_matches(".VegetTreeModel.Gbx").to_string();
        let ident = format!("AV{:08}.Item.Gbx", self.next);
        let result = crate::static_item::build::static_item_from_veget_report(store, &model_path, &ident, &ident, scale, collection);
        let out = match result {
            Ok((bytes, m, bake)) => {
                if bake.height < self.min_height {
                    outcomes.push(Outcome { alias: "-".into(), kind: "tree", source: stem.clone(), placements: 0, result: Ok(format!("{:.1} m tall: under the {:.1} m bake threshold, stays a stock item", bake.height, self.min_height)) });
                    None
                } else if bake.hull_triangles == 0 && !self.bake_hullless {
                    // No collision hull = filler foliage the car never touches (grass,
                    // ferns, the JungleForest cards: 7 m tall, one material, no trunk):
                    // the game instances those by the tens of thousands (tiny 01 would
                    // carry 29 000 of them as items, 34 106 placements against 4 755),
                    // so they keep the stock path. TINY_TREE_BAKE_HULLLESS=1 bakes them.
                    outcomes.push(Outcome { alias: "-".into(), kind: "tree", source: stem.clone(), placements: 0, result: Ok(format!("{:.1} m tall but no collision hull: filler foliage, stays on the stock path", bake.height)) });
                    None
                } else {
                    self.next += 1;
                    self.item_bytes += bytes.len();
                    for (file, dds) in &m.pictures {
                        let name = format!("Items/{file}");
                        if !pictures.contains_key(&name) {
                            self.texture_bytes += dds.len();
                            pictures.insert(name, dds.clone());
                        }
                    }
                    files.insert(format!("Items/{ident}"), bytes.clone());
                    self.dims.insert(ident.clone(), (bake.radius * scale, bake.height * scale));
                    outcomes.push(Outcome {
                        alias: ident.clone(),
                        kind: "tree",
                        source: stem.clone(),
                        placements: 0,
                        result: Ok(format!(
                            "{} bytes, {} visuals in {} levels {:?}, switch {:?} m (unscaled), {} materials, hull {} tris, {:.1} m tall r {:.1} -> {:.1} m; textures {}",
                            bytes.len(),
                            m.visuals.len(),
                            bake.levels.len(),
                            bake.levels,
                            bake.switch,
                            m.materials.len(),
                            bake.hull_triangles,
                            bake.height,
                            bake.radius,
                            bake.height * scale,
                            bake.textures.iter().map(|(f, n)| format!("{f} {n} B")).collect::<Vec<_>>().join(", ")
                        )),
                    });
                    Some(ident)
                }
            }
            Err(e) => {
                outcomes.push(Outcome { alias: String::new(), kind: "tree", source: stem.clone(), placements: 0, result: Err(format!("bake failed, stock stand-in kept: {e}")) });
                None
            }
        };
        self.baked.insert(key, out.clone());
        out
    }
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
            tris.push(Triangle { indices: [b + f[0], b + f[1], b + f[2]], material_id: 0, gameplay: 0, surface_index: 0 });
        }
    }
    CPlugSurface::mesh(verts, tris, vec![0], [0.0, 0.0, 1.0])
}

/// One distinct block key of the map: the block name, its placement flags
/// (variant bits) and the material modifier a generated filler inherits from
/// the authored block it finishes (`inherited_mod`; empty for an authored
/// block), with how many placements share it.
struct BlockKey<'k> {
    name: &'k str,
    flags: u32,
    inherited_mods: &'k str,
    placements: usize,
}

impl BlockKey<'_> {
    /// `Name FLAGS`, the report's source column for a key without a variant.
    fn source(&self) -> String {
        format!("{} {:08X}", self.name, self.flags)
    }

    fn map_key(&self) -> (String, u32, String) {
        (self.name.to_string(), self.flags, self.inherited_mods.to_string())
    }

    fn outcome(&self, alias: &str, source: String, result: Result<String, String>) -> Outcome {
        Outcome { alias: alias.to_string(), kind: "block", source, placements: self.placements, result }
    }
}

/// The picked variant's footprint: the unit cells in the block's frame, the
/// footprint they span, and the terrain tiles the variant brings with it
/// (`auto_terrains`, offset + zone, and the place type; `None` for a terrain
/// tile itself — only the OTHER blocks hide tiles).
struct Footprint {
    sx: u32,
    sz: u32,
    units: Vec<[i32; 3]>,
    auto_terrain: Option<(Vec<([i32; 3], String)>, i32)>,
}

/// What the library does about one distinct block key, decided before
/// anything is baked.
enum BlockPlan<'b> {
    /// No item, on purpose (an ambient zone the genealogy regenerates, the
    /// Stadium grass floor, a variant without geometry): `why` is the report
    /// line, `label` the variant's when one was picked.
    Nothing { why: String, label: Option<String>, footprint: Option<Footprint> },
    /// Refused: the report's FAIL line (source detail, error).
    Refused { source: String, error: String },
    /// The same recipe was baked already under `alias`.
    Reuse { alias: String, footprint: Footprint },
    /// Bake a new item from this variant.
    Bake(Box<BlockBake<'b>>),
}

/// Everything a block bake needs, resolved from the block info.
struct BlockBake<'b> {
    pk: crate::blockinfo::Picked<'b>,
    footprint: Footprint,
    /// The variant's prefabs: path, mobil translation, mobil rotation.
    prefabs: Vec<(String, Option<[f32; 3]>, Option<[f32; 3]>)>,
    /// A converted-archive item standing in for a prefab-less model (`LEGACY`).
    legacy_item: Option<&'static str>,
    /// The recipe key: what gets baked + waypoint + modifier (two keys with
    /// the same recipe share one item).
    recipe: String,
    /// The block's own modifier refs plus the inherited one.
    effective_mods: Vec<String>,
    /// A terrain tile (Flat/Frontier/Transition zone block).
    terrain: bool,
}

/// The block info of a key, through the index (`--debug lookup` says why a
/// name has none).
fn load_block_info(idx: &mut crate::blockmap::BlockInfoIndex, store: &mut DataStore, name: &str) -> Result<(String, crate::blockinfo::BlockInfo), String> {
    let Some(path) = idx.path_for(name) else {
        if crate::debug::on("lookup") {
            eprintln!("  lookup {name:?}: no path; index knows {} stems; store has {} entries", idx.stem_count(), store.entries().count());
        }
        return Err("no block info file with this name".into());
    };
    let bi = idx.load(store, &path).map_err(|e| format!("block info: {e}"))?.clone();
    Ok((path, bi))
}

/// The decision ladder for one block key.
fn plan_block<'b>(bi: &'b crate::blockinfo::BlockInfo, key: &BlockKey, collection: u32, ambient: &str, tile_zones: &std::collections::BTreeSet<String>, alias_of_recipe: &BTreeMap<String, String>) -> BlockPlan<'b> {
    let name = key.name;
    let flags = key.flags;
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
    if matches!(collection, 0x10 | 0x1d | 0xf) && !ambient.is_empty() && name == ambient {
        return BlockPlan::Nothing { why: format!("{} ambient {name}: regenerated full size by the genealogy, no item", crate::static_item::build::env_name(collection)), label: None, footprint: None };
    }
    if collection == 0x1a && name == "Grass" {
        return BlockPlan::Nothing { why: "Stadium grass floor: regenerated full size by the genealogy, no item".into(), label: None, footprint: None };
    }
    let ground = flags & crate::blockmap::FLAG_GROUND != 0;
    let vindex = (flags & crate::blockmap::FLAG_VARIANT_MASK) as usize;
    let sub = ((flags >> crate::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
    let addv = ((flags >> crate::blockmap::FLAG_ADDITIONAL_SHIFT) & 0x7F) as usize;
    let Some(pk) = bi.pick_placement_add(ground, vindex, sub, addv) else {
        return BlockPlan::Refused { source: key.source(), error: "block info has no variant with units or mobils".into() };
    };
    let units: Vec<[i32; 3]> = pk.variant.block_units.iter().map(|u| u.offset).collect();
    let (sx, sz) = units.iter().fold((1u32, 1u32), |(sx, sz), u| (sx.max(u[0] as u32 + 1), sz.max(u[2] as u32 + 1)));
    // a terrain tile's auto terrain is itself: only the OTHER blocks hide tiles
    let auto_terrain = if !tile_zones.contains(name) && !pk.variant.auto_terrains.is_empty() {
        Some((pk.variant.auto_terrains.iter().map(|(off, _, cur)| (*off, cur.clone())).collect(), pk.variant.auto_terrain_place_type))
    } else {
        None
    };
    let footprint = Footprint { sx, sz, units: units.clone(), auto_terrain };
    let prefabs: Vec<(String, Option<[f32; 3]>, Option<[f32; 3]>)> = pk.mobils.iter().filter_map(|mb| mb.prefab.clone().map(|p| (p, mb.translation, mb.rotation))).collect();
    let solids: Vec<String> = pk.mobils.iter().filter_map(|mb| mb.solid.clone()).collect();
    let legacy_item = LEGACY.iter().find(|(n, _)| *n == name).map(|(_, p)| *p);
    // recipe key: what gets baked (prefab set or legacy item) + waypoint
    // + the block's material modifier — PlatformGrass*/PlatformDirt*/
    // PlatformIce* share the PlatformTech prefabs and differ ONLY by the
    // modifier folder their materials are taken from (Summer 03: the tech
    // slopes next to the first checkpoint came out grass, keyed to the
    // PlatformGrassSlope2Straight item built first).
    // the block's own modifier plus the one a filler inherits from the
    // authored block it finishes (`inherited_mods`, see `inherited_mod`)
    let effective_mods: Vec<String> = bi.material_modifier.iter().cloned().chain(key.inherited_mods.split('|').filter(|s| !s.is_empty()).map(String::from)).collect();
    let recipe = if let Some(l) = legacy_item { format!("legacy:{l}") } else { format!("{}|wp{:?}|units{:?}|mod{:?}", prefabs.iter().map(|p| format!("{}@{:?}/{:?}", p.0, p.1, p.2)).collect::<Vec<_>>().join(","), bi.waypoint_type, units, effective_mods) };
    if prefabs.is_empty() && legacy_item.is_none() {
        if solids.is_empty() {
            // intentionally empty variant (e.g. the hidden pillar)
            return BlockPlan::Nothing { why: "no geometry in this variant: intentionally no item".into(), label: Some(pk.label.clone()), footprint: Some(footprint) };
        }
        return BlockPlan::Refused { source: format!("{} [{}] solids {:?}", key.source(), pk.label, solids), error: "legacy CPlugSolid model without a converted archive item".into() };
    }
    if let Some(alias) = alias_of_recipe.get(&recipe) {
        return BlockPlan::Reuse { alias: alias.clone(), footprint };
    }
    let terrain = matches!(bi.kind, crate::blockinfo::Kind::Flat | crate::blockinfo::Kind::Frontier | crate::blockinfo::Kind::Transition);
    BlockPlan::Bake(Box::new(BlockBake { pk, footprint, prefabs, legacy_item, recipe, effective_mods, terrain }))
}

/// The bake of one block key into an item: the legacy archive item
/// re-identified, or the variant's prefabs merged with the block's
/// modifier, sea-floor depth, special trigger, waypoint and skin. `water`:
/// the collection's water row and the surface's height above that row's
/// floor; `at_water_row`: whether every placement of this key sits on it.
#[allow(clippy::too_many_arguments)]
fn bake_block(store: &mut DataStore, plan: &BlockBake, name: &str, path: &str, bi: &crate::blockinfo::BlockInfo, ident: &str, scale: f32, collection: u32, legacy: &BTreeMap<String, Vec<u8>>, water: Option<(u8, f32)>, at_water_row: bool) -> Result<(Vec<u8>, crate::static_item::build::Merged, bool), String> {
    if let Some(l) = plan.legacy_item {
        return match legacy.get(l) {
            Some(bytes) => crate::static_item::build::static_item_from_item_report(bytes, ident, ident, scale, collection).map(|(b, m)| (b, m, false)),
            None => Err(format!("{l} absent from the legacy archive (pass --legacy-zip)")),
        };
    }
    let mut m = crate::static_item::build::Merged::default();
    m.keep_water = crate::static_item::build::keep_water_for(collection);
    m.modifier = modifier_links(store, &plan.effective_mods);
    m.collision_redress = modifier_redress(store, &plan.effective_mods, &m.modifier);
    // A gameplay gate BLOCK (GateSpecialBoost / Boost2 / Reset / …) is the
    // Turbo-dressed Special prefab re-dressed by its `<Kind>.TerrainModifier`
    // folder; the ring's sign panels resolve through `gate_kind` (signlogo.rs),
    // exactly as the gate ITEMS do — without it every gate block wore the
    // Turbo chevrons (Summer 15's reactor gate by the inflatable loop, vjeux
    // 2026-09-08 23:27Z: "the ring turned from a REACTOR ring to a BOOSTER").
    if m.modifier.iter().any(|l| l.ends_with("\\Sign")) {
        if let Some(kind) = m.modifier.iter().find(|l| l.ends_with("\\Sign")).and_then(|l| l.strip_prefix("Stadium\\Media\\Modifier\\")).and_then(|r| r.split('\\').next()) {
            m.gate_kind = Some(kind.to_string());
        }
    }
    // (The DecoPlatform blocks — Slope2Start, SlopeBase, … — keep their
    // `Deco` material: it IS what the game draws, the grass-topped
    // decorative platform, phys 2. 70d461c re-dressed them as grey
    // PlatformTech for a day; the original's cp3 slopes are green grass.)
    // Terrain (Flat/Frontier/Transition zone blocks) sits where the
    // source puts it. (A 0.2 m drop hid the coplanar deck/Land pairs
    // of 2026-09-06; since `tmmaps tiny` hides the tile under every
    // deck the drop only opened a 0.1 m step to the lowered
    // neighbours — a black line across Summer 06's sand; gone.)
    for (p, tr, rot) in &plan.prefabs {
        let mut at = crate::geom::IDENTITY;
        if let Some(t) = tr {
            at[9] = t[0];
            at[10] = t[1];
            at[11] = t[2];
        }
        // The mobil's own rotation, degrees about (x, y, z). The four BlueBay
        // `Road*OnLandHillSlopeBase2x1` block infos author their ground
        // variant as "OnLandHill 180°": the prefab turned 180° about Y and
        // moved by (32, 0, 64) so it lands back on the 1×2 footprint. Ignored
        // until 2026-09-09, the translation alone put Argentina 21's first ice
        // slope one cell east and two north of its cell — in the sea — while
        // the cell itself stayed empty (vjeux: "missing an entire ice block at
        // the beginning"). Only Y rotations exist in the packs (surveyed:
        // every OnLand* block info); any other axis is still refused loudly.
        if let Some(r) = rot.filter(|r| r.iter().any(|v| v.abs() > 1e-6)) {
            if r[0].abs() > 1e-6 || r[2].abs() > 1e-6 {
                return Err(format!("{p}: mobil rotation {r:?} has an x/z component; only a yaw is implemented"));
            }
            // yaw_quarter(2) == 180° either way; for other angles the sign
            // convention is the grid dir's (clockwise looking down)
            let yaw = crate::geom::yaw(r[1].to_radians(), [at[9], at[10], at[11]]);
            at = yaw;
            m.notes.push(format!("mobil rotation {:?} applied for {p} (translation {:?})", r, tr));
        }
        crate::static_item::build::add_prefab(store, p, &at, scale, &mut m, 0)?;
    }
    // A terrain tile at the water row: the sea floor regains its depth (the
    // apron under the water would otherwise sit at half depth and shade the
    // sea a cell wide).
    let mut deepened = false;
    if let (true, Some((_, wlocal)), true) = (plan.terrain, water, at_water_row) {
        crate::static_item::build::restore_depth(&mut m, wlocal * scale, scale);
        deepened = true;
    }
    // A gameplay gate BLOCK (GateSpecialBoost/Reset/…): its trigger
    // disc lives in the block info, not the prefab (`blockinfo_special_
    // trigger`); the effect is the block's modifier Collision material
    // (the disc's own bytes say Turbo). The item takes the prefab form.
    if m.special.is_none() && name.starts_with("GateSpecial") {
        match crate::static_item::build::blockinfo_special_trigger(store, path) {
            Some((verts, tris, own, dir)) => {
                let (ids, from) = match crate::static_item::build::special_collision_ids(store, &m) {
                    Some((link, ids)) => (ids, link),
                    None => (own, "the block info's own disc".to_string()),
                };
                if ids.1 != 0 {
                    let sf = CPlugSurface::mesh(verts, tris, vec![ids.0 as u16 | ((ids.1 as u16) << 8)], dir);
                    match crate::static_item::build::trigger_mesh(&sf, &crate::geom::IDENTITY, scale, ids) {
                        Some(t) => {
                            m.notes.push(format!("special trigger from the block info: physics {} gameplay {} from {from} ({} triangles, main dir {:?}) — prefab form", ids.0, ids.1, t.surf.counts().1, dir));
                            m.special = Some(t);
                        }
                        None => m.notes.push("special trigger from the block info has no triangles".to_string()),
                    }
                } else {
                    m.notes.push(format!("special trigger from the block info has gameplay 0 ({from}); not emitted"));
                }
            }
            None => m.notes.push("GateSpecial block without a Collision-material trigger disc in its block info".to_string()),
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
        // The block info's NoRespawn (chunk 0x0304E00F): the `GateCheckpoint`
        // ring. The game never respawns at it — a respawn goes back to the
        // previous checkpoint with a spawn. The item form that carries the
        // flag is the pack's prefab layout (NPlugTrigger_SWaypoint.NoRespawn);
        // without it Argentina 21's ring dropped the car at the item's origin,
        // beside the ring (vjeux, 2026-09-09: "the double respawn of the ring
        // … drops you to the side").
        if bi.no_respawn && wt == 2 {
            m.no_respawn = true;
            m.notes.push("no-respawn waypoint (block info NoRespawn): prefab form".to_string());
        }
        if wt != 0 {
            let mut trig = None;
            for sp in &plan.pk.variant.trigger_shapes {
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
                unit_box_trigger(&plan.footprint.units, scale)
            }));
        }
        let sl = plan.pk.variant.spawn_loc;
        m.spawn = [sl[0] * scale, sl[1] * scale, sl[2] * scale];
    }
    // the block info's skin declaration (the screen blocks'
    // `Any\Advertisement16x9\`) travels into the item header
    if let Some(chunk) = store.read(path).ok().and_then(|b| tmmaps::header::game_skin_chunk(&b)) {
        if let Some(s) = tmmaps::header::GameSkin::decode(&chunk) {
            m.notes.push(format!("skin {} ({} slots)", s.dir, s.fids.len()));
        }
        m.skin = Some(chunk);
    }
    // WATER is not a wall. A block's water surface (the `Water` material, physics
    // id 13: the 32×32 m DecoWallWaterFCT plane over a channel, WaterBase's own
    // quad) is a volume boundary in the game — the author of Summer 15 drives
    // UNDER the channel's plane at 14.5–15.5 s (y 42–46, plane at 48) and UP
    // through the next one at 18.3 s (y 69.9 → 75.8 across the plane at 72),
    // `mapgeom ghostpath`. The same triangles in an ITEM's collision are a solid
    // (physics 13 on an item surface): the tiny's car lands on the water instead
    // of going under, then meets the ghost loop-top roads the author passes
    // beneath, and hits the second plane from below — vjeux on ship13 (03:12Z):
    // "15, 20 — road blocks in the middle of the path"; the eyes' frame d187 =
    // the cream plate across the sand road. So every Water-physics collision
    // triangle becomes NotCollidable (28); the visual quad stays.
    {
        let mut n = 0usize;
        for t in m.surf_triangles.iter_mut() {
            if t.material_id == 13 {
                t.material_id = 28;
                t.gameplay = 0;
                n += 1;
            }
        }
        if n > 0 {
            // the (physics | gameplay << 8) table the triangles index: keep the indices, retarget the entries
            for id in m.surf_ids.iter_mut() {
                if *id & 0xff == 13 {
                    *id = 28;
                }
            }
            m.notes.push(format!("{n} Water-physics collision triangles made NotCollidable (water is a volume, not a wall)"));
        }
    }
    let opts = crate::static_item::build::BuildOpts { ident: ident.to_string(), author: ident.to_string(), scale, collection, skin: m.skin.clone() };
    let f = crate::static_item::build::assemble(&m, &opts)?;
    Ok((crate::static_item::file::write_file(&f), m, deepened))
}

#[allow(clippy::too_many_arguments)]
/// The cells covered by any UNIT of an authored non-pillar, non-terrain block
/// (the footprint turned like `blockmap::footprint`): what the `covered` filler
/// rule and the flag-driver guard both read.
fn covered_cells(source: &MapFile, block_map: &BTreeMap<(String, u32, String), (String, u32, u32, Vec<[i32; 3]>)>, tile_zones: &std::collections::BTreeSet<String>) -> std::collections::HashSet<[u8; 3]> {
    let mut footprint_cells: std::collections::HashSet<[u8; 3]> = std::collections::HashSet::new();
    for b in source.blocks.iter().filter(|b| b.flags & crate::blockmap::FLAG_FREE == 0 && b.flags & crate::blockmap::FLAG_PILLAR == 0 && !tile_zones.contains(&b.name)) {
        let units: Vec<[i32; 3]> = block_map.get(&(b.name.clone(), b.flags, String::new())).map(|(_, _, _, u)| u.clone()).unwrap_or_default();
        let units = if units.is_empty() { vec![[0, 0, 0]] } else { units };
        let (mut minx, mut maxx, mut minz, mut maxz) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        for u in &units {
            minx = minx.min(u[0]);
            maxx = maxx.max(u[0]);
            minz = minz.min(u[2]);
            maxz = maxz.max(u[2]);
        }
        let (w, d) = (maxx - minx + 1, maxz - minz + 1);
        for u in &units {
            let (x, z) = (u[0] - minx, u[2] - minz);
            let (rx, rz) = match b.dir & 3 {
                0 => (x, z),
                1 => (d - 1 - z, x),
                2 => (w - 1 - x, d - 1 - z),
                _ => (z, w - 1 - x),
            };
            let (cx, cy, cz) = (b.file_cell[0] as i32 + rx, b.file_cell[1] as i32 + u[1], b.file_cell[2] as i32 + rz);
            if (0..=255).contains(&cx) && (0..=255).contains(&cy) && (0..=255).contains(&cz) {
                footprint_cells.insert([cx as u8, cy as u8, cz as u8]);
            }
        }
    }
    footprint_cells
}

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
    // (`Reset.TerrainModifier .Gbx` — the pack spells the Reset gates' modifier
    // with a space before the extension, block info and item alike; until
    // 2026-09-08 every GateSpecialReset block was baked in its Turbo dress)
    // `TrackWallToDecoCliff.Gbx` (DecoHill*, DecoPlatformBase, PlatformTechBase,
    // WaterBase, WaterWall, DecoCliff*: the Tech-family deco blocks) is a
    // material modifier too — the same class as the terrain modifiers, its
    // folder `Modifier\PlatformGrass\`, its skin the TrackWall slot: the
    // block's TrackWall clip panels are drawn as DecoCliff concrete (grey, no
    // hue mask) — see `modifier_links`. Skipped until 2026-09-08: Summer 20
    // cp3's hill sides and 15's pool walls came out plain TrackWall, tinted
    // red / blue by the map's colour where the original shows grey concrete.
    //
    // WHICH blocks dress their clips at all: those with a `MatModifier`
    // PLACEMENT TAG (CGameCtnBlockInfo::MatModifierPlacementTag, chunk
    // 0x0304E023 v8: ("MatModifier", "Grass" | "Dirt" | …)). DecoHill*,
    // WaterBase, WaterWall, DecoPlatformBase, DecoCliff*, OpenTechRoad/Zone*
    // carry `Grass` (their modifier `TrackWallToDecoCliff`, folder
    // PlatformGrass), OpenDirtRoad/Zone*, DecoHillDirt*, WaterWallDirt,
    // DecoPlatformDirtBase carry `Dirt` (modifier PlatformDirt). A block with a
    // terrain modifier but NO tag — DecoWallBaseGrass, DecoWallLoopEndGrass,
    // PlatformGrassBase, every PlatformPlastic*, the plastic checkpoints — wears
    // the modifier on its OWN prefab only; its clip panels stay the plain
    // material and take the placement colour: Summer 10's DecoWallBaseGrass
    // walls are GREEN-tinted TrackWall in the original where the folder's
    // TrackWall (grey DecoCliff) had been baked (same-camera frames own10 /
    // dc10, 2026-09-08). The tag is the whole difference between the two.
    // …EXCEPT a GAMEPLAY kind: a special pad's `<Kind>.TerrainModifier` (Turbo,
    // Turbo2, Boost, Reset, NoEngine, … — the folders that carry a `Sign`
    // material, the LED panel picture) dresses the pad's own side clips
    // (`PlatformSpecialFCLeft/Right`, the generic special skirts with the LED
    // strip) whether or not the pad carries a placement tag: Summer 16's
    // PlatformDirtSpecialTurbo2 (no tag) had its skirts baked with the prefab's
    // Turbo dress (yellow) where the pad is Turbo2 (red) — audit of 2026-09-08.
    let gameplay_folders: std::collections::HashSet<String> = store
        .entries()
        .filter_map(|e| e.path().strip_suffix("\\Sign.Material.Gbx").map(|s| s.to_uppercase()))
        .filter(|s| s.contains("\\MEDIA\\MODIFIER\\"))
        .collect();
    let terrain_mods = |bi: &crate::blockinfo::BlockInfo| -> Vec<String> {
        let refs = bi.material_modifier.iter().map(|r| r.replace(' ', "")).filter(|r| r.ends_with(".TerrainModifier.Gbx") || is_track_wall_to_deco_cliff(r));
        if bi.mat_modifier.is_none() {
            return refs.filter(|r| r.strip_suffix(".TerrainModifier.Gbx").map(|b| gameplay_folders.contains(&b.to_uppercase())).unwrap_or(false)).collect();
        }
        refs.collect()
    };
    let mut cell_mod: std::collections::HashMap<(u8, u8, u8), Vec<String>> = std::collections::HashMap::new();
    // (x, z) column -> [(y, is_pillar, mods)] for the pillar rule below
    let mut columns: std::collections::HashMap<(u8, u8), Vec<(u8, bool, Vec<String>)>> = std::collections::HashMap::new();
    for b in source.blocks.iter().filter(|b| b.flags & crate::blockmap::FLAG_FREE == 0) {
        let Some(path) = idx.path_for(&b.name) else { continue };
        let mods = match idx.load(store, &path) {
            Ok(bi) => terrain_mods(bi),
            Err(_) => Vec::new(),
        };
        let c = (b.file_cell[0], b.file_cell[1], b.file_cell[2]);
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
        let c = (b.file_cell[0], b.file_cell[1], b.file_cell[2]);
        // The block the clip BELONGS to first: a vertical clip is the wall on
        // its cell's side `dir`, completing the block ACROSS that side
        // (tmmaps::fillers) — Summer 20 cp3's DecoWallSlope2StraightVFCLeft
        // panels stand in the pillar cells and finish the DecoHillSlope2Straight
        // across, whose TrackWallToDecoCliff dresses them grey; the pillar's
        // cell gave them nothing and they came out red (2026-09-08).
        if let Some(a) = tmmaps::fillers::across(b.file_cell, b.dir) {
            if let Some(m) = cell_mod.get(&(a[0], a[1], a[2])) {
                return m.clone();
            }
        }
        // then the cell's own authored block (its tagged modifier, or none)
        if let Some(m) = cell_mod.get(&c) {
            return m.clone();
        }
        // Nothing on either side of the panel's own row: a MERGED panel
        // (`DecoWallBaseVFC` variants 5..10 = Middle×2/3/4/8/16/32) is recorded
        // in the bottom cell of its span and completes the blocks stacked above
        // — the first block up the ACROSS column decides, else the first up the
        // own column; nothing at all -> the plain material. (The old vote over
        // the vertical pair and the four horizontal neighbours dressed 10's
        // green-tinted b2412 as grey DecoCliff from a WaterGrassCornerOut one
        // cell below, which does not own it.)
        let first_up = |x: u8, z: u8| -> Option<Vec<String>> {
            (1..32u32).map(|dy| c.1 as u32 + dy).take_while(|y| *y <= 255).find_map(|y| cell_mod.get(&(x, y as u8, z)).cloned())
        };
        if let Some(a) = tmmaps::fillers::across(b.file_cell, b.dir) {
            if let Some(m) = first_up(a[0], a[2]) {
                return m;
            }
        }
        first_up(c.0, c.2).unwrap_or_default()
    };
    let mods_key = |mods: &[String]| -> String { mods.join("|") };

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
        let mk = mods_key(&inherited_mod(b));
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
        rows_by_key.entry((b.name.clone(), b.flags)).or_default().insert(b.file_cell[1]);
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
    // (every underwater vertex takes its source depth: the Beach apron is
    // only 0.8..3 m deep in the source and the sea over it reads as open sea
    // from 3 m down — see `restore_depth`)
    let mut deepened: Vec<String> = Vec::new();
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    // picture files (gate sign logos) the items name: added after the ident pass below (they are not GBX)
    let mut pictures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut outcomes: Vec<Outcome> = Vec::new();
    // key -> (alias or "-", footprint sx, sz, the variant's unit cells in the block's frame)
    let mut block_map: BTreeMap<(String, u32, String), (String, u32, u32, Vec<[i32; 3]>)> = BTreeMap::new();
    // key -> the picked variant's AUTO TERRAIN: the terrain tiles the block
    // brings with it, (offset in the block's frame, zone block name), and the
    // variant's place type. The game does not draw a tile that is a block's
    // own auto terrain — the block's ground prefab and FCBGround fillers are
    // the ground there (a RoadTechStart's Grass cell: TrackToDeco…FCBGround
    // is the apron; a StructurePillar's: StructurePillarFCBGround is the
    // plate). Summer 04's start straight and finish plaza stood on BAKED
    // Grass that the tiny drew as items under the decks, coplanar (2026-09-08).
    let mut auto_terrain: BTreeMap<(String, u32, String), (Vec<([i32; 3], String)>, i32)> = BTreeMap::new();
    let tile_zones: std::collections::BTreeSet<String> = source.genealogy_zones().into_iter().collect();
    let mut alias_of_recipe: BTreeMap<String, String> = BTreeMap::new();
    let mut next_alias = 0usize;
    // `v@ALIAS` rows (the prefabs' vegetation as stock items) and the
    // VegetTreeModel stem -> stock item cache behind them.
    let mut veget_rows = String::new();
    // the same rows structured, per alias in row order: (stock item, position in the item's scaled frame)
    let mut veget_list: BTreeMap<String, Vec<(String, [f32; 3])>> = BTreeMap::new();
    // alias -> up-facing collision triangles (item space, scaled) of a DECK block model (tree_clear)
    let mut deck_tris: BTreeMap<String, Vec<crate::tree_clear::Tri>> = BTreeMap::new();
    // deck-block alias -> block name (the tree_clear report)
    let mut deck_name: BTreeMap<String, String> = BTreeMap::new();
    let mut veget_cache: BTreeMap<String, Option<String>> = BTreeMap::new();
    // species -> trunk top (metres) for the sink; (model, variant, skin) key -> sink of re-pointed vegetation items
    let mut height_cache: BTreeMap<String, Option<f32>> = BTreeMap::new();
    let mut sink_map: BTreeMap<(String, u8, Option<String>), f32> = BTreeMap::new();
    let mut sunk_rows = 0usize;
    // the tree bake (`--veget bake`, the default): species -> half-size item
    let mut baker = TreeBaker::new(veget_mode);
    let substitute = veget_mode == "substitute" || veget_mode == "bake";
    let mut baked_tree_rows = 0usize;
    let ambient = source.ambient_zone().unwrap_or_default();
    for ((name, flags, inherited_mods), n) in &keys {
        if !wanted(name) {
            continue;
        }
        let key = BlockKey { name, flags: *flags, inherited_mods, placements: *n };
        let (path, bi) = match load_block_info(&mut idx, store, name) {
            Ok(x) => x,
            Err(e) => {
                outcomes.push(key.outcome("", key.source(), Err(e)));
                continue;
            }
        };
        let plan = match plan_block(&bi, &key, collection, &ambient, &tile_zones, &alias_of_recipe) {
            BlockPlan::Nothing { why, label, footprint } => {
                let (sx, sz, units) = match &footprint {
                    Some(f) => (f.sx, f.sz, f.units.clone()),
                    None => (1, 1, Vec::new()),
                };
                if let Some(Footprint { auto_terrain: Some(auto), .. }) = footprint {
                    auto_terrain.insert(key.map_key(), auto);
                }
                block_map.insert(key.map_key(), ("-".into(), sx, sz, units));
                let source = match label {
                    Some(l) => format!("{} [{l}]", key.source()),
                    None => key.source(),
                };
                outcomes.push(key.outcome("-", source, Ok(why)));
                continue;
            }
            BlockPlan::Refused { source, error } => {
                outcomes.push(key.outcome("", source, Err(error)));
                continue;
            }
            BlockPlan::Reuse { alias, footprint } => {
                if let Some(auto) = footprint.auto_terrain {
                    auto_terrain.insert(key.map_key(), auto);
                }
                block_map.insert(key.map_key(), (alias, footprint.sx, footprint.sz, footprint.units));
                continue;
            }
            BlockPlan::Bake(plan) => plan,
        };
        if let Some(auto) = plan.footprint.auto_terrain.clone() {
            auto_terrain.insert(key.map_key(), auto);
        }
        let alias = format!("AC{next_alias:08}");
        next_alias += 1;
        let ident = format!("{alias}.Item.Gbx");
        let at_water_row = water.map(|(wrow, _)| rows_by_key.get(&(name.clone(), *flags)).map(|r| r.len() == 1 && r.contains(&wrow)).unwrap_or(false)).unwrap_or(false);
        let res = bake_block(store, &plan, name, &path, &bi, &ident, scale, collection, &legacy, water, at_water_row);
        let (sx, sz, units) = (plan.footprint.sx, plan.footprint.sz, &plan.footprint.units);
        let label = &plan.pk.label;
        let recipe = &plan.recipe;
        match res {
            Ok((bytes, m, deepened_here)) => {
                if deepened_here {
                    deepened.push(key.source());
                }
                let nv = m.visuals.len();
                let veget = m.notes.iter().filter(|n| n.to_ascii_lowercase().contains(".vegettreemodel.gbx")).count();
                let other_skips = m.notes.iter().filter(|n| (n.contains("skipped") || n.contains("failed") || n.contains("unnamed")) && !n.to_ascii_lowercase().contains(".vegettreemodel.gbx")).count();
                let wp = match m.waypoint_type { Some(t) => format!(", waypoint {t} spawn {:?} trigger {}", m.spawn, m.trigger.is_some()), None => String::new() };
                // The prefab's vegetation entities become stock tree items placed
                // with the block (`v@ALIAS` rows: item, position in the item's
                // scaled frame, yaw), the species one step smaller like the
                // map's own vegetation. Only in `substitute` mode.
                let mut re_emitted = 0usize;
                if substitute {
                    for (p, iso) in &m.veget {
                        let yaw = (-iso[2]).atan2(iso[0]);
                        // a baked species: its half-size item at the scaled position, no sink
                        if let Some(tree) = baker.ident_for(store, p, scale, collection, &mut files, &mut pictures, &mut outcomes) {
                            veget_rows.push_str(&format!("v@{ident}\t{tree}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\n", iso[9] * scale, iso[10] * scale, iso[11] * scale, yaw));
                            veget_list.entry(ident.clone()).or_default().push((tree, [iso[9] * scale, iso[10] * scale, iso[11] * scale]));
                            re_emitted += 1;
                            baked_tree_rows += 1;
                            continue;
                        }
                        let Some((orig, item)) = veget_item_pair(store, collection, p, &mut veget_cache) else { continue };
                        let sink = veget_sink(store, &orig, &item, scale, &mut height_cache);
                        if sink > 0.0 {
                            sunk_rows += 1;
                        }
                        veget_rows.push_str(&format!("v@{ident}\t{item}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\n", iso[9] * scale, iso[10] * scale - sink, iso[11] * scale, yaw));
                        veget_list.entry(ident.clone()).or_default().push((item.clone(), [iso[9] * scale, iso[10] * scale - sink, iso[11] * scale]));
                        re_emitted += 1;
                    }
                }
                let summary = format!("{} bytes, {} visuals, {} collision tris, {} vegetation entities ({} re-emitted as items), {} other skips{wp}{}", bytes.len(), nv, m.surf_triangles.len(), veget, re_emitted, other_skips, lod_summary(&m));
                if nv == 0 {
                    outcomes.push(key.outcome(&alias, format!("{} [{label}] {recipe}", key.source()), Err(format!("no visuals ({summary}); notes: {}", m.notes.iter().take(3).cloned().collect::<Vec<_>>().join(" | ")))));
                    continue;
                }
                outcomes.push(key.outcome(&alias, format!("{} [{label}] {}", key.source(), plan.prefabs.iter().map(|p| p.0.rsplit('\\').next().unwrap_or(&p.0).to_string()).collect::<Vec<_>>().join("+")), Ok(summary)));
                files.insert(format!("Items/{ident}"), bytes);
                alias_of_recipe.insert(recipe.clone(), alias.clone());
                // a deck block's driving surface, for the tree clearance below
                if crate::tree_clear::is_deck_block(name) {
                    deck_tris.insert(ident.clone(), crate::tree_clear::up_facing(&m.surf_vertices, &m.surf_triangles));
                    deck_name.insert(ident.clone(), name.clone());
                }
                block_map.insert(key.map_key(), (alias, sx, sz, units.clone()));
            }
            // a prefab with no entities at all (Stadium\Structure\PillarToFlat_ACB
            // is one): the game draws nothing there either
            Err(e) if e.starts_with("no visuals: nothing to build") => {
                next_alias -= 1;
                block_map.insert(key.map_key(), ("-".into(), sx, sz, units.clone()));
                alias_of_recipe.insert(recipe.clone(), "-".into());
                outcomes.push(key.outcome("-", format!("{} [{label}] {recipe}", key.source()), Ok("empty prefab (no entities): intentionally no item".into())));
            }
            Err(e) => outcomes.push(key.outcome(&alias, format!("{} [{label}] {recipe}", key.source()), Err(e))),
        }
    }
    // item models — one library entry per (model, VARIANT): the placement's
    // variant byte picks which external of a variant-list item it shows
    // (Summer 11's `Show` rigs are 2 m stubs, 32 m beams, spot bars, speakers
    // and foggers of ONE item; a `PalmForest` placement's variant is its palm
    // species). Items whose file has no variant list share one entry.
    // …and per LIGHT COLOUR SKIN (light_skin.rs): a placement whose skin is
    // `Skins\Stadium\LightColors\Coral.dds` gets its own copy with coral
    // lights and glass (Summer 17: 108 Orange lamps; 20: 115 Green tubes).
    let footprint_cells = covered_cells(&source, &block_map, &tile_zones);
    // The flag driver guard (2026-09-08, anim thread). An embedded tween cloth
    // draws right only while a STOCK flag is drawn in the same view at the
    // same detail level (its tween draw borrows the stock's per-material frame
    // state), so `tmmaps tiny` hangs a stock flag upside down under every
    // converted flag placement (`TINY_FLAG_DRIVER`, TINY.md "Animated items").
    // That driver is a full-size 7.5 m flag reaching 15 m below the placement
    // in source metres; it must be HIDDEN: under the terrain (below is the
    // void, never seen) or inside a closed block. A placement whose two cells
    // below are neither terrain nor covered by an authored non-pillar block
    // (a flag on a deck over open air) gets an `xf@INDEX` row: no driver, its
    // cloth stays still (frame 0) rather than showing a stock flag in the air.
    let terrain_top: std::collections::HashMap<(u8, u8), u8> = {
        let mut m: std::collections::HashMap<(u8, u8), u8> = std::collections::HashMap::new();
        for b in source.blocks.iter().chain(source.baked.iter()).filter(|b| b.flags & crate::blockmap::FLAG_FREE == 0 && tile_zones.contains(&b.name)) {
            let e = m.entry((b.file_cell[0], b.file_cell[2])).or_insert(0);
            *e = (*e).max(b.file_cell[1]);
        }
        m
    };
    let ground_y = tmmaps::map::ground_y(collection);
    let driver_hidden = |it: &tmmaps::map::ItemRec| -> bool {
        // the FILE cell (game cell + (1, 0, 1), see BlockRec::file_cell)
        let cx = (it.pos[0] / 32.0).floor() as i64 + 1;
        let cz = (it.pos[2] / 32.0).floor() as i64 + 1;
        let cy_base = ((it.pos[1] - ground_y) / 8.0).floor() as i64;
        if !(0..=255).contains(&cx) || !(0..=255).contains(&cz) {
            return false;
        }
        let top = terrain_top.get(&(cx as u8, cz as u8)).map(|t| *t as i64);
        (1..=2).all(|d| {
            let cy = cy_base - d;
            top.map(|t| cy <= t).unwrap_or(false) || (0..=255).contains(&cy) && footprint_cells.contains(&[cx as u8, cy as u8, cz as u8])
        })
    };
    let light_skin_of = |it: &tmmaps::map::ItemRec| -> Option<String> { it.skin(&source.gbx.body).and_then(|f| crate::light_skin::skin_name(&f.path)) };
    // A converted flag placement with nowhere to hide its driver gets the
    // STILL cloth (frame 0 under ItemFlagNoAnim) — a tween cloth with no stock
    // flag drawn in view is a bare pole. The still copy is a variant of its
    // own under the pseudo light-skin key `still` (baked with the tween off).
    let is_flag = |it: &tmmaps::map::ItemRec| matches!(it.model.as_str(), "Flag16m" | "Flag8m");
    let key_skin_of = |it: &tmmaps::map::ItemRec| -> Option<String> {
        if is_flag(it) && crate::static_item::build::tween_parts_enabled() && !driver_hidden(it) {
            Some(STILL_FLAG_KEY.to_string())
        } else {
            light_skin_of(it)
        }
    };
    let mut item_counts: BTreeMap<(String, u8, Option<String>), usize> = BTreeMap::new();
    for it in &source.items {
        *item_counts.entry((it.model.clone(), it.variant(), key_skin_of(it))).or_insert(0) += 1;
    }
    // (model, variant, light skin) -> new model name: an embedded alias (AI...Item.Gbx) or a stock species
    let mut item_map: BTreeMap<(String, u8, Option<String>), String> = BTreeMap::new();
    // the map's own embedded files (custom items live under Items\…)
    let embedded: BTreeMap<String, Vec<u8>> = crate::embedded::files(&source).unwrap_or_default();
    let mut item_alias_n = 0usize;
    // a model without a variant list is built once; later variants reuse it
    let mut single_variant: BTreeMap<String, String> = BTreeMap::new();
    // stock half-size variants used as targets: their mapping rows carry
    // model_scale = scale like an embedded half-size copy
    let mut half_stock: std::collections::BTreeSet<String> = Default::default();
    for ((model, variant, lskin), n) in &item_counts {
        if model.is_empty() || !wanted(model) {
            continue;
        }
        // (Every item model bakes — the club's custom nation items included:
        // until 2026-09-08 the `TME\` items were left out on the belief that
        // they carried custom textures the bake could not embed. They are
        // mesh-modeler items in the game's `Material_BlockCustom` materials
        // with a `TargetColor` constant per part plus a bare modeler link, and
        // `Merged::material_inst_slot` carries both now.)
        if lskin.is_none() {
            if let Some(target) = single_variant.get(model) {
                item_map.insert((model.clone(), *variant, None), target.clone());
                continue;
            }
        }
        // the STILL copy of a converted flag (pseudo skin key, see `key_skin_of`)
        let still_flag = lskin.as_deref() == Some(STILL_FLAG_KEY);
        let light_skin = match lskin {
            Some(_) if still_flag => None,
            Some(name) => match crate::light_skin::lookup(name) {
                Some(s) => Some(s),
                None => {
                    outcomes.push(Outcome { alias: String::new(), kind: "item", source: format!("{model} skin {name}"), placements: *n, result: Err(format!("light skin {name}: not one of the game's LightColors swatches")) });
                    continue;
                }
            },
            None => None,
        };
        // Stock HALF-SIZE variants (2026-09-07): Nadeo ships every screen in a
        // `Small` version that is exactly half in both dimensions
        // (RaceScreen6x1 24×4 m → RaceScreen6x1Small 12×2 m; Screen2x3Big →
        // Screen2x3 → Screen2x3Small). A stock item keeps the game's own
        // behaviour — the live in-game advertisement on its panel — which no
        // baked copy can have (the skin remap needs the model's own texture
        // file). The mapping row carries model_scale = scale so the placement
        // is treated like a half-size copy (scale 1, pivot halved).
        // (The gates have no such twin, see `stock_half_variant`.)
        if let Some(small) = stock_half_variant(model, *variant) {
            if let Some(logical) = find_item_file(store, small) {
                // The placement must carry the item's OWN ident, case-exact: the
                // pack stores `Stadium\Items\ShowFogger8M.Item.Gbx` whose header
                // says `ShowFogger8m`, and the game resolves a stock ident by the
                // exact string — a `ShowFogger8M` placement is silently DROPPED
                // (tiny 02, 2026-09-08: four foggers gone, no dialog). Every
                // stand-in name is read back from its file here.
                let ident = store.read(&logical).ok().and_then(|b| tmmaps::header::item_ident_author(&b)).map(|(id, _)| id);
                let small: &str = match ident.as_deref() {
                    Some(id) if id != small => {
                        eprintln!("  stock stand-in {small}: the item file's ident is `{id}` (pack path {logical}); the placement gets the ident");
                        id
                    }
                    _ => small,
                };
                let key = (model.clone(), *variant, lskin.clone());
                // a stand-in for the model as a whole is remembered for its
                // later variants; one for a single variant (`Show` 28, the
                // fogger rig) leaves the others to the bake
                if stock_half_variant(model, 0) == Some(small) {
                    single_variant.insert(model.clone(), small.to_string());
                }
                item_map.insert(key, small.to_string());
                half_stock.insert(small.to_string());
                let why = match small {
                    "Flag8m" => "its cloth waves under the game's own vertex tween",
                    "ShowFogger8m" | "ShowFoggerWithLight8m" => "its smoke is the game's own particle system, at half reach",
                    "Sparkler8m" if model == "Sparkler8m" => "kept as the stock item, its sparks are the game's own particle system at their full 8 m reach (no 4 m sibling exists)",
                    "Sparkler8m" => "its sparks are the game's own particle system, at half reach",
                    "ShowTorchSmall" => "its flame is the game's own particle system, on the small torch",
                    _ => "its screen keeps the live advertisement",
                };
                // the report names the variant when only that one stands in
                let source = if stock_half_variant(model, 0) == Some(small) { model.clone() } else { format!("{model} v{variant}") };
                outcomes.push(Outcome { alias: small.to_string(), kind: "item", source, placements: *n, result: Ok(format!("stock half-size variant {small}: the game's own item, {why}")) });
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
                        {
                            if still_flag {
                                crate::static_item::build::TWEEN_OVERRIDE.with(|o| o.set(Some(false)));
                            }
                            let r = crate::static_item::build::static_item_from_pack_item_report_skin(store, &logical, &ident, &ident, scale, collection, *variant as usize, light_skin.clone());
                            crate::static_item::build::TWEEN_OVERRIDE.with(|o| o.set(None));
                            r
                        }
                    }
                    None => Err("no .Item.Gbx in the client packs, the map's embedded files, or --items-dir".into()),
                },
            },
        };
        let multi = variants.len() > 1;
        let mut source_name = if multi {
            let picked = variants.get(*variant as usize).or(variants.first()).map(|p| p.rsplit('\\').next().unwrap_or(p).to_string()).unwrap_or_default();
            format!("{model} v{variant} ({picked})")
        } else {
            model.clone()
        };
        if let Some(name) = lskin {
            source_name.push_str(&if still_flag { " still cloth (no place to hide a stock driver)".to_string() } else { format!(" skin {name}") });
        }
        let key = (model.clone(), *variant, lskin.clone());
        let mut remember = |target: &str| {
            if !multi && lskin.is_none() {
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
            Ok((_, m)) if !m.veget.is_empty() && substitute => {
                let mut placed = 0usize;
                for (p, iso) in &m.veget {
                    let yaw = (-iso[2]).atan2(iso[0]);
                    if let Some(tree) = baker.ident_for(store, p, scale, collection, &mut files, &mut pictures, &mut outcomes) {
                        veget_rows.push_str(&format!("v@{model}\t{tree}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\n", iso[9] * scale, iso[10] * scale, iso[11] * scale, yaw));
                        veget_list.entry(model.clone()).or_default().push((tree, [iso[9] * scale, iso[10] * scale, iso[11] * scale]));
                        placed += 1;
                        baked_tree_rows += 1;
                        continue;
                    }
                    let Some((orig, item)) = veget_item_pair(store, collection, p, &mut veget_cache) else { continue };
                    let sink = veget_sink(store, &orig, &item, scale, &mut height_cache);
                    if sink > 0.0 {
                        sunk_rows += 1;
                    }
                    veget_rows.push_str(&format!("v@{model}\t{item}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\n", iso[9] * scale, iso[10] * scale - sink, iso[11] * scale, yaw));
                    veget_list.entry(model.clone()).or_default().push((item.clone(), [iso[9] * scale, iso[10] * scale - sink, iso[11] * scale]));
                    placed += 1;
                }
                remember("-");
                item_map.insert(key, "-".into());
                outcomes.push(Outcome { alias: "-".into(), kind: "item", source: source_name, placements: *n, result: Ok(format!("vegetation cluster: {} of {} trees re-emitted as stock items per placement", placed, m.veget.len())) });
            }
            Ok((_, m)) => outcomes.push(Outcome { alias: String::new(), kind: "item", source: source_name, placements: *n, result: Err(format!("no visuals; notes: {}", m.notes.iter().take(3).cloned().collect::<Vec<_>>().join(" | "))) }),
            Err(e) if e.contains("procedural vegetation") => match if substitute { "substitute" } else { veget_mode } {
                "substitute" => {
                    // the variant names the SPECIES (`…\PalmTreeBigB1.VegetTreeModel.Gbx`):
                    // the stock item of that species one size down; else the
                    // collection ladder for the item's own name
                    let species = e.split_once("procedural vegetation: ").and_then(|(_, rest)| rest.split(" (").next()).filter(|p| p.to_ascii_lowercase().ends_with(".vegettreemodel.gbx")).map(|s| s.to_string());
                    // a baked species: the placement is re-pointed at its half-size item
                    // (model_scale = scale like any embedded copy), no sink
                    let species_or_item = species.clone().or_else(|| find_item_file(store, model));
                    if let Some(tree) = species_or_item.as_deref().and_then(|p| baker.ident_for(store, p, scale, collection, &mut files, &mut pictures, &mut outcomes)) {
                        remember(&tree);
                        item_map.insert(key, tree.clone());
                        baked_tree_rows += *n;
                        outcomes.push(Outcome { alias: tree, kind: "item", source: source_name, placements: *n, result: Ok("vegetation: baked half-size (see the tree row)".into()) });
                        continue;
                    }
                    let by_species = species.as_deref().and_then(|p| veget_item_pair(store, collection, p, &mut veget_cache));
                    let by_name = || veget_substitute(collection, model).filter(|s| find_item_file(store, s).is_some()).map(|s| (model.clone(), s.to_string()));
                    match by_species.or_else(by_name) {
                        Some((orig, sub)) => {
                            let how = if species.is_some() { "species of this variant" } else { "the item's own name" };
                            // the stand-in keeps its full height: sunk so its trunk top
                            // sits where the original's would at the tiny scale
                            let sink = veget_sink(store, &orig, &sub, scale, &mut height_cache);
                            if sink > 0.0 {
                                sink_map.insert(key.clone(), sink);
                            }
                            remember(&sub);
                            item_map.insert(key, sub.clone());
                            outcomes.push(Outcome { alias: sub.to_string(), kind: "item", source: source_name, placements: *n, result: Ok(format!("vegetation: re-pointed at stock {sub} by {how} (placement scale is ignored by the game), sunk {sink:.1} m")) });
                        }
                        None => {
                            // the species itself stays: sunk by half its own height
                            let sink = veget_sink(store, model, model, scale, &mut height_cache);
                            if sink > 0.0 {
                                sink_map.insert(key.clone(), sink);
                            }
                            remember(model);
                            item_map.insert(key, model.clone());
                            outcomes.push(Outcome { alias: model.clone(), kind: "item", source: source_name, placements: *n, result: Ok(format!("vegetation: already a small species, kept, sunk {sink:.1} m")) });
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
    let mut mapping = String::from("# tiny-library mapping: @block_index<TAB>ITEM|-<TAB>model_scale<TAB>sx<TAB>sz<TAB>units(x,y,z;...)<TAB>auto_terrain(dx,dy,dz=Zone;...|placetype) ; i@item_index<TAB>ITEM|stock model|-\n");
    let mut missing_blocks: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows = 0usize;
    // TINY_DROP_BAKED=glob,glob names generated fillers left out of the map
    // wholesale (`-` / unset: none). ⚠ HACK KNOB — a hand-written deletion
    // list, never a rule of the game's. Its former DEFAULT, `DecoWall*VFC*`
    // (d668446, 2026-09-07, after Summer 20 cp3's "big bar": eleven such
    // fillers in the DecoPlatform slope and water cells drew a bar the
    // original does not show), deleted EVERY DecoWall vertical clip of every
    // map — and those clips ARE the pillars: a `DecoWallBasePillar` block has
    // no prefab of its own, its four walls are the `DecoWallBaseVFC` pieces
    // the game generates into the neighbouring cells (variant word: 0 Middle,
    // 1 Top, 2 Bottom, 3 TopBottom, 4 nothing/covered, 5..10 Middle x2/3/4/8/
    // 16/32; ground 0/1 Bottom/TopBottom_Ground). Summer 10 lost its 2 864
    // pillars' walls (1 429 VFC records), 05 its 1 210 — the "road block at
    // the start", the "hollow platforms", the "missing undersides" vjeux
    // drove into on 2026-09-08 (see `tmmaps fillers MAP --summary`).
    let drop_baked: Vec<String> = std::env::var("TINY_DROP_BAKED").unwrap_or_default().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s != "-").collect();
    // Which of the game's recorded fillers are emitted: ALL OF THEM. Settled
    // 2026-09-09 from the engine (Trackmania.exe InitChallengeData_Clips, see
    // `crate::bake`): the client regenerates every free clip from the authored
    // blocks with the very algorithm the editor baked the file's records with,
    // so the baked list is exactly what the game draws — `mapgeom bake MAP
    // --diff` finds 0 stale records on all 25 Summer 2026 maps. The fitted
    // hiding rules that lived here until a1b91d23 (fullfree / face / covered /
    // occupied / accepted / ghost / free, TINY_FILLER_RULE) each hid pieces the
    // game draws (Summer 15's arch floor and pool-approach plate, 21's gate
    // floor, 05's water-road floor); what an original does not SHOW at a
    // record's position is occluded by neighbouring geometry, to be matched at
    // the geometry level. The bake is run here as the invariant: a record the
    // engine would not keep is reported (and, with TINY_BAKE_STRICT=1, fatal).
    if std::env::var("TINY_FILLER_RULE").is_ok() || std::env::var("TINY_VFC_RULE").is_ok() {
        println!("  ⚠ TINY_FILLER_RULE/TINY_VFC_RULE are gone: every generated filler the game draws is emitted (a1b91d23); the variable is ignored");
    }
    {
        let faces = crate::fillers::faces(store, &mut idx, &source);
        let dirs: std::collections::HashMap<usize, u8> = source.blocks.iter().map(|b| (b.index, b.dir)).collect();
        let grounds = crate::bake::record_grounds(&faces, &source);
        let clips = crate::bake::simulate(&faces, &dirs, &grounds);
        let d = crate::bake::diff(&clips, &faces, &source);
        println!("  engine bake check: {} generated records confirmed drawn, {} the engine would not keep, {} clips the engine draws without a record", d.confirmed, d.stale.len(), d.missing.len());
        for (i, name, why) in d.stale.iter().take(12) {
            println!("    ⚠ stale b{i} {name}: {why}");
        }
        if !d.stale.is_empty() && std::env::var("TINY_BAKE_STRICT").map(|v| v == "1").unwrap_or(false) {
            eprintln!("{} generated records the engine would not draw (TINY_BAKE_STRICT=1)", d.stale.len());
            std::process::exit(3);
        }
    }
    let glob_match = |pat: &str, name: &str| -> bool {
        // `*` matches any run; anchored at both ends
        let parts: Vec<&str> = pat.split('*').collect();
        if parts.len() == 1 {
            return pat == name;
        }
        let mut rest = name;
        for (i, p) in parts.iter().enumerate() {
            if i == 0 {
                if !rest.starts_with(p) {
                    return false;
                }
                rest = &rest[p.len()..];
            } else if i == parts.len() - 1 {
                return rest.ends_with(p);
            } else if let Some(at) = rest.find(p) {
                rest = &rest[at + p.len()..];
            } else {
                return false;
            }
        }
        true
    };
    let mut dropped_baked: BTreeMap<String, usize> = BTreeMap::new();
    // The tree clearance (tree_clear.rs): every deck placement's driving
    // surface and every tree, in the scaled source frame, placed the way
    // `tmmaps tiny` places them.
    tmmaps::tiny::set_ground(collection);
    let mut grid = crate::tree_clear::Grid::new();
    let mut trees: Vec<crate::tree_clear::Tree> = Vec::new();
    let mut dims_cache: BTreeMap<String, Option<(f32, f32)>> = BTreeMap::new();
    let mut deck_placements = 0usize;
    for (prefix, b) in source.blocks.iter().map(|b| ("@", b)).chain(source.baked.iter().filter(|b| b.name != "Sea").map(|b| ("b@", b))) {
        if prefix == "b@" && drop_baked.iter().any(|g| glob_match(g, &b.name)) {
            mapping.push_str(&format!("b@{}\t-\n", b.index));
            *dropped_baked.entry(b.name.clone()).or_insert(0) += 1;
            rows += 1;
            continue;
        }
        let inherited_mods = if prefix == "b@" { baked_key.get(&b.index).cloned().unwrap_or_default() } else { String::new() };
        match block_map.get(&(b.name.clone(), b.flags, inherited_mods.clone())) {
            Some((alias, sx, sz, units)) => {
                let model = if alias == "-" { "-".to_string() } else { format!("{alias}.Item.Gbx") };
                // the unit cells, so `tmmaps tiny` can hide the terrain tile under EVERY
                // cell a ground deck covers (a Curve5 kept the Grass tiles of its 12
                // other cells at deck height: the physics read Grass on the road, 2026-09-07)
                let cells = units.iter().map(|u| format!("{},{},{}", u[0], u[1], u[2])).collect::<Vec<_>>().join(";");
                // 7th field: the variant's auto terrain, `dx,dy,dz=Zone;…|placetype`
                // (empty when the variant declares none) — `tmmaps tiny` hides a
                // tile the block declares as its own ground, authored or baked
                let auto = match auto_terrain.get(&(b.name.clone(), b.flags, inherited_mods.clone())) {
                    Some((list, place)) => format!("{}|{place}", list.iter().map(|(o, z)| format!("{},{},{}={z}", o[0], o[1], o[2])).collect::<Vec<_>>().join(";")),
                    None => String::new(),
                };
                mapping.push_str(&format!("{prefix}{}\t{}\t{}\t{}\t{}\t{}\t{}\n", b.index, model, scale, sx, sz, cells, auto));
                rows += 1;
                if alias != "-" {
                    // where `tmmaps tiny` puts this item: origin (source metres) and yaw
                    let origin = tmmaps::tiny::block_origin(b, (*sx, *sz));
                    let origin = [origin[0] * scale, origin[1] * scale, origin[2] * scale];
                    let yaw = b.free_rot.map(|r| r[0]).unwrap_or_else(|| tmmaps::tiny::block_yaw(b));
                    let key = format!("{prefix}{}", b.index);
                    if let Some(tris) = deck_tris.get(&model) {
                        crate::tree_clear::add_deck(&mut grid, &crate::tree_clear::Deck { alias: model.clone(), name: deck_name.get(&model).cloned().unwrap_or_default(), key: key.clone(), origin, yaw, tris });
                        deck_placements += 1;
                    }
                    if let Some(list) = veget_list.get(&model) {
                        let (s, c) = yaw.sin_cos();
                        for (k, (item, local)) in list.iter().enumerate() {
                            let Some((radius, height)) = baker.dims.get(item).copied().or_else(|| crate::tree_clear::species_dims(store, item, &mut dims_cache)) else { continue };
                            let pos = [origin[0] + local[0] * c + local[2] * s, origin[1] + local[1], origin[2] - local[0] * s + local[2] * c];
                            let row = if prefix == "b@" { format!("xvb@{}\t{k}", b.index) } else { format!("xv@{}\t{k}", b.index) };
                            trees.push(crate::tree_clear::Tree { row, species: item.clone(), pos, radius, height, owner: format!("{} {} #{k}", b.name, model), from: key.clone() });
                        }
                    }
                }
            }
            None => *missing_blocks.entry(format!("{} {:08X}", b.name, b.flags)).or_insert(0) += 1,
        }
    }
    let mut missing_items: BTreeMap<String, usize> = BTreeMap::new();
    let mut driver_skipped: Vec<usize> = Vec::new();
    let mut drivers = 0usize;
    // TINY_DROP_ITEMS=glob,glob: source ITEMS whose model matches are left out of
    // the map (`i@N<TAB>-`). ⚠ DIAGNOSTIC KNOB, never a rule: the player
    // project's "Summer 15 without its 35 moving obstacles" build (2026-09-08),
    // to tell whether a fork-vs-validator disagreement is the pushers' clock.
    let drop_items: Vec<String> = std::env::var("TINY_DROP_ITEMS").unwrap_or_default().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s != "-").collect();
    let mut dropped_items: BTreeMap<String, usize> = BTreeMap::new();
    for it in &source.items {
        if drop_items.iter().any(|g| glob_match(g, &it.model)) {
            mapping.push_str(&format!("i@{}\t-\n", it.index));
            *dropped_items.entry(it.model.clone()).or_insert(0) += 1;
            rows += 1;
            continue;
        }
        match item_map.get(&(it.model.clone(), it.variant(), key_skin_of(it))) {
            Some(target) => {
                let ms = if target.ends_with(".Item.Gbx") || half_stock.contains(target) { scale } else { 1.0 };
                mapping.push_str(&format!("i@{}\t{}\t{}\n", it.index, target, ms));
                rows += 1;
                // a converted flag (our tween cloth): does its hidden stock driver fit?
                if is_flag(it) && target.ends_with(".Item.Gbx") && crate::static_item::build::tween_parts_enabled() {
                    if driver_hidden(it) {
                        drivers += 1;
                    } else {
                        mapping.push_str(&format!("xf@{}\n", it.index));
                        driver_skipped.push(it.index);
                    }
                }
                // `iv@INDEX<TAB>0`: a stock stand-in has its own variant list —
                // the placement's byte (an index into the SOURCE model's) is
                // rewritten to 0 (`Show` 28 → `ShowFogger8M`'s only variant)
                if half_stock.contains(target) && it.variant() != 0 {
                    mapping.push_str(&format!("iv@{}\t0\n", it.index));
                }
                // `y@INDEX<TAB>DY`: the vegetation stand-in is lowered by DY metres
                let sink = sink_map.get(&(it.model.clone(), it.variant(), key_skin_of(it))).copied();
                if let Some(sink) = sink {
                    mapping.push_str(&format!("y@{}\t{:.3}\n", it.index, sink));
                    sunk_rows += 1;
                }
                // a stock tree standing in for the map's own vegetation item, or a baked one
                if target != "-" && (!target.ends_with(".Item.Gbx") || baker.dims.contains_key(target)) {
                    if let Some((radius, height)) = baker.dims.get(target).copied().or_else(|| crate::tree_clear::species_dims(store, target, &mut dims_cache)) {
                        let pos = [it.pos[0] * scale, it.pos[1] * scale - sink.unwrap_or(0.0), it.pos[2] * scale];
                        trees.push(crate::tree_clear::Tree { row: format!("xi@{}", it.index), species: target.clone(), pos, radius, height, owner: format!("item {} {}", it.index, it.model), from: format!("i@{}", it.index) });
                    }
                }
                // a vegetation CLUSTER item's trees (`v@<model>` rows, placed at the item)
                if target == "-" {
                    if let Some(list) = veget_list.get(&it.model) {
                        let (s, c) = it.yaw.sin_cos();
                        let origin = [it.pos[0] * scale, it.pos[1] * scale, it.pos[2] * scale];
                        for (k, (item, local)) in list.iter().enumerate() {
                            let Some((radius, height)) = baker.dims.get(item).copied().or_else(|| crate::tree_clear::species_dims(store, item, &mut dims_cache)) else { continue };
                            let pos = [origin[0] + local[0] * c + local[2] * s, origin[1] + local[1], origin[2] - local[0] * s + local[2] * c];
                            trees.push(crate::tree_clear::Tree { row: format!("xvi@{}\t{k}", it.index), species: item.clone(), pos, radius, height, owner: format!("cluster item {} {} #{k}", it.index, it.model), from: format!("i@{}", it.index) });
                        }
                    }
                }
            }
            None => *missing_items.entry(it.model.clone()).or_insert(0) += 1,
        }
    }
    mapping.push_str(&veget_rows);
    // the verdicts: `xv@N\tK` / `xvb@N\tK` / `xvi@N\tK` / `xi@N` rows, one per dropped tree
    let verdict = crate::tree_clear::judge(&grid, &trees);
    // A BAKED tree is judged for the census only: at half size in a half-size
    // place it meets a deck exactly when the original did — the verdicts are
    // printed, not written.
    let is_baked = |t: &crate::tree_clear::Tree| baker.dims.contains_key(&t.species);
    let baked_dropped = verdict.dropped.iter().filter(|(t, _, _)| is_baked(t)).count();
    let baked_tested = trees.iter().filter(|t| is_baked(t)).count();
    for (t, _, _) in &verdict.dropped {
        if is_baked(t) {
            continue;
        }
        mapping.push_str(&t.row);
        mapping.push('\n');
    }
    if baked_tested > 0 {
        println!("  baked trees: {baked_tested} judged against the decks, {baked_dropped} would be dropped (census only, none dropped)");
        if baked_dropped > 0 {
            let mut by_species: BTreeMap<&str, usize> = BTreeMap::new();
            for (t, _, _) in verdict.dropped.iter().filter(|(t, _, _)| is_baked(t)) {
                *by_species.entry(t.species.as_str()).or_default() += 1;
            }
            println!("    baked by species: {}", by_species.iter().map(|(k, n)| format!("{k} x{n}")).collect::<Vec<_>>().join(", "));
            if crate::debug::on("trees") {
                for (t, owner, y) in verdict.dropped.iter().filter(|(t, _, _)| is_baked(t)) {
                    println!("    baked {}\t{} {} r {:.1} h {:.1} at {:.1},{:.1},{:.1} — {} at y {:.1}", t.row.replace('\t', " "), t.species, t.owner, t.radius, t.height, t.pos[0], t.pos[1], t.pos[2], owner, y);
                }
            }
        }
    }
    {
        let mut by_species: BTreeMap<&str, usize> = BTreeMap::new();
        let mut by_owner: BTreeMap<&str, usize> = BTreeMap::new();
        for (t, owner, _) in &verdict.dropped {
            *by_species.entry(t.species.as_str()).or_default() += 1;
            *by_owner.entry(owner.as_str()).or_default() += 1;
        }
        println!("  tree clearance: {} trees tested against {} up-facing triangles of {} deck placements; {} DROPPED (overlapping a deck), {} kept", trees.len(), grid.len(), deck_placements, verdict.dropped.len(), verdict.kept);
        if !verdict.dropped.is_empty() {
            let mut sp: Vec<_> = by_species.into_iter().collect();
            sp.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            println!("    by species: {}", sp.iter().map(|(k, n)| { let d = dims_cache.get(*k).copied().flatten().unwrap_or((0.0, 0.0)); format!("{k} x{n} (r {:.1} h {:.1})", d.0, d.1) }).collect::<Vec<_>>().join(", "));
            let mut ow: Vec<_> = by_owner.into_iter().collect();
            ow.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            println!("    decks hit most: {}", ow.iter().take(6).map(|(k, n)| format!("{k} x{n}")).collect::<Vec<_>>().join(", "));
            // the worst spot: the dropped tree with the most dropped neighbours within 24 m
            let mut best = (0usize, [0.0f32; 3]);
            for (t, _, _) in &verdict.dropped {
                let n = verdict.dropped.iter().filter(|(u, _, _)| (u.pos[0] - t.pos[0]).powi(2) + (u.pos[2] - t.pos[2]).powi(2) < 24.0 * 24.0).count();
                if n > best.0 {
                    best = (n, t.pos);
                }
            }
            println!("    worst spot: {} dropped trees within 24 m of scaled-source {:.1},{:.1},{:.1} (source {:.1},{:.1},{:.1})", best.0, best.1[0], best.1[1], best.1[2], best.1[0] / scale, best.1[1] / scale, best.1[2] / scale);
            if crate::debug::on("trees") {
                for (t, owner, y) in &verdict.dropped {
                    println!("    drop {}\t{} {} r {:.1} h {:.1} at {:.1},{:.1},{:.1} — {} at y {:.1}", t.row.replace('\t', " "), t.species, t.owner, t.radius, t.height, t.pos[0], t.pos[1], t.pos[2], owner, y);
                }
            }
        }
    }
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
        println!("  sea floor at source depth under {} shore tile models at the water row: {}", deepened.len(), deepened.join(", "));
    }
    println!("  mapping: {} rows -> {} ({} vegetation placements sunk to half-tree crown height; {} tree placements on {} baked half-size species, {} KB of items + {} KB of textures)", rows, out_mapping.display(), sunk_rows, baked_tree_rows, baker.next, baker.item_bytes / 1024, baker.texture_bytes / 1024);
    if drivers + driver_skipped.len() > 0 {
        println!("  ⚠ HACK hidden stock flag driver per converted flag placement (our tween cloth borrows the frame state of a drawn stock tween; TINY.md \"Animated items\"): {drivers} drivers, {} placements left still (deck over open air, no place to hide one){}", driver_skipped.len(), if driver_skipped.is_empty() { String::new() } else { format!(": items {}", driver_skipped.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(" ")) });
    }
    if !dropped_items.is_empty() {
        println!("  ⚠ HACK TINY_DROP_ITEMS: {} source items left out: {}", dropped_items.values().sum::<usize>(), dropped_items.iter().map(|(k, v)| format!("{k} x{v}")).collect::<Vec<_>>().join(", "));
    }
    if !dropped_baked.is_empty() {
        println!("  ⚠ HACK baked fillers left out by name (TINY_DROP_BAKED): {}", dropped_baked.iter().map(|(k, v)| format!("{k} x{v}")).collect::<Vec<_>>().join(", "));
    }
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
        // (`y@` sink rows, `iv@` variant rows and the `xv@`/`xvb@`/`xvi@` clearance rows carry a number, not a model)
        if head.starts_with('#') || head.starts_with("y@") || head.starts_with("iv@") || head.starts_with("xv") || model == "-" || model.ends_with(".Item.Gbx") {
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

/// The folder `Stadium\Media\Modifier\TrackWallToDecoCliff.Gbx` names in its
/// chunk 0x0915D000 (read off the file: the string is the PlatformGrass
/// folder, the skin ref `TrackWallToDecoCliff.GameSkin.gbx` — a skin the pack
/// does not ship, so the slot set is taken from its name: TrackWall).
pub const TRACK_WALL_TO_DECO_CLIFF_FOLDER: &str = "Stadium\\Media\\Modifier\\PlatformGrass\\";

/// A block info's `…\Modifier\TrackWallToDecoCliff.Gbx` material modifier ref
/// (DecoHill*, DecoPlatformBase, PlatformTechBase, WaterBase, WaterWall,
/// DecoCliff*…). The generated pillars carry `TrackWallFromParent.Gbx`
/// instead, which the pillar rule above resolves through the parent block.
pub fn is_track_wall_to_deco_cliff(r: &str) -> bool {
    r.trim_end().replace(' ', "").to_ascii_lowercase().ends_with("\\trackwalltodecocliff.gbx")
}

/// `Stadium\Media\Modifier\Reset.TerrainModifier .Gbx` -> `Stadium\Media\Modifier\Reset`,
/// with or without Nadeo's space before the extension (the Reset blocks' infos
/// and the pack entry itself spell it with one — 93acd7bb strips it before the
/// folder lookup; this accepts either spelling). Anything that is not a
/// terrain modifier (the parent block info, a `TrackWallToDecoCliff.Gbx` game
/// skin) is None.
pub fn terrain_modifier_base(r: &str) -> Option<&str> {
    let t = r.trim_end();
    let t = t.strip_suffix(".Gbx").or_else(|| t.strip_suffix(".gbx"))?;
    t.trim_end().strip_suffix(".TerrainModifier")
}

/// What a block's modifier does to the prefab's COLLISION materials — the
/// game's own mechanism, read from the data instead of guessed by stem:
/// `X.TerrainModifier.Gbx` names a GameSkin (`Specials.GameSkin.gbx` for
/// Reset, `SpecialsOriented` for Boost, `Platform` for PlatformGrass…) whose
/// entries are `slot name = the pack material the prefab is authored with`,
/// and the modifier's folder holds a `<slot>.Material.Gbx` for the slots it
/// re-dresses. One row per `Collision*` slot whose folder file exists:
/// (default material path, lower-cased; replacement link; its (physics,
/// gameplay) surface ids). `Effects\Media\Material\CollisionTurboGreen.Material.Gbx`
/// -> slot `CollisionGrass` -> `Stadium\Media\Modifier\Boost\CollisionGrass` =
/// (Green 76, ReactorBoost 12); `CollisionTurbo` -> `Collision` ->
/// `Modifier\Reset\Collision` = (Concrete 0, Reset 8). The platform-special
/// prefabs are authored in their Turbo dress (gameplay 1 on the deck hull), so
/// until this table was applied to the hull every tiny Boost/Reset/NoEngine
/// PLATFORM drove as a Turbo (Summer 24 cp10, 2026-09-08: a Reset slope) —
/// the gate ITEMS were already re-dressed through their prefab trigger entity
/// (`special_collision_ids`). The modifier file is loaded under the pack's
/// own spelling: the ref as given, else with Nadeo's space put back.
pub fn modifier_collision_redress(store: &mut DataStore, refs: &[String]) -> Vec<Redress> {
    let mut out: Vec<Redress> = Vec::new();
    for r in refs {
        let Some(base) = terrain_modifier_base(r) else { continue };
        let folder = format!("{base}\\");
        let spaced = format!("{base}.TerrainModifier .Gbx");
        let Ok(model) = store.load_model(r).or_else(|_| store.load_model(&spaced)) else { continue };
        let Some(skin_path) = model.externals.iter().map(|(_, p)| p.clone()).find(|p| p.to_ascii_lowercase().ends_with(".gameskin.gbx")) else { continue };
        let Ok(bytes) = store.read(&skin_path) else { continue };
        let Some(chunk) = tmmaps::header::game_skin_chunk(&bytes) else { continue };
        let Some(skin) = tmmaps::header::GameSkin::decode(&chunk) else { continue };
        for f in &skin.fids {
            if !f.name.to_ascii_lowercase().starts_with("collision") {
                continue;
            }
            let link = format!("{folder}{}", f.name);
            let Some(ids) = crate::static_item::build::material_surface_ids(store, &format!("{link}.Material.Gbx")) else { continue };
            let default = RedressKey::Path(f.file.to_ascii_lowercase());
            if !out.iter().any(|r| r.matches == default) {
                out.push(Redress { matches: default, link, ids });
            }
        }
    }
    out
}

/// The re-dress a modifier FOLDER implies for the hull, by material file name
/// (`RedressKey::File`): for every material link the folder provides —
/// `…\Modifier\X\S` — the row (`s.material.gbx`, the link, that material's
/// (physics, gameplay)). Materials without a surface chunk (decals, pure
/// shaders) contribute nothing, so a triangle they would have matched keeps
/// the prefab's own id.
pub fn modifier_folder_redress(store: &mut DataStore, links: &[String]) -> Vec<Redress> {
    let mut out: Vec<Redress> = Vec::new();
    for link in links {
        let file = RedressKey::File(format!("{}.material.gbx", link.rsplit('\\').next().unwrap_or(link).to_ascii_lowercase()));
        if out.iter().any(|r| r.matches == file) {
            continue;
        }
        if let Some(ids) = crate::static_item::build::material_surface_ids(store, &format!("{link}.Material.Gbx")) {
            out.push(Redress { matches: file, link: link.clone(), ids });
        }
    }
    out
}

/// The whole re-dress table of a block's modifiers: the GameSkin slot-table
/// rows first, then the folder shadows of every material link the folders
/// provide (the first matching row wins in `Merged::redress_collision`).
pub fn modifier_redress(store: &mut DataStore, refs: &[String], links: &[String]) -> Vec<Redress> {
    let mut rows = modifier_collision_redress(store, refs);
    rows.extend(modifier_folder_redress(store, links));
    rows
}

/// The material links a block's modifier folder provides: for each
/// `…\Modifier\X.TerrainModifier.Gbx` among the block info's material
/// modifier refs, every `…\Modifier\X\S.Material.Gbx` in the packs, as the
/// link `…\Modifier\X\S`. (The modifier file itself only names that folder.)
/// `…\Modifier\TrackWallToDecoCliff.Gbx` provides ONE link: the TrackWall of
/// the folder its file names (`Stadium\Media\Modifier\PlatformGrass\` —
/// `PlatformGrass\TrackWall` is the DecoCliffPxz concrete, no hue mask); its
/// skin is named for that one slot, and the other materials of that folder
/// (PlatformTech, DecalPlatform…) are not what a Tech deco block wears.
pub fn modifier_links(store: &DataStore, refs: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for r in refs {
        let r = r.replace(' ', "");
        if is_track_wall_to_deco_cliff(&r) {
            let link = format!("{TRACK_WALL_TO_DECO_CLIFF_FOLDER}TrackWall");
            if store.entries().any(|e| e.path().eq_ignore_ascii_case(&format!("{link}.Material.Gbx"))) {
                out.push(link);
            }
            continue;
        }
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
