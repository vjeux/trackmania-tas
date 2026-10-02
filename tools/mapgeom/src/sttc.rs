//! "Straight to the Center" (Hugo's centered-finish campaign, 2026-10-02).
//!
//! Two steps, each a whole-map rewrite through `tmmaps::map::MapFile`:
//!
//!   * `sttf`  — Start-To-The-Finish: every CHECKPOINT goes. A checkpoint BLOCK
//!     is replaced by its plain twin (the same road/platform piece without the
//!     arch: `RoadTechCheckpoint` → `RoadTechStraight`, `PlatformTechCheckpoint`
//!     → `PlatformTechBase`, `RoadTechCheckpointSlopeUp` → `RoadTechSlopeStraight`
//!     turned the right way …), chosen among name candidates by GEOMETRY: the
//!     twin's collision surface must be contained in the checkpoint's (the
//!     checkpoint = the plain piece + an arch), tried at the four quarter turns;
//!     a checkpoint block with no such twin is removed (the `GateCheckpoint`
//!     gates). Checkpoint ITEMS (the gate rings) are removed. The validation
//!     ghost is dropped (it crossed checkpoints that no longer exist).
//!   * `center_finish` — every FINISH (blocks whose block info says Finish,
//!     items whose model says Finish; the baked clip records they own) moves by
//!     ONE common whole-cell offset (dx, dz) that puts the centroid of the
//!     finishes on the map's centre — `(size−1)/2` in cells: 23.5 on a 48 grid,
//!     31.5 on a 64 grid (world 16·size m) — y and rotations untouched, items
//!     by the same offset in metres so a multi-piece finish keeps its shape.
//!     A half-cell tie rounds UP (toward +x/+z): a single-cell finish lands on
//!     cell size/2. The map gets a new uid (prefix `SttC`) and the name
//!     `<name> Straight to the Center`.
//!
//! WHAT A WAYPOINT IS is decided by the MODEL, never by the record's tag
//! (`docs/formats/map-blocks.md` §5): blocks through the block info's
//! `waypoint_type` (0 Start, 1 Finish, 2 Checkpoint, 3 None, 4 StartFinish),
//! items through the item file's `0x2E00201F` chunk (embedded zip or pack).
//! The record's tag (`Spawn`/`Checkpoint`/`LinkedCheckpoint`/`Goal`) is read
//! too and a disagreement is reported, never silently resolved.

use crate::blockinfo::waypoint_name;
use crate::blockmap::{footprint, BlockInfoIndex, FLAG_GROUND, FLAG_PILLAR, FLAG_SUBVARIANT_SHIFT, FLAG_VARIANT_MASK};
use crate::store::DataStore;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use tmmaps::map::{BlockRec, GhostForm, ItemRec, MapFile, FREE_BLOCK_FLAG};

pub const WP_START: i32 = 0;
pub const WP_FINISH: i32 = 1;
pub const WP_CHECKPOINT: i32 = 2;
pub const WP_NONE: i32 = 3;
pub const WP_STARTFINISH: i32 = 4;

/// The uid prefix of every map this module writes (same length as the source
/// uid: the last four characters go, like the tiny converter's `Tin2`).
pub const UID_PREFIX: &str = "SttC";
pub const NAME_SUFFIX: &str = " Straight to the Center";

pub fn collection_name(c: u32) -> &'static str {
    match c {
        0x1c => "BlueBay",
        0x10 => "RedIsland",
        0x1d => "WhiteShore",
        0xf => "GreenCoast",
        _ => "Stadium",
    }
}

fn tag_type(tag: Option<&str>) -> Option<i32> {
    match tag {
        Some("Spawn") => Some(WP_START),
        Some("Goal") => Some(WP_FINISH),
        Some("Checkpoint") | Some("LinkedCheckpoint") => Some(WP_CHECKPOINT),
        Some("StartFinish") => Some(WP_STARTFINISH),
        _ => None,
    }
}

fn wp_str(t: Option<i32>) -> String {
    match t {
        Some(t) => waypoint_name(t).to_string(),
        None => "-".to_string(),
    }
}

/// What one record is, by the model and by the tag.
#[derive(Clone, Debug)]
pub struct Class {
    /// the model's waypoint type (block info / item file); None = no chunk
    pub model: Option<i32>,
    /// the model could be found (a block info / item file was read)
    pub resolved: bool,
    pub tag: Option<i32>,
}

impl Class {
    /// The type the GAME uses: the model's when the model was read (3 = None
    /// → not a waypoint), the tag's only when no model could be read.
    pub fn effective(&self) -> Option<i32> {
        if self.resolved {
            self.model.filter(|t| *t != WP_NONE)
        } else {
            self.tag
        }
    }
    pub fn mismatch(&self) -> bool {
        self.resolved && self.effective() != self.tag
    }
}

/// The pack side of one map: its collection's block info index and its
/// embedded items, with the lookups cached.
pub struct Ctx<'a> {
    pub store: &'a mut DataStore,
    pub idx: BlockInfoIndex,
    pub collection: String,
    /// embedded item files by lower-cased base name
    embedded: BTreeMap<String, Vec<u8>>,
    item_wp: HashMap<String, Option<i32>>,
    clip_names: HashMap<String, bool>,
}

