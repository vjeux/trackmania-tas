//! Manufacturing a `.Ghost.Gbx` **from nothing**.
//!
//! This is rung 0 of the autopilot project and it is the file the no-ghost rule
//! stands or falls on. Every container in this project's history is a human's
//! recording with its input archive patched; this module writes one out of
//! constants, an input tape and a map uid, and nothing else.
//!
//! # What is deliberately absent
//!
//! There is no template parameter, no donor path, no "start from a known-good
//! file". The bytes below are either a constant of the GBX format, a value
//! computed from the tape, or a value we chose. **If this file compiles and its
//! output validates, no human recording informed it**, and that is a property
//! of the source you can read rather than a claim in a manifest.
//!
//! # Layout, as this writer emits it
//!
//! ```text
//! "GBX" u16 version=6  u8 'B' (binary)  u8 'U' (refs uncompressed)
//!       u8 'U' (BODY UNCOMPRESSED)      u8 'R' (unknown, version>=4)
//!       u32 class_id = 0x03092000  (CGameCtnGhost)
//!       u32 user_data_len, user_data
//!       u32 num_nodes
//!       u32 num_external_nodes = 0     (the whole reference table)
//! body: a sequence of chunks, then 0xFACADE01
//! ```
//!
//! The body is written **uncompressed** (`'U'`). The server accepts it, and it
//! means this module never needs an LZO *compressor* — the `gbx` crate only
//! dlopens the decompressor.
//!
//! # Chunk framing
//!
//! A *skippable* chunk is `u32 id, "PIKS", u32 size, payload`: a parser that
//! does not know the id can step over it. A *non-skippable* chunk is `u32 id,
//! payload` and the parser must know its layout or the parse dies there. Which
//! of ours are which is not a guess — `gbx::all_skip_chunks` finds `0x03092005`,
//! `0x0309201D` and `0x0309202B` by their `PIKS` marker in the wild, and
//! `ghost::ident` documents `0x0309200F` and `0x03092010` as inline chunks with
//! no marker and no table entry.

use crate::tape::{Input, Tape as InputTape};
use gbx::record::{Desc, Ent, RecordData, CLASS_CSCENEVEHICLEVIS};
use gbx::recwrite::{encode_record_node, write_transform, Xform};
use gbx::tape::{Archive, Encoding, Packet, StateEnc, Tape as GbxTape};

/// `CGameCtnGhost`.
pub const CLASS_CGAMECTNGHOST: u32 = 0x03092000;
/// `CGameCtnReplayRecord` — what a `.Replay.Gbx` is.
pub const CLASS_CGAMECTNREPLAYRECORD: u32 = 0x03093000;
/// End-of-body marker.
pub const FACADE: u32 = 0xFACADE01;

pub const CHUNK_RACETIME: u32 = 0x03092005;
pub const CHUNK_LOGIN: u32 = 0x0309200F;
pub const CHUNK_VALIDATE_UID: u32 = 0x03092010;
/// The ghost's own game version. **Read from the engine's chunk dispatch
/// table**: `CGameCtnGhost`'s switch is indexed by `chunk_id & 0xFF` over
/// `0x03092000..=0x0309202E`, and index `0x14`'s handler reads one `u32` into
/// the ghost's version field and sets its "version present" flag.
///
/// Without this chunk the engine forces the version to **6** and refuses the
/// file with *"Replay version TMr.6 is not compatible with the current game
/// version TMr.8."* — measured.
pub const CHUNK_VERSION: u32 = 0x03092014;
pub const CHUNK_INPUTS: u32 = 0x0309201D;
pub const CHUNK_RESULT: u32 = 0x0309202B;
/// The validation block: exe version, checksum, os/cpu kind, **the walltime
/// pair**, the race-settings string and **the race-settings flags**. Index
/// `0x2D` in the same table.
///
/// Two of its fields are hard gates in the validator, and a file without this
/// chunk fails both with their defaults:
///
/// * `settings` (`+0x220`, default `1`) must satisfy
///   `(s & 0xE0) != 0 && (s & 0x700) != 0 && (s & 0x1C) != 0`, or the server
///   says *"cannot validate scripted modes (settings: 1)"*;
/// * the walltime pair (`+0x1DC`, `+0x1E0`, default `-1`) must both be set, or
///   the server says *"walltime not set"* — and their difference must sit
///   within `race_ms ± (10 s + 10 %)`, which is the arithmetic at the check.
pub const CHUNK_VALIDATION: u32 = 0x0309202D;

/// Everything that is not the tape.
///
/// Each field is a value we CHOOSE, and the ones that matter are called out.
/// There is no `Default` that silently fills these in: a container whose
/// declared time was defaulted is exactly the defect this project keeps paying
/// for, so the caller states them.
#[derive(Clone, Debug)]
pub struct GhostMeta {
    /// The map the tape is for. The validator matches this against the map it
    /// has; getting it wrong is a DNF that looks like bad driving.
    pub map_uid: String,
    /// What the file CLAIMS the run does. The validator compares its own
    /// simulation against this, so a wrong value here reads as a rejection.
    pub declared_ms: u32,
    /// The checkpoint times the file claims, finish last.
    pub declared_cps: Vec<i32>,
    /// The login the file reports. Ours is always `TAS`: short enough that it
    /// carries no account id (the id is base64 of a 16-byte login).
    pub login: String,
    /// The validation seed. **This feeds the simulation**: a different seed is
    /// a different — but equally real — run. We pick one and own it.
    pub validation_seed: u32,
    /// Milliseconds of countdown before tick 0 of the archive.
    pub start_offset_ms: i32,
    /// Input-archive format version: 11 (33-bit state literal) or 12 (34-bit).
    pub format_version: u32,
    /// The `0x0309201D` chunk version word.
    pub input_chunk_version: u32,
    /// Archive header word whose meaning is not established here. Named
    /// `field0` in `gbx::tape` for the same reason.
    pub field0: u32,
    /// Half-open tick ranges carrying named-but-not-yet-understood state flags.
    /// Empty is the minimal baseline. Diagnostic callers can reproduce pulses
    /// observed in game-written input archives without copying any packet bytes.
    pub state_flag_ranges: Vec<(usize, usize, u32)>,
    /// The ghost's declared game version, written into chunk `0x03092014`.
    /// The engine's current version is **8**; anything else is refused.
    pub game_version: u32,
    /// Validation block (`0x0309202D`) — the engine build string. Empty is
    /// accepted; it is not a gate.
    pub exe_version: String,
    pub exe_checksum: u32,
    pub os_kind: u32,
    pub cpu_kind: u32,
    pub title_id: String,
    pub title_checksum: [u8; 32],
    /// The walltime pair, in unix seconds. Both must be set (`!= -1`) and
    /// their difference must sit within the race time ± (10 s + 10 %).
    pub walltime_start: u32,
    pub walltime_end: u32,
    pub race_settings: String,
    /// The index of the semantic Spawn inside the engine's checkpoint array.
    /// TM2020 orders non-Spawn block waypoints first, then Spawn blocks, then
    /// item waypoints. A zero here selects array entry zero — on Summer 2026 -
    /// 01 that is the last checkpoint, which was the wrong-start defect.
    pub validation_start_index: u32,
    pub validation_u04: u32,
    /// The race-settings flags. **Three 3-bit fields must each be non-zero**
    /// or the server refuses with "cannot validate scripted modes".
    pub settings_flags: u32,
}

