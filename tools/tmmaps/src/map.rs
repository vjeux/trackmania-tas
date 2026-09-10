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

/// World y of cell row 0, per collection: the map stores block heights as a
/// cell index and item heights in metres, and the constant joining them is
/// the environment's, not the file's. Stadium (0x1a): -64 — a cell-9 block
/// has its deck at y 10 = 9*8 - 64 + the prefabs' +2 deck offset, flush with
/// the grass (Grass zone at cell 9, plane local +2). It was -62 until Summer
/// 15 (2026-09-07): the "block top at 10" had been read as the cell floor, so
/// every Stadium block item stood 2 m (1 m tiny) above the items placed on
/// it — road signs buried to the rim, pushers squat — and the whole tiny map
/// floated 1 m above the regenerated grass (05 and 10 shipped that way).
/// BlueBay (0x1c): -40, measured on Summer 2026 - 01 (Land at
/// cell 6, the palms standing on it at y 10.0 = 6*8 - 40 + 2 m block top;
/// GateCheckpoint at y 10 on a cell-6 platform).
pub fn ground_y(collection: u32) -> f32 {
    match collection {
        0x1c => -40.0,
        // RedIsland (0x10), measured on Summer 2026 - 02: the Dirt zone plane
        // sits at local +2 in its prefab; authored Dirt at cell 19 carries
        // TreePineBig items at y 34 (152 - 120 + 2), the regenerated Dirt at
        // cell 15 carries Bush items at y 2 (120 - 120 + 2). Lake water
        // (Water at cell 14, surface local +7.5) is at y -0.5.
        0x10 => -120.0,
        // WhiteShore (0x1d), measured on Summer 2026 - 03: two RoadSignC items
        // at y 72.0 and 72.05 carry cells 23 and 24, so the cell 24 floor is
        // exactly 72 (24*8 - 120); trees on DecoPlatformBase at cells 18/19
        // stand at 26/34 (the platform top local +2), the LinkedCheckpoint
        // gates on a DecoWallBasePillar column top (cell 24 floor) at 72.
        0x1d => -120.0,
        // GreenCoast (0xf), measured on Summer 2026 - 04: 2052 vegetation items
        // carry cell 5 and stand at y 2 on the regenerated Grass (cell 5, plane
        // local +2: 40 - 40 + 2); Lake/LakeShore at cell 4, Grass at 6/7/9.
        0xf => -40.0,
        _ => -64.0,
    }
}

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
    /// The cell as the FILE stores it: the game's (and gbx-py's) cell is this
    /// minus (1, 0, 1) — see `coords()`.
    pub file_cell: [u8; 3],
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
            self.file_cell[0] as i32 - 1,
            self.file_cell[1] as i32,
            self.file_cell[2] as i32 - 1,
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
    pub collection_raw: u32,
    /// the collection word (a plain u32 collection number, 4 bytes) — see `set_item_collection`
    pub collection_field: usize,
    pub author: Option<String>,
    pub author_field: usize,
    /// offsets of the mutable fixed-size fields, absolute in the body
    pub yaw_off: usize,
    pub pitch_off: usize,
    pub roll_off: usize,
    pub coord_off: usize,
    pub pos_off: usize,
    pub pivot_off: usize,
    pub yaw: f32,
    pub file_cell: [u8; 3],
    pub pos: [f32; 3],
    pub pitch: f32,
    pub roll: f32,
    /// The point of the model this placement's `pos` names, in model space.
    /// Read from the PLACEMENT, not the model: an item may declare several
    /// pivots and only the placement says which point was used.
    pub pivot: [f32; 3],
    pub scale: f32,
    /// Absolute body offset of the placement scale.
    pub scale_off: usize,
    /// Absolute body range occupied by the waypoint node (a 4-byte null or the
    /// complete inline CGameWaypointSpecialProperty).
    pub waypoint_region: (usize, usize),
    /// Entire CGameCtnAnchoredObject record, from class id through FACADE.
    pub record_region: (usize, usize),
    /// The tag of the placement's waypoint special property (`Spawn`,
    /// `Checkpoint`, `LinkedCheckpoint`, `Goal`), and its ORDER — the number
    /// inside a linked group; carried from the source placement.
    pub waypoint_tag: Option<String>,
    pub waypoint_order: u32,
    /// The v8 flags word (at `waypoint_region.1`): bit 2 = the record carries
    /// external of a variant-list item this placement shows (Summer 11's
    /// `Show` rigs: 4 = RigStraight32m, 23 = Light4Spots, 28 = Fogger16M;
    /// a `PalmForest` placement's variant is its palm species). Read off the
    /// placements with `tmmaps region --items --raw`.
    pub flags: u16,
    /// Absolute body range of the skin `FileRef` (`packDesc`) when flags bit 2
    /// is set: the skin file this placement applies to its model (Summer 15's
    /// lights: `Skins\Stadium\LightColors\WhiteCold.dds`). Decode it with
    /// `header::FileRef::decode`. The model must DECLARE a skin (header chunk
    /// 0x090F4000) for the game to apply it.
    pub skin_region: Option<(usize, usize)>,
}

impl ItemRec {
    /// The placement's skin reference, decoded from `body`.
    pub fn skin(&self, body: &[u8]) -> Option<crate::header::FileRef> {
        let (a, b) = self.skin_region?;
        crate::header::FileRef::decode(&body[a..b]).map(|(f, _)| f)
    }
    /// The placement's variant index (the high byte of `flags`).
    pub fn variant(&self) -> u8 {
        (self.flags >> 8) as u8
    }

    pub fn coords(&self) -> (i32, i32, i32) {
        (
            self.file_cell[0] as i32,
            self.file_cell[1] as i32,
            self.file_cell[2] as i32,
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
    /// Grid-block direction 0..3. `None` for item-carried waypoints.
    pub dir: Option<u8>,
}

impl std::fmt::Display for Waypoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let k = if self.kind == Kind::Block {
            "block"
        } else {
            "item"
        };
        let pos = match self.pos {
            Some(p) => format!("({}, {}, {})", p[0], p[1], p[2]),
            None => "None".to_string(),
        };
        write!(
            f,
            "<{}#{} {} tag={} cell={:?} pos={} dir={:?}>",
            k, self.index, self.name, self.tag, self.coords, pos, self.dir
        )
    }
}

pub struct MapFile {
    pub gbx: Gbx,
    /// Map grid dimensions from the blocks chunk. They are exposed for format
    /// completeness; vertical placement is selected by `decoration_id`, not by
    /// these dimensions (both control maps are 64³ and have different origins).
    pub size: [i32; 3],
    /// Decoration id from the map's Ident. It selects the map-wide vertical
    /// origin; unlike x/z, y cannot be derived from the block cell alone.
    pub decoration_id: String,
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
    /// Absolute body offset of the item archive's `nbItems` word.
    pub items_count_off: Option<usize>,
    /// Absolute body offset of the blocks chunk's `nbBlocks` word.
    pub blocks_count_off: usize,
    /// Absolute body range of the authored block RECORDS (from the first
    /// record's name word to the end of the last record, extras past
    /// `nbBlocks` included). Record i starts at `body_ids[blocks[i].name_field].off`
    /// and ends where record i+1 starts (`block_spans`).
    pub blocks_records: (usize, usize),
    /// Baked chunk: absolute offsets of its `nbBakedBlocks` word and of its
    /// record range (`payload + 12` .. the word after the last record, where
    /// the chunk's `U01` and the baked-clip list begin).
    pub baked_count_off: Option<usize>,
    pub baked_records: Option<(usize, usize)>,
    pub item_ids: Vec<IdField>,
    pub items: Vec<ItemRec>,
    /// pending edits
    pub renames: Vec<(bool, usize, String)>, // (is_item, field index, new name)
    /// (item Id-field index, new raw word) for the collection words the
    /// re-encoder would otherwise copy verbatim (`set_item_collection`)
    pub item_raw_overrides: Vec<(usize, u32)>,
    pub raw_patches: Vec<(usize, Vec<u8>)>,
    /// Variable-length body replacements. Kept separate from fixed patches so
    /// offsets stay in the source body's coordinate system.
    pub raw_splices: Vec<((usize, usize), Vec<u8>)>,
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
        return IdField {
            off,
            len: 4,
            name: None,
            is_def: false,
            raw: w,
            slot: None,
        };
    }
    let idx = w & 0x3FFF_FFFF;
    if idx == 0 {
        let n = r.u32() as usize;
        let s = String::from_utf8_lossy(r.bytes(n)).into_owned();
        let slot = table.len();
        table.push(s.clone());
        IdField {
            off,
            len: 8 + n,
            name: Some(s),
            is_def: true,
            raw: w,
            slot: Some(slot),
        }
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
                        _ => emitted
                            .iter()
                            .position(|t| *t == name)
                            .map(|p| p as u32 + 1),
                    }
                }
                Mode::Fresh => emitted
                    .iter()
                    .position(|t| *t == name)
                    .map(|p| p as u32 + 1),
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
/// The tag and the ORDER of a placement's `CGameWaypointSpecialProperty`. The
/// order is the checkpoint's number in a multi-lap/linked group; our writer
/// used to emit 0 for every placement, which the route finder read as "unset"
/// on 25 maps (2026-09-07).
fn read_waypoint_node(r: &mut Reader) -> (Option<String>, u32) {
    let mut tag = None;
    let mut order = 0u32;
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
                order = r.u32();
            } else {
                order = r.u32();
                r.u32();
            }
            continue;
        }
        panic!(
            "unknown chunk 0x{:08X} in waypoint node at {}",
            cid,
            r.o - 4
        );
    }
    (tag, order)
}

impl MapFile {
    /// The GBX class of a `.Map.Gbx`: `CGameCtnChallenge`.
    pub const CLASS_CHALLENGE: u32 = 0x0304_3000;

