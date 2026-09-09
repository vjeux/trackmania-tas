//! `mapgeom bake` — the ENGINE's free-clip bake, simulated.
//!
//! Established 2026-09-09 (Trackmania.exe disassembly, `/tmp/tmexe/tm.asm`,
//! anchored on the profiler strings `CGameCtnChallenge::InitChallengeData_*`):
//! the client does not draw the map file's BakedBlocks. At load,
//! `InitChallengeData_Clips` walks every authored block unit (Flat/Frontier
//! terrain and clip blocks excepted), every face, every clip with
//! `ClipType != 0` (a FREE clip), allocates a brand-new clip block for it
//! (0x140f37110: a 184-byte CGameCtnBlock — the file's record, if the grid
//! holds one for that owner face, only donates attributes: variant alternate,
//! lightmap id, colour), and decides with the neighbour's clips whether the
//! new block is instantiated (0x140f362c0) or removed (0x140f36370):
//!
//! ```text
//! for each B in clips registered at (cell + step(face), opposite(face)):
//!     if !ClipsConnect(A, B) continue                 // 0x140d23610
//!     if IsFreeClipDeletedBy(A, B) deletedA = true    // 0x140d236e0
//!     if IsFreeClipDeletedBy(B, A) remove(B)          // (anti-clip: instantiate)
//! keep(A) = A.IsAntiClip ? deletedA : !deletedA
//! ```
//!
//! with, decoded instruction by instruction:
//!
//! * `IsFreeClip(c)`        = c.ClipType != 0                              (0x140d23600)
//! * `ClipsConnect(a, b)`   = both free and both non-exclusive → true; else
//!   the grounds equal, the keys equal, and `Match` either way              (0x140d23610)
//! * `IsFreeClipDeletedBy(a, b, dirA, dirB, groundA, groundB)`:             (0x140d236e0)
//!   both free; groundA == groundB; key(a) == key(b); a.IsAlwaysVisibleFreeClip
//!   → NOT deleted; b.IsFullFreeClip && a.CanBeDeletedByFullFreeClip → deleted;
//!   else Top(2) needs b Bottom(3) and Bottom(3) needs b Top(2), with
//!   `DirsCompatible(dirA, dirB, a.TopBottomMultiDir)` and
//!   `DirsCompatible(dirB, dirA, b.TopBottomMultiDir)` (0x140f42200); a Side
//!   clip needs dirA == opposite(dirB); then `Match(a, b)`.
//! * `Match(a, b)`                                                          (0x140d23820)
//!   a's symmetrical group ids non-empty → they intersect b's group ids;
//!   else a's group ids non-empty → they intersect b's group ids;
//!   else a.SymmetricalClipId set → == b's name; else same name.
//!   (an id list is [id1] or [id1, id2]; id2 is ignored when id1 is unset —
//!   0x140d22da0.)
//! * `Opposite`: 0↔2, 1↔3, 4↔5                                              (0x140f40dd0)
//!
//! The ground bit of a clip block is its owner block's (flags bit 12); the
//! direction of a side clip block is the face it hangs on, of a top/bottom
//! clip block its owner's direction.
//!
//! `mapgeom bake MAP [--collection C] [--diff] [--summary]` prints every free
//! clip the engine derives with its verdict, and with `--diff` the file's
//! generated records that the engine does NOT draw (stale) and the clips it
//! draws that the file does NOT hold (missing) — the two lists a converter
//! that works from the file's records gets wrong.

use std::collections::HashMap;

use tmmaps::map::{BlockRec, MapFile};

use crate::fillers::{ClipId, Faces, SIDE_VEC};

/// One free clip the engine derives: on `face` of the unit at `cell`.
#[derive(Clone, Debug)]
pub struct Clip {
    pub cell: [u8; 3],
    pub face: usize,
    pub name: String,
    pub id: ClipId,
    pub ground: bool,
    /// the clip block's direction word: the face for a side clip; for a top/bottom
    /// clip the owner's direction plus the clip's own (chunk 0x0303600C), mod 4
    pub dir: u8,
    pub owner: String,
    pub owner_index: usize,
    pub unit: usize,
    pub deleted: bool,
    pub deleted_by: Vec<String>,
}

