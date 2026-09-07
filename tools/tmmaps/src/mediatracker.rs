//! The map's MediaTracker: chunk `0x03043049` of `CGameCtnChallenge`.
//!
//! ```text
//! 0x03043049 (NOT skippable: no PIKS header, so `tmmaps chunks` never lists
//!             it; it follows the baked-blocks chunk 0x03043048)
//!   u32 version (2 on the Summer 2026 maps)
//!   noderef ClipIntro          CGameCtnMediaClip      0x03079000
//!   noderef ClipPodium         CGameCtnMediaClip
//!   noderef ClipGroupInGame    CGameCtnMediaClipGroup 0x0307A000
//!   noderef ClipGroupEndRace   CGameCtnMediaClipGroup
//!   noderef ClipAmbiance       CGameCtnMediaClip      (version >= 2)
//!   Int3 trigger size          (version >= 1; the in-game trigger grid,
//!                               cells per block: 3,1,3)
//! ```
//!
//! A node ref is `u32 index`: `0xFFFFFFFF` = null; an index seen before =
//! back-reference (nothing follows); a NEW index is followed by `u32 class`
//! and the node's chunks, each `u32 chunk id` + payload, ended by
//! `0xFACADE01`. Node indices are assigned in write order, so a new node's
//! index is the next free one — the walker leans on that to find the end of a
//! block whose layout it does not know (an "opaque" block: kept verbatim).
//!
//! Classes read in full (their keys carry WORLD coordinates the tiny transform
//! must move): CameraCustom 0x030A2000 (chunk 006), CameraPath 0x030A1000
//! (chunk 003), CameraOrbital 0x030A0000 (chunk 001), Triangles3D 0x0304C000
//! (chunk 0x03029001, world-space vertices). Every other block (Text, Image,
//! Triangles2D, Fx*, Sound, Time, ...) is an opaque byte span, listed in the
//! report and copied unchanged.
//!
//! The clip group's triggers are lists of Int3 cells in the trigger grid
//! (block cell × trigger size), one trigger per clip; they are re-emitted, so
//! the chunk rewrite is variable-length (`MapFile::set_mediatracker` splices
//! it; the chunk has no size field to fix).

use crate::gbx::Reader;

pub const CHUNK_MEDIATRACKER: u32 = 0x0304_3049;
pub const CLASS_CLIP: u32 = 0x0307_9000;
pub const CLASS_TRACK: u32 = 0x0307_8000;
pub const CLASS_GROUP: u32 = 0x0307_A000;
pub const CLASS_CAMERA_ORBITAL: u32 = 0x030A_0000;
pub const CLASS_CAMERA_PATH: u32 = 0x030A_1000;
pub const CLASS_CAMERA_CUSTOM: u32 = 0x030A_2000;
pub const CLASS_TRIANGLES_2D: u32 = 0x0304_B000;
pub const CLASS_TRIANGLES_3D: u32 = 0x0304_C000;
pub const CLASS_FOG: u32 = 0x0319_9000;
const NODE_END: u32 = 0xFACA_DE01;
const NULL_REF: u32 = 0xFFFF_FFFF;

/// A human name for the media block classes this tool has met (GBX.NET names).
pub fn class_name(class: u32) -> &'static str {
    match class {
        CLASS_CLIP => "CGameCtnMediaClip",
        CLASS_TRACK => "CGameCtnMediaTrack",
        CLASS_GROUP => "CGameCtnMediaClipGroup",
        CLASS_CAMERA_ORBITAL => "CameraOrbital",
        CLASS_CAMERA_PATH => "CameraPath",
        CLASS_CAMERA_CUSTOM => "CameraCustom",
        0x030A_4000 => "CameraEffectShake",
        0x030A_5000 => "Image",
        0x030A_6000 => "MusicEffect",
        0x030A_7000 => "Sound",
        0x030A_8000 => "Text",
        0x030A_9000 => "Trails",
        0x030A_B000 => "TransitionFade",
        0x0304_B000 => "Triangles2D",
        0x0304_C000 => "Triangles3D",
        0x0308_0000 => "FxColors",
        0x0308_1000 => "FxBlurDepth",
        0x0308_2000 => "FxBlurMotion",
        0x0308_3000 => "FxBloom",
        0x0308_4000 => "CameraGame",
        0x0308_5000 => "Time",
        0x0312_6000 => "DOF",
        0x0312_7000 => "ToneMapping",
        0x0312_8000 => "BloomHdr",
        0x0316_5000 => "DirtyLens",
        0x0318_6000 => "ColorGrading",
        0x0319_5000 => "Interface",
        CLASS_FOG => "Fog",
        0x0329_F000 => "Entity",
        _ => "?",
    }
}

fn plausible_class(c: u32) -> bool {
    (c & 0xFFF) == 0 && (c >> 24) == 0x03 && c != 0
}

#[derive(Clone, Debug)]
pub struct CameraState {
    pub position: [f32; 3],
    pub pitch_yaw_roll: [f32; 3],
    pub fov: f32,
    pub target_position: [f32; 3],
    /// the two words after the target (near-Z-like 0.05 and 1.0 on every key measured)
    pub u01: f32,
    pub u02: f32,
}

#[derive(Clone, Debug)]
pub struct CustomKey {
    pub off: usize, // body offset of the time word
    pub time: f32,
    pub interpolation: i32,
    pub anchor_rot: i32,
    pub anchor: i32,
    pub anchor_vis: i32,
    pub target: i32,
    pub state: CameraState,
    /// left/right tangents: the same 12-word shape as `state`, VECTORS (scaled, not moved)
    pub tangents: [CameraState; 2],
}