    /// `load`, without taking the process down.
    ///
    /// `load` panics on a malformed map, which is the right behaviour for a
    /// one-shot CLI and the wrong behaviour inside a long-haul sweep: one bad
    /// file in a corpus of hundreds would end the run rather than produce a
    /// row saying which file it was. The reader is a deep recursive walk with
    /// panics throughout, so the honest wrapper is `catch_unwind` rather than
    /// a rewrite of every call site.
    pub fn try_load(path: &std::path::Path) -> Result<MapFile, String> {
        let p = path.to_path_buf();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || MapFile::load(&p))).map_err(
            |e| {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "the map reader panicked".to_string());
                format!("{}: {msg}", path.display())
            },
        )
    }

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
        let (blocks_region, mut body_ids, blocks, table, size, decoration_id, blocks_count_off, records_start) =
            parse_blocks(&body, &mut seen_nodes);
        let blocks_records = (records_start, blocks_region.1);
        let mut body_regions = vec![blocks_region];
        let mut baked_chunk_off = None;
        let mut baked_count_off = None;
        let mut baked_records = None;
        let mut baked: Vec<BlockRec> = Vec::new();
        let mut baked_parsed = false;
        if std::env::var("TMMAPS_NO_BAKED").is_err() {
            baked_parsed = true;
            if let Some((off, s, e, bk, recs_end)) = parse_baked(&body, table, &mut body_ids, &mut seen_nodes)
            {
                baked_chunk_off = Some(off);
                body_regions.push((s, e));
                baked = bk;
                baked_count_off = Some(s + 8);
                baked_records = Some((s + 12, recs_end));
            }
        }
        let mut blocks = blocks;
        parse_free_positions(&body, &mut blocks, &mut baked, baked_parsed);
        let (items_chunk_off, items_region, items_count_off, item_ids, items) = parse_items(&body);
        MapFile {
            gbx,
            size,
            decoration_id,
            body_regions,
            body_ids,
            blocks,
            baked,
            items_chunk_off,
            baked_chunk_off,
            items_region,
            items_count_off,
            blocks_count_off,
            blocks_records,
            baked_count_off,
            baked_records,
            item_ids,
            items,
            renames: Vec::new(),
            item_raw_overrides: Vec::new(),
            raw_patches: Vec::new(),
            raw_splices: Vec::new(),
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
                dir: Some(b.dir),
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
                dir: None,
            });
        }
        out
    }

    // ---------------------------------------------------------------- edits
    pub fn set_decoration(&mut self, decoration: &str) {
        self.renames.push((false, 3, decoration.to_string()));
    }

    pub fn set_block_name(&mut self, block_index: usize, name: &str) {
        let f = self.blocks[block_index].name_field;
        self.renames.push((false, f, name.to_string()));
    }

    /// Rename the MAP itself: the name the game shows in the map list, the
    /// editor's title bar and the playground HUD. It is written in three
    /// places, all length-prefixed GBX strings — header chunk 0x03043003
    /// (`CGameCtnChallenge::Common`: uid, author, NAME), the community XML
    /// chunk 0x03043005 (`<ident name="…">`, one string holding the whole
    /// document), and the body's own copy of the same Common chunk — and a
    /// name that reaches only some of them shows the old one somewhere. The
    /// length changes, so the header table is rebuilt and the body edit goes
    /// through the splice path (apply it LAST, after a write+reload, like
    /// every other variable-length edit).
    ///
    /// Rewrite the header's XML chunk (0x03043005) with `f`, rebuilding the
    /// user-data table around the new length. Returns whether `f` changed it.
    pub fn edit_header_xml(&mut self, f: &dyn Fn(&str) -> Option<String>) -> bool {
        let ud = self.gbx.user_data.clone();
        if ud.len() < 4 {
            return false;
        }
        let n = u32::from_le_bytes(ud[0..4].try_into().unwrap()) as usize;
        let mut heads: Vec<(u32, bool, Vec<u8>)> = Vec::new();
        let mut off = 4 + n * 8;
        for i in 0..n {
            let o = 4 + i * 8;
            let id = u32::from_le_bytes(ud[o..o + 4].try_into().unwrap());
            let raw = u32::from_le_bytes(ud[o + 4..o + 8].try_into().unwrap());
            let size = (raw & 0x7fff_ffff) as usize;
            heads.push((id, raw & 0x8000_0000 != 0, ud[off..off + size].to_vec()));
            off += size;
        }
        let mut changed = false;
        for (id, _, data) in heads.iter_mut() {
            if *id != 0x0304_3005 || data.len() < 4 {
                continue;
            }
            let len = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
            if data.len() < 4 + len {
                continue;
            }
            let xml = String::from_utf8_lossy(&data[4..4 + len]).to_string();
            if let Some(xml2) = f(&xml) {
                if xml2 != xml {
                    let mut d = (xml2.len() as u32).to_le_bytes().to_vec();
                    d.extend_from_slice(xml2.as_bytes());
                    *data = d;
                    changed = true;
                }
            }
        }
        if changed {
            let mut out = Vec::new();
            out.extend_from_slice(&(heads.len() as u32).to_le_bytes());
            for (id, heavy, data) in &heads {
                out.extend_from_slice(&id.to_le_bytes());
                out.extend_from_slice(&((data.len() as u32) | if *heavy { 0x8000_0000 } else { 0 }).to_le_bytes());
            }
            for (_, _, data) in &heads {
                out.extend_from_slice(data);
            }
            self.gbx.user_data = out;
        }
        changed
    }

    /// Rewrite one header chunk's raw payload with `f` (the whole chunk body,
    /// size field excluded), rebuilding the user-data table around the new
    /// length. Returns whether `f` changed it.
    pub fn edit_header_chunk(&mut self, chunk_id: u32, f: &dyn Fn(&[u8]) -> Option<Vec<u8>>) -> bool {
        let ud = self.gbx.user_data.clone();
        if ud.len() < 4 {
            return false;
        }
        let n = u32::from_le_bytes(ud[0..4].try_into().unwrap()) as usize;
        let mut heads: Vec<(u32, bool, Vec<u8>)> = Vec::new();
        let mut off = 4 + n * 8;
        for i in 0..n {
            let o = 4 + i * 8;
            let id = u32::from_le_bytes(ud[o..o + 4].try_into().unwrap());
            let raw = u32::from_le_bytes(ud[o + 4..o + 8].try_into().unwrap());
            let size = (raw & 0x7fff_ffff) as usize;
            heads.push((id, raw & 0x8000_0000 != 0, ud[off..off + size].to_vec()));
            off += size;
        }
        let mut changed = false;
        for (id, _, data) in heads.iter_mut() {
            if *id != chunk_id {
                continue;
            }
            if let Some(d2) = f(data) {
                if d2 != *data {
                    *data = d2;
                    changed = true;
                }
            }
        }
        if changed {
            let mut out = Vec::new();
            out.extend_from_slice(&(heads.len() as u32).to_le_bytes());
            for (id, heavy, data) in &heads {
                out.extend_from_slice(&id.to_le_bytes());
                out.extend_from_slice(&((data.len() as u32) | if *heavy { 0x8000_0000 } else { 0 }).to_le_bytes());
            }
            for (_, _, data) in &heads {
                out.extend_from_slice(data);
            }
            self.gbx.user_data = out;
        }
        changed
    }

    /// Rename the decoration (the mood: `48x48Screen155Day` → `…Night`) in
    /// the HEADER's copy of the Common chunk 0x03043003 as well — the body's
    /// copy is `set_decoration`. The header's lookback strings are plain
    /// `u32 (index | 0x40000000)`, `u32 length`, bytes: the old ident is
    /// found by its length-prefixed bytes and re-emitted at the new length.
    pub fn set_header_decoration(&mut self, from: &str, to: &str) -> bool {
        let mut needle = (from.len() as u32).to_le_bytes().to_vec();
        needle.extend_from_slice(from.as_bytes());
        let mut repl = (to.len() as u32).to_le_bytes().to_vec();
        repl.extend_from_slice(to.as_bytes());
        self.edit_header_chunk(0x0304_3003, &|d: &[u8]| {
            let at = d.windows(needle.len()).position(|w| w == needle.as_slice())?;
            let mut out = d[..at].to_vec();
            out.extend_from_slice(&repl);
            out.extend_from_slice(&d[at + needle.len()..]);
            Some(out)
        })
    }

    /// Remove the author's validation ghost — chunk 0x0305B00F of the
    /// original map, replayed over the tiny map as a car driving the
    /// full-size line in the air — and mark the map unvalidated.
    /// Returns the bytes removed (0 = no ghost chunk).
    pub fn strip_validation_ghost(&mut self) -> usize {
        self.strip_validation_ghost_to(GhostForm::Remove)
    }

    /// `strip_validation_ghost` with the chunk's replacement chosen. The
    /// chunk's payload is `u32 version (0)`, `u32 byte length`, then the
    /// CGameCtnGhost node written inline (class id + chunks; no node index) —
    /// a null node is the 4-byte `FFFFFFFF`, the form every unvalidated Nadeo
    /// map carries (Summer 20/21/22/23/25's sources). A REAL ghost defines
    /// lookback Ids (CarSport, Nadeo, the ghost uid, two empty ones) that the
    /// game numbers BEFORE every other Id of the body, so a map written with a
    /// real ghost and one written with the skeleton differ in every later
    /// chunk's raw indices: the player project's `authorghost embed` replaces
    /// a real ghost byte-safely but cannot insert one into a skeleton map
    /// ("Can't load map"). Hence `Dummy`: a real ghost — Summer 2026 - 01's
    /// validation ghost, 13 148 bytes, `assets/dummy-ghost-summer01.bin` —
    /// stands in for the map's own, header validated="0" either way.
    pub fn strip_validation_ghost_to(&mut self, form: GhostForm) -> usize {
        let found = crate::gbx::all_skip_chunks(&self.gbx.body).iter().find(|(c, ..)| *c == 0x0305_B00F).copied();
        let Some((_, off, payload, size)) = found else {
            // No ghost chunk at all (the chunk-REMOVED form). Dummy INSERTS one right after 0x0305B00E --
            // the ghost chunk carries its own lookback context, so the later chunks' Id numbering is untouched
            // (converter, 2026-09-08: the 21 test). The other forms have nothing to do.
            if let GhostForm::Dummy = form {
                if let Some(&(_, _o, p, s)) = crate::gbx::all_skip_chunks(&self.gbx.body).iter().find(|(c, ..)| *c == 0x0305_B00E) {
                    let mut chunk = Vec::with_capacity(12 + DUMMY_GHOST.len());
                    chunk.extend_from_slice(&0x0305_B00Fu32.to_le_bytes());
                    chunk.extend_from_slice(b"PIKS");
                    chunk.extend_from_slice(&(DUMMY_GHOST.len() as u32).to_le_bytes());
                    chunk.extend_from_slice(DUMMY_GHOST);
                    let n = chunk.len();
                    self.raw_splices.push(((p + s, p + s), chunk));
                    return n;
                }
            }
            return 0;
        };
        const SKELETON: [u8; 12] = [0, 0, 0, 0, 4, 0, 0, 0, 0xff, 0xff, 0xff, 0xff];
        let current = &self.gbx.body[payload..payload + size];
        let chunk_with = |data: &[u8]| -> Vec<u8> {
            let mut chunk = Vec::with_capacity(12 + data.len());
            chunk.extend_from_slice(&0x0305_B00Fu32.to_le_bytes());
            chunk.extend_from_slice(b"PIKS");
            chunk.extend_from_slice(&(data.len() as u32).to_le_bytes());
            chunk.extend_from_slice(data);
            chunk
        };
        let changed = match form {
            GhostForm::Remove => {
                self.raw_splices.push(((off, payload + size), Vec::new()));
                true
            }
            GhostForm::Skeleton => {
                if current == SKELETON {
                    false
                } else {
                    self.raw_splices.push(((off, payload + size), chunk_with(&SKELETON)));
                    true
                }
            }
            GhostForm::Dummy => {
                if current == DUMMY_GHOST {
                    false
                } else {
                    self.raw_splices.push(((off, payload + size), chunk_with(DUMMY_GHOST)));
                    true
                }
            }
            GhostForm::Keep => false,
        };
        let unvalidated = self.edit_header_xml(&|xml| if xml.contains("validated=\"1\"") { Some(xml.replace("validated=\"1\"", "validated=\"0\"")) } else { None });
        if !changed && !unvalidated {
            return 0;
        }
        payload + size - off
    }

    /// Returns how many occurrences were rewritten (header, body). Both being
    /// zero means the map does not declare the name this call was given.
    pub fn set_map_name(&mut self, old: &str, new: &str) -> (usize, usize) {
        let pat = gbx_string(old);
        let rep = gbx_string(new);
        // --- header: rewrite the affected chunks, then rebuild the table
        let ud = self.gbx.user_data.clone();
        let n = u32::from_le_bytes(ud[0..4].try_into().unwrap()) as usize;
        let mut heads: Vec<(u32, bool, Vec<u8>)> = Vec::new();
        let mut off = 4 + n * 8;
        for i in 0..n {
            let o = 4 + i * 8;
            let id = u32::from_le_bytes(ud[o..o + 4].try_into().unwrap());
            let raw = u32::from_le_bytes(ud[o + 4..o + 8].try_into().unwrap());
            let size = (raw & 0x7fff_ffff) as usize;
            heads.push((id, raw & 0x8000_0000 != 0, ud[off..off + size].to_vec()));
            off += size;
        }
        let mut header_hits = 0;
        for (id, _, data) in heads.iter_mut() {
            if *id == 0x0304_3005 {
                // the XML chunk is ONE string: patch the text, then its length
                if data.len() >= 4 {
                    let len = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
                    if data.len() >= 4 + len {
                        let xml = String::from_utf8_lossy(&data[4..4 + len]).to_string();
                        let want = format!("name=\"{}\"", esc_xml(old));
                        if xml.contains(&want) {
                            let xml2 = xml.replace(&want, &format!("name=\"{}\"", esc_xml(new)));
                            let mut d = (xml2.len() as u32).to_le_bytes().to_vec();
                            d.extend_from_slice(xml2.as_bytes());
                            d.extend_from_slice(&data[4 + len..]);
                            *data = d;
                            header_hits += 1;
                        }
                    }
                }
                continue;
            }
            let hits = replace_all(data, &pat, &rep);
            header_hits += hits;
        }
        if header_hits > 0 {
            let mut out = Vec::new();
            out.extend_from_slice(&(heads.len() as u32).to_le_bytes());
            for (id, heavy, data) in &heads {
                out.extend_from_slice(&id.to_le_bytes());
                out.extend_from_slice(&((data.len() as u32) | if *heavy { 0x8000_0000 } else { 0 }).to_le_bytes());
            }
            for (_, _, data) in &heads {
                out.extend_from_slice(data);
            }
            self.gbx.user_data = out;
        }
        // --- body: every occurrence of the same string, as a splice each
        let mut body_hits = 0;
        let mut at = 0;
        while let Some(p) = find_sub(&self.gbx.body[at..], &pat) {
            let s = at + p;
            self.raw_splices.push(((s, s + pat.len()), rep.clone()));
            body_hits += 1;
            at = s + pat.len();
        }
        (header_hits, body_hits)
    }

    pub fn set_map_uid(&mut self, uid: &str) {
        let old = self
            .body_ids
            .first()
            .and_then(|f| f.name.clone())
            .expect("map uid Id");
        assert_eq!(
            old.len(),
            uid.len(),
            "replacement map uid must keep the {}-byte length",
            old.len()
        );
        self.renames.push((false, 0, uid.to_string()));
        let mut hits = 0;
        for i in 0..=self.gbx.user_data.len().saturating_sub(old.len()) {
            if &self.gbx.user_data[i..i + old.len()] == old.as_bytes() {
                self.gbx.user_data[i..i + uid.len()].copy_from_slice(uid.as_bytes());
                hits += 1;
            }
        }
        assert!(hits > 0, "map uid was absent from the header chunks");
    }

    pub fn set_item_model_same_len(&mut self, item_index: usize, name: &str) {
        let f = &self.item_ids[self.items[item_index].model_field];
        assert!(
            f.is_def,
            "item#{item_index} model is a lookback reference, not an inline definition"
        );
        let old = f.name.as_deref().expect("item model name");
        assert_eq!(
            old.len(),
            name.len(),
            "item#{item_index} replacement model must keep {} bytes",
            old.len()
        );
        self.raw_patches.push((f.off + 8, name.as_bytes().to_vec()));
    }

    pub fn set_item_model(&mut self, item_index: usize, name: &str) {
        let f = self.items[item_index].model_field;
        self.renames.push((true, f, name.to_string()));
    }

    pub fn set_item_author(&mut self, item_index: usize, author: &str) {
        let f = self.items[item_index].author_field;
        self.renames.push((true, f, author.to_string()));
    }

    /// The placement's collection word. A placement resolves its model by the
    /// FULL ident (name, collection, author): tiny 21's dropped club items
    /// were re-pointed at `AC00000000.Item.Gbx` with their model and author
    /// renamed but their collection left at the source's Stadium (26), while
    /// the manifest listed the item under the map's BlueBay (28) — so the game
    /// found no such item and asked "Missing Items: AC00000000.Item.Gbx …
    /// load anyway?" on EVERY load of 21 (2026-09-08; read as a 1-in-3 loader
    /// flake for a day). A collection number is a plain 4-byte word, so this
    /// is a fixed-size patch; a lookback-string collection is refused.
    pub fn set_item_collection(&mut self, item_index: usize, collection: u32) {
        let f = &self.item_ids[self.items[item_index].collection_field];
        assert!(
            f.len == 4 && f.name.is_none(),
            "item {item_index}: collection is not a plain collection number (len {}, {:?})",
            f.len,
            f.name
        );
        if f.raw != collection {
            // the patch serves the no-rename write; the override serves the
            // re-encoded write (which copies a raw word from the field, not
            // from the patched body)
            self.raw_patches.push((f.off, collection.to_le_bytes().to_vec()));
            self.item_raw_overrides.push((self.items[item_index].collection_field, collection));
        }
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
            (0..=254).contains(&cell.0)
                && (0..=255).contains(&cell.1)
                && (0..=254).contains(&cell.2),
            "cell {:?} out of the one-byte grid range",
            cell
        );
        self.raw_patches.push((
            b.coord_off,
            vec![(cell.0 + 1) as u8, cell.1 as u8, (cell.2 + 1) as u8],
        ));
    }

    /// Move a GENERATED (baked) record to another cell, position-only — the
    /// three cell bytes of its record in 0x03043048 overwritten in place, dir,
    /// flags and model untouched. The "does the game draw THIS record" probe:
    /// a record moved into the open air says whether the record is drawn as
    /// placed (file records drive the picture) or not (the game re-derives
    /// the clips itself, or hides it for a reason intrinsic to the record).
    /// `cell` is in world-grid coordinates; the file stores x and z one higher.
    pub fn move_baked_cell(&mut self, baked_index: usize, cell: (i32, i32, i32)) {
        let b = self.baked.iter().find(|b| b.index == baked_index).cloned().unwrap_or_else(|| panic!("no baked record b{baked_index}"));
        assert!((0..=254).contains(&cell.0) && (0..=255).contains(&cell.1) && (0..=254).contains(&cell.2), "cell {:?} out of the one-byte grid range", cell);
        self.raw_patches.push((b.coord_off, vec![(cell.0 + 1) as u8, cell.1 as u8, (cell.2 + 1) as u8]));
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
            panic!(
                "block#{} {} is a GRID block; use set_block_dir",
                block_index, b.name
            )
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

    /// Write every placement-frame field of an item: yaw/pitch/roll and pivot.
    /// Appended clones inherit their donor's decoration frame; a block-derived
    /// item must be re-based to the block's own frame or terrain arrives tilted.
    pub fn set_item_frame(&mut self, item_index: usize, rot: [f32; 3], pivot: [f32; 3]) {
        let it = self.items[item_index].clone();
        self.raw_patches
            .push((it.yaw_off, rot[0].to_le_bytes().to_vec()));
        self.raw_patches
            .push((it.pitch_off, rot[1].to_le_bytes().to_vec()));
        self.raw_patches
            .push((it.roll_off, rot[2].to_le_bytes().to_vec()));
        let mut p = Vec::new();
        for v in pivot {
            p.extend_from_slice(&v.to_le_bytes());
        }
        self.raw_patches.push((it.pivot_off, p));
    }

    /// Relocate a waypoint gate item: position, yaw and the block cell it is
    /// declared to sit in.
    pub fn move_item(&mut self, item_index: usize, pos: [f32; 3], yaw: f32, cell: (i32, i32, i32)) {
        let it = self.items[item_index].clone();
        self.raw_patches
            .push((it.yaw_off, yaw.to_le_bytes().to_vec()));
        let mut p = Vec::new();
        for v in pos {
            p.extend_from_slice(&v.to_le_bytes());
        }
        self.raw_patches.push((it.pos_off, p));
        self.raw_patches
            .push((it.coord_off, vec![cell.0 as u8, cell.1 as u8, cell.2 as u8]));
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
        // Variable-length edits are deliberately a separate pass. Combining
        // them with Id-table re-encoding would invalidate every saved offset;
        // callers that need both write, reload, then perform this pass.
        if !self.raw_splices.is_empty() {
            assert!(
                self.renames.is_empty(),
                "variable-length body splices must be applied after model renames (write and reload first)"
            );
            let chunks = crate::gbx::all_skip_chunks(&self.gbx.body);
            let mut size_deltas: Vec<(usize, i64, bool)> = Vec::new();
            for cid in [
                ITEMS_CHUNK,
                0x0304_3048,
                0x0304_3054,
                FREE_POS_CHUNK,
                0x0304_3062,
                0x0304_3063,
                0x0304_3065,
                0x0304_3068,
                0x0304_3069,
            ] {
                if let Some(&(_, off, payload, size)) = chunks.iter().find(|(c, ..)| *c == cid) {
                    let delta: i64 = self
                        .raw_splices
                        .iter()
                        .filter(|((s, e), _)| *s >= payload && *e <= payload + size)
                        .map(|((s, e), bytes)| bytes.len() as i64 - (*e - *s) as i64)
                        .sum();
                    if delta != 0 {
                        size_deltas.push((off, delta, cid == ITEMS_CHUNK || cid == 0x0304_3054));
                    }
                }
            }
            for (off, delta, has_inner_size) in size_deltas {
                let old = u32::from_le_bytes(body[off + 8..off + 12].try_into().unwrap());
                let new = (old as i64 + delta) as u32;
                body[off + 8..off + 12].copy_from_slice(&new.to_le_bytes());
                if has_inner_size {
                    let payload = off + 12;
                    let inner =
                        u32::from_le_bytes(body[payload + 8..payload + 12].try_into().unwrap());
                    let new_inner = (inner as i64 + delta) as u32;
                    body[payload + 8..payload + 12].copy_from_slice(&new_inner.to_le_bytes());
                }
            }
            let mut edits = self.raw_splices.clone();
            edits.sort_by_key(|((s, _), _)| std::cmp::Reverse(*s));
            for ((s, e), bytes) in edits {
                body.splice(s..e, bytes);
            }
            return body;
        }
        if self.renames.is_empty() {
            return body;
        }
        let mut bf = self.body_ids.clone();
        let mut itf = self.item_ids.clone();
        for (field, raw) in &self.item_raw_overrides {
            itf[*field].raw = *raw;
        }
        for (is_item, field, name) in &self.renames {
            let f = if *is_item {
                &mut itf[*field]
            } else {
                &mut bf[*field]
            };
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
        // A LENGTH CHANGE WITH NO RENAME IN PLAY IS A WRITER BUG, NOT AN EDIT.
        //
        // Every mover here writes a fixed-size field, so with no rename the
        // re-emitted regions must come back the length they went in. When they
        // do not, this tool's Id-table re-encoder has not reproduced the map,
        // and the file it would write is one whose blocks chunk silently grew —
        // which nothing downstream can see. Found by sweeping 285 maps: one
        // (`route_170035_roseshaft.Map.Gbx`, a derived map with a 268-entry
        // lookback table) comes back 1010 bytes longer with no edit at all.
        // Refuse rather than ship it.
        assert!(
            !self.renames.is_empty() || out.len() == self.gbx.body.len(),
            "this map's body came back {} bytes {} with NO rename asked for — the Id-table \
             re-encoder has not reproduced it, so every edit written here would silently \
             re-serialise the blocks chunk. Refusing to write.",
            (out.len() as i64 - self.gbx.body.len() as i64).abs(),
            if out.len() > self.gbx.body.len() {
                "longer"
            } else {
                "shorter"
            },
        );
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
) -> (
    (usize, usize),
    Vec<IdField>,
    Vec<BlockRec>,
    Vec<String>,
    [i32; 3],
    String,
    usize,
    usize,
) {
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
    push(&mut r, &mut table, &mut ids); // map uid
    push(&mut r, &mut table, &mut ids); // map collection
    push(&mut r, &mut table, &mut ids); // map author
    let _map_name = r.string();
    let decoration_field = push(&mut r, &mut table, &mut ids);
    let decoration_id = ids[decoration_field].name.clone().unwrap_or_default();
    push(&mut r, &mut table, &mut ids); // decoration collection
    push(&mut r, &mut table, &mut ids); // decoration author
    let size = [r.u32() as i32, r.u32() as i32, r.u32() as i32];
    let _need_unlock = r.u32();
    let _version = r.u32();
    let count_off = r.o;
    let nb = r.u32();
    let records_start = r.o;

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
        let file_cell = [r.u8(), r.u8(), r.u8()];
        let flags = r.u32();
        if flags == 0xFFFF_FFFF {
            // "unassigned" placeholder: does not count towards nbBlocks
            blocks.push(BlockRec {
                index: blocks.len(),
                name,
                name_field: nf,
                dir,
                file_cell,
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
            file_cell,
            coord_off,
            flags,
            waypoint_tag: tag,
            free_off: None,
            free_pos: None,
            free_rot: None,
        });
        count += 1;
    }
    ((start, r.o), ids, blocks, table, size, decoration_id, count_off, records_start)
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
) -> Option<(usize, usize, usize, Vec<BlockRec>, usize)> {
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
        let file_cell = [r.u8(), r.u8(), r.u8()];
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
            file_cell,
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
    let records_end = r.o;
    let _u02 = r.u32();
    let nb_clips = r.u32();
    assert_eq!(
        nb_clips, 0,
        "chunk 0x03043048 has {} baked-clip entries; their Idents are not parsed",
        nb_clips
    );
    if std::env::var("TMMAPS_DEBUG").is_ok() {
        eprintln!(
            "    [baked] {} blocks, {} of them FREE",
            baked.len(),
            n_free
        );
    }
    assert_eq!(r.o, end, "baked-blocks parse ended at {} not {}", r.o, end);
    Some((off, payload, end, baked, records_end))
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
        return read_waypoint_node(r).0;
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
) -> (
    Option<usize>,
    (usize, usize),
    Option<usize>,
    Vec<IdField>,
    Vec<ItemRec>,
) {
    let chunks = crate::gbx::all_skip_chunks(body);
    let c = chunks.iter().find(|(cid, ..)| *cid == ITEMS_CHUNK);
    let (coff, payload, size) = match c {
        Some(&(_, off, poff, size)) => (off, poff, size),
        None => return (None, (0, 0), None, Vec::new(), Vec::new()),
    };
    let end = payload + size;
    let mut r = Reader::at(body, payload);
    let _version = r.u32();
    let _u01 = r.u32();
    let _size_of_node = r.u32(); // = payload size - 12, rewritten on build
    let region_start = r.o;
    let _archive_version = r.u32();
    let count_off = r.o;
    let nb = r.u32();
    let mut table: Vec<String> = Vec::new();
    let mut ids: Vec<IdField> = Vec::new();
    let mut items = Vec::new();
    for i in 0..nb {
        let record_start = r.o;
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
            assert_eq!(
                cid,
                0x03101002,
                "unexpected item chunk 0x{:08X} at item {} off {}",
                cid,
                i,
                r.o - 4
            );
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
            let collection_field = ids.len() - 1;
            let collection_raw = ids.last().unwrap().raw;
            ids.push(read_id(&mut r, &mut table)); // author
            let author_field = ids.len() - 1;
            let author = ids[author_field].name.clone();
            let yaw_off = r.o;
            let yaw = r.f32();
            let pitch_off = r.o;
            let pitch = r.f32();
            let roll_off = r.o;
            let roll = r.f32();
            let coord_off = r.o;
            let file_cell = [r.u8(), r.u8(), r.u8()];
            ids.push(read_id(&mut r, &mut table)); // anchorTreeId
            let pos_off = r.o;
            let pos = [r.f32(), r.f32(), r.f32()];
            // waypointSpecialProperty: written with its class id, no index
            let waypoint_start = r.o;
            let w = r.u32();
            let (tag, waypoint_order) = if w == 0xFFFF_FFFF {
                (None, 0)
            } else {
                assert_eq!(
                    w,
                    WAYPOINT_CLASS,
                    "unexpected item sub-node 0x{:08X} at item {} off {} model {}",
                    w,
                    i,
                    r.o - 4,
                    model
                );
                read_waypoint_node(&mut r)
            };
            let waypoint_region = (waypoint_start, r.o);
            // v8 tail: u16 flags, Vec3 pivot, f32 scale, [FileRef packDesc if
            // flags & 4], Vec3, Vec3
            let flags = r.u16();
            // The PLACEMENT carries its own pivot and scale, and the pivot is
            // the one that counts: an item model may declare several pivots
            // (`InflatableTubeCurve4` has two) and nothing in the model says
            // which one a given placement used. This does.
            let pivot_off = r.o;
            let pivot = [r.f32(), r.f32(), r.f32()];
            let scale_off = r.o;
            let scale = r.f32();
            let mut skin_region = None;
            if flags & 4 != 0 {
                let skin_start = r.o;
                read_file_ref(&mut r);
                skin_region = Some((skin_start, r.o));
            }
            r.skip(12 + 12);
            rec = Some(ItemRec {
                index: i as usize,
                model,
                model_field,
                collection_raw,
                collection_field,
                author,
                author_field,
                yaw_off,
                pitch_off,
                roll_off,
                coord_off,
                pos_off,
                pivot_off,
                yaw,
                file_cell,
                pos,
                pitch,
                roll,
                pivot,
                scale,
                scale_off,
                waypoint_region,
                record_region: (record_start, 0),
                waypoint_tag: tag,
                waypoint_order,
                flags,
                skin_region,
            });
        }
        let mut rec = rec.expect("item without a 0x03101002 chunk");
        rec.record_region.1 = r.o;
        items.push(rec);
    }
    (Some(coff), (region_start, end), Some(count_off), ids, items)
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
    let n_free_blocks = blocks
        .iter()
        .filter(|b| b.flags & FREE_BLOCK_FLAG != 0)
        .count();
    let n_free_baked = baked
        .iter()
        .filter(|b| b.flags & FREE_BLOCK_FLAG != 0)
        .count();
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
        self.raw_patches
            .push((it.yaw_off, yaw.to_le_bytes().to_vec()));
    }

    /// The map's per-item ANIMATION PHASE OFFSET byte — chunk 0x03043063
    /// (`CGameCtnAnchoredObject::AnimPhaseOffset`, EPhaseOffset in eighths of
    /// the period: 4 = half). Summer 15's two facing channel pistons carry 0
    /// and 4 — that is why the original's never meet. Must run on a file whose
    /// item array is already its final size (like `set_item_color`).
    pub fn set_item_phase8(&mut self, item_index: usize, phase8: u8) {
        let chunks = crate::gbx::all_skip_chunks(&self.gbx.body);
        let &(_, _, payload, size) = chunks.iter().find(|(c, ..)| *c == 0x0304_3063).expect("phase chunk 0x03043063");
        let off = payload + 4 + item_index;
        assert!(off < payload + size, "item {item_index} past the phase chunk ({} bytes)", size - 4);
        self.raw_patches.push((off, vec![phase8]));
    }

    /// The int of the anchored object's skippable in-record chunk 0x03101005
    /// (`version 1, int, byte`; 4 on every Summer placement). NOT the
    /// animation phase — that is the per-item byte of chunk 0x03043063
    /// (`set_item_phase8`); this word was probed as the phase on 2026-09-08
    /// (0 vs 4 changed nothing) before the real chunk was found. Patched in
    /// place inside the record; false when the record has no such chunk.
    pub fn set_item_record_word5(&mut self, item_index: usize, phase: u32) -> bool {
        let it = self.items[item_index].clone();
        let rec = &self.gbx.body[it.record_region.0..it.record_region.1];
        // chunk id, PIKS, size 9, version 1 — then the int
        const HEAD: [u8; 16] = [0x05, 0x10, 0x10, 0x03, b'P', b'I', b'K', b'S', 9, 0, 0, 0, 1, 0, 0, 0];
        let Some(k) = rec.windows(HEAD.len()).position(|w| w == HEAD) else { return false };
        self.raw_patches.push((it.record_region.0 + k + HEAD.len(), phase.to_le_bytes().to_vec()));
        true
    }

    pub fn append_item_clones(&mut self, total_items: usize) {
        assert!(
            total_items >= self.items.len(),
            "cannot shrink item array from {} to {}",
            self.items.len(),
            total_items
        );
        let add = total_items - self.items.len();
        if add == 0 {
            return;
        }
        let donor = self
            .items
            .iter()
            .find(|it| {
                it.waypoint_tag.is_none()
                    && !self.item_ids[it.model_field].is_def
                    && !self.item_ids[it.author_field].is_def
            })
            .expect(
                "map needs one non-waypoint item whose model and author are lookback references",
            );
        let bytes = self.gbx.body[donor.record_region.0..donor.record_region.1].to_vec();
        let mut inserted = Vec::with_capacity(bytes.len() * add);
        for _ in 0..add {
            inserted.extend_from_slice(&bytes);
        }
        let count_off = self.items_count_off.expect("map has no item count");
        self.raw_patches
            .push((count_off, (total_items as u32).to_le_bytes().to_vec()));
        let insert_off = self
            .items
            .last()
            .expect("map has no item records")
            .record_region
            .1;
        self.raw_splices.push(((insert_off, insert_off), inserted));

        // Chunk 0x03043040 version 8 ends with five int arrays; the final one
        // has exactly one snapped-on index per anchored object.
        let mut r = Reader::at(&self.gbx.body, insert_off);
        let version = u32::from_le_bytes(
            self.gbx.body[self.items_chunk_off.unwrap() + 12..self.items_chunk_off.unwrap() + 16]
                .try_into()
                .unwrap(),
        );
        assert!(
            version == 7 || version == 8,
            "item-array append supports chunk 40 versions 7 and 8 (got {version})"
        );
        if version == 7 {
            // v7 carries an Int2[] `itemsOnItem` before the five int arrays
            // (dropped in v8); nothing to add for new items.
            let n = r.u32() as usize;
            r.skip(n * 8);
        }
        for array_index in 0..5 {
            let count_off = r.o;
            let n = r.u32() as usize;
            if array_index == 4 {
                assert_eq!(n, self.items.len(), "snapped-index count");
                self.raw_patches
                    .push((count_off, (total_items as u32).to_le_bytes().to_vec()));
                self.raw_splices
                    .push(((r.o + n * 4, r.o + n * 4), vec![0xFF; add * 4]));
            }
            r.skip(n * 4);
        }

        let chunks = crate::gbx::all_skip_chunks(&self.gbx.body);
        for cid in [0x0304_3062, 0x0304_3063, 0x0304_3065, 0x0304_3068] {
            let &(_, _, payload, size) = chunks
                .iter()
                .find(|(c, ..)| *c == cid)
                .expect("parallel item chunk");
            let fill = match cid {
                0x0304_3062 | 0x0304_3063 | 0x0304_3065 | 0x0304_3068 => 0u8,
                _ => unreachable!(),
            };
            self.raw_splices
                .push(((payload + size, payload + size), vec![fill; add]));
        }
        // Macroblock references: one i32 per block, then one per item, then a
        // trailing self-sized array of instance flags. Insert new -1 item refs
        // immediately before that final array.
        let &(_, _, p69, _) = chunks
            .iter()
            .find(|(c, ..)| *c == 0x0304_3069)
            .expect("macroblock refs chunk");
        let item_refs_end = p69 + 4 + self.blocks.len() * 4 + self.items.len() * 4;
        self.raw_splices
            .push(((item_refs_end, item_refs_end), vec![0xFF; add * 4]));
    }

    /// The u16 placement flags right after the waypoint node. On Nadeo
    /// vegetation the high byte is the SVariantList variant index; a cloned
    /// record keeps its donor's value, which means nothing on a generated
    /// crystal item and must be cleared.
    pub fn set_item_flags(&mut self, item_index: usize, flags: u16) {
        let it = self.items[item_index].clone();
        self.raw_patches
            .push((it.waypoint_region.1, flags.to_le_bytes().to_vec()));
    }

    /// Clear the variant byte (high byte) of the placement flags and keep the
    /// low bits (bit 2 = the record carries a skin PackDesc, which the record
    /// layout depends on).
    pub fn clear_item_variant(&mut self, item_index: usize) {
        self.set_item_variant(item_index, 0);
    }

    /// Set the variant byte (high byte of the placement flags: which entry of
    /// the item's variant list the placement shows — `ShowLights` 23 =
    /// Light4Spots), keeping the low bits.
    pub fn set_item_variant(&mut self, item_index: usize, variant: u8) {
        let it = self.items[item_index].clone();
        let o = it.waypoint_region.1;
        let flags = u16::from_le_bytes(self.gbx.body[o..o + 2].try_into().unwrap());
        self.raw_patches.push((o, ((flags & 0x00FF) | ((variant as u16) << 8)).to_le_bytes().to_vec()));
    }

    /// Chunk 0x03043062 — one colour byte per unbaked block, then per baked
    /// block, then per item (0 Default, 1 White, 2 Green, 3 Blue, 4 Red,
    /// 5 Black). `None` when the map has no such chunk.
    pub fn colors(&self) -> Option<Colors> {
        let chunks = crate::gbx::all_skip_chunks(&self.gbx.body);
        let &(_, _, payload, size) = chunks.iter().find(|(c, ..)| *c == 0x0304_3062)?;
        Some(Colors { bytes: self.gbx.body[payload + 4..payload + size].to_vec(), n_blocks: self.blocks.len(), n_baked: self.baked.len() })
    }

    /// Set an item's colour byte in chunk 0x03043062 (see [`Self::colors`]).
    /// Must run on a file whose item array is already its final size.
    pub fn set_item_color(&mut self, item_index: usize, color: u8) {
        let chunks = crate::gbx::all_skip_chunks(&self.gbx.body);
        let &(_, _, payload, size) = chunks.iter().find(|(c, ..)| *c == 0x0304_3062).expect("colour chunk 0x03043062");
        let off = payload + 4 + self.blocks.len() + self.baked.len() + item_index;
        assert!(off < payload + size, "item {item_index} past the colour chunk ({} bytes)", size - 4);
        self.raw_patches.push((off, vec![color]));
    }

    /// The placement's LIGHTMAP QUALITY byte (chunk 0x03043068, one byte per
    /// block/baked/item like the colours): the editor's enum Normal 0, High 1,
    /// VeryHigh 2, Highest 3, Lowest 4, VeryLow 5, Low 6 — the texel budget the
    /// game's lightmapper gives the item (the trees quality pass, 2026-09-10).
    pub fn set_item_lightmap_quality(&mut self, item_index: usize, quality: u8) {
        let chunks = crate::gbx::all_skip_chunks(&self.gbx.body);
        let &(_, _, payload, size) = chunks.iter().find(|(c, ..)| *c == 0x0304_3068).expect("lightmap quality chunk 0x03043068");
        let off = payload + 4 + self.blocks.len() + self.baked.len() + item_index;
        assert!(off < payload + size, "item {item_index} past the lightmap quality chunk ({} bytes)", size - 4);
        self.raw_patches.push((off, vec![quality]));
    }

    pub fn set_item_scale(&mut self, item_index: usize, scale: f32) {
        assert!(
            scale.is_finite() && scale > 0.0,
            "item scale must be positive and finite"
        );
        let it = self.items[item_index].clone();
        self.raw_patches
            .push((it.scale_off, scale.to_le_bytes().to_vec()));
    }

    /// Set (or clear) a placement's waypoint special property. `order` is the
    /// number inside a linked checkpoint group — carried from the source
    /// placement; it was hard-coded to 0 until 2026-09-07, which made every
    /// gate of every tiny map read as "unset" to a route reader.
    pub fn set_item_waypoint_tag(&mut self, item_index: usize, tag: Option<&str>) {
        let order = self.items[item_index].waypoint_order;
        self.set_item_waypoint(item_index, tag, order)
    }

    pub fn set_item_waypoint(&mut self, item_index: usize, tag: Option<&str>, order: u32) {
        let it = self.items[item_index].clone();
        let mut bytes = Vec::new();
        match tag {
            None => bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()),
            Some(tag) => {
                bytes.extend_from_slice(&WAYPOINT_CLASS.to_le_bytes());
                bytes.extend_from_slice(&WAYPOINT_CLASS.to_le_bytes());
                bytes.extend_from_slice(&2u32.to_le_bytes());
                bytes.extend_from_slice(&(tag.len() as u32).to_le_bytes());
                bytes.extend_from_slice(tag.as_bytes());
                bytes.extend_from_slice(&order.to_le_bytes());
                bytes.extend_from_slice(&FACADE.to_le_bytes());
            }
        }
        self.raw_splices.push((it.waypoint_region, bytes));
    }

    /// Give a placement a skin: the v8 tail's `packDesc` FileRef (a
    /// `Skins\…` path inside the map archive or the game's skins, plus the
    /// flags bit 2 that announces it), replacing one already there. `None`
    /// removes it. A variable-length edit: write and reload first, like the
    /// waypoint tags. The MODEL must declare a skin folder (header chunk
    /// 0x090F4000) for the game to apply the file.
    pub fn set_item_skin(&mut self, item_index: usize, skin: Option<&crate::header::FileRef>) {
        let it = self.items[item_index].clone();
        let flags_off = it.waypoint_region.1;
        let after_scale = flags_off + 2 + 12 + 4;
        let region = it.skin_region.unwrap_or((after_scale, after_scale));
        let (flags, bytes) = match skin {
            Some(f) => (it.flags | 4, f.encode()),
            None => (it.flags & !4, Vec::new()),
        };
        self.raw_patches.push((flags_off, flags.to_le_bytes().to_vec()));
        self.raw_splices.push((region, bytes));
    }

    /// Empty chunk 0x03043043 (genealogies): the per-cell terrain zone
    /// records the game regenerates Land/Beach/Hill/Cliff blocks from at load
    /// (1656 Land + 852 Beach + ... showed up in the loaded map with every
    /// authored block parked). Payload becomes version 0, buffer length 4,
    /// count 0. Returns the number of zone records dropped.
    pub fn clear_genealogy_file(path: &std::path::Path) -> Result<usize, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let g = Gbx::parse(&bytes);
        let body = g.body.clone();
        let (_, off, payload, size) = *crate::gbx::all_skip_chunks(&body)
            .iter()
            .find(|(cid, ..)| *cid == 0x03043043)
            .ok_or("no genealogy chunk")?;
        let n = u32::from_le_bytes(body[payload + 8..payload + 12].try_into().unwrap()) as usize;
        let mut out = Vec::with_capacity(body.len());
        out.extend_from_slice(&body[..off]);
        out.extend_from_slice(&0x03043043u32.to_le_bytes());
        out.extend_from_slice(&body[off + 4..off + 8]); // PIKS
        out.extend_from_slice(&12u32.to_le_bytes());
        out.extend_from_slice(&body[payload..payload + 4]); // version
        out.extend_from_slice(&4u32.to_le_bytes()); // inner buffer length
        out.extend_from_slice(&0u32.to_le_bytes()); // zero genealogies
        out.extend_from_slice(&body[payload + size..]);
        std::fs::write(path, g.write_body_recompressed(&out)).map_err(|e| e.to_string())?;
        Ok(n)
    }

    /// Remove both editor-password controls. `needUnlock` lives in the block
    /// chunk; `0x03043029` carries the 16-byte password hash plus CRC32.
    pub fn remove_password(&mut self) {
        // Header chunk 0x03043002 (TM2020 version 13): version byte, then
        // NeedUnlock byte. This is the flag the menu checks before body load.
        let ud = &mut self.gbx.user_data;
        let n = u32::from_le_bytes(ud[0..4].try_into().unwrap()) as usize;
        let mut data_off = 4 + n * 8;
        let mut cleared_header = false;
        for i in 0..n {
            let o = 4 + i * 8;
            let id = u32::from_le_bytes(ud[o..o + 4].try_into().unwrap());
            let size =
                (u32::from_le_bytes(ud[o + 4..o + 8].try_into().unwrap()) & 0x7fff_ffff) as usize;
            if id == 0x0304_3002 {
                assert!(size >= 2, "header description chunk is too short");
                assert!(ud[data_off] >= 3, "unsupported header description version");
                ud[data_off + 1] = 0;
                cleared_header = true;
            }
            data_off += size;
        }
        assert!(cleared_header, "map has no header description chunk");

        let start = self.body_regions[0].0;
        let mut r = Reader::at(&self.gbx.body, start + 4);
        if r.peek_u32() == 3 {
            r.u32();
        }
        let mut table = Vec::new();
        for _ in 0..3 {
            read_id(&mut r, &mut table);
        }
        r.string();
        for _ in 0..3 {
            read_id(&mut r, &mut table);
        }
        r.skip(12); // map dimensions
        let need_unlock_off = r.o;
        let need_unlock = r.u32();
        assert!(
            need_unlock <= 1,
            "unexpected needUnlock value {need_unlock}"
        );
        self.raw_patches
            .push((need_unlock_off, 0u32.to_le_bytes().to_vec()));

        if let Some((_, off, payload, size)) = crate::gbx::all_skip_chunks(&self.gbx.body)
            .into_iter()
            .find(|(cid, ..)| *cid == 0x0304_3029)
        {
            assert_eq!(size, 20, "unexpected password chunk size");
            self.raw_splices.push(((off, payload + size), Vec::new()));
        }
    }

    /// Replace only the ZIP of 0x03043054 and keep the manifest bytes as they
    /// are — for a map whose manifest is not the tiny library's ident = author
    /// form (a TMX map with custom BLOCKS). The zip is preceded by its u32 length
    /// and followed by the chunk's 4-byte tail, both kept.
    pub fn replace_embedded_zip_keep_manifest(&mut self, zip: &[u8]) {
        let (_, _, payload, size) = crate::gbx::all_skip_chunks(&self.gbx.body)
            .into_iter()
            .find(|(cid, ..)| *cid == 0x0304_3054)
            .expect("map has no embedded-objects chunk 0x03043054");
        let seg = &self.gbx.body[payload..payload + size];
        let z = seg.windows(4).position(|w| w == b"PK\x03\x04").expect("no zip in the embedded-objects chunk");
        let old_len = u32::from_le_bytes(seg[z - 4..z].try_into().unwrap()) as usize;
        let mut b = Vec::with_capacity(zip.len() + 4);
        b.extend_from_slice(&(zip.len() as u32).to_le_bytes());
        b.extend_from_slice(zip);
        self.raw_splices.push(((payload + z - 4, payload + z + old_len), b));
    }

    /// Replace the embedded-object manifest and ZIP in 0x03043054.
    /// The manifest lists item Idents only; support files (prefabs, materials)
    /// are ordinary ZIP entries and need no manifest row.
    pub fn replace_embedded_objects(&mut self, items: &[(&str, &str)], zip: &[u8]) {
        // The manifest ident must match the placements' (name, collection,
        // author) exactly: a BlueBay map places items in collection 0x1C, a
        // Stadium map in 0x1A. Take it from the map's own items.
        let collection = self.items.first().map(|it| it.collection_raw).unwrap_or(26);
        let (_, _, payload, size) = crate::gbx::all_skip_chunks(&self.gbx.body)
            .into_iter()
            .find(|(cid, ..)| *cid == 0x0304_3054)
            .expect("map has no embedded-objects chunk 0x03043054");
        assert!(
            size >= 24,
            "embedded-objects chunk is shorter than its fixed header"
        );
        let b = embedded_objects_payload(items, zip, collection);
        self.raw_splices.push(((payload + 12, payload + size), b[12..].to_vec()));
    }
}

