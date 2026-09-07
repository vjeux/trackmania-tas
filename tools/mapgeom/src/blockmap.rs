//! A map's authored blocks against their block infos: which variant each
//! placement uses, which cells it occupies, and what each cell's four sides
//! face — a neighbour, or nothing (in which case the game hangs the side's
//! CLIP prefab there).
//!
//! Cell rotation follows `place.rs` (clockwise looking down, shifted back onto
//! the footprint), applied to unit offsets in whole cells:
//!
//! ```text
//!   dir=0: (x, z)          dir=1: (D-1-z, x)
//!   dir=2: (W-1-x, D-1-z)  dir=3: (z, W-1-x)
//! ```
//!
//! with `W x D` the variant's footprint in cells. Side vectors turn the same
//! way, so a unit's local side `s` faces world side `(s + dir) % 4`.

use crate::blockinfo::{self, BlockInfo, Variant, SIDE_NAMES};
use crate::store::DataStore;
use std::collections::{BTreeMap, HashMap};

/// Block flag bits, as GBX.NET's `CGameCtnBlock` names them.
pub const FLAG_VARIANT_MASK: u32 = 63;
pub const FLAG_SUBVARIANT_SHIFT: u32 = 6;
pub const FLAG_GROUND: u32 = 1 << 12;
pub const FLAG_CLIP: u32 = 1 << 13;
pub const FLAG_PILLAR: u32 = 1 << 14;
pub const FLAG_SKINNABLE: u32 = 1 << 15;
pub const FLAG_REPLACEMENT: u32 = 1 << 16;
pub const FLAG_DECAL: u32 = 1 << 17;
pub const FLAG_WAYPOINT: u32 = 1 << 20;
pub const FLAG_BIT21: u32 = 1 << 21;
pub const FLAG_GHOST: u32 = 1 << 28;
pub const FLAG_FREE: u32 = 1 << 29;

/// Local side vectors in (x, z), indexed like `SIDE_NAMES`. **North is +z and
/// East is -x**: RoadTechCurve1 hangs its clips on North and East, and its
/// road-chunk path runs from the x = 0 edge to the z = 32 edge; RoadTechStraight
/// (clips North/South) runs z = 0..32. See REPORT.md.
pub const SIDE_VEC: [(i32, i32); 4] = [(0, 1), (-1, 0), (0, -1), (1, 0)];

pub fn rotate_vec(v: (i32, i32), dir: u8) -> (i32, i32) {
    match dir & 3 {
        0 => v,
        1 => (-v.1, v.0),
        2 => (-v.0, -v.1),
        _ => (v.1, -v.0),
    }
}

/// A variant's unit offsets turned by `dir` and shifted back onto the
/// footprint, as (unit index, world-relative cell).
pub fn footprint(v: &Variant, dir: u8) -> Vec<(usize, [i32; 3])> {
    if v.block_units.is_empty() {
        return vec![(usize::MAX, [0, 0, 0])];
    }
    let (mut minx, mut maxx, mut minz, mut maxz) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    for u in &v.block_units {
        minx = minx.min(u.offset[0]);
        maxx = maxx.max(u.offset[0]);
        minz = minz.min(u.offset[2]);
        maxz = maxz.max(u.offset[2]);
    }
    let (w, d) = (maxx - minx + 1, maxz - minz + 1);
    v.block_units
        .iter()
        .enumerate()
        .map(|(i, u)| {
            let (x, z) = (u.offset[0] - minx, u.offset[2] - minz);
            let (rx, rz) = match dir & 3 {
                0 => (x, z),
                1 => (d - 1 - z, x),
                2 => (w - 1 - x, d - 1 - z),
                _ => (z, w - 1 - x),
            };
            (i, [rx, u.offset[1], rz])
        })
        .collect()
}