#[derive(Clone, Debug)]
pub struct PathKey {
    pub off: usize,
    pub time: f32,
    pub position: [f32; 3],
    pub pitch_yaw_roll: [f32; 3],
    pub fov: f32,
    pub near_z: Option<f32>,
    pub anchor_rot: i32,
    pub anchor: i32,
    pub anchor_vis: i32,
    pub target: i32,
    pub target_position: [f32; 3],
    pub weight: f32,
    /// words after the weight (version-dependent), kept verbatim
    pub tail: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct OrbitalKey {
    pub off: usize,
    pub time: f32,
    /// the words after the time, raw: the layout is documented in `report`
    pub words: Vec<u32>,
}

#[derive(Clone, Debug)]
pub enum Kind {
    CameraCustom { version: u32, keys: Vec<CustomKey> },
    CameraPath { version: u32, keys: Vec<PathKey> },
    CameraOrbital { version: u32, keys: Vec<OrbitalKey> },
    /// fog keys: (body offset of the distance word, distance in metres)
    Fog { version: u32, distances: Vec<(usize, f32)> },
    /// world-space triangles: `positions[key][vertex]`, each with its body offset
    Triangles3D { times: Vec<f32>, positions: Vec<Vec<(usize, [f32; 3])>> },
    /// a block whose chunk layout this tool does not know (or whose layout
    /// did not match the expectation: `note` says which); bytes kept verbatim
    Opaque { note: String },
    /// a back-reference to a node already written (index only)
    BackRef,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub index: u32,
    pub class: u32,
    /// body span: the index word .. past the node end marker
    pub span: (usize, usize),
    pub kind: Kind,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub index: u32,
    pub name: String,
    pub blocks: Vec<Block>,
    pub span: (usize, usize),
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub index: u32,
    pub name: String,
    pub tracks: Vec<Track>,
    pub span: (usize, usize),
}

#[derive(Clone, Debug)]
pub struct Trigger {
    pub u01: i32,
    pub u02: i32,
    pub u03: i32,
    pub u04: i32,
    pub condition: i32,
    pub condition_value: f32,
    pub coords: Vec<[i32; 3]>,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub index: u32,
    pub list_version: u32,
    pub clips: Vec<Clip>,
    pub triggers: Vec<Trigger>,
    /// body span of the whole node (index word .. past the node end)
    pub span: (usize, usize),
    /// bytes after the trigger list up to the node end (further chunks, the end marker)
    pub tail: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum Slot {
    Null,
    Clip(Clip),
    Group(Group),
}

#[derive(Clone, Debug)]
pub struct MediaTracker {
    /// body offsets: the chunk id word .. the next chunk's header
    pub start: usize,
    pub end: usize,
    pub version: u32,
    pub intro: Slot,
    pub podium: Slot,
    pub in_game: Slot,
    pub end_race: Slot,
    pub ambiance: Slot,
    pub trigger_size: Option<[i32; 3]>,
    /// fixed-size edits to apply on emit: (absolute body offset, 4 bytes)
    pub edits: Vec<(usize, [u8; 4])>,
    /// when set, every clip slot is emitted null
    pub strip: bool,
}

struct Walker<'a> {
    body: &'a [u8],
    end: usize,
    /// the next node index a NEW node is expected to carry
    next_index: u32,
    seen: std::collections::HashSet<u32>,
    notes: Vec<String>,
}

type R<T> = Result<T, String>;

impl<'a> Walker<'a> {
    fn u32_at(&self, o: usize) -> Option<u32> {
        if o + 4 <= self.end {
            Some(u32::from_le_bytes(self.body[o..o + 4].try_into().unwrap()))
        } else {
            None
        }
    }

    fn take_index(&mut self, r: &mut Reader) -> R<Option<(u32, bool)>> {
        // -> None for a null ref; Some((index, is_new))
        let idx = r.u32();
        if idx == NULL_REF {
            return Ok(None);
        }
        if self.seen.contains(&idx) {
            return Ok(Some((idx, false)));
        }
        if idx < self.next_index {
            // an index defined before the MediaTracker (a waypoint node, a skin):
            // a back-reference into the rest of the body
            return Ok(Some((idx, false)));
        }
        if idx > self.next_index {
            // nodes are defined in index order, so a gap means nodes this
            // walker did not see: the ones nested inside an opaque block that
            // closed a list (a Text block's effect node). The caller checks
            // the class word that follows, which is what catches a lost sync.
            if idx - self.next_index > 64 {
                return Err(format!(
                    "node index {idx} at {} where {} was expected (lost sync)",
                    r.o - 4,
                    self.next_index
                ));
            }
            self.notes.push(format!("node indices {}..{} not seen (inside an opaque block)", self.next_index, idx - 1));
        }
        self.seen.insert(idx);
        self.next_index = idx + 1;
        Ok(Some((idx, true)))
    }

    /// Is `o` the start of a skippable chunk header? -> its total length.
    fn skippable_len(&self, o: usize) -> Option<usize> {
        if o + 12 <= self.end && &self.body[o + 4..o + 8] == crate::gbx::SKIP_MAGIC {
            let size = self.u32_at(o + 8)? as usize;
            if o + 12 + size <= self.end {
                return Some(12 + size);
            }
        }
        None
    }

    /// Find the end of a node whose layout is unknown, scanning for the node
    /// end marker followed by what the enclosing list expects next: another
    /// block (a fresh node index and a media class), or the list's terminator
    /// `-1` and a track chunk / node end when this was the list's last entry.
    fn resync(&self, from: usize, is_last: bool) -> R<usize> {
        let marker = NODE_END.to_le_bytes();
        let mut p = from;
        while p + 4 <= self.end {
            if self.body[p..p + 4] == marker {
                let q = p + 4;
                if is_last {
                    if let (Some(w), Some(next)) = (self.u32_at(q), self.u32_at(q + 4)) {
                        if w == NULL_REF && ((next & 0xFFFF_F000) == CLASS_TRACK || next == NODE_END) {
                            return Ok(q);
                        }
                    }
                } else if let (Some(w), Some(c)) = (self.u32_at(q), self.u32_at(q + 4)) {
                    // the next block: a NEW node (nested nodes inside the opaque
                    // span may have used a few indices) and a media class
                    if w >= self.next_index && w < self.next_index + 64 && plausible_class(c) {
                        return Ok(q);
                    }
                }
            }
            p += 1;
        }
        Err(format!("no node end found after {from}"))
    }