/// The lowest value that satisfies the validator's settings mask: one bit set
/// in each of the three fields it tests (`0x1C`, `0xE0`, `0x700`).
///
/// Chosen as the MINIMAL passing value rather than "all bits on": every bit in
/// there asserts something about the race, and asserting nine things to satisfy
/// a test that asks for three is how a constant nobody can re-derive gets into
/// a file.
pub const SETTINGS_MINIMAL_VALID: u32 = 0x004 | 0x020 | 0x100;

impl GhostMeta {
    /// A container for `map_uid` declaring nothing yet — the shape used for the
    /// first probe, before any time is known.
    pub fn probe(map_uid: &str) -> GhostMeta {
        GhostMeta {
            map_uid: map_uid.to_string(),
            declared_ms: 0,
            declared_cps: Vec::new(),
            login: "TAS".to_string(),
            validation_seed: 0,
            start_offset_ms: 0,
            format_version: 11,
            input_chunk_version: 4,
            field0: 0,
            state_flag_ranges: Vec::new(),
            game_version: 8,
            exe_version: "date=2026-05-15_18_00".to_string(),
            exe_checksum: 0,
            os_kind: 0,
            cpu_kind: 0,
            title_id: String::new(),
            title_checksum: [0; 32],
            // A zero-length race means a zero-length walltime, which satisfies
            // the window trivially. `set_declared` moves both together so they
            // can never drift apart.
            walltime_start: 1_700_000_000,
            walltime_end: 1_700_000_000,
            race_settings: String::new(),
            validation_start_index: 0,
            validation_u04: 0,
            settings_flags: SETTINGS_MINIMAL_VALID,
        }
    }

    /// Declare a time, and move the walltime with it.
    ///
    /// These two are one fact and they are checked against each other, so they
    /// are set by one call. Setting the declared time and forgetting the
    /// walltime produces "unexcepted walltime", which reads as a container bug
    /// rather than as the two-line arithmetic it is.
    pub fn set_declared(&mut self, ms: u32, cps: Vec<i32>) {
        self.declared_ms = ms;
        self.declared_cps = cps;
        self.walltime_end = self.walltime_start + (ms + 500) / 1000;
    }
}

/// A GBX string: `u32 length` then the bytes.
fn gbx_string(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 + s.len());
    v.extend_from_slice(&(s.len() as u32).to_le_bytes());
    v.extend_from_slice(s.as_bytes());
    v
}

/// How to encode the map uid, which the engine stores as an interned **Id**
/// rather than as text.
///
/// A GBX `Id` (the "lookback string") is not a length-prefixed string: it is an
/// index into a per-file table, and a *new* string is written as a flagged
/// index followed by the text. The first `Id` in a file is preceded by a
/// version word. Which of those the ghost chunk wants is a fact about this game
/// build, so all of them are reachable and the server decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UidEnc {
    /// `u32 len, bytes` — a plain string.
    PlainString,
    /// `u32 3, u32 0x40000000, u32 len, bytes` — a new Id, with the version
    /// word that precedes the first Id in a file.
    IdWithVersion,
    /// `u32 0x40000000, u32 len, bytes` — a new Id, no version word.
    IdNoVersion,
}

const ID_NEW: u32 = 0x4000_0000;

impl UidEnc {
    fn encode(self, s: &str) -> Vec<u8> {
        let mut v = Vec::new();
        match self {
            UidEnc::PlainString => return gbx_string(s),
            UidEnc::IdWithVersion => {
                v.extend_from_slice(&3u32.to_le_bytes());
                v.extend_from_slice(&ID_NEW.to_le_bytes());
            }
            UidEnc::IdNoVersion => v.extend_from_slice(&ID_NEW.to_le_bytes()),
        }
        v.extend_from_slice(&gbx_string(s));
        v
    }
}

/// `u32 id, "PIKS", u32 size, payload`.
fn skippable(id: u32, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + payload.len());
    v.extend_from_slice(&id.to_le_bytes());
    v.extend_from_slice(gbx::SKIP_MAGIC);
    v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    v.extend_from_slice(payload);
    v
}

/// `u32 id, payload` — the parser must know this id.
fn inline(id: u32, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 + payload.len());
    v.extend_from_slice(&id.to_le_bytes());
    v.extend_from_slice(payload);
    v
}

/// Turn our per-tick [`Input`]s into the packet list the archive stores.
///
/// **Every packet is written explicitly** — an explicit state literal and
/// explicit vehicle fields — rather than using the format's "same as the
/// previous tick" shortcut. The shortcut is a space optimisation with a sharp
/// edge: a tick coded as "same" has no fields behind it, so a later edit to
/// that tick silently does nothing. A synthesized tape has no reason to want
/// that, and a search that later pokes tick N must be able to.
pub fn packets_for(inputs: &[Input]) -> Vec<Packet> {
    packets_for_state_flags(inputs, &[])
}

