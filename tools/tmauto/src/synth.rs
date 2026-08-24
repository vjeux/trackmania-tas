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
pub const CHUNK_INPUTS: u32 = 0x0309201D;
pub const CHUNK_RESULT: u32 = 0x0309202B;

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
    /// Archive header word whose meaning is not established here. Named
    /// `field0` in `gbx::tape` for the same reason.
    pub field0: u32,
}

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
            field0: 0,
        }
    }
}

/// A GBX string: `u32 length` then the bytes.
fn gbx_string(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 + s.len());
    v.extend_from_slice(&(s.len() as u32).to_le_bytes());
    v.extend_from_slice(s.as_bytes());
    v
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
    inputs
        .iter()
        .map(|i| {
            // mode 2 = normal vehicle input, the mode a real run drives in.
            // The state literal's low nibble IS the mode; bit 31 is respawn.
            let mut lit: u64 = 2;
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
    let packets = packets_for(inputs);
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
    GbxTape { chunk_version: 2, archives: vec![archive] }.to_payload(Encoding::Explicit)
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
    pub inputs: bool,
    pub result: bool,
    /// Emit `0x0309200F` / `0x03092010` with a `PIKS` marker and a size word,
    /// so a parser that does not know them can step over them. Inline is how
    /// `ghost::ident` reports finding them in the wild; skippable is the form
    /// that cannot kill a parse. Which one this game build wants is measurable.
    pub identity_skippable: bool,
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
        inputs: true,
        result: true,
        identity_skippable: false,
        num_nodes: 1,
    };
    /// The tape and nothing else.
    pub const TAPE_ONLY: ChunkSet = ChunkSet {
        class_id: CLASS_CGAMECTNGHOST,
        login: false,
        validate_uid: false,
        racetime: false,
        inputs: true,
        result: false,
        identity_skippable: false,
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
        body.extend_from_slice(&ident(CHUNK_VALIDATE_UID, &gbx_string(&meta.map_uid)));
    }
    if set.inputs {
        body.extend_from_slice(&skippable(CHUNK_INPUTS, &inputs_payload(inputs, meta)));
    }
    if set.result {
        body.extend_from_slice(&skippable(CHUNK_RESULT, &result_payload(meta)));
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
}