/// A GBX string as it sits in a file: 4-byte little-endian length, then the
/// bytes. Searching for one of these (rather than the bare text) is what makes
/// a name replacement safe — the text alone also occurs inside the XML chunk
/// and in any string that merely contains it.
fn gbx_string(s: &str) -> Vec<u8> {
    let mut v = (s.len() as u32).to_le_bytes().to_vec();
    v.extend_from_slice(s.as_bytes());
    v
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// Replace every occurrence, in place; returns how many.
fn replace_all(data: &mut Vec<u8>, pat: &[u8], rep: &[u8]) -> usize {
    let mut hits = 0;
    let mut at = 0;
    while let Some(p) = find_sub(&data[at..], pat) {
        let s = at + p;
        data.splice(s..s + pat.len(), rep.iter().copied());
        at = s + rep.len();
        hits += 1;
    }
    hits
}

/// The five predefined XML entities, for a name going into the header's XML
/// chunk. Map names in this project are plain text; this is the guard, not a
/// general escaper.
fn esc_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// The payload of chunk 0x03043054: version 1, a zero, the byte count
/// of what follows, the manifest of (Ident, collection, author), the ZIP with
/// its length, and a trailing zero. `items` are (item ident, author).
pub fn embedded_objects_payload(items: &[(&str, &str)], zip: &[u8], collection: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes()); // byte count of the rest, patched below
    b.extend_from_slice(&(items.len() as u32).to_le_bytes());
    if !items.is_empty() {
        b.extend_from_slice(&3u32.to_le_bytes()); // lookback-id version
        // One lookback string table for the whole manifest: a string is
        // defined (0x40000000 + text) the first time and back-referenced
        // (0x40000000 | 1-based index) after that, as the game writes it.
        let mut table: Vec<String> = Vec::new();
        let mut put = |b: &mut Vec<u8>, s: &str| match table.iter().position(|t| t == s) {
            Some(i) => b.extend_from_slice(&(0x4000_0000u32 | (i as u32 + 1)).to_le_bytes()),
            None => {
                table.push(s.to_string());
                b.extend_from_slice(&0x4000_0000u32.to_le_bytes());
                b.extend_from_slice(&(s.len() as u32).to_le_bytes());
                b.extend_from_slice(s.as_bytes());
            }
        };
        for (name, author) in items.iter() {
            // The Ident is the file name relative to Items/, verbatim: it is
            // what the placements name and what the ZIP entry is called.
            put(&mut b, name);
            b.extend_from_slice(&collection.to_le_bytes());
            put(&mut b, author);
        }
    }
    b.extend_from_slice(&(zip.len() as u32).to_le_bytes());
    b.extend_from_slice(zip);
    b.extend_from_slice(&0u32.to_le_bytes());
    let n = (b.len() - 12) as u32;
    b[8..12].copy_from_slice(&n.to_le_bytes());
    b
}

