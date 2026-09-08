//! `mapgeom fillers MAP` — every recorded (baked) clip filler of a map against
//! the block UNIT FACE it stands on: who occupies its cell, what clip list that
//! occupant hangs on the shared face, who owns the piece, and the piece's own
//! clip flags. The table the filler DRAW rule is read off (2026-09-08, the
//! elevated water-road floors of Summer 05 / 15 that `fullfree` dropped).
//!
//! Conventions (measured on the pack prefabs and Summer 20 cp3):
//! * a recorded piece with `dir` d stands on side d of its cell (the
//!   `Base_VFCMiddle_Air` wall is the local z = 32 = North plane; dir turns it
//!   like a block) and its OWNER is the block unit across side d;
//! * a FreeClipBottom piece (FCB: the underside / channel floor of the block
//!   ABOVE) is recorded in the cell below the owner and lies at the top of its
//!   cell (`Straight_FCBInside` is y 8..9) — it faces the occupant's TOP list;
//!   a FreeClipTop piece (FCT) is the top plate of the block BELOW, recorded in
//!   the cell above, and faces the occupant's BOTTOM list;
//! * world side w of a unit holds the block's local side (w - dir) mod 4.
//!
//! ```text
//! mapgeom fillers MAP [--collection Stadium] [--filter PAT] [--cells X0,Z0:X1,Z1]
//!                     [--covered] [--summary]
//! ```

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::blockmap::{BlockInfoIndex, FLAG_ADDITIONAL_SHIFT, FLAG_FREE, FLAG_GROUND, FLAG_PILLAR, FLAG_SUBVARIANT_SHIFT, FLAG_VARIANT_MASK};
use crate::store::DataStore;
use tmmaps::map::{BlockRec, MapFile};

/// Local side vectors (x, z): North +z, East -x, South -z, West +x.
pub const SIDE_VEC: [(i32, i32); 4] = [(0, 1), (-1, 0), (0, -1), (1, 0)];
pub const SIDE_NAMES: [&str; 6] = ["N", "E", "S", "W", "T", "B"];

pub fn stem(p: &str) -> String {
    p.rsplit('\\').next().unwrap_or(p).split('.').next().unwrap_or("").to_ascii_lowercase()
}

/// One authored block unit standing in a cell.
#[derive(Clone, Debug)]
pub struct Occupant {
    pub index: usize,
    pub name: String,
    pub pillar: bool,
    pub tile: bool,
    pub ghost: bool,
    pub unit: usize,
    /// world N, E, S, W, Top, Bottom → clip stems (lowercase)
    pub faces: [Vec<String>; 6],
}

/// The piece's own clip identity.
#[derive(Clone, Debug, Default)]
pub struct ClipId {
    pub ty: Option<i32>,
    pub full_free: bool,
    pub exclusive: bool,
    pub deletable: bool,
    pub group: String,
    pub sym: String,
    pub group2: String,
    pub sym2: String,
    pub vert: String,
    pub horiz: String,
    pub asym: String,
}

impl ClipId {
    pub fn ids(&self) -> Vec<&str> {
        [&self.group, &self.sym, &self.group2, &self.sym2].iter().map(|s| s.as_str()).filter(|s| !s.is_empty()).collect()
    }
}

pub struct Faces {
    pub occupants: HashMap<[u8; 3], Vec<Occupant>>,
    pub clips: HashMap<String, ClipId>,
}

