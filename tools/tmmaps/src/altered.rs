//! `tmmaps altered-learn` / `tmmaps altered-apply` — the Altered Nadeo "Wood" and
//! "Wet Wood" recipes, LEARNED from the published campaigns rather than invented
//! (json9841 to vjeux, 2026-09-22: "Can the agent/tools analyze the past campaigns
//! to extract campaign rulesets? AR might have had specific rules for what he
//! changed to wood, where to place water").
//!
//! How AR's maps work (read off Winter 2026 Wood / Fall 2025 Wood + Wet Wood):
//! the original blocks all stay; a wood ITEM from his `z_WoodForBlocks` set is
//! placed on every block the set has a wood twin for (`RoadTechStraight` →
//! `…\RoadTechStraight-Wood-Light.Item.Gbx` at the cell centre, the block's
//! quarter turn), a few block swaps (`StructureSupport*` → `StructureBase` under a
//! `StructureSupport*-Wood` item), and Wet Wood adds a no-mesh water custom block
//! (`Water Blocks (no mesh)\…\RoadWaterStraight.Block.Gbx_CustomBlock`, a free
//! block 1.96 m under the deck so its sheet stands 4 cm over it) under the start
//! and under checkpoints.
//!
//! `altered-learn --orig DIR --alt DIR [--wet-alt DIR] --out RULES.tsv --items-out LIB`
//!   pairs `NN-*.Map.Gbx` files, diffs every pair, writes the rule table and the
//!   item library (every embedded file of the altered maps, by archive path;
//!   `LIB/manifest.tsv` = ident \t author \t archive path).
//! `altered-apply MAP --rules RULES.tsv --items LIB --out OUT [--wet] [--name N]`
//!   places the learned items (+ water for `--wet`), embeds the files, renames.

use crate::map::{BlockRec, ItemRec, MapFile};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

fn dir_yaw(dir: u8) -> f32 {
    match dir & 3 {
        0 => 0.0,
        1 => -std::f32::consts::FRAC_PI_2,
        2 => std::f32::consts::PI,
        _ => std::f32::consts::FRAC_PI_2,
    }
}

fn norm_angle(a: f32) -> f32 {
    let mut a = a;
    while a > std::f32::consts::PI {
        a -= 2.0 * std::f32::consts::PI;
    }
    while a <= -std::f32::consts::PI {
        a += 2.0 * std::f32::consts::PI;
    }
    a
}

/// A block's cell centre in world coordinates (x/z centre, y = cell floor).
fn cell_center(b: &BlockRec, ground: f32) -> [f32; 3] {
    let (cx, cy, cz) = b.coords();
    [cx as f32 * 32.0 + 16.0, cy as f32 * 8.0 + ground, cz as f32 * 32.0 + 16.0]
}

/// World → the block's local frame (its quarter turn undone).
fn to_local(p: [f32; 3], center: [f32; 3], yaw: f32) -> [f32; 3] {
    let dx = p[0] - center[0];
    let dz = p[2] - center[2];
    let (s, c) = (-yaw).sin_cos();
    [dx * c - dz * s, p[1] - center[1], dx * s + dz * c]
}

fn from_local(l: [f32; 3], center: [f32; 3], yaw: f32) -> [f32; 3] {
    let (s, c) = yaw.sin_cos();
    [center[0] + l[0] * c - l[2] * s, center[1] + l[1], center[2] + l[0] * s + l[2] * c]
}

fn file_stem(model: &str) -> String {
    let base = model.rsplit(['\\', '/']).next().unwrap_or(model);
    base.trim_end_matches(".Item.Gbx").trim_end_matches(".item.gbx").to_string()
}

/// `RoadTechStraight-Wood-Light` → `RoadTechStraight`; `StructureSupportCross-Pillar-Wood`
/// → `StructureSupportCross` (the `-Pillar` marks the pillar variant, learned separately).
fn item_block_stem(model: &str) -> String {
    let mut s = file_stem(model);
    for suf in ["-Light", "-Wood", "-Pillar", "-NoClips", "-NoMesh"] {
        while let Some(t) = s.strip_suffix(suf) {
            s = t.to_string();
        }
    }
    // "RoadTechStraight-Wood-Light" → suffixes are removed in any order
    let mut again = true;
    while again {
        again = false;
        for suf in ["-Light", "-Wood", "-Pillar"] {
            if let Some(t) = s.strip_suffix(suf) {
                s = t.to_string();
                again = true;
            }
        }
    }
    s
}

/// A block name matches an item stem when it IS the stem or becomes it by
/// deleting one `On…` terrain infix (`RoadTechOnLandHillSlopeBase2x1` →
/// `RoadTechSlopeBase2x1`).
fn block_matches_stem(block: &str, stem: &str) -> bool {
    if block == stem {
        return true;
    }
    let bytes = block.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if &block[i..i + 2] == "On" {
            for j in i + 2..=bytes.len() {
                if block.is_char_boundary(j) && format!("{}{}", &block[..i], &block[j..]) == stem {
                    return true;
                }
            }
        }
    }
    false
}

/// The block name with its `On…` terrain infix removed (the rule key).
pub fn norm_block(block: &str, stems: &BTreeSet<String>) -> String {
    for stem in stems {
        if block_matches_stem(block, stem) {
            return stem.clone();
        }
    }
    block.to_string()
}

/// The record flags that pick a block VARIANT (pillar / no pillar …): everything but
/// the form bits (free, waypoint, author/skin) and the ground bit.
fn variant_bits(flags: u32) -> u32 {
    flags & 0x00FF_FFFF & !(0x8000 | 0x1000 | 0x0010_0000 | 0x0020_0000)
}

fn family(name: &str) -> &str {
    let stem = file_stem(name);
    let stem = stem.trim_start_matches("M2_");
    for fam in FAMILIES {
        if stem.starts_with(fam) {
            // every Platform surface is one family for the wood twins (PlatformWoodBase
            // goes on PlatformTechBase, PlatformGrassBase, PlatformDirtBase …)
            if fam.starts_with("Platform") {
                return "Platform";
            }
            return fam;
        }
    }
    ""
}

/// The cells the author's validation ghost drives through: (cx, cz) → the ghost
/// heights seen there. A block is DRIVEN when the ghost passes its cell within
/// 12 m above its floor (a jump over a canyon still counts the landing).
fn driven_cells(map: &Path) -> Option<HashMap<(i32, i32), Vec<f32>>> {
    let d = gbx::record::decode_ghost(map.to_str()?).ok()?;
    if d.samples.len() < 10 {
        return None;
    }
    let mut cells: HashMap<(i32, i32), Vec<f32>> = HashMap::new();
    let mut prev: Option<(f32, f32, f32)> = None;
    for smp in &d.samples {
        let p = (smp.x, smp.y, smp.z);
        // interpolate between samples every 4 m so a fast car does not skip a cell
        if let Some(q) = prev {
            let dist = ((p.0 - q.0).powi(2) + (p.2 - q.2).powi(2)).sqrt();
            let n = (dist / 4.0).ceil().max(1.0) as usize;
            for k in 1..=n {
                let t = k as f32 / n as f32;
                let x = q.0 + (p.0 - q.0) * t;
                let y = q.1 + (p.1 - q.1) * t;
                let z = q.2 + (p.2 - q.2) * t;
                cells.entry(((x / 32.0).floor() as i32, (z / 32.0).floor() as i32)).or_default().push(y);
            }
        } else {
            cells.entry(((p.0 / 32.0).floor() as i32, (p.2 / 32.0).floor() as i32)).or_default().push(p.1);
        }
        prev = Some(p);
    }
    Some(cells)
}

fn is_driven(cells: &HashMap<(i32, i32), Vec<f32>>, b: &BlockRec, ground: f32) -> bool {
    let (cx, cy, cz) = b.coords();
    let floor = cy as f32 * 8.0 + ground;
    // the block's own cell and the 8 around it (the ghost is a point, the car is 2 m wide, a block up to 3 cells long)
    for dx in -1..=1 {
        for dz in -1..=1 {
            if let Some(ys) = cells.get(&(cx + dx, cz + dz)) {
                if ys.iter().any(|y| *y >= floor - 3.0 && *y <= floor + 12.0) {
                    return true;
                }
            }
        }
    }
    false
}

/// Families converted wholesale — visible structure, not a driving surface question
fn wholesale(name: &str) -> bool {
    name.starts_with("StructureSupport") || name.starts_with("Stage") || name.starts_with("Structure") || name.starts_with("Canopy")
}

fn is_waypoint_name(name: &str) -> bool {
    name.contains("Start") || name.contains("Checkpoint") || name.contains("Finish") || name.contains("Multilap")
}

fn pairs(orig: &Path, alt: &Path) -> Vec<(String, PathBuf, PathBuf)> {
    let mut out = Vec::new();
    let mut alts: BTreeMap<String, PathBuf> = BTreeMap::new();
    for e in std::fs::read_dir(alt).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n.ends_with(".Map.Gbx") {
            if let Some((nn, _)) = n.split_once('-') {
                alts.insert(nn.to_string(), e.path());
            }
        }
    }
    for e in std::fs::read_dir(orig).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n.ends_with(".Map.Gbx") {
            if let Some((nn, _)) = n.split_once('-') {
                if let Some(a) = alts.get(nn) {
                    out.push((nn.to_string(), e.path(), a.clone()));
                }
            }
        }
    }
    out.sort();
    out
}

#[derive(Default)]
struct Learn {
    /// (norm block, item model, block dir, variant bits) → (world offset from the cell corner, item yaw/pitch/roll) seen
    map: BTreeMap<(String, String, u8, u32), Vec<([f32; 3], [f32; 3])>>,
    /// norm block → (total in originals, with an item)
    coverage: BTreeMap<String, (usize, usize)>,
    removed: BTreeMap<String, usize>,
    added: BTreeMap<String, usize>,
    /// (removed name, removed flags, added name, added flags) → count
    swaps: BTreeMap<(String, u32, String, u32), usize>,
    unmatched_items: BTreeMap<String, usize>,
    /// water: (waypoint norm block) → local offsets + rot delta
    water: BTreeMap<String, Vec<([f32; 3], [f32; 3])>>,
    water_cov: BTreeMap<String, (usize, usize)>,
    water_far: usize,
    water_names: BTreeMap<String, usize>,
    /// item record properties per model: author, flags, scale, collection, pivot
    itemrec: BTreeMap<String, (String, u16, f32, u32, [f32; 3])>,
    files: BTreeMap<String, Vec<u8>>,
    manifest: BTreeMap<String, String>,
}

fn key3(p: [f32; 3]) -> (i64, i64, i64) {
    ((p[0] * 100.0).round() as i64, (p[1] * 100.0).round() as i64, (p[2] * 100.0).round() as i64)
}