impl<'a> Ctx<'a> {
    pub fn new(store: &'a mut DataStore, m: &MapFile) -> Ctx<'a> {
        let coll = m.body_collections().map(|c| c[0].1).unwrap_or(26);
        let collection = collection_name(coll).to_string();
        let idx = BlockInfoIndex::build(store, &collection);
        let embedded = crate::embedded::items(m).unwrap_or_default();
        Ctx { store, idx, collection, embedded, item_wp: HashMap::new(), clip_names: HashMap::new() }
    }

    /// The block info of a map block name (stems, then the ident ids).
    fn block_info(&mut self, name: &str) -> Option<&crate::blockinfo::BlockInfo> {
        let path = self.idx.resolve_one(self.store, name)?;
        self.idx.load(self.store, &path).ok()
    }

    pub fn block_class(&mut self, b: &BlockRec) -> Class {
        let tag = tag_type(b.waypoint_tag.as_deref());
        match self.block_info(&b.name) {
            Some(bi) => Class { model: bi.waypoint_type, resolved: true, tag },
            None => Class { model: None, resolved: false, tag },
        }
    }

    /// Whether a (baked) record's name is a CLIP block info — the generated
    /// fillers; terrain tiles (`Sea`, `Grass`, `Land…`) are not.
    pub fn is_clip(&mut self, name: &str) -> bool {
        if let Some(v) = self.clip_names.get(name) {
            return *v;
        }
        let v = match self.idx.resolve_one(self.store, name) {
            Some(p) => p.to_uppercase().contains("GAMECTNBLOCKINFOCLIP"),
            None => name.contains("FC") || name.contains("Clip"),
        };
        self.clip_names.insert(name.to_string(), v);
        v
    }

    /// The picked variant's footprint in the block's own frame (dir 0):
    /// (W, D) in cells, and the unit cells for `dir`.
    pub fn block_footprint(&mut self, b: &BlockRec, dir: u8) -> Option<((i32, i32), Vec<[i32; 3]>)> {
        let ground = b.flags & FLAG_GROUND != 0;
        let vindex = (b.flags & FLAG_VARIANT_MASK) as usize;
        let sub = ((b.flags >> FLAG_SUBVARIANT_SHIFT) & 63) as usize;
        let bi = self.block_info(&b.name)?;
        let pk = bi.pick_placement(ground, vindex, sub)?;
        let local = footprint(pk.variant, 0);
        let w = local.iter().map(|(_, c)| c[0]).max().unwrap_or(0) + 1;
        let d = local.iter().map(|(_, c)| c[2]).max().unwrap_or(0) + 1;
        let cells = footprint(pk.variant, dir).into_iter().map(|(_, c)| c).collect();
        Some(((w, d), cells))
    }

    /// World grid cells of a GRID block (its record cell + the variant's
    /// footprint for its dir); the record cell alone when the info is unknown.
    pub fn world_cells(&mut self, b: &BlockRec) -> Vec<[i32; 3]> {
        let (x, y, z) = b.coords();
        match self.block_footprint(b, b.dir) {
            Some((_, cells)) if !cells.is_empty() => cells.into_iter().map(|c| [x + c[0], y + c[1], z + c[2]]).collect(),
            _ => vec![[x, y, z]],
        }
    }

    /// An item model's waypoint type: the embedded file, else the pack's.
    pub fn item_wp(&mut self, model: &str) -> (Option<i32>, bool) {
        if let Some(v) = self.item_wp.get(model) {
            return (*v, true);
        }
        let base = model.rsplit(['/', '\\']).next().unwrap_or(model).to_lowercase();
        let base = if base.ends_with(".item.gbx") { base } else { format!("{base}.item.gbx") };
        let bytes: Option<Vec<u8>> = match self.embedded.get(&base) {
            Some(b) => Some(b.clone()),
            None => {
                let stem = model.trim_end_matches(".Item.Gbx");
                let mut cands = vec![format!("Stadium\\Items\\{stem}.Item.Gbx"), format!("Stadium\\Items\\Theme\\{stem}.Item.Gbx")];
                for env in ["GreenCoast", "RedIsland", "BlueBay", "WhiteShore"] {
                    cands.push(format!("{env}\\Items\\Stadium\\{stem}.Item.Gbx"));
                    cands.push(format!("{env}\\Items\\{stem}.Item.Gbx"));
                }
                cands.into_iter().find_map(|p| self.store.read(&p).ok().map(|b| b.as_ref().clone()))
            }
        };
        let Some(bytes) = bytes else {
            return (None, false);
        };
        let wp = match crate::static_item::parse_file(&bytes) {
            Ok(f) => f.item.chunks.iter().find_map(|c| match c {
                crate::static_item::item::ItemChunk::Waypoint { waypoint_type, .. } => Some(*waypoint_type),
                _ => None,
            }),
            Err(_) => None,
        };
        self.item_wp.insert(model.to_string(), wp);
        (wp, true)
    }

    pub fn item_class(&mut self, it: &ItemRec) -> Class {
        let tag = tag_type(it.waypoint_tag.as_deref());
        let (model, resolved) = self.item_wp(&it.model);
        Class { model, resolved, tag }
    }
}

// ------------------------------------------------------------------ report

/// One line of the report: a record touched (or deliberately not), per step.
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub map: String,
    pub step: String,
    pub kind: String,
    pub index: String,
    pub name: String,
    pub tag: String,
    pub model_wp: String,
    pub action: String,
    pub to_name: String,
    pub dir_from: String,
    pub dir_to: String,
    pub from: String,
    pub to: String,
    pub y: String,
    pub overlap: String,
    pub note: String,
}

pub const REPORT_HEADER: &str = "map\tstep\tkind\tindex\tname\ttag\tmodel_wp\taction\tto_name\tdir_from\tdir_to\tfrom\tto\ty\toverlap\tnote";

impl Row {
    pub fn tsv(&self) -> String {
        let f = |s: &str| if s.is_empty() { "-".to_string() } else { s.replace('\t', " ") };
        [&self.map, &self.step, &self.kind, &self.index, &self.name, &self.tag, &self.model_wp, &self.action, &self.to_name, &self.dir_from, &self.dir_to, &self.from, &self.to, &self.y, &self.overlap, &self.note]
            .iter()
            .map(|s| f(s))
            .collect::<Vec<_>>()
            .join("\t")
    }
    fn new(map: &str, step: &str, kind: &str, index: usize, name: &str) -> Row {
        Row { map: map.to_string(), step: step.to_string(), kind: kind.to_string(), index: index.to_string(), name: name.to_string(), ..Default::default() }
    }
}

fn cell_str(c: (i32, i32, i32)) -> String {
    format!("{},{},{}", c.0, c.1, c.2)
}
fn pos_str(p: [f32; 3]) -> String {
    format!("{:.3},{:.3},{:.3}", p[0], p[1], p[2])
}

// -------------------------------------------------------------------- sttf

/// Name candidates for the plain twin of a checkpoint block, most specific
/// first. Existence and geometry are checked by the caller.
pub fn twin_candidates(name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if s != name && !out.contains(&s) {
            out.push(s);
        }
    };
    // the engine's own spellings: `CheckPoint` on the diagonal tech roads
    let norm = name.replace("CheckPoint", "Checkpoint");
    for (from, to) in [
        ("CheckpointTiltLeft", "TiltStraight"),
        ("CheckpointTiltRight", "TiltStraight"),
        ("CheckpointSlopeUp", "SlopeStraight"),
        ("CheckpointSlopeDown", "SlopeStraight"),
        ("CheckpointSlope2Up", "Slope2Base"),
        ("CheckpointSlope2Down", "Slope2Base"),
        ("CheckpointSlope2Left", "Slope2Base"),
        ("CheckpointSlope2Right", "Slope2Base"),
        ("CheckpointSlope2Up", "Slope2Straight"),
        ("CheckpointSlope2Down", "Slope2Straight"),
        ("CheckpointSlope2Left", "Slope2Straight"),
        ("CheckpointSlope2Right", "Slope2Straight"),
        ("CheckpointLeft", ""),
        ("CheckpointRight", ""),
        ("Checkpoint", "Straight"),
        ("Checkpoint", "StraightX2"),
        ("Checkpoint", "Base"),
        ("Checkpoint", ""),
    ] {
        if norm.contains(from) {
            push(norm.replacen(from, to, 1));
        }
    }
    out
}

/// The surface heights of a block model over its footprint, sampled on a
/// 2-m grid in the block's own frame: (x, z) → the sorted heights (and
/// physics materials) of every surface in that column.
struct Profile {
    w: f32,
    d: f32,
    cols: Vec<((f32, f32), Vec<(f32, String)>)>,
}

fn profile(asm: &mut crate::assemble::Assembler, name: &str) -> Option<Profile> {
    let lm = asm.block_model(name)?;
    let (w, d) = lm.size;
    let index = crate::probe::Index::build(&lm.scene, 8.0);
    let step = 2.0f32;
    let mut cols = Vec::new();
    let (nx, nz) = ((w / step).round() as i32, (d / step).round() as i32);
    for i in 0..nx {
        for j in 0..nz {
            let (x, z) = ((i as f32 + 0.5) * step, (j as f32 + 0.5) * step);
            let mut h = index.column(x, z);
            h.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            cols.push(((x, z), h));
        }
    }
    Some(Profile { w, d, cols })
}