/// Every block info file in the store, by upper-cased file stem.
pub struct BlockInfoIndex {
    by_stem: HashMap<String, Vec<String>>,
    cache: HashMap<String, Result<BlockInfo, String>>,
    /// The collection whose files win when two packs carry the same name
    /// (`BlueBay\...` vs `Stadium\...`): the Summer 2026 map's baked clips
    /// name BlueBay's `TrackToDecoStraightFCBGround`, not Stadium's
    /// `TrackToGrass...`, so BlueBay is the default.
    collection: String,
}

impl BlockInfoIndex {
    pub fn build(store: &DataStore, collection: &str) -> BlockInfoIndex {
        let mut by_stem: HashMap<String, Vec<String>> = HashMap::new();
        for e in store.entries() {
            let p = e.path();
            if !p.to_uppercase().contains("\\GAMECTNBLOCKINFO\\") {
                continue;
            }
            let file = p.rsplit('\\').next().unwrap_or(&p);
            let stem = file.split('.').next().unwrap_or(file).to_uppercase();
            by_stem.entry(stem).or_default().push(p.to_string());
        }
        BlockInfoIndex { by_stem, cache: HashMap::new(), collection: collection.to_uppercase() }
    }

    /// Every block info path a map block NAME could refer to, best first:
    /// the preferred collection wins, then Classic, Pillar, other, Clip.
    pub fn paths_for(&self, name: &str) -> Vec<String> {
        let up = name.to_uppercase();
        // A BlueBay map names a Stadium-family clip block with the collection
        // folded into the name (`StadiumStructurePillarToFlatACB` is
        // `GameCtnBlockInfoClip\Stadium\StructurePillarToFlatACB.EDClip.Gbx`).
        let cands: Vec<String> = match self.by_stem.get(&up) {
            Some(c) => c.clone(),
            None => match up.strip_prefix("STADIUM").and_then(|rest| self.by_stem.get(rest)) {
                Some(c) => c.iter().filter(|p| p.to_uppercase().contains("\\STADIUM\\")).cloned().collect(),
                // `GateSpecialBoostOriented` (Summer 15): no block info of that
                // name in the 2026-04 packs; the plain gate is the same prefab
                // bar the orientation arrow.
                None => match up.strip_suffix("ORIENTED").and_then(|base| self.by_stem.get(base)) {
                    Some(c) => c.clone(),
                    None => return Vec::new(),
                },
            },
        };
        let rank = |p: &str| -> (u32, u32) {
            let u = p.to_uppercase();
            let coll = if u.starts_with(&format!("{}\\", self.collection)) { 0 } else { 1 };
            let kind = if u.contains("GAMECTNBLOCKINFOCLASSIC\\") {
                0
            } else if u.contains("GAMECTNBLOCKINFOPILLAR\\") {
                1
            } else if u.contains("GAMECTNBLOCKINFOCLIP\\") {
                9
            } else {
                2
            };
            (coll, kind)
        };
        let mut best: Vec<&String> = cands.iter().collect();
        best.sort_by_key(|p| (rank(p), p.len()));
        best.into_iter().cloned().collect()
    }

    pub fn path_for(&self, name: &str) -> Option<String> {
        self.paths_for(name).into_iter().next()
    }

    pub fn load(&mut self, store: &mut DataStore, path: &str) -> Result<&BlockInfo, String> {
        let key = path.to_uppercase();
        if !self.cache.contains_key(&key) {
            let r = blockinfo::load(store, path);
            self.cache.insert(key.clone(), r);
        }
        match &self.cache[&key] {
            Ok(b) => Ok(b),
            Err(e) => Err(e.clone()),
        }
    }

    pub fn loaded(&self) -> impl Iterator<Item = (&String, &Result<BlockInfo, String>)> {
        self.cache.iter()
    }
}

