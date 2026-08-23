//! map.rs -- TARGETED byte-level surgery on a CGameCtnChallenge (.Map.Gbx).
//!
//! The Python original leaned on gbx-py + `construct` to fully parse and
//! regenerate the map. That is not ported: this reads exactly the two chunks
//! the surgery touches and rewrites only the bytes that change. Everything
//! else in the ~1.2 MB body is copied verbatim, which is why the three
//! container gotchas the Python fought (see make_segments.py's docstring) are
//! structurally impossible here:
//!
//!   * `numNodes` (the writer computed 10 instead of 39 -> "Can't load map"):
//!     a patcher never touches the node array, so the header's value stands.
//!   * collection id 28 (gbx-py rewrote the packed id as the literal "U28"):
//!     collection words are raw u32s that are copied, never re-encoded.
//!   * chunk 0x03043040's internal size field: still needed (an item model
//!     name changes length), and it is one `u32` write -- see `write_to`.
//!
//! What IS needed is the lookback ("Id") string table, because block model
//! names and item model ids are stored in it:
//!
//!   word u32:  0xFFFFFFFF          -> null / unassigned
//!              top 2 bits == 0     -> a *collection* number (28 = the 2026
//!                                     Stadium collection), adds nothing to
//!                                     the table
//!              0x40000000          -> a NEW string follows (u32 len + bytes),
//!                                     appended to the table
//!              0x40000000 | n      -> the n-th (1-based) table entry
//!
//! The table is per-stream: the block chunk 0x0304301F shares the body's state
//! (measured: it starts empty, and the "id version" word 3 was already written
//! by an earlier chunk), while the skippable items chunk 0x03043040 opens its
//! own sub-state (measured: it re-writes the version word 3 and re-defines
//! "Nadeo", which the block chunk had already defined).
//!
//! So a rename is re-encoded by replaying the table over the chunk's Id fields
//! only -- every other byte is memcpy'd. When nothing is renamed the output is
//! byte-identical to the input (asserted by `tests::roundtrip_*`).

use crate::gbx::{Gbx, Reader};

pub const BLOCKS_CHUNK: u32 = 0x0304301F;
pub const ITEMS_CHUNK: u32 = 0x03043040;
pub const WAYPOINT_CLASS: u32 = 0x2E009000;
pub const ANCHORED_OBJECT_CLASS: u32 = 0x03101000;
pub const FACADE: u32 = 0xFACADE01;

pub const FINISH_GATE: &str = "GateFinish32m";
/// TM2020 block cell size in metres (horizontal / vertical).
pub const CELL_XZ: f32 = 32.0;
pub const CELL_Y: f32 = 8.0;

/// One lookback ("Id") field: where it sits, how long its original encoding
/// was, and what string it holds (None = null or a collection number, which is
/// copied through untouched).
#[derive(Clone, Debug)]
pub struct IdField {
    pub off: usize,
    pub len: usize,
    pub name: Option<String>,
    /// The original encoding defined the string inline (rather than referencing).
    pub is_def: bool,
    /// Raw word, for null/collection fields that are copied verbatim.
    pub raw: u32,
    /// Which table slot this field defines (is_def) or references, 0-based.
    pub slot: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct BlockRec {
    pub index: usize,
    pub name: String,
    /// index into `MapFile::block_ids`
    pub name_field: usize,
    pub dir: u8,
    /// raw file cell; the game's / gbx-py's cell is this minus (1,0,1)
    pub raw_coords: [u8; 3],
    /// w612: absolute body offset of the three cell bytes, immediately after
    /// the `dir` byte in this block's record in chunk 0x0304301F. Overwriting
    /// them in place moves a GRID block: no field changes length, the
    /// Id/lookback table is untouched, no chunk changes size, nothing is
    /// re-encoded. (153527's val_valgate finding, ported.)
    pub coord_off: usize,
    pub flags: u32,
    pub waypoint_tag: Option<String>,
    /// `prs`: a FREE block ignores its cell bytes entirely. Its position lives
    /// as six f32 (Vec3 position, Vec3 pitch/yaw/roll) in chunk `0x0304305F`,
    /// one entry per free block in block order. `free_off` is the absolute
    /// body offset of this block's entry, when it is free.
    ///
    /// This matters because on some maps the Goal gate is a free block:
    /// 210218's two `GateExpandableFinish` Goals both sit at raw cell (0,0,0)
    /// and every one of `move_block_cell`'s three bytes is dead there. Moving
    /// such a gate by its cell writes bytes the game never reads, so the map
    /// loads, the ladder runs, and every rung is silent -- a false negative
    /// that looks exactly like "the car does not go there".
    pub free_off: Option<usize>,
    pub free_pos: Option<[f32; 3]>,
    pub free_rot: Option<[f32; 3]>,
}

impl BlockRec {
    /// Cell coordinates the way gbx-py (and therefore the measured geometry in
    /// the Python's report) reports them: the file stores x and z one cell
    /// higher than the world grid.
    pub fn coords(&self) -> (i32, i32, i32) {
        (
            self.raw_coords[0] as i32 - 1,
            self.raw_coords[1] as i32,
            self.raw_coords[2] as i32 - 1,
        )
    }
    pub fn is_waypoint(&self) -> bool {
        self.flags & 0x100000 != 0
    }
}

#[derive(Clone, Debug)]
pub struct ItemRec {
    pub index: usize,
    pub model: String,
    pub model_field: usize,
    /// offsets of the mutable fixed-size fields, absolute in the body
    pub yaw_off: usize,
    pub coord_off: usize,
    pub pos_off: usize,
    pub yaw: f32,
    pub raw_coords: [u8; 3],
    pub pos: [f32; 3],
    pub pitch: f32,
    pub roll: f32,
    /// The point of the model this placement's `pos` names, in model space.
    /// Read from the PLACEMENT, not the model: an item may declare several
    /// pivots and only the placement says which point was used.
    pub pivot: [f32; 3],
    pub scale: f32,
    pub waypoint_tag: Option<String>,
}

impl ItemRec {
    pub fn coords(&self) -> (i32, i32, i32) {
        (
            self.raw_coords[0] as i32,
            self.raw_coords[1] as i32,
            self.raw_coords[2] as i32,
        )
    }
}

/// A waypoint, block- or item-carried, in the order the file lists them
/// (blocks first, then items) -- same order the Python's `find_waypoints`
/// produced, so `--list` output lines up with the Python's.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Block,
    Item,
}