/// How well `twin` placed with `k` extra quarter turns reproduces `cp`:
/// (containment fraction, coverage fraction). Containment: of the columns
/// where the twin has a surface, the share whose every twin surface has a
/// checkpoint surface within 0.15 m of the same material class. Coverage: of
/// the columns where the checkpoint has a surface, the share where the twin
/// has one too.
fn match_score(cp: &Profile, twin: &Profile, k: u8) -> Option<(f32, f32)> {
    // the twin's footprint must be the checkpoint's, turned
    let (tw, td) = if k % 2 == 1 { (twin.d, twin.w) } else { (twin.w, twin.d) };
    if (tw - cp.w).abs() > 0.5 || (td - cp.d).abs() > 0.5 {
        return None;
    }
    // the twin column that lands on the checkpoint column (x, z) after the turn
    let lookup: HashMap<(i32, i32), &Vec<(f32, String)>> = twin.cols.iter().map(|((x, z), h)| (((x * 2.0) as i32, (z * 2.0) as i32), h)).collect();
    let (mut contained, mut twin_cols, mut covered, mut cp_cols) = (0usize, 0usize, 0usize, 0usize);
    for ((x, z), hcp) in &cp.cols {
        // world-local (lx, lz) = (x, z) of the checkpoint frame (dir 0); the
        // twin at dir k maps its local (xt, zt) to (lx, lz) by map-blocks.md §2
        let (xt, zt) = match k % 4 {
            0 => (*x, *z),
            1 => (*z, twin.d - *x),
            2 => (twin.w - *x, twin.d - *z),
            _ => (twin.w - *z, *x),
        };
        let key = ((xt * 2.0).round() as i32, (zt * 2.0).round() as i32);
        let ht: &[(f32, String)] = lookup.get(&key).map(|v| v.as_slice()).unwrap_or(&[]);
        if !hcp.is_empty() {
            cp_cols += 1;
            if !ht.is_empty() {
                covered += 1;
            }
        }
        if !ht.is_empty() {
            twin_cols += 1;
            let ok = ht.iter().all(|(h, m)| hcp.iter().any(|(h2, m2)| (h - h2).abs() <= 0.15 && m == m2));
            if ok {
                contained += 1;
            }
        }
    }
    if twin_cols == 0 || cp_cols == 0 {
        return None;
    }
    Some((contained as f32 / twin_cols as f32, covered as f32 / cp_cols as f32))
}

/// `pos` of a free record moved by a world offset.
fn shifted(p: [f32; 3], d: [f32; 3]) -> [f32; 3] {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

/// The baked GRID records a set of authored grid cells owns: a vertical clip
/// stands in the cell across its side from the owner; an FCT (the owner's top
/// plate) is recorded in the cell above the owner, an FCB (the owner's
/// underside) in the cell below (`docs/formats/map-blocks.md` §6).
pub fn owned_baked(ctx: &mut Ctx, m: &MapFile, cells: &HashSet<[i32; 3]>) -> Vec<usize> {
    let mut out = Vec::new();
    for r in m.baked.iter().filter(|r| r.flags & FREE_BLOCK_FLAG == 0 && r.flags != 0xFFFF_FFFF) {
        if !ctx.is_clip(&r.name) {
            continue;
        }
        let (x, y, z) = r.coords();
        let owner = if r.name.contains("FCT") {
            [x, y - 1, z]
        } else if r.name.contains("FCB") {
            [x, y + 1, z]
        } else {
            let (dx, dz) = crate::blockmap::SIDE_VEC[(r.dir & 3) as usize];
            [x + dx, y, z + dz]
        };
        if cells.contains(&owner) {
            out.push(r.index);
        }
    }
    out
}

pub struct SttfOutcome {
    pub rows: Vec<Row>,
    pub replaced: usize,
    pub removed_blocks: usize,
    pub removed_items: usize,
    pub removed_baked: usize,
    pub mismatches: usize,
    pub unresolved: usize,
}

/// Start-To-The-Finish: write `out` from `src` with every checkpoint gone.
/// What a checkpoint BLOCK becomes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CpMode {
    /// swapped for its plain twin (geometry-verified), removed when it has none
    Plain,
    /// deleted, the cell left empty (the twin is still looked up and reported)
    Remove,
}