impl MapFile {
    /// Replace only the ZIP tail. Prefer `replace_embedded_objects` when adding
    /// new item models, because a ZIP without its Ident manifest is invisible.
    pub fn replace_embedded_zip(&mut self, zip: &[u8]) {
        let (_, _, payload, size) = crate::gbx::all_skip_chunks(&self.gbx.body)
            .into_iter()
            .find(|(cid, ..)| *cid == 0x0304_3054)
            .expect("map has no embedded-objects chunk 0x03043054");
        assert!(
            size >= 24,
            "embedded-objects chunk is shorter than its 24-byte prefix"
        );
        self.raw_splices
            .push(((payload + 24, payload + size), zip.to_vec()));
    }
}

/// The per-placement colour bytes of chunk 0x03043062 (see [`MapFile::colors`]).
pub struct Colors {
    pub bytes: Vec<u8>,
    pub n_blocks: usize,
    pub n_baked: usize,
}

impl Colors {
    pub fn block(&self, index: usize) -> u8 {
        self.bytes.get(index).copied().unwrap_or(0)
    }
    pub fn baked(&self, index: usize) -> u8 {
        self.bytes.get(self.n_blocks + index).copied().unwrap_or(0)
    }
    pub fn item(&self, index: usize) -> u8 {
        self.bytes.get(self.n_blocks + self.n_baked + index).copied().unwrap_or(0)
    }
}