/// Every authored (non-free) block's units, turned by dir, with the clip stems
/// on each world side; the clip identity of every clip block info the map's
/// baked list or those faces name.
pub fn faces(store: &mut DataStore, idx: &mut BlockInfoIndex, m: &MapFile) -> Faces {
    let tiles: std::collections::BTreeSet<String> = m.genealogy_zones().into_iter().collect();
    let mut occupants: HashMap<[u8; 3], Vec<Occupant>> = HashMap::new();
    let mut clip_names: std::collections::BTreeSet<String> = m.baked.iter().filter(|b| b.name != "Sea").map(|b| b.name.to_ascii_lowercase()).collect();
    for b in m.blocks.iter().filter(|b| b.flags & FLAG_FREE == 0) {
        let Some(path) = idx.path_for(&b.name) else { continue };
        let Ok(bi) = idx.load(store, &path) else { continue };
        let ground = b.flags & FLAG_GROUND != 0;
        let vindex = (b.flags & FLAG_VARIANT_MASK) as usize;
        let sub = ((b.flags >> FLAG_SUBVARIANT_SHIFT) & 63) as usize;
        let addv = ((b.flags >> FLAG_ADDITIONAL_SHIFT) & 0x7F) as usize;
        let Some(pk) = bi.pick_placement_add(ground, vindex, sub, addv) else { continue };
        let units = &pk.variant.block_units;
        let (mut minx, mut maxx, mut minz, mut maxz) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        for u in units {
            minx = minx.min(u.offset[0]);
            maxx = maxx.max(u.offset[0]);
            minz = minz.min(u.offset[2]);
            maxz = maxz.max(u.offset[2]);
        }
        let (w, d) = (maxx - minx + 1, maxz - minz + 1);
        let dir = (b.dir & 3) as usize;
        for (ui, u) in units.iter().enumerate() {
            let (x, z) = (u.offset[0] - minx, u.offset[2] - minz);
            let (rx, rz) = match dir {
                0 => (x, z),
                1 => (d - 1 - z, x),
                2 => (w - 1 - x, d - 1 - z),
                _ => (z, w - 1 - x),
            };
            let (cx, cy, cz) = (b.file_cell[0] as i32 + rx, b.file_cell[1] as i32 + u.offset[1], b.file_cell[2] as i32 + rz);
            if !((0..=255).contains(&cx) && (0..=255).contains(&cy) && (0..=255).contains(&cz)) {
                continue;
            }
            let mut faces: [Vec<String>; 6] = Default::default();
            for wside in 0..4 {
                let local = (wside + 4 - dir) % 4;
                faces[wside] = u.clips[local].iter().map(|p| stem(p)).collect();
            }
            faces[4] = u.clips[4].iter().map(|p| stem(p)).collect();
            faces[5] = u.clips[5].iter().map(|p| stem(p)).collect();
            for f in faces.iter().flatten() {
                clip_names.insert(f.clone());
            }
            occupants.entry([cx as u8, cy as u8, cz as u8]).or_default().push(Occupant {
                index: b.index,
                name: b.name.clone(),
                pillar: b.flags & FLAG_PILLAR != 0 || bi.is_pillar == Some(true),
                tile: tiles.contains(&b.name),
                ghost: b.flags & (1 << 28) != 0,
                unit: ui,
                faces,
            });
        }
    }
    let mut clips: HashMap<String, ClipId> = HashMap::new();
    for n in clip_names {
        let Some(p) = idx.path_for(&n) else { continue };
        let Ok(bi) = idx.load(store, &p) else { continue };
        let Some(c) = bi.clip.as_ref() else { continue };
        let s = |o: &Option<String>| o.clone().unwrap_or_default();
        let (g2, s2) = c.clip_group_ids_v1.clone().unwrap_or_default();
        clips.insert(
            n,
            ClipId {
                ty: c.clip_type,
                full_free: c.is_full_free_clip.unwrap_or(false),
                exclusive: c.is_exclusive_free_clip.unwrap_or(false),
                deletable: c.can_be_deleted_by_full_free_clip.unwrap_or(false),
                group: s(&c.clip_group_id),
                sym: s(&c.symmetrical_clip_group_id),
                group2: g2,
                sym2: s2,
                vert: s(&c.vertical_clip_group_id),
                horiz: s(&c.horizontal_clip_group_id),
                asym: s(&c.asym_clip_id),
            },
        );
    }
    Faces { occupants, clips }
}