#[derive(Clone, Debug)]
pub struct Waypoint {
    pub kind: Kind,
    pub index: usize,
    pub name: String,
    pub tag: String,
    pub coords: (i32, i32, i32),
    pub pos: Option<[f32; 3]>,
    pub yaw: Option<f32>,
}

impl std::fmt::Display for Waypoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let k = if self.kind == Kind::Block { "block" } else { "item" };
        let pos = match self.pos {
            Some(p) => format!("({}, {}, {})", p[0], p[1], p[2]),
            None => "None".to_string(),
        };
        write!(
            f,
            "<{}#{} {} tag={} cell={:?} pos={}>",
            k, self.index, self.name, self.tag, self.coords, pos
        )
    }
}

pub struct MapFile {
    pub gbx: Gbx,
    /// The BODY-LEVEL Id stream. Chunk 0x0304301F (blocks) and chunk
    /// 0x03043048 (baked blocks) share one lookback table -- measured: the
    /// baked chunk opens with `0x40000000 "Sea"` (a new definition, no id
    /// version word) and its next blocks reference index 50, a string defined
    /// back in the blocks chunk. So both regions must be re-encoded together;
    /// renumbering one alone is what made the server say "Can't load map".
    pub body_regions: Vec<(usize, usize)>,
    pub body_ids: Vec<IdField>,
    pub blocks: Vec<BlockRec>,
    /// Chunk 0x03043048 records, parsed exactly like `blocks`. Most of a map's
    /// geometry can live here: 267460 has 31 records in 0x0304301F and 2 462
    /// baked; 210218 has 21 025 and 26 733. A census that reads only `blocks`
    /// is reading a fraction of the map, and on an all-baked map it reports
    /// zero and looks exactly like an item-built map.
    ///
    /// Authored by the answer-key agent (session 9f00f635, node 105213), whose
    /// the `census` command and the 267460 numbers this carries.
    ///
    /// **These are NOT addressable by any mover.** A baked block's index is
    /// its position in this list, so it aliases an unrelated `blocks` index;
    /// `Move` addresses them `bN@` and every mover refuses them outright.
    pub baked: Vec<BlockRec>,
    /// items chunk: chunk header offset, payload region, Id fields, items
    pub items_chunk_off: Option<usize>,
    /// baked-blocks chunk header offset, for its skippable size field
    pub baked_chunk_off: Option<usize>,
    pub items_region: (usize, usize),
    pub item_ids: Vec<IdField>,
    pub items: Vec<ItemRec>,
    /// pending edits
    pub renames: Vec<(bool, usize, String)>, // (is_item, field index, new name)
    pub raw_patches: Vec<(usize, Vec<u8>)>,
}

fn find_all(hay: &[u8], needle: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            out.push(i);
        }
        i += 1;
    }
    out
}

/// Read one lookback word (plus its inline string when it defines one).
fn read_id(r: &mut Reader, table: &mut Vec<String>) -> IdField {
    let off = r.o;
    let w = r.u32();
    if w == 0xFFFF_FFFF || (w >> 30) == 0 {
        // null, or a collection number -- neither touches the table
        return IdField { off, len: 4, name: None, is_def: false, raw: w, slot: None };
    }
    let idx = w & 0x3FFF_FFFF;
    if idx == 0 {
        let n = r.u32() as usize;
        let s = String::from_utf8_lossy(r.bytes(n)).into_owned();
        let slot = table.len();
        table.push(s.clone());
        IdField { off, len: 8 + n, name: Some(s), is_def: true, raw: w, slot: Some(slot) }
    } else {
        let s = table
            .get(idx as usize - 1)
            .cloned()
            .unwrap_or_else(|| format!("<bad id {}>", idx));
        IdField {
            off,
            len: 4,
            name: Some(s),
            is_def: false,
            raw: w,
            slot: Some(idx as usize - 1),
        }
    }
}

/// Re-encode a chunk's Id fields, SLOT-PRESERVING: every string the original
/// defined keeps its own table slot, so an index that some other part of the
/// file may hold onto still resolves. This matters -- the naive
/// "replay the table, first use defines" encoder produced a file the server
/// rejected with `Can't load map` on map 2, where renaming the map's only
/// `RoadDirtCheckpoint` block DELETED that string from the table and shifted
/// every later index down by one. Keeping the slot (its content simply becomes
/// `RoadDirtFinish`) leaves every index untouched and the map loads.
///
/// Slots are only ever ADDED, never removed: a reference whose wanted name no
/// longer matches its old slot re-points at an equal slot defined earlier, or
/// failing that defines the string afresh at that spot.
fn reemit(body: &[u8], region: (usize, usize), fields: &[IdField]) -> Vec<u8> {
    reemit_regions(body, &[region], fields).pop().unwrap()
}

/// How the lookback table is rebuilt. MEASURED, the hard way: the dedicated
/// server answers `Can't load map` whenever the table's LENGTH changes, because
/// parts of the file downstream of the blocks chunk still hold raw indices into
/// it. Neither encoder is universally safe, so both exist and `reemit_regions`
/// picks the one that leaves the length alone:
///
/// * `SlotPreserving` -- every field that defined a string still defines one,
///   so slots keep their index and only their CONTENT changes. Perfect when
///   the renamed block owns its name outright (map 2: the map's only
///   `RoadDirtCheckpoint`). Has to APPEND a slot when some other field still
///   needs the old name, which shifts every later index (broke map 1).
/// * `Fresh` -- first-use-defines, exactly the rule the game's own writer uses
///   (proof: with no renames it reproduces the file byte for byte). A rename
///   moves a definition, so indices shift only inside the window between the
///   old and new definition sites -- and the length is preserved as long as no
///   name disappears entirely. Right for map 1, wrong for map 2, where the
///   renamed name vanished and every later index shifted down by one.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    SlotPreserving,
    Fresh,
}