/// What one side of one placed unit faces.
#[derive(Clone, Debug)]
pub struct SideFacing {
    pub unit: usize,
    /// the unit's LOCAL side, 0..6 (North..Bottom)
    pub local_side: usize,
    /// the world side after rotation, 0..6
    pub world_side: usize,
    pub neighbour_cell: [i32; 3],
    /// non-clip occupants of the neighbour cell (`a#<index> Name` authored,
    /// `b#<index> Name` baked); empty means the side is FREE
    pub occupants: Vec<String>,
    /// clip blocks (the game's own generated clips, baked) in that cell
    pub clip_occupants: Vec<String>,
    /// the clip block infos this unit hangs on that side
    pub clips: Vec<String>,
    /// the prefab(s) those clips draw, one entry per clip
    pub clip_prefabs: Vec<String>,
    /// per clip: the occupants whose facing side presents a MATCHING clip (the
    /// two connect and neither is drawn); empty means this clip is DRAWN
    pub connected: Vec<Vec<String>>,
}

impl SideFacing {
    /// The clips this side actually draws (not connected to anything).
    pub fn drawn(&self) -> Vec<usize> {
        (0..self.clips.len()).filter(|i| self.connected.get(*i).is_none_or(|c| c.is_empty())).collect()
    }
}

impl SideFacing {
    pub fn is_free(&self) -> bool {
        self.occupants.is_empty()
    }
    /// Does a baked clip in the neighbour cell name one of this side's
    /// clips? `None` when the cell holds no baked clip.
    pub fn baked_clip_agrees(&self) -> Option<bool> {
        if self.clip_occupants.is_empty() {
            return None;
        }
        let mine: Vec<String> = self.clips.iter().map(|c| short(c).to_uppercase()).collect();
        Some(self.clip_occupants.iter().any(|o| {
            let name = o.split_once(' ').map(|(_, n)| n).unwrap_or(o).to_uppercase();
            mine.contains(&name)
        }))
    }
}

pub struct Placement {
    pub index: usize,
    pub name: String,
    pub path: Option<String>,
    pub alt_paths: Vec<String>,
    pub cell: [i32; 3],
    pub dir: u8,
    pub flags: u32,
    pub free: bool,
    pub baked: bool,
    pub is_clip: bool,
    pub variant_label: String,
    pub variant_name: String,
    /// mobil list used, lists available, additional variants (ground, air)
    pub list: usize,
    pub lists: usize,
    pub additional: (usize, usize),
    pub notes: Vec<String>,
    pub cells: Vec<(usize, [i32; 3])>,
    pub prefabs: Vec<String>,
    pub sides: Vec<SideFacing>,
    /// per unit, per local side: the clip block-info paths (what the side
    /// evaluation of the NEIGHBOURS needs)
    pub unit_clips: Vec<[Vec<String>; 6]>,
    pub error: Option<String>,
}

fn rec_cell(b: &tmmaps::map::BlockRec) -> [i32; 3] {
    let c = b.coords();
    [c.0, c.1, c.2]
}