/// The face of the OCCUPANT of the piece's cell the piece lies against:
/// side d for a side clip, Top for an FCB (the owner is above), Bottom for an
/// FCT (the owner is below).
pub fn facing_face(ty: Option<i32>, dir: u8) -> usize {
    match ty {
        Some(3) => 4,
        Some(2) => 5,
        _ => (dir & 3) as usize,
    }
}

/// The owner's cell and the owner's face that lists the piece: across side d
/// (its face (d+2) mod 4), above (its Bottom) for an FCB, below (its Top) for
/// an FCT.
pub fn owner_cell_face(cell: [u8; 3], ty: Option<i32>, dir: u8) -> Option<([u8; 3], usize)> {
    let (dx, dy, dz, face) = match ty {
        Some(3) => (0, 1, 0, 5),
        Some(2) => (0, -1, 0, 4),
        _ => {
            let (vx, vz) = SIDE_VEC[(dir & 3) as usize];
            (vx, 0, vz, ((dir & 3) as usize + 2) % 4)
        }
    };
    let (x, y, z) = (cell[0] as i32 + dx, cell[1] as i32 + dy, cell[2] as i32 + dz);
    if !(0..=255).contains(&x) || !(0..=255).contains(&y) || !(0..=255).contains(&z) {
        return None;
    }
    Some(([x as u8, y as u8, z as u8], face))
}

/// Do two clips CONNECT by the pack's group ids (either of A's group ids equals
/// either of B's, the asym id names the other, or the two share a vertical /
/// horizontal group)?
pub fn connects(a: &ClipId, a_name: &str, b: &ClipId, b_name: &str) -> &'static str {
    if a.ids().iter().any(|x| b.ids().contains(x)) {
        return "group";
    }
    if (!a.asym.is_empty() && a.asym.eq_ignore_ascii_case(b_name)) || (!b.asym.is_empty() && b.asym.eq_ignore_ascii_case(a_name)) {
        return "asym";
    }
    if !a.vert.is_empty() && a.vert == b.vert {
        return "vertical";
    }
    if !a.horiz.is_empty() && a.horiz == b.horiz {
        return "horizontal";
    }
    ""
}

/// THE FILLER DRAW RULE (`TINY_FILLER_RULE=face`, 2026-09-08): the game draws
/// a recorded free-clip piece only where the face it stands on is FREE. A face
/// is free when nothing stands in the piece's cell, when the occupant's unit
/// hangs NO clip on that face (an undefined face: the top of a slope base's
/// upper unit, the open sides of a lattice pillar), or when the occupant's
/// face carries a FULL-FREE clip (a complete wall — the pillar / deco-wall
/// family: the neighbour dresses its side against the wall as if the cell were
/// empty, minus the pieces the wall makes redundant, `can_be_deleted_by_full_
/// free_clip`). A full-free piece is drawn wherever it is recorded. Everything
/// else — a free clip against a neighbour's face that carries its own
/// (non-full-free) clips, two blocks joined — is hidden.
///
/// Read off the pack data and the same-camera A/Bs: Summer 20 cp3's plastic
/// ramp wall `DecoWallSlope2StartVFCLeft` in the DecoPlatformSlopeBase cell
/// (the wedge's face carries `DecoPlatformSlopeBaseFCSmall`) — hidden; the
/// checkpoint's OpenTech skirts in the DecoHill cells (hill faces carry
/// `DecoWallSlope2StraightVFC*` / `DecoHillSlope2StraightFC*`) — hidden; the
/// pool's `WaterFCCenter`/`WaterHFC*` rim in the wedge cell (the wedge's face
/// carries the full-free `DecoWallBaseVFC`) — drawn; the wedge's `DecoWall
/// SlopeBaseVFCRight` in the DecoWallBasePillar cell (full-free walls) —
/// drawn; Summer 05's elevated water-road floor `TrackWallWaterStraightFCB
/// InsideV2` in the cell below the road, the wedge's upper unit whose TOP
/// carries no clip — drawn (the regression of the `fullfree` rule, which
/// dropped it: vjeux fell through that road once already).
///
/// `pillars`: whether pillar blocks count as occupants (their faces decide
/// like any block's — DecoWallBasePillar's four DecoWallBaseVFC are full-free
/// walls, StructurePillar's sides carry nothing, TrackWallStraightPillar's
/// N/S carry TrackWallVFC); `false` = the `fullfree` rule's reading (a pillar
/// cell is an empty cell).
pub fn verdict(f: &Faces, b: &BlockRec, pillars: bool) -> Option<String> {
    verdict_with(f, b, pillars, closes_default())
}

