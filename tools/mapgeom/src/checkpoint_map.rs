//! `mapgeom checkpoint-map --out DIR [--hugo FILE.cs]` — Hugo's CheckpointsCleaned block mapping (2026-10-24 05:00):
//! every checkpoint BLOCK of every loaded collection → the plain block + DirectionOffset that leaves the same
//! drivable surface, found GEOMETRICALLY: the block's ground (else air) base variant is baked, its standable
//! collision (upward faces, |n.y| > 0.5) rasterised on a 1 m grid over the footprint keeping the LOWEST height per
//! square (the deck under the arch pillars, not the pillar tops), and every same-family plain block with a
//! matching (turned) footprint is compared under the four rotations of the block-frame rule (grid_block: dir 1
//! (x, z) → (sz − z, x), dir 2 → (sx − x, sz − z), dir 3 → (z, sx − x)); the mismatch = the fraction of squares
//! whose heights differ by more than 0.1 m or that only one of the two covers. Hugo's own entries are scored the
//! same way and disagreements reported. Writes MAPPING.tsv, CheckpointsCleaned.cs (his format and comments) and
//! the per-block details.
use std::collections::{BTreeMap, BTreeSet};

use crate::store::DataStore;

const TOLERANCE: f32 = 0.1;
/// A mapping is a MATCH under this mismatch fraction, APPROX under the second, else NONE.
const MATCH_MAX: f32 = 0.02;
pub const MATCH_MAX_PUB: f32 = MATCH_MAX;
const APPROX_MAX: f32 = 0.15;

/// TM2020 `EWayPointType`: Start 0, Finish 1, Checkpoint 2, None 3, StartFinish 4, Dispenser 5 (a block info without
/// the chunk reads as `None` here).
pub const WP_CHECKPOINT: i32 = 2;
pub const WP_NONE: i32 = 3;

pub fn is_plain(wp: Option<i32>) -> bool {
    matches!(wp, None | Some(WP_NONE))
}

/// A block's standable surface: per 1 m square (index z·(32·sx) + x) the lowest upward collision height. `all`
/// is the collision SILHOUETTE (the highest point of any collision face per square) — the test used for pieces
/// without a drivable deck (the `*WallCheckpoint*` pieces).
#[derive(Clone, Debug)]
pub struct HeightField {
    pub size: (i32, i32),
    pub h: Vec<Option<f32>>,
    pub all: Vec<Option<f32>>,
}

impl HeightField {
    fn w(&self) -> usize {
        32 * self.size.0 as usize
    }
    fn d(&self) -> usize {
        32 * self.size.1 as usize
    }
    fn at(&self, x: usize, z: usize) -> Option<f32> {
        self.h[z * self.w() + x]
    }
    /// The field turned by `dir` quarter turns under the block-frame rule (the turned footprint is (sz, sx) for
    /// odd turns).
    pub fn turned(&self, dir: u8) -> HeightField {
        let (sx, sz) = self.size;
        let size = if dir & 1 == 1 { (sz, sx) } else { (sx, sz) };
        let (w, d) = (32 * size.0 as usize, 32 * size.1 as usize);
        let mut h = vec![None; w * d];
        let mut all = vec![None; w * d];
        let (smx, smz) = (32.0 * sx as f32, 32.0 * sz as f32);
        for z in 0..self.d() {
            for x in 0..self.w() {
                let (lx, lz) = (x as f32 + 0.5, z as f32 + 0.5);
                let (wx, wz) = match dir & 3 {
                    0 => (lx, lz),
                    1 => (smz - lz, lx),
                    2 => (smx - lx, smz - lz),
                    _ => (lz, smx - lx),
                };
                let (ix, iz) = (wx.floor() as usize, wz.floor() as usize);
                if ix < w && iz < d {
                    h[iz * w + ix] = self.h[z * self.w() + x];
                    all[iz * w + ix] = self.all[z * self.w() + x];
                }
            }
        }
        HeightField { size, h, all }
    }
    pub fn covered(&self) -> usize {
        self.h.iter().filter(|v| v.is_some()).count()
    }
}