pub fn sttf(ctx: &mut Ctx, src: &Path, out: &Path, cp: CpMode, dry: bool) -> Result<SttfOutcome, String> {
    let map_label = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut m = MapFile::try_load(src)?;
    let mut rows = Vec::new();
    let (mut replaced, mut removed_blocks, mut removed_items, mut mismatches, mut unresolved) = (0, 0, 0, 0, 0);
    // (block index, twin, dir)
    let mut renames: Vec<(usize, String, u8)> = Vec::new();
    let mut drop_blocks: HashSet<usize> = HashSet::new();
    let mut drop_cells: HashSet<[i32; 3]> = HashSet::new();
    let mut drop_items: HashSet<usize> = HashSet::new();
    let cps: Vec<(usize, BlockRec)> = m.blocks.iter().enumerate().filter(|(_, b)| b.flags != 0xFFFF_FFFF).map(|(i, b)| (i, b.clone())).collect();
    let mut classified: Vec<(usize, BlockRec, Class)> = Vec::new();
    for (i, b) in cps {
        let c = ctx.block_class(&b);
        if !c.resolved && c.tag.is_some() {
            unresolved += 1;
        }
        if c.mismatch() {
            mismatches += 1;
            let mut r = Row::new(&map_label, "sttf", "block", i, &b.name);
            r.tag = b.waypoint_tag.clone().unwrap_or_default();
            r.model_wp = wp_str(c.model);
            r.action = "MISMATCH".into();
            r.from = cell_str(b.coords());
            r.note = "the record's tag and the block info disagree; the block info decides".into();
            rows.push(r);
        }
        classified.push((i, b, c));
    }
    let mut twin_cache: HashMap<(String, u8, u32), Result<(String, u8, String), String>> = HashMap::new();
    for (i, b, c) in &classified {
        if c.effective() != Some(WP_CHECKPOINT) {
            continue;
        }
        let mut r = Row::new(&map_label, "sttf", "block", *i, &b.name);
        r.tag = b.waypoint_tag.clone().unwrap_or_default();
        r.model_wp = wp_str(c.model);
        r.dir_from = b.dir.to_string();
        r.from = match b.free_pos {
            Some(p) => format!("free {}", pos_str(p)),
            None => cell_str(b.coords()),
        };
        let key = (b.name.clone(), b.dir, b.flags & 0x0FFF_FFFF);
        let twin = match twin_cache.get(&key) {
            Some(t) => t.clone(),
            None => {
                // the assembler and the index both read the packs: disjoint
                // field borrows of the context
                let t = {
                    let mut asm = crate::assemble::Assembler::new(&mut *ctx.store);
                    find_twin_split(&mut ctx.idx, &mut asm, b)
                };
                twin_cache.insert(key, t.clone());
                t
            }
        };
        match twin {
            Ok((name, dir, note)) if cp == CpMode::Remove => {
                r.action = "removed".into();
                r.to_name = name;
                r.dir_to = dir.to_string();
                r.note = format!("--cp remove (the plain twin would be {}): {note}", r.to_name);
                drop_blocks.insert(*i);
                if b.free_pos.is_none() {
                    for c in ctx.world_cells(b) {
                        drop_cells.insert(c);
                    }
                }
                removed_blocks += 1;
            }
            Ok((name, dir, note)) => {
                r.action = "replaced".into();
                r.to_name = name.clone();
                r.dir_to = dir.to_string();
                r.note = note;
                renames.push((*i, name, dir));
                replaced += 1;
            }
            Err(why) => {
                r.action = "removed".into();
                r.note = why;
                drop_blocks.insert(*i);
                if b.free_pos.is_none() {
                    for c in ctx.world_cells(b) {
                        drop_cells.insert(c);
                    }
                }
                removed_blocks += 1;
            }
        }
        rows.push(r);
    }
    for (i, it) in m.items.iter().enumerate() {
        let c = ctx.item_class(it);
        if c.mismatch() {
            mismatches += 1;
            let mut r = Row::new(&map_label, "sttf", "item", i, &it.model);
            r.tag = it.waypoint_tag.clone().unwrap_or_default();
            r.model_wp = wp_str(c.model);
            r.action = "MISMATCH".into();
            r.from = pos_str(it.pos);
            r.note = "the placement's tag and the item model disagree; the model decides".into();
            rows.push(r);
        }
        if !c.resolved && c.tag.is_some() {
            unresolved += 1;
        }
        if c.effective() == Some(WP_CHECKPOINT) {
            let mut r = Row::new(&map_label, "sttf", "item", i, &it.model);
            r.tag = it.waypoint_tag.clone().unwrap_or_default();
            r.model_wp = wp_str(c.model);
            r.action = "removed".into();
            r.from = pos_str(it.pos);
            r.note = if c.resolved { String::new() } else { "model not read: the tag decided".into() };
            rows.push(r);
            drop_items.insert(i);
            removed_items += 1;
        }
    }
    let owned = if drop_cells.is_empty() { Vec::new() } else { owned_baked(ctx, &m, &drop_cells) };
    let removed_baked = owned.len();
    if dry {
        return Ok(SttfOutcome { rows, replaced, removed_blocks, removed_items, removed_baked, mismatches, unresolved });
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // pass A: the twins (a rename re-encodes the Id table; dirs are in-place bytes)
    if !renames.is_empty() {
        for (i, name, dir) in &renames {
            m.set_block_name(*i, name);
            if *dir != m.blocks[*i].dir {
                m.set_block_dir(*i, *dir);
            }
        }
        m.write_to(out).map_err(|e| e.to_string())?;
        m = MapFile::try_load(out)?;
    } else {
        m.write_to(out).map_err(|e| e.to_string())?;
        m = MapFile::try_load(out)?;
    }
    // pass A2: the replaced blocks' waypoint nodes go. The game decides by the
    // model, but every tool that reads tags (`tmmaps waypoints`, the start
    // check, the ladder) would still see a checkpoint. The node is the record's
    // tail — `u32 node index` then, on its first use, the inline class + chunks
    // — so the tail is cut and flag bit 20 cleared; a record with a skin
    // (bit 15: author + skin node sit in between) or whose node index another
    // record shares keeps the residue and says so.
    let mut stripped: HashSet<usize> = HashSet::new();
    {
        let spans = m.block_spans();
        let body = m.gbx.body.clone();
        let rd = |o: usize| u32::from_le_bytes(body[o..o + 4].try_into().unwrap());
        let mut idx_count: HashMap<u32, usize> = HashMap::new();
        for b in m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF && b.flags & 0x100000 != 0 && b.flags & 0x8000 == 0) {
            *idx_count.entry(rd(b.coord_off + 7)).or_default() += 1;
        }
        for (i, _, _) in &renames {
            let b = m.blocks[*i].clone();
            if b.flags & 0x100000 == 0 || b.flags & 0x8000 != 0 {
                continue;
            }
            let (start, end) = (b.coord_off + 7, spans[*i].1);
            let idx = rd(start);
            if idx != 0xFFFF_FFFF && idx_count.get(&idx).copied().unwrap_or(0) > 1 {
                continue;
            }
            if idx != 0xFFFF_FFFF && rd(start + 4) != 0x2E00_9000 && end != start + 4 {
                continue; // not the layout this expects: leave it
            }
            m.raw_splices.push(((start, end), Vec::new()));
            m.raw_patches.push((b.coord_off + 3, (b.flags & !0x100000).to_le_bytes().to_vec()));
            stripped.insert(*i);
        }
        if !stripped.is_empty() {
            m.write_to(out).map_err(|e| e.to_string())?;
            m = MapFile::try_load(out)?;
        }
    }
    for r in rows.iter_mut().filter(|r| r.action == "replaced") {
        let i: usize = r.index.parse().unwrap_or(usize::MAX);
        if stripped.contains(&i) {
            r.note = format!("waypoint node stripped; {}", r.note);
        } else {
            r.note = format!("WAYPOINT TAG LEFT (skinned or shared node; the game ignores it); {}", r.note);
        }
    }
    // pass B: the checkpoint blocks with no twin, and the clips they owned
    if !drop_blocks.is_empty() {
        let owned_set: HashSet<usize> = owned.iter().copied().collect();
        m.remove_blocks(|b| drop_blocks.contains(&b.index), |r| owned_set.contains(&r.index));
        m.write_to(out).map_err(|e| e.to_string())?;
        m = MapFile::try_load(out)?;
    }
    // pass C: the checkpoint items
    if !drop_items.is_empty() {
        m.remove_items(|it| drop_items.contains(&it.index));
        m.write_to(out).map_err(|e| e.to_string())?;
        m = MapFile::try_load(out)?;
    }
    // pass D: the validation ghost (it drove through checkpoints that no longer exist)
    m.strip_validation_ghost_to(GhostForm::Remove);
    m.write_to(out).map_err(|e| e.to_string())?;
    Ok(SttfOutcome { rows, replaced, removed_blocks, removed_items, removed_baked, mismatches, unresolved })
}

/// `find_twin` over the index and an assembler that share no borrow: the
/// assembler owns a `&mut DataStore`, the index only needs one for a cache
/// miss — the misses are served through the assembler's store.
fn find_twin_split(idx: &mut BlockInfoIndex, asm: &mut crate::assemble::Assembler, b: &BlockRec) -> Result<(String, u8, String), String> {
    let cands = twin_candidates(&b.name);
    let Some(cp) = profile(asm, &b.name) else {
        return Err(format!("no geometry for {}", b.name));
    };
    let mut tried: Vec<String> = Vec::new();
    let mut best: Option<(f32, f32, String, u8)> = None;
    for cand in &cands {
        let Some(path) = idx.resolve_one(&mut *asm.store, cand) else {
            continue;
        };
        let ident = match idx.load(&mut *asm.store, &path) {
            Ok(bi) if !bi.ident.is_empty() => bi.ident.clone(),
            Ok(bi) => bi.name.clone(),
            Err(_) => cand.clone(),
        };
        let Some(tp) = profile(asm, cand) else {
            tried.push(format!("{cand}:no-geometry"));
            continue;
        };
        for k in 0..4u8 {
            if let Some((c, cov)) = match_score(&cp, &tp, k) {
                tried.push(format!("{cand}+{k}:{c:.2}/{cov:.2}"));
                let better = match &best {
                    None => true,
                    Some((bc, bcov, _, _)) => c + cov * 0.5 > bc + bcov * 0.5,
                };
                if better {
                    best = Some((c, cov, ident.clone(), (b.dir + k) % 4));
                }
            }
        }
        if let Some((c, cov, _, _)) = &best {
            if *c >= 0.95 && *cov >= 0.8 {
                break;
            }
        }
    }
    match best {
        Some((c, cov, ident, dir)) if c >= 0.95 && cov >= 0.8 => Ok((ident, dir, format!("contained {c:.2} covered {cov:.2}; tried {}", tried.join(" ")))),
        Some((c, cov, ident, _)) => Err(format!("best {ident} contained {c:.2} covered {cov:.2} < 0.95/0.80; tried {}", tried.join(" "))),
        None => Err(format!("no candidate exists among {:?}", cands)),
    }
}