/// One decoded `CGameCtnZoneGenealogy` record of chunk 0x03043043.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenealogyRec {
    /// Byte range of the record within the chunk payload.
    pub start: usize,
    pub end: usize,
    /// The zone chain (`ZoneIds`), root first.
    pub ids: Vec<String>,
    pub current_index: u32,
    pub dir: u32,
    /// `CurrentZoneId`: the terrain block the game regenerates for the cell.
    pub current: String,
}

impl GenealogyRec {
    /// `Lake>LakeShore>Grass @1 d2 = LakeShore` — the chain, the current index
    /// and direction, and the current zone; the form the histograms print.
    pub fn describe(&self) -> String {
        format!("{} @{} d{} = {}", self.ids.join(">"), self.current_index, self.dir, self.current)
    }
}

/// The genealogy chunk (0x03043043) decoded: one record per cell in the
/// chunk's own order (64 x 64 = 4096 records, no coordinates stored), each
/// a `CGameCtnZoneGenealogy` node: `count`, [lookback version 3 once],
/// `count` zone Ids, CurrentIndex, Dir, CurrentZoneId, FACADE. Returns
/// (record byte ranges within the chunk payload, current zone names).
pub fn genealogy_records(payload: &[u8]) -> Result<Vec<(usize, usize, String)>, String> {
    Ok(genealogy_full(payload)?.into_iter().map(|r| (r.start, r.end, r.current)).collect())
}