impl Clip {
    /// The engine instantiates a normal clip that nothing deleted, an
    /// anti-clip that something did.
    pub fn drawn(&self) -> bool {
        if self.id.anti { self.deleted } else { !self.deleted }
    }
    /// The direction word of the clip block: the face for a side clip, the
    /// owner's direction for a top/bottom one.
    pub fn dir_word(&self) -> usize {
        if self.face < 4 { self.face } else { (self.dir & 3) as usize }
    }
}

pub fn opposite(face: usize) -> usize {
    match face {
        0 => 2,
        1 => 3,
        2 => 0,
        3 => 1,
        4 => 5,
        5 => 4,
        _ => usize::MAX,
    }
}

pub fn step(face: usize) -> (i32, i32, i32) {
    match face {
        0..=3 => {
            let (dx, dz) = SIDE_VEC[face];
            (dx, 0, dz)
        }
        4 => (0, 1, 0),
        _ => (0, -1, 0),
    }
}

/// `IsFreeClip`.
pub fn is_free(c: &ClipId) -> bool {
    c.ty.map(|t| t != 0).unwrap_or(false)
}

fn ids2(a: &str, b: &str) -> Vec<String> {
    if a.is_empty() {
        return Vec::new();
    }
    let mut v = vec![a.to_ascii_lowercase()];
    if !b.is_empty() {
        v.push(b.to_ascii_lowercase());
    }
    v
}

/// `Match(a, b)`.
pub fn matches(a: &ClipId, a_name: &str, b: &ClipId, b_name: &str) -> bool {
    let a_sym = ids2(&a.sym, &a.sym2);
    let b_grp = ids2(&b.group, &b.group2);
    if !a_sym.is_empty() {
        return a_sym.iter().any(|x| b_grp.contains(x));
    }
    let a_grp = ids2(&a.group, &a.group2);
    if !a_grp.is_empty() {
        return a_grp.iter().any(|x| b_grp.contains(x));
    }
    if !a.asym.is_empty() {
        return a.asym.eq_ignore_ascii_case(b_name);
    }
    a_name.eq_ignore_ascii_case(b_name)
}

/// `DirsCompatible(dirA, dirB, multiDir)` — TopBottomMultiDir: 0 SameDir,
/// 1 SymmetricalDirs, 2 AllDir, 3 OpposedDirOnly, 4 PerpendicularDirsOnly,
/// 5 NextDirOnly, 6 PreviousDirOnly.
pub fn dirs_compatible(a: usize, b: usize, multidir: i32) -> bool {
    let opp = (b + 2) % 4;
    let next = (b + 1) % 4;
    let prev = (b + 3) % 4;
    match multidir {
        0 => a == b,
        1 => a == b || a == opp,
        2 => true,
        3 => a == opp,
        4 => a == next || a == prev,
        5 => a == next,
        6 => a == prev,
        _ => false,
    }
}

/// BAKE_GROUND=0 switches the ground-equality test off (a probe of its reading).
fn ground_check() -> bool {
    std::env::var("BAKE_GROUND").map(|v| v != "0").unwrap_or(true)
}

/// `ClipsConnect(a, b)`.
pub fn connects(a: &Clip, b: &Clip) -> bool {
    if !is_free(&a.id) || !is_free(&b.id) {
        return false;
    }
    if !a.id.exclusive && !b.id.exclusive {
        return true;
    }
    if a.ground != b.ground {
        return false;
    }
    matches(&a.id, &a.name, &b.id, &b.name) || matches(&b.id, &b.name, &a.id, &a.name)
}

/// `IsFreeClipDeletedBy(a, b)`: is a removed by b across their shared face?
pub fn deleted_by(a: &Clip, b: &Clip) -> bool {
    if !is_free(&a.id) || !is_free(&b.id) {
        return false;
    }
    if ground_check() && a.ground != b.ground {
        return false;
    }
    if a.id.always_visible {
        return false;
    }
    if b.id.full_free && a.id.deletable {
        return true;
    }
    let (da, db) = (a.dir_word(), b.dir_word());
    match a.id.ty {
        Some(2) => {
            if b.id.ty != Some(3) {
                return false;
            }
            if !dirs_compatible(da, db, a.id.multidir) || !dirs_compatible(db, da, b.id.multidir) {
                return false;
            }
        }
        Some(3) => {
            if b.id.ty != Some(2) {
                return false;
            }
            if !dirs_compatible(da, db, a.id.multidir) || !dirs_compatible(db, da, b.id.multidir) {
                return false;
            }
        }
        _ => {
            if da != (db + 2) % 4 {
                return false;
            }
        }
    }
    matches(&a.id, &a.name, &b.id, &b.name)
}