    fn parse_block(&mut self, r: &mut Reader, is_last: bool) -> R<Block> {
        let start = r.o;
        let Some((index, is_new)) = self.take_index(r)? else {
            return Err(format!("null block reference at {start}"));
        };
        if !is_new {
            return Ok(Block { index, class: 0, span: (start, r.o), kind: Kind::BackRef });
        }
        let class = r.u32();
        if !plausible_class(class) {
            return Err(format!("block node {index} at {start}: {class:#010x} is not a class id"));
        }
        let mut kind: Option<Kind> = None;
        loop {
            let cid = match self.u32_at(r.o) {
                Some(c) => c,
                None => return Err(format!("block {index}: ran past the chunk end at {}", r.o)),
            };
            if cid == NODE_END {
                r.u32();
                break;
            }
            if let Some(n) = self.skippable_len(r.o) {
                r.skip(n);
                continue;
            }
            let chunk_start = r.o;
            let parsed: Option<R<Kind>> = match (class, cid) {
                (CLASS_CAMERA_CUSTOM, 0x030A_2006) => Some(parse_camera_custom(r)),
                (CLASS_CAMERA_PATH, 0x030A_1003) => Some(parse_camera_path(r)),
                (CLASS_CAMERA_ORBITAL, 0x030A_0001) => Some(parse_camera_orbital(r)),
                (CLASS_TRIANGLES_3D, 0x0302_9001) => Some(parse_triangles(r, true)),
                (CLASS_TRIANGLES_2D, 0x0302_9001) => Some(parse_triangles(r, false)),
                (CLASS_FOG, 0x0319_9000) => Some(parse_fog(r)),
                _ => None,
            };
            match parsed {
                // A known layout must land on something a node continues with;
                // anything else means the layout guess was wrong: fall back to
                // the opaque walk from the chunk start and SAY so.
                // (a chunk of THIS class, or an inherited one: Triangles2D's
                // 0x03029001/2 come from the CGameCtnMediaBlockTriangles base)
                Some(Ok(k)) if matches!(self.u32_at(r.o), Some(w) if w == NODE_END || (w >> 24) == 0x03 && (w & 0xFFF) < 0x100) => {
                    kind = Some(k);
                }
                Some(Ok(_)) | Some(Err(_)) => {
                    let why = match parsed {
                        Some(Err(e)) => e,
                        _ => format!("chunk {cid:#010x} parsed but was not followed by a chunk or the node end"),
                    };
                    let end = self.resync(chunk_start, is_last)?;
                    r.o = end;
                    self.note(format!("block {index} {} ({class:#010x}): layout mismatch, kept verbatim: {why}", class_name(class)));
                    return Ok(Block { index, class, span: (start, end), kind: Kind::Opaque { note: format!("layout mismatch in chunk {cid:#010x}: {why}") } });
                }
                None => {
                    let end = self.resync(chunk_start, is_last)?;
                    // nested nodes may have consumed indices: resume from the next block's index
                    if !is_last {
                        if let Some(w) = self.u32_at(end) {
                            if w >= self.next_index {
                                self.next_index = w;
                            }
                        }
                    }
                    r.o = end;
                    return Ok(Block { index, class, span: (start, end), kind: Kind::Opaque { note: format!("chunk {cid:#010x} not read") } });
                }
            }
        }
        Ok(Block { index, class, span: (start, r.o), kind: kind.unwrap_or(Kind::Opaque { note: "no chunk read".to_string() }) })
    }

    fn note(&mut self, s: String) {
        self.notes.push(s);
    }

    fn parse_track(&mut self, r: &mut Reader) -> R<Track> {
        let start = r.o;
        let Some((index, is_new)) = self.take_index(r)? else {
            return Err(format!("null track reference at {start}"));
        };
        if !is_new {
            return Err(format!("track at {start} is a back-reference to node {index}: not supported"));
        }
        let class = r.u32();
        if class != CLASS_TRACK {
            return Err(format!("track node {index} at {start} has class {class:#010x}, not CGameCtnMediaTrack"));
        }
        let mut name = String::new();
        let mut blocks = Vec::new();
        loop {
            let cid = r.u32();
            match cid {
                NODE_END => break,
                0x0307_8001 => {
                    name = r.string();
                    let list_version = r.u32();
                    if list_version != 10 {
                        return Err(format!("track {name:?}: block list version {list_version}, expected 10"));
                    }
                    let n = r.u32() as usize;
                    if n > 10_000 {
                        return Err(format!("track {name:?}: {n} blocks is not a block count"));
                    }
                    for i in 0..n {
                        blocks.push(self.parse_block(r, i + 1 == n)?);
                    }
                    let terminator = r.u32();
                    if terminator != NULL_REF {
                        return Err(format!("track {name:?}: block list not terminated by -1 at {} ({terminator:#010x})", r.o - 4));
                    }
                }
                0x0307_8004 => {
                    r.u32();
                }
                0x0307_8005 => {
                    let v = r.u32();
                    r.skip(12); // IsKeepPlaying, IsReadOnly, IsCycling
                    if v >= 1 {
                        r.skip(8); // two floats (-1, -1)
                    }
                }
                _ => {
                    if let Some(n) = self.skippable_len(r.o - 4) {
                        r.skip(n - 4);
                    } else {
                        return Err(format!("track {name:?}: unknown chunk {cid:#010x} at {}", r.o - 4));
                    }
                }
            }
        }
        Ok(Track { index, name, blocks, span: (start, r.o) })
    }

    fn parse_clip(&mut self, r: &mut Reader) -> R<Option<Clip>> {
        let start = r.o;
        let Some((index, is_new)) = self.take_index(r)? else {
            return Ok(None);
        };
        if !is_new {
            return Err(format!("clip at {start} is a back-reference to node {index}: not supported"));
        }
        let class = r.u32();
        if class != CLASS_CLIP {
            return Err(format!("clip node {index} at {start} has class {class:#010x}, not CGameCtnMediaClip"));
        }
        let mut name = String::new();
        let mut tracks = Vec::new();
        loop {
            let cid = r.u32();
            match cid {
                NODE_END => break,
                0x0307_900D => {
                    let _version = r.u32();
                    let list_version = r.u32();
                    if list_version != 10 {
                        return Err(format!("clip at {start}: track list version {list_version}, expected 10"));
                    }
                    let n = r.u32() as usize;
                    if n > 10_000 {
                        return Err(format!("clip at {start}: {n} tracks is not a track count"));
                    }
                    for _ in 0..n {
                        tracks.push(self.parse_track(r)?);
                    }
                    name = r.string();
                    r.skip(12); // StopWhenLeave, U02, StopWhenRespawn
                    let _u03 = r.string();
                    r.skip(8); // U04, LocalPlayerClipEntIndex
                }
                _ => {
                    if let Some(n) = self.skippable_len(r.o - 4) {
                        r.skip(n - 4);
                    } else {
                        return Err(format!("clip {name:?} at {start}: unknown chunk {cid:#010x} at {}", r.o - 4));
                    }
                }
            }
        }
        Ok(Some(Clip { index, name, tracks, span: (start, r.o) }))
    }