/// Walk a map. `with_baked` adds the baked blocks to the occupancy map (they
/// are terrain and clips the game generated; a road ending against baked
/// terrain is not free, and a baked CLIP in a neighbour cell is the game's
/// own answer to "what does this free side get").
pub fn walk(
    store: &mut DataStore,
    m: &tmmaps::map::MapFile,
    with_baked: bool,
    collection: &str,
) -> (Vec<Placement>, BlockInfoIndex) {
    let mut idx = BlockInfoIndex::build(store, collection);
    let mut placements: Vec<Placement> = Vec::new();
    let recs: Vec<(bool, &tmmaps::map::BlockRec)> = m
        .blocks
        .iter()
        .map(|b| (false, b))
        .chain(if with_baked { m.baked.iter().map(|b| (true, b)).collect::<Vec<_>>() } else { Vec::new() })
        .collect();
    for (baked, b) in &recs {
        if b.flags == 0xFFFF_FFFF {
            continue;
        }
        let free = b.flags & FLAG_FREE != 0;
        let ground = b.flags & FLAG_GROUND != 0;
        let vindex = (b.flags & FLAG_VARIANT_MASK) as usize;
        let sub = ((b.flags >> FLAG_SUBVARIANT_SHIFT) & 63) as usize;
        let paths = idx.paths_for(&b.name);
        let mut p = Placement {
            index: b.index,
            name: b.name.clone(),
            path: paths.first().cloned(),
            alt_paths: paths.iter().skip(1).cloned().collect(),
            cell: rec_cell(b),
            dir: b.dir,
            flags: b.flags,
            free,
            baked: *baked,
            is_clip: false,
            variant_label: String::new(),
            variant_name: String::new(),
            list: 0,
            lists: 0,
            additional: (0, 0),
            notes: Vec::new(),
            cells: Vec::new(),
            prefabs: Vec::new(),
            sides: Vec::new(),
            unit_clips: Vec::new(),
            error: None,
        };
        let Some(path) = p.path.clone() else {
            p.error = Some("no block info file with this name".into());
            placements.push(p);
            continue;
        };
        match idx.load(store, &path) {
            Err(e) => p.error = Some(e),
            Ok(bi) => {
                p.is_clip = bi.clip.is_some();
                p.additional = (bi.additional_ground.len(), bi.additional_air.len());
                match bi.pick_placement(ground, vindex, sub) {
                    None => p.error = Some("block info has no variant with units or mobils".into()),
                    Some(pk) => {
                        p.variant_label = pk.label.clone();
                        p.variant_name = pk.variant.name.clone();
                        p.list = pk.list;
                        p.lists = pk.variant.mobils.len();
                        p.notes = pk.notes.clone();
                        p.prefabs = pk.prefabs();
                        p.unit_clips = pk.variant.block_units.iter().map(|u| u.clips.clone()).collect();
                        if !free {
                            p.cells = footprint(pk.variant, b.dir);
                        }
                    }
                }
            }
        }
        placements.push(p);
    }
    // Occupancy: world cell -> placement list index.
    let mut occ: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    for (pi, p) in placements.iter().enumerate() {
        if p.free {
            continue;
        }
        for (_, rel) in &p.cells {
            let w = [p.cell[0] + rel[0], p.cell[1] + rel[1], p.cell[2] + rel[2]];
            occ.entry(w).or_default().push(pi);
        }
    }
    // Sides. A side's clip is DRAWN unless the block across it presents a
    // MATCHING clip on its facing side (same file, same ClipGroupId, or the
    // declared asymmetrical partner): matching clips connect and vanish,
    // anything else — nothing, terrain, a pillar, a block with a different
    // clip — leaves the clip standing. Checked against the map's own baked
    // clip blocks in `summary`.
    let n = placements.len();
    for pi in 0..n {
        if placements[pi].free || placements[pi].path.is_none() || placements[pi].error.is_some() {
            continue;
        }
        let ground = placements[pi].flags & FLAG_GROUND != 0;
        let dir = placements[pi].dir;
        let cell = placements[pi].cell;
        let cells = placements[pi].cells.clone();
        let unit_clips = placements[pi].unit_clips.clone();
        let mut sides = Vec::new();
        for (ui, rel) in &cells {
            let Some(uc) = unit_clips.get(*ui) else { continue };
            for side in 0..6 {
                let clips = uc[side].clone();
                let (world_side, nb) = if side < 4 {
                    let (vx, vz) = rotate_vec(SIDE_VEC[side], dir);
                    ((side + dir as usize) % 4, [cell[0] + rel[0] + vx, cell[1] + rel[1], cell[2] + rel[2] + vz])
                } else {
                    let dy = if side == 4 { 1 } else { -1 };
                    (side, [cell[0] + rel[0], cell[1] + rel[1] + dy, cell[2] + rel[2]])
                };
                let mut occupants = Vec::new();
                let mut clip_occupants = Vec::new();
                let mut connected: Vec<Vec<String>> = vec![Vec::new(); clips.len()];
                if let Some(list) = occ.get(&nb) {
                    for o in list {
                        if *o == pi {
                            continue;
                        }
                        let q = &placements[*o];
                        let tag = format!("{}#{} {}", if q.baked { "b" } else { "a" }, q.index, q.name);
                        if q.is_clip {
                            clip_occupants.push(tag);
                            continue;
                        }
                        // The neighbour's clips on the side facing us.
                        let theirs: Vec<String> = q
                            .cells
                            .iter()
                            .find(|(_, r)| [q.cell[0] + r[0], q.cell[1] + r[1], q.cell[2] + r[2]] == nb)
                            .and_then(|(qu, _)| q.unit_clips.get(*qu))
                            .map(|qc| {
                                let qside = if side < 4 {
                                    let opp_world = (world_side + 2) % 4;
                                    (opp_world + 4 - q.dir as usize % 4) % 4
                                } else if side == 4 {
                                    5
                                } else {
                                    4
                                };
                                qc[qside].clone()
                            })
                            .unwrap_or_default();
                        // A PILLAR's top and bottom clips are drawn only into
                        // empty cells: the game bakes no StructurePillarFCT under
                        // a road, no FCB above another pillar (Summer 2026 - 01,
                        // 130 such cells, zero baked clips).
                        let pillar_vertical = side >= 4 && placements[pi].flags & FLAG_PILLAR != 0;
                        for (ci, c) in clips.iter().enumerate() {
                            if pillar_vertical || clips_match(store, &mut idx, std::slice::from_ref(c), &theirs) {
                                connected[ci].push(tag.clone());
                            }
                        }
                        occupants.push(tag);
                    }
                }
                let mut clip_prefabs = Vec::new();
                for c in &clips {
                    match idx.load(store, c) {
                        Ok(cb) => match cb.pick_placement(ground, 0, 0) {
                            Some(cp) => clip_prefabs.push(cp.prefabs().join("+")),
                            None => clip_prefabs.push("(clip has no variant)".into()),
                        },
                        Err(e) => clip_prefabs.push(format!("(clip failed: {})", e)),
                    }
                }
                sides.push(SideFacing {
                    unit: *ui,
                    local_side: side,
                    world_side,
                    neighbour_cell: nb,
                    occupants,
                    clip_occupants,
                    connected,
                    clips,
                    clip_prefabs,
                });
            }
        }
        placements[pi].sides = sides;
    }
    (placements, idx)
}