/// Every free clip of every authored unit face, with the engine's verdict.
/// The ground bit the file's record carries for a clip slot (owner cell, face, name):
/// the engine copies a matching record's attributes onto the clip block it creates
/// (0x140d26d90) before pairing, so where the file speaks, its bit is the one compared.
pub fn record_grounds(f: &Faces, m: &MapFile) -> HashMap<([u8; 3], usize, String), bool> {
    let mut out = HashMap::new();
    for b in &m.baked {
        if b.flags & crate::blockmap::FLAG_FREE != 0 {
            continue;
        }
        let me = f.stem_of(&b.name);
        let Some(id) = f.clips.get(&me) else { continue };
        if let Some((cell, face)) = record_slot(b, id.ty) {
            out.insert((cell, face, me), b.flags & crate::blockmap::FLAG_GROUND != 0);
        }
    }
    out
}

pub fn simulate(f: &Faces, dirs: &HashMap<usize, u8>, grounds: &HashMap<([u8; 3], usize, String), bool>) -> Vec<Clip> {
    let mut clips: Vec<Clip> = Vec::new();
    let mut by_slot: HashMap<([u8; 3], usize), Vec<usize>> = HashMap::new();
    let mut cells: Vec<&[u8; 3]> = f.occupants.keys().collect();
    cells.sort();
    for cell in cells {
        for o in &f.occupants[cell] {
            if o.tile {
                continue;
            }
            for face in 0..6 {
                for (ci, name) in o.faces[face].iter().enumerate() {
                    let Some(id) = f.clips.get(name) else { continue };
                    let odir = dirs.get(&o.index).copied().unwrap_or(0) & 3;
                    let cdir = if face >= 4 { (odir + o.face_dirs[face].get(ci).copied().unwrap_or(0)) & 3 } else { odir };
                    if !is_free(id) {
                        continue;
                    }
                    let i = clips.len();
                    clips.push(Clip {
                        cell: *cell,
                        face,
                        name: name.clone(),
                        id: id.clone(),
                        ground: grounds.get(&(*cell, face, name.clone())).copied().unwrap_or(face < 4 && o.ground_unit),
                        dir: cdir,
                        owner: o.name.clone(),
                        owner_index: o.index,
                        unit: o.unit,
                        deleted: false,
                        deleted_by: Vec::new(),
                    });
                    by_slot.entry((*cell, face)).or_default().push(i);
                }
            }
        }
    }
    for i in 0..clips.len() {
        let (cell, face) = (clips[i].cell, clips[i].face);
        let (dx, dy, dz) = step(face);
        let (nx, ny, nz) = (cell[0] as i32 + dx, cell[1] as i32 + dy, cell[2] as i32 + dz);
        if !((0..=255).contains(&nx) && (0..=255).contains(&ny) && (0..=255).contains(&nz)) {
            continue;
        }
        let ncell = [nx as u8, ny as u8, nz as u8];
        let Some(neigh) = by_slot.get(&(ncell, opposite(face))) else { continue };
        let mut hits = Vec::new();
        for &j in neigh {
            if !connects(&clips[i], &clips[j]) {
                continue;
            }
            if deleted_by(&clips[i], &clips[j]) {
                hits.push(format!("{}@{}", clips[j].name, clips[j].owner));
            }
        }
        if !hits.is_empty() {
            clips[i].deleted = true;
            clips[i].deleted_by = hits;
        }
    }
    clips
}

/// The simulated clip a file record stands for: the record in cell R on side
/// d belongs to the unit across side d, on that unit's face opposite(d); an
/// FCB (owner above) to the unit above, face Bottom; an FCT to the unit
/// below, face Top.
pub fn record_slot(b: &BlockRec, ty: Option<i32>) -> Option<([u8; 3], usize)> {
    let (dx, dy, dz, face) = match ty {
        Some(3) => (0, 1, 0, 5),
        Some(2) => (0, -1, 0, 4),
        _ => {
            let (vx, vz) = SIDE_VEC[(b.dir & 3) as usize];
            (vx, 0, vz, opposite((b.dir & 3) as usize))
        }
    };
    let (x, y, z) = (b.file_cell[0] as i32 + dx, b.file_cell[1] as i32 + dy, b.file_cell[2] as i32 + dz);
    if !((0..=255).contains(&x) && (0..=255).contains(&y) && (0..=255).contains(&z)) {
        return None;
    }
    Some(([x as u8, y as u8, z as u8], face))
}