/// The mismatch fraction between two fields of the same size: squares that differ by more than `TOLERANCE` or
/// that only one covers, over the squares either covers. `None` when the sizes differ.
pub fn mismatch(a: &HeightField, b: &HeightField) -> Option<f32> {
    if a.size != b.size {
        return None;
    }
    // a piece without a drivable deck (a wall checkpoint: standable < 20 % of its footprint) is compared by the
    // TALL part of its collision silhouette — the squares within 1 m of its highest point, i.e. the wall itself;
    // the checkpoint's arch (lower) and the plain wall's skirt are ignored. Jaccard mismatch of the two sets.
    if (a.covered() as f32) < 0.2 * a.h.len() as f32 {
        // the threshold is the CHECKPOINT's top (− 1 m) for both: an 8 m wall never stands in for a 32 m one
        let top = a.all.iter().flatten().cloned().fold(f32::NEG_INFINITY, f32::max);
        let tall = |f: &HeightField| -> Vec<bool> { f.all.iter().map(|v| v.map_or(false, |y| y >= top - 1.0)).collect() };
        let (ta, tb) = (tall(a), tall(b));
        let (mut inter, mut union) = (0usize, 0usize);
        for (x, y) in ta.iter().zip(tb.iter()) {
            if *x || *y {
                union += 1;
            }
            if *x && *y {
                inter += 1;
            }
        }
        return Some(if union == 0 { 1.0 } else { 1.0 - inter as f32 / union as f32 });
    }
    let (pa, pb) = (&a.h, &b.h);
    let (mut bad, mut any) = (0usize, 0usize);
    for (x, y) in pa.iter().zip(pb.iter()) {
        match (x, y) {
            (None, None) => {}
            (Some(p), Some(q)) => {
                any += 1;
                if (p - q).abs() > TOLERANCE {
                    bad += 1;
                }
            }
            _ => {
                any += 1;
                bad += 1;
            }
        }
    }
    Some(if any == 0 { 0.0 } else { bad as f32 / any as f32 })
}

fn variant_size(v: &crate::blockinfo::Variant) -> (i32, i32) {
    if v.block_units.is_empty() {
        return (1, 1);
    }
    (v.block_units.iter().map(|u| u.offset[0] + 1).max().unwrap_or(1), v.block_units.iter().map(|u| u.offset[2] + 1).max().unwrap_or(1))
}