fn packets_for_state_flags(
    inputs: &[Input],
    state_flag_ranges: &[(usize, usize, u32)],
) -> Vec<Packet> {
    inputs
        .iter()
        .enumerate()
        .map(|(tick, i)| {
            // mode 2 = normal vehicle input, the mode a real run drives in.
            // The state literal's low nibble IS the mode; bit 31 is respawn.
            // Bits 5..26 encode the separate 22-bit state-flags field.
            let flags = state_flag_ranges
                .iter()
                .filter(|(start, end, _)| *start <= tick && tick < *end)
                .fold(0u32, |all, (_, _, value)| all | value);
            let mut lit: u64 = 2 | (((flags & 0x3f_ffff) as u64) << 5);
            if i.respawn {
                lit |= 1u64 << 31;
            }
            let (word0, flags) = gbx::tape::unpack_word_pub(lit);
            Packet {
                word0,
                flags,
                mode: word0 & 0xF,
                state: StateEnc::Lit(lit),
                mouse: None,
                vsame: false,
                steer: i.steer_raw() as u32,
                accel: i.gas as u32,
                brake: i.brake as u32,
                tri: None,
            }
        })
        .collect()
}

/// The `0x0309201D` chunk payload for a tape.
pub fn inputs_payload(inputs: &[Input], meta: &GhostMeta) -> Vec<u8> {
    let packets = packets_for_state_flags(inputs, &meta.state_flag_ranges);
    let archive = Archive {
        format_version: meta.format_version,
        field0: meta.field0,
        start_offset_ms: meta.start_offset_ms,
        packets,
        orig_bitstream_len: 0,
        orig_bits_used: 0,
        tail: Vec::new(),
        orig_bitstream: Vec::new(),
    };
    GbxTape {
        chunk_version: meta.input_chunk_version,
        archives: vec![archive],
    }
    .to_payload(Encoding::Explicit)
}

/// The `0x0309202D` validation-block payload.
///
/// Field widths are not guessed: each was read off the engine's own reader
/// calls in the chunk handler — three distinct 4-byte readers, a string reader,
/// and one reader whose immediate is `0x20`, i.e. a 32-byte blob.
fn validation_payload(meta: &GhostMeta) -> Vec<u8> {
    let mut v = Vec::new();
    // A non-zero leading flag makes the engine take an extra branch we have no
    // reason to enter. Zero keeps the block to exactly the fields below.
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&gbx_string(&meta.exe_version)); // -> +0x1C0
    v.extend_from_slice(&meta.exe_checksum.to_le_bytes()); // -> +0x1D0
    v.extend_from_slice(&meta.os_kind.to_le_bytes()); // -> +0x1D4
    v.extend_from_slice(&meta.cpu_kind.to_le_bytes()); // -> +0x1D8
    v.extend_from_slice(&meta.walltime_start.to_le_bytes()); // -> +0x1DC
    v.extend_from_slice(&meta.walltime_end.to_le_bytes()); // -> +0x1E0
    v.extend_from_slice(&gbx_string(&meta.title_id)); // -> +0x1E8, title id
    v.extend_from_slice(&meta.title_checksum); // -> +0x200, title checksum
    v.extend_from_slice(&meta.settings_flags.to_le_bytes()); // -> +0x220
    v.extend_from_slice(&meta.validation_start_index.to_le_bytes()); // -> +0x224, start checkpoint index
    v.extend_from_slice(&meta.validation_seed.to_le_bytes()); // -> +0x228
    v.extend_from_slice(&meta.validation_u04.to_le_bytes()); // -> +0x22C, unidentified
    v.extend_from_slice(&gbx_string(&meta.race_settings)); // -> +0x230
    v
}

/// The `0x0309202B` ghost-result payload.
fn result_payload(meta: &GhostMeta) -> Vec<u8> {
    let r = gbx::container::GhostResult {
        version: 1,
        race_ms: meta.declared_ms as i32,
        u01: 0,
        u02: 0,
        word4_unidentified: 0,
        entries: meta.declared_cps.iter().map(|&t| (t, 0)).collect(),
    };
    r.encode()
}

/// Read a map's uid out of the map file and build a container spec for it.
///
/// This is the entry point a consumer should use: it means a caller never
/// handles a uid, and a uid is the one field whose silent absence makes the
/// server skip the file with no output at all.
pub fn meta_for_map(map: &std::path::Path) -> Result<GhostMeta, String> {
    let data = std::fs::read(map).map_err(|e| format!("{}: {}", map.display(), e))?;
    let uid = gbx::map_uid_of(&data).ok_or_else(|| {
        format!(
            "{}: no map uid found. That is a harness limit -- this reader did not find \
             where the uid lives in this file -- not a statement that the file has none.",
            map.display()
        )
    })?;
    Ok(GhostMeta::probe(&uid))
}

/// Index of the semantic start in TM2020's validator checkpoint array.
///
/// The engine's array order is not file waypoint order. Causal U03 sweeps on
/// Summer 2026 - 01 mapped values 0, 1, 2, 3 and 4 respectively to its three
/// non-Spawn block gates, its Spawn block, and its item gate. Independent game
/// recordings carry U03=3 on that map, U03=2 on the two-block-gate test map,
/// and U03=1 on Training - 10 Long. Thus a block Spawn's index is exactly the
/// number of non-Spawn block waypoints. A missing/default zero selects gate zero
/// and starts validation from that checkpoint instead of from RoadTechStart.
pub fn validation_start_index_for_map(map: &std::path::Path) -> Result<u32, String> {
    let m = tmmaps::map::MapFile::load(map);
    let waypoints = m.waypoints();
    let starts: Vec<_> = waypoints
        .iter()
        .filter(|w| w.tag == "Spawn" && w.name == "RoadTechStart")
        .collect();
    if starts.len() != 1 {
        return Err(format!(
            "{}: expected exactly one semantic RoadTechStart, found {}",
            map.display(),
            starts.len()
        ));
    }
    if starts[0].kind != tmmaps::map::Kind::Block {
        return Err(format!(
            "{}: RoadTechStart is an item; validator checkpoint ordering for item starts is not established",
            map.display()
        ));
    }
    Ok(waypoints
        .iter()
        .filter(|w| w.kind == tmmaps::map::Kind::Block && w.tag != "Spawn")
        .count() as u32)
}