/// Re-encode several regions that SHARE one lookback stream, in offset order.
fn reemit_regions(body: &[u8], regions: &[(usize, usize)], fields: &[IdField]) -> Vec<Vec<u8>> {
    let orig_len = fields
        .iter()
        .filter(|f| f.is_def)
        .filter_map(|f| f.slot)
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);
    let (a, alen) = encode(body, regions, fields, Mode::SlotPreserving);
    let (b, blen) = encode(body, regions, fields, Mode::Fresh);
    if std::env::var("TMMAPS_DEBUG").is_ok() {
        eprintln!(
            "    [table] orig={} slot-preserving={} fresh={} regions={:?}",
            orig_len, alen, blen, regions
        );
    }
    if alen == orig_len {
        return a;
    }
    if blen == orig_len {
        return b;
    }
    eprintln!(
        "warning: lookback table length {} -> {} (slot-preserving) / {} (fresh); \
         downstream indices may not resolve",
        orig_len, alen, blen
    );
    if (alen as i64 - orig_len as i64).abs() <= (blen as i64 - orig_len as i64).abs() {
        a
    } else {
        b
    }
}

fn encode(
    body: &[u8],
    regions: &[(usize, usize)],
    fields: &[IdField],
    mode: Mode,
) -> (Vec<Vec<u8>>, usize) {
    // slot -> its (possibly renamed) content, taken from its defining field
    let mut slot_content: Vec<String> = Vec::new();
    for f in fields {
        if f.is_def {
            let s = f.slot.unwrap();
            if slot_content.len() <= s {
                slot_content.resize(s + 1, String::new());
            }
            slot_content[s] = f.name.clone().unwrap_or_default();
        }
    }
    let mut emitted: Vec<String> = Vec::new(); // the new table, in order
    let mut new_index: Vec<Option<u32>> = vec![None; slot_content.len()]; // 1-based
    let mut outs = Vec::new();
    let mut fi = 0usize;
    for &(start, end) in regions {
        let mut out = Vec::with_capacity(end - start + 64);
        let mut cur = start;
        while fi < fields.len() && fields[fi].off < end {
            let f = &fields[fi];
            fi += 1;
            out.extend_from_slice(&body[cur..f.off]);
            cur = f.off + f.len;
            let name = match &f.name {
                None => {
                    out.extend_from_slice(&f.raw.to_le_bytes());
                    continue;
                }
                Some(s) => s.clone(),
            };
            // where can this field point without adding a slot?
            let target: Option<u32> = match mode {
                Mode::SlotPreserving if f.is_def => None,
                Mode::SlotPreserving => {
                    let own = f.slot.and_then(|s| new_index.get(s).copied().flatten());
                    match (f.slot, own) {
                        (Some(s), Some(i)) if slot_content[s] == name => Some(i),
                        _ => emitted.iter().position(|t| *t == name).map(|p| p as u32 + 1),
                    }
                }
                Mode::Fresh => emitted.iter().position(|t| *t == name).map(|p| p as u32 + 1),
            };
            match target {
                Some(i) => out.extend_from_slice(&(0x4000_0000u32 | i).to_le_bytes()),
                None => {
                    emitted.push(name.clone());
                    out.extend_from_slice(&0x4000_0000u32.to_le_bytes());
                    out.extend_from_slice(&(name.len() as u32).to_le_bytes());
                    out.extend_from_slice(name.as_bytes());
                    if let Some(s) = f.slot {
                        if f.is_def {
                            new_index[s] = Some(emitted.len() as u32);
                        }
                    }
                }
            }
        }
        out.extend_from_slice(&body[cur..end]);
        outs.push(out);
    }
    let n = emitted.len();
    (outs, n)
}

/// A CGameCtnBlockSkin node hanging off a block with flags & 0x8000. It holds
/// no Id fields (only plain strings and FileRefs), so it is parsed purely to
/// find where it ends; its bytes are copied through untouched.
fn read_skin_node(r: &mut Reader) {
    loop {
        let cid = r.u32();
        if cid == FACADE {
            break;
        }
        if r.b[r.o..r.o + 4] == *b"PIKS" {
            r.skip(4);
            let n = r.u32() as usize;
            r.skip(n);
            continue;
        }
        match cid {
            0x03059000 => {
                r.string();
                r.string();
            }
            0x03059001 => {
                r.string();
                read_file_ref(r);
            }
            0x03059002 => {
                r.string();
                read_file_ref(r);
                read_file_ref(r);
            }
            0x03059003 => {
                r.u32();
                read_file_ref(r);
            }
            _ => panic!("unknown chunk 0x{:08X} in block skin at {}", cid, r.o - 4),
        }
    }
}

fn read_file_ref(r: &mut Reader) {
    let version = r.u8();
    if version >= 3 {
        r.skip(32); // checksum
    }
    let path = r.string();
    if version >= 1 && (!path.is_empty() || version >= 3) {
        r.string(); // locatorUrl
    }
}

/// A node ref written *with* its class id and no index (how the items
/// sub-archive writes CGameWaypointSpecialProperty). Returns the tag.
fn read_waypoint_node(r: &mut Reader) -> Option<String> {
    let mut tag = None;
    loop {
        let cid = r.u32();
        if cid == FACADE {
            break;
        }
        if r.b[r.o..r.o + 4] == *b"PIKS" {
            r.skip(4);
            let n = r.u32() as usize;
            r.skip(n);
            continue;
        }
        if cid == WAYPOINT_CLASS {
            let version = r.u32();
            if version >= 2 {
                let n = r.u32() as usize;
                tag = Some(String::from_utf8_lossy(r.bytes(n)).into_owned());
                r.u32(); // order (always 0 -- which is why order is measured)
            } else {
                r.u32();
                r.u32();
            }
            continue;
        }
        panic!("unknown chunk 0x{:08X} in waypoint node at {}", cid, r.o - 4);
    }
    tag
}

impl MapFile {
    /// The GBX class of a `.Map.Gbx`: `CGameCtnChallenge`.
    pub const CLASS_CHALLENGE: u32 = 0x0304_3000;