// ----------------------------------------------------------- center-finish

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Center {
    /// the size words: ((size_x − 1)/2, (size_z − 1)/2) in cells
    Auto,
    /// the centre of the bounding box of every authored block and item
    Bbox,
    /// explicit, in Hugo's cell units (23.5 = the centre of a 48 grid)
    At(f64, f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Occupied {
    Overlap,
    Raise,
    Fail,
}

pub struct CenterOpts {
    pub center: Center,
    pub occupied: Occupied,
    /// StartFinish (multilap) blocks count as finishes
    pub include_startfinish: bool,
    pub rename: bool,
    pub reuid: bool,
}

pub struct CenterOutcome {
    pub rows: Vec<Row>,
    pub finish_blocks: usize,
    pub finish_items: usize,
    pub moved_baked: usize,
    pub startfinish: usize,
    pub centroid: (f64, f64),
    pub center: (f64, f64),
    pub offset_cells: (i32, i32, i32),
    pub overlaps: usize,
    pub mismatches: usize,
    pub new_name: String,
    pub new_uid: String,
}

fn round_half_up(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}

/// Move the finish(es) of `src` to the map centre, into `out`.
pub fn center_finish(ctx: &mut Ctx, src: &Path, out: &Path, o: &CenterOpts, dry: bool) -> Result<CenterOutcome, String> {
    let map_label = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut m = MapFile::try_load(src)?;
    let hdr = tmmaps::header::read(src.to_str().unwrap_or_default())?;
    let mut rows = Vec::new();
    let (mut mismatches, mut startfinish) = (0usize, 0usize);
    // --- the finishes
    struct FinBlock {
        index: usize,
        rec: BlockRec,
        cells: Vec<[i32; 3]>,
        centre: (f64, f64),
    }
    let mut fin_blocks: Vec<FinBlock> = Vec::new();
    let mut fin_items: Vec<(usize, ItemRec)> = Vec::new();
    let blocks: Vec<(usize, BlockRec)> = m.blocks.iter().enumerate().filter(|(_, b)| b.flags != 0xFFFF_FFFF).map(|(i, b)| (i, b.clone())).collect();
    let mut occupancy: HashMap<[i32; 3], Vec<String>> = HashMap::new();
    let mut bbox: Option<([f64; 2], [f64; 2])> = None;
    let mut grow = |x: f64, z: f64, bbox: &mut Option<([f64; 2], [f64; 2])>| {
        *bbox = Some(match *bbox {
            None => ([x, z], [x, z]),
            Some((lo, hi)) => ([lo[0].min(x), lo[1].min(z)], [hi[0].max(x), hi[1].max(z)]),
        });
    };
    for (i, b) in &blocks {
        let c = ctx.block_class(b);
        if c.mismatch() {
            mismatches += 1;
            let mut r = Row::new(&map_label, "center", "block", *i, &b.name);
            r.tag = b.waypoint_tag.clone().unwrap_or_default();
            r.model_wp = wp_str(c.model);
            r.action = "MISMATCH".into();
            r.from = cell_str(b.coords());
            r.note = "the record's tag and the block info disagree; the block info decides".into();
            rows.push(r);
        }
        let eff = c.effective();
        let is_fin = eff == Some(WP_FINISH) || (eff == Some(WP_STARTFINISH) && o.include_startfinish);
        if eff == Some(WP_STARTFINISH) && !o.include_startfinish {
            startfinish += 1;
            let mut r = Row::new(&map_label, "center", "block", *i, &b.name);
            r.tag = b.waypoint_tag.clone().unwrap_or_default();
            r.model_wp = wp_str(c.model);
            r.action = "STARTFINISH-NOT-MOVED".into();
            r.from = cell_str(b.coords());
            r.note = "a multilap start/finish; pass --include-startfinish to move it".into();
            rows.push(r);
        }
        let free = b.flags & FREE_BLOCK_FLAG != 0;
        if free {
            let p = b.free_pos.unwrap_or([0.0; 3]);
            grow(p[0] as f64, p[2] as f64, &mut bbox);
            if is_fin {
                // the block's centre: its origin corner + half the footprint,
                // turned by the yaw (the first angle of the free triple)
                let (w, d) = ctx.block_footprint(b, 0).map(|(wd, _)| wd).unwrap_or((1, 1));
                let yaw = b.free_rot.map(|r| r[0]).unwrap_or(0.0) as f64;
                let (hx, hz) = (w as f64 * 16.0, d as f64 * 16.0);
                let (cx, cz) = (p[0] as f64 + hx * yaw.cos() + hz * yaw.sin(), p[2] as f64 - hx * yaw.sin() + hz * yaw.cos());
                fin_blocks.push(FinBlock { index: *i, rec: b.clone(), cells: Vec::new(), centre: (cx, cz) });
            }
            continue;
        }
        let cells = ctx.world_cells(b);
        for c in &cells {
            grow(c[0] as f64 * 32.0, c[2] as f64 * 32.0, &mut bbox);
            grow(c[0] as f64 * 32.0 + 32.0, c[2] as f64 * 32.0 + 32.0, &mut bbox);
        }
        if is_fin {
            let n = cells.len().max(1) as f64;
            let cx = cells.iter().map(|c| c[0] as f64 * 32.0 + 16.0).sum::<f64>() / n;
            let cz = cells.iter().map(|c| c[2] as f64 * 32.0 + 16.0).sum::<f64>() / n;
            fin_blocks.push(FinBlock { index: *i, rec: b.clone(), cells, centre: (cx, cz) });
        } else if b.flags & FLAG_PILLAR == 0 {
            for c in cells {
                occupancy.entry(c).or_default().push(b.name.clone());
            }
        }
    }
    for (i, it) in m.items.iter().enumerate() {
        grow(it.pos[0] as f64, it.pos[2] as f64, &mut bbox);
        let c = ctx.item_class(it);
        if c.mismatch() {
            mismatches += 1;
            let mut r = Row::new(&map_label, "center", "item", i, &it.model);
            r.tag = it.waypoint_tag.clone().unwrap_or_default();
            r.model_wp = wp_str(c.model);
            r.action = "MISMATCH".into();
            r.from = pos_str(it.pos);
            r.note = "the placement's tag and the item model disagree; the model decides".into();
            rows.push(r);
        }
        let eff = c.effective();
        if eff == Some(WP_FINISH) || (eff == Some(WP_STARTFINISH) && o.include_startfinish) {
            fin_items.push((i, it.clone()));
        }
    }
    if fin_blocks.is_empty() && fin_items.is_empty() {
        return Err(format!("{map_label}: no finish found (no block info / item model of waypoint type Finish)"));
    }
    // --- centroid and centre (world metres)
    let n = (fin_blocks.len() + fin_items.len()) as f64;
    let sx: f64 = fin_blocks.iter().map(|f| f.centre.0).sum::<f64>() + fin_items.iter().map(|(_, it)| it.pos[0] as f64).sum::<f64>();
    let sz: f64 = fin_blocks.iter().map(|f| f.centre.1).sum::<f64>() + fin_items.iter().map(|(_, it)| it.pos[2] as f64).sum::<f64>();
    let centroid = (sx / n, sz / n);
    let center = match o.center {
        Center::Auto => (m.size[0] as f64 * 16.0, m.size[2] as f64 * 16.0),
        Center::Bbox => {
            let (lo, hi) = bbox.ok_or("empty map")?;
            ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0)
        }
        Center::At(x, z) => ((x + 0.5) * 32.0, (z + 0.5) * 32.0),
    };
    let dx = round_half_up((center.0 - centroid.0) / 32.0);
    let dz = round_half_up((center.1 - centroid.1) / 32.0);
    // --- destination check
    let (sx_cells, sy_cells, sz_cells) = (m.size[0], m.size[1], m.size[2]);
    let mut dy = 0i32;
    let overlap_at = |dy: i32, fin_blocks: &[FinBlock], occupancy: &HashMap<[i32; 3], Vec<String>>| -> Vec<([i32; 3], String)> {
        let mut v = Vec::new();
        for f in fin_blocks {
            for c in &f.cells {
                let d = [c[0] + dx, c[1] + dy, c[2] + dz];
                if let Some(names) = occupancy.get(&d) {
                    v.push((d, names.join("+")));
                }
            }
        }
        v
    };
    let mut overlaps = overlap_at(0, &fin_blocks, &occupancy);
    if !overlaps.is_empty() {
        match o.occupied {
            Occupied::Overlap => {}
            Occupied::Fail => {
                return Err(format!(
                    "{map_label}: the destination is occupied ({} cells: {}) and --occupied fail was asked",
                    overlaps.len(),
                    overlaps.iter().take(5).map(|(c, n)| format!("{:?} {n}", c)).collect::<Vec<_>>().join(", ")
                ));
            }
            Occupied::Raise => {
                let top = fin_blocks.iter().flat_map(|f| f.cells.iter().map(|c| c[1])).max().unwrap_or(0);
                let mut found = None;
                for cand in 1..(sy_cells - top) {
                    if overlap_at(cand, &fin_blocks, &occupancy).is_empty() {
                        found = Some(cand);
                        break;
                    }
                }
                dy = found.ok_or_else(|| format!("{map_label}: no free height above the destination"))?;
                overlaps = Vec::new();
            }
        }
    }
    for f in &fin_blocks {
        for c in &f.cells {
            let d = [c[0] + dx, c[1] + dy, c[2] + dz];
            if d[0] < 0 || d[2] < 0 || d[0] >= sx_cells || d[2] >= sz_cells || d[1] < 0 || d[1] >= sy_cells {
                return Err(format!("{map_label}: the moved finish leaves the grid at {:?} (size {:?})", d, m.size));
            }
        }
    }
    let overlap_cells: HashSet<[i32; 3]> = overlaps.iter().map(|(c, _)| *c).collect();
    let dw = [dx as f32 * 32.0, dy as f32 * 8.0, dz as f32 * 32.0];
    // --- the baked records that move along
    let fin_cells: HashSet<[i32; 3]> = fin_blocks.iter().flat_map(|f| f.cells.iter().copied()).collect();
    let owned = if fin_cells.is_empty() { Vec::new() } else { owned_baked(ctx, &m, &fin_cells) };
    // baked FREE pieces named after a free finish block, within 48 m of its anchor
    let mut owned_free: Vec<usize> = Vec::new();
    for r in m.baked.iter().filter(|r| r.flags & FREE_BLOCK_FLAG != 0) {
        let Some(rp) = r.free_pos else { continue };
        let near = fin_blocks.iter().any(|f| {
            f.rec.flags & FREE_BLOCK_FLAG != 0
                && r.name.starts_with(&f.rec.name)
                && f.rec.free_pos.map(|p| ((p[0] - rp[0]).powi(2) + (p[1] - rp[1]).powi(2) + (p[2] - rp[2]).powi(2)).sqrt() <= 48.0).unwrap_or(false)
        });
        if near {
            owned_free.push(r.index);
        }
    }
    // --- rows
    for f in &fin_blocks {
        let mut r = Row::new(&map_label, "center", "block", f.index, &f.rec.name);
        r.tag = f.rec.waypoint_tag.clone().unwrap_or_default();
        r.model_wp = "Finish".into();
        r.action = "moved".into();
        r.dir_from = f.rec.dir.to_string();
        r.dir_to = f.rec.dir.to_string();
        match f.rec.free_pos {
            Some(p) => {
                r.from = format!("free {}", pos_str(p));
                r.to = format!("free {}", pos_str(shifted(p, dw)));
                r.y = format!("{:.3}", p[1] + dw[1]);
            }
            None => {
                let c = f.rec.coords();
                r.from = cell_str(c);
                r.to = cell_str((c.0 + dx, c.1 + dy, c.2 + dz));
                r.y = (c.1 + dy).to_string();
                r.overlap = if f.cells.iter().any(|c| overlap_cells.contains(&[c[0] + dx, c[1] + dy, c[2] + dz])) {
                    let names: Vec<String> = overlaps.iter().filter(|(oc, _)| f.cells.iter().any(|c| [c[0] + dx, c[1] + dy, c[2] + dz] == *oc)).map(|(_, n)| n.clone()).collect();
                    format!("Y:{}", names.join("|"))
                } else {
                    "N".into()
                };
            }
        }
        rows.push(r);
    }
    for (i, it) in &fin_items {
        let mut r = Row::new(&map_label, "center", "item", *i, &it.model);
        r.tag = it.waypoint_tag.clone().unwrap_or_default();
        r.model_wp = "Finish".into();
        r.action = "moved".into();
        r.from = pos_str(it.pos);
        r.to = pos_str(shifted(it.pos, dw));
        r.y = format!("{:.3}", it.pos[1] + dw[1]);
        let c = it.coords();
        let dest = [c.0 + dx, c.1 + dy, c.2 + dz];
        r.overlap = if occupancy.contains_key(&dest) { format!("Y:{}", occupancy[&dest].join("|")) } else { "N".into() };
        rows.push(r);
    }
    for idx in owned.iter().chain(owned_free.iter()) {
        let rec = m.baked.iter().find(|r| r.index == *idx).cloned();
        if let Some(rec) = rec {
            let mut r = Row::new(&map_label, "center", "baked", rec.index, &rec.name);
            r.action = "moved-with-finish".into();
            r.dir_from = rec.dir.to_string();
            r.dir_to = rec.dir.to_string();
            match rec.free_pos {
                Some(p) => {
                    r.from = format!("free {}", pos_str(p));
                    r.to = format!("free {}", pos_str(shifted(p, dw)));
                }
                None => {
                    let c = rec.coords();
                    r.from = cell_str(c);
                    r.to = cell_str((c.0 + dx, c.1 + dy, c.2 + dz));
                }
            }
            rows.push(r);
        }
    }
    let new_name = format!("{}{}", hdr.name, NAME_SUFFIX);
    let old_uid = hdr.uid.clone();
    let new_uid = if old_uid.len() > UID_PREFIX.len() { format!("{UID_PREFIX}{}", &old_uid[..old_uid.len() - UID_PREFIX.len()]) } else { old_uid.clone() };
    let outcome = |rows: Vec<Row>| CenterOutcome {
        rows,
        finish_blocks: fin_blocks.len(),
        finish_items: fin_items.len(),
        moved_baked: owned.len() + owned_free.len(),
        startfinish,
        centroid,
        center,
        offset_cells: (dx, dy, dz),
        overlaps: overlaps.len(),
        mismatches,
        new_name: new_name.clone(),
        new_uid: new_uid.clone(),
    };
    if dry {
        return Ok(outcome(rows));
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // pass 1: every move is an in-place fixed-size patch; the ghost strip and
    // the rename are splices (they may share a write with patches, not with
    // Id-table renames)
    for f in &fin_blocks {
        match f.rec.free_pos {
            Some(p) => m.move_block_free(f.index, shifted(p, dw)),
            None => {
                let c = f.rec.coords();
                m.move_block_cell(f.index, (c.0 + dx, c.1 + dy, c.2 + dz));
            }
        }
    }
    for (i, it) in &fin_items {
        m.move_item_pos(*i, shifted(it.pos, dw));
        let c = it.coords();
        let cell = [(c.0 + dx).clamp(0, 255) as u8, (c.1 + dy).clamp(0, 255) as u8, (c.2 + dz).clamp(0, 255) as u8];
        m.raw_patches.push((it.coord_off, cell.to_vec()));
    }
    for idx in &owned {
        let rec = m.baked.iter().find(|r| r.index == *idx).cloned().expect("owned baked record");
        let c = rec.coords();
        m.move_baked_cell(*idx, (c.0 + dx, c.1 + dy, c.2 + dz));
    }
    for idx in &owned_free {
        let rec = m.baked.iter().find(|r| r.index == *idx).cloned().expect("owned baked free record");
        let p = rec.free_pos.expect("free pos");
        m.move_baked_free(*idx, shifted(p, dw));
    }
    m.strip_validation_ghost_to(GhostForm::Remove);
    if o.rename {
        let (h, b) = m.set_map_name(&hdr.name, &new_name);
        if h + b == 0 {
            return Err(format!("{map_label}: the map does not declare the name {:?}", hdr.name));
        }
    }
    m.write_to(out).map_err(|e| e.to_string())?;
    // pass 2: the uid (an Id-table rename: its own write)
    if o.reuid && new_uid != old_uid {
        let mut m2 = MapFile::try_load(out)?;
        m2.set_map_uid(&new_uid);
        m2.write_to(out).map_err(|e| e.to_string())?;
    }
    Ok(outcome(rows))
}

// ------------------------------------------------------------------ verify

fn has_chunk(body: &[u8], id: u32) -> bool {
    tmmaps::gbx::all_skip_chunks(body).iter().any(|(c, ..)| *c == id)
}

/// The round-trip checks a written map must pass against its source: it
/// parses, the counts agree with what the step did, and nothing but the
/// listed records differ. Returns the list of findings (empty = clean).
pub fn verify_center(src: &Path, out: &Path, outcome: &CenterOutcome) -> Result<Vec<String>, String> {
    let a = MapFile::try_load(src)?;
    let b = MapFile::try_load(out)?;
    let mut bad = Vec::new();
    if a.blocks.len() != b.blocks.len() {
        bad.push(format!("block count {} -> {}", a.blocks.len(), b.blocks.len()));
    }
    if a.items.len() != b.items.len() {
        bad.push(format!("item count {} -> {}", a.items.len(), b.items.len()));
    }
    if a.baked.len() != b.baked.len() {
        bad.push(format!("baked count {} -> {}", a.baked.len(), b.baked.len()));
    }
    let moved_blocks: HashSet<usize> = outcome.rows.iter().filter(|r| r.step == "center" && r.kind == "block" && r.action == "moved").filter_map(|r| r.index.parse().ok()).collect();
    let moved_items: HashSet<usize> = outcome.rows.iter().filter(|r| r.step == "center" && r.kind == "item" && r.action == "moved").filter_map(|r| r.index.parse().ok()).collect();
    let moved_baked: HashSet<usize> = outcome.rows.iter().filter(|r| r.step == "center" && r.kind == "baked").filter_map(|r| r.index.parse().ok()).collect();
    let (dx, dy, dz) = outcome.offset_cells;
    let dw = [dx as f32 * 32.0, dy as f32 * 8.0, dz as f32 * 32.0];
    for (i, (x, y)) in a.blocks.iter().zip(b.blocks.iter()).enumerate() {
        let same_pose = x.coords() == y.coords() && x.free_pos == y.free_pos;
        let exp = if moved_blocks.contains(&i) {
            match x.free_pos {
                Some(p) => y.free_pos == Some(shifted(p, dw)),
                None => {
                    let c = x.coords();
                    y.coords() == (c.0 + dx, c.1 + dy, c.2 + dz)
                }
            }
        } else {
            same_pose
        };
        if x.name != y.name || x.dir != y.dir || x.flags != y.flags || x.free_rot != y.free_rot || !exp {
            bad.push(format!("block#{i} {} differs ({:?}/{:?} -> {:?}/{:?})", x.name, x.coords(), x.free_pos, y.coords(), y.free_pos));
        }
    }
    for (i, (x, y)) in a.items.iter().zip(b.items.iter()).enumerate() {
        let exp = if moved_items.contains(&i) { y.pos == shifted(x.pos, dw) } else { y.pos == x.pos && y.coords() == x.coords() };
        if x.model != y.model || x.yaw != y.yaw || x.pitch != y.pitch || x.roll != y.roll || x.pivot != y.pivot || x.scale != y.scale || x.waypoint_tag != y.waypoint_tag || !exp {
            bad.push(format!("item#{i} {} differs ({:?} -> {:?})", x.model, x.pos, y.pos));
        }
    }
    for (i, (x, y)) in a.baked.iter().zip(b.baked.iter()).enumerate() {
        let exp = if moved_baked.contains(&x.index) {
            match x.free_pos {
                Some(p) => y.free_pos == Some(shifted(p, dw)),
                None => {
                    let c = x.coords();
                    y.coords() == (c.0 + dx, c.1 + dy, c.2 + dz)
                }
            }
        } else {
            x.coords() == y.coords() && x.free_pos == y.free_pos
        };
        if x.name != y.name || x.dir != y.dir || x.flags != y.flags || !exp {
            bad.push(format!("baked b{i} {} differs", x.name));
        }
    }
    if b.waypoints().iter().filter(|w| w.tag == "Goal").count() == 0 && outcome.finish_blocks + outcome.finish_items > 0 {
        // tags travel with the records; a Goal that vanished means a record did
        bad.push("no Goal-tagged record left".into());
    }
    let hb = tmmaps::header::read(out.to_str().unwrap_or_default())?;
    if hb.validated != "0" {
        bad.push(format!("header validated={}", hb.validated));
    }
    if has_chunk(&b.gbx.body, 0x0305_B00F) {
        bad.push("validation ghost chunk still present".into());
    }
    Ok(bad)
}

pub fn verify_sttf(src: &Path, out: &Path, outcome: &SttfOutcome) -> Result<Vec<String>, String> {
    let a = MapFile::try_load(src)?;
    let b = MapFile::try_load(out)?;
    let mut bad = Vec::new();
    let removed_b: usize = outcome.rows.iter().filter(|r| r.kind == "block" && r.action == "removed").count();
    let removed_i: usize = outcome.rows.iter().filter(|r| r.kind == "item" && r.action == "removed").count();
    if a.blocks.len() - removed_b != b.blocks.len() {
        bad.push(format!("block count {} - {removed_b} != {}", a.blocks.len(), b.blocks.len()));
    }
    if a.items.len() - removed_i != b.items.len() {
        bad.push(format!("item count {} - {removed_i} != {}", a.items.len(), b.items.len()));
    }
    if a.baked.len() - outcome.removed_baked != b.baked.len() {
        bad.push(format!("baked count {} - {} != {}", a.baked.len(), outcome.removed_baked, b.baked.len()));
    }
    let cps = b.waypoints().iter().filter(|w| w.tag == "Checkpoint" || w.tag == "LinkedCheckpoint").count();
    // the tags ride on renamed blocks (the model decides, map-blocks.md §5):
    // every surviving checkpoint TAG must sit on a replaced block, never on an item
    let item_cps = b.items.iter().filter(|it| matches!(it.waypoint_tag.as_deref(), Some("Checkpoint") | Some("LinkedCheckpoint"))).count();
    if item_cps > 0 {
        bad.push(format!("{item_cps} checkpoint-tagged items remain"));
    }
    let replaced: HashSet<String> = outcome.rows.iter().filter(|r| r.action == "replaced").map(|r| r.to_name.clone()).collect();
    let residue_ok = outcome.rows.iter().filter(|r| r.action == "replaced" && r.note.starts_with("WAYPOINT TAG LEFT")).count();
    let mut residue = 0usize;
    for bl in b.blocks.iter().filter(|bl| matches!(bl.waypoint_tag.as_deref(), Some("Checkpoint") | Some("LinkedCheckpoint"))) {
        if !replaced.contains(&bl.name) {
            bad.push(format!("checkpoint-tagged block {} is not a replaced twin", bl.name));
        } else {
            residue += 1;
        }
    }
    if residue > residue_ok {
        bad.push(format!("{residue} checkpoint tags remain on replaced blocks, {residue_ok} expected"));
    }
    let _ = cps;
    // nothing else moved: every kept block / item keeps its pose
    let kept_blocks: Vec<&BlockRec> = {
        let dropped: HashSet<usize> = outcome.rows.iter().filter(|r| r.kind == "block" && r.action == "removed").filter_map(|r| r.index.parse().ok()).collect();
        a.blocks.iter().filter(|x| !dropped.contains(&x.index)).collect()
    };
    for (x, y) in kept_blocks.iter().zip(b.blocks.iter()) {
        if x.coords() != y.coords() || x.free_pos != y.free_pos || (x.flags & !0x100000) != (y.flags & !0x100000) {
            bad.push(format!("block {} ({:?}) changed pose/flags", x.name, x.coords()));
            break;
        }
    }
    let kept_items: Vec<&ItemRec> = {
        let dropped: HashSet<usize> = outcome.rows.iter().filter(|r| r.kind == "item" && r.action == "removed").filter_map(|r| r.index.parse().ok()).collect();
        a.items.iter().filter(|x| !dropped.contains(&x.index)).collect()
    };
    for (x, y) in kept_items.iter().zip(b.items.iter()) {
        if x.pos != y.pos || x.model != y.model || x.yaw != y.yaw {
            bad.push(format!("item {} ({:?}) changed", x.model, x.pos));
            break;
        }
    }
    if has_chunk(&b.gbx.body, 0x0305_B00F) {
        bad.push("validation ghost chunk still present".into());
    }
    Ok(bad)
}

// ------------------------------------------------------------------ driver

pub struct PipelineOpts {
    pub cp: CpMode,
    pub center: CenterOpts,
    pub dry: bool,
    pub keep_sttf: bool,
}

/// One map through both steps: `out_dir/sttf/<stem>.sttf.Map.Gbx`, then
/// `out_dir/<stem>-Straight-to-the-Center.Map.Gbx`. Returns the rows and a
/// one-line summary.
pub fn pipeline(store: &mut DataStore, src: &Path, out_dir: &Path, o: &PipelineOpts) -> Result<(Vec<Row>, String), String> {
    let stem = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let stem = stem.trim_end_matches(".Map.Gbx").to_string();
    let sttf_out: PathBuf = out_dir.join("sttf").join(format!("{stem}.sttf.Map.Gbx"));
    let final_out: PathBuf = out_dir.join(format!("{stem}-Straight-to-the-Center.Map.Gbx"));
    let m = MapFile::try_load(src)?;
    let mut rows = Vec::new();
    let summary;
    {
        let mut ctx = Ctx::new(store, &m);
        let s = sttf(&mut ctx, src, &sttf_out, o.cp, o.dry)?;
        let mut bad_s = Vec::new();
        if !o.dry {
            bad_s = verify_sttf(src, &sttf_out, &s)?;
        }
        let center_src: &Path = if o.dry { src } else { &sttf_out };
        // a fresh context: the STTF output is a new file (indices shifted)
        let m2 = MapFile::try_load(center_src)?;
        let mut ctx2 = Ctx::new(ctx.store, &m2);
        let c = center_finish(&mut ctx2, center_src, &final_out, &o.center, o.dry)?;
        let mut bad_c = Vec::new();
        if !o.dry {
            bad_c = verify_center(center_src, &final_out, &c)?;
        }
        rows.extend(s.rows.iter().cloned());
        rows.extend(c.rows.iter().cloned());
        let mut r = Row::new(&format!("{stem}.Map.Gbx"), "summary", "map", 0, &format!("{}{}", m.size[0], if m.size[0] == 48 { " (Stadium)" } else { "" }));
        r.action = format!(
            "sttf: {} replaced, {} blocks removed, {} items removed, {} baked removed; center: {} finish blocks + {} items + {} baked moved by ({},{},{}) cells, centroid ({:.1},{:.1}) -> centre ({:.1},{:.1}), {} overlaps, {} startfinish, {} mismatches",
            s.replaced, s.removed_blocks, s.removed_items, s.removed_baked, c.finish_blocks, c.finish_items, c.moved_baked, c.offset_cells.0, c.offset_cells.1, c.offset_cells.2, c.centroid.0, c.centroid.1, c.center.0, c.center.1, c.overlaps, c.startfinish, s.mismatches + c.mismatches
        );
        r.to_name = c.new_name.clone();
        r.note = if bad_s.is_empty() && bad_c.is_empty() { if o.dry { "dry-run".into() } else { "verified".into() } } else { format!("VERIFY FAILED: {} {}", bad_s.join("; "), bad_c.join("; ")) };
        r.to = if o.dry { String::new() } else { final_out.display().to_string() };
        r.y = c.new_uid.clone();
        summary = r.tsv();
        rows.push(r);
        if !o.dry && !o.keep_sttf {
            // the intermediate stays by default (the coordinator asked for it)
        }
    }
    Ok((rows, summary))
}