/// What CLOSES a face (TINY_FILLER_CLOSE): `any` — any clip on the occupant's
/// face closes it unless one of them is full-free (then only a deletable piece
/// is hidden, deleted by the wall); `nondeletable` — a face closes only when it
/// carries a clip that is NOT `can_be_deleted_by_full_free_clip` (every
/// full-free clip is deletable, so a wall never closes; the deletable
/// TrackWallStraightFCT on a TrackWall pillar's top, RoadTechFC, TechnicsScreen
/// *FCB, StructureSupportFC do not close either — the elevated water road's
/// floor over its own auto-pillars IS drawn, Summer 05 turbo section,
/// same-camera 2026-09-08 22:05Z).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Closes {
    Any,
    NonDeletable,
    /// only SIDE faces close: an FCB / FCT piece (the top plate of the block
    /// below, the floor of the block above) is drawn wherever it is recorded
    SideOnly,
}

pub fn closes_default() -> Closes {
    closes_default_for(std::env::var("TINY_FILLER_CLOSE").ok().as_deref())
}

pub fn closes_default_for(v: Option<&str>) -> Closes {
    match v {
        Some("any") => Closes::Any,
        Some("nondeletable") => Closes::NonDeletable,
        _ => Closes::SideOnly,
    }
}

pub fn verdict_with(f: &Faces, b: &BlockRec, pillars: bool, closes: Closes) -> Option<String> {
    let me = b.name.to_ascii_lowercase();
    // only CLIP records are judged (a terrain tile in the baked list is not a
    // filler; `hidden_tiles` owns those)
    let mine = f.clips.get(&me)?.clone();
    let face = facing_face(mine.ty, b.dir);
    // a full-free SIDE piece (a pillar or deco wall panel) is drawn wherever it
    // is recorded (Summer 10's start, 20 cp3); a full-free TOP / BOTTOM plate
    // (PlatformBaseFCT / FCB, the pillar and platform plates: full-free AND
    // deletable) follows the plate rule below
    if mine.full_free && face < 4 {
        return None;
    }
    let Some(occ) = f.occupants.get(&b.file_cell) else { return None };
    if closes == Closes::SideOnly && face >= 4 {
        // A TOP / BOTTOM piece — a plate or floor hanging between stacked
        // blocks. A NON-deletable one (the water roads' channel floors
        // `TrackWallWaterStraightFCBInside*`, del=0) is drawn wherever it is
        // recorded: Summer 05 over the wedge and over the road's own pillar,
        // Summer 15 over the arch top — the frames of 22:00Z. A DELETABLE one
        // (`TrackWallStraightFCB`, a road's underside plate; `PlatformBaseFCT`,
        // a pillar's top plate — the pack marks them CanBeDeletedByFullFreeClip)
        // is the optional dressing of a free face: hidden as soon as the block
        // it faces hangs its own top / bottom clips there. Summer 15, the water
        // channel through the reactor gate (vjeux, 2026-09-08 23:26Z): the
        // road slope's FCB plates and the pillars' FCT plates recorded in the
        // DecoWallWaterBase cells (Top [DecoWallWaterBaseFCT|…FCTInside], Bottom
        // [DecoWallWaterBaseFCB|…FCBInside]) came out as grey slabs in the water;
        // the original shows water.
        if !mine.deletable {
            return None;
        }
        for o in occ.iter().filter(|o| !o.tile && (pillars || !o.pillar)) {
            let list = &o.faces[face];
            if !list.is_empty() {
                return Some(format!("deletable plate against {}'s {} face [{}]", o.name, if face == 4 { "Top" } else { "Bottom" }, list.join("|")));
            }
        }
        return None;
    }
    for o in occ.iter().filter(|o| !o.tile && (pillars || !o.pillar)) {
        let list = &o.faces[face];
        if list.is_empty() {
            continue;
        }
        match closes {
            Closes::NonDeletable => {
                let firm: Vec<&String> = list.iter().filter(|c| f.clips.get(*c).map(|x| !x.deletable).unwrap_or(true)).collect();
                if !firm.is_empty() {
                    return Some(format!("free clip against {}'s face [{}]", o.name, firm.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("|")));
                }
            }
            Closes::Any | Closes::SideOnly => {
                // a HORIZONTAL clip (an HFC rim along the top of a wall) whose
                // neighbour hangs a rim of the same horizontal group on the shared
                // face: the two rims run on (the editor never pairs HFC clips —
                // both are baked — and the game draws both: Summer 20's two facing
                // WaterRampZoneCurveOut, b4087/b4115, the ResonantMetal rim the
                // author drives at t 27.1–27.5 s, gone from ship10)
                if !mine.horiz.is_empty() && list.iter().any(|c| f.clips.get(c).map(|x| x.horiz == mine.horiz).unwrap_or(false)) {
                    continue;
                }
                let wall = list.iter().any(|c| f.clips.get(c).map(|x| x.full_free).unwrap_or(false));
                if wall {
                    if mine.deletable {
                        return Some(format!("deletable, against {}'s full-free wall", o.name));
                    }
                    continue;
                }
                return Some(format!("free clip against {}'s face [{}]", o.name, list.join("|")));
            }
        }
    }
    None
}