    pub fn load(path: &std::path::Path) -> MapFile {
        let gbx = Gbx::load(path).unwrap_or_else(|e| panic!("{}: {}", path.display(), e));
        // REFUSE ANYTHING THAT IS NOT A MAP.
        //
        // A `.Replay.Gbx` (class 0x03093000) carries a whole map inside chunk
        // 0x03093002, so a chunk walk over its body finds this map's blocks and
        // items and every offset below is an offset INSIDE a nested container.
        // The ghost arm found the sharp end of that: a carried map's own chunk
        // declares a size running past the end of the carried map, so a walk
        // that "corrects" a size word writes four bytes into the middle of a
        // map — producing a file whose every string reads back perfectly and
        // which then validates to nothing.
        //
        // Nothing here needs to do that, so nothing here is allowed to. The
        // map comes out of the recording with `ghost map extract` and goes
        // back in with `ghost map set`, and both are controlled where they
        // live.
        if gbx.class_id != MapFile::CLASS_CHALLENGE {
            panic!(
                "{}: this is GBX class {:#010X}, not a map ({:#010X}).\n  \
                 A recording is `tools/ghost`'s job. If it carries a map, take the map out and \
                 put it back:\n    \
                 ghost map extract IN --out m.Map.Gbx\n    \
                 tmmaps <edit> m.Map.Gbx --out m2.Map.Gbx\n    \
                 ghost map set IN OUT --map m2.Map.Gbx",
                path.display(),
                gbx.class_id,
                MapFile::CLASS_CHALLENGE
            );
        }
        MapFile::from_gbx(gbx)
    }

    pub fn from_gbx(gbx: Gbx) -> MapFile {
        let body = gbx.body.clone();
        let mut seen_nodes: std::collections::HashSet<u32> = std::collections::HashSet::new();
        let (blocks_region, mut body_ids, blocks, table) = parse_blocks(&body, &mut seen_nodes);
        let mut body_regions = vec![blocks_region];
        let mut baked_chunk_off = None;
        let mut baked: Vec<BlockRec> = Vec::new();
        let mut baked_parsed = false;
        if std::env::var("TMMAPS_NO_BAKED").is_err() {
            baked_parsed = true;
            if let Some((off, s, e, bk)) =
                parse_baked(&body, table, &mut body_ids, &mut seen_nodes)
            {
                baked_chunk_off = Some(off);
                body_regions.push((s, e));
                baked = bk;
            }
        }
        let mut blocks = blocks;
        parse_free_positions(&body, &mut blocks, &mut baked, baked_parsed);
        let (items_chunk_off, items_region, item_ids, items) = parse_items(&body);
        MapFile {
            gbx,
            body_regions,
            body_ids,
            blocks,
            baked,
            items_chunk_off,
            baked_chunk_off,
            items_region,
            item_ids,
            items,
            renames: Vec::new(),
            raw_patches: Vec::new(),
        }
    }

    pub fn waypoints(&self) -> Vec<Waypoint> {
        let mut out = Vec::new();
        for b in &self.blocks {
            if !b.is_waypoint() {
                continue;
            }
            let tag = match &b.waypoint_tag {
                Some(t) => t.clone(),
                None => continue,
            };
            // yaw for a relocated gate: North/South face x, East/West face z
            let yaw = match b.dir {
                0 | 2 => 0.0f32,
                _ => std::f32::consts::FRAC_PI_2,
            };
            out.push(Waypoint {
                kind: Kind::Block,
                index: b.index,
                name: b.name.clone(),
                tag,
                coords: b.coords(),
                // `prs`: a FREE block's cell bytes are dead; its real position
                // is the f32 triple in chunk 0x0304305F. Reporting `pos=None`
                // for one -- as this did -- hides the single fact that decides
                // how it must be moved.
                pos: b.free_pos,
                yaw: Some(yaw),
            });
        }
        for it in &self.items {
            let tag = match &it.waypoint_tag {
                Some(t) => t.clone(),
                None => continue,
            };
            out.push(Waypoint {
                kind: Kind::Item,
                index: it.index,
                name: it.model.clone(),
                tag,
                coords: it.coords(),
                pos: Some(it.pos),
                yaw: Some(it.yaw),
            });
        }
        out
    }

    // ---------------------------------------------------------------- edits
    pub fn set_block_name(&mut self, block_index: usize, name: &str) {
        let f = self.blocks[block_index].name_field;
        self.renames.push((false, f, name.to_string()));
    }

    pub fn set_item_model(&mut self, item_index: usize, name: &str) {
        let f = self.items[item_index].model_field;
        self.renames.push((true, f, name.to_string()));
    }

    /// w612: rotate a GRID block in place (the `dir` byte immediately before
    /// the three cell bytes). Same model, same size, same record length --
    /// only the facing changes. Needed because this map's Goal gate triggers
    /// on a PLANE perpendicular to its facing: an unrotated gate is silent for
    /// a car travelling along the plane's own axis.
    pub fn set_block_dir(&mut self, block_index: usize, dir: u8) {
        let b = self.blocks[block_index].clone();
        assert!(dir < 4, "dir is 0..3, got {}", dir);
        self.raw_patches.push((b.coord_off - 1, vec![dir]));
    }

    /// w612: move a GRID block to another cell, position-only. Overwrites the
    /// three cell bytes in place: no model swap, no promotion, so the trigger
    /// volume is exactly the one the block always had
    /// (FLEET_NOTICE_origin_control_insufficient_v1). `cell` is in gbx-py /
    /// world-grid coordinates; the file stores x and z one higher.
    pub fn move_block_cell(&mut self, block_index: usize, cell: (i32, i32, i32)) {
        let b = self.blocks[block_index].clone();
        assert!(
            (0..=254).contains(&cell.0) && (0..=255).contains(&cell.1) && (0..=254).contains(&cell.2),
            "cell {:?} out of the one-byte grid range",
            cell
        );
        self.raw_patches.push((
            b.coord_off,
            vec![(cell.0 + 1) as u8, cell.1 as u8, (cell.2 + 1) as u8],
        ));
    }