pub struct Diff {
    /// file records the engine does not draw: (record, why)
    pub stale: Vec<(usize, String, String)>,
    /// clips the engine draws that the file does not hold
    pub missing: Vec<usize>,
    /// file records matched to a drawn simulated clip
    pub confirmed: usize,
}

pub fn diff(clips: &[Clip], f: &Faces, m: &MapFile) -> Diff {
    let mut index: HashMap<([u8; 3], usize, String), Vec<usize>> = HashMap::new();
    for (i, c) in clips.iter().enumerate() {
        index.entry((c.cell, c.face, c.name.clone())).or_default().push(i);
    }
    let mut matched = vec![false; clips.len()];
    let mut stale = Vec::new();
    let mut confirmed = 0;
    let mut free_records = 0usize;
    let mut unknown: Vec<String> = Vec::new();
    for b in &m.baked {
        // a FREE-placed block's clips ride with it (the engine runs them through
        // 0x140f389f0, off the grid); they are not this grid simulation's business
        if b.flags & crate::blockmap::FLAG_FREE != 0 {
            free_records += 1;
            continue;
        }
        let me = f.stem_of(&b.name);
        let Some(id) = f.clips.get(&me) else {
            unknown.push(b.name.clone());
            continue;
        };
        let Some((cell, face)) = record_slot(b, id.ty) else {
            stale.push((b.index, b.name.clone(), "owner cell out of the grid".into()));
            continue;
        };
        match index.get(&(cell, face, me.clone())) {
            None => stale.push((b.index, b.name.clone(), format!("no unit at ({},{},{}) hangs it on face {}", cell[0], cell[1], cell[2], face))),
            Some(v) => {
                let drawn = v.iter().find(|&&i| clips[i].drawn());
                match drawn {
                    Some(&i) => {
                        matched[i] = true;
                        confirmed += 1;
                        if face >= 4 {
                            let rel = ((b.dir & 3) as i32 - (clips[i].dir & 3) as i32).rem_euclid(4);
                            eprintln!("# topbottom-dir\t{}\t{}\tu{}\tface {}\trecord dir {} owner dir {} rel {}", b.name, clips[i].owner, clips[i].unit, face, b.dir & 3, clips[i].dir & 3, rel);
                        }
                    }
                    None => {
                        let c = &clips[v[0]];
                        let why = if c.id.anti { "anti-clip with nothing to answer".to_string() } else { format!("deleted by {}", c.deleted_by.join(", ")) };
                        stale.push((b.index, b.name.clone(), why));
                    }
                }
            }
        }
    }
    let missing: Vec<usize> = clips.iter().enumerate().filter(|(i, c)| c.drawn() && !matched[*i]).map(|(i, _)| i).collect();
    if free_records > 0 {
        eprintln!("# {free_records} records of free-placed blocks not judged");
    }
    if !unknown.is_empty() {
        let mut u: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for n in unknown {
            *u.entry(n).or_insert(0) += 1;
        }
        for (n, k) in u {
            eprintln!("# {k} records named {n}: no clip block info of that name (terrain tile or unknown) — not judged");
        }
    }
    Diff { stale, missing, confirmed }
}