/// The standable surface of a variant: bake its mobils (translation + yaw as authored), keep the upward collision
/// faces, rasterise at 1 m keeping the lowest height per square.
pub fn variant_heightfield(store: &mut DataStore, v: &crate::blockinfo::Variant) -> Result<HeightField, String> {
    let size = variant_size(v);
    let mut m = crate::static_item::build::Merged::default();
    m.keep_water = true;
    let mut n = 0usize;
    for list in &v.mobils {
        for mobil in list {
            let Some(prefab) = mobil.prefab.as_deref() else { continue };
            let low = prefab.to_ascii_lowercase();
            if low.ends_with(".vegettreemodel.gbx") {
                continue;
            }
            let mut at = crate::geom::IDENTITY;
            if let Some(t) = mobil.translation {
                at[9] = t[0];
                at[10] = t[1];
                at[11] = t[2];
            }
            if let Some(r) = mobil.rotation.filter(|r| r.iter().any(|q| q.abs() > 1e-6)) {
                at = crate::geom::euler_xyz_deg(r, [at[9], at[10], at[11]]);
            }
            let r = if low.ends_with(".prefab.gbx") { crate::static_item::build::add_prefab(store, prefab, &at, 1.0, &mut m, 0) } else { crate::static_item::build::add_static_object_file(store, prefab, &at, 1.0, &mut m) };
            if r.is_ok() {
                n += 1;
            }
        }
    }
    if n == 0 {
        return Err("no mobil baked".into());
    }
    let (w, d) = (32 * size.0 as usize, 32 * size.1 as usize);
    let mut h: Vec<Option<f32>> = vec![None; w * d];
    let mut all: Vec<Option<f32>> = vec![None; w * d];
    for t in &m.surf_triangles {
        let pts: Vec<[f32; 3]> = t.indices.iter().filter_map(|&i| m.surf_vertices.get(i as usize).copied()).collect();
        if pts.len() < 3 {
            continue;
        }
        let (a, b, c) = (pts[0], pts[1], pts[2]);
        let (u, v2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let nrm = [u[1] * v2[2] - u[2] * v2[1], u[2] * v2[0] - u[0] * v2[2], u[0] * v2[1] - u[1] * v2[0]];
        let len = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        if len < 1e-9 {
            continue;
        }
        let standable = nrm[1] / len >= 0.5;
        // the silhouette: every face's highest vertex over the squares its xz box covers (walls are near-vertical:
        // a per-point height is meaningless, the box is what a car meets)
        {
            let (x0, x1) = (pts.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min), pts.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max));
            let (z0, z1) = (pts.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min), pts.iter().map(|p| p[2]).fold(f32::NEG_INFINITY, f32::max));
            let top = pts.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
            let (ix0, ix1) = (x0.floor().max(0.0) as usize, (x1.ceil() as isize).min(w as isize).max(0) as usize);
            let (iz0, iz1) = (z0.floor().max(0.0) as usize, (z1.ceil() as isize).min(d as isize).max(0) as usize);
            for iz in iz0..iz1 {
                for ix in ix0..ix1 {
                    let cell = &mut all[iz * w + ix];
                    *cell = Some(cell.map_or(top, |q: f32| q.max(top)));
                }
            }
        }
        if !standable {
            continue;
        }
        let (x0, x1) = (pts.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min), pts.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max));
        let (z0, z1) = (pts.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min), pts.iter().map(|p| p[2]).fold(f32::NEG_INFINITY, f32::max));
        let (ix0, ix1) = (x0.floor().max(0.0) as usize, (x1.ceil() as isize).min(w as isize).max(0) as usize);
        let (iz0, iz1) = (z0.floor().max(0.0) as usize, (z1.ceil() as isize).min(d as isize).max(0) as usize);
        let det = (b[0] - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (b[2] - a[2]);
        if det.abs() < 1e-9 {
            continue;
        }
        for iz in iz0..iz1 {
            for ix in ix0..ix1 {
                let p = [ix as f32 + 0.5, iz as f32 + 0.5];
                let l1 = ((b[0] - p[0]) * (c[2] - p[1]) - (c[0] - p[0]) * (b[2] - p[1])) / det;
                let l2 = ((c[0] - p[0]) * (a[2] - p[1]) - (a[0] - p[0]) * (c[2] - p[1])) / det;
                let l3 = 1.0 - l1 - l2;
                if l1 < -1e-4 || l2 < -1e-4 || l3 < -1e-4 {
                    continue;
                }
                let y = l1 * a[1] + l2 * b[1] + l3 * c[1];
                let cell = &mut h[iz * w + ix];
                *cell = Some(cell.map_or(y, |q: f32| q.min(y)));
            }
        }
    }
    Ok(HeightField { size, h, all })
}

/// One block of one collection: name, path, waypoint type, the variant used for the surface (ground, else air).
pub struct BlockRow {
    pub collection: String,
    pub name: String,
    pub path: String,
    pub waypoint_type: Option<i32>,
    pub size: (i32, i32),
    pub ground: bool,
}

/// Every block info of the loaded paks, by (collection, name). `label`: the collection name to stamp on every
/// row (the vista paks re-author Stadium block infos under `Stadium\…` paths, so the path's first folder is not
/// the collection — load ONE pak per run and label it by the pak).
pub fn all_blocks(store: &mut DataStore, label: Option<&str>) -> Vec<BlockRow> {
    all_blocks_from(store, label, None)
}

/// `all_blocks` restricted to the block-info paths in `only` (the collection's own pak, listed apart: its block
/// infos reference prefabs that live in the Stadium pak, so both are loaded for the bake).
pub fn all_blocks_from(store: &mut DataStore, label: Option<&str>, only: Option<&BTreeSet<String>>) -> Vec<BlockRow> {
    let paths: Vec<String> = store.entries().map(|e| e.path()).filter(|p| { let u = p.to_uppercase(); u.contains("\\GAMECTNBLOCKINFO\\") && u.ends_with(".GBX") }).filter(|p| only.map_or(true, |o| o.contains(&p.to_uppercase()))).collect();
    let mut out = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for p in &paths {
        let coll = label.map(|l| l.to_string()).unwrap_or_else(|| p.split('\\').next().unwrap_or("").to_string());
        let Ok(bi) = crate::blockinfo::load(store, p) else { continue };
        let name = if bi.name.is_empty() { p.rsplit('\\').next().unwrap_or(p).split('.').next().unwrap_or("").to_string() } else { bi.name.clone() };
        let (size, ground) = match (&bi.variant_base_ground, &bi.variant_base_air) {
            (Some(g), _) => (variant_size(g), true),
            (None, Some(a)) => (variant_size(a), false),
            _ => continue,
        };
        if !seen.insert(format!("{coll}/{name}")) {
            continue;
        }
        out.push(BlockRow { collection: coll, name, path: p.clone(), waypoint_type: bi.waypoint_type, size, ground });
    }
    out
}