/// Metadata for the complete writer. Kept separate from [`meta_for_map`] so
/// legacy artifact-v1 reconstruction remains byte-identical.
pub fn complete_meta_for_map(map: &std::path::Path) -> Result<GhostMeta, String> {
    let mut meta = meta_for_map(map)?;
    meta.validation_start_index = validation_start_index_for_map(map)?;
    Ok(meta)
}

/// The authoritative initial transform encoded into a from-scratch record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InitialState {
    pub pos: [f32; 3],
    /// `(x, y, z, w)`, matching `gbx::recwrite::Xform`.
    pub quat: [f64; 4],
    pub vel: [f64; 3],
    pub roadtech_dir: Option<u8>,
}

/// One rung of the record ablation ladder. Each value adds exactly one
/// structural feature to the preceding value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordMode {
    /// The pre-existing tmauto container: no 0x03092000 parent and no record.
    None,
    /// A version-9 0x03092000 parent with a null RecordData node reference.
    Parent,
    /// A v11 RecordData node with the vehicle descriptor and no entity.
    Descriptor,
    /// The descriptor plus one empty controlled-car entity.
    Entity,
    /// The entity plus one 116-byte sample at t=0.
    Sample,
    /// A 50 ms grid through the declared horizon, initially repeating t=0.
    /// This is scaffolding for the authoritative validator-state recorder.
    Grid,
}

