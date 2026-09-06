//! Build the complete tiny library of a map through the STATIC-ITEM path (the
//! item kind Granady's tiny blocks are): every authored block model (name +
//! flags -> the prefab the game picked) and every item model becomes a
//! half-scale `CPlugStaticObjectModel` item with the stage-1 mechanisms
//! (`static_item/bake.rs`; recipe env vars apply), all under the map's own
//! collection. Writes the library zip, the `tmmaps tiny` mapping, and a
//! report of every model that could not be baked (with the reason) so the
//! gaps are explicit, never silent.
//!
//! Usage: mapgeom tiny-library MAP.Map.Gbx --catalog resolved.tsv --footprints fp.tsv
//!        --library-out ITEMS.zip --mapping-out placements.tsv [--report REPORT.tsv]
//!        [--scale 0.5] [--items-dir DIR] [--only NAME[,NAME]]
//!
//! Catalog rows: NAME<TAB>FLAGS(hex)<TAB>PREFAB logical path (one per picked
//! variant; several rows per NAME are fine). Footprints: NAME<TAB>SX<TAB>SZ.
//! Needs the client packs (`--packs DIR` with BlueBay.pak + Stadium pak, or
//! `--pak FILE[:KEY]`).

use crate::store::DataStore;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use tmmaps::map::MapFile;

pub struct Outcome {
    pub alias: String,
    pub kind: &'static str,
    pub source: String,
    pub placements: usize,
    pub result: Result<String, String>,
}

fn find_item_file(store: &DataStore, model: &str) -> Option<String> {
    let want = format!("\\{}.ITEM.GBX", model.to_uppercase());
    let mut hits: Vec<String> = store.entries().map(|e| e.path()).filter(|p| p.to_uppercase().ends_with(&want)).collect();
    hits.sort_by_key(|p| (!p.to_uppercase().contains("\\ITEMS\\"), p.len()));
    hits.into_iter().next()
}