fn learn_pair(l: &mut Learn, nn: &str, orig: &MapFile, alt: &MapFile, stems: &BTreeSet<String>, water: bool) {
    let collection = alt.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let ground = crate::map::ground_y(collection);
    // items of the original, by (model, position)
    let mut have: HashMap<(String, (i64, i64, i64)), usize> = HashMap::new();
    for it in &orig.items {
        *have.entry((it.model.clone(), key3(it.pos))).or_default() += 1;
    }
    let mut new_items: Vec<&ItemRec> = Vec::new();
    for it in &alt.items {
        let k = (it.model.clone(), key3(it.pos));
        match have.get_mut(&k) {
            Some(n) if *n > 0 => *n -= 1,
            _ => new_items.push(it),
        }
    }
    // blocks by cell
    let mut by_cell: HashMap<(i32, i32, i32), Vec<&BlockRec>> = HashMap::new();
    for b in &orig.blocks {
        by_cell.entry(b.coords()).or_default().push(b);
    }
    let mut covered: BTreeSet<usize> = BTreeSet::new();
    for it in &new_items {
        let stem = item_block_stem(&it.model);
        let cx = (it.pos[0] / 32.0).floor() as i32;
        let cz = (it.pos[2] / 32.0).floor() as i32;
        let cy = ((it.pos[1] - ground) / 8.0).round() as i32;
        // candidates: same cell first, then the 3×3×3 neighbourhood (multi-cell blocks)
        let mut best: Option<(&BlockRec, f32)> = None;
        for dx in -1..=1 {
            for dz in -1..=1 {
                for dy in -1..=1 {
                    if let Some(bs) = by_cell.get(&(cx + dx, cy + dy, cz + dz)) {
                        for b in bs {
                            if !block_matches_stem(&b.name, &stem) {
                                continue;
                            }
                            let c = cell_center(b, ground);
                            let d = ((it.pos[0] - c[0]).powi(2) + (it.pos[1] - c[1]).powi(2) + (it.pos[2] - c[2]).powi(2)).sqrt();
                            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                                best = Some((b, d));
                            }
                        }
                    }
                }
            }
        }
        // no name match: the same family in the very cell (AR lays RoadTechStraight-Wood
        // on RoadTechCheckpoint — his set has no checkpoint twin)
        if best.is_none() {
            let fam = family(&it.model);
            if !fam.is_empty() {
                if let Some(bs) = by_cell.get(&(cx, cy, cz)) {
                    for b in bs {
                        if family(&b.name) == fam && (shape(&b.name) == shape(&it.model) || fam != "Platform") {
                            let c = cell_center(b, ground);
                            let d = ((it.pos[0] - c[0]).powi(2) + (it.pos[1] - c[1]).powi(2) + (it.pos[2] - c[2]).powi(2)).sqrt();
                            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                                best = Some((b, d));
                            }
                        }
                    }
                }
            }
        }
        match best {
            Some((b, _)) => {
                // the offset from the cell's low corner in WORLD axes, per quarter turn (the
                // game pivots a multi-cell block so its footprint stays on its cells; the item
                // sits at the pivot cell's centre — a per-dir table, not a rotated frame)
                let (cx, cy, cz) = b.coords();
                let corner = [cx as f32 * 32.0, cy as f32 * 8.0 + ground, cz as f32 * 32.0];
                let off = [it.pos[0] - corner[0], it.pos[1] - corner[1], it.pos[2] - corner[2]];
                if std::env::var("ALTERED_DEBUG").is_ok() {
                    eprintln!("    sample {nn} {} dir {} at {:?} → {} item ({:.1},{:.1},{:.1}) yaw {:.3} → off ({:.1},{:.1},{:.1})", b.name, b.dir & 3, b.coords(), file_stem(&it.model), it.pos[0], it.pos[1], it.pos[2], it.yaw, off[0], off[1], off[2]);
                }
                l.map.entry((norm_block(&b.name, stems), it.model.clone(), b.dir & 3, variant_bits(b.flags))).or_default().push((off, [it.yaw, it.pitch, it.roll]));
                covered.insert(b.index);
                l.itemrec.entry(it.model.clone()).or_insert((it.author.clone().unwrap_or_default(), it.flags, it.scale, it.collection_raw, it.pivot));
            }
            None => {
                *l.unmatched_items.entry(it.model.clone()).or_default() += 1;
            }
        }
    }
    for b in &orig.blocks {
        let e = l.coverage.entry(norm_block(&b.name, stems)).or_default();
        e.0 += 1;
        if covered.contains(&b.index) {
            e.1 += 1;
        }
    }
    // removed / added blocks (name, cell, dir)
    let sig = |b: &BlockRec| (b.name.clone(), b.coords(), b.dir & 3, b.free_pos.map(key3));
    let mut o_sig: HashMap<_, usize> = HashMap::new();
    for b in &orig.blocks {
        *o_sig.entry(sig(b)).or_default() += 1;
    }
    let mut a_sig: HashMap<_, usize> = HashMap::new();
    for b in &alt.blocks {
        *a_sig.entry(sig(b)).or_default() += 1;
    }
    let mut removed_here: Vec<&BlockRec> = Vec::new();
    for b in &orig.blocks {
        let s = sig(b);
        if a_sig.get(&s).copied().unwrap_or(0) == 0 {
            removed_here.push(b);
            *l.removed.entry(b.name.clone()).or_default() += 1;
        } else if let Some(n) = a_sig.get_mut(&s) {
            *n -= 1;
        }
    }
    let mut added_here: Vec<&BlockRec> = Vec::new();
    let mut o_left = o_sig.clone();
    for b in &alt.blocks {
        let s = sig(b);
        match o_left.get_mut(&s) {
            Some(n) if *n > 0 => *n -= 1,
            _ => added_here.push(b),
        }
    }
    let waypoints: Vec<&BlockRec> = orig.blocks.iter().filter(|b| is_waypoint_name(&b.name) || b.waypoint_tag.is_some()).collect();
    let mut wp_with_water: BTreeSet<usize> = BTreeSet::new();
    for b in &added_here {
        if water && b.name.contains("Water") && b.name.contains("_CustomBlock") {
            *l.water_names.entry(b.name.clone()).or_default() += 1;
            let p = b.free_pos.unwrap_or_else(|| cell_center(b, ground));
            // the nearest waypoint block
            let mut best: Option<(&BlockRec, f32)> = None;
            for w in &waypoints {
                let c = cell_center(w, ground);
                let d = ((p[0] - c[0]).powi(2) + (p[2] - c[2]).powi(2)).sqrt() + (p[1] - c[1]).abs();
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((w, d));
                }
            }
            match best {
                Some((w, d)) if d < 48.0 => {
                    let c = cell_center(w, ground);
                    let yaw = dir_yaw(w.dir);
                    let rot = b.free_rot.map(|r| r[0]).unwrap_or(dir_yaw(b.dir));
                    l.water.entry(norm_block(&w.name, stems)).or_default().push((to_local(p, c, yaw), [norm_angle(rot - yaw), 0.0, 0.0]));
                    wp_with_water.insert(w.index);
                }
                _ => l.water_far += 1,
            }
            continue;
        }
        *l.added.entry(b.name.clone()).or_default() += 1;
    }
    // swaps: within one cell, removed ↔ added paired by shape first, then a lone
    // pair; several-to-several cells (stacked loop pieces) stay unpaired
    let mut cells: BTreeMap<(i32, i32, i32), (Vec<&BlockRec>, Vec<&BlockRec>)> = BTreeMap::new();
    for r in &removed_here {
        cells.entry(r.coords()).or_default().0.push(r);
    }
    for a in &added_here {
        if water && a.name.contains("Water") && a.name.contains("_CustomBlock") {
            continue;
        }
        cells.entry(a.coords()).or_default().1.push(a);
    }
    for (_, (rs, adds)) in cells {
        let mut used_a: Vec<bool> = vec![false; adds.len()];
        let mut left_r: Vec<&BlockRec> = Vec::new();
        for r in rs {
            let rshape = shape(&r.name);
            match adds.iter().enumerate().position(|(k, a)| !used_a[k] && shape(&a.name) == rshape && a.name != r.name) {
                Some(k) => {
                    used_a[k] = true;
                    *l.swaps.entry((r.name.clone(), r.flags, adds[k].name.clone(), adds[k].flags)).or_default() += 1;
                }
                None => left_r.push(r),
            }
        }
        let left_a: Vec<&BlockRec> = adds.iter().enumerate().filter(|(k, _)| !used_a[*k]).map(|(_, a)| *a).collect();
        if left_r.len() == 1 && left_a.len() == 1 {
            *l.swaps.entry((left_r[0].name.clone(), left_r[0].flags, left_a[0].name.clone(), left_a[0].flags)).or_default() += 1;
        }
    }
    if water {
        for w in &waypoints {
            let e = l.water_cov.entry(norm_block(&w.name, stems)).or_default();
            e.0 += 1;
            if wp_with_water.contains(&w.index) {
                e.1 += 1;
            }
        }
    }
    // the item library
    if let Some((rows, zip)) = alt.embedded_objects() {
        for (ident, author) in rows {
            l.manifest.entry(ident).or_insert(author);
        }
        for (name, bytes) in crate::header::zip_entries(&zip) {
            l.files.entry(name).or_insert(bytes);
        }
    }
    eprintln!("  {nn}: {} new items, {} removed / {} added blocks", new_items.len(), removed_here.len(), added_here.len());
}

fn stats(v: &[([f32; 3], [f32; 3])]) -> ([f32; 3], [f32; 3], f32) {
    // the modal offset (rounded to 1 cm) and the share that agrees with it; the yaw / pitch /
    // roll modals on their own (AR's PlatformBase-Wood filler is placed UPSIDE DOWN at the cell
    // top — pitch π; a rule without the pitch put the box one row up: Spring 2026 - 01 wood CP3,
    // json9841 2026-09-23)
    let mut counts: BTreeMap<(i64, i64, i64), usize> = BTreeMap::new();
    let mut angles: [BTreeMap<i64, usize>; 3] = [BTreeMap::new(), BTreeMap::new(), BTreeMap::new()];
    for (p, r) in v {
        *counts.entry(key3(*p)).or_default() += 1;
        for k in 0..3 {
            *angles[k].entry((r[k] * 100.0).round() as i64).or_default() += 1;
        }
    }
    let (k, n) = counts.into_iter().max_by_key(|(_, n)| *n).unwrap();
    let mut rot = [0.0f32; 3];
    for (k, a) in angles.into_iter().enumerate() {
        rot[k] = a.into_iter().max_by_key(|(_, n)| *n).map(|(y, _)| y as f32 / 100.0).unwrap_or(0.0);
    }
    ([k.0 as f32 / 100.0, k.1 as f32 / 100.0, k.2 as f32 / 100.0], rot, n as f32 / v.len() as f32)
}

pub fn learn_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let orig = PathBuf::from(f("--orig").ok_or("--orig DIR (the Nadeo campaign)")?);
    let alt = PathBuf::from(f("--alt").ok_or("--alt DIR (the Wood campaign)")?);
    let wet = f("--wet-alt").map(PathBuf::from);
    let out = PathBuf::from(f("--out").ok_or("--out RULES.tsv")?);
    let items_out = f("--items-out").map(PathBuf::from);
    let mut l = Learn::default();
    // pass 1: the item stems the wood set knows (from every altered map's embedded idents)
    let mut stems: BTreeSet<String> = BTreeSet::new();
    let mut all_pairs = pairs(&orig, &alt).into_iter().map(|(n, o, a)| (n, o, a, false)).collect::<Vec<_>>();
    if let Some(w) = &wet {
        all_pairs.extend(pairs(&alt, w).into_iter().map(|(n, o, a)| (n, o, a, true)));
    }
    for (_, _, a, _) in &all_pairs {
        let m = MapFile::load(a);
        if let Some((rows, _)) = m.embedded_objects() {
            for (ident, _) in rows {
                if ident.contains("Wood") {
                    stems.insert(item_block_stem(&ident));
                }
            }
        }
    }
    eprintln!("{} wood item stems", stems.len());
    for (nn, o, a, water) in &all_pairs {
        let om = MapFile::load(o);
        let am = MapFile::load(a);
        learn_pair(&mut l, nn, &om, &am, &stems, *water);
    }
    let mut w = String::new();
    w.push_str("# kind\tblock\titem/added\tdx\tdy\tdz\tyaw\tcount\tagree\tdir\tvariant\tpitch\troll\n");
    for ((block, item, dir, var), v) in &l.map {
        let (p, r, agree) = stats(v);
        w.push_str(&format!("map\t{block}\t{item}\t{:.2}\t{:.2}\t{:.2}\t{:.4}\t{}\t{:.2}\t{dir}\t{var:08X}\t{:.4}\t{:.4}\n", p[0], p[1], p[2], r[0], v.len(), agree, r[1], r[2]));
    }
    for (block, (t, c)) in &l.coverage {
        if *c > 0 || stems.iter().any(|s| block_matches_stem(block, s)) {
            w.push_str(&format!("coverage\t{block}\t\t\t\t\t\t{t}\t{c}\n"));
        }
    }
    for ((r, rf, a, af), n) in &l.swaps {
        w.push_str(&format!("swap\t{r}\t{a}\t{rf:08X}\t{af:08X}\t\t\t{n}\t\n"));
    }
    for (r, n) in &l.removed {
        w.push_str(&format!("removed\t{r}\t\t\t\t\t\t{n}\t\n"));
    }
    for (a, n) in &l.added {
        w.push_str(&format!("added\t{a}\t\t\t\t\t\t{n}\t\n"));
    }
    for (wp, v) in &l.water {
        let (p, r, agree) = stats(v);
        w.push_str(&format!("water\t{wp}\t\t{:.2}\t{:.2}\t{:.2}\t{:.4}\t{}\t{:.2}\n", p[0], p[1], p[2], r[0], v.len(), agree));
    }
    for (wp, (t, c)) in &l.water_cov {
        w.push_str(&format!("watercov\t{wp}\t\t\t\t\t\t{t}\t{c}\n"));
    }
    for (n, c) in &l.water_names {
        w.push_str(&format!("waterblock\t{n}\t\t\t\t\t\t{c}\t\n"));
    }
    if l.water_far > 0 {
        w.push_str(&format!("waterfar\t\t\t\t\t\t\t{}\t\n", l.water_far));
    }
    for (m, n) in &l.unmatched_items {
        w.push_str(&format!("unmatched\t\t{m}\t\t\t\t\t{n}\t\n"));
    }
    for (m, (author, flags, scale, coll, pivot)) in &l.itemrec {
        w.push_str(&format!("itemrec\t\t{m}\t{author}\t{flags}\t{scale}\t{coll}\t{:.3},{:.3},{:.3}\t\n", pivot[0], pivot[1], pivot[2]));
    }
    std::fs::write(&out, w).map_err(|e| format!("{}: {e}", out.display()))?;
    if let Some(dir) = items_out {
        let mut n = 0usize;
        for (name, bytes) in &l.files {
            let p = dir.join(name.replace('\\', "/"));
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            if !p.exists() {
                std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
                n += 1;
            }
        }
        let mut mf = String::new();
        for (ident, author) in &l.manifest {
            // the archive entry that holds this ident (an `Items/…/<ident with / separators>` path)
            let tail = ident.replace('\\', "/");
            let entry = l.files.keys().find(|k| k.ends_with(&tail) || k.ends_with(&format!("/{tail}"))).cloned().unwrap_or_default();
            mf.push_str(&format!("{ident}\t{author}\t{entry}\n"));
        }
        let mp = dir.join("manifest.tsv");
        let mut existing = std::fs::read_to_string(&mp).unwrap_or_default();
        existing.push_str(&mf);
        std::fs::write(&mp, existing).map_err(|e| e.to_string())?;
        println!("items: {n} new files under {}, manifest {} rows", dir.display(), l.manifest.len());
    }
    println!("rules: {} block→item mappings, {} swaps, {} water offsets → {}", l.map.len(), l.swaps.len(), l.water.len(), out.display());
    Ok(())
}

// ---------------------------------------------------------------- apply

#[derive(Default)]
struct Rules {
    /// vanilla block name → candidate custom-block targets: (target, target flags, count) —
    /// keyed by the source flags too, so a ground variant keeps its ground twin
    swaps: BTreeMap<String, Vec<(u32, String, u32, usize)>>,
    /// (norm block, dir, variant bits) → (item model, world offset from the cell corner, item yaw/pitch/roll, count)
    items: BTreeMap<(String, u8, u32), Vec<(String, [f32; 3], [f32; 3], usize)>>,
    /// item model → (author, flags, scale, collection, pivot)
    itemrec: BTreeMap<String, (String, u16, f32, u32, [f32; 3])>,
    /// `water` rows: (waypoint block ident in AR's map, the pad's local y in the cell)
    water_raw: Vec<(String, f32)>,
    stems: BTreeSet<String>,
}

fn is_wood_custom(name: &str) -> bool {
    name.contains("_CustomBlock") && (name.contains("Wood") || name.contains("HeavyWood"))
}

const FAMILIES: &[&str] = &[
    "PlatformTech", "PlatformGrass", "PlatformDirt", "PlatformIce", "PlatformPlastic", "PlatformWood", "PlatformWater",
    "RoadTech", "RoadDirt", "RoadIce", "RoadBump", "RoadWater", "RoadWood", "OpenTechRoad", "OpenTechZone", "OpenTechHillsShort", "OpenTech",
    "OpenDirtRoad", "OpenDirtZone", "OpenDirt", "OpenIceRoad", "OpenIceZone", "OpenIce", "OpenGrassRoad", "OpenGrassZone", "OpenGrass", "Canopy", "StageTechnics", "Stage", "DecoPlatform", "DecoWall", "DecoHill",
    "TrackWall", "StructureSupport", "Structure", "RallyCastleRoad", "RallyCastleWall", "RallyRoadDirtHighXLowBridge", "RallyRoadDirtHigh", "RallyRoadDirtLow", "RallyRoadDirt",
    "SnowRoad", "DesertRoad", "Road", "Platform",
];