/// One record's verdict under the candidate rules, as a class label.
pub struct Row {
    pub class: String,
    pub line: String,
}

pub fn classify(f: &Faces, b: &BlockRec) -> Row {
    let me = b.name.to_ascii_lowercase();
    let mine = f.clips.get(&me).cloned().unwrap_or_default();
    let face = facing_face(mine.ty, b.dir);
    let occ: Vec<&Occupant> = f.occupants.get(&b.file_cell).map(|v| v.iter().collect()).unwrap_or_default();
    let real: Vec<&Occupant> = occ.iter().copied().filter(|o| !o.pillar && !o.tile).collect();
    let pillars: Vec<&Occupant> = occ.iter().copied().filter(|o| o.pillar).collect();
    // the owner check
    let owner = owner_cell_face(b.file_cell, mine.ty, b.dir).and_then(|(c, of)| {
        f.occupants.get(&c).and_then(|v| v.iter().find(|o| o.faces[of].iter().any(|s| *s == me)).map(|o| format!("{}#{}u{}", o.name, o.index, o.unit)))
    });
    let facing: Vec<String> = real.iter().map(|o| format!("{}u{}[{}]", o.name, o.unit, o.faces[face].join("|"))).collect();
    let facing_lists: Vec<&Vec<String>> = real.iter().map(|o| &o.faces[face]).collect();
    let any_clips = facing_lists.iter().any(|l| !l.is_empty());
    let named = facing_lists.iter().any(|l| l.iter().any(|c| *c == me));
    let conn: Vec<String> = facing_lists
        .iter()
        .flat_map(|l| l.iter())
        .filter_map(|c| f.clips.get(c).and_then(|theirs| { let how = connects(&mine, &me, theirs, c); if how.is_empty() { None } else { Some(format!("{c}:{how}")) } }))
        .collect();
    let facing_full_free = facing_lists.iter().flat_map(|l| l.iter()).any(|c| f.clips.get(c).map(|x| x.full_free).unwrap_or(false));
    let class = if real.is_empty() {
        if pillars.is_empty() { "free".to_string() } else { "pillar-only".to_string() }
    } else if mine.full_free {
        "covered:fullfree".to_string()
    } else if named {
        "covered:named".to_string()
    } else if !conn.is_empty() {
        "covered:connects".to_string()
    } else if !any_clips {
        "covered:face-empty".to_string()
    } else if facing_full_free {
        "covered:face-fullfree".to_string()
    } else {
        "covered:face-other".to_string()
    };
    // the three rules side by side: the landed `fullfree` (a non-full-free piece in a
    // cell a non-pillar block unit covers is out unless the occupant's OPPOSITE
    // face names it — the face it read), and `face` with and without pillars
    let fullfree_out = !real.is_empty() && !mine.full_free && !real.iter().any(|o| o.faces[((b.dir & 3) as usize + 2) % 4].iter().any(|c| *c == me));
    let face_p = verdict(f, b, true);
    let face_np = verdict(f, b, false);
    let ty = mine.ty.map(crate::blockinfo::clip_type_name).unwrap_or("-");
    let flags = format!("{}{}{}", if mine.full_free { "F" } else { "-" }, if mine.exclusive { "X" } else { "-" }, if mine.deletable { "d" } else { "-" });
    let line = format!(
        "b{}\t{}\t{}\t{}\t{:08X}\t{},{},{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        b.index,
        b.name,
        ty,
        flags,
        b.flags,
        b.coords().0,
        b.coords().1,
        b.coords().2,
        SIDE_NAMES[(b.dir & 3) as usize],
        class,
        owner.unwrap_or_else(|| "?".to_string()),
        if facing.is_empty() { "-".to_string() } else { facing.join(" ; ") },
        if pillars.is_empty() { String::new() } else { format!("P:{}", pillars.iter().map(|o| o.name.as_str()).collect::<Vec<_>>().join("+")) },
        conn.join(","),
        mine.group,
        if fullfree_out { "OUT" } else { "keep" },
        face_p.as_deref().map(|_| "OUT").unwrap_or("keep"),
        face_np.as_deref().map(|_| "OUT").unwrap_or("keep"),
    );
    Row { class, line }
}

