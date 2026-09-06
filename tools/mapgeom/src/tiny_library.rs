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
fn veget_substitute(model: &str) -> Option<&'static str> {
    Some(match model {
        "PalmForest" | "PalmGrove" | "PalmEcotone" => "PalmTreeSmallA",
        m if m.starts_with("PalmTreeBig") => "PalmTreeSmallB",
        m if m.starts_with("PalmTreeSugarBig") => "PalmTreeSugarSmallA",
        m if m.starts_with("PalmTreeSugarMedium") => "PalmTreeSugarSmallB",
        "TreeMediumA" => "BushBigA",
        m if m.starts_with("BushBig") => "BushMediumA",
        m if m.starts_with("BushMedium") => "BushSmallA",
        m if m.starts_with("BushSmall") => "PlantSmallA",
        m if m.starts_with("PlantSmall") => "PlantSmallB",
        m if m.starts_with("PalmTreeSmall") || m.starts_with("PalmTreeSugarSmall") => return None,
        _ => return None,
    })
}

fn find_item_file(store: &DataStore, model: &str) -> Option<String> {
    let want = format!("\\{}.ITEM.GBX", model.to_uppercase());
    let mut hits: Vec<String> = store.entries().map(|e| e.path()).filter(|p| p.to_uppercase().ends_with(&want)).collect();
    hits.sort_by_key(|p| (!p.to_uppercase().contains("\\ITEMS\\"), p.len()));
    hits.into_iter().next()
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
    let wanted = |name: &str| only.map(|o| o.split(',').any(|n| n == name)).unwrap_or(true);
    let legacy: BTreeMap<String, Vec<u8>> = match legacy_zip {
        Some(p) => crate::embedded::unzip(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).expect("legacy zip"),
        None => BTreeMap::new(),
    };
    let mut idx = crate::blockmap::BlockInfoIndex::build(store, collection_name);

    // distinct (name, flags) among authored grid blocks AND the generated
    // (baked) non-Sea blocks -- the FC clip fillers that finish the authored
    // structures; the Sea itself stays the full-size foundation
    let mut keys: BTreeMap<(String, u32), usize> = BTreeMap::new();
    for b in source.blocks.iter().chain(source.baked.iter().filter(|b| b.name != "Sea")) {
        if b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0 {
            continue;
        }
        *keys.entry((b.name.clone(), b.flags)).or_insert(0) += 1;
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut outcomes: Vec<Outcome> = Vec::new();
    // key -> (alias or "-", footprint sx, sz)
    let mut block_map: BTreeMap<(String, u32), (String, u32, u32)> = BTreeMap::new();
    let mut alias_of_recipe: BTreeMap<String, String> = BTreeMap::new();
    let mut next_alias = 0usize;
    for ((name, flags), n) in &keys {
        if !wanted(name) {
            continue;
        }
        let ground = flags & crate::blockmap::FLAG_GROUND != 0;
        let vindex = (flags & crate::blockmap::FLAG_VARIANT_MASK) as usize;
        let sub = ((flags >> crate::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
        let Some(path) = idx.path_for(name) else {
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
        let Some(pk) = bi.pick_placement(ground, vindex, sub) else {
            outcomes.push(Outcome { alias: String::new(), kind: "block", source: format!("{name} {flags:08X}"), placements: *n, result: Err("block info has no variant with units or mobils".into()) });
            continue;
        };
        let units: Vec<[i32; 3]> = pk.variant.block_units.iter().map(|u| u.offset).collect();
        let (sx, sz) = units.iter().fold((1u32, 1u32), |(sx, sz), u| (sx.max(u[0] as u32 + 1), sz.max(u[2] as u32 + 1)));
        let prefabs: Vec<(String, Option<[f32; 3]>, Option<[f32; 3]>)> = pk.mobils.iter().filter_map(|mb| mb.prefab.clone().map(|p| (p, mb.translation, mb.rotation))).collect();
        let solids: Vec<String> = pk.mobils.iter().filter_map(|mb| mb.solid.clone()).collect();
        let legacy_item = LEGACY.iter().find(|(n, _)| n == name).map(|(_, p)| *p);
        // recipe key: what gets baked (prefab set or legacy item) + waypoint
        let recipe = if let Some(l) = legacy_item { format!("legacy:{l}") } else { format!("{}|wp{:?}|units{:?}", prefabs.iter().map(|p| format!("{}@{:?}/{:?}", p.0, p.1, p.2)).collect::<Vec<_>>().join(","), bi.waypoint_type, units) };
        if prefabs.is_empty() && legacy_item.is_none() {
            if solids.is_empty() {
                // intentionally empty variant (e.g. the hidden pillar)
                block_map.insert((name.clone(), *flags), ("-".into(), sx, sz));
                outcomes.push(Outcome { alias: "-".into(), kind: "block", source: format!("{name} {flags:08X} [{}]", pk.label), placements: *n, result: Ok("no geometry in this variant: intentionally no item".into()) });
            } else {
                outcomes.push(Outcome { alias: String::new(), kind: "block", source: format!("{name} {flags:08X} [{}] solids {:?}", pk.label, solids), placements: *n, result: Err("legacy CPlugSolid model without a converted archive item".into()) });
            }
            continue;
        }
        if let Some(alias) = alias_of_recipe.get(&recipe) {
            block_map.insert((name.clone(), *flags), (alias.clone(), sx, sz));
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
            // TINY_NO_SPLIT_FOR=name,name (default DecoBeachMangrove): models baked
            // without the per-layer split (the Mangrove split crashes the client;
            // minimal repro var-m1, open bug).
            let no_split_for = std::env::var("TINY_NO_SPLIT_FOR").unwrap_or_else(|_| "DecoBeachMangrove".into());
            m.no_split = no_split_for.split(',').any(|s| !s.is_empty() && s == name);
            let mut err = None;
            // Terrain (Flat/Frontier/Transition zone blocks) is lowered by
            // TERRAIN_DROP (full-scale metres): the Land plane and a road deck
            // in the same cell are coplanar in the source (the game resolves
            // that in its terrain pass), and as two items they z-fight into
            // grass stripes across the road (2026-09-06).
            let terrain = matches!(bi.kind, crate::blockinfo::Kind::Flat | crate::blockinfo::Kind::Frontier | crate::blockinfo::Kind::Transition);
            let drop: f32 = std::env::var("TINY_TERRAIN_DROP").ok().and_then(|s| s.parse().ok()).unwrap_or(0.2);
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
                    // waypoint: type from the block info, trigger = unit boxes,
                    // spawn = variant spawn_loc scaled (Granady's items)
                    if let Some(wt) = bi.waypoint_type.filter(|t| (0..=2).contains(t) || *t == 4) {
                        m.waypoint_type = Some(wt);
                        if wt != 0 {
                            m.trigger = Some(unit_box_trigger(&units, scale));
                        }
                        let sl = pk.variant.spawn_loc;
                        m.spawn = [sl[0] * scale, sl[1] * scale, sl[2] * scale];
                    }
                    let opts = crate::static_item::build::BuildOpts { ident: ident.clone(), author: ident.clone(), scale, collection, editors: m.editors };
                    crate::static_item::build::assemble(&m, &opts).map(|f| (crate::static_item::file::write_file(&f), m))
                }
            }
        };
        match res {
            Ok((bytes, m)) => {
                let nv = m.visuals.len();
                let veget = m.notes.iter().filter(|n| n.contains(".VegetTreeModel.Gbx")).count();
                let other_skips = m.notes.iter().filter(|n| (n.contains("skipped") || n.contains("failed") || n.contains("unnamed")) && !n.contains(".VegetTreeModel.Gbx")).count();
                let wp = match m.waypoint_type { Some(t) => format!(", waypoint {t} spawn {:?} trigger {}", m.spawn, m.trigger.is_some()), None => String::new() };
                let summary = format!("{} bytes, {} visuals, {} collision tris, {} vegetation entities dropped, {} other skips{wp}", bytes.len(), nv, m.surf_triangles.len(), veget, other_skips);
                if nv == 0 {
                    outcomes.push(Outcome { alias: alias.clone(), kind: "block", source: format!("{name} {flags:08X} [{}] {}", pk.label, recipe), placements: *n, result: Err(format!("no visuals ({summary}); notes: {}", m.notes.iter().take(3).cloned().collect::<Vec<_>>().join(" | "))) });
                    continue;
                }
                outcomes.push(Outcome { alias: alias.clone(), kind: "block", source: format!("{name} {flags:08X} [{}] {}", pk.label, prefabs.iter().map(|p| p.0.rsplit('\\').next().unwrap_or(&p.0).to_string()).collect::<Vec<_>>().join("+")), placements: *n, result: Ok(summary) });
                files.insert(format!("Items/{ident}"), bytes);
                alias_of_recipe.insert(recipe, alias.clone());
                block_map.insert((name.clone(), *flags), (alias, sx, sz));
            }
            // a prefab with no entities at all (Stadium\Structure\PillarToFlat_ACB
            // is one): the game draws nothing there either
            Err(e) if e.starts_with("no visuals: nothing to build") => {
                next_alias -= 1;
                block_map.insert((name.clone(), *flags), ("-".into(), sx, sz));
                alias_of_recipe.insert(recipe.clone(), "-".into());
                outcomes.push(Outcome { alias: "-".into(), kind: "block", source: format!("{name} {flags:08X} [{}] {}", pk.label, recipe), placements: *n, result: Ok("empty prefab (no entities): intentionally no item".into()) });
            }
            Err(e) => outcomes.push(Outcome { alias: alias.clone(), kind: "block", source: format!("{name} {flags:08X} [{}] {}", pk.label, recipe), placements: *n, result: Err(e) }),
        }
    }
    // item models
    let mut item_counts: BTreeMap<String, usize> = BTreeMap::new();
    for it in &source.items {
        *item_counts.entry(it.model.clone()).or_insert(0) += 1;
    }
    // model -> new model name: an embedded alias (AI...Item.Gbx) or a stock species
    let mut item_map: BTreeMap<String, String> = BTreeMap::new();
    let mut item_alias_n = 0usize;
    for (model, n) in &item_counts {
        if model.is_empty() || !wanted(model) {
            continue;
        }
        let alias = format!("AI{item_alias_n:08}");
        let ident = format!("{alias}.Item.Gbx");
        let local = items_dir.map(|d| d.join(format!("{model}.Item.Gbx"))).filter(|p| p.is_file());
        let res = match &local {
            Some(p) => crate::static_item::build::static_item_from_item_report(&std::fs::read(p).unwrap(), &ident, &ident, scale, collection),
            None => match find_item_file(store, model) {
                Some(logical) => crate::static_item::build::static_item_from_pack_item_report(store, &logical, &ident, &ident, scale, collection),
                None => Err("no .Item.Gbx in the client packs (nor --items-dir)".into()),
            },
        };
        match res {
            Ok((out, m)) if !m.visuals.is_empty() => {
                item_alias_n += 1;
                let summary = format!("{} bytes, {} visuals, {} collision tris{}", out.len(), m.visuals.len(), m.surf_triangles.len(), match m.waypoint_type { Some(t) => format!(", waypoint {t} trigger {} spawn {:?}", m.trigger.is_some(), m.spawn), None => String::new() });
                files.insert(format!("Items/{ident}"), out);
                item_map.insert(model.clone(), ident.clone());
                outcomes.push(Outcome { alias: ident, kind: "item", source: model.clone(), placements: *n, result: Ok(summary) });
            }
            Ok((_, m)) => outcomes.push(Outcome { alias: String::new(), kind: "item", source: model.clone(), placements: *n, result: Err(format!("no visuals; notes: {}", m.notes.iter().take(3).cloned().collect::<Vec<_>>().join(" | "))) }),
            Err(e) if e.contains("procedural vegetation") => match veget_mode {
                "substitute" => match veget_substitute(model) {
                    Some(sub) => {
                        item_map.insert(model.clone(), sub.to_string());
                        outcomes.push(Outcome { alias: sub.to_string(), kind: "item", source: model.clone(), placements: *n, result: Ok(format!("vegetation: re-pointed at stock {sub} (placement scale is ignored by the game)")) });
                    }
                    None => outcomes.push(Outcome { alias: model.clone(), kind: "item", source: model.clone(), placements: *n, result: Ok("vegetation: already a small species, kept".into()) }),
                },
                "drop" => {
                    item_map.insert(model.clone(), "-".into());
                    outcomes.push(Outcome { alias: "-".into(), kind: "item", source: model.clone(), placements: *n, result: Ok("vegetation: dropped".into()) });
                }
                _ => outcomes.push(Outcome { alias: model.clone(), kind: "item", source: model.clone(), placements: *n, result: Ok("vegetation: kept full size".into()) }),
            },
            Err(e) => outcomes.push(Outcome { alias: String::new(), kind: "item", source: model.clone(), placements: *n, result: Err(e) }),
        }
    }
    // every embedded item claims the map's collection (header + body idents)
    for bytes in files.values_mut() {
        *bytes = crate::tiny_assets::set_ident_collection(bytes, collection);
    }
    let archive = crate::tiny_assets::zip(&files);
    std::fs::write(out_zip, &archive).unwrap();
    // mapping: @index rows for blocks (alias or "-" = intentionally nothing), i@ rows for items
    let mut mapping = String::from("# tiny-library mapping: @block_index<TAB>ITEM|-<TAB>model_scale<TAB>sx<TAB>sz ; i@item_index<TAB>ITEM|stock model|-\n");
    let mut missing_blocks: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows = 0usize;
    for (prefix, b) in source.blocks.iter().map(|b| ("@", b)).chain(source.baked.iter().filter(|b| b.name != "Sea").map(|b| ("b@", b))) {
        if b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0 {
            continue;
        }
        match block_map.get(&(b.name.clone(), b.flags)) {
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
        match item_map.get(&it.model) {
            Some(target) => {
                let ms = if target.ends_with(".Item.Gbx") { scale } else { 1.0 };
                mapping.push_str(&format!("i@{}\t{}\t{}\n", it.index, target, ms));
                rows += 1;
            }
            None => *missing_items.entry(it.model.clone()).or_insert(0) += 1,
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
}