/// The shape part of a block or wood-twin name: the family prefix and the wood
/// suffixes removed (`PlatformGrassSlope2Straight` → `Slope2Straight`,
/// `PlatformPlasticSlope2Straight-HeavyWood` → `Slope2Straight`, `M2_PlatformTechBase` → `Base`).
fn shape(name: &str) -> String {
    let mut s = file_stem(name).trim_end_matches("_CustomBlock").trim_end_matches(".Block.Gbx").to_string();
    for suf in ["-HeavyWood", "-Wood-Light", "-Wood", "-Light", "-Pillar"] {
        while let Some(t) = s.strip_suffix(suf) {
            s = t.to_string();
        }
    }
    if let Some(t) = s.strip_prefix("M2_") {
        s = t.to_string();
    }
    for fam in FAMILIES {
        if let Some(t) = s.strip_prefix(fam) {
            s = t.to_string();
            break;
        }
    }
    // a terrain-top marker at the end (DecoWallSlopeBaseGrass → SlopeBase)
    for suf in ["Grass", "Dirt", "Ice", "Snow"] {
        if let Some(t) = s.strip_suffix(suf) {
            if !t.is_empty() {
                s = t.to_string();
            }
        }
    }
    s
}

fn read_rules(paths: &[String]) -> Result<Rules, String> {
    let mut r = Rules::default();
    // swap candidates first: (src, src flags, tgt, tgt flags) → count, then filtered
    let mut raw: BTreeMap<(String, u32, String, u32), usize> = BTreeMap::new();
    for p in paths {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
        // this file's own swap totals per source: a newer campaign's choice for a block
        // replaces the older one (AR moved PlatformTech from HeavyWood to the M2_ set)
        let mut here: BTreeMap<String, usize> = BTreeMap::new();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() >= 9 && f[0] == "swap" && is_wood_custom(f[2]) {
                *here.entry(f[1].to_string()).or_default() += f[7].parse::<usize>().unwrap_or(0);
            }
        }
        for (src, n) in &here {
            if *n >= 3 {
                raw.retain(|(s, _, _, _), _| s != src);
            }
        }
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 9 {
                continue;
            }
            match f[0] {
                "swap" if is_wood_custom(f[2]) && !f[1].starts_with("DecoWall") && !f[1].starts_with("StructurePillar") && !f[1].starts_with("TrackWall") => {
                    let sf = u32::from_str_radix(f[3], 16).unwrap_or(0);
                    let tf = u32::from_str_radix(f[4], 16).unwrap_or(0x8000);
                    let n: usize = f[7].parse().unwrap_or(0);
                    *raw.entry((f[1].to_string(), sf, f[2].to_string(), tf)).or_default() += n;
                }
                "map" => {
                    let n: usize = f[7].parse().unwrap_or(0);
                    let agree: f32 = f[8].parse().unwrap_or(0.0);
                    let dir: u8 = f.get(9).and_then(|d| d.trim().parse().ok()).unwrap_or(0);
                    let var: u32 = f.get(10).and_then(|d| u32::from_str_radix(d.trim(), 16).ok()).unwrap_or(0);
                    if agree >= 0.5 && f[2].contains("Wood") {
                        let off = [f[3].parse().unwrap_or(0.0), f[4].parse().unwrap_or(0.0), f[5].parse().unwrap_or(0.0)];
                        let rot = [f[6].parse().unwrap_or(0.0), f.get(11).and_then(|x| x.trim().parse().ok()).unwrap_or(0.0), f.get(12).and_then(|x| x.trim().parse().ok()).unwrap_or(0.0)];
                        r.items.entry((f[1].to_string(), dir, var)).or_default().push((f[2].to_string(), off, rot, n));
                        r.stems.insert(f[1].to_string());
                    }
                }
                "water" if f.len() >= 5 => {
                    if let Ok(y) = f[4].trim().parse::<f32>() {
                        r.water_raw.push((f[1].to_string(), y));
                    }
                }
                "itemrec" => {
                    let coll: u32 = f[6].parse().unwrap_or(26);
                    let pv: Vec<f32> = f[7].split(',').map(|x| x.trim().parse().unwrap_or(0.0)).collect();
                    let pivot = if pv.len() == 3 { [pv[0], pv[1], pv[2]] } else { [0.0; 3] };
                    r.itemrec.insert(f[2].to_string(), (f[3].to_string(), f[4].parse().unwrap_or(1), f[5].parse().unwrap_or(1.0), coll, pivot));
                }
                _ => {}
            }
        }
    }
    // per source name: the share of each target among its wood-twin swaps; a rule
    // needs a matching shape (RoadTechCurve1 → RoadTechCurve1-HeavyWood) or a strong
    // majority (StageStraight → PlatformPlasticBase-HeavyWood, 240 times)
    let mut per_src: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_src_tgt: BTreeMap<(String, String), usize> = BTreeMap::new();
    for ((src, _, tgt, _), n) in &raw {
        *per_src.entry(src.clone()).or_default() += n;
        *per_src_tgt.entry((src.clone(), tgt.clone())).or_default() += n;
    }
    let mut dropped: BTreeMap<String, usize> = BTreeMap::new();
    for ((src, sf, tgt, tf), n) in raw {
        let share = per_src_tgt[&(src.clone(), tgt.clone())] as f32 / per_src[&src] as f32;
        let same_shape = shape(&src) == shape(&tgt);
        if same_shape || (per_src_tgt[&(src.clone(), tgt.clone())] >= 10 && share >= 0.6) {
            r.swaps.entry(src).or_default().push((sf, tgt, tf, n));
        } else {
            *dropped.entry(format!("{src} → {}", file_stem(&tgt))).or_default() += n;
        }
    }
    if std::env::var("ALTERED_DEBUG").is_ok() {
        let mut v: Vec<_> = dropped.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        eprintln!("swap rules dropped (shape mismatch, minority): {}", v.iter().take(40).map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join("; "));
    }
    Ok(r)
}

/// The library: `manifest.tsv` (ident \t author \t archive path) + the files.
struct Lib {
    dir: PathBuf,
    rows: BTreeMap<String, (String, String)>,
}

impl Lib {
    fn load(dir: &Path) -> Result<Lib, String> {
        let text = std::fs::read_to_string(dir.join("manifest.tsv")).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut rows = BTreeMap::new();
        for l in text.lines() {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() >= 3 && !f[2].is_empty() {
                rows.entry(f[0].to_string()).or_insert((f[1].to_string(), f[2].to_string()));
            }
        }
        Ok(Lib { dir: dir.to_path_buf(), rows })
    }
    /// (author, archive entry name, bytes) for an ident (a `…_CustomBlock` name is looked up without the suffix)
    fn get(&self, ident: &str) -> Option<(String, String, Vec<u8>)> {
        let key = ident.trim_end_matches("_CustomBlock");
        let (author, entry) = self.rows.get(key)?;
        let bytes = std::fs::read(self.dir.join(entry.replace('\\', "/"))).ok()?;
        Some((author.clone(), entry.clone(), bytes))
    }
}

pub fn new_uid(prefix: &str, len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut x = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) ^ 0x9E37_79B9_7F4A_7C15;
    let alphabet: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-".chars().collect();
    let mut s = prefix.to_string();
    while s.len() < len.max(prefix.len() + 8) {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.push(alphabet[(x % 64) as usize]);
    }
    s
}