    fn parse_group(&mut self, r: &mut Reader) -> R<Option<Group>> {
        let start = r.o;
        let Some((index, is_new)) = self.take_index(r)? else {
            return Ok(None);
        };
        if !is_new {
            return Err(format!("clip group at {start} is a back-reference to node {index}: not supported"));
        }
        let class = r.u32();
        if class != CLASS_GROUP {
            return Err(format!("group node {index} at {start} has class {class:#010x}, not CGameCtnMediaClipGroup"));
        }
        let cid = r.u32();
        if cid != 0x0307_A003 {
            return Err(format!("clip group at {start}: first chunk {cid:#010x}, expected 0x0307A003"));
        }
        let list_version = r.u32();
        if list_version != 10 {
            return Err(format!("clip group at {start}: clip list version {list_version}, expected 10"));
        }
        let n = r.u32() as usize;
        if n > 10_000 {
            return Err(format!("clip group at {start}: {n} clips is not a clip count"));
        }
        let mut clips = Vec::new();
        for _ in 0..n {
            match self.parse_clip(r)? {
                Some(c) => clips.push(c),
                None => return Err(format!("clip group at {start}: a null clip in the list")),
            }
        }
        let nt = r.u32() as usize;
        if nt != n {
            return Err(format!("clip group at {start}: {nt} triggers for {n} clips"));
        }
        let mut triggers = Vec::new();
        for _ in 0..nt {
            let u01 = r.i32();
            let u02 = r.i32();
            let u03 = r.i32();
            let u04 = r.i32();
            let condition = r.i32();
            let condition_value = r.f32();
            let nc = r.u32() as usize;
            if nc > 1_000_000 {
                return Err(format!("clip group at {start}: {nc} trigger cells is not a count"));
            }
            let mut coords = Vec::with_capacity(nc);
            for _ in 0..nc {
                coords.push([r.i32(), r.i32(), r.i32()]);
            }
            triggers.push(Trigger { u01, u02, u03, u04, condition, condition_value, coords });
        }
        let tail_start = r.o;
        loop {
            let cid = r.u32();
            if cid == NODE_END {
                break;
            }
            if let Some(nn) = self.skippable_len(r.o - 4) {
                r.skip(nn - 4);
            } else {
                return Err(format!("clip group at {start}: unknown chunk {cid:#010x} at {}", r.o - 4));
            }
        }
        let tail = self.body[tail_start..r.o].to_vec();
        Ok(Some(Group { index, list_version, clips, triggers, span: (start, r.o), tail }))
    }
}

fn vec3(r: &mut Reader) -> [f32; 3] {
    [r.f32(), r.f32(), r.f32()]
}

fn camera_state(r: &mut Reader) -> CameraState {
    CameraState { position: vec3(r), pitch_yaw_roll: vec3(r), fov: r.f32(), target_position: vec3(r), u01: r.f32(), u02: r.f32() }
}

/// CGameCtnMediaBlockCameraCustom chunk 0x030A2006, version 4 (measured on
/// Summer 2026 - 15): version, N keys of 42 words: time, interpolation,
/// anchorRot, anchor, anchorVis, target, then three 12-word camera states —
/// the key's value, its left tangent, its right tangent — each {position,
/// pitch/yaw/roll, fov, target position, 0.05, 1.0}.
fn parse_camera_custom(r: &mut Reader) -> R<Kind> {
    let cid = r.u32();
    debug_assert_eq!(cid, 0x030A_2006);
    let version = r.u32();
    if version != 4 {
        return Err(format!("CameraCustom chunk 006 version {version}: only 4 is known"));
    }
    let n = r.u32() as usize;
    if n > 100_000 || r.o + n * 168 > r.b.len() {
        return Err(format!("{n} keys is not a key count"));
    }
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        let off = r.o;
        let time = r.f32();
        let interpolation = r.i32();
        let anchor_rot = r.i32();
        let anchor = r.i32();
        let anchor_vis = r.i32();
        let target = r.i32();
        let state = camera_state(r);
        let tangents = [camera_state(r), camera_state(r)];
        keys.push(CustomKey { off, time, interpolation, anchor_rot, anchor, anchor_vis, target, state, tangents });
    }
    Ok(Kind::CameraCustom { version, keys })
}

/// CGameCtnMediaBlockCameraPath chunk 0x030A1003 (GBX.NET): version, N keys
/// of {time, position, pitch/yaw/roll, fov, [v>=3: nearZ], anchorRot,
/// anchor, anchorVis, target, target position, weight, [v>=4: ...]}. The
/// per-version tail is worked out from the key stride: the keys are
/// contiguous, so the stride is (chunk bytes to the node end) / N, and the
/// walker checks the landing.
fn parse_camera_path(r: &mut Reader) -> R<Kind> {
    let cid = r.u32();
    debug_assert_eq!(cid, 0x030A_1003);
    let version = r.u32();
    let n = r.u32() as usize;
    if n == 0 || n > 100_000 {
        return Err(format!("{n} keys is not a key count"));
    }
    // words per key by version, GBX.NET: v0..2: 1+3+3+1 +1+1+1+1+3 +1 = 16;
    // v>=3 adds nearZ (17); v>=4 adds two more words (19)?  Measure instead:
    // find the stride from the first key's time to the second key's time by
    // looking for the node end marker.
    let base = 16usize;
    let extra = match version {
        0..=2 => 0,
        3 => 1,
        _ => 3,
    };
    let stride = base + extra;
    if r.o + n * stride * 4 > r.b.len() {
        return Err(format!("{n} keys × {stride} words run past the body"));
    }
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        let off = r.o;
        let time = r.f32();
        let position = vec3(r);
        let pitch_yaw_roll = vec3(r);
        let fov = r.f32();
        let near_z = if version >= 3 { Some(r.f32()) } else { None };
        let anchor_rot = r.i32();
        let anchor = r.i32();
        let anchor_vis = r.i32();
        let target = r.i32();
        let target_position = vec3(r);
        let weight = r.f32();
        let mut tail = Vec::new();
        for _ in 0..(stride - base - if version >= 3 { 1 } else { 0 }) {
            tail.push(r.u32());
        }
        keys.push(PathKey { off, time, position, pitch_yaw_roll, fov, near_z, anchor_rot, anchor, anchor_vis, target, target_position, weight, tail });
    }
    Ok(Kind::CameraPath { version, keys })
}

/// CGameCtnMediaBlockCameraOrbital chunk 0x030A0001: version, N keys. The key
/// layout is not pinned down here; the keys are read as raw words with the
/// stride derived from the version (GBX.NET: v0 = 15 words after the time).
fn parse_camera_orbital(r: &mut Reader) -> R<Kind> {
    let cid = r.u32();
    debug_assert_eq!(cid, 0x030A_0001);
    let version = r.u32();
    let n = r.u32() as usize;
    if n == 0 || n > 100_000 {
        return Err(format!("{n} keys is not a key count"));
    }
    let stride_words = 16usize;
    if r.o + n * stride_words * 4 > r.b.len() {
        return Err(format!("{n} keys × {stride_words} words run past the body"));
    }
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        let off = r.o;
        let time = r.f32();
        let mut words = Vec::with_capacity(stride_words - 1);
        for _ in 1..stride_words {
            words.push(r.u32());
        }
        keys.push(OrbitalKey { off, time, words });
    }
    Ok(Kind::CameraOrbital { version, keys })
}

/// CGameCtnMediaBlockTriangles chunk 0x03029001 (measured on Summer 15's
/// Triangles2D): N key times, N again, V vertices, N×V Vec3 positions, V
/// RGBA colours, T Int3 triangles, then int, int, int, float, int, long.
/// CGameCtnMediaBlockFog chunk 0x03199000 (measured on Summer 13/16/17's
/// ambiance clips, version 2): version, N keys of {time, intensity,
/// skyIntensity, distance, [v>=1: coefficient, colour RGB], [v>=2: clouds
/// opacity, clouds speed]}. The distance is metres of world: scaled.
fn parse_fog(r: &mut Reader) -> R<Kind> {
    let cid = r.u32();
    debug_assert_eq!(cid, 0x0319_9000);
    let version = r.u32();
    if version > 2 {
        return Err(format!("Fog chunk version {version}: only 0..2 are known"));
    }
    let n = r.u32() as usize;
    let words = 4 + if version >= 1 { 4 } else { 0 } + if version >= 2 { 2 } else { 0 };
    if n > 100_000 || r.o + n * words * 4 > r.b.len() {
        return Err(format!("{n} keys is not a key count"));
    }
    let mut distances = Vec::with_capacity(n);
    for _ in 0..n {
        let _time = r.f32();
        let _intensity = r.f32();
        let _sky = r.f32();
        distances.push((r.o, r.f32()));
        r.skip((words - 4) * 4);
    }
    Ok(Kind::Fog { version, distances })
}