    /// `prs`: move a FREE block, position-only, by overwriting the three f32
    /// of its entry in chunk `0x0304305F`. Same model, same rotation, same
    /// record length, same trigger volume -- the
    /// `FLEET_NOTICE_origin_control_insufficient_v1` question does not arise.
    ///
    /// This is the free-block twin of `move_block_cell`, and on a map whose
    /// Goal gate is free it is the ONLY thing that moves the gate.
    /// `move_block_cell` on a free block writes bytes the game does not read:
    /// the map still loads, the ladder still runs, and every rung is silent.
    pub fn move_block_free(&mut self, block_index: usize, pos: [f32; 3]) {
        let b = self.blocks[block_index].clone();
        let off = b.free_off.unwrap_or_else(|| {
            panic!(
                "block#{} {} is a GRID block (flags {:08X}); use move_block_cell",
                block_index, b.name, b.flags
            )
        });
        let mut p = Vec::new();
        for v in pos {
            p.extend_from_slice(&v.to_le_bytes());
        }
        self.raw_patches.push((off, p));
    }

    /// `prs`: rotate a FREE block in place (its pitch/yaw/roll triple, the
    /// three f32 immediately after its position in `0x0304305F`). The
    /// free-block twin of `set_block_dir` -- a rotation, not a promotion.
    pub fn set_block_free_rot(&mut self, block_index: usize, rot: [f32; 3]) {
        let b = self.blocks[block_index].clone();
        let off = b.free_off.unwrap_or_else(|| {
            panic!("block#{} {} is a GRID block; use set_block_dir", block_index, b.name)
        });
        let mut p = Vec::new();
        for v in rot {
            p.extend_from_slice(&v.to_le_bytes());
        }
        self.raw_patches.push((off + 12, p));
    }

    /// Move a **baked** free block, by position.
    ///
    /// The mover used to refuse every baked index outright ("baked terrain is
    /// not relocatable"). That is correct for a *cell* move — a baked block's
    /// cell bytes are dead, and baked index N is not unbaked index N — and it
    /// is wrong for a baked FREE block, whose position is six f32 in chunk
    /// `0x0304305F` exactly like an unbaked free block's. Fifteen of the
    /// sixteen pieces of 173691's added finish gate are baked free blocks, and
    /// the blanket refusal is what let a pass move one piece of sixteen and
    /// believe the gate had moved.
    pub fn move_baked_free(&mut self, baked_index: usize, pos: [f32; 3]) {
        let b = self.baked[baked_index].clone();
        let off = b.free_off.unwrap_or_else(|| {
            panic!(
                "b{} {} is a baked GRID block (flags {:08X}): its cell bytes are dead and it has \
                 no stored position, so nothing can move it",
                baked_index, b.name, b.flags
            )
        });
        let mut p = Vec::new();
        for v in pos {
            p.extend_from_slice(&v.to_le_bytes());
        }
        self.raw_patches.push((off, p));
    }

    /// `prs`: move a gate ITEM by position only -- the three f32 of its
    /// absolute position, nothing else. `move_item` also rewrites the yaw and
    /// the declared cell; when all you want is to relocate a trigger onto a
    /// known point of a trajectory, those extra writes are two more things
    /// that can be wrong.
    pub fn move_item_pos(&mut self, item_index: usize, pos: [f32; 3]) {
        let it = self.items[item_index].clone();
        let mut p = Vec::new();
        for v in pos {
            p.extend_from_slice(&v.to_le_bytes());
        }
        self.raw_patches.push((it.pos_off, p));
    }

    /// Relocate a waypoint gate item: position, yaw and the block cell it is
    /// declared to sit in.
    pub fn move_item(&mut self, item_index: usize, pos: [f32; 3], yaw: f32, cell: (i32, i32, i32)) {
        let it = self.items[item_index].clone();
        self.raw_patches.push((it.yaw_off, yaw.to_le_bytes().to_vec()));
        let mut p = Vec::new();
        for v in pos {
            p.extend_from_slice(&v.to_le_bytes());
        }
        self.raw_patches.push((it.pos_off, p));
        self.raw_patches.push((
            it.coord_off,
            vec![cell.0 as u8, cell.1 as u8, cell.2 as u8],
        ));
    }

    /// Build the patched file bytes.
    pub fn build(&self) -> Vec<u8> {
        self.build_reporting().0
    }

    /// Build the patched file bytes and say how the compressed stream was
    /// produced (`splice.rs`). `build` is this without the report.
    pub fn build_reporting(&self) -> (Vec<u8>, crate::splice::Spliced) {
        let body = self.patched_body();
        self.gbx.write_body(&body)
    }

    /// The new DECOMPRESSED body: every raw patch applied, and the two chunks
    /// this tool can re-encode replaced. With no rename in play, `reemit`
    /// reproduces its region byte for byte, so the body differs from the stock
    /// one in exactly the bytes of the edit — which is what lets the writer
    /// splice rather than recompress.
    pub fn patched_body(&self) -> Vec<u8> {
        let mut body = self.gbx.body.clone();
        for (off, bytes) in &self.raw_patches {
            body[*off..*off + bytes.len()].copy_from_slice(bytes);
        }
        let mut bf = self.body_ids.clone();
        let mut itf = self.item_ids.clone();
        for (is_item, field, name) in &self.renames {
            let f = if *is_item { &mut itf[*field] } else { &mut bf[*field] };
            f.name = Some(name.clone());
        }
        // Collect every region's replacement, then splice from the back so
        // earlier regions' offsets stay valid.
        let mut splices: Vec<((usize, usize), Vec<u8>)> = Vec::new();
        let body_new = reemit_regions(&body, &self.body_regions, &bf);
        let mut baked_fix: Option<(usize, usize)> = None;
        for (i, (r, b)) in self.body_regions.iter().zip(body_new).enumerate() {
            // region 0 is the blocks chunk (not skippable, no size field);
            // region 1, when present, is the baked-blocks chunk, which is
            // skippable and therefore carries one.
            if i == 1 {
                if let Some(off) = self.baked_chunk_off {
                    baked_fix = Some((off, b.len()));
                }
            }
            splices.push((*r, b));
        }
        let mut items_fix: Option<(usize, usize)> = None; // (chunk off, new size)
        if let Some(coff) = self.items_chunk_off {
            let new_items = reemit(&body, self.items_region, &itf);
            // chunk 0x03043040 carries TWO sizes that must agree: the
            // skippable chunk size at +8, and its own internal
            // "sizeOfNodeWithClassId" = payload - 12 at payload+8. A stale
            // value presents as the single unhelpful line "Can't load map".
            let payload = coff + 12;
            let head = self.items_region.0 - payload; // = 12 (version, u01, size)
            items_fix = Some((coff, head + new_items.len()));
            splices.push((self.items_region, new_items));
        }
        // Both size fields sit OUTSIDE the spliced region, so patch them into
        // `body` first -- their offsets are pre-splice ones, and applying them
        // afterwards writes 4 bytes into whatever the splices shifted into
        // place instead (which is exactly what "Can't load map" looked like
        // the first time round).
        if let Some((coff, size)) = items_fix {
            let payload = coff + 12;
            body[coff + 8..coff + 12].copy_from_slice(&(size as u32).to_le_bytes());
            body[payload + 8..payload + 12].copy_from_slice(&(size as u32 - 12).to_le_bytes());
        }
        if let Some((coff, size)) = baked_fix {
            body[coff + 8..coff + 12].copy_from_slice(&(size as u32).to_le_bytes());
        }
        splices.sort_by_key(|((s, _), _)| std::cmp::Reverse(*s));
        let mut out = body.clone();
        for ((s, e), b) in splices {
            out.splice(s..e, b);
        }
        out
    }