pub fn apply_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let input = PathBuf::from(args.iter().skip(1).find(|a| !a.starts_with("--") && a.ends_with(".Map.Gbx")).ok_or("tmmaps altered-apply MAP --rules R.tsv[,R2.tsv] --items LIB --out OUT [--wet] [--name N] [--uid-prefix P]")?);
    let rules_paths: Vec<String> = f("--rules").ok_or("--rules")?.split(',').map(String::from).collect();
    let lib = Lib::load(Path::new(&f("--items").ok_or("--items LIB")?))?;
    let out = PathBuf::from(f("--out").ok_or("--out")?);
    let wet = crate::cli::has(args, "--wet");
    let rules = read_rules(&rules_paths)?;
    let water_block = f("--water-block").unwrap_or_else(|| "Water Blocks (no mesh)\\Road\\RoadWater\\RoadWaterStraight\\RoadWaterStraight.Block.Gbx_CustomBlock".to_string());
    let stadium_only_swaps = !crate::cli::has(args, "--swap-anywhere");

    let source = MapFile::load(&input);
    let collection = source.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let ground = crate::map::ground_y(collection);
    let is_stadium = collection == 26;
    // the author's route: AR converts what is driven (plus the visible structures);
    // without a ghost everything eligible is converted
    let driven = if crate::cli::has(args, "--all") { None } else { driven_cells(&input) };
    let mut skipped_undriven: BTreeMap<String, usize> = BTreeMap::new();
    // a ghost that starts late (Spring 2026 - 10: first sample at 6.75 s) misses the
    // head of the route: the straight line from the spawn to its first sample counts
    let driven = driven.map(|mut cells| {
        if let Some(spawn) = source.blocks.iter().find(|b| b.waypoint_tag.as_deref() == Some("Spawn")).map(|b| cell_center(b, ground)).or_else(|| source.items.iter().find(|it| it.waypoint_tag.as_deref() == Some("Spawn")).map(|it| it.pos)) {
            if let Some(first) = gbx::record::decode_ghost(input.to_str().unwrap_or_default()).ok().and_then(|d| d.samples.first().map(|s| (s.time_ms, s.x, s.y, s.z))) {
                if first.0 > 500 {
                    let n = ((((first.1 - spawn[0]).powi(2) + (first.3 - spawn[2]).powi(2)).sqrt()) / 4.0).ceil().max(1.0) as usize;
                    for k in 0..=n {
                        let t = k as f32 / n as f32;
                        let x = spawn[0] + (first.1 - spawn[0]) * t;
                        let y = spawn[1] + 2.0 + (first.2 - spawn[1] - 2.0) * t;
                        let z = spawn[2] + (first.3 - spawn[2]) * t;
                        cells.entry(((x / 32.0).floor() as i32, (z / 32.0).floor() as i32)).or_default().push(y);
                    }
                    println!("  route: the ghost starts at {:.2} s — the spawn → first-sample line added ({n} steps)", first.0 as f32 / 1000.0);
                }
            }
        }
        cells
    });
    let wants = |b: &BlockRec, skipped: &mut BTreeMap<String, usize>| -> bool {
        if b.waypoint_tag.is_some() {
            return true; // a start, checkpoint or finish is on the route by definition
        }
        match &driven {
            Some(cells) if !wholesale(&b.name) && !is_driven(cells, b, ground) => {
                *skipped.entry(b.name.clone()).or_default() += 1;
                false
            }
            _ => true,
        }
    };
    println!("  route: {}", match &driven { Some(c) => format!("validation ghost over {} cells", c.len()), None => "no ghost — every eligible block converted".to_string() });
    println!("{}: {} blocks, {} items, collection {collection:#x} ({})", input.display(), source.blocks.len(), source.items.len(), if is_stadium { "Stadium: block swaps" } else { "terrain environment: wood items" });

    // 1. block swaps (rename in place + flags) — Stadium maps (AR's own practice)
    let mut swapped: BTreeMap<String, usize> = BTreeMap::new();
    let mut swap_plan: Vec<(usize, String, u32)> = Vec::new();
    let mut free_swaps: Vec<(usize, String, u32)> = Vec::new();
    let mut unswapped: BTreeMap<String, usize> = BTreeMap::new();
    let mut needed: BTreeSet<String> = BTreeSet::new();
    if is_stadium || !stadium_only_swaps {
        for b in &source.blocks {
            if b.flags & crate::map::FREE_BLOCK_FLAG != 0 && b.name.contains("_CustomBlock") {
                continue;
            }
            // ALTERED_DEBUG=1: waypoints + starts; ALTERED_DEBUG=<substring>: the blocks whose name carries it
            let dbg = match std::env::var("ALTERED_DEBUG") {
                Ok(v) if v == "1" => b.waypoint_tag.is_some() || b.name.contains("Start"),
                Ok(v) => !v.is_empty() && b.name.contains(v.as_str()),
                Err(_) => false,
            };
            if !wants(b, &mut skipped_undriven) {
                if dbg {
                    eprintln!("    [swap] {} #{} at {:?}: off the route", b.name, b.index, b.coords());
                }
                continue;
            }
            if dbg {
                eprintln!("    [swap] {} #{} at {:?} flags {:08X}: rules {:?}", b.name, b.index, b.coords(), b.flags, rules.swaps.get(&b.name).map(|c| c.iter().map(|x| format!("{}×{}", file_stem(&x.1), x.3)).collect::<Vec<_>>()));
            }
            let Some(cands) = rules.swaps.get(&b.name) else {
                if b.name.starts_with("Road") || b.name.starts_with("Platform") || b.name.starts_with("Open") || b.name.starts_with("Canopy") || b.name.starts_with("Stage") || b.name.starts_with("RallyCastle") || b.name.starts_with("RallyRoad") {
                    *unswapped.entry(b.name.clone()).or_default() += 1;
                }
                continue;
            };
            // the same source flags first, else the most frequent target with the ground bit kept
            let variant_mask = 0x0000_FFFF & !0x8000u32; // variant + ground bits
            let mut best: Option<&(u32, String, u32, usize)> = cands.iter().filter(|c| c.0 == b.flags).max_by_key(|c| c.3);
            if best.is_none() {
                best = cands.iter().filter(|c| (c.0 & variant_mask) == (b.flags & variant_mask)).max_by_key(|c| c.3);
            }
            if best.is_none() {
                best = cands.iter().max_by_key(|c| c.3);
            }
            let Some((_, target, tflags, _)) = best else { continue };
            if lib.get(target).is_none() {
                *unswapped.entry(format!("{} (twin {} not in the library)", b.name, file_stem(target))).or_default() += 1;
                continue;
            }
            // keep the record's form bits (free / waypoint / author) as they are — a rename in place
            let keep = 0x2000_0000 | 0x4000_0000 | 0x0010_0000 | 0x0000_8000;
            // AR's flags VERBATIM (minus the record-form bits): a mesh-modeler block is an
            // ADDITIONAL VARIANT of its archetype, bit 21 selects the custom mesh — his records
            // all carry it. Applying only his set/clear diff (v6, 2026-09-23) dropped bit 21 on
            // every base placed over a 00000000 source → the game drew the vanilla archetype
            // (json9841: "very few of the blocks driven on are wood")
            let flags = (tflags & !keep) | (b.flags & keep);
            if b.flags & crate::map::FREE_BLOCK_FLAG != 0 && crate::cli::has(args, "--author-free-swaps") {
                // opt-in: a FREE vanilla block removed and re-added as a custom block with AR's
                // author field (flags | 0x8000). Not the default: a renamed-in-place custom block
                // loads fine (the Spring 2026 - 12 refusal was chunk 0x03043065, not this), and
                // the re-add path writes no waypoint node for a free START/CP block (Winter 2026
                // - 14 [Stadium] Wet Wood came out unreadable, 2026-09-22)
                if crate::cli::has(args, "--no-free-swaps") {
                    *unswapped.entry(format!("{} (free block)", b.name)).or_default() += 1;
                    continue;
                }
                free_swaps.push((b.index, target.clone(), (flags & !0x8000) | 0x8000));
                *swapped.entry(b.name.clone()).or_default() += 1;
                needed.insert(target.clone());
                continue;
            }
            swap_plan.push((b.index, target.clone(), flags));
            *swapped.entry(b.name.clone()).or_default() += 1;
            needed.insert(target.clone());
        }
    }
    // 2. items (the terrain environments, and any block a swap did not cover)
    let mut item_plan: Vec<(String, [f32; 3], [f32; 3])> = Vec::new();
    let swapped_idx: BTreeSet<usize> = swap_plan.iter().map(|s| s.0).collect();
    let mut itemed: BTreeMap<String, usize> = BTreeMap::new();
    for b in &source.blocks {
        if swapped_idx.contains(&b.index) || b.flags & crate::map::FREE_BLOCK_FLAG != 0 {
            continue;
        }
        let key = norm_block(&b.name, &rules.stems);
        if !rules.items.keys().any(|(k, _, _)| *k == key) {
            continue;
        }
        if !wants(b, &mut skipped_undriven) {
            continue;
        }
        let dir = b.dir & 3;
        let (cx, cy, cz) = b.coords();
        let corner = [cx as f32 * 32.0, cy as f32 * 8.0 + ground, cz as f32 * 32.0];
        let var = variant_bits(b.flags);
        // this variant + dir; else this dir with any variant; else another quarter turn rotated
        let exact = rules.items.get(&(key.clone(), dir, var)).and_then(|c| c.iter().max_by_key(|c| c.3)).or_else(|| {
            rules.items.iter().filter(|((k, d, _), _)| *k == key && *d == dir).flat_map(|(_, v)| v.iter()).max_by_key(|c| c.3)
        });
        let placement = match exact {
            Some((model, off, rot, _)) => Some((model.clone(), [corner[0] + off[0], corner[1] + off[1], corner[2] + off[2]], *rot)),
            None => {
                // another quarter turn known: rotate its offset about the cell centre — only
                // for a piece whose known placements all sit at the cell centre (a 1×1 piece;
                // a longer piece pivots on its footprint and a rotated guess lands a cell off —
                // Summer 2026 - 04's tilt transition became a wall across the road)
                let centred = rules.items.iter().filter(|((k, _, _), _)| *k == key).flat_map(|(_, v)| v.iter()).all(|c| (c.1[0] - 16.0).abs() < 0.1 && (c.1[2] - 16.0).abs() < 0.1);
                if !centred {
                    *skipped_undriven.entry(format!("{} (dir {dir} never seen)", b.name)).or_default() += 1;
                    continue;
                }
                (0u8..4).filter_map(|d| rules.items.iter().filter(|((k, dd, _), _)| *k == key && *dd == d).flat_map(|(_, v)| v.iter()).max_by_key(|c| c.3).map(|c| (d, c))).next().map(|(d, (model, off, rot, _))| {
                    let delta = dir_yaw(dir) - dir_yaw(d);
                    let c = [corner[0] + 16.0, corner[1], corner[2] + 16.0];
                    let local = [off[0] - 16.0, off[1], off[2] - 16.0];
                    (model.clone(), from_local(local, c, delta), [norm_angle(rot[0] + delta), rot[1], rot[2]])
                })
            }
        };
        let Some((model, pos, rot)) = placement else { continue };
        if lib.get(&model).is_none() {
            continue;
        }
        item_plan.push((model.clone(), pos, rot));
        *itemed.entry(b.name.clone()).or_default() += 1;
        needed.insert(model);
    }
    // 3. water under the start and every checkpoint (wet)
    let mut water: Vec<crate::map::FreeBlockSpec> = Vec::new();
    // a Stadium-collection CUSTOM block is dropped by a terrain-environment map (Winter
    // 2026 - 03 Wet Wood on RedIsland: wetness stayed 0) — there the pad is the stock
    // `PlatformWaterRampBase` (the U10S wet-wood pad; a Stadium block, but stock
    // Stadium blocks are what those maps' roads are made of), same 4 cm sheet
    // WET in a terrain environment is not possible (2026-09-22, Winter 2026 - 03 on
    // RedIsland): the stock PlatformWaterRampBase fails the load there, AR's no-mesh
    // Stadium water block is dropped silently, re-collected to the map's collection it
    // crashes the client, and the environment's own water ZONE tile (`Water`/`Lake`/`Sea`)
    // as a free block loads but wets nothing — the sea is one fixed plane, not a tile's
    // volume. `--wet` on such a map refuses (`--zone-pad` keeps the experiment).
    // --lake-pad: the environment's OWN lake block (RedIsland `DecoLakeCurve2In`, a 2×2×2
    // piece carrying a real water volume, sheet at local +7.7 per its block info) as a
    // free block under the start — the vista twin of the Stadium water pad (vjeux
    // 2026-09-22: "very sus that you can't have water blocks in the new environments")
    let zone_pad: Option<(&str, f32)> = if let Some(spec) = crate::cli::flag(args, "--lake-pad") {
        let (name, h) = spec.split_once(':').unwrap_or((spec, "7.7"));
        Some((Box::leak(name.to_string().into_boxed_str()), h.parse().unwrap_or(7.7)))
    } else if crate::cli::has(args, "--zone-pad") {
        match collection {
            0x10 => Some(("Water", 7.5)),
            0x1d => Some(("Water", 7.0)),
            0xf => Some(("Lake", 7.2)),
            0x1c => Some(("Sea", 7.0)),
            _ => None,
        }
    } else {
        None
    };
    let stock_pad = !is_stadium && zone_pad.is_none() && crate::cli::has(args, "--stock-pad");
    if wet && !is_stadium && zone_pad.is_none() && !stock_pad {
        return Err(format!("{}: a terrain-environment map (collection {collection:#x}) cannot be made WET — no water block loads there (see altered.rs)", input.display()));
    }
    if wet {
        let author: Option<String> = if stock_pad || zone_pad.is_some() { None } else { Some(lib.get(&water_block).ok_or_else(|| format!("the library has no {water_block}"))?.0) };
        let water_block = if let Some((z, _)) = zone_pad { z.to_string() } else if stock_pad { "PlatformWaterRampBase".to_string() } else { water_block.clone() };
        let flags: u32 = if stock_pad || zone_pad.is_some() { std::env::var("ALTERED_PAD_FLAGS").ok().and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(0) } else { 0x8040 };
        // the sheet's height in the pad's own frame: 2.0 for the Stadium water blocks
        let sheet = zone_pad.map(|(_, h)| h).unwrap_or(2.0);
        let dy = 2.04 - sheet; // block base relative to the waypoint's cell floor (deck = floor + 2)
        // the pad's base so that the sheet sits 4 cm over the waypoint block's DRIVING
        // surface. The deck heights are measured from the validation ghosts (car centre
        // over the cell floor, p50 of 100–2000 samples per family, 2026-09-22): tech /
        // platform / open roads / bump 2.0; RoadDirt 0.55 (the bumpy dirt road — the
        // tech-height pad put 60 cm of water on a dirt CP: json9841 "CP1 not passable");
        // RallyRoad 1.5; RoadIce (the bobsleigh trough) 2.3. AR's own `water` rows were
        // not usable here: a single dirt pad 30 m from its CP taught −0.77.
        let deck_of = |name: &str| -> f32 {
            let base = name.rsplit(['\\', '/']).next().unwrap_or(name).trim_start_matches("M2_");
            if base.starts_with("RoadDirt") {
                0.55
            } else if base.starts_with("RallyRoad") || base.starts_with("RallyCastleRoad") {
                1.5
            } else if base.starts_with("RoadIce") {
                2.3
            } else {
                2.0
            }
        };
        let pad_dy_for = |name: &str| -> f32 {
            if zone_pad.is_some() || stock_pad {
                return dy;
            }
            deck_of(name) + 0.04 - sheet
        };
        for b in &source.blocks {
            let tag = b.waypoint_tag.as_deref().unwrap_or("");
            if !(tag == "Spawn" || tag == "Checkpoint" || tag == "Multilap" || tag == "StartFinish") {
                continue;
            }
            let dy = pad_dy_for(&b.name);
            let (cx, cy, cz) = b.coords();
            let pos = match b.free_pos {
                Some(p) => [p[0] - 16.0, p[1] - 2.0 + dy, p[2] - 16.0],
                None => [cx as f32 * 32.0, cy as f32 * 8.0 + ground + dy, cz as f32 * 32.0],
            };
            water.push(crate::map::FreeBlockSpec { name: water_block.clone(), author: author.clone(), flags, pos, rot: [0.0, 0.0, 0.0], grid: None, dir: 0 });
        }
        for it in &source.items {
            let tag = it.waypoint_tag.as_deref().unwrap_or("");
            if !(tag == "Spawn" || tag == "Checkpoint" || tag == "Multilap" || tag == "StartFinish") {
                continue;
            }
            water.push(crate::map::FreeBlockSpec { name: water_block.clone(), author: author.clone(), flags, pos: [it.pos[0] - 16.0, it.pos[1] - 2.0 + dy, it.pos[2] - 16.0], rot: [0.0, 0.0, 0.0], grid: None, dir: 0 });
        }
        if !stock_pad && zone_pad.is_none() {
            needed.insert(water_block.clone());
        }
    }
    println!("  swaps: {} blocks ({} names); items: {} placements ({} block names); water pads: {}", swap_plan.len(), swapped.len(), item_plan.len(), itemed.len(), water.len());
    if !skipped_undriven.is_empty() {
        let mut v: Vec<(&String, &usize)> = skipped_undriven.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        println!("  left as they are (off the route): {}", v.iter().take(12).map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join(", "));
    }
    if !unswapped.is_empty() {
        let mut v: Vec<(&String, &usize)> = unswapped.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        println!("  no wood twin: {}", v.iter().take(30).map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join(", "));
    }

    // --- write: renames + flags (in place), then items, then free blocks, then the archive, then the name/uid
    let tmp = out.with_extension("tmp1.Map.Gbx");
    let mut m = MapFile::load(&input);
    for (i, target, _) in &swap_plan {
        m.set_block_name(*i, target);
    }
    m.write_to(&tmp).map_err(|e| e.to_string())?;
    let mut m = MapFile::load(&tmp);
    for (i, _, flags) in &swap_plan {
        m.set_block_flags(*i, *flags);
    }
    m.write_to(&tmp).map_err(|e| e.to_string())?;
    if !free_swaps.is_empty() {
        // the free swaps: the vanilla records removed, custom records with AR's author added
        let mut m = MapFile::load(&tmp);
        let mut adds: Vec<crate::map::FreeBlockSpec> = Vec::new();
        for (i, target, flags) in &free_swaps {
            let b = &m.blocks[*i];
            let author = lib.get(target).map(|(a, _, _)| a).unwrap_or_default();
            adds.push(crate::map::FreeBlockSpec { name: target.clone(), author: Some(author), flags: *flags, pos: b.free_pos.unwrap_or([0.0; 3]), rot: b.free_rot.unwrap_or([0.0; 3]), grid: None, dir: 0 });
        }
        let drop: BTreeSet<usize> = free_swaps.iter().map(|(i, _, _)| *i).collect();
        let r = m.remove_and_add_blocks(|b| drop.contains(&b.index), |_| false, &adds);
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        println!("  free swaps: {} vanilla free blocks re-added as custom blocks with the author field ({:?})", free_swaps.len(), r);
    }
    if !item_plan.is_empty() {
        let mut m = MapFile::load(&tmp);
        if m.items.is_empty() {
            let g = crate::gbx::Gbx::parse(&std::fs::read(&tmp).map_err(|e| e.to_string())?);
            let body = crate::map::seed_item_record(&g.body, collection)?;
            std::fs::write(&tmp, g.write_body_recompressed(&body)).map_err(|e| e.to_string())?;
            let mut m = MapFile::load(&tmp);
            m.move_item(0, [8.0, -900.0, 8.0], 0.0, crate::tiny::cell_for([8.0, -900.0, 8.0]));
            m.write_to(&tmp).map_err(|e| e.to_string())?;
        }
        let mut m = MapFile::load(&tmp);
        crate::tiny::set_ground(collection);
        let first = m.items.len();
        m.append_item_clones(first + item_plan.len());
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        let mut m = MapFile::load(&tmp);
        for (k, (model, pos, rot)) in item_plan.iter().enumerate() {
            let i = first + k;
            // the record wears what AR's records wear: his author id, the ITEM FILE's
            // collection (Stadium 0x1a even inside a WhiteShore map — a mismatch drops
            // the item silently), the pivot the item was authored with
            let (author, flags, scale, coll, pivot) = rules.itemrec.get(model).cloned().unwrap_or((String::new(), 1, 1.0, collection, [0.0; 3]));
            m.set_item_model(i, model);
            if !author.is_empty() {
                m.set_item_author(i, &author);
            }
            m.set_item_collection(i, coll);
            m.move_item(i, *pos, rot[0], crate::tiny::cell_for(*pos));
            m.set_item_frame(i, *rot, pivot);
            m.set_item_scale(i, scale);
            m.clear_item_variant(i);
            m.set_item_flags(i, flags);
        }
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        let mut m = MapFile::load(&tmp);
        for k in 0..item_plan.len() {
            m.set_item_waypoint(first + k, None, 0);
        }
        m.write_to(&tmp).map_err(|e| e.to_string())?;
    }
    if !water.is_empty() {
        let mut m = MapFile::load(&tmp);
        let r = m.remove_and_add_blocks(|_| false, |_| false, &water);
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        println!("  water: {} free blocks added ({:?})", water.len(), r);
    }
    // the archive: the source's own embedded files + the library files we need
    {
        let mut m = MapFile::load(&tmp);
        let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut rows: Vec<(String, String)> = Vec::new();
        if let Some((src_rows, src_zip)) = m.embedded_objects() {
            for (n, b) in crate::header::zip_entries(&src_zip) {
                files.insert(n, b);
            }
            rows = src_rows;
        }
        let mut recollected = 0usize;
        for ident in &needed {
            let (author, entry, mut bytes) = lib.get(ident).ok_or_else(|| format!("library: {ident}"))?;
            // a custom BLOCK in a terrain-environment map: the file's Ident takes the map's
            // collection (a Stadium-collection block is dropped silently there)
            if !is_stadium && ident.ends_with("_CustomBlock") {
                match recollect_file(&bytes, collection) {
                    Ok(b) => {
                        bytes = b;
                        recollected += 1;
                    }
                    Err(e) => println!("  {}: not re-collected ({e})", file_stem(ident)),
                }
            }
            files.entry(entry).or_insert(bytes);
            let key = ident.trim_end_matches("_CustomBlock").to_string();
            if !rows.iter().any(|(i, _)| *i == key) {
                rows.push((key, author));
            }
        }
        let zip = crate::header::deflated_zip(&files);
        let manifest: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        m.replace_embedded_objects(&manifest, &zip);
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        println!("  archive: {} files, {} manifest rows, {} bytes{}", files.len(), rows.len(), zip.len(), if recollected > 0 { format!(", {recollected} custom blocks re-collected to {collection:#x}") } else { String::new() });
    }
    {
        let mut m = MapFile::load(&tmp);
        let hdr = crate::header::read(tmp.to_str().unwrap_or_default()).ok();
        let old_name = hdr.as_ref().map(|h| h.name.clone()).unwrap_or_default();
        let old_uid = hdr.as_ref().map(|h| h.uid.clone()).unwrap_or_default();
        let new_name = crate::stadium::gen_name(&f("--name").unwrap_or_else(|| format!("{old_name} {}", if wet { "Wet Wood" } else { "Wood" })));
        // deterministic since 2026-09-22: prefix + the source uid's tail (a rebuild updates
        // the same record); --random-uid for the old behaviour
        let prefix = f("--uid-prefix").unwrap_or_else(|| if wet { "WetW".into() } else { "Wood".into() });
        let uid = if crate::cli::has(args, "--random-uid") { new_uid(&prefix, old_uid.len().max(5)) } else { crate::stadium::stable_uid(&prefix, &old_uid) };
        m.set_map_uid(&uid);
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        let mut m = MapFile::load(&tmp);
        if !old_name.is_empty() && new_name != old_name {
            m.set_map_name(&old_name, &new_name);
        }
        m.write_to(&out).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&tmp);
        println!("  -> {} ({})", out.display(), new_name);
    }
    Ok(())
}