fn parse_triangles(r: &mut Reader, world: bool) -> R<Kind> {
    let cid = r.u32();
    debug_assert_eq!(cid, 0x0302_9001);
    let n = r.u32() as usize;
    if n > 100_000 {
        return Err(format!("{n} keys is not a key count"));
    }
    let mut times = Vec::with_capacity(n);
    for _ in 0..n {
        times.push(r.f32());
    }
    let n2 = r.u32() as usize;
    if n2 != n {
        return Err(format!("key count {n} then {n2}"));
    }
    let nv = r.u32() as usize;
    if nv > 1_000_000 || r.o + n * nv * 12 > r.b.len() {
        return Err(format!("{nv} vertices is not a vertex count"));
    }
    let mut positions = Vec::with_capacity(n);
    for _ in 0..n {
        let mut row = Vec::with_capacity(nv);
        for _ in 0..nv {
            row.push((r.o, vec3(r)));
        }
        positions.push(row);
    }
    let nc = r.u32() as usize;
    if nc != nv {
        return Err(format!("{nc} colours for {nv} vertices"));
    }
    r.skip(nc * 16);
    let nt = r.u32() as usize;
    if nt > 1_000_000 || r.o + nt * 12 > r.b.len() {
        return Err(format!("{nt} triangles is not a triangle count"));
    }
    r.skip(nt * 12);
    r.skip(4 * 5 + 8); // U01..U03 int, U04 float, U05 int, U06 long
    if world {
        Ok(Kind::Triangles3D { times, positions })
    } else {
        Ok(Kind::Opaque { note: format!("Triangles2D: {n} keys × {nv} screen-space vertices, {nt} triangles (kept)") })
    }
}

/// Find chunk 0x03043049 in a map body: it is not skippable, so it is located
/// as the chunk that starts where a skippable chunk ends (the baked-blocks
/// chunk 0x03043048 on every Summer map) and runs to the next skippable
/// chunk's header.
pub fn locate(body: &[u8]) -> Option<(usize, usize)> {
    let chunks = crate::gbx::all_skip_chunks(body);
    for (i, &(_, _, payload, size)) in chunks.iter().enumerate() {
        let e = payload + size;
        if e + 4 <= body.len() && u32::from_le_bytes(body[e..e + 4].try_into().unwrap()) == CHUNK_MEDIATRACKER {
            // the MediaTracker's own clips carry skippable chunks (0x0307900E
            // per clip, 0x03029002 per triangles block): the chunk ends at the
            // first skippable chunk of the CHALLENGE class after it
            let end = chunks[i + 1..].iter().find(|(c, ..)| (*c & 0xFFFF_F000) == 0x0304_3000).map(|(_, off, _, _)| *off)?;
            return Some((e, end));
        }
    }
    None
}

pub fn parse(body: &[u8], span: (usize, usize)) -> R<MediaTracker> {
    let (start, end) = span;
    let mut r = Reader::at(body, start);
    let cid = r.u32();
    if cid != CHUNK_MEDIATRACKER {
        return Err(format!("chunk at {start} is {cid:#010x}, not 0x03043049"));
    }
    let version = r.u32();
    if version > 2 {
        return Err(format!("MediaTracker chunk version {version}: only 0..2 are known"));
    }
    // Node indices are assigned in write order: the first node here carries
    // the index right after everything the body defined before it. Read it
    // ahead so back-references (< first) and definitions (== next) separate.
    let first_new = {
        let mut probe = Reader::at(body, r.o);
        let mut first = None;
        for _ in 0..5 {
            let w = probe.u32();
            if w != NULL_REF {
                first = Some(w);
                break;
            }
        }
        first
    };
    let mut w = Walker { body, end, next_index: first_new.unwrap_or(0), seen: Default::default(), notes: Vec::new() };
    let intro = match w.parse_clip(&mut r)? {
        Some(c) => Slot::Clip(c),
        None => Slot::Null,
    };
    let podium = match w.parse_clip(&mut r)? {
        Some(c) => Slot::Clip(c),
        None => Slot::Null,
    };
    let in_game = match w.parse_group(&mut r)? {
        Some(g) => Slot::Group(g),
        None => Slot::Null,
    };
    let end_race = match w.parse_group(&mut r)? {
        Some(g) => Slot::Group(g),
        None => Slot::Null,
    };
    let ambiance = if version >= 2 {
        match w.parse_clip(&mut r)? {
            Some(c) => Slot::Clip(c),
            None => Slot::Null,
        }
    } else {
        Slot::Null
    };
    let trigger_size = if version >= 1 { Some([r.i32(), r.i32(), r.i32()]) } else { None };
    if r.o != end {
        return Err(format!(
            "MediaTracker chunk parsed to {} but the next chunk starts at {end} ({} bytes {})",
            r.o,
            (r.o as i64 - end as i64).abs(),
            if r.o < end { "unread" } else { "overrun" }
        ));
    }
    let mut mt = MediaTracker { start, end, version, intro, podium, in_game, end_race, ambiance, trigger_size, edits: Vec::new(), strip: false };
    mt.notes_from(w.notes);
    Ok(mt)
}

impl MediaTracker {
    fn notes_from(&mut self, notes: Vec<String>) {
        for n in notes {
            eprintln!("  mediatracker: {n}");
        }
    }