/// Every field of every genealogy record (see `genealogy_records`).
pub fn genealogy_full(payload: &[u8]) -> Result<Vec<GenealogyRec>, String> {
    let count = u32::from_le_bytes(payload[8..12].try_into().unwrap()) as usize;
    let mut table: Vec<String> = Vec::new();
    let mut seen_version = false;
    let mut o = 12usize;
    let mut out = Vec::with_capacity(count);
    let rd = |o: &mut usize| -> u32 { let v = u32::from_le_bytes(payload[*o..*o + 4].try_into().unwrap()); *o += 4; v };
    for _ in 0..count {
        let start = o;
        let class = rd(&mut o);
        let chunk = rd(&mut o);
        if class != 0x0311_D000 || chunk != 0x0311_D002 {
            return Err(format!("genealogy record at {start:#x}: class {class:#010x} chunk {chunk:#010x}"));
        }
        let n = rd(&mut o) as usize;
        let mut id = |o: &mut usize, table: &mut Vec<String>, seen: &mut bool| -> String {
            if !*seen {
                let v = rd(o);
                if v != 3 { return format!("<lookback version {v}>"); }
                *seen = true;
            }
            let v = rd(o);
            if v == 0xFFFF_FFFF { return String::new(); }
            if v & 0x4000_0000 != 0 {
                let idx = (v & 0x3FFF_FFFF) as usize;
                if idx == 0 {
                    let len = rd(o) as usize;
                    let s = String::from_utf8_lossy(&payload[*o..*o + len]).to_string();
                    *o += len;
                    table.push(s.clone());
                    s
                } else {
                    table.get(idx - 1).cloned().unwrap_or_default()
                }
            } else {
                format!("#{v}")
            }
        };
        let mut ids = Vec::with_capacity(n);
        for _ in 0..n {
            ids.push(id(&mut o, &mut table, &mut seen_version));
        }
        let current_index = rd(&mut o);
        let dir = rd(&mut o);
        let cur = id(&mut o, &mut table, &mut seen_version);
        let facade = rd(&mut o);
        if facade != 0xFACA_DE01 {
            return Err(format!("genealogy record at {start:#x}: no terminator ({facade:#010x})"));
        }
        out.push(GenealogyRec { start, end: o, ids, current_index, dir, current: cur });
    }
    Ok(out)
}

impl MapFile {
    /// The current zone of every genealogy record (chunk 0x03043043), in
    /// chunk order — the terrain block names the game regenerates from
    /// (Land, LandHill2, Water, Lake, LakeShore, GrassCliff2…). Empty when the
    /// map has no genealogy chunk.
    pub fn genealogy_zones(&self) -> Vec<String> {
        crate::gbx::all_skip_chunks(&self.gbx.body)
            .iter()
            .find(|(cid, ..)| *cid == 0x0304_3043)
            .and_then(|&(_, _, payload, size)| genealogy_records(&self.gbx.body[payload..payload + size]).ok())
            .map(|recs| recs.into_iter().map(|r| r.2).collect())
            .unwrap_or_default()
    }

    /// The map's most common genealogy zone — the ambient terrain the island
    /// sits in (RedIsland/WhiteShore `Water`, GreenCoast `Lake`), which is
    /// what `fill_genealogy_file` spreads over every cell.
    pub fn ambient_zone(&self) -> Option<String> {
        let mut hist: std::collections::BTreeMap<String, usize> = Default::default();
        for z in self.genealogy_zones() {
            *hist.entry(z).or_default() += 1;
        }
        hist.into_iter().max_by_key(|(_, c)| *c).map(|(z, _)| z)
    }

    /// Fill chunk 0x03043043 with ONE zone everywhere: every cell gets a copy
    /// of the map's first genealogy record (RedIsland: `Water`, the lake the
    /// island sits in — a zone BLOCK there, not decoration like BlueBay's
    /// sea), so the game regenerates the full-size ambient terrain around
    /// and under the tiny map instead of void. The first record carries the
    /// lookback strings; the copies reference them. Refuses unless the first
    /// record's zone is the map's most common one. Returns (zone, count).
    pub fn fill_genealogy_file(path: &std::path::Path) -> Result<(String, usize), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let g = Gbx::parse(&bytes);
        let body = g.body.clone();
        let (_, off, payload, size) = *crate::gbx::all_skip_chunks(&body)
            .iter()
            .find(|(cid, ..)| *cid == 0x03043043)
            .ok_or("no genealogy chunk")?;
        let chunk = &body[payload..payload + size];
        let recs = genealogy_records(chunk)?;
        let n = recs.len();
        let (r0s, r0e, zone) = recs.first().cloned().ok_or("no genealogy records")?;
        let mut hist: std::collections::BTreeMap<&str, usize> = Default::default();
        for (_, _, z) in &recs {
            *hist.entry(z.as_str()).or_default() += 1;
        }
        let top = hist.iter().max_by_key(|(_, c)| **c).map(|(z, _)| z.to_string()).unwrap_or_default();
        if top != zone {
            return Err(format!("first genealogy record is {zone}, the most common zone is {top}: no fill"));
        }
        // record 0 verbatim (defines the lookback strings), then short copies:
        // count, refs 1..=count, CurrentIndex, Dir, ref count+1, FACADE — the
        // form the source's own later records of the same zone take.
        let rec0 = &chunk[r0s..r0e];
        let count = u32::from_le_bytes(chunk[r0s + 8..r0s + 12].try_into().unwrap());
        let mut short = Vec::new();
        short.extend_from_slice(&0x0311_D000u32.to_le_bytes());
        short.extend_from_slice(&0x0311_D002u32.to_le_bytes());
        short.extend_from_slice(&count.to_le_bytes());
        for i in 1..=count {
            short.extend_from_slice(&(0x4000_0000 | i).to_le_bytes());
        }
        // CurrentIndex and Dir of record 0: walk past its ids (lookback version, then `count` new strings)
        let mut o = r0s + 12 + 4;
        for _ in 0..count {
            let marker = u32::from_le_bytes(chunk[o..o + 4].try_into().unwrap());
            o += 4;
            if marker == 0x4000_0000 {
                let len = u32::from_le_bytes(chunk[o..o + 4].try_into().unwrap()) as usize;
                o += 4 + len;
            }
        }
        short.extend_from_slice(&chunk[o..o + 8]);
        short.extend_from_slice(&(0x4000_0000 | (count + 1)).to_le_bytes());
        short.extend_from_slice(&0xFACA_DE01u32.to_le_bytes());
        // check the source's own second record has this exact shape when it is the same zone
        if let Some((s, e, z)) = recs.get(1) {
            if *z == zone && &chunk[*s..*e] != &short[..] {
                return Err(format!("genealogy record 1 ({z}) is not the short form this fill writes: {:02x?} vs {:02x?}", &chunk[*s..*e], short));
            }
        }
        let mut inner = Vec::with_capacity(4 + rec0.len() + short.len() * (n - 1));
        inner.extend_from_slice(&(n as u32).to_le_bytes());
        inner.extend_from_slice(rec0);
        for _ in 1..n {
            inner.extend_from_slice(&short);
        }
        let mut out = Vec::with_capacity(body.len());
        out.extend_from_slice(&body[..off]);
        out.extend_from_slice(&0x03043043u32.to_le_bytes());
        out.extend_from_slice(&body[off + 4..off + 8]); // PIKS
        out.extend_from_slice(&((8 + inner.len()) as u32).to_le_bytes());
        out.extend_from_slice(&chunk[0..4]); // version
        out.extend_from_slice(&(inner.len() as u32).to_le_bytes()); // inner buffer length
        out.extend_from_slice(&inner);
        out.extend_from_slice(&body[payload + size..]);
        std::fs::write(path, g.write_body_recompressed(&out)).map_err(|e| e.to_string())?;
        Ok((zone, n))
    }
}

// ------------------------------------------------------------ block removal

/// Chunk 0x03043040's tail, after the anchored-object records (GBX.NET
/// `Chunk03043040`, versions 7 and 8): the "snapped on" tables. An item
/// placed ON a block or on another item in the editor is deleted with it; the
/// file records that as groups — group k is BLOCK `block_indexes[k]` (an
/// index into the authored blocks list) or, when that is -1, ITEM
/// `item_indexes[k]`; `snap_groups[k]` and `u07[k]` (always -1) ride along;
/// `snapped[i]`, one per item, is the group item i hangs off, or -1.
#[derive(Clone, Debug)]
pub struct SnapTables {
    /// Absolute body range of the five int arrays (the v7 Int2 list before
    /// them is not part of it).
    pub span: (usize, usize),
    pub block_indexes: Vec<i32>,
    pub item_indexes: Vec<i32>,
    pub snap_groups: Vec<i32>,
    pub u07: Vec<i32>,
    pub snapped: Vec<i32>,
}