impl RecordMode {
    pub fn parse(s: &str) -> Option<RecordMode> {
        Some(match s {
            "none" => RecordMode::None,
            "parent" => RecordMode::Parent,
            "descriptor" => RecordMode::Descriptor,
            "entity" => RecordMode::Entity,
            "sample" => RecordMode::Sample,
            "grid" => RecordMode::Grid,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            RecordMode::None => "none",
            RecordMode::Parent => "parent",
            RecordMode::Descriptor => "descriptor",
            RecordMode::Entity => "entity",
            RecordMode::Sample => "sample",
            RecordMode::Grid => "grid",
        }
    }
}

/// `RoadTechStart` places the car 2.002 m above the grid cell origin. The 2 m
/// road deck is a model constant; the extra 0.002 m is the standing suspension
/// offset measured identically in game recordings on the two control maps.
pub const ROADTECH_START_LOCAL_Y: f32 = 2.002;

/// Read the semantic `Spawn` waypoint whose model is `RoadTechStart`.
///
/// The horizontal cell centre and four-way direction come directly from the
/// map. The vertical origin comes from the map's decoration id and a table of
/// decoration constants licensed by game-recorded tick-0 controls; then the
/// RoadTechStart model contributes its fixed 2.002 m local spawn height.
pub fn initial_state_for_map(map: &std::path::Path) -> Result<InitialState, String> {
    let m = tmmaps::map::MapFile::load(map);
    let w = m
        .waypoints()
        .into_iter()
        .find(|w| w.tag == "Spawn" && w.name == "RoadTechStart")
        .ok_or_else(|| {
            format!(
                "{}: no semantic RoadTechStart waypoint tagged Spawn",
                map.display()
            )
        })?;
    let (cx, cy, cz) = w.coords;
    let yoff = match m.decoration_id.as_str() {
        // Measured against game-recorded tick-0 samples on two independent
        // TM2020 maps. These are decoration properties, not per-run values.
        "Day64" => -40.0,
        "Day" => -120.0,
        "48x48Day" => -64.0,
        other => {
            return Err(format!(
                "{}: RoadTechStart uses decoration {other:?}, whose vertical origin is not in the \n                 from-scratch writer's constant table. This is an unimplemented map constant, not \n                 evidence that the map has no start.",
                map.display()
            ))
        }
    };
    let pos = match w.pos {
        Some(mut p) => {
            p[1] += ROADTECH_START_LOCAL_Y;
            p
        }
        None => [
            cx as f32 * 32.0 + 16.0,
            cy as f32 * 8.0 + yoff + ROADTECH_START_LOCAL_Y,
            cz as f32 * 32.0 + 16.0,
        ],
    };
    let yaw = match w.dir {
        Some(0) => 0.0,
        Some(1) => -std::f64::consts::FRAC_PI_2,
        Some(2) => std::f64::consts::PI,
        Some(3) => std::f64::consts::FRAC_PI_2,
        Some(d) => return Err(format!("RoadTechStart has invalid direction {d}")),
        None => w.yaw.unwrap_or(0.0) as f64,
    };
    let h = yaw * 0.5;
    Ok(InitialState {
        pos,
        quat: [0.0, h.sin(), 0.0, h.cos()],
        vel: [0.0; 3],
        roadtech_dir: w.dir,
    })
}

/// A 116-byte TM2020 `CSceneVehicleVis` sample built from named constants, the
/// map-derived transform, and tape tick 0.
fn first_vehicle_sample(initial: InitialState, input: Input, corrupt_x_m: f32) -> Vec<u8> {
    let mut d = vec![0u8; gbx::sample::SAMPLE_SIZE];
    // Offset-binary neutral side speed and suspension lengths.
    d[2..4].copy_from_slice(&32768u16.to_le_bytes());
    for i in [23usize, 25, 27, 29] {
        d[i] = 128;
    }
    // The three input-echo channels are completely determined by tape tick 0.
    d[14] = gbx::record::steer_byte(input.steer);
    d[15] = if input.gas { 255 } else { 0 };
    d[18] = if input.brake { 255 } else { 0 };
    // Grounded neutral render state. 32/33 are the documented offset-binary
    // slip constants; ground-mode bit 0 means ground contact; gear raw 5 means
    // first gear; simulation time coefficient 255 means 1.0.
    d[32] = 128;
    d[33] = 42;
    d[89] = 1;
    d[91] = 5;
    d[102] = 255;
    let mut x = initial;
    x.pos[0] += corrupt_x_m;
    write_transform(
        &mut d,
        gbx::sample::TRANSFORM.start,
        &Xform {
            pos: x.pos,
            quat: x.quat,
            vel: x.vel,
        },
    );
    d
}

fn from_scratch_record(
    inputs: &[Input],
    meta: &GhostMeta,
    initial: InitialState,
    mode: RecordMode,
    corrupt_x_m: f32,
) -> RecordData {
    let descs = vec![Desc {
        class_id: CLASS_CSCENEVEHICLEVIS,
        // CSceneVehicleVisState's vocabulary is 864 bytes in TM2020; schema 33
        // writes 116-byte deltas. These are format constants, not donor bytes.
        u01: 864,
        u02: 0,
        u03: 33,
        u04: Vec::new(),
        u05: 0,
    }];
    let ents = if mode == RecordMode::Descriptor {
        Vec::new()
    } else {
        let (times, raw, sample_size) = if matches!(mode, RecordMode::Sample | RecordMode::Grid) {
            let sample = first_vehicle_sample(
                initial,
                inputs.first().copied().unwrap_or(Input::NEUTRAL),
                corrupt_x_m,
            );
            if mode == RecordMode::Grid {
                let times: Vec<i32> = (0..=meta.declared_ms as i32).step_by(50).collect();
                let mut raw = Vec::with_capacity(times.len() * sample.len());
                for _ in &times {
                    raw.extend_from_slice(&sample);
                }
                (times, raw, gbx::sample::SAMPLE_SIZE)
            } else {
                (vec![0], sample, gbx::sample::SAMPLE_SIZE)
            }
        } else {
            (Vec::new(), Vec::new(), 0)
        };
        vec![Ent {
            type_: 0,
            // The engine's CSceneVehicleVis entity-kind word in TM2020 v11
            // records. It is a format constant observed across recordings; it
            // contains no driver or run data.
            u01: 0x0200_0006,
            u02: 0,
            u03: times.last().copied().unwrap_or(0),
            u04: 0,
            times,
            raw,
            sample_size,
            deltas2: Vec::new(),
        }]
    };
    RecordData {
        version: 11,
        start_ms: 0,
        end_ms: if mode == RecordMode::Sample {
            0
        } else {
            meta.declared_ms as i32
        },
        descs,
        notices: Vec::new(),
        ents,
        bulk_notices: Vec::new(),
        custom_modules: Vec::new(),
        bytes_consumed: 0,
        bytes_total: 0,
    }
}

/// Version 9 of `CGameCtnGhost` chunk 0x03092000, encoded from the public
/// schema. The parent-only ablation writes a null RecordData reference; later
/// rungs replace that one field with an inline node.
fn record_parent_payload(record: Option<&RecordData>) -> Vec<u8> {
    let mut v = Vec::new();
    let put_u32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    let put_string = |v: &mut Vec<u8>, s: &str| {
        v.extend_from_slice(&(s.len() as u32).to_le_bytes());
        v.extend_from_slice(s.as_bytes());
    };
    put_u32(&mut v, 9);
    put_u32(&mut v, 0); // appearance version: no extra appearance string
                        // Ident("CarSport", collection 10003, "Nadeo"). The first Id opens the
                        // lookback stream with version 3; both strings are fresh definitions.
    put_u32(&mut v, 3);
    put_u32(&mut v, 0x4000_0000);
    put_string(&mut v, "CarSport");
    put_u32(&mut v, 10003);
    put_u32(&mut v, 0x4000_0000);
    put_string(&mut v, "Nadeo");
    for x in [0.0f32, 0.0, 0.0] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    put_u32(&mut v, 0); // no skin PackDesc entries
    put_u32(&mut v, 0); // no badge
    put_string(&mut v, "TAS");
    put_string(&mut v, ""); // avatar
    put_string(&mut v, "tmauto-from-scratch");
    put_u32(&mut v, 0); // unnamed v4 boolean
    match record {
        Some(rd) => {
            put_u32(&mut v, 1); // the first non-root node index
            v.extend_from_slice(&encode_record_node(rd));
        }
        None => put_u32(&mut v, u32::MAX), // null RecordData node reference
    }
    put_u32(&mut v, 0); // trailing int array, empty
    put_string(&mut v, "TAS"); // v6 trigram
    put_string(&mut v, ""); // v7 zone
    put_string(&mut v, ""); // v8 club tag
    v
}

/// Synthesize a ghost with an independently encoded RecordData node.
///
/// This is separate from [`synthesize`] so the historical artifact-v1 byte
/// identity remains replayable. New callers use this function; the ablation
/// ladder can still request `RecordMode::None` to reproduce the old shape.
pub fn synthesize_complete(
    inputs: &[Input],
    meta: &GhostMeta,
    set: &ChunkSet,
    initial: InitialState,
    mode: RecordMode,
    corrupt_x_m: f32,
) -> Vec<u8> {
    if mode == RecordMode::None {
        return synthesize(inputs, meta, set);
    }
    let record = match mode {
        RecordMode::None | RecordMode::Parent => None,
        RecordMode::Descriptor | RecordMode::Entity | RecordMode::Sample | RecordMode::Grid => {
            Some(from_scratch_record(
                inputs,
                meta,
                initial,
                mode,
                corrupt_x_m,
            ))
        }
    };
    let mut with_nodes = set.clone();
    with_nodes.num_nodes = if record.is_some() { 2 } else { 1 };
    // Chunk 0x03092000 already opened the body-level Id stream for CarSport and
    // Nadeo. The later validation uid is therefore a fresh Id without another
    // version word; writing version 3 twice makes the server resolve no map.
    with_nodes.uid_enc = UidEnc::IdNoVersion;
    let base = synthesize(inputs, meta, &with_nodes);
    let g = gbx::Gbx::parse(&base);
    let mut body = skippable(CLASS_CGAMECTNGHOST, &record_parent_payload(record.as_ref()));
    body.extend_from_slice(&g.body);
    let mut out = g.header_bytes_u();
    out.extend_from_slice(&body);
    out
}

/// One-call complete writer: map + generated tape + documented constants.
pub fn write_complete_for(
    map: &std::path::Path,
    inputs: &[Input],
    min_ticks: usize,
    meta: &GhostMeta,
    mode: RecordMode,
    corrupt_x_m: f32,
    out: &std::path::Path,
) -> Result<Vec<u8>, String> {
    let initial = initial_state_for_map(map)?;
    let mut complete_meta = meta.clone();
    complete_meta.validation_start_index = validation_start_index_for_map(map)?;
    let padded = pad_to(inputs, min_ticks);
    let bytes = synthesize_complete(
        &padded,
        &complete_meta,
        &ChunkSet::ALL,
        initial,
        mode,
        corrupt_x_m,
    );
    std::fs::write(out, &bytes).map_err(|e| format!("{}: {}", out.display(), e))?;
    Ok(bytes)
}

/// Lengthen a tape so the archive outlives the run.
///
/// **The validator only simulates while the input archive lasts.** A tape
/// shorter than the run it is trying to produce stops early and the server
/// reports a DNF — which reads as *"the car did not finish"* when the truth is
/// *"the container ran out of tape"*. Those two are indistinguishable in the
/// verdict, so the length is not something to discover from a result.
///
/// Padding repeats the LAST input. That is not a neutral choice and it is the
/// right one: a car already past the finish line is not steered by anything,
/// and a neutral pad would lift the throttle before the line on any tape whose
/// length was underestimated.
pub fn pad_to(inputs: &[Input], ticks: usize) -> Vec<Input> {
    let mut v = inputs.to_vec();
    let last = v.last().copied().unwrap_or(Input::NEUTRAL);
    while v.len() < ticks {
        v.push(last);
    }
    v
}

/// Write a container for `tape` on `map`, padded to at least `min_ticks`.
///
/// The one-call form. Returns the bytes written.
pub fn write_for(
    map: &std::path::Path,
    inputs: &[Input],
    min_ticks: usize,
    out: &std::path::Path,
) -> Result<Vec<u8>, String> {
    let meta = meta_for_map(map)?;
    let bytes = synthesize(&pad_to(inputs, min_ticks), &meta, &ChunkSet::ALL);
    std::fs::write(out, &bytes).map_err(|e| format!("{}: {}", out.display(), e))?;
    Ok(bytes)
}

/// Which chunks to emit, and how. The synthesizer grows a container by varying
/// this set and asking the server what it thinks, so it is a parameter rather
/// than a constant — the shape of the experiment, not an implementation detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkSet {
    /// The container's class. `CGameCtnGhost` is the obvious reading of a file
    /// named `.Ghost.Gbx`; `CGameCtnReplayRecord` (`0x03093000`) is what a
    /// `.Replay.Gbx` is, and this project's `.Ghost.Gbx` files carry embedded
    /// maps, which only a replay does. Both are reachable from here because
    /// which one `/validatepath` wants is a question for the server, not for
    /// me.
    pub class_id: u32,
    pub login: bool,
    pub validate_uid: bool,
    pub racetime: bool,
    /// Chunk `0x03092014`, the ghost version. Without it the engine forces 6.
    pub version_chunk: bool,
    /// Chunk `0x0309202D`, the validation block (walltime + race settings).
    pub validation: bool,
    /// Emit the version and validation chunks with a `PIKS` marker and a size
    /// word. Which form the engine wants is measured, not assumed.
    pub new_chunks_skippable: bool,
    pub inputs: bool,
    pub result: bool,
    /// Emit `0x0309200F` / `0x03092010` with a `PIKS` marker and a size word,
    /// so a parser that does not know them can step over them. Inline is how
    /// `ghost::ident` reports finding them in the wild; skippable is the form
    /// that cannot kill a parse. Which one this game build wants is measurable.
    pub identity_skippable: bool,
    /// How the map uid is encoded. Measured against the server, not assumed.
    pub uid_enc: UidEnc,
    /// The `num_nodes` word.
    pub num_nodes: u32,
}