    pub fn slots(&self) -> [(&'static str, &Slot); 5] {
        [("intro", &self.intro), ("podium", &self.podium), ("in-game", &self.in_game), ("end-race", &self.end_race), ("ambiance", &self.ambiance)]
    }

    fn slots_mut(&mut self) -> [(&'static str, &mut Slot); 5] {
        [("intro", &mut self.intro), ("podium", &mut self.podium), ("in-game", &mut self.in_game), ("end-race", &mut self.end_race), ("ambiance", &mut self.ambiance)]
    }

    /// Every clip, with the slot it hangs off.
    pub fn clips(&self) -> Vec<(&'static str, &Clip)> {
        let mut out = Vec::new();
        for (slot, s) in self.slots() {
            match s {
                Slot::Null => {}
                Slot::Clip(c) => out.push((slot, c)),
                Slot::Group(g) => out.extend(g.clips.iter().map(|c| (slot, c))),
            }
        }
        out
    }

    /// The report `tmmaps mediatracker MAP` prints.
    pub fn report(&self, ground: f32) -> String {
        let ts = self.trigger_size.unwrap_or([3, 1, 3]);
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(s, "MediaTracker chunk 0x03043049 v{} at body {}..{} ({} bytes); trigger size {:?}", self.version, self.start, self.end, self.end - self.start, self.trigger_size);
        for (slot, sl) in self.slots() {
            match sl {
                Slot::Null => {
                    let _ = writeln!(s, "{slot}: -");
                }
                Slot::Clip(c) => report_clip(&mut s, slot, c, None, ts, ground),
                Slot::Group(g) => {
                    let _ = writeln!(s, "{slot}: group node {} with {} clips", g.index, g.clips.len());
                    for (c, t) in g.clips.iter().zip(&g.triggers) {
                        report_clip(&mut s, slot, c, Some(t), ts, ground);
                    }
                }
            }
        }
        s
    }

    /// The field offsets `transform` writes through must be the ones the
    /// parser read from: every world-coordinate field re-read from `body` at
    /// its edit offset must equal the parsed value.
    pub fn check_offsets(&self, body: &[u8]) -> Result<(), String> {
        let f = |o: usize| f32::from_le_bytes(body[o..o + 4].try_into().unwrap());
        let v3 = |o: usize| [f(o), f(o + 4), f(o + 8)];
        for (_, c) in self.clips() {
            for t in &c.tracks {
                for b in &t.blocks {
                    match &b.kind {
                        Kind::CameraCustom { keys, .. } => {
                            for k in keys {
                                let so = k.off + 6 * 4;
                                if v3(so) != k.state.position || v3(so + 7 * 4) != k.state.target_position || f(so + 6 * 4) != k.state.fov {
                                    return Err(format!("CameraCustom key at {}: state offsets disagree with the parse", k.off));
                                }
                                for (i, tg) in k.tangents.iter().enumerate() {
                                    let to = so + 12 * 4 * (i + 1);
                                    if v3(to) != tg.position || v3(to + 7 * 4) != tg.target_position {
                                        return Err(format!("CameraCustom key at {}: tangent {i} offsets disagree with the parse", k.off));
                                    }
                                }
                            }
                        }
                        Kind::CameraPath { version, keys } => {
                            for k in keys {
                                let tp = k.off + 4 * (1 + 3 + 3 + 1 + if *version >= 3 { 1 } else { 0 } + 4);
                                if v3(k.off + 4) != k.position || v3(tp) != k.target_position {
                                    return Err(format!("CameraPath key at {}: offsets disagree with the parse", k.off));
                                }
                            }
                        }
                        Kind::Triangles3D { positions, .. } => {
                            for (o, p) in positions.iter().flatten() {
                                if v3(*o) != *p {
                                    return Err(format!("Triangles3D vertex at {o}: offset disagrees with the parse"));
                                }
                            }
                        }
                        Kind::Fog { distances, .. } => {
                            for (o, d) in distances {
                                if f(*o) != *d {
                                    return Err(format!("Fog key at {o}: offset disagrees with the parse"));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Camera keys (world positions) of every block, for plausibility checks.
    pub fn camera_positions(&self) -> Vec<[f32; 3]> {
        let mut out = Vec::new();
        for (_, c) in self.clips() {
            for t in &c.tracks {
                for b in &t.blocks {
                    match &b.kind {
                        Kind::CameraCustom { keys, .. } => out.extend(keys.iter().map(|k| k.state.position)),
                        Kind::CameraPath { keys, .. } => out.extend(keys.iter().map(|k| k.position)),
                        Kind::Triangles3D { positions, .. } => out.extend(positions.iter().flatten().map(|(_, p)| *p)),
                        _ => {}
                    }
                }
            }
        }
        out
    }

    /// Opaque blocks: (class, note) — what the transform leaves alone.
    pub fn opaque_blocks(&self) -> Vec<(u32, String)> {
        let mut out = Vec::new();
        for (_, c) in self.clips() {
            for t in &c.tracks {
                for b in &t.blocks {
                    if let Kind::Opaque { note } = &b.kind {
                        out.push((b.class, note.clone()));
                    }
                }
            }
        }
        out
    }

    /// Move every world coordinate through `point` (a position: the items'
    /// transform), every tangent/distance through `scale`, and every trigger
    /// cell through `cell`. Times, angles and fields of view stay. Returns
    /// (camera keys moved, triangle vertices moved, trigger cells before ->
    /// after, blocks left alone).
    pub fn transform(&mut self, point: &dyn Fn([f32; 3]) -> [f32; 3], scale: f32, cell: &dyn Fn([i32; 3]) -> Vec<[i32; 3]>) -> (usize, usize, (usize, usize), usize) {
        let mut edits: Vec<(usize, [u8; 4])> = Vec::new();
        let mut keys_moved = 0usize;
        let mut verts_moved = 0usize;
        let mut cells = (0usize, 0usize);
        let mut left = 0usize;
        let put3 = |edits: &mut Vec<(usize, [u8; 4])>, off: usize, v: [f32; 3]| {
            for (k, x) in v.iter().enumerate() {
                edits.push((off + k * 4, x.to_le_bytes()));
            }
        };
        let scale3 = |v: [f32; 3]| [v[0] * scale, v[1] * scale, v[2] * scale];
        let used = |v: [f32; 3]| v.iter().any(|x| *x != 0.0);
        for (_, slot) in self.slots_mut() {
            let clips: Vec<&mut Clip> = match slot {
                Slot::Null => Vec::new(),
                Slot::Clip(c) => vec![c],
                Slot::Group(g) => {
                    for t in &mut g.triggers {
                        cells.0 += t.coords.len();
                        let mut mapped: Vec<[i32; 3]> = t.coords.iter().flat_map(|c| cell(*c)).collect();
                        // neighbouring source cells share target cells
                        let mut seen = std::collections::BTreeSet::new();
                        mapped.retain(|c| seen.insert(*c));
                        cells.1 += mapped.len();
                        t.coords = mapped;
                    }
                    g.clips.iter_mut().collect()
                }
            };
            for c in clips {
                for t in &mut c.tracks {
                    for b in &mut t.blocks {
                        match &mut b.kind {
                            Kind::CameraCustom { keys, .. } => {
                                for k in keys.iter_mut() {
                                    // a camera hung on an anchor (the player car) is an
                                    // OFFSET from a full-size car: not a world point
                                    if k.anchor != -1 {
                                        left += 1;
                                        continue;
                                    }
                                    // key layout: time, 5 ints, state (12 words), 2 tangents (12 words each)
                                    let state_off = k.off + 6 * 4;
                                    k.state.position = point(k.state.position);
                                    put3(&mut edits, state_off, k.state.position);
                                    if used(k.state.target_position) {
                                        k.state.target_position = point(k.state.target_position);
                                        put3(&mut edits, state_off + 7 * 4, k.state.target_position);
                                    }
                                    for (ti, tg) in k.tangents.iter_mut().enumerate() {
                                        let toff = state_off + 12 * 4 * (ti + 1);
                                        if used(tg.position) {
                                            tg.position = scale3(tg.position);
                                            put3(&mut edits, toff, tg.position);
                                        }
                                        if used(tg.target_position) {
                                            tg.target_position = scale3(tg.target_position);
                                            put3(&mut edits, toff + 7 * 4, tg.target_position);
                                        }
                                    }
                                    keys_moved += 1;
                                }
                            }
                            Kind::CameraPath { version, keys } => {
                                for k in keys.iter_mut() {
                                    if k.anchor != -1 {
                                        left += 1;
                                        continue;
                                    }
                                    // time, position(3), pyr(3), fov, [nearZ], anchorRot, anchor, anchorVis, target, targetPosition(3), weight
                                    k.position = point(k.position);
                                    put3(&mut edits, k.off + 4, k.position);
                                    if used(k.target_position) {
                                        let tp_off = k.off + 4 * (1 + 3 + 3 + 1 + if *version >= 3 { 1 } else { 0 } + 4);
                                        k.target_position = point(k.target_position);
                                        put3(&mut edits, tp_off, k.target_position);
                                    }
                                    keys_moved += 1;
                                }
                            }
                            Kind::Triangles3D { positions, .. } => {
                                for row in positions.iter_mut() {
                                    for (off, p) in row.iter_mut() {
                                        *p = point(*p);
                                        put3(&mut edits, *off, *p);
                                        verts_moved += 1;
                                    }
                                }
                            }
                            Kind::Fog { distances, .. } => {
                                for (off, d) in distances.iter_mut() {
                                    *d *= scale;
                                    edits.push((*off, d.to_le_bytes()));
                                }
                            }
                            Kind::CameraOrbital { .. } | Kind::Opaque { .. } | Kind::BackRef => left += 1,
                        }
                    }
                }
            }
        }
        self.edits.extend(edits);
        (keys_moved, verts_moved, cells, left)
    }

    /// The chunk's bytes (id word included) with the edits applied and the
    /// trigger lists re-emitted. With no edit and no transform this reproduces
    /// the source bytes exactly (`tests::mediatracker_roundtrips`).
    pub fn emit(&self, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.end - self.start);
        out.extend_from_slice(&CHUNK_MEDIATRACKER.to_le_bytes());
        out.extend_from_slice(&self.version.to_le_bytes());
        let copy = |out: &mut Vec<u8>, span: (usize, usize)| {
            let base = out.len();
            out.extend_from_slice(&body[span.0..span.1]);
            for (off, bytes) in &self.edits {
                if *off >= span.0 && *off + 4 <= span.1 {
                    let o = base + (*off - span.0);
                    out[o..o + 4].copy_from_slice(bytes);
                }
            }
        };
        let clip = |out: &mut Vec<u8>, s: &Slot| match s {
            Slot::Clip(c) if !self.strip => copy(out, c.span),
            Slot::Group(_) => unreachable!("a clip slot holding a group"),
            _ => out.extend_from_slice(&NULL_REF.to_le_bytes()),
        };
        let group = |out: &mut Vec<u8>, s: &Slot| match s {
            Slot::Group(g) if !self.strip => {
                out.extend_from_slice(&g.index.to_le_bytes());
                out.extend_from_slice(&CLASS_GROUP.to_le_bytes());
                out.extend_from_slice(&0x0307_A003u32.to_le_bytes());
                out.extend_from_slice(&g.list_version.to_le_bytes());
                out.extend_from_slice(&(g.clips.len() as u32).to_le_bytes());
                for c in &g.clips {
                    copy(out, c.span);
                }
                out.extend_from_slice(&(g.triggers.len() as u32).to_le_bytes());
                for t in &g.triggers {
                    for v in [t.u01, t.u02, t.u03, t.u04, t.condition] {
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                    out.extend_from_slice(&t.condition_value.to_le_bytes());
                    out.extend_from_slice(&(t.coords.len() as u32).to_le_bytes());
                    for c in &t.coords {
                        for v in c {
                            out.extend_from_slice(&v.to_le_bytes());
                        }
                    }
                }
                out.extend_from_slice(&g.tail);
            }
            Slot::Clip(_) => unreachable!("a group slot holding a clip"),
            _ => out.extend_from_slice(&NULL_REF.to_le_bytes()),
        };
        clip(&mut out, &self.intro);
        clip(&mut out, &self.podium);
        group(&mut out, &self.in_game);
        group(&mut out, &self.end_race);
        if self.version >= 2 {
            clip(&mut out, &self.ambiance);
        }
        if let Some(ts) = self.trigger_size {
            for v in ts {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out
    }
}

fn fmt3(v: [f32; 3]) -> String {
    format!("({:.2},{:.2},{:.2})", v[0], v[1], v[2])
}

/// World box of a set of trigger cells: the grid divides a 32×8×32 block cell
/// into `ts` boxes, row 0 at the collection's `ground`.
pub fn trigger_world_box(coords: &[[i32; 3]], ts: [i32; 3], ground: f32) -> Option<([f32; 3], [f32; 3])> {
    let unit = [32.0 / ts[0].max(1) as f32, 8.0 / ts[1].max(1) as f32, 32.0 / ts[2].max(1) as f32];
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for c in coords {
        for k in 0..3 {
            let base = if k == 1 { ground } else { 0.0 };
            lo[k] = lo[k].min(base + c[k] as f32 * unit[k]);
            hi[k] = hi[k].max(base + (c[k] + 1) as f32 * unit[k]);
        }
    }
    if coords.is_empty() {
        None
    } else {
        Some((lo, hi))
    }
}

fn report_clip(s: &mut String, slot: &str, c: &Clip, trigger: Option<&Trigger>, ts: [i32; 3], ground: f32) {
    use std::fmt::Write;
    let _ = writeln!(s, "{slot}: clip {:?} (node {}, {} bytes, {} tracks)", c.name, c.index, c.span.1 - c.span.0, c.tracks.len());
    if let Some(t) = trigger {
        let world = trigger_world_box(&t.coords, ts, ground).map(|(lo, hi)| format!(" = world x {:.0}..{:.0} y {:.0}..{:.0} z {:.0}..{:.0}", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2])).unwrap_or_default();
        let _ = writeln!(s, "  trigger: condition {} value {} u {},{},{},{}; {} cells: {}{world}", t.condition, t.condition_value, t.u01, t.u02, t.u03, t.u04, t.coords.len(), t.coords.iter().map(|c| format!("{},{},{}", c[0], c[1], c[2])).collect::<Vec<_>>().join(" "));
    }
    for t in &c.tracks {
        let _ = writeln!(s, "  track {:?} ({} blocks)", t.name, t.blocks.len());
        for b in &t.blocks {
            match &b.kind {
                Kind::CameraCustom { version, keys } => {
                    let _ = writeln!(s, "    CameraCustom v{version} ({} keys, {} bytes)", keys.len(), b.span.1 - b.span.0);
                    for k in keys {
                        let _ = writeln!(s, "      t={:.3} pos={} pyr=({:.3},{:.3},{:.3}) fov={:.1} anchor={} vis={} rot={} target={} tpos={} interp={} tangents pos {} / {}", k.time, fmt3(k.state.position), k.state.pitch_yaw_roll[0], k.state.pitch_yaw_roll[1], k.state.pitch_yaw_roll[2], k.state.fov, k.anchor, k.anchor_vis, k.anchor_rot, k.target, fmt3(k.state.target_position), k.interpolation, fmt3(k.tangents[0].position), fmt3(k.tangents[1].position));
                    }
                }
                Kind::CameraPath { version, keys } => {
                    let _ = writeln!(s, "    CameraPath v{version} ({} keys, {} bytes)", keys.len(), b.span.1 - b.span.0);
                    for k in keys {
                        let _ = writeln!(s, "      t={:.3} pos={} pyr=({:.3},{:.3},{:.3}) fov={:.1} nearZ={:?} anchor={} vis={} rot={} target={} tpos={} weight={} tail={:?}", k.time, fmt3(k.position), k.pitch_yaw_roll[0], k.pitch_yaw_roll[1], k.pitch_yaw_roll[2], k.fov, k.near_z, k.anchor, k.anchor_vis, k.anchor_rot, k.target, fmt3(k.target_position), k.weight, k.tail);
                    }
                }
                Kind::CameraOrbital { version, keys } => {
                    let _ = writeln!(s, "    CameraOrbital v{version} ({} keys, {} bytes) — NOT transformed (layout unknown)", keys.len(), b.span.1 - b.span.0);
                    for k in keys {
                        let _ = writeln!(s, "      t={:.3} words={}", k.time, k.words.iter().map(|w| format!("{:#x}/{}", w, f32::from_bits(*w))).collect::<Vec<_>>().join(" "));
                    }
                }
                Kind::Fog { version, distances } => {
                    let _ = writeln!(s, "    Fog v{version} ({} keys, {} bytes): distances {}", distances.len(), b.span.1 - b.span.0, distances.iter().map(|(_, d)| format!("{d}")).collect::<Vec<_>>().join(" "));
                }
                Kind::Triangles3D { times, positions } => {
                    let _ = writeln!(s, "    Triangles3D ({} keys, {} vertices, {} bytes)", times.len(), positions.first().map(|r| r.len()).unwrap_or(0), b.span.1 - b.span.0);
                    for (t, row) in times.iter().zip(positions) {
                        let _ = writeln!(s, "      t={:.3} {}", t, row.iter().map(|(_, p)| fmt3(*p)).collect::<Vec<_>>().join(" "));
                    }
                }
                Kind::Opaque { note } => {
                    let _ = writeln!(s, "    {} {:#010x} ({} bytes) kept verbatim: {note}", class_name(b.class), b.class, b.span.1 - b.span.0);
                }
                Kind::BackRef => {
                    let _ = writeln!(s, "    back-reference to node {}", b.index);
                }
            }
        }
    }
}

impl crate::map::MapFile {
    /// The map's MediaTracker, or None when the chunk is absent.
    pub fn mediatracker(&self) -> Option<R<MediaTracker>> {
        let span = locate(&self.gbx.body)?;
        Some(parse(&self.gbx.body, span))
    }

    /// Replace chunk 0x03043049 with `mt` re-emitted (a variable-length
    /// splice: only after a write+reload, like the other splices).
    pub fn set_mediatracker(&mut self, mt: &MediaTracker) {
        let bytes = mt.emit(&self.gbx.body);
        self.raw_splices.push(((mt.start, mt.end), bytes));
    }
}

/// `tmmaps mediatracker MAP [--brief]`: the report.
pub fn cmd(args: &[String]) {
    let path = std::path::Path::new(&args[2]);
    let m = crate::map::MapFile::load(path);
    match m.mediatracker() {
        None => println!("{}: no MediaTracker chunk (0x03043049)", path.display()),
        Some(Err(e)) => crate::cli::die(&format!("{}: MediaTracker: {e}", path.display())),
        Some(Ok(mt)) => {
            if crate::cli::has(args, "--brief") {
                let clips = mt.clips();
                let mut classes: std::collections::BTreeMap<String, usize> = Default::default();
                let mut opaque = 0usize;
                for (_, c) in &clips {
                    for t in &c.tracks {
                        for b in &t.blocks {
                            *classes.entry(format!("{}{}", class_name(b.class), if matches!(b.kind, Kind::Opaque { .. }) { "*" } else { "" })).or_default() += 1;
                            if matches!(b.kind, Kind::Opaque { .. }) {
                                opaque += 1;
                            }
                        }
                    }
                }
                let pos = mt.camera_positions();
                let inside = pos.iter().filter(|p| p.iter().all(|v| (-64.0..=2112.0).contains(v))).count();
                println!(
                    "{}: v{} clips {} (intro {}, podium {}, in-game {}, end-race {}, ambiance {}); blocks {} ({opaque} opaque); camera/vertex positions {} ({inside} inside 0..2048); trigger size {:?}; classes {}",
                    path.file_name().unwrap().to_string_lossy(),
                    mt.version,
                    clips.len(),
                    matches!(mt.intro, Slot::Clip(_)) as u8,
                    matches!(mt.podium, Slot::Clip(_)) as u8,
                    match &mt.in_game { Slot::Group(g) => g.clips.len(), _ => 0 },
                    match &mt.end_race { Slot::Group(g) => g.clips.len(), _ => 0 },
                    matches!(mt.ambiance, Slot::Clip(_)) as u8,
                    classes.values().sum::<usize>(),
                    pos.len(),
                    mt.trigger_size,
                    classes.iter().map(|(k, v)| format!("{k}×{v}")).collect::<Vec<_>>().join(" ")
                );
            } else {
                let collection = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
                print!("{}", mt.report(crate::map::ground_y(collection)));
            }
            if let Err(e) = mt.check_offsets(&m.gbx.body) {
                crate::cli::die(&format!("{}: {e}", path.display()));
            }
            // the re-emit must reproduce the chunk
            let again = mt.emit(&m.gbx.body);
            let orig = &m.gbx.body[mt.start..mt.end];
            if again != orig {
                let first = again.iter().zip(orig).position(|(a, b)| a != b);
                crate::cli::die(&format!("re-emitting the chunk unchanged gives {} bytes for {} (first difference at {:?}): the writer does not reproduce this map", again.len(), orig.len(), first));
            }
        }
    }
}