/// The base variant a block's surface is compared on: ground, else air.
fn base_variant(bi: &crate::blockinfo::BlockInfo) -> Option<&crate::blockinfo::Variant> {
    bi.variant_base_ground.as_ref().or(bi.variant_base_air.as_ref())
}

/// The family prefix of a checkpoint block name: everything before "Checkpoint" (RoadTechDiagRightCheckpoint →
/// RoadTechDiagRight; PlatformTechCheckpointSlope2Down → PlatformTech).
pub fn family_prefix(name: &str) -> &str {
    match name.find("Checkpoint") {
        Some(i) => &name[..i],
        None => name,
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub name: String,
    pub dir: u8,
    pub mismatch: f32,
}

#[derive(Debug)]
pub struct Mapping {
    pub collection: String,
    pub block: String,
    pub size: (i32, i32),
    pub ground: bool,
    pub waypoint_type: Option<i32>,
    pub covered: usize,
    /// best first
    pub ranked: Vec<Candidate>,
    pub candidates_tried: usize,
}

pub fn offset_name(dir: u8) -> &'static str {
    match dir & 3 {
        0 => "None",
        1 => "Quarter",
        2 => "Half",
        _ => "ThreeQuarters",
    }
}

/// Map every checkpoint block of every loaded collection. `hugo`: his (block → (replacement, offset)) entries to
/// score alongside.
pub fn map_all(store: &mut DataStore, label: Option<&str>, only: Option<&BTreeSet<String>>, hugo: &BTreeMap<String, (String, u8)>, log: &mut Vec<String>) -> Result<Vec<Mapping>, String> {
    let blocks = all_blocks_from(store, label, only);
    log.push(format!("{} block infos over {} collections", blocks.len(), blocks.iter().map(|b| b.collection.clone()).collect::<BTreeSet<_>>().len()));
    let mut fields: BTreeMap<(String, String), Option<HeightField>> = BTreeMap::new();
    let mut field = |store: &mut DataStore, coll: &str, name: &str, path: &str, fields: &mut BTreeMap<(String, String), Option<HeightField>>| -> Option<HeightField> {
        let key = (coll.to_string(), name.to_string());
        if let Some(f) = fields.get(&key) {
            return f.clone();
        }
        let f = crate::blockinfo::load(store, path).ok().and_then(|bi| base_variant(&bi).map(|v| variant_heightfield(store, v))).and_then(|r| r.ok());
        fields.insert(key, f.clone());
        f
    };
    let mut out = Vec::new();
    let checkpoints: Vec<usize> = blocks.iter().enumerate().filter(|(_, b)| b.waypoint_type == Some(WP_CHECKPOINT) || (b.name.contains("Checkpoint") && b.waypoint_type.is_none())).map(|(i, _)| i).collect();
    for b in blocks.iter().filter(|b| b.name.contains("Checkpoint") && b.waypoint_type == Some(WP_NONE)) {
        log.push(format!("{}/{}: carries \"Checkpoint\" in its name but is no waypoint (a baked clip / filler piece) — not mapped", b.collection, b.name));
    }
    for &ci in &checkpoints {
        let k = &blocks[ci];
        let Some(kf) = field(store, &k.collection, &k.name, &k.path, &mut fields) else {
            log.push(format!("{}/{}: no bakeable base variant", k.collection, k.name));
            out.push(Mapping { collection: k.collection.clone(), block: k.name.clone(), size: k.size, ground: k.ground, waypoint_type: k.waypoint_type, covered: 0, ranked: Vec::new(), candidates_tried: 0 });
            continue;
        };
        let prefix = family_prefix(&k.name).to_string();
        // candidates: same collection, same family prefix (or the family's generic prefix when the checkpoint name
        // carries a shape before "Checkpoint", e.g. RoadTechDiagRight → RoadTech), no "Checkpoint", footprint matching
        // under some rotation
        let mut prefixes: Vec<String> = vec![prefix.clone()];
        for cut in ["Diag", "Slope", "Tilt", "Ring", "Trigger", "Center", "Left", "Right", "Up", "Down", "Narrow", "Wall", "Loop"] {
            if let Some(i) = prefix.find(cut) {
                if i > 0 {
                    prefixes.push(prefix[..i].to_string());
                }
            }
        }
        prefixes.sort_by_key(|p| std::cmp::Reverse(p.len()));
        prefixes.dedup();
        let mut ranked: Vec<Candidate> = Vec::new();
        let mut tried = 0usize;
        let mut names_tried: BTreeSet<String> = BTreeSet::new();
        for pre in &prefixes {
            for c in blocks.iter().filter(|c| c.collection == k.collection && !c.name.contains("Checkpoint") && is_plain(c.waypoint_type) && c.name.starts_with(pre.as_str()) && !is_gameplay(&c.name)) {
                if !names_tried.insert(c.name.clone()) {
                    continue;
                }
                let fits = |d: u8| if d & 1 == 1 { (c.size.1, c.size.0) == k.size } else { c.size == k.size };
                if !(0..4u8).any(fits) {
                    continue;
                }
                let Some(cf) = field(store, &c.collection, &c.name, &c.path, &mut fields) else { continue };
                tried += 1;
                for d in 0..4u8 {
                    if !fits(d) {
                        continue;
                    }
                    if let Some(mm) = mismatch(&kf, &cf.turned(d)) {
                        ranked.push(Candidate { name: c.name.clone(), dir: d, mismatch: mm });
                    }
                }
            }
            // the family's own prefix found good candidates: don't widen to the generic prefix
            if ranked.iter().any(|r| r.mismatch <= MATCH_MAX) {
                break;
            }
        }
        // Hugo's entry, scored too (even when its block is outside the candidate prefixes)
        if let Some((hname, hdir)) = hugo.get(&k.name) {
            if !ranked.iter().any(|r| &r.name == hname && r.dir == *hdir) {
                if let Some(c) = blocks.iter().find(|c| c.collection == k.collection && &c.name == hname) {
                    if let Some(cf) = field(store, &c.collection, &c.name, &c.path, &mut fields) {
                        let t = cf.turned(*hdir);
                        ranked.push(Candidate { name: hname.clone(), dir: *hdir, mismatch: mismatch(&kf, &t).unwrap_or(1.0) });
                    }
                }
            }
        }
        // ranking: the surface first (mismatch rounded to 0.5 %), then the semantics — a candidate whose name shares the
        // checkpoint's shape words (Slope2, Tilt, Diag, WithWall, …) beats one that merely has the same surface under
        // some rotation (a StraightTilt2 turned a quarter IS a slope), then "Straight"/"Base" names, then the shorter
        // name, then the smaller offset (a 180°-symmetric straight ties None with Half: None)
        let shape_words: Vec<String> = shape_words_of(&k.name);
        ranked.sort_by(|a, b| {
            let (ma, mb) = ((a.mismatch * 200.0).round() as i64, (b.mismatch * 200.0).round() as i64);
            ma.cmp(&mb)
                .then_with(|| shared_words(&b.name, &shape_words).cmp(&shared_words(&a.name, &shape_words)))
                .then_with(|| plainness(&b.name).cmp(&plainness(&a.name)))
                .then_with(|| a.name.len().cmp(&b.name.len()))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.dir.cmp(&b.dir))
        });
        out.push(Mapping { collection: k.collection.clone(), block: k.name.clone(), size: k.size, ground: k.ground, waypoint_type: k.waypoint_type, covered: kf.covered(), ranked, candidates_tried: tried });
    }
    Ok(out)
}