    pub fn write_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        self.write_to_reporting(path).map(|_| ())
    }

    /// Write, and hand back how the file was produced. The commands a human
    /// drives print it; the loops (`ladder`, `dropscan`) do not, because a
    /// thousand identical lines is not a report.
    pub fn write_to_reporting(
        &self,
        path: &std::path::Path,
    ) -> std::io::Result<crate::splice::Spliced> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let (bytes, sp) = self.build_reporting();
        std::fs::write(path, bytes)?;
        Ok(sp)
    }
}

// ------------------------------------------------------------------ parsing

fn parse_blocks(
    body: &[u8],
    seen_nodes: &mut std::collections::HashSet<u32>,
) -> ((usize, usize), Vec<IdField>, Vec<BlockRec>, Vec<String>) {
    let hits = find_all(body, &BLOCKS_CHUNK.to_le_bytes());
    let start = *hits
        .iter()
        .find(|&&h| plausible_blocks(body, h))
        .unwrap_or_else(|| panic!("no plausible 0x0304301F chunk (hits: {:?})", hits));
    let mut r = Reader::at(body, start + 4);
    // The lookback "id version" word (always 3) is written once per stream,
    // just before the first Id. In every map measured it was already written
    // by an earlier body chunk, so it is absent here -- but handle both.
    if r.peek_u32() == 3 {
        r.u32();
    }
    let mut table: Vec<String> = Vec::new();
    let mut ids: Vec<IdField> = Vec::new();
    let push = |r: &mut Reader, table: &mut Vec<String>, ids: &mut Vec<IdField>| -> usize {
        let f = read_id(r, table);
        ids.push(f);
        ids.len() - 1
    };
    // Ident mapInfo, string mapName, Ident decoration
    push(&mut r, &mut table, &mut ids); // uid
    push(&mut r, &mut table, &mut ids); // collection (raw)
    push(&mut r, &mut table, &mut ids); // author
    let _map_name = r.string();
    push(&mut r, &mut table, &mut ids);
    push(&mut r, &mut table, &mut ids);
    push(&mut r, &mut table, &mut ids);
    let _size = [r.u32(), r.u32(), r.u32()];
    let _need_unlock = r.u32();
    let _version = r.u32();
    let nb = r.u32();

    let mut blocks = Vec::new();
    let mut count = 0u32;
    loop {
        if count >= nb {
            // extra blocks past nbBlocks are listed while the next word has its
            // top bits set (a lookback word always does; a chunk id does not)
            if r.o + 4 > body.len() || (r.peek_u32() & 0xC000_0000) == 0 {
                break;
            }
        }
        let nf = push(&mut r, &mut table, &mut ids);
        let name = ids[nf].name.clone().unwrap_or_default();
        let dir = r.u8();
        let coord_off = r.o;
        let raw_coords = [r.u8(), r.u8(), r.u8()];
        let flags = r.u32();
        if flags == 0xFFFF_FFFF {
            // "unassigned" placeholder: does not count towards nbBlocks
            blocks.push(BlockRec {
                index: blocks.len(),
                name,
                name_field: nf,
                dir,
                raw_coords,
                coord_off,
                flags,
                waypoint_tag: None,
                free_off: None,
                free_pos: None,
                free_rot: None,
            });
            continue;
        }
        if flags & 0x8000 != 0 {
            push(&mut r, &mut table, &mut ids); // author
            read_node_ref(&mut r, seen_nodes); // skin
        }
        let mut tag = None;
        if flags & 0x100000 != 0 {
            tag = read_node_ref(&mut r, seen_nodes);
        }
        blocks.push(BlockRec {
            index: blocks.len(),
            name,
            name_field: nf,
            dir,
            raw_coords,
            coord_off,
            flags,
            waypoint_tag: tag,
            free_off: None,
            free_pos: None,
            free_rot: None,
        });
        count += 1;
    }
    ((start, r.o), ids, blocks, table)
}