/// Do two facing clip lists connect? Same file; the same non-empty
/// ClipGroupId; one's SymmetricalClipGroupId is the other's ClipGroupId
/// (`StructurePillarFCT` ↔ `StructurePillarFCB`, `StructureSupportFCLeft` ↔
/// `FCRight`, read off the EDClip files); or one names the other as its
/// asymmetrical partner.
fn clips_match(store: &mut DataStore, idx: &mut BlockInfoIndex, mine: &[String], theirs: &[String]) -> bool {
    if theirs.is_empty() {
        return false;
    }
    for a in mine {
        for b in theirs {
            if a.eq_ignore_ascii_case(b) {
                return true;
            }
        }
    }
    // (name, group, symmetrical group, asymmetrical partner)
    let meta = |store: &mut DataStore, idx: &mut BlockInfoIndex, p: &str| -> (String, String, String, String) {
        match idx.load(store, p) {
            Ok(bi) => {
                let c = bi.clip.clone().unwrap_or_default();
                (
                    short(p).to_uppercase(),
                    c.clip_group_id.unwrap_or_default().to_uppercase(),
                    c.symmetrical_clip_group_id.unwrap_or_default().to_uppercase(),
                    c.asym_clip_id.unwrap_or_default().to_uppercase(),
                )
            }
            Err(_) => (short(p).to_uppercase(), String::new(), String::new(), String::new()),
        }
    };
    let ms: Vec<_> = mine.iter().map(|p| meta(store, idx, p)).collect();
    let ts: Vec<_> = theirs.iter().map(|p| meta(store, idx, p)).collect();
    for (mn, mg, msg, ma) in &ms {
        for (tn, tg, tsg, ta) in &ts {
            if (!mg.is_empty() && mg == tg)
                || (!msg.is_empty() && msg == tg)
                || (!tsg.is_empty() && tsg == mg)
                || (!ma.is_empty() && ma == tn)
                || (!ta.is_empty() && ta == mn)
            {
                return true;
            }
        }
    }
    false
}
pub fn tsv(placements: &[Placement]) -> String {
    let mut s = String::from(
        "index\tkind\tname\tblockinfo\talt_blockinfo\tcell_x\tcell_y\tcell_z\tdir\tflags\tground\tvariant\tsubvariant\tvariant_picked\tvariant_name\tmobil_list\tmobil_lists\tadditional_ground\tadditional_air\tnotes\tcells\tprefabs\tNorth\tEast\tSouth\tWest\tTop\tBottom\terror\n",
    );
    for p in placements {
        let cells: Vec<String> = p
            .cells
            .iter()
            .map(|(u, r)| format!("u{}@({},{},{})", u, p.cell[0] + r[0], p.cell[1] + r[1], p.cell[2] + r[2]))
            .collect();
        let mut side_cols: [Vec<String>; 6] = Default::default();
        for f in &p.sides {
            // Only sides worth a word: a clip to hang, or something there.
            if f.clips.is_empty() && f.occupants.is_empty() && f.clip_occupants.is_empty() {
                continue;
            }
            let state = if f.is_free() { "FREE".to_string() } else { f.occupants.join("+") };
            // Per clip: DRAW, or CONNECTED to whom.
            let clips: Vec<String> = f
                .clips
                .iter()
                .zip(f.clip_prefabs.iter())
                .enumerate()
                .map(|(i, (c, pf))| {
                    let conn = f.connected.get(i).cloned().unwrap_or_default();
                    format!(
                        "{}{}=>{}",
                        short(c),
                        if conn.is_empty() { " DRAW".to_string() } else { format!(" CONNECTED({})", conn.join("+")) },
                        if pf.is_empty() { "(no prefab)" } else { pf }
                    )
                })
                .collect();
            side_cols[f.world_side].push(format!(
                "u{}->({},{},{}) {}{}{}",
                f.unit,
                f.neighbour_cell[0],
                f.neighbour_cell[1],
                f.neighbour_cell[2],
                state,
                if clips.is_empty() { String::new() } else { format!(" clips {}", clips.join(" , ")) },
                if f.clip_occupants.is_empty() {
                    String::new()
                } else {
                    format!(
                        " game-placed {} [{}]",
                        f.clip_occupants.join("+"),
                        match f.baked_clip_agrees() {
                            Some(true) => "agrees",
                            Some(false) => "DIFFERS",
                            None => "",
                        }
                    )
                }
            ));
        }
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            p.index,
            if p.baked { "baked" } else if p.free { "free" } else { "grid" },
            p.name,
            p.path.as_deref().unwrap_or(""),
            p.alt_paths.join(";"),
            p.cell[0],
            p.cell[1],
            p.cell[2],
            p.dir,
            p.flags,
            (p.flags & FLAG_GROUND != 0) as u8,
            p.flags & FLAG_VARIANT_MASK,
            (p.flags >> FLAG_SUBVARIANT_SHIFT) & 63,
            p.variant_label,
            p.variant_name,
            p.list,
            p.lists,
            p.additional.0,
            p.additional.1,
            p.notes.join("; "),
            cells.join(";"),
            p.prefabs.join(";"),
            side_cols[0].join(" ; "),
            side_cols[1].join(" ; "),
            side_cols[2].join(" ; "),
            side_cols[3].join(" ; "),
            side_cols[4].join(" ; "),
            side_cols[5].join(" ; "),
            p.error.as_deref().unwrap_or(""),
        ));
    }
    s
}