pub fn verdict(m: &Mapping) -> &'static str {
    // wall pieces (no deck) are judged on the wall squares alone: a 1 m-thick plain wall against a 2 m standing
    // deck one metre over never overlaps perfectly — approx up to 0.6
    let wall = (m.covered as f32) < 0.2 * (32 * 32 * m.size.0 * m.size.1) as f32;
    match m.ranked.first() {
        Some(c) if c.mismatch <= MATCH_MAX => "match",
        Some(c) if c.mismatch <= APPROX_MAX || (wall && c.mismatch <= 0.65) => "approx",
        Some(_) => "none",
        None => "no-candidate",
    }
}

/// Hugo's dictionary entries from his CheckpointsCleaned.cs: `["Block"] = ("Plain", DirectionOffset.X),`.
pub fn parse_hugo(text: &str) -> BTreeMap<String, (String, u8)> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let l = line.trim();
        let Some(rest) = l.strip_prefix("[\"") else { continue };
        let Some((block, rest)) = rest.split_once("\"]") else { continue };
        let Some(i) = rest.find("(\"") else { continue };
        let rest = &rest[i + 2..];
        let Some((plain, rest)) = rest.split_once('"') else { continue };
        let dir = if rest.contains("ThreeQuarters") { 3 } else if rest.contains("Quarter") { 1 } else if rest.contains("Half") { 2 } else { 0 };
        out.insert(block.to_string(), (plain.to_string(), dir));
    }
    out
}