impl SnapTables {
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::new();
        for arr in [&self.block_indexes, &self.item_indexes, &self.snap_groups, &self.u07, &self.snapped] {
            b.extend_from_slice(&(arr.len() as u32).to_le_bytes());
            for v in arr {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        b
    }
}

/// Chunk 0x03043069: one macroblock-instance index per authored block, then
/// one per item, then the instance (id, flags) pairs.
#[derive(Clone, Debug)]
pub struct MacroblockRefs {
    pub payload: usize,
    pub size: usize,
    pub blocks: Vec<i32>,
    pub items: Vec<i32>,
    /// The trailing `Int2[]` (count + pairs), verbatim.
    pub tail: Vec<u8>,
}

/// One authored/baked block record's layout, walked from its first byte with
/// the same rules as `parse_blocks`. `node_refs` are the skin and waypoint
/// node-ref words: (offset of the index word, node index, the node body's
/// byte range when it is written inline here — from its class id through its
/// FACADE — or None for a back-reference).
struct RecordLayout {
    end: usize,
    node_refs: Vec<(usize, u32, Option<(usize, usize)>)>,
}

fn walk_record(body: &[u8], start: usize, seen: &mut std::collections::HashSet<u32>) -> RecordLayout {
    let mut r = Reader::at(body, start);
    let mut scratch: Vec<String> = Vec::new();
    read_id(&mut r, &mut scratch); // name (its length is all that is needed here)
    r.u8(); // dir
    r.skip(3); // coords
    let flags = r.u32();
    let mut node_refs = Vec::new();
    if flags != 0xFFFF_FFFF {
        if flags & 0x8000 != 0 {
            read_id(&mut r, &mut scratch); // author
            walk_node_ref(&mut r, seen, &mut node_refs);
        }
        if flags & 0x100000 != 0 {
            walk_node_ref(&mut r, seen, &mut node_refs);
        }
    }
    RecordLayout { end: r.o, node_refs }
}

fn walk_node_ref(r: &mut Reader, seen: &mut std::collections::HashSet<u32>, out: &mut Vec<(usize, u32, Option<(usize, usize)>)>) {
    let off = r.o;
    let idx = r.u32();
    if idx == 0xFFFF_FFFF {
        return;
    }
    if !seen.insert(idx) {
        out.push((off, idx, None));
        return;
    }
    let body_start = r.o;
    let class = r.u32();
    match class {
        0x03059000 => read_skin_node(r),
        WAYPOINT_CLASS => {
            read_waypoint_node(r);
        }
        _ => panic!("unhandled inline node class 0x{:08X} at {}", class, r.o - 4),
    }
    out.push((off, idx, Some((body_start, r.o))));
}

/// What `remove_blocks` took out, for the caller's report.
#[derive(Clone, Debug, Default)]
pub struct Removed {
    pub blocks: usize,
    pub baked: usize,
    pub free_entries: usize,
    pub snap_groups: usize,
    pub snapped_items_cleared: usize,
    pub table_before: usize,
    pub table_after: usize,
    /// Shared nodes (skins) whose defining record went and that were written
    /// inline again at their first surviving reference.
    pub reinlined_nodes: usize,
}

impl MapFile {
    /// Byte span of every authored block record, in `blocks` order.
    pub fn block_spans(&self) -> Vec<(usize, usize)> {
        let starts: Vec<usize> = self.blocks.iter().map(|b| self.body_ids[b.name_field].off).collect();
        spans_from_starts(&starts, self.blocks_records.1)
    }

    /// Byte span of every baked block record, in `baked` order.
    pub fn baked_spans(&self) -> Vec<(usize, usize)> {
        let Some((_, end)) = self.baked_records else { return Vec::new() };
        let starts: Vec<usize> = self.baked.iter().map(|b| self.body_ids[b.name_field].off).collect();
        spans_from_starts(&starts, end)
    }

    /// The snapped-on tables at the end of chunk 0x03043040 (None: no items
    /// chunk, or a version without them).
    pub fn snap_tables(&self) -> Option<SnapTables> {
        let coff = self.items_chunk_off?;
        let body = &self.gbx.body;
        let version = u32::from_le_bytes(body[coff + 12..coff + 16].try_into().unwrap());
        if version < 7 {
            return None;
        }
        let mut o = match self.items.last() {
            Some(it) => it.record_region.1,
            None => self.items_count_off? + 4,
        };
        let mut r = Reader::at(body, o);
        if version == 7 {
            let n = r.u32() as usize;
            r.skip(n * 8);
            o = r.o;
        }
        let mut arr = |r: &mut Reader| -> Vec<i32> {
            let n = r.u32() as usize;
            (0..n).map(|_| r.i32()).collect()
        };
        let block_indexes = arr(&mut r);
        let item_indexes = arr(&mut r);
        let snap_groups = arr(&mut r);
        let u07 = arr(&mut r);
        let snapped = arr(&mut r);
        assert_eq!(
            r.o, self.items_region.1,
            "chunk 0x03043040 v{version}: the five snapped-on arrays end at {} not at the chunk end {}",
            r.o, self.items_region.1
        );
        Some(SnapTables { span: (o, r.o), block_indexes, item_indexes, snap_groups, u07, snapped })
    }

    /// Chunk 0x03043069 decoded against the current block and item counts.
    pub fn macroblock_refs(&self) -> Option<MacroblockRefs> {
        let &(_, _, payload, size) = crate::gbx::all_skip_chunks(&self.gbx.body).iter().find(|(c, ..)| *c == 0x0304_3069)?;
        let body = &self.gbx.body;
        let need = 4 + 4 * (self.blocks.len() + self.items.len());
        assert!(size >= need + 4, "chunk 0x03043069 is {size} bytes, shorter than version + {} blocks + {} items + a count", self.blocks.len(), self.items.len());
        let mut r = Reader::at(body, payload + 4);
        let blocks = (0..self.blocks.len()).map(|_| r.i32()).collect();
        let items = (0..self.items.len()).map(|_| r.i32()).collect();
        let tail = body[r.o..payload + size].to_vec();
        Some(MacroblockRefs { payload, size, blocks, items, tail })
    }