pub fn cmd(store: &mut crate::store::DataStore, rest: &[String]) {
    let path = rest.first().cloned().unwrap_or_else(|| {
        eprintln!("usage: mapgeom bake MAP [--collection C] [--diff] [--summary]");
        std::process::exit(2)
    });
    let flag = |k: &str| rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1)).cloned();
    let has = |k: &str| rest.iter().any(|a| a == k);
    let m = MapFile::load(std::path::Path::new(&path));
    let collection = flag("--collection").unwrap_or_else(|| "Stadium".to_string());
    let mut idx = crate::blockmap::BlockInfoIndex::build(store, &collection);
    let faces = crate::fillers::faces(store, &mut idx, &m);
    let dirs: HashMap<usize, u8> = m.blocks.iter().map(|b| (b.index, b.dir)).collect();
    let grounds = record_grounds(&faces, &m);
    let clips = simulate(&faces, &dirs, &grounds);
    let drawn = clips.iter().filter(|c| c.drawn()).count();
    println!("# {} free clips derived from {} authored units; {} drawn, {} deleted", clips.len(), faces.occupants.values().map(|v| v.len()).sum::<usize>(), drawn, clips.len() - drawn);
    if has("--diff") {
        let d = diff(&clips, &faces, &m);
        println!("# file records: {} confirmed drawn, {} stale (not drawn by the engine); {} drawn clips have no file record", d.confirmed, d.stale.len(), d.missing.len());
        let mut why: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for (i, name, w) in &d.stale {
            *why.entry(format!("{name}: {w}")).or_insert(0) += 1;
            if !has("--summary") {
                println!("stale\tb{i}\t{name}\t{w}");
            }
        }
        for (k, n) in &why {
            println!("# stale x{n}: {k}");
        }
        let mut miss: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        // the neighbour's clips across the shared face: what the EDITOR saw when
        // it did not bake this one
        let mut slot: HashMap<([u8; 3], usize), Vec<usize>> = HashMap::new();
        for (i, c) in clips.iter().enumerate() {
            slot.entry((c.cell, c.face)).or_default().push(i);
        }
        let across = |c: &Clip| -> String {
            let (dx, dy, dz) = step(c.face);
            let n = [(c.cell[0] as i32 + dx) as u8, (c.cell[1] as i32 + dy) as u8, (c.cell[2] as i32 + dz) as u8];
            let occ: Vec<String> = faces.occupants.get(&n).map(|v| v.iter().filter(|o| !o.tile).map(|o| format!("{}{}", o.name, if o.ground { "(G)" } else { "" })).collect()).unwrap_or_default();
            let cl: Vec<String> = slot.get(&(n, opposite(c.face))).map(|v| v.iter().map(|&j| clips[j].name.clone()).collect()).unwrap_or_default();
            format!("across: [{}] clips [{}]", occ.join("|"), cl.join("|"))
        };
        for &i in &d.missing {
            let c = &clips[i];
            *miss.entry(format!("{} on face {} of {}{} — {}", c.name, c.face, c.owner, if c.ground { "(G)" } else { "" }, across(c))).or_insert(0) += 1;
            if !has("--summary") {
                println!("missing\t({},{},{})\tface {}\t{}\towner {}#{}u{}", c.cell[0], c.cell[1], c.cell[2], c.face, c.name, c.owner, c.owner_index, c.unit);
            }
        }
        for (k, n) in &miss {
            println!("# missing x{n}: {k}");
        }
        return;
    }
    if !has("--summary") {
        println!("cell\tface\tclip\tground\towner\tunit\tdir\tverdict\tdeleted_by");
        for c in &clips {
            println!("{},{},{}\t{}\t{}\t{}\t{}#{}\tu{}\t{}\t{}\t{}", c.cell[0], c.cell[1], c.cell[2], c.face, c.name, if c.ground { "G" } else { "A" }, c.owner, c.owner_index, c.unit, c.dir, if c.drawn() { "drawn" } else { "deleted" }, c.deleted_by.join(", "));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(name: &str, ty: i32, full_free: bool, deletable: bool, group: &str, sym: &str) -> ClipId {
        ClipId { ty: Some(ty), full_free, deletable, group: group.into(), sym: sym.into(), ..Default::default() }
    }

    fn at(cell: [u8; 3], face: usize, name: &str, id: ClipId, ground: bool) -> Clip {
        Clip { cell, face, name: name.into(), id, ground, dir: 0, owner: "X".into(), owner_index: 0, unit: 0, deleted: false, deleted_by: Vec::new() }
    }

    #[test]
    fn a_full_free_wall_deletes_a_deletable_skirt_and_nothing_else() {
        let wall = at([1, 1, 1], 0, "decowallbasevfc", clip("decowallbasevfc", 1, true, true, "DecoWallBaseVFC", ""), false);
        let skirt = at([1, 1, 2], 2, "decoplatformfcsmall", clip("decoplatformfcsmall", 1, false, true, "DecoPlatformFCSmallClips", ""), false);
        let firm = at([1, 1, 2], 2, "opentechroadfc", clip("opentechroadfc", 1, false, false, "OpenRoad-Clips", "OpenRoad-Clips"), false);
        assert!(deleted_by(&skirt, &wall), "the wall replaces the deletable skirt");
        assert!(!deleted_by(&firm, &wall), "a non-deletable piece survives a wall");
        assert!(!deleted_by(&wall, &skirt), "the wall itself stays");
    }

    #[test]
    fn same_group_side_clips_delete_each_other_only_across_the_shared_face() {
        let a = at([1, 1, 1], 0, "roadtechfc", clip("roadtechfc", 1, false, true, "RoadTechFreeClips", ""), false);
        let b = at([1, 1, 2], 2, "roadwatervfc", clip("roadwatervfc", 1, false, false, "RoadTechFreeClips", ""), false);
        assert!(deleted_by(&a, &b) && deleted_by(&b, &a), "same group across the face: both go");
        let mut c = b.clone();
        c.face = 1;
        assert!(!deleted_by(&a, &c), "not facing each other: kept");
        let mut g = b.clone();
        g.ground = true;
        assert!(!deleted_by(&a, &g), "a ground clip never pairs with an air clip");
    }

    #[test]
    fn top_bottom_pairing_needs_the_asymmetric_partner_and_compatible_dirs() {
        let fcb = ClipId { ty: Some(3), deletable: true, group: "TrackWallStraight-Clips".into(), asym: "TrackWallStraightFCT".into(), multidir: 1, ..Default::default() };
        let fct = ClipId { ty: Some(2), deletable: true, group: "TrackWallStraight-Clips".into(), asym: "TrackWallStraightFCB".into(), multidir: 1, ..Default::default() };
        let mut a = at([1, 2, 1], 5, "trackwallstraightfcb", fcb, false);
        let mut b = at([1, 1, 1], 4, "trackwallstraightfct", fct, false);
        assert!(deleted_by(&a, &b), "the road's underside meets the wall's top plate: same group");
        b.dir = 1;
        assert!(!deleted_by(&a, &b), "SymmetricalDirs: a quarter turn does not pair");
        b.dir = 2;
        assert!(deleted_by(&a, &b), "SymmetricalDirs: the opposite direction pairs");
        a.id.always_visible = true;
        assert!(!deleted_by(&a, &b), "an always-visible clip is never deleted");
    }

    #[test]
    fn the_water_floor_over_a_wedge_survives_and_the_anti_clip_inverts() {
        let floor = at([19, 19, 18], 5, "trackwallwaterstraightfcbinsidev2", clip("trackwallwaterstraightfcbinsidev2", 3, false, false, "TrackWallWaterStraightFCT", ""), false);
        let plate = at([19, 18, 18], 4, "trackwallstraightfct", clip("trackwallstraightfct", 2, false, true, "TrackWallStraight-Clips", ""), false);
        assert!(!deleted_by(&floor, &plate), "groups differ: the floor stays over the pillar's plate");
        let mut anti = at([1, 1, 1], 0, "opentechzoneacleft", clip("opentechzoneacleft", 1, false, false, "", ""), false);
        anti.id.anti = true;
        assert!(!anti.drawn(), "an anti-clip with nothing beside it is not drawn");
        anti.deleted = true;
        assert!(anti.drawn(), "… and appears once a neighbour deletes it");
    }

    #[test]
    fn matching_reads_symmetrical_groups_first_then_groups_then_names() {
        let a = clip("a", 1, false, false, "G1", "S1");
        let b = clip("b", 1, false, false, "S1", "");
        assert!(matches(&a, "a", &b, "b"), "a's sym group S1 is b's group");
        let c = clip("c", 1, false, false, "G1", "");
        assert!(!matches(&a, "a", &c, "c"), "a has sym groups: its plain group is not consulted");
        assert!(matches(&c, "c", &a, "a"), "c has no sym groups: plain groups intersect");
        let d = ClipId { ty: Some(1), asym: "e".into(), ..Default::default() };
        let e = clip("e", 1, false, false, "", "");
        assert!(matches(&d, "d", &e, "e") && !matches(&e, "e", &d, "d"), "the asymmetric partner id names the other by name");
        assert!(matches(&e, "e", &e, "e"), "nothing else: same name");
    }
}