#[allow(clippy::too_many_arguments)]
pub fn build(store: &mut DataStore, map: &Path, catalog: &Path, footprints: &Path, out_zip: &Path, out_mapping: &Path, report: Option<&Path>, scale: f32, items_dir: Option<&Path>, only: Option<&str>) {
    let source = MapFile::load(map);
    let collection = source.items.first().map(|it| it.collection_raw).unwrap_or(26);
    println!("  map collection {collection:#x}; {} blocks, {} items", source.blocks.len(), source.items.len());
    // catalog
    let mut catalog_map: BTreeMap<(String, u32), String> = BTreeMap::new();
    for (ln, line) in std::fs::read_to_string(catalog).unwrap().lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        assert!(f.len() == 3, "{}:{}: NAME<TAB>FLAGS<TAB>PREFAB", catalog.display(), ln + 1);
        let flags = u32::from_str_radix(f[1], 16).expect("hex flags");
        catalog_map.insert((f[0].to_string(), flags), f[2].to_string());
    }
    let mut footprint_map: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for line in std::fs::read_to_string(footprints).unwrap().lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        footprint_map.insert(f[0].to_string(), (f[1].parse().unwrap(), f[2].parse().unwrap()));
    }
    let wanted = |name: &str| only.map(|o| o.split(',').any(|n| n == name)).unwrap_or(true);

    // distinct (name, flags) among authored grid blocks -> prefab
    let mut keys: BTreeMap<(String, u32), usize> = BTreeMap::new();
    for b in &source.blocks {
        if b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0 {
            continue;
        }
        *keys.entry((b.name.clone(), b.flags)).or_insert(0) += 1;
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut alias_of_key: BTreeMap<(String, u32), String> = BTreeMap::new();
    let mut alias_of_prefab: BTreeMap<String, String> = BTreeMap::new();
    let mut next_alias = 0usize;
    for ((name, flags), n) in &keys {
        if !wanted(name) {
            continue;
        }
        let Some(prefab) = catalog_map.get(&(name.clone(), *flags)) else {
            outcomes.push(Outcome { alias: String::new(), kind: "block", source: format!("{name} {flags:08X}"), placements: *n, result: Err("no prefab resolved in the catalog".into()) });
            continue;
        };
        if let Some(alias) = alias_of_prefab.get(prefab) {
            alias_of_key.insert((name.clone(), *flags), alias.clone());
            continue;
        }
        let alias = format!("AC{next_alias:08}");
        next_alias += 1;
        let ident = format!("{alias}.Item.Gbx");
        let res = crate::static_item::build::static_item_from_prefab_report(store, prefab, &ident, &ident, scale, collection);
        match res {
            Ok((bytes, m)) => {
                let nv: usize = m.visuals.len();
                let skipped: Vec<&String> = m.notes.iter().filter(|n| n.contains("skipped") || n.contains("unnamed")).collect();
                let summary = format!("{} bytes, {} visuals, {} collision tris, {} skipped entities", bytes.len(), nv, m.surf_triangles.len(), skipped.len());
                if nv == 0 {
                    outcomes.push(Outcome { alias: alias.clone(), kind: "block", source: prefab.clone(), placements: *n, result: Err(format!("no visuals ({summary}); notes: {}", m.notes.iter().take(3).cloned().collect::<Vec<_>>().join(" | "))) });
                } else {
                    outcomes.push(Outcome { alias: alias.clone(), kind: "block", source: prefab.clone(), placements: *n, result: Ok(summary) });
                }
                files.insert(format!("Items/{ident}"), bytes);
                alias_of_prefab.insert(prefab.clone(), alias.clone());
                alias_of_key.insert((name.clone(), *flags), alias);
            }
            Err(e) => {
                outcomes.push(Outcome { alias: alias.clone(), kind: "block", source: prefab.clone(), placements: *n, result: Err(e) });
            }
        }
    }
    // item models
    let mut item_counts: BTreeMap<String, usize> = BTreeMap::new();
    for it in &source.items {
        *item_counts.entry(it.model.clone()).or_insert(0) += 1;
    }
    let mut alias_of_item: BTreeMap<String, String> = BTreeMap::new();
    let mut item_alias_n = 0usize;
    for (model, n) in &item_counts {
        if model.is_empty() || !wanted(model) {
            continue;
        }
        let alias = format!("AI{item_alias_n:08}");
        item_alias_n += 1;
        let ident = format!("{alias}.Item.Gbx");
        // a local file (items-dir/<model>.Item.Gbx) first, then the packs
        let local = items_dir.map(|d| d.join(format!("{model}.Item.Gbx"))).filter(|p| p.is_file());
        let bytes = match &local {
            Some(p) => std::fs::read(p).ok(),
            None => match find_item_file(store, model) {
                Some(logical) => store.read(&logical).ok(),
                None => None,
            },
        };
        let Some(bytes) = bytes else {
            outcomes.push(Outcome { alias: alias.clone(), kind: "item", source: model.clone(), placements: *n, result: Err("no .Item.Gbx in the client packs (nor --items-dir)".into()) });
            continue;
        };
        match crate::static_item::build::static_item_from_item_report(&bytes, &ident, &ident, scale, collection) {
            Ok((out, m)) => {
                let summary = format!("{} bytes, {} visuals, {} collision tris{}", out.len(), m.visuals.len(), m.surf_triangles.len(), if m.trigger.is_some() { format!(", waypoint {}", m.waypoint_type) } else { String::new() });
                if m.visuals.is_empty() {
                    outcomes.push(Outcome { alias: alias.clone(), kind: "item", source: model.clone(), placements: *n, result: Err(format!("no visuals ({summary})")) });
                } else {
                    files.insert(format!("Items/{ident}"), out);
                    alias_of_item.insert(model.clone(), alias.clone());
                    outcomes.push(Outcome { alias, kind: "item", source: model.clone(), placements: *n, result: Ok(summary) });
                }
            }
            Err(e) => outcomes.push(Outcome { alias, kind: "item", source: model.clone(), placements: *n, result: Err(e) }),
        }
    }
    // every item claims the map's collection (header + body idents)
    for bytes in files.values_mut() {
        *bytes = crate::tiny_assets::set_ident_collection(bytes, collection);
    }
    let archive = crate::tiny_assets::zip(&files);
    std::fs::write(out_zip, &archive).unwrap();
    // mapping: @index rows for blocks (footprint from the table), i@ rows for items
    let mut mapping = String::new();
    let mut missing_blocks: BTreeMap<String, usize> = BTreeMap::new();
    for b in &source.blocks {
        if b.flags & tmmaps::map::FREE_BLOCK_FLAG != 0 {
            continue;
        }
        match alias_of_key.get(&(b.name.clone(), b.flags)) {
            Some(alias) => {
                let (sx, sz) = footprint_map.get(&b.name).copied().unwrap_or((1, 1));
                mapping.push_str(&format!("@{}\t{}.Item.Gbx\t{}\t{}\t{}\n", b.index, alias, scale, sx, sz));
            }
            None => *missing_blocks.entry(format!("{} {:08X}", b.name, b.flags)).or_insert(0) += 1,
        }
    }
    let mut missing_items: BTreeMap<String, usize> = BTreeMap::new();
    for it in &source.items {
        match alias_of_item.get(&it.model) {
            Some(alias) => mapping.push_str(&format!("i@{}\t{}.Item.Gbx\t{}\n", it.index, alias, scale)),
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
    println!("  library: {} items ({} ok, {} failed models) -> {}", files.len(), ok, bad, out_zip.display());
    println!("  mapping: {} rows -> {}", mapping.lines().count(), out_mapping.display());
    if !missing_blocks.is_empty() {
        println!("  BLOCK PLACEMENTS WITHOUT A MODEL:");
        for (k, n) in &missing_blocks {
            println!("    {n:>5} x {k}");
        }
    }
    if !missing_items.is_empty() {
        println!("  ITEM PLACEMENTS WITHOUT A MODEL:");
        for (k, n) in &missing_items {
            println!("    {n:>5} x {k}");
        }
    }
    for o in &outcomes {
        if let Err(e) = &o.result {
            println!("  FAIL {} {} ({} placements): {}", o.kind, o.source, o.placements, e);
        }
    }
    let _ = BTreeSet::<u8>::new();
}