/// Chunk 0x03043048 -- the BAKED blocks (the terrain the editor bakes into the
/// map). Structurally the same block records as 0x0304301F, and crucially it
/// CONTINUES the same lookback table: its references index strings defined in
/// the blocks chunk, so both regions have to be re-encoded as one stream.
/// Nothing here is ever edited; it is parsed only so its Id words can be
/// renumbered when the blocks chunk gains or loses a table slot.
fn parse_baked(
    body: &[u8],
    mut table: Vec<String>,
    ids: &mut Vec<IdField>,
    seen_nodes: &mut std::collections::HashSet<u32>,
) -> Option<(usize, usize, usize, Vec<BlockRec>)> {
    let (_, off, payload, size) = *crate::gbx::all_skip_chunks(body)
        .iter()
        .find(|(cid, ..)| *cid == 0x03043048)?;
    let end = payload + size;
    let mut r = Reader::at(body, payload);
    let _version = r.u32();
    let _u01 = r.u32();
    let nb = r.u32();
    let mut count = 0u32;
    let mut n_free = 0usize;
    let mut baked: Vec<BlockRec> = Vec::new();
    while count < nb {
        let nf = {
            ids.push(read_id(&mut r, &mut table));
            ids.len() - 1
        };
        let name = ids[nf].name.clone().unwrap_or_default();
        let dir = r.u8();
        let coord_off = r.o;
        let raw_coords = [r.u8(), r.u8(), r.u8()];
        let flags = r.u32();
        if flags == 0xFFFF_FFFF {
            continue;
        }
        if flags & FREE_BLOCK_FLAG != 0 {
            n_free += 1;
        }
        if flags & 0x8000 != 0 {
            ids.push(read_id(&mut r, &mut table));
            read_node_ref(&mut r, seen_nodes);
        }
        let mut tag = None;
        if flags & 0x100000 != 0 {
            tag = read_node_ref(&mut r, seen_nodes);
        }
        baked.push(BlockRec {
            index: baked.len(),
            name,
            name_field: nf,
            dir,
            raw_coords,
            coord_off,
            flags,
            waypoint_tag: tag,
            free_off: None,
            free_pos: None,
            free_rot: None,
        });
        count += 1;
    }
    // tail: u32, then a count of "baked clips additional data" -- entries there
    // would carry Idents, so refuse rather than silently mis-encode.
    let _u02 = r.u32();
    let nb_clips = r.u32();
    assert_eq!(
        nb_clips, 0,
        "chunk 0x03043048 has {} baked-clip entries; their Idents are not parsed",
        nb_clips
    );
    if std::env::var("TMMAPS_DEBUG").is_ok() {
        eprintln!("    [baked] {} blocks, {} of them FREE", baked.len(), n_free);
    }
    assert_eq!(r.o, end, "baked-blocks parse ended at {} not {}", r.o, end);
    Some((off, payload, end, baked))
}

fn plausible_blocks(body: &[u8], h: usize) -> bool {
    if h + 8 > body.len() {
        return false;
    }
    // the chunk opens with Ident.id, which here is a fresh lookback definition
    u32::from_le_bytes(body[h + 4..h + 8].try_into().unwrap()) == 0x4000_0000
}

/// Body-level node ref: u32 index (-1 = null), and the node is written inline
/// the first time its index appears. Returns the waypoint tag when the node is
/// a CGameWaypointSpecialProperty.
fn read_node_ref(r: &mut Reader, seen: &mut std::collections::HashSet<u32>) -> Option<String> {
    let idx = r.u32();
    if idx == 0xFFFF_FFFF {
        return None;
    }
    let fresh = seen.insert(idx);
    if std::env::var("TMMAPS_DEBUG_NODES").is_ok() {
        eprintln!(
            "    [node] idx={} at {} {}",
            idx,
            r.o - 4,
            if fresh { "INLINE" } else { "backref" }
        );
    }
    if !fresh {
        return None; // already-written node, only the index is stored
    }
    let class = r.u32();
    if class == WAYPOINT_CLASS {
        return read_waypoint_node(r);
    }
    if class == 0x03059000 {
        read_skin_node(r);
        return None;
    }
    // any other inline node would need its own reader
    panic!("unhandled inline node class 0x{:08X} at {}", class, r.o - 4);
}

fn parse_items(
    body: &[u8],
) -> (Option<usize>, (usize, usize), Vec<IdField>, Vec<ItemRec>) {
    let chunks = crate::gbx::all_skip_chunks(body);
    let c = chunks.iter().find(|(cid, ..)| *cid == ITEMS_CHUNK);
    let (coff, payload, size) = match c {
        Some(&(_, off, poff, size)) => (off, poff, size),
        None => return (None, (0, 0), Vec::new(), Vec::new()),
    };
    let end = payload + size;
    let mut r = Reader::at(body, payload);
    let _version = r.u32();
    let _u01 = r.u32();
    let _size_of_node = r.u32(); // = payload size - 12, rewritten on build
    let region_start = r.o;
    let _archive_version = r.u32();
    let nb = r.u32();
    let mut table: Vec<String> = Vec::new();
    let mut ids: Vec<IdField> = Vec::new();
    let mut items = Vec::new();
    for i in 0..nb {
        // each item is a "node with class id": class id, chunks, 0xFACADE01
        let class = r.u32();
        assert_eq!(
            class, ANCHORED_OBJECT_CLASS,
            "item {} is class 0x{:08X}, not CGameCtnAnchoredObject",
            i, class
        );
        let mut rec: Option<ItemRec> = None;
        loop {
            let cid = r.u32();
            if cid == FACADE {
                break;
            }
            if r.b[r.o..r.o + 4] == *b"PIKS" {
                r.skip(4);
                let n = r.u32() as usize;
                r.skip(n);
                continue;
            }
            assert_eq!(cid, 0x03101002, "unexpected item chunk 0x{:08X} at item {} off {}", cid, i, r.o - 4);
            let version = r.u32();
            assert_eq!(version, 8, "unsupported CGameCtnAnchoredObject version");
            // this sub-archive keeps its OWN lookback state: the id version
            // word (3) is re-written here, and "Nadeo" is re-defined even
            // though the blocks chunk already defined it.
            if r.peek_u32() == 3 {
                r.u32();
            }
            ids.push(read_id(&mut r, &mut table)); // itemModel.id
            let model_field = ids.len() - 1;
            let model = ids[model_field].name.clone().unwrap_or_default();
            ids.push(read_id(&mut r, &mut table)); // collection (raw u32)
            ids.push(read_id(&mut r, &mut table)); // author
            let yaw_off = r.o;
            let yaw = r.f32();
            let pitch = r.f32();
            let roll = r.f32();
            let coord_off = r.o;
            let raw_coords = [r.u8(), r.u8(), r.u8()];
            ids.push(read_id(&mut r, &mut table)); // anchorTreeId
            let pos_off = r.o;
            let pos = [r.f32(), r.f32(), r.f32()];
            // waypointSpecialProperty: written with its class id, no index
            let w = r.u32();
            let tag = if w == 0xFFFF_FFFF {
                None
            } else {
                assert_eq!(w, WAYPOINT_CLASS, "unexpected item sub-node 0x{:08X} at item {} off {} model {}", w, i, r.o - 4, model);
                read_waypoint_node(&mut r)
            };
            // v8 tail: u16 flags, Vec3 pivot, f32 scale, [FileRef packDesc if
            // flags & 4], Vec3, Vec3
            let flags = r.u16();
            // The PLACEMENT carries its own pivot and scale, and the pivot is
            // the one that counts: an item model may declare several pivots
            // (`InflatableTubeCurve4` has two) and nothing in the model says
            // which one a given placement used. This does.
            let pivot = [r.f32(), r.f32(), r.f32()];
            let scale = r.f32();
            if flags & 4 != 0 {
                read_file_ref(&mut r);
            }
            r.skip(12 + 12);
            rec = Some(ItemRec {
                index: i as usize,
                model,
                model_field,
                yaw_off,
                coord_off,
                pos_off,
                yaw,
                raw_coords,
                pos,
                pitch,
                roll,
                pivot,
                scale,
                waypoint_tag: tag,
            });
        }
        items.push(rec.expect("item without a 0x03101002 chunk"));
    }
    (Some(coff), (region_start, end), ids, items)
}