pub fn cmd(store: &mut DataStore, args: &[String]) {
    let path = args.first().cloned().unwrap_or_default();
    let flag = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let has = |k: &str| args.iter().any(|a| a == k);
    let collection = flag("--collection").unwrap_or_else(|| "Stadium".to_string());
    let pat = flag("--filter");
    let cells = flag("--cells").map(|s| {
        let (a, b) = s.split_once(':').expect("--cells X0,Z0:X1,Z1");
        let p = |t: &str| -> (i32, i32) {
            let v: Vec<i32> = t.split(',').map(|x| x.trim().parse().expect("a cell number")).collect();
            (v[0], v[1])
        };
        (p(a), p(b))
    });
    let covered_only = has("--covered");
    let summary = has("--summary");
    let m = MapFile::load(Path::new(&path));
    let mut idx = BlockInfoIndex::build(store, &collection);
    let f = faces(store, &mut idx, &m);
    let mut tally: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut by_class: BTreeMap<String, usize> = BTreeMap::new();
    if !summary {
        println!("id\tname\tclip_type\tFXd\tflags\tcell\tside\tclass\towner\tfacing(occupant unit[face list])\tpillars\tconnects\tgroup\tfullfree\tface\tface_nopillar");
    }
    for b in m.baked.iter().filter(|b| b.name != "Sea" && b.flags & FLAG_FREE == 0) {
        if let Some(p) = &pat {
            if !b.name.contains(p.as_str()) {
                continue;
            }
        }
        let c = b.coords();
        if let Some(((x0, z0), (x1, z1))) = cells {
            if c.0 < x0.min(x1) || c.0 > x0.max(x1) || c.2 < z0.min(z1) || c.2 > z0.max(z1) {
                continue;
            }
        }
        let row = classify(&f, b);
        if covered_only && !row.class.starts_with("covered") {
            continue;
        }
        *tally.entry((b.name.clone(), row.class.clone())).or_insert(0) += 1;
        *by_class.entry(row.class.clone()).or_insert(0) += 1;
        if !summary {
            println!("{}", row.line);
        }
    }
    if summary {
        println!("class\tcount");
        for (k, v) in &by_class {
            println!("{k}\t{v}");
        }
        println!("\nname\tclass\tcount");
        for ((n, c), v) in &tally {
            println!("{n}\t{c}\t{v}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(ty: i32, full_free: bool, deletable: bool) -> ClipId {
        ClipId { ty: Some(ty), full_free, deletable, ..Default::default() }
    }

    fn rec(name: &str, cell: [u8; 3], dir: u8) -> BlockRec {
        BlockRec { index: 0, name: name.to_string(), name_field: 0, dir, file_cell: cell, coord_off: 0, flags: 0, waypoint_tag: None, free_off: None, free_pos: None, free_rot: None }
    }

    fn occupant(name: &str, pillar: bool, faces: [Vec<&str>; 6]) -> Occupant {
        Occupant { index: 0, name: name.to_string(), pillar, tile: false, ghost: false, unit: 0, faces: faces.map(|v| v.into_iter().map(String::from).collect()) }
    }

    /// The pack's roles: a full-free wall (deletable, like every full-free
    /// clip), a deletable skirt, a firm (non-deletable) skirt, a floor (FCB).
    fn table() -> Faces {
        let mut clips = HashMap::new();
        clips.insert("wall".to_string(), clip(1, true, true));
        clips.insert("skirt".to_string(), clip(1, false, true));
        clips.insert("firm".to_string(), clip(1, false, false));
        clips.insert("floor".to_string(), clip(3, false, false));
        clips.insert("plate".to_string(), clip(2, false, true));
        Faces { occupants: HashMap::new(), clips }
    }

    const C: [u8; 3] = [10, 10, 10];

    #[test]
    fn free_cell_and_empty_face_draw() {
        let mut f = table();
        assert!(verdict(&f, &rec("firm", C, 0), false).is_none(), "no occupant");
        f.occupants.insert(C, vec![occupant("Deck", false, [vec![], vec![], vec![], vec![], vec![], vec![]])]);
        assert!(verdict(&f, &rec("firm", C, 0), false).is_none(), "occupant's North face hangs no clip");
        assert!(verdict(&f, &rec("floor", C, 0), false).is_none(), "occupant's Top hangs no clip: the water-road floor over a slope base");
    }

    #[test]
    fn a_joined_side_face_hides_a_free_clip() {
        let mut f = table();
        f.occupants.insert(C, vec![occupant("Wedge", false, [vec!["firm"], vec![], vec![], vec![], vec![], vec![]])]);
        assert!(verdict(&f, &rec("skirt", C, 0), false).is_some(), "the ramp wall against the wedge's skirt face");
        assert!(verdict(&f, &rec("skirt", C, 1), false).is_none(), "another side of the same cell is free");
        assert!(verdict(&f, &rec("wall", C, 0), false).is_none(), "a full-free piece is drawn wherever it is recorded");
        // the piece's own deletability does not matter against a firm face
        assert!(verdict(&f, &rec("firm", C, 0), false).is_some());
    }

    #[test]
    fn a_full_free_wall_frees_the_face_but_deletes_deletable_pieces() {
        let mut f = table();
        f.occupants.insert(C, vec![occupant("Wedge", false, [vec![], vec![], vec!["wall"], vec![], vec![], vec![]])]);
        assert!(verdict(&f, &rec("firm", C, 2), false).is_none(), "the pool rim against the wedge's DecoWallBaseVFC");
        assert!(verdict(&f, &rec("skirt", C, 2), false).is_some(), "a deletable skirt is what the wall replaces");
    }

    #[test]
    fn top_and_bottom_pieces_follow_their_own_deletable_flag() {
        let mut f = table();
        f.occupants.insert(C, vec![occupant("ArchTop", false, [vec![], vec![], vec![], vec![], vec!["plate"], vec!["firm"]])]);
        // Summer 15's water floor (del=0) over TrackWallArch1x2SideTop (top face carries a firm FCT)
        assert!(verdict_with(&f, &rec("floor", C, 0), false, Closes::SideOnly).is_none());
        assert!(verdict_with(&f, &rec("floor", C, 0), false, Closes::Any).is_some(), "the `any` variant hid it — the variant the 15 frame refuted");
        // a DELETABLE FCT plate (a pillar's top) under a block whose Bottom hangs its own clips: hidden (15's water channel)
        assert!(verdict_with(&f, &rec("plate", C, 0), false, Closes::SideOnly).is_some(), "a deletable plate against an occupied Bottom face");
        f.occupants.insert(C, vec![occupant("Deck", false, [vec![], vec![], vec![], vec![], vec!["plate"], vec![]])]);
        assert!(verdict_with(&f, &rec("plate", C, 0), false, Closes::SideOnly).is_none(), "the same plate under a block with an empty Bottom face is drawn");
        assert_eq!(closes_default_for(None), Closes::SideOnly);
    }

    #[test]
    fn pillars_are_open_cells_unless_asked() {
        let mut f = table();
        f.occupants.insert(C, vec![occupant("TrackWallStraightPillar", true, [vec!["firm"], vec![], vec![], vec![], vec![], vec![]])]);
        assert!(verdict(&f, &rec("skirt", C, 0), false).is_none(), "a pillar cell is an open cell");
        assert!(verdict(&f, &rec("skirt", C, 0), true).is_some(), "TINY_FILLER_PILLARS=occupant: its faces decide");
    }

    #[test]
    fn only_clip_records_are_judged() {
        let mut f = table();
        f.occupants.insert(C, vec![occupant("Wedge", false, [vec!["firm"], vec![], vec![], vec![], vec![], vec![]])]);
        assert!(verdict(&f, &rec("Grass", C, 0), false).is_none(), "a terrain tile in the baked list is not a filler");
    }
}

#[cfg(test)]
mod tests_horizontal {
    use super::*;

    #[test]
    fn a_rim_meeting_a_rim_of_its_horizontal_group_runs_on() {
        let mut clips = HashMap::new();
        clips.insert("rim".to_string(), ClipId { ty: Some(1), horiz: "WaterRampZoneHFClips".into(), ..Default::default() });
        clips.insert("wall".to_string(), ClipId { ty: Some(1), deletable: true, vert: "DecoWallBaseVFC".into(), ..Default::default() });
        clips.insert("skirt".to_string(), ClipId { ty: Some(1), ..Default::default() });
        let mut occupants = HashMap::new();
        let cell = [5u8, 5, 5];
        let faces: [Vec<String>; 6] = [vec!["wall".into(), "rim".into()], vec!["skirt".into()], vec![], vec![], vec![], vec![]];
        occupants.insert(cell, vec![Occupant { index: 0, name: "WaterRampZoneCurveOut".into(), pillar: false, tile: false, ghost: false, unit: 0, faces }]);
        let f = Faces { occupants, clips };
        let rec = |dir: u8| BlockRec { index: 0, name: "rim".into(), name_field: 0, dir, file_cell: cell, coord_off: 0, flags: 0, waypoint_tag: None, free_off: None, free_pos: None, free_rot: None };
        assert!(verdict(&f, &rec(0), false).is_none(), "the face carries a rim of the same horizontal group");
        assert!(verdict(&f, &rec(1), false).is_some(), "a skirt face still closes a rim");
    }
}