/// The `// Family` group a block belongs to in his file: the family prefix split into words
/// (RoadTech → "Road Tech", PlatformDirt → "Platform Dirt", OpenTechRoad → "Open Tech Road").
pub fn family_label(name: &str) -> String {
    let p = family_prefix(name);
    // drop the shape words so RoadTechDiagRight groups under Road Tech
    let mut base = p.to_string();
    for cut in ["Diag", "Slope", "Tilt", "Ring", "Trigger", "Center", "Narrow", "Wall", "Loop"] {
        if cut == "Wall" && base.ends_with("WithWall") {
            continue; // RoadIceWithWall is a family of its own
        }
        if let Some(i) = base.find(cut) {
            if i > 0 {
                base.truncate(i);
            }
        }
    }
    let mut s = String::new();
    for (i, ch) in base.chars().enumerate() {
        if i > 0 && ch.is_ascii_uppercase() {
            s.push(' ');
        }
        s.push(ch);
    }
    s
}

/// Gameplay / special variants are never a plain replacement.
pub fn is_gameplay(name: &str) -> bool {
    ["Special", "Penalty", "Boost", "Reset", "Turbo", "Fragile", "NoEngine", "SlowMotion", "NoSteering", "NoBrake", "Cruise", "Start", "Finish", "Multilap", "Dispenser", "Hole", "End", "Deadend"].iter().any(|w| name.contains(w))
}

/// The shape words of a checkpoint block name (the name without the family and without "Checkpoint", split at
/// capitals, drop the trigger-direction words).
pub fn shape_words_of(name: &str) -> Vec<String> {
    let rest = name.replacen("Checkpoint", "", 1);
    let prefix = family_label(name).replace(' ', "");
    let rest = rest.strip_prefix(prefix.as_str()).unwrap_or(&rest);
    let mut words = Vec::new();
    let mut cur = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_uppercase() && !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words.into_iter().filter(|w| !matches!(w.as_str(), "Left" | "Right" | "Up" | "Down" | "X2")).collect()
}

pub fn shared_words(candidate: &str, words: &[String]) -> usize {
    words.iter().filter(|w| candidate.contains(w.as_str())).count()
}

pub fn plainness(name: &str) -> u8 {
    if name.ends_with("Straight") || name.ends_with("Base") { 2 } else if name.contains("Straight") || name.contains("Base") { 1 } else { 0 }
}

/// Read a MAPPING.tsv written by `checkpoint-map` back into mappings (the merge of per-collection runs): the
/// ranked list holds the best and the second only, enough for the dictionary and the verdicts.
pub fn parse_mapping_tsv(text: &str) -> Vec<Mapping> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 14 {
            continue;
        }
        let size = f[2].split_once('x').map(|(a, b)| (a.parse().unwrap_or(1), b.parse().unwrap_or(1))).unwrap_or((1, 1));
        let dir_of = |s: &str| match s { "Quarter" => 1u8, "Half" => 2, "ThreeQuarters" => 3, _ => 0 };
        let mut ranked = Vec::new();
        if f[6] != "-" {
            ranked.push(Candidate { name: f[6].to_string(), dir: dir_of(f[7]), mismatch: f[8].parse().unwrap_or(1.0) });
        }
        if f[10] != "-" {
            ranked.push(Candidate { name: f[10].to_string(), dir: dir_of(f[11]), mismatch: f[12].parse().unwrap_or(1.0) });
        }
        out.push(Mapping { collection: f[0].to_string(), block: f[1].to_string(), size, ground: f[3] == "ground", waypoint_type: f[4].parse().ok(), covered: f[5].parse().unwrap_or(0), ranked, candidates_tried: f[13].parse().unwrap_or(0) });
    }
    out
}