/// Re-export of the body chunk scanner, for the `chunks` debug subcommand.
pub fn skip_chunks(body: &[u8]) -> Vec<(u32, usize, usize, usize)> {
    crate::gbx::all_skip_chunks(body)
}

pub const FREE_POS_CHUNK: u32 = 0x0304305F;
/// Bit in a block's flags word marking it a FREE block (position stored as
/// floats in `0x0304305F`, cell bytes dead).
pub const FREE_BLOCK_FLAG: u32 = 0x2000_0000;

/// `prs`, folding the answer-key agent's free-block walk: attach chunk
/// `0x0304305F` entries to the free blocks of BOTH `0x0304301F` and the baked
/// chunk `0x03043048`.
///
/// Layout, MEASURED not assumed: `u32 version`, then **24 bytes per free
/// block** -- `Vec3 position`, `Vec3 pitchYawRoll` -- for every free block of
/// the blocks chunk in block order, then every free block of the BAKED chunk
/// in its order.
///
/// This used to be two facts held separately and neither of them checked: this
/// function asserted the entry COUNT balanced and assumed the ordering, while
/// the answer-key agent's walk consumed the payload in order and had no
/// end assertion at all. **An assertion that only counts cannot fail on a
/// wrong ordering, and a walk with no end check cannot fail at all.** So both
/// halves are now hard here: one walk, in order, over both lists, required to
/// land exactly on the chunk end.
///
/// The ordering itself is confirmed from two directions: structurally, the
/// walk consumes 3 148 of 3 148 payload bytes on 267460 (131 records = 24
/// unbaked + 107 baked); behaviourally, writing the unbaked Goal's entry on
/// 210218 -- where 11 762 of 14 542 free entries are BAKED, so a wrong
/// ordering would land on some other block -- produced 13 predicted gate
/// crossings with 13 hits at max 6 ms error.
fn parse_free_positions(
    body: &[u8],
    blocks: &mut [BlockRec],
    baked: &mut [BlockRec],
    baked_parsed: bool,
) -> Option<usize> {
    let (_, _off, payload, size) = *crate::gbx::all_skip_chunks(body)
        .iter()
        .find(|(cid, ..)| *cid == FREE_POS_CHUNK)?;
    let end = payload + size;
    let n_free_blocks = blocks.iter().filter(|b| b.flags & FREE_BLOCK_FLAG != 0).count();
    let n_free_baked = baked.iter().filter(|b| b.flags & FREE_BLOCK_FLAG != 0).count();
    assert_eq!(
        (size - 4) % 24,
        0,
        "chunk 0x0304305F payload {} is not 4 + 24k; the free-block entry is not 6 f32 on this map",
        size
    );
    let entries = (size - 4) / 24;
    if baked_parsed {
        assert_eq!(
            entries,
            n_free_blocks + n_free_baked,
            "chunk 0x0304305F holds {} entries but the map has {} free blocks + {} free baked \
             blocks; refusing to guess which entry belongs to which block",
            entries,
            n_free_blocks,
            n_free_baked
        );
    } else if entries < n_free_blocks {
        // TMMAPS_NO_BAKED: the baked side is unknown, so neither the total nor
        // the end can be checked -- one more reason not to use that variable.
        panic!(
            "chunk 0x0304305F holds {} entries, fewer than the {} free blocks in 0x0304301F",
            entries, n_free_blocks
        );
    }
    let mut r = Reader::at(body, payload);
    let _version = r.u32();
    let mut rank = 0usize;
    for b in blocks.iter_mut().chain(baked.iter_mut()) {
        if b.flags & FREE_BLOCK_FLAG == 0 {
            continue;
        }
        let off = r.o;
        let pos = [r.f32(), r.f32(), r.f32()];
        let rot = [r.f32(), r.f32(), r.f32()];
        b.free_off = Some(off);
        b.free_pos = Some(pos);
        b.free_rot = Some(rot);
        rank += 1;
    }
    if baked_parsed {
        // THE HARD END CHECK. Without it the walk can be short or long and
        // still look perfectly healthy -- which is exactly how the ordering
        // went unverified in both tools for a night.
        assert_eq!(
            r.o, end,
            "chunk 0x0304305F walk ended at {} not {} after {} records ({} unbaked + {} baked \
             free blocks); the stream order is not unbaked-then-baked on this map",
            r.o, end, rank, n_free_blocks, n_free_baked
        );
    }
    Some(rank)
}

impl MapFile {
    /// `prs`: set a gate ITEM's yaw in place (the f32 at `yaw_off`), without
    /// touching its position or its model. The item-regime twin of
    /// `set_block_dir`.
    pub fn set_item_yaw(&mut self, item_index: usize, yaw: f32) {
        let it = self.items[item_index].clone();
        self.raw_patches.push((it.yaw_off, yaw.to_le_bytes().to_vec()));
    }
}