/// `tmmaps altered-itemdump MAP --model SUBSTR`: the raw record bytes of the matching
/// items (hex) — for comparing a placement we wrote with one the editor wrote.
pub fn itemdump_cmd(args: &[String]) -> Result<(), String> {
    let input = args.iter().skip(1).find(|a| a.ends_with(".Map.Gbx")).ok_or("MAP")?;
    let want = crate::cli::flag(args, "--model").unwrap_or("");
    let m = MapFile::load(Path::new(input));
    let body = &m.gbx.body;
    let take: usize = crate::cli::flag(args, "--take").and_then(|v| v.parse().ok()).unwrap_or(3);
    for it in m.items.iter().filter(|it| it.model.contains(want)).take(take) {
        let (a, b) = it.record_region;
        println!("item {} {} at ({:.1},{:.2},{:.1}) yaw {:.4} pitch {:.4} roll {:.4} pivot {:?} scale {} flags {:#x} author {:?} coll {:#x} cell {:?} waypoint {:?} skin {:?} record {}..{} ({} bytes)", it.index, file_stem(&it.model), it.pos[0], it.pos[1], it.pos[2], it.yaw, it.pitch, it.roll, it.pivot, it.scale, it.flags, it.author, it.collection_raw, it.file_cell, it.waypoint_tag, it.skin_region, a, b, b - a);
        let hex: String = body[a..b].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ");
        println!("  {hex}");
    }
    Ok(())
}

/// Re-collect a custom block / item FILE: its Ident's collection word (header chunk
/// 0x2E001003 and body chunk 0x2E00100B — the u32 right after the path Id) set to
/// `collection`. A Stadium-collection (26) custom block is dropped silently by a
/// terrain-environment map (Winter 2026 - 03 Wet Wood: the no-mesh water pad never
/// appeared); the map's own collection word makes it load there.
pub fn recollect_file(bytes: &[u8], collection: u32) -> Result<Vec<u8>, String> {
    fn patch_ident(data: &mut [u8], start: usize, from: u32) -> Option<usize> {
        // start = offset of the first Id word (after the version / lookback words)
        let mut p = start;
        let w = u32::from_le_bytes(data.get(p..p + 4)?.try_into().ok()?);
        p += 4;
        if w & 0x3FFF_FFFF == 0 && w >> 30 == 1 {
            let len = u32::from_le_bytes(data.get(p..p + 4)?.try_into().ok()?) as usize;
            p += 4 + len;
        }
        let c = u32::from_le_bytes(data.get(p..p + 4)?.try_into().ok()?);
        if c == from {
            Some(p)
        } else {
            None
        }
    }
    let g = crate::gbx::Gbx::parse(bytes);
    let mut out = bytes.to_vec();
    let mut patched = 0usize;
    // header chunks: id/size table after the 21-byte header
    let n = u32::from_le_bytes(bytes[17..21].try_into().unwrap()) as usize;
    let mut off = 21;
    let mut chunks = Vec::new();
    for _ in 0..n {
        let id = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        let sz = u32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap()) & 0x7FFF_FFFF;
        off += 8;
        chunks.push((id, sz as usize));
    }
    let mut hdr = off;
    for (id, sz) in chunks {
        if id == 0x2E00_1003 {
            // version u32, then the Ident (path, collection, author)
            for from in [26u32, 28, 29, 16, 15] {
                if let Some(p) = patch_ident(&mut out[hdr..hdr + sz], 4, from) {
                    out[hdr + p..hdr + p + 4].copy_from_slice(&collection.to_le_bytes());
                    patched += 1;
                    if std::env::var("ALTERED_DEBUG").is_ok() {
                        eprintln!("    recollect: header word {from} → {collection} at file offset {}", hdr + p);
                    }
                    break;
                }
            }
        }
        hdr += sz;
    }
    // body chunk 0x2E00100B
    let mut body = g.body.clone();
    let key = 0x2E00_100Bu32.to_le_bytes();
    if let Some(i) = body.windows(4).position(|w| w == key) {
        let mut start = i + 4;
        // the body's first Id carries the lookback version word (3) first
        if body.get(start..start + 4).map(|w| u32::from_le_bytes(w.try_into().unwrap())) == Some(3) {
            let next = body.get(start + 4..start + 8).map(|w| u32::from_le_bytes(w.try_into().unwrap())).unwrap_or(0);
            if next >> 30 == 1 {
                start += 4;
            }
        }
        for from in [26u32, 28, 29, 16, 15] {
            if let Some(p) = patch_ident(&mut body[..], start, from) {
                body[p..p + 4].copy_from_slice(&collection.to_le_bytes());
                patched += 1;
                if std::env::var("ALTERED_DEBUG").is_ok() {
                    eprintln!("    recollect: body word {from} → {collection} at body offset {p} (chunk at {i})");
                }
                break;
            }
        }
        if std::env::var("ALTERED_DEBUG").is_ok() {
            let hex: String = body[i..(i + 140).min(body.len())].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ");
            eprintln!("    body chunk 0x2E00100B bytes: {hex}");
            // every other u32 == 26 in the body (other collection words?)
            let hits: Vec<usize> = (0..body.len() - 3).step_by(1).filter(|&k| u32::from_le_bytes(body[k..k + 4].try_into().unwrap()) == 26).collect();
            eprintln!("    body u32==26 at offsets: {:?} (body {} bytes)", hits, body.len());
        }
    } else if std::env::var("ALTERED_DEBUG").is_ok() {
        eprintln!("    recollect: no 0x2E00100B chunk in the body");
    }
    if patched == 0 {
        return Err("no collection word found to patch".into());
    }
    let g2 = crate::gbx::Gbx::parse(&out);
    Ok(g2.write_body_recompressed(&body))
}

pub fn recollect_cmd(args: &[String]) -> Result<(), String> {
    let input = args.iter().skip(1).find(|a| !a.starts_with("--") && (a.ends_with(".Gbx") || a.ends_with(".gbx"))).ok_or("FILE.Block.Gbx --collection N --out OUT")?;
    let coll: u32 = crate::cli::flag(args, "--collection").and_then(|v| v.parse().ok()).ok_or("--collection N")?;
    let out = crate::cli::flag(args, "--out").ok_or("--out")?;
    let bytes = std::fs::read(input).map_err(|e| e.to_string())?;
    let patched = recollect_file(&bytes, coll)?;
    std::fs::write(out, &patched).map_err(|e| e.to_string())?;
    println!("{input} -> {out}: collection {coll}, {} bytes", patched.len());
    Ok(())
}

/// `tmmaps altered-route MAP…`: how much of the author's route runs on BUILT blocks
/// (Stadium road / platform / structure pieces — what a "migrate to Stadium" keeps)
/// versus on the environment's terrain (Land, hills, beach, cliffs — what Stadium has
/// no counterpart for). One row per map: size, collection, ghost samples, built share,
/// the terrain block names driven, the terrain-conformed `On…` road variants used.
pub fn route_cmd(args: &[String]) -> Result<(), String> {
    println!("map\tenvir\tsize\tsamples\tbuilt%\tterrain_driven\ton_variants");
    for input in args.iter().skip(1).filter(|a| a.ends_with(".Map.Gbx")) {
        let m = MapFile::load(Path::new(input));
        let collection = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
        let ground = crate::map::ground_y(collection);
        let Ok(d) = gbx::record::decode_ghost(input) else {
            println!("{input}\t{collection:#x}\t{:?}\t0\t-\tno ghost\t", m.size);
            continue;
        };
        let built = |name: &str| -> bool {
            let n = name.trim_start_matches("M2_");
            FAMILIES.iter().any(|f| n.starts_with(f)) || n.starts_with("Structure") || n.starts_with("Gate") || n.starts_with("Technics") || n.contains("_CustomBlock")
        };
        let mut by_cell: HashMap<(i32, i32), Vec<(f32, &str)>> = HashMap::new();
        for b in &m.blocks {
            let (cx, cy, cz) = b.coords();
            let floor = match b.free_pos {
                Some(p) => p[1],
                None => cy as f32 * 8.0 + ground,
            };
            by_cell.entry((cx, cz)).or_default().push((floor, b.name.as_str()));
        }
        let mut pts: Vec<[f32; 3]> = Vec::new();
        let mut prev: Option<[f32; 3]> = None;
        for s in &d.samples {
            let p = [s.x, s.y, s.z];
            if let Some(q) = prev {
                let dist = ((p[0] - q[0]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
                let n = (dist / 4.0).ceil().max(1.0) as usize;
                for k in 1..=n {
                    let t = k as f32 / n as f32;
                    pts.push([q[0] + (p[0] - q[0]) * t, q[1] + (p[1] - q[1]) * t, q[2] + (p[2] - q[2]) * t]);
                }
            } else {
                pts.push(p);
            }
            prev = Some(p);
        }
        let mut n_built = 0usize;
        let mut terrain: BTreeMap<String, usize> = BTreeMap::new();
        let mut airborne = 0usize;
        for p in &pts {
            let (cx, cz) = ((p[0] / 32.0).floor() as i32, (p[2] / 32.0).floor() as i32);
            let mut any_built = false;
            let mut nearest_terrain: Option<(f32, &str)> = None;
            for dx in -1..=1 {
                for dz in -1..=1 {
                    if let Some(v) = by_cell.get(&(cx + dx, cz + dz)) {
                        for (floor, name) in v {
                            let d = p[1] - floor;
                            if d >= -3.0 && d <= 12.0 {
                                if built(name) {
                                    any_built = true;
                                } else if dx == 0 && dz == 0 && nearest_terrain.map(|(bd, _)| d.abs() < bd.abs()).unwrap_or(true) {
                                    nearest_terrain = Some((d, name));
                                }
                            }
                        }
                    }
                }
            }
            if any_built {
                n_built += 1;
            } else if let Some((_, name)) = nearest_terrain {
                *terrain.entry(name.trim_end_matches(|c: char| c.is_ascii_digit()).to_string()).or_default() += 1;
            } else {
                airborne += 1;
            }
        }
        let on_variants: BTreeSet<String> = m.blocks.iter().filter(|b| built(&b.name) && (b.name.contains("OnLand") || b.name.contains("OnDirt") || b.name.contains("OnGrass") || b.name.contains("OnSea") || b.name.contains("OnSnow") || b.name.contains("OnWater"))).map(|b| b.name.clone()).collect();
        let total = pts.len().max(1);
        let mut t: Vec<(&String, &usize)> = terrain.iter().collect();
        t.sort_by(|a, b| b.1.cmp(a.1));
        let terrain_share = (total - n_built - airborne) as f32 * 100.0 / total as f32;
        println!(
            "{}\t{collection:#x}\t{}x{}x{}\t{}\t{:.0}\tterrain {:.0}% ({}) air {:.0}%\t{} On-variants ({})",
            Path::new(input).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            m.size[0],
            m.size[1],
            m.size[2],
            total,
            n_built as f32 * 100.0 / total as f32,
            terrain_share,
            t.iter().take(4).map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join(" "),
            airborne as f32 * 100.0 / total as f32,
            on_variants.len(),
            on_variants.iter().take(3).map(|s| s.as_str()).collect::<Vec<_>>().join(",")
        );
    }
    Ok(())
}


/// `tmmaps altered-unzip MAP --out DIR`: the map's embedded archive extracted.
pub fn unzip_cmd(args: &[String]) -> Result<(), String> {
    let input = args.iter().skip(1).find(|a| a.ends_with(".Map.Gbx")).ok_or("MAP")?;
    let out = PathBuf::from(crate::cli::flag(args, "--out").ok_or("--out DIR")?);
    let m = MapFile::load(Path::new(input));
    let (rows, zip) = m.embedded_objects().ok_or("no embedded archive")?;
    let mut n = 0usize;
    for (name, bytes) in crate::header::zip_entries(&zip) {
        let p = out.join(name.replace('\\', "/"));
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
        n += 1;
    }
    println!("{input}: {n} files, {} manifest rows → {}", rows.len(), out.display());
    Ok(())
}


/// `tmmaps altered-addblock MAP --out OUT --name BLOCK (--pos x,y,z [--yaw R] | --grid cx,cy,cz [--dir D]) [--flags HEX]
/// [--author A]`: one stock (or embedded custom) block added — the "what happens if you put a normal
/// water block in a vista map" probe (vjeux 2026-09-22).
pub fn addblock_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let input = PathBuf::from(args.iter().skip(1).find(|a| !a.starts_with("--") && a.ends_with(".Map.Gbx")).ok_or("MAP")?);
    let out = PathBuf::from(f("--out").ok_or("--out")?);
    let name = f("--name").ok_or("--name")?;
    let flags = f("--flags").map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or(0)).unwrap_or(0);
    let v3 = |s: &str| -> [f32; 3] {
        let p: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap_or(0.0)).collect();
        [p[0], p[1], p[2]]
    };
    let spec = if let Some(g) = f("--grid") {
        let c = v3(&g);
        crate::map::FreeBlockSpec { name: name.clone(), author: f("--author"), flags, pos: [0.0; 3], rot: [0.0; 3], grid: Some([c[0] as i32, c[1] as i32, c[2] as i32]), dir: f("--dir").and_then(|d| d.parse().ok()).unwrap_or(0) }
    } else {
        let pos = v3(&f("--pos").ok_or("--pos x,y,z or --grid cx,cy,cz")?);
        crate::map::FreeBlockSpec { name: name.clone(), author: f("--author"), flags, pos, rot: [f("--yaw").and_then(|y| y.parse().ok()).unwrap_or(0.0), 0.0, 0.0], grid: None, dir: 0 }
    };
    let mut m = MapFile::load(&input);
    let r = m.remove_and_add_blocks(|_| false, |_| false, &[spec]);
    m.write_to(&out).map_err(|e| e.to_string())?;
    // --keep-uid: the editor-refusal bisect of 2026-09-24 (is it the add, or the new uid?)
    if !crate::cli::has(args, "--keep-uid") {
        let mut m = MapFile::load(&out);
        let hdr = crate::header::read(out.to_str().unwrap_or_default()).ok();
        let uid_len = hdr.as_ref().map(|h| h.uid.len()).unwrap_or(27);
        m.set_map_uid(&new_uid("Blk", uid_len));
        m.write_to(&out).map_err(|e| e.to_string())?;
    }
    println!("{} + {name} → {} ({:?})", input.display(), out.display(), r);
    Ok(())
}