    /// DELETE block records — the authored blocks `drop_block` selects and the
    /// baked (generated) blocks `drop_baked` selects — from the file, as a
    /// rewrite of everything that lists blocks:
    ///
    ///   * chunk 0x0304301F: the kept records verbatim, `nbBlocks` fixed;
    ///   * chunk 0x03043048: likewise, `nbBakedBlocks` fixed, chunk size fixed;
    ///   * the lookback table both chunks share is re-encoded first-use-defines
    ///     over the kept records (a dropped record may have carried the only
    ///     definition of a name every later record referenced by index);
    ///   * chunk 0x0304305F: only the kept FREE blocks' six floats;
    ///   * chunks 0x03043062 (colour) and 0x03043068 (lightmap quality): the
    ///     per-block bytes of the dropped records go, the per-item bytes stay;
    ///   * chunk 0x03043069: the per-block macroblock refs go;
    ///   * chunk 0x03043040's snapped-on tables: a group naming a dropped block
    ///     is removed and the items that hung off it are un-snapped; block
    ///     indices are renumbered.
    ///
    /// Node indices: a record's skin/waypoint node is written inline at its
    /// first reference; a kept record that only BACK-references a node some
    /// dropped record carried is a refusal (never seen — every block owns its
    /// nodes — but the check is cheap). Gaps in the node numbering are fine:
    /// the reader fills its node table by index as nodes appear.
    ///
    /// Variable-length: the edit is staged as `raw_splices`, so it cannot be
    /// combined with a rename in one write. Write, reload, then continue —
    /// `blocks`, `baked` and every saved offset describe the OLD file.
    pub fn remove_blocks<F, G>(&mut self, drop_block: F, drop_baked: G) -> Removed
    where
        F: Fn(&BlockRec) -> bool,
        G: Fn(&BlockRec) -> bool,
    {
        assert!(self.renames.is_empty(), "remove_blocks cannot share a write with renames (write and reload first)");
        assert!(self.raw_splices.is_empty(), "remove_blocks wants a fresh load (other variable-length edits are pending)");
        let body = &self.gbx.body;
        let keep_block: Vec<bool> = self.blocks.iter().map(|b| !drop_block(b)).collect();
        let keep_baked: Vec<bool> = self.baked.iter().map(|b| !drop_baked(b)).collect();
        let mut removed = Removed::default();

        // --- the lookback stream: header fields define the first slots
        let mut table: Vec<String> = Vec::new();
        for f in self.body_ids.iter().filter(|f| f.off < self.blocks_records.0) {
            if f.is_def {
                table.push(f.name.clone().unwrap_or_default());
            }
        }
        removed.table_before = self.body_ids.iter().filter(|f| f.is_def).count();
        let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        // Node bodies written inline by DROPPED records, by node index: a kept
        // record that only back-referenced one gets the body re-inlined after
        // its index word (the reader writes a node the first time its index
        // appears). Blocks do share nodes — 267460's signs share one skin.
        let mut dropped_nodes: std::collections::HashMap<u32, Vec<u8>> = std::collections::HashMap::new();
        let mut reinlined = 0usize;
        let rec_fields: Vec<IdField> = self.body_ids.iter().filter(|f| f.off >= self.blocks_records.0).cloned().collect();
        let mut fields = rec_fields.iter().peekable();

        // Re-encode one record: its Id fields through `table`, a dropped
        // shared node re-inlined after the word that back-references it, every
        // other byte verbatim.
        let mut emit = |out: &mut Vec<u8>,
                        (s, e): (usize, usize),
                        lay: &RecordLayout,
                        fields: &mut std::iter::Peekable<std::slice::Iter<IdField>>,
                        table: &mut Vec<String>,
                        dropped: &mut std::collections::HashMap<u32, Vec<u8>>| {
            // (offset after which to insert, bytes) — a back-referenced dropped node
            let mut inserts: Vec<(usize, Vec<u8>)> = Vec::new();
            for (off, idx, inline) in &lay.node_refs {
                if inline.is_none() {
                    if let Some(node) = dropped.remove(idx) {
                        inserts.push((*off + 4, node));
                        reinlined += 1;
                    }
                }
            }
            let mut cur = s;
            let mut copy_to = |out: &mut Vec<u8>, cur: &mut usize, to: usize| {
                // copy body[cur..to], dropping in any pending insert at its point
                while *cur < to {
                    let next = inserts.iter().filter(|(p, _)| *p > *cur && *p <= to).map(|(p, _)| *p).min().unwrap_or(to);
                    out.extend_from_slice(&body[*cur..next]);
                    *cur = next;
                    if let Some(pos) = inserts.iter().position(|(p, _)| *p == next) {
                        let (_, bytes) = inserts.remove(pos);
                        out.extend_from_slice(&bytes);
                    }
                }
            };
            while let Some(f) = fields.peek() {
                if f.off >= e {
                    break;
                }
                let f = fields.next().unwrap();
                copy_to(out, &mut cur, f.off);
                cur = f.off + f.len;
                match &f.name {
                    None => out.extend_from_slice(&f.raw.to_le_bytes()),
                    Some(name) => match table.iter().position(|t| t == name) {
                        Some(i) => out.extend_from_slice(&(0x4000_0000u32 | (i as u32 + 1)).to_le_bytes()),
                        None => {
                            table.push(name.clone());
                            out.extend_from_slice(&0x4000_0000u32.to_le_bytes());
                            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
                            out.extend_from_slice(name.as_bytes());
                        }
                    },
                }
            }
            copy_to(out, &mut cur, e);
            assert!(inserts.is_empty(), "a re-inlined node fell outside its record");
        };
        let skip_fields = |fields: &mut std::iter::Peekable<std::slice::Iter<IdField>>, e: usize| {
            while fields.peek().map(|f| f.off < e).unwrap_or(false) {
                fields.next();
            }
        };
        let note_dropped = |lay: &RecordLayout, dropped: &mut std::collections::HashMap<u32, Vec<u8>>| {
            for (_, idx, inline) in &lay.node_refs {
                if let Some((a, b)) = inline {
                    dropped.insert(*idx, body[*a..*b].to_vec());
                }
            }
        };

        let mut new_blocks = Vec::new();
        let mut kept_blocks = 0u32;
        for (i, span) in self.block_spans().into_iter().enumerate() {
            let lay = walk_record(body, span.0, &mut seen);
            assert_eq!(lay.end, span.1, "block record {i} walks to {} but its span ends at {}", lay.end, span.1);
            if keep_block[i] {
                emit(&mut new_blocks, span, &lay, &mut fields, &mut table, &mut dropped_nodes);
                if self.blocks[i].flags != 0xFFFF_FFFF {
                    kept_blocks += 1;
                }
            } else {
                note_dropped(&lay, &mut dropped_nodes);
                skip_fields(&mut fields, span.1);
                removed.blocks += 1;
            }
        }
        let mut new_baked = Vec::new();
        let mut kept_baked = 0u32;
        if let Some((rs, _)) = self.baked_records {
            // the fields between the two regions (none expected) stay as they are
            skip_fields(&mut fields, rs);
            for (i, span) in self.baked_spans().into_iter().enumerate() {
                let lay = walk_record(body, span.0, &mut seen);
                assert_eq!(lay.end, span.1, "baked record {i} walks to {} but its span ends at {}", lay.end, span.1);
                if keep_baked[i] {
                    emit(&mut new_baked, span, &lay, &mut fields, &mut table, &mut dropped_nodes);
                    if self.baked[i].flags != 0xFFFF_FFFF {
                        kept_baked += 1;
                    }
                } else {
                    note_dropped(&lay, &mut dropped_nodes);
                    skip_fields(&mut fields, span.1);
                    removed.baked += 1;
                }
            }
        }
        removed.table_after = table.len();
        removed.reinlined_nodes = reinlined;

        let mut patches: Vec<(usize, Vec<u8>)> = Vec::new();
        let mut splices: Vec<((usize, usize), Vec<u8>)> = Vec::new();
        patches.push((self.blocks_count_off, kept_blocks.to_le_bytes().to_vec()));
        splices.push((self.blocks_records, new_blocks));
        if let (Some(coff), Some(recs)) = (self.baked_count_off, self.baked_records) {
            patches.push((coff, kept_baked.to_le_bytes().to_vec()));
            splices.push((recs, new_baked));
        }

        // --- 0x0304305F: the kept free blocks' entries, blocks then baked
        let chunks = crate::gbx::all_skip_chunks(body);
        if let Some(&(_, _, payload, size)) = chunks.iter().find(|(c, ..)| *c == FREE_POS_CHUNK) {
            let mut entries = Vec::new();
            for (b, keep) in self.blocks.iter().zip(&keep_block).chain(self.baked.iter().zip(&keep_baked)) {
                let Some(off) = b.free_off else { continue };
                if *keep {
                    entries.extend_from_slice(&body[off..off + 24]);
                } else {
                    removed.free_entries += 1;
                }
            }
            splices.push(((payload + 4, payload + size), entries));
        }

        // --- per-block bytes: colours (0x62) and lightmap quality (0x68)
        let nb = self.blocks.len();
        let nk = self.baked.len();
        let ni = self.items.len();
        for cid in [0x0304_3062u32, 0x0304_3068] {
            let Some(&(_, _, payload, size)) = chunks.iter().find(|(c, ..)| *c == cid) else { continue };
            assert_eq!(size, 4 + nb + nk + ni, "chunk {cid:#010x} has {size} bytes, not 4 + {nb} blocks + {nk} baked + {ni} items");
            let mut kept = Vec::with_capacity(nb + nk);
            for (j, keep) in keep_block.iter().chain(keep_baked.iter()).enumerate() {
                if *keep {
                    kept.push(body[payload + 4 + j]);
                }
            }
            splices.push(((payload + 4, payload + 4 + nb + nk), kept));
        }

        // --- 0x03043069: the per-block macroblock refs
        if let Some(mb) = self.macroblock_refs() {
            let mut kept = Vec::with_capacity(nb * 4);
            for (v, keep) in mb.blocks.iter().zip(&keep_block) {
                if *keep {
                    kept.extend_from_slice(&v.to_le_bytes());
                }
            }
            splices.push(((mb.payload + 4, mb.payload + 4 + 4 * nb), kept));
        }

        // --- 0x03043040: snapped-on groups naming a dropped block go
        if let Some(st) = self.snap_tables() {
            let g = st.block_indexes.len();
            assert!(
                st.item_indexes.len() == g && st.snap_groups.len() == g && st.u07.len() == g,
                "snapped-on tables disagree on the group count: {} blocks, {} items, {} groups, {} u07",
                g, st.item_indexes.len(), st.snap_groups.len(), st.u07.len()
            );
            assert_eq!(st.snapped.len(), ni, "snapped-on table has {} entries for {ni} items", st.snapped.len());
            let mut new_index: Vec<i32> = Vec::with_capacity(nb);
            let mut next = 0i32;
            for keep in &keep_block {
                new_index.push(if *keep { next } else { -1 });
                if *keep {
                    next += 1;
                }
            }
            let mut group_map: Vec<i32> = vec![-1; g];
            let mut out = SnapTables { span: st.span, block_indexes: vec![], item_indexes: vec![], snap_groups: vec![], u07: vec![], snapped: vec![] };
            for k in 0..g {
                // The block word is (u8 tag, u24 block index): Summer 02 snaps
                // its lake-shore vegetation on Water / WaterHill ZONE blocks
                // with tag 0xFF (`0xff000ce1` = block 3297); every other
                // group seen has tag 0. -1 alone means "an item, see
                // item_indexes".
                let bi = st.block_indexes[k];
                let new_bi = if bi == -1 {
                    -1
                } else {
                    let word = bi as u32;
                    let old = (word & 0x00FF_FFFF) as usize;
                    assert!(old < nb, "snapped-on group {k} names block {old} (word {word:#010x}), past the {nb} blocks");
                    match new_index[old] {
                        -1 => -1,
                        n => ((word & 0xFF00_0000) | n as u32) as i32,
                    }
                };
                if bi != -1 && new_bi == -1 {
                    removed.snap_groups += 1;
                    continue;
                }
                group_map[k] = out.block_indexes.len() as i32;
                out.block_indexes.push(new_bi);
                out.item_indexes.push(st.item_indexes[k]);
                out.snap_groups.push(st.snap_groups[k]);
                out.u07.push(st.u07[k]);
            }
            for &s in &st.snapped {
                if s < 0 {
                    out.snapped.push(-1);
                } else {
                    assert!((s as usize) < g, "an item is snapped on group {s}, past the {g} groups");
                    let m = group_map[s as usize];
                    if m < 0 {
                        removed.snapped_items_cleared += 1;
                    }
                    out.snapped.push(m);
                }
            }
            splices.push((st.span, out.encode()));
        }

        self.raw_patches.extend(patches);
        self.raw_splices.extend(splices);
        removed
    }
}

fn spans_from_starts(starts: &[usize], end: usize) -> Vec<(usize, usize)> {
    starts
        .iter()
        .enumerate()
        .map(|(i, &s)| (s, starts.get(i + 1).copied().unwrap_or(end)))
        .collect()
}

/// Chunk 0x0304305D decoded (2026-09-07, read off Summer 03/08/15; GBX.NET
/// lists it as "ignore"): `version 1`, a tree count (0 or 1), then per tree
/// `grid size` (a power of two: 32, 64, 128), an `Int3`, a node count and the
/// nodes in index order. An INTERNAL node is `i32 parent` + 8 child node
/// indices (-1 = none); a node at the leaf level — the level is the node's
/// depth, the size halving from `grid` down to 1 — is `i32 parent` + one
/// `u8` flag (1, 3, 7 seen). Every index is a NODE index inside the chunk:
/// the tree names no block or item, so block deletion leaves it valid (and
/// the cells it flags are in the same place). Returns
/// (grid, origin, node count, leaf flag histogram) or the parse error.
pub fn octree_chunk_summary(payload: &[u8]) -> Result<Option<(u32, [i32; 3], usize, std::collections::BTreeMap<u8, usize>)>, String> {
    let mut r = Reader::new(payload);
    let version = r.u32();
    if version != 1 {
        return Err(format!("version {version}"));
    }
    let trees = r.u32();
    if trees == 0 {
        if r.o != payload.len() {
            return Err(format!("{} bytes after an empty tree list", payload.len() - r.o));
        }
        return Ok(None);
    }
    if trees != 1 {
        return Err(format!("{trees} trees"));
    }
    let grid = r.u32();
    if !grid.is_power_of_two() {
        return Err(format!("grid {grid} is not a power of two"));
    }
    let origin = [r.i32(), r.i32(), r.i32()];
    let n = r.u32() as usize;
    let leaf_depth = grid.trailing_zeros() as usize; // grid 32 -> internal levels 32,16,8,4,2 -> leaves at depth 5
    // depth of node i = depth of its parent + 1; the root (node 0) is depth 0
    let mut depth: Vec<usize> = vec![usize::MAX; n];
    let mut hist = std::collections::BTreeMap::new();
    for i in 0..n {
        if r.o + 4 > payload.len() {
            return Err(format!("node {i} of {n}: chunk ends early"));
        }
        let parent = r.i32();
        let d = if i == 0 {
            if parent != -1 {
                return Err(format!("root parent {parent}"));
            }
            0
        } else {
            if parent < 0 || parent as usize >= i {
                return Err(format!("node {i}: parent {parent} is not an earlier node"));
            }
            depth[parent as usize] + 1
        };
        depth[i] = d;
        if d < leaf_depth {
            for _ in 0..8 {
                let c = r.i32();
                if c != -1 && (c < 0 || c as usize >= n) {
                    return Err(format!("node {i}: child {c} outside the {n} nodes"));
                }
            }
        } else if d == leaf_depth {
            *hist.entry(r.u8()).or_insert(0) += 1;
        } else {
            return Err(format!("node {i} at depth {d} below the leaf level {leaf_depth}"));
        }
    }
    if r.o != payload.len() {
        return Err(format!("{} bytes left after {n} nodes", payload.len() - r.o));
    }
    Ok(Some((grid, origin, n, hist)))
}

impl MapFile {
    /// Drop the stored lightmap: chunk 0x0304305B becomes `version,
    /// HasLightmaps = 0, U01, U02` (16 bytes, the form of a map whose shadows
    /// were never computed). A tiny map's lightmap would have to be computed
    /// for the TINY layout; the source's (computed for the full-size map, ~1 MB
    /// of WEBP atlases + the CHmsLightMapCache) is stale data the client may
    /// or may not apply — the editor applied it to a block-deleted Summer 05
    /// (shadows of the full-size structures on the deck) and discarded it on
    /// the parked build ("NOT VALIDATED"). Returns the bytes removed; a map
    /// without the chunk, or already without lightmaps, is left alone (0).
    pub fn strip_lightmap(&mut self) -> usize {
        let Some(&(_, off, payload, size)) = crate::gbx::all_skip_chunks(&self.gbx.body).iter().find(|(c, ..)| *c == 0x0304_305B) else { return 0 };
        let body = &self.gbx.body;
        if size < 16 || u32::from_le_bytes(body[payload + 4..payload + 8].try_into().unwrap()) == 0 {
            return 0;
        }
        let mut head = body[payload..payload + 16].to_vec();
        head[4..8].copy_from_slice(&0u32.to_le_bytes()); // HasLightmaps = false
        // the chunk's own size field, then the payload (size deltas in
        // `patched_body` only cover the item-side chunks, so write it here)
        self.raw_patches.push((off + 8, 16u32.to_le_bytes().to_vec()));
        self.raw_splices.push(((payload, payload + size), head));
        size - 16
    }
}

/// What `MapFile::strip_validation_ghost_to` leaves in chunk 0x0305B00F.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GhostForm {
    /// A real ghost (Summer 2026 - 01's), the same on every map.
    Dummy,
    /// The game's own 12-byte empty node.
    Skeleton,
    /// No chunk at all.
    Remove,
    /// The map's own chunk untouched (only the header is unvalidated).
    Keep,
}

/// Summer 2026 - 01's validation ghost, the whole 0x0305B00F payload (13 148
/// bytes): `tmmaps chunks 01-Summer-2026---01.Map.Gbx --only 0x0305B00F --hex 13148`.
pub const DUMMY_GHOST: &[u8] = include_bytes!("../assets/dummy-ghost-summer01.bin");