fn short(p: &str) -> &str {
    let f = p.rsplit('\\').next().unwrap_or(p);
    f.split('.').next().unwrap_or(f)
}

/// The cross-check against the game's own baked clips, per clip.
pub fn summary(placements: &[Placement]) -> String {
    let (mut total, mut draw, mut draw_baked_agree, mut draw_baked_differ, mut draw_no_baked) = (0, 0, 0, 0, 0);
    let (mut conn, mut conn_baked_same) = (0, 0);
    let mut differ: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing: BTreeMap<String, usize> = BTreeMap::new();
    for p in placements.iter().filter(|p| !p.baked) {
        for f in &p.sides {
            for (ci, c) in f.clips.iter().enumerate() {
                total += 1;
                let name = short(c).to_uppercase();
                let baked_names: Vec<String> = f
                    .clip_occupants
                    .iter()
                    .map(|o| o.split_once(' ').map(|(_, n)| n).unwrap_or(o).to_uppercase())
                    .collect();
                let baked_same = baked_names.contains(&name);
                let key = format!(
                    "{} {} side {} ({}) clip {} across {:?} game placed {:?}",
                    p.name,
                    p.variant_label,
                    SIDE_NAMES[f.local_side],
                    if f.is_free() { "free" } else { "occupied" },
                    short(c),
                    f.occupants.iter().map(|o| o.split_once(' ').map(|(_, n)| n).unwrap_or(o)).collect::<Vec<_>>(),
                    baked_names
                );
                let connected = f.connected.get(ci).is_some_and(|v| !v.is_empty());
                if connected {
                    conn += 1;
                    if baked_same {
                        conn_baked_same += 1;
                        *differ.entry(format!("CONNECTED but the game baked it: {}", key)).or_default() += 1;
                    }
                } else {
                    draw += 1;
                    if baked_same {
                        draw_baked_agree += 1;
                    } else if !baked_names.is_empty() {
                        draw_baked_differ += 1;
                        *differ.entry(key).or_default() += 1;
                    } else {
                        draw_no_baked += 1;
                        *missing.entry(key).or_default() += 1;
                    }
                }
            }
        }
    }
    let mut s = format!(
        "{} clips hang on unit-sides of authored blocks.\n  {} CONNECT to a matching clip across (the game baked the clip anyway in {} cases)\n  {} should be DRAWN: the game's baked clip block is there for {}, a different clip is there for {}, none is there for {}\n",
        total, conn, conn_baked_same, draw, draw_baked_agree, draw_baked_differ, draw_no_baked
    );
    for (k, n) in differ.iter() {
        s.push_str(&format!("  DIFFERS x{}: {}\n", n, k));
    }
    for (k, n) in missing.iter() {
        s.push_str(&format!("  NO BAKED CLIP x{}: {}\n", n, k));
    }
    s
}

/// Per block-info file: did it parse to its end.
pub fn parse_report(idx: &BlockInfoIndex) -> String {
    let mut rows: BTreeMap<String, String> = BTreeMap::new();
    for (k, r) in idx.loaded() {
        let line = match r {
            Ok(b) => format!(
                "{}\t{}/{} bytes{}{}",
                if b.parsed_to_end() { "OK" } else { "SHORT" },
                b.consumed.0,
                b.consumed.1,
                if b.recovered.is_empty() { "" } else { "\trecovered" },
                if b.skipped_chunks.is_empty() {
                    String::new()
                } else {
                    format!("\tskipped {}", b.skipped_chunks.iter().map(|(c, n)| format!("{:08X}({}B)", c, n)).collect::<Vec<_>>().join(","))
                }
            ),
            Err(e) => format!("FAIL\t{}", e),
        };
        rows.insert(k.clone(), line);
    }
    let mut s = String::new();
    for (k, v) in rows {
        s.push_str(&format!("{}\t{}\n", k, v));
    }
    s
}