/// `tmmaps effect-catalog HOST.Map.Gbx --list BLOCKS.tsv --out OUT.Map.Gbx [--cols 11] [--pitch 3] [--y 12]`:
/// the exhaustive effect-block audit map (vjeux 2026-09-24 "one map with all of them aligned in
/// a grid, one screenshot of the front and one of the back"). The host is emptied (every
/// authored block and item removed), then every row of BLOCKS.tsv (`name<TAB>flags-hex`, one
/// distinct effect block kind per row, e.g. the census of the 975 sources) is placed as a
/// GRID block facing north (dir 0) on a `pitch`-cell lattice — cell (x0 + 3*col, y, z0 + 3*row),
/// so each block's generated skirts have room and a camera sees it alone. A ground-flagged
/// kind (0x1000) goes on the ground row (cell y 9), the rest float at `--y`. The grid TSV
/// beside OUT lists `name, flags, cell, world centre` for the cameras. Save the result
/// through the EDITOR once (the game generates the fillers on save) before converting it.
pub fn effect_catalog_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let host = PathBuf::from(args.iter().skip(1).find(|a| !a.starts_with("--") && a.ends_with(".Map.Gbx")).ok_or("HOST.Map.Gbx")?);
    let list = PathBuf::from(f("--list").ok_or("--list BLOCKS.tsv")?);
    let out = PathBuf::from(f("--out").ok_or("--out")?);
    let cols: i32 = f("--cols").and_then(|v| v.parse().ok()).unwrap_or(11);
    let pitch: i32 = f("--pitch").and_then(|v| v.parse().ok()).unwrap_or(3);
    let y_air: i32 = f("--y").and_then(|v| v.parse().ok()).unwrap_or(12);
    let (x0, z0) = (4i32, 4i32);
    let text = std::fs::read_to_string(&list).map_err(|e| format!("{}: {e}", list.display()))?;
    let mut rows: Vec<(String, u32)> = Vec::new();
    for line in text.lines() {
        let mut it = line.split('\t');
        let (Some(name), Some(flags)) = (it.next(), it.next()) else { continue };
        if name.is_empty() || name.starts_with('#') {
            continue;
        }
        let flags = u32::from_str_radix(flags.trim().trim_start_matches("0x"), 16).map_err(|e| format!("{line}: flags {e}"))?;
        rows.push((name.to_string(), flags));
    }
    let mut specs: Vec<crate::map::FreeBlockSpec> = Vec::new();
    let mut tsv = String::from("name\tflags\tcx\tcy\tcz\twx\twy\twz\n");
    for (i, (name, flags)) in rows.iter().enumerate() {
        let (col, row) = ((i as i32) % cols, (i as i32) / cols);
        let ground = flags & 0x1000 != 0;
        let cell = [x0 + col * pitch, if ground { 9 } else { y_air }, z0 + row * pitch];
        // grid → world (Stadium: 32 m cells, 8 m rows, the map's y origin at −62 like `census`)
        let world = [cell[0] as f32 * 32.0 + 16.0, cell[1] as f32 * 8.0 - 62.0, cell[2] as f32 * 32.0 + 16.0];
        tsv.push_str(&format!("{name}\t{flags:08x}\t{}\t{}\t{}\t{:.1}\t{:.1}\t{:.1}\n", cell[0], cell[1], cell[2], world[0], world[1], world[2]));
        specs.push(crate::map::FreeBlockSpec { name: name.clone(), author: None, flags: *flags, pos: [0.0; 3], rot: [0.0; 3], grid: Some(cell), dir: 0 });
    }
    // a START (the race must begin for the playground camera to yield to /lookat) and a
    // FINISH, in the grid's south-west corner, out of every block's frame
    specs.push(crate::map::FreeBlockSpec { name: "PlatformTechStart".into(), author: None, flags: 0x1000, pos: [0.0; 3], rot: [0.0; 3], grid: Some([1, 9, 1]), dir: 0 });
    specs.push(crate::map::FreeBlockSpec { name: "PlatformTechFinish".into(), author: None, flags: 0x1000, pos: [0.0; 3], rot: [0.0; 3], grid: Some([1, 9, 2]), dir: 0 });
    let mut m = MapFile::load(&host);
    // --keep-baked: the host's generated records stay (a file with NO baked record may be what
    // the editor refuses — probe of 2026-09-24)
    let keep_baked = crate::cli::has(args, "--keep-baked");
    let r = m.remove_and_add_blocks(|_| true, |_| !keep_baked, &specs);
    m.write_to(&out).map_err(|e| e.to_string())?;
    // every original item REMOVED (parking them under the map put them outside the 48×40×48
    // volume — a likely reason the editor refused the file, 2026-09-24)
    let mut m = MapFile::load(&out);
    let n_items = m.items.len();
    let keep: Vec<bool> = vec![false; n_items];
    m.remove_items(&keep);
    m.write_to(&out).map_err(|e| e.to_string())?;
    let mut m = MapFile::load(&out);
    let hdr = crate::header::read(out.to_str().unwrap_or_default()).ok();
    let uid_len = hdr.as_ref().map(|h| h.uid.len()).unwrap_or(27);
    m.set_map_uid(&new_uid("Cat", uid_len));
    m.write_to(&out).map_err(|e| e.to_string())?;
    if !crate::cli::has(args, "--keep-genealogy") {
        let _ = MapFile::clear_genealogy_file(&out);
    }
    let tsv_path = out.with_extension("grid.tsv");
    std::fs::write(&tsv_path, tsv).map_err(|e| e.to_string())?;
    println!("{}: {} blocks on a {cols}-wide grid (pitch {pitch}), {} items parked, host cleared {:?}; grid {}", out.display(), rows.len(), n_items, r, tsv_path.display());
    Ok(())
}

/// `tmmaps block-lineup HOST.Map.Gbx --inventory inv.tsv --list NAMES.txt --out-prefix P
/// [--lanes N] [--width 46] [--depth 44] [--y 9] [--gap 1] [--start-x 1 --start-z 3]`:
/// the review map(s) of a block family (vjeux 2026-10-03: "inventory of all the black
/// blocks and pool blocks, put them in a map side by side and build the dirt variant on
/// the next row; different ways to convert → different rows"). Every name of NAMES.txt
/// (one per line; `#` comments) is placed `lanes` times as a GRID block facing north on
/// the ground row: lane 0 at the block's slot, lane k directly behind it (+z), so a
/// reviewer compares the original with each converted lane at a glance. Slots are
/// packed by the block's footprint from `mapgeom blockinfo-all --footprints` (x/z cell
/// extents, `gap` cells between blocks); a block row is `lanes × (max z extent + gap)`
/// deep; when the next row would leave the map's `depth`, a new map starts
/// (`<P>-1.Map.Gbx`, `<P>-2.Map.Gbx`, …). The host is emptied like `effect-catalog`
/// (its generated records kept), a start and a finish go in the south-west corner.
/// Beside every map: `<map>.lanes.tsv` — `name lane cx cy cz wx wy wz` — and
/// `<map>.lane<k>.Map.Gbx` for every k: the same map with ONLY lane k's blocks (lane 0 =
/// the originals; a conversion run takes a lane map; `tmmaps merge` folds them back).
pub fn block_lineup_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let host = PathBuf::from(args.iter().skip(1).find(|a| !a.starts_with("--") && a.ends_with(".Map.Gbx")).ok_or("HOST.Map.Gbx")?);
    let inv = PathBuf::from(f("--inventory").ok_or("--inventory inv.tsv (mapgeom blockinfo-all --footprints)")?);
    let list = PathBuf::from(f("--list").ok_or("--list NAMES.txt")?);
    let prefix = f("--out-prefix").ok_or("--out-prefix P")?;
    let lanes: i32 = f("--lanes").and_then(|v| v.parse().ok()).unwrap_or(2).max(1);
    let width: i32 = f("--width").and_then(|v| v.parse().ok()).unwrap_or(46);
    let depth: i32 = f("--depth").and_then(|v| v.parse().ok()).unwrap_or(44);
    let y: i32 = f("--y").and_then(|v| v.parse().ok()).unwrap_or(9);
    let gap: i32 = f("--gap").and_then(|v| v.parse().ok()).unwrap_or(1);
    let x0: i32 = f("--start-x").and_then(|v| v.parse().ok()).unwrap_or(1);
    let z0: i32 = f("--start-z").and_then(|v| v.parse().ok()).unwrap_or(3);
    // --dir D: every block's cardinal (0 = the inventory's own, i.e. a road's axis along z).
    // A drive test along a ROW (which runs along x) wants the road pieces turned onto the
    // row's axis: `--dir 3` (2026-10-06, the water-road physics measurement).
    let block_dir: u8 = f("--dir").and_then(|v| v.parse().ok()).unwrap_or(0);
    // --lane: ONE drivable line instead of a grid — every name in the list is laid in
    // the order given, one footprint after another along +z in a single column, with the
    // start pad in front of it. That is the shape the physics bench needs: the car drives
    // through every kind in one run and the wheel log segments by z (2026-10-07).
    let lane_mode = crate::cli::has(args, "--lane");
    // footprints: name → (x, z) cell extents of the ground (else air) variant
    let mut foot: std::collections::BTreeMap<String, (i32, i32)> = std::collections::BTreeMap::new();
    for line in std::fs::read_to_string(&inv).map_err(|e| format!("{}: {e}", inv.display()))?.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() >= 7 {
            if let (Ok(x), Ok(z)) = (c[4].parse::<i32>(), c[6].parse::<i32>()) {
                foot.insert(c[0].to_string(), (x.max(1), z.max(1)));
            }
        }
    }
    let names: Vec<String> = std::fs::read_to_string(&list).map_err(|e| format!("{}: {e}", list.display()))?.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    let mut missing: Vec<&String> = names.iter().filter(|n| !foot.contains_key(*n)).collect();
    if !missing.is_empty() {
        missing.truncate(8);
        return Err(format!("{} name(s) not in the inventory, e.g. {:?}", names.iter().filter(|n| !foot.contains_key(*n)).count(), missing));
    }
    // deepest footprints first (then widest, then the name): rows stay uniform in
    // depth instead of one arch making a whole row of 1×1 pools 3 cells deep
    // (--keep-order leaves the list order alone)
    let mut names = names;
    if !crate::cli::has(args, "--keep-order") {
        names.sort_by(|a, b| foot[b].1.cmp(&foot[a].1).then(foot[b].0.cmp(&foot[a].0)).then(a.cmp(b)));
    }
    // shelf packing: rows of blocks across x; each row `lanes` lanes deep
    struct Slot { name: String, cx: i32, cz: i32, fz: i32 }
    let mut maps: Vec<Vec<Slot>> = vec![Vec::new()];
    let mut row: Vec<(String, i32, i32)> = Vec::new(); // name, x, fz
    let mut x = x0;
    let mut z = z0;
    let flush_row = |row: &mut Vec<(String, i32, i32)>, z: &mut i32, maps: &mut Vec<Vec<Slot>>| {
        if row.is_empty() {
            return;
        }
        let pitch_z = row.iter().map(|r| r.2).max().unwrap_or(1) + gap;
        if *z + lanes * pitch_z > z0 + depth && !maps.last().unwrap().is_empty() {
            maps.push(Vec::new());
            *z = z0;
        }
        for (name, rx, fz) in row.drain(..) {
            maps.last_mut().unwrap().push(Slot { name, cx: rx, cz: *z, fz: pitch_z });
            let _ = fz;
        }
        *z += lanes * pitch_z;
    };
    if lane_mode {
        // one column: every name in order along +z, footprint after footprint
        let mut z = z0;
        let mut slots: Vec<Slot> = Vec::new();
        for n in &names {
            let (_fx, fz) = foot.get(n).copied().unwrap_or((1, 1));
            slots.push(Slot { name: n.clone(), cx: x0, cz: z, fz });
            z += fz.max(1) + gap;
        }
        maps.push(slots);
    }
    for n in &names {
        if lane_mode {
            break;
        }
        let (fx, fz) = foot[n];
        if x + fx > x0 + width && !row.is_empty() {
            flush_row(&mut row, &mut z, &mut maps);
            x = x0;
        }
        row.push((n.clone(), x, fz));
        x += fx + gap;
    }
    flush_row(&mut row, &mut z, &mut maps);
    let hdr = crate::header::read(host.to_str().unwrap_or_default()).ok();
    let uid_len = hdr.as_ref().map(|h| h.uid.len()).unwrap_or(27);
    let mut total = 0usize;
    for (mi, slots) in maps.iter().enumerate() {
        let out = PathBuf::from(format!("{prefix}-{}.Map.Gbx", mi + 1));
        let mut tsv = String::from("name\tlane\tcx\tcy\tcz\twx\twy\twz\n");
        let mut per_lane: Vec<Vec<crate::map::FreeBlockSpec>> = vec![Vec::new(); lanes as usize];
        for s in slots {
            for lane in 0..lanes {
                let cell = [s.cx, y, s.cz + lane * s.fz];
                let world = [cell[0] as f32 * 32.0 + 16.0, cell[1] as f32 * 8.0 - 62.0, cell[2] as f32 * 32.0 + 16.0];
                tsv.push_str(&format!("{}\t{lane}\t{}\t{}\t{}\t{:.1}\t{:.1}\t{:.1}\n", s.name, cell[0], cell[1], cell[2], world[0], world[1], world[2]));
                // on the ground row the GROUND variant (flag 0x1000); anywhere above, the AIR
                // variant (Everios96 2026-10-03 on the first lineup at ground level: "invisible
                // parts everywhere, maybe place them not at ground level")
                let flags = if y <= 9 { 0x1000 } else { 0 };
                per_lane[lane as usize].push(crate::map::FreeBlockSpec { name: s.name.clone(), author: None, flags, pos: [0.0; 3], rot: [0.0; 3], grid: Some(cell), dir: block_dir });
            }
        }
        let gates = [
            crate::map::FreeBlockSpec { name: "PlatformTechStart".into(), author: None, flags: if y <= 9 { 0x1000 } else { 0 }, pos: [0.0; 3], rot: [0.0; 3], grid: Some([0, y, 0]), dir: 0 },
            crate::map::FreeBlockSpec { name: "PlatformTechFinish".into(), author: None, flags: if y <= 9 { 0x1000 } else { 0 }, pos: [0.0; 3], rot: [0.0; 3], grid: Some([0, y, 1]), dir: 0 },
        ];
        // the full map (every lane) and one map per converted lane (lane k only)
        let mut variants: Vec<(PathBuf, Vec<crate::map::FreeBlockSpec>)> = Vec::new();
        let all: Vec<crate::map::FreeBlockSpec> = per_lane.iter().flatten().cloned().chain(gates.iter().cloned()).collect();
        variants.push((out.clone(), all));
        for lane in 0..lanes as usize {
            let specs: Vec<crate::map::FreeBlockSpec> = per_lane[lane].iter().cloned().chain(gates.iter().cloned()).collect();
            variants.push((PathBuf::from(format!("{prefix}-{}.lane{lane}.Map.Gbx", mi + 1)), specs));
        }
        for (path, specs) in variants {
            let mut m = MapFile::load(&host);
            // the host's generated records go, except the terrain zone plates (the
            // ground the lineup stands on); the editor regenerates the rest on save
            let _ = m.remove_and_add_blocks(|_| true, |b| !matches!(b.name.as_str(), "Grass" | "Dirt" | "Snow" | "Ice" | "Sand" | "Rock"), &specs);
            m.write_to(&path).map_err(|e| e.to_string())?;
            let mut m = MapFile::load(&path);
            let n_items = m.items.len();
            m.remove_items(&vec![false; n_items]);
            m.write_to(&path).map_err(|e| e.to_string())?;
            let mut m = MapFile::load(&path);
            m.set_map_uid(&new_uid("Cat", uid_len));
            m.write_to(&path).map_err(|e| e.to_string())?;
            // the name is a variable-length splice: after the uid rename, on a reloaded file
            let mut m = MapFile::load(&path);
            if let Some(stem) = f("--name") {
                let old = hdr.as_ref().map(|h| h.name.clone()).unwrap_or_default();
                let lane_tag = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let lane_tag = lane_tag.split(".lane").nth(1).map(|t| format!(" lane {}", t.trim_end_matches(".Map.Gbx"))).unwrap_or_default();
                if !old.is_empty() {
                    let _ = m.set_map_name(&old, &format!("{stem} {}/{}{lane_tag}", mi + 1, maps.len()));
                }
            }
            m.write_to(&path).map_err(|e| e.to_string())?;
            let _ = MapFile::clear_genealogy_file(&path);
        }
        std::fs::write(out.with_extension("lanes.tsv"), &tsv).map_err(|e| e.to_string())?;
        total += slots.len();
        println!("{}: {} blocks × {lanes} lanes (rows end at z {})", out.display(), slots.len(), slots.iter().map(|s| s.cz + lanes * s.fz).max().unwrap_or(z0));
    }
    println!("{} blocks over {} map(s)", total, maps.len());
    Ok(())
}