impl ChunkSet {
    /// Everything this writer knows how to emit, as a ghost.
    pub const ALL: ChunkSet = ChunkSet {
        class_id: CLASS_CGAMECTNGHOST,
        login: true,
        validate_uid: true,
        racetime: true,
        version_chunk: true,
        validation: true,
        new_chunks_skippable: true,
        inputs: true,
        result: true,
        identity_skippable: false,
        uid_enc: UidEnc::IdWithVersion,
        num_nodes: 1,
    };
    /// The tape and nothing else.
    pub const TAPE_ONLY: ChunkSet = ChunkSet {
        class_id: CLASS_CGAMECTNGHOST,
        login: false,
        validate_uid: false,
        racetime: false,
        version_chunk: true,
        validation: true,
        new_chunks_skippable: true,
        inputs: true,
        result: false,
        identity_skippable: false,
        uid_enc: UidEnc::IdWithVersion,
        num_nodes: 1,
    };
}

/// Write a whole `.Ghost.Gbx`.
///
/// Chunks are emitted in ascending id order, which is the order a GBX body is
/// written in.
pub fn synthesize(inputs: &[Input], meta: &GhostMeta, set: &ChunkSet) -> Vec<u8> {
    let ident = |id: u32, payload: &[u8]| -> Vec<u8> {
        if set.identity_skippable {
            skippable(id, payload)
        } else {
            inline(id, payload)
        }
    };
    let mut body = Vec::new();
    if set.racetime {
        body.extend_from_slice(&skippable(CHUNK_RACETIME, &meta.declared_ms.to_le_bytes()));
    }
    if set.login {
        body.extend_from_slice(&ident(CHUNK_LOGIN, &gbx_string(&meta.login)));
    }
    if set.validate_uid {
        body.extend_from_slice(&ident(
            CHUNK_VALIDATE_UID,
            &set.uid_enc.encode(&meta.map_uid),
        ));
    }
    let newc = |id: u32, p: &[u8]| -> Vec<u8> {
        if set.new_chunks_skippable {
            skippable(id, p)
        } else {
            inline(id, p)
        }
    };
    if set.version_chunk {
        body.extend_from_slice(&newc(CHUNK_VERSION, &meta.game_version.to_le_bytes()));
    }
    if set.inputs {
        body.extend_from_slice(&skippable(CHUNK_INPUTS, &inputs_payload(inputs, meta)));
    }
    if set.result {
        body.extend_from_slice(&skippable(CHUNK_RESULT, &result_payload(meta)));
    }
    if set.validation {
        body.extend_from_slice(&newc(CHUNK_VALIDATION, &validation_payload(meta)));
    }
    body.extend_from_slice(&FACADE.to_le_bytes());

    let mut out = Vec::with_capacity(64 + body.len());
    out.extend_from_slice(b"GBX");
    out.extend_from_slice(&6u16.to_le_bytes()); // version
    out.push(b'B'); // binary
    out.push(b'U'); // reference table uncompressed
    out.push(b'U'); // BODY UNCOMPRESSED
    out.push(b'R'); // version >= 4
    out.extend_from_slice(&set.class_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // user_data length: none
    out.extend_from_slice(&set.num_nodes.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // reference table: no externals
    out.extend_from_slice(&body);
    out
}

/// Synthesize straight from one of our own [`InputTape`]s.
pub fn synthesize_tape(tape: &InputTape, meta: &GhostMeta, set: &ChunkSet) -> Vec<u8> {
    synthesize(&tape.inputs, meta, set)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> GhostMeta {
        GhostMeta::probe("SomeMapUid1234567890abcde")
    }

    /// The output must parse as a GBX container with the class we asked for.
    /// This is a round-trip through the crate's INDEPENDENT reader — the reader
    /// was written for real files and knows nothing about this writer.
    #[test]
    fn the_output_parses_as_a_ghost() {
        let bytes = synthesize(&[Input::FULL_GAS; 10], &meta(), &ChunkSet::ALL);
        let g = gbx::Gbx::parse(&bytes);
        assert_eq!(g.class_id, CLASS_CGAMECTNGHOST);
        assert_eq!(g.num_nodes, 1);
        assert!(g.user_data.is_empty());
    }

    /// The tape we wrote must be the tape the reader finds. Both operands come
    /// out of the world: one from our input list, one from parsing our own
    /// bytes back with the crate's decoder.
    #[test]
    fn the_tape_round_trips_through_the_readers_decoder() {
        let inputs: Vec<Input> = (0..50)
            .map(|k| Input {
                steer: (k as i8).wrapping_mul(3).max(-127),
                gas: k % 3 != 0,
                brake: k % 7 == 0,
                respawn: k == 11,
            })
            .collect();
        let bytes = synthesize(&inputs, &meta(), &ChunkSet::ALL);
        let g = gbx::Gbx::parse(&bytes);
        let t = GbxTape::from_body(&g.body).expect("the input chunk must decode");
        assert_eq!(t.archives.len(), 1);
        let a = &t.archives[0];
        assert_eq!(a.packets.len(), inputs.len());
        for (k, (p, i)) in a.packets.iter().zip(&inputs).enumerate() {
            assert_eq!(p.steer_i8(), i.steer, "steer at tick {}", k);
            assert_eq!(p.accel == 1, i.gas, "gas at tick {}", k);
            assert_eq!(p.brake == 1, i.brake, "brake at tick {}", k);
            assert_eq!(p.respawn(), i.respawn, "respawn at tick {}", k);
        }
    }

    #[test]
    fn state_flag_ranges_land_only_on_the_named_half_open_ticks() {
        let mut m = meta();
        m.state_flag_ranges.push((2, 5, 0x404));
        let bytes = synthesize(&[Input::FULL_GAS; 7], &m, &ChunkSet::ALL);
        let g = gbx::Gbx::parse(&bytes);
        let t = GbxTape::from_body(&g.body).expect("the input chunk must decode");
        let flags: Vec<u32> = t.archives[0].packets.iter().map(|p| p.flags).collect();
        assert_eq!(flags, vec![0, 0, 0x404, 0x404, 0x404, 0, 0]);
        assert_eq!(t.archives[0].packets[2].state, StateEnc::Lit(0x8082));
    }

    /// The declared fields must read back as declared. A container whose
    /// declaration silently did not land is the defect behind five maps'
    /// corrupted objectives.
    #[test]
    fn the_declaration_reads_back() {
        let mut m = meta();
        m.declared_ms = 43_079;
        m.declared_cps = vec![10_000, 25_500, 43_079];
        let bytes = synthesize(&[Input::FULL_GAS; 10], &m, &ChunkSet::ALL);
        std::fs::create_dir_all("/tmp/tmauto-synth-test").unwrap();
        let p = "/tmp/tmauto-synth-test/decl.Ghost.Gbx";
        std::fs::write(p, &bytes).unwrap();
        let c = gbx::Container::load(p).unwrap();
        assert_eq!(c.declared_times(), vec![(c.declared_times()[0].0, 43_079)]);
        assert_eq!(c.splits(), vec![10_000, 25_500, 43_079]);
    }

    /// Two different tapes must produce different bytes, and the same tape the
    /// same bytes. The writer is a function of its inputs and nothing else —
    /// no timestamp, no randomness, nothing that would make a container
    /// unreproducible.
    #[test]
    fn the_writer_is_deterministic_and_injective() {
        let a = synthesize(&[Input::FULL_GAS; 10], &meta(), &ChunkSet::ALL);
        let b = synthesize(&[Input::FULL_GAS; 10], &meta(), &ChunkSet::ALL);
        assert_eq!(a, b, "the same tape must produce the same bytes");
        let mut v = vec![Input::FULL_GAS; 10];
        v[4].brake = true;
        assert_ne!(a, synthesize(&v, &meta(), &ChunkSet::ALL));
    }

    #[test]
    fn a_smaller_chunk_set_is_a_smaller_file() {
        let all = synthesize(&[Input::FULL_GAS; 10], &meta(), &ChunkSet::ALL);
        let min = synthesize(&[Input::FULL_GAS; 10], &meta(), &ChunkSet::TAPE_ONLY);
        assert!(min.len() < all.len());
        assert_eq!(gbx::Gbx::parse(&min).class_id, CLASS_CGAMECTNGHOST);
    }

    #[test]
    fn validation_seed_and_settings_reach_the_named_validation_fields() {
        let mut m = meta();
        m.validation_seed = 32_611_514;
        m.validation_start_index = 3;
        m.validation_u04 = 16;
        m.title_id = "Trackmania".into();
        m.title_checksum = [0xab; 32];
        m.race_settings = "from-scratch-control".into();
        let bytes = synthesize(&[Input::FULL_GAS; 10], &m, &ChunkSet::ALL);
        let g = gbx::Gbx::parse(&bytes);
        let v = gbx::manifest::validation_manifest(&g.body).expect("validation chunk");
        assert!(v.contains("\"validation_seed\":32611514"));
        assert!(v.contains("\"u03\":3"));
        assert!(v.contains("\"u04\":16"));
        assert!(v.contains("\"title_id\":\"Trackmania\""));
        assert!(v.contains(&format!("\"title_checksum_hex\":\"{}\"", "ab".repeat(32))));
        assert!(v.contains("\"race_settings\":\"from-scratch-control\""));
    }

    #[test]
    fn validation_start_index_follows_the_engine_checkpoint_order() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let map = root.join("testdata/map2.Map.Gbx");
        // This fixture has two non-Spawn block gates, then a block Spawn, then
        // an item gate. The game-written controls carry U03=2.
        assert_eq!(validation_start_index_for_map(&map).unwrap(), 2);
        assert_eq!(
            complete_meta_for_map(&map).unwrap().validation_start_index,
            2
        );
    }

    #[test]
    fn map_derived_roadtech_start_matches_an_independent_game_recording() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let map = root.join("testdata/map2.Map.Gbx");
        let ghost = root.join("testdata/human_22730.Ghost.Gbx");
        let initial = initial_state_for_map(&map).expect("semantic start");
        let recorded =
            gbx::record::decode_ghost(&ghost.to_string_lossy()).expect("recorded control");
        let first = recorded.samples.first().expect("first sample");
        assert!((initial.pos[0] as f64 - first.x as f64).abs() < 0.01);
        assert!((initial.pos[1] as f64 - first.y as f64).abs() < 0.01);
        assert!((initial.pos[2] as f64 - first.z as f64).abs() < 0.01);
        assert_eq!(initial.roadtech_dir, Some(0));
    }

    #[test]
    fn grid_mode_creates_the_full_50ms_scaffold_without_a_donor() {
        let mut m = meta();
        m.set_declared(200, vec![200]);
        let initial = InitialState {
            pos: [1.0, 2.0, 3.0],
            quat: [0.0, 0.0, 0.0, 1.0],
            vel: [0.0; 3],
            roadtech_dir: Some(0),
        };
        let bytes = synthesize_complete(
            &[Input::FULL_GAS; 20],
            &m,
            &ChunkSet::ALL,
            initial,
            RecordMode::Grid,
            0.0,
        );
        let d = gbx::record::decode_body(&gbx::Gbx::parse(&bytes).body, "grid").unwrap();
        let times: Vec<i32> = d.samples.iter().map(|s| s.time_ms).collect();
        assert_eq!(times, vec![0, 50, 100, 150, 200]);
        assert!(d.raw_samples().all(|s| s == d.raw_sample(0).unwrap()));
    }

    #[test]
    fn the_complete_writer_has_one_live_116_byte_car_at_the_requested_transform() {
        let initial = InitialState {
            pos: [10.25, 20.5, 30.75],
            quat: [0.0, 0.0, 0.0, 1.0],
            vel: [0.0; 3],
            roadtech_dir: Some(0),
        };
        let bytes = synthesize_complete(
            &[Input::FULL_GAS; 10],
            &meta(),
            &ChunkSet::ALL,
            initial,
            RecordMode::Sample,
            0.0,
        );
        let g = gbx::Gbx::parse(&bytes);
        assert_eq!(g.num_nodes, 2);
        let d = gbx::record::decode_body(&g.body, "synthetic").expect("record decodes");
        assert_eq!(d.version, 11);
        assert_eq!(d.ents.len(), 1);
        assert_eq!(d.samples.len(), 1);
        assert_eq!(d.sample_size, 116);
        assert_eq!(d.samples[0].time_ms, 0);
        assert!((d.samples[0].x - 10.25).abs() < 1e-6);
        assert!((d.samples[0].y - 20.5).abs() < 1e-6);
        assert!((d.samples[0].z - 30.75).abs() < 1e-6);
    }

    #[test]
    fn corrupting_the_initial_field_moves_exactly_that_field() {
        let initial = InitialState {
            pos: [10.0, 20.0, 30.0],
            quat: [0.0, 0.0, 0.0, 1.0],
            vel: [0.0; 3],
            roadtech_dir: Some(0),
        };
        let good = synthesize_complete(
            &[Input::FULL_GAS],
            &meta(),
            &ChunkSet::ALL,
            initial,
            RecordMode::Sample,
            0.0,
        );
        let bad = synthesize_complete(
            &[Input::FULL_GAS],
            &meta(),
            &ChunkSet::ALL,
            initial,
            RecordMode::Sample,
            64.0,
        );
        let a = gbx::record::decode_body(&gbx::Gbx::parse(&good).body, "good").unwrap();
        let b = gbx::record::decode_body(&gbx::Gbx::parse(&bad).body, "bad").unwrap();
        assert!((b.samples[0].x - a.samples[0].x - 64.0).abs() < 1e-6);
        assert_eq!(a.samples[0].y, b.samples[0].y);
        assert_eq!(a.samples[0].z, b.samples[0].z);
    }
}