/// `tmmaps merge BASE.Map.Gbx OTHER.Map.Gbx… --out F`: BASE plus every authored block,
/// item placement and embedded item file of the OTHER maps (their generated records are
/// not carried: the editor rebuilds them on save). Two archives naming the same file with
/// different bytes is an error — give each conversion run its own alias base. The result
/// gets a fresh uid. Built for the lane review maps of `block-lineup` (2026-10-03).
pub fn merge_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let out = PathBuf::from(f("--out").ok_or("--out F")?);
    // positionals only: a flag's VALUE (`--out X.Map.Gbx`) is not a map to merge
    let mut maps: Vec<PathBuf> = Vec::new();
    let mut skip_next = false;
    for a in args.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if a.starts_with("--") {
            skip_next = true;
            continue;
        }
        if a.ends_with(".Map.Gbx") {
            maps.push(PathBuf::from(a));
        }
    }
    if maps.len() < 2 {
        return Err("merge needs BASE and at least one OTHER map".into());
    }
    let base = &maps[0];
    let mut m = MapFile::load(base);
    let mut specs: Vec<crate::map::FreeBlockSpec> = Vec::new();
    struct NewItem { model: String, author: Option<String>, collection: u32, pos: [f32; 3], yaw: f32, frame: Option<([f32; 3], [f32; 3])>, scale: f32, variant: u8, color: u8, tag: Option<String>, order: u32, skin: Option<crate::header::FileRef> }
    let mut new_items: Vec<NewItem> = Vec::new();
    let mut files: std::collections::BTreeMap<String, Vec<u8>> = std::collections::BTreeMap::new();
    let mut manifest: Vec<(String, String)> = Vec::new();
    fn take_archive(mm: &MapFile, manifest: &mut Vec<(String, String)>, files: &mut std::collections::BTreeMap<String, Vec<u8>>) -> Result<(), String> {
        if let Some((rows, zip)) = mm.embedded_objects() {
            for (name, author) in rows {
                if !manifest.iter().any(|(n, _)| *n == name) {
                    manifest.push((name, author));
                }
            }
            for (name, bytes) in crate::header::zip_entries(&zip) {
                let key = name.replace('\\', "/");
                match files.get(&key) {
                    Some(old) if *old != bytes => return Err(format!("archive conflict: {key} differs between the merged maps (use distinct alias bases)")),
                    Some(_) => {}
                    None => { files.insert(key, bytes); }
                }
            }
        }
        Ok(())
    }
    take_archive(&m, &mut manifest, &mut files)?;
    // --shift-z A,B,…: cells to move each OTHER map's content along +z (one value per
    // OTHER map, or one for all) — a lineup lane converted from the saved lane-0 source
    // lands in its own lane; items follow (×32 m)
    let shifts: Vec<i32> = f("--shift-z").map(|v| v.split(',').filter_map(|t| t.trim().parse().ok()).collect()).unwrap_or_default();
    // --lanes-tsv L.lanes.tsv --lane K[,K2,…]: a `block-lineup` lane table; every block and
    // item of OTHER map i (converted from the saved lane-0 map) moves from its row's lane-0
    // z to lane K_i's z of the SAME row (rows have their own lane pitch)
    let lane_rows: Vec<(i32, i32, i32)> = f("--lanes-tsv").map(|p| {
        std::fs::read_to_string(&p).unwrap_or_default().lines().skip(1).filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c.len() >= 5).then(|| (c[1].parse::<i32>().unwrap_or(0), c[4].parse::<i32>().unwrap_or(0), c[2].parse::<i32>().unwrap_or(0)))
        }).collect()
    }).unwrap_or_default(); // (lane, cz, cx)
    let lane_targets: Vec<i32> = f("--lane").map(|v| v.split(',').filter_map(|t| t.trim().parse().ok()).collect()).unwrap_or_default();
    let lane_dz = |cz0: i32, lane: i32| -> i32 {
        // the row whose lane-0 z band holds cz0: the greatest lane-0 cz <= cz0
        // (+1: a wall's SOUTH face piece sits one cell before its row's first cell, in the
        // previous row's gap cell — the one-cell gap keeps it inside this row's reach)
        let row_z0 = lane_rows.iter().filter(|(l, z, _)| *l == 0 && *z <= cz0 + 1).map(|(_, z, _)| *z).max().unwrap_or(cz0);
        let target = lane_rows.iter().filter(|(l, _, _)| *l == lane).map(|(_, z, _)| *z).filter(|z| *z > row_z0).min();
        // lane k's z of this row = the k-th lane-z above row_z0 among this row's lanes: the
        // table lists lane k of a row at row_z0 + k × pitch, so the first lane-k z above
        // row_z0 that belongs to the same row is the one closest to row_z0
        // shift = lane k's row z − lane 0's row z (a block's offset inside its row is kept)
        target.map(|z| z - row_z0).unwrap_or(0)
    };
    for (oi, other) in maps[1..].iter().enumerate() {
        let lane_k = lane_targets.get(oi).or(lane_targets.first()).copied();
        let dz_fixed: i32 = shifts.get(oi).or(shifts.first()).copied().unwrap_or(0);
        let o = MapFile::load(other);
        let mut dup = 0;
        for b in &o.blocks {
            let (cx, cy, cz0) = b.coords();
            let dz = match lane_k { Some(k) => lane_dz(cz0, k), None => dz_fixed };
            let cz = cz0 + dz;
            let grid = if b.free_pos.is_some() { None } else { Some([cx, cy, cz]) };
            // a grid block on a cell the BASE already fills is the same lineup fixture
            // (the start/finish gates every lane map carries): not doubled
            if grid.is_some() && m.blocks.iter().any(|bb| bb.free_pos.is_none() && bb.coords() == (cx, cy, cz)) {
                dup += 1;
                continue;
            }
            let mut pos = b.free_pos.unwrap_or([0.0; 3]);
            pos[2] += dz as f32 * 32.0;
            // the waypoint-presence bit (0x00100000: an editor-saved start/finish/checkpoint
            // record carries a waypoint node) goes — the spec writer carries no node, and
            // the bit without the node mis-parses the record's tail (W-1 merge, 2026-10-04)
            specs.push(crate::map::FreeBlockSpec { name: b.name.clone(), author: None, flags: b.flags & !0x0010_0000, pos, rot: b.free_rot.unwrap_or([0.0; 3]), grid, dir: b.dir });
        }
        if dup > 0 {
            println!("  {}: {dup} grid block(s) on cells the base fills skipped", other.display());
        }
        let colors = o.colors();
        let mut parked = 0;
        for it in &o.items {
            // the converter's seed placement (a RoadSign parked at y −900 when the source
            // had no item to clone): not a lineup fixture, and under the map volume
            if it.pos[1] < -500.0 {
                parked += 1;
                continue;
            }
            let dz = match lane_k { Some(k) => lane_dz((it.pos[2] / 32.0).floor() as i32, k), None => dz_fixed };
            new_items.push(NewItem {
                model: it.model.clone(),
                author: it.author.clone(),
                collection: it.collection_raw,
                pos: [it.pos[0], it.pos[1], it.pos[2] + dz as f32 * 32.0],
                yaw: it.yaw,
                frame: Some(([it.pitch, it.yaw, it.roll], it.pivot)),
                scale: it.scale,
                variant: it.variant(),
                color: colors.as_ref().map(|c| c.item(it.index)).unwrap_or(0),
                tag: it.waypoint_tag.clone(),
                order: it.waypoint_order,
                skin: it.skin(&o.gbx.body),
            });
        }
        take_archive(&o, &mut manifest, &mut files)?;
        println!("  {}: {} blocks, {} items ({parked} parked seed item(s) dropped), archive now {} files", other.display(), o.blocks.len(), o.items.len(), files.len());
    }
    let tmp = out.with_extension("merge-stage.Map.Gbx");
    let _ = m.remove_and_add_blocks(|_| false, |_| false, &specs);
    m.write_to(&tmp).map_err(|e| e.to_string())?;
    let mut m = MapFile::load(&tmp);
    let first = m.items.len();
    if !new_items.is_empty() {
        if first == 0 {
            // no clone donor in the base: seed one record (parked under the map, dropped
            // below with the other parked seeds) — the converter's own trick
            let g = crate::gbx::Gbx::parse(&std::fs::read(&tmp).map_err(|e| e.to_string())?);
            let collection = new_items.first().map(|it| it.collection).unwrap_or(26);
            let body = crate::map::seed_item_record(&g.body, collection)?;
            std::fs::write(&tmp, g.write_body_recompressed(&body)).map_err(|e| e.to_string())?;
            let mut m0 = MapFile::load(&tmp);
            m0.move_item(0, [8.0, -900.0, 8.0], 0.0, crate::tiny::cell_for([8.0, -900.0, 8.0]));
            m0.write_to(&tmp).map_err(|e| e.to_string())?;
            m = MapFile::load(&tmp);
        }
        let first = m.items.len();
        // the clones exist in the FILE only after a write: reload before addressing them
        m.append_item_clones(first + new_items.len());
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        m = MapFile::load(&tmp);
        for (k, it) in new_items.iter().enumerate() {
            let i = first + k;
            m.set_item_model(i, &it.model);
            if let Some(a) = &it.author {
                m.set_item_author(i, a);
            }
            m.set_item_collection(i, it.collection);
            // the item cell: 32 m columns, 8 m rows from the Stadium item ground at −64
            // (the converted lane maps' own items read back that way)
            let cell = ((it.pos[0] / 32.0).floor().clamp(0.0, 254.0) as i32, ((it.pos[1] + 64.0) / 8.0).floor().clamp(0.0, 255.0) as i32, (it.pos[2] / 32.0).floor().clamp(0.0, 254.0) as i32);
            m.move_item(i, it.pos, it.yaw, cell);
            if let Some((rot, pivot)) = it.frame {
                m.set_item_frame(i, rot, pivot);
            }
            m.set_item_scale(i, it.scale);
            if it.variant != 0 {
                m.set_item_variant(i, it.variant);
            } else {
                m.clear_item_variant(i);
            }
            m.set_item_color(i, it.color);
        }
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        let mut m2 = MapFile::load(&tmp);
        for (k, it) in new_items.iter().enumerate() {
            m2.set_item_waypoint(first + k, it.tag.as_deref(), it.order);
        }
        m2.write_to(&tmp).map_err(|e| e.to_string())?;
        let mut m3 = MapFile::load(&tmp);
        for (k, it) in new_items.iter().enumerate() {
            if it.skin.is_some() {
                m3.set_item_skin(first + k, it.skin.as_ref());
            }
        }
        m3.write_to(&tmp).map_err(|e| e.to_string())?;
        m = MapFile::load(&tmp);
    }
    if !manifest.is_empty() {
        let zip = crate::header::deflated_zip(&files);
        let rows: Vec<(&str, &str)> = manifest.iter().map(|(n, a)| (n.as_str(), a.as_str())).collect();
        m.replace_embedded_objects(&rows, &zip);
    }
    m.write_to(&tmp).map_err(|e| e.to_string())?;
    let mut m = MapFile::load(&tmp);
    let hdr = crate::header::read(base.to_str().unwrap_or_default()).ok();
    m.set_map_uid(&new_uid("Mrg", hdr.as_ref().map(|h| h.uid.len()).unwrap_or(27)));
    m.write_to(&tmp).map_err(|e| e.to_string())?;
    // --name N: the merged map's name (a variable-length splice: on the reloaded file)
    let mut m = MapFile::load(&tmp);
    if let Some(new_name) = f("--name") {
        let old = hdr.as_ref().map(|h| h.name.clone()).unwrap_or_default();
        if !old.is_empty() {
            let _ = m.set_map_name(&old, &new_name);
        }
    }
    m.write_to(&tmp).map_err(|e| e.to_string())?;
    // the BASE's own parked seed placement goes the same way
    let mut m = MapFile::load(&tmp);
    let keep: Vec<bool> = m.items.iter().map(|it| it.pos[1] >= -500.0).collect();
    if keep.iter().any(|k| !k) {
        m.remove_items(&keep);
    }
    m.write_to(&out).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    let chk = MapFile::load(&out);
    println!("{}: {} blocks, {} items, {} embedded files", out.display(), chk.blocks.len(), chk.items.len(), manifest.len());
    Ok(())
}

/// `tmmaps swap-items LIVE.Map.Gbx --swap STOCK=F.Item.Gbx[,STOCK2=G.Item.Gbx…] --out OUT
/// [--uid-prefix XXXX]`: every placement of the stock item STOCK (`RampMedv2`) is
/// re-pointed at the embedded item F (its ident = its file name, its own author, the
/// map's collection), F joins the map's archive and manifest; nothing else in the map
/// changes — the in-place form of an item conversion (the ramp round, 2026-10-03: a
/// full rebuild would have replayed every later round's choices, the live file is the
/// truth). `--uid-prefix` gives the result a fresh uid (first four characters replaced):
/// a records reset, the mapper's decision.
pub fn swap_items_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let live = PathBuf::from(args.iter().skip(1).find(|a| !a.starts_with("--") && a.ends_with(".Map.Gbx")).ok_or("LIVE.Map.Gbx")?);
    let out = PathBuf::from(f("--out").ok_or("--out OUT")?);
    let swaps: Vec<(String, PathBuf)> = f("--swap").ok_or("--swap STOCK=F.Item.Gbx[,…]")?.split(',').filter(|s| !s.trim().is_empty()).map(|s| {
        let (a, b) = s.split_once('=').ok_or_else(|| format!("--swap entry `{s}`: want STOCK=F.Item.Gbx"))?;
        Ok::<_, String>((a.trim().to_string(), PathBuf::from(b.trim())))
    }).collect::<Result<_, _>>()?;
    let mut m = MapFile::load(&live);
    let collection = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let mut manifest: Vec<(String, String)> = Vec::new();
    let mut zip: Vec<u8> = Vec::new();
    if let Some((rows, z)) = m.embedded_objects() {
        manifest = rows;
        zip = z;
    }
    let mut swapped = 0usize;
    for (stock, file) in &swaps {
        let ident = file.file_name().map(|n| n.to_string_lossy().to_string()).ok_or("item file name")?;
        if !ident.ends_with(".Item.Gbx") {
            return Err(format!("{ident}: an embedded item is named *.Item.Gbx"));
        }
        let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let hits: Vec<usize> = m.items.iter().filter(|it| it.model == *stock).map(|it| it.index).collect();
        if hits.is_empty() {
            return Err(format!("{stock}: no placement in {}", live.display()));
        }
        let author = crate::surface::item_author(&ident);
        for &i in &hits {
            m.set_item_model(i, &ident);
            m.set_item_author(i, &author);
            m.set_item_collection(i, collection);
            m.clear_item_variant(i);
        }
        swapped += hits.len();
        if !manifest.iter().any(|(n, _)| *n == ident) {
            manifest.push((ident.clone(), author.clone()));
            zip = if zip.is_empty() {
                let mut files = std::collections::BTreeMap::new();
                files.insert(format!("Items/{ident}"), bytes);
                crate::header::deflated_zip(&files)
            } else {
                crate::header::zip_add(&zip, &format!("Items/{ident}"), &bytes)
            };
        }
        println!("  {stock}: {} placement(s) -> {ident}", hits.len());
    }
    // the renames are fixed-size patches; the archive is a variable-length splice:
    // write, reload, then splice
    let tmp = out.with_extension("swap-stage.Map.Gbx");
    m.write_to(&tmp).map_err(|e| e.to_string())?;
    let mut m = MapFile::load(&tmp);
    let rows: Vec<(&str, &str)> = manifest.iter().map(|(n, a)| (n.as_str(), a.as_str())).collect();
    m.replace_embedded_objects(&rows, &zip);
    m.write_to(&tmp).map_err(|e| e.to_string())?;
    let mut m = MapFile::load(&tmp);
    if let Some(p) = f("--uid-prefix") {
        if p.len() != 4 {
            return Err("--uid-prefix is 4 characters".into());
        }
        let old = crate::header::read(live.to_str().unwrap_or_default()).map(|h| h.uid).map_err(|e| e.to_string())?;
        m.set_map_uid(&format!("{p}{}", &old[4..]));
        m.remove_password();
    }
    m.write_to(&out).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    let chk = MapFile::load(&out);
    let hdr = crate::header::read(out.to_str().unwrap_or_default()).map_err(|e| e.to_string())?;
    println!("{}: {swapped} placement(s) swapped, {} items, {} embedded files, uid {}", out.display(), chk.items.len(), manifest.len(), hdr.uid);
    Ok(())
}

/// `tmmaps restart MAP --cell x,y,z --dir D --out F [--block PlatformTechStart]`: the map's
/// start block(s) (`Platform*Start`, `Road*Start`) replaced by ONE start at the given cell
/// facing D (0 north/+z, 1 east/+x, 2 south/−z, 3 west/−x). A review camera: the race
/// opens behind the car on the start, looking the way the start faces — so the start is
/// where the camera must stand (the MediaTracker intro is not played by the plugin's
/// /playmap, 2026-10-04). The uid stays (a review file).
pub fn restart_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let map = PathBuf::from(args.iter().skip(1).find(|a| !a.starts_with("--") && a.ends_with(".Map.Gbx")).ok_or("MAP.Map.Gbx")?);
    let out = PathBuf::from(f("--out").ok_or("--out F")?);
    let dir: u8 = f("--dir").and_then(|d| d.parse().ok()).unwrap_or(0) & 3;
    let block = f("--block").unwrap_or_else(|| "PlatformTechStart".into());
    let mut m = MapFile::load(&map);
    let is_start = |n: &str| (n.starts_with("Platform") || n.starts_with("Road")) && n.ends_with("Start");
    // `--free x,y,z`: a FREE start at a world point (yaw from --dir), the map's own starts
    // removed — for a spot no grid cell reaches, e.g. right behind U10S 354's free-placed
    // custom finish (2026-10-09: the grid start two cells back hung over a gap)
    if let Some(free) = f("--free") {
        let p: Vec<f32> = free.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        if p.len() != 3 {
            return Err("--free wants x,y,z (world metres)".into());
        }
        let yaw = match dir { 0 => 0.0, 1 => std::f32::consts::FRAC_PI_2, 2 => std::f32::consts::PI, _ => -std::f32::consts::FRAC_PI_2 };
        let specs = vec![crate::map::FreeBlockSpec { name: block.clone(), author: None, flags: 0, pos: [p[0], p[1], p[2]], rot: [yaw, 0.0, 0.0], grid: None, dir: 0 }];
        let removed = m.remove_and_add_blocks(|b| is_start(&b.name), |_| false, &specs);
        m.write_to(&out).map_err(|e| e.to_string())?;
        let chk = MapFile::load(&out);
        let nb = chk.blocks.iter().find(|x| is_start(&x.name)).map(|x| (x.name.clone(), x.free_pos));
        println!("{}: {removed:?} start(s) removed; free {block} at {:?} yaw {yaw:.4} (reads back {:?})", out.display(), p, nb);
        return Ok(());
    }
    let cell: Vec<i32> = f("--cell").ok_or("--cell x,y,z (or --free x,y,z)")?.split(',').filter_map(|t| t.trim().parse().ok()).collect();
    if cell.len() != 3 {
        return Err("--cell wants x,y,z".into());
    }
    // in place when the map has a start: its cell and dir bytes patched, the record (and
    // its waypoint node — the editor-saved spawn) kept; a re-added node-less start block
    // did NOT move the spawn (2026-10-04: two different cells, the same frame)
    if let Some(b) = m.blocks.iter().find(|b| is_start(&b.name) && b.free_pos.is_none()).cloned() {
        m.raw_patches.push((b.coord_off, vec![(cell[0] + 1) as u8, cell[1] as u8, (cell[2] + 1) as u8]));
        m.raw_patches.push((b.coord_off - 1, vec![dir]));
        m.write_to(&out).map_err(|e| e.to_string())?;
        let chk = MapFile::load(&out);
        let nb = chk.blocks.iter().find(|x| is_start(&x.name)).map(|x| (x.coords(), x.dir));
        println!("{}: start {} moved in place to {:?} dir {dir} (reads back {:?})", out.display(), b.name, cell, nb);
        return Ok(());
    }
    let specs = vec![crate::map::FreeBlockSpec { name: block.clone(), author: None, flags: if cell[1] <= 9 { 0x1000 } else { 0 }, pos: [0.0; 3], rot: [0.0; 3], grid: Some([cell[0], cell[1], cell[2]]), dir }];
    let _ = m.remove_and_add_blocks(|_| false, |_| false, &specs);
    m.write_to(&out).map_err(|e| e.to_string())?;
    let chk = MapFile::load(&out);
    println!("{}: no start block; {block} added at {:?} dir {dir}; {} blocks", out.display(), cell, chk.blocks.len());
    Ok(())
}

/// `tmmaps set-times MAP --author MS [--gold MS --silver MS --bronze MS] --out F`: the medal
/// times in the header (0x03043002 + the XML) and the body's challenge parameters (where
/// the game reads them). A map without times (the editor's save of a merged lineup:
/// author −1) is refused by Nadeo's upload ("validation error", 2026-10-04). Missing
/// medals default to author × 1.06 / 1.2 / 1.5 (whole seconds rounded up).
pub fn set_times_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let map = PathBuf::from(args.iter().skip(1).find(|a| !a.starts_with("--") && a.ends_with(".Map.Gbx")).ok_or("MAP.Map.Gbx")?);
    let out = PathBuf::from(f("--out").ok_or("--out F")?);
    let author: u32 = f("--author").ok_or("--author MS")?.parse().map_err(|_| "--author wants milliseconds")?;
    let medal = |k: &str, factor: f64| -> u32 { f(k).and_then(|v| v.parse().ok()).unwrap_or_else(|| (((author as f64) * factor / 1000.0).ceil() * 1000.0) as u32) };
    let times = [author, medal("--gold", 1.06), medal("--silver", 1.2), medal("--bronze", 1.5)];
    let hdr = crate::header::read(map.to_str().unwrap_or_default()).map_err(|e| e.to_string())?;
    let old_at: u32 = hdr.authortime.parse::<i64>().map(|v| if v < 0 { u32::MAX } else { v as u32 }).map_err(|_| format!("author time `{}` in the header", hdr.authortime))?;
    let mut m = MapFile::load(&map);
    let nick = m.author_nickname().unwrap_or_default();
    let (hh, bh) = m.set_author_and_times(&hdr.author, &hdr.author, &nick, &nick, old_at, times)?;
    m.set_body_times(old_at, times)?;
    m.write_to(&out).map_err(|e| e.to_string())?;
    let back = MapFile::load(&out);
    match back.body_times() {
        Some(t) if t == times => {}
        other => return Err(format!("the body times read back as {other:?}, not {times:?}")),
    }
    println!("{}: times author {:.3} gold {:.3} silver {:.3} bronze {:.3} s ({hh} header hits, {bh} body hits)", out.display(), times[0] as f64 / 1000.0, times[1] as f64 / 1000.0, times[2] as f64 / 1000.0, times[3] as f64 / 1000.0);
    Ok(())
}

/// `tmmaps drop-blocks MAP --name A[,B…] --out F`: remove every authored record of those
/// block names (and the generated faces the game would grow around them die with the
/// block). Used to pull a block out of a review lineup — `TrackWallWithHoleWater` is a
/// WATER wall whose name does not start with `TrackWallWater`, so the water filter missed
/// it and it flickered in the dirt lane (Everios96 2026-10-05: "the flickering block is
/// the last block of the file").
pub fn drop_blocks_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| crate::cli::flag(args, k).map(String::from);
    let map = PathBuf::from(args.iter().skip(1).find(|a| !a.starts_with("--") && a.ends_with(".Map.Gbx")).ok_or("MAP.Map.Gbx")?);
    let out = PathBuf::from(f("--out").ok_or("--out F")?);
    let names: Vec<String> = f("--name").ok_or("--name A[,B…]")?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let mut m = MapFile::load(&map);
    let hit = |n: &str| names.iter().any(|x| x == n);
    let dropped: Vec<String> = m.blocks.iter().filter(|b| hit(&b.name)).map(|b| b.name.clone()).collect();
    if dropped.is_empty() {
        return Err(format!("none of {names:?} is in {}", map.display()));
    }
    let _ = m.remove_and_add_blocks(|b| hit(&b.name), |_| false, &[]);
    m.write_to(&out).map_err(|e| e.to_string())?;
    let chk = MapFile::load(&out);
    println!("{}: dropped {} record(s) ({} left)", out.display(), dropped.len(), chk.blocks.len());
    Ok(())
}
