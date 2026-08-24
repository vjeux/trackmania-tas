//! `Tape` — the input sequence, and the `PROV` record that says where it came
//! from.
//!
//! A tape is what the explorer and the polisher produce, and it is the ONLY
//! thing they produce. Assembling one into a `.Ghost.Gbx` the dedicated server
//! will read is agent A's job and nobody else's, so there is exactly one writer
//! of containers and exactly one place a human recording could get in.

use crate::sha::sha256;
use crate::verdict::TapeHash;

/// One tick of input. The tick index is the position in [`Tape::inputs`].
///
/// `steer` is the signed value the game reads: `0` centre, `127` full right,
/// `-127` full left. (`-128` is not reachable through the 8-bit field the way
/// the game decodes it, so the constructors clamp to ±127 rather than letting a
/// value exist that cannot round-trip.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Input {
    pub steer: i8,
    pub gas: bool,
    pub brake: bool,
    pub respawn: bool,
}

impl Input {
    pub const NEUTRAL: Input = Input { steer: 0, gas: false, brake: false, respawn: false };

    /// Full throttle, straight ahead.
    pub const FULL_GAS: Input = Input { steer: 0, gas: true, brake: false, respawn: false };

    pub fn new(steer: i8, gas: bool, brake: bool) -> Input {
        Input { steer: steer.max(-127), gas, brake, respawn: false }
    }

    /// The raw 8-bit field the input archive stores.
    pub fn steer_raw(self) -> u8 {
        self.steer as u8
    }

    /// Nine bytes that identify this input exactly, for hashing. Fixed width so
    /// that no two different inputs can serialise to the same bytes.
    fn hash_bytes(self) -> [u8; 4] {
        [self.steer as u8, self.gas as u8, self.brake as u8, self.respawn as u8]
    }
}

/// Which component produced a tape. A free string would let a typo root a
/// chain; an enum cannot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Producer {
    /// The container synthesizer. **The only legal root of a provenance
    /// chain.** A tape produced here has no parent by definition.
    Synthesizer,
    /// Agent C's archive search.
    Explorer,
    /// Agent D's savestate tree.
    SavestateTree,
    /// The existing mutation search, `tmsearch`.
    Polisher,
    /// A test fixture. Roots nothing: chain-rooting requires `Synthesizer`.
    TestFixture,
}

impl Producer {
    pub fn as_str(self) -> &'static str {
        match self {
            Producer::Synthesizer => "synthesizer",
            Producer::Explorer => "explorer",
            Producer::SavestateTree => "savestate-tree",
            Producer::Polisher => "polisher",
            Producer::TestFixture => "test-fixture",
        }
    }
    pub fn parse(s: &str) -> Option<Producer> {
        Some(match s {
            "synthesizer" => Producer::Synthesizer,
            "explorer" => Producer::Explorer,
            "savestate-tree" => Producer::SavestateTree,
            "polisher" => Producer::Polisher,
            "test-fixture" => Producer::TestFixture,
            _ => return None,
        })
    }
    /// Only the synthesizer roots a chain.
    pub fn is_root(self) -> bool {
        matches!(self, Producer::Synthesizer)
    }
}

/// The `PROV` record: where this tape came from.
///
/// Note what this is NOT. It is a **claim** the producer writes, and a claim is
/// not a check — this project has already read a manifest line saying "header
/// inherited from the carrier" as provenance when it was a finding. The gate in
/// [`crate::gate`] is what turns these claims into a verdict, by walking them
/// to a root and refusing when the walk does not reach one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prov {
    pub producer: Producer,
    /// The tape this one was derived from. `None` only for a root.
    pub parent: Option<TapeHash>,
    pub seed: u64,
    pub timestamp_unix: i64,
    /// The map this tape is for, by uid. Carried so a tape cannot be silently
    /// evaluated against the wrong map — "the editor is open" is not "the map I
    /// want is open".
    pub map_uid: String,
}

impl Prov {
    /// A root record: produced by the synthesizer, with no parent.
    pub fn root(seed: u64, map_uid: &str) -> Prov {
        Prov {
            producer: Producer::Synthesizer,
            parent: None,
            seed,
            timestamp_unix: now_unix(),
            map_uid: map_uid.to_string(),
        }
    }

    /// A child record.
    pub fn child(producer: Producer, parent: TapeHash, seed: u64, map_uid: &str) -> Prov {
        Prov {
            producer,
            parent: Some(parent),
            seed,
            timestamp_unix: now_unix(),
            map_uid: map_uid.to_string(),
        }
    }

    /// The record's on-disk form: one line, tab-separated, greppable.
    pub fn to_line(&self) -> String {
        format!(
            "PROV\t1\t{}\t{}\t{}\t{}\t{}",
            self.producer.as_str(),
            self.parent.map(|h| h.hex()).unwrap_or_else(|| "-".into()),
            self.seed,
            self.timestamp_unix,
            self.map_uid
        )
    }

    pub fn parse_line(line: &str) -> Option<Prov> {
        let f: Vec<&str> = line.trim_end().split('\t').collect();
        if f.len() != 7 || f[0] != "PROV" || f[1] != "1" {
            return None;
        }
        Some(Prov {
            producer: Producer::parse(f[2])?,
            parent: if f[3] == "-" { None } else { Some(TapeHash::from_hex(f[3])?) },
            seed: f[4].parse().ok()?,
            timestamp_unix: f[5].parse().ok()?,
            map_uid: f[6].to_string(),
        })
    }
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// An input sequence plus its provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tape {
    /// One entry per 10 ms tick, tick 0 first.
    pub inputs: Vec<Input>,
    pub prov: Prov,
}

impl Tape {
    pub fn new(inputs: Vec<Input>, prov: Prov) -> Tape {
        Tape { inputs, prov }
    }

    pub fn ticks(&self) -> usize {
        self.inputs.len()
    }

    /// How long this tape covers, in milliseconds.
    pub fn span_ms(&self) -> u32 {
        self.inputs.len() as u32 * 10
    }

    /// The tape's identity: a hash of the INPUTS ONLY.
    ///
    /// Deliberately not of the provenance record. Two searches that arrive at
    /// the same input sequence by different routes produced the same tape, and
    /// the oracle will give them the same answer; a hash that folded in the
    /// seed or the timestamp would call them different and quietly defeat every
    /// duplicate check downstream.
    pub fn hash(&self) -> TapeHash {
        let mut buf = Vec::with_capacity(4 + self.inputs.len() * 4);
        buf.extend_from_slice(&(self.inputs.len() as u32).to_le_bytes());
        for i in &self.inputs {
            buf.extend_from_slice(&i.hash_bytes());
        }
        TapeHash(sha256(&buf))
    }

    /// Where this tape first differs from `other`, and how many ticks differ.
    /// A tape that is a strict extension of another differs from the end of the
    /// shorter one onward.
    pub fn distance_from(&self, other: &Tape) -> crate::verdict::ForkDistance {
        let n = self.inputs.len().max(other.inputs.len());
        let mut first = None;
        let mut count = 0u32;
        for t in 0..n {
            let a = self.inputs.get(t);
            let b = other.inputs.get(t);
            if a != b {
                if first.is_none() {
                    first = Some(t as u32);
                }
                count += 1;
            }
        }
        crate::verdict::ForkDistance { first_differing_tick: first, differing_ticks: count }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(inputs: Vec<Input>) -> Tape {
        Tape::new(inputs, Prov::root(0, "TestMapUid"))
    }

    #[test]
    fn hash_is_of_the_inputs_only() {
        let a = Tape::new(vec![Input::FULL_GAS; 3], Prov::root(1, "M"));
        let mut b = Tape::new(vec![Input::FULL_GAS; 3], Prov::root(999, "M"));
        b.prov.timestamp_unix += 10_000;
        assert_eq!(a.hash(), b.hash(), "the same inputs are the same tape");
    }

    /// The negative half: a one-tick change must move the hash. Without this,
    /// "the same inputs are the same tape" is satisfied by a hash of nothing.
    #[test]
    fn one_tick_changes_the_hash() {
        let a = t(vec![Input::FULL_GAS; 3]);
        let mut v = vec![Input::FULL_GAS; 3];
        v[1].steer = 1;
        assert_ne!(a.hash(), t(v).hash());
    }

    /// A length change must move the hash even when the shared prefix is
    /// identical — the length word in front is what guarantees it.
    #[test]
    fn a_longer_tape_is_a_different_tape() {
        assert_ne!(t(vec![Input::FULL_GAS; 3]).hash(), t(vec![Input::FULL_GAS; 4]).hash());
    }

    #[test]
    fn distance_finds_the_first_difference() {
        let a = t(vec![Input::FULL_GAS; 5]);
        let mut v = vec![Input::FULL_GAS; 5];
        v[3].brake = true;
        let d = a.distance_from(&t(v));
        assert_eq!(d.first_differing_tick, Some(3));
        assert_eq!(d.differing_ticks, 1);
    }

    #[test]
    fn an_extension_differs_from_where_it_extends() {
        let a = t(vec![Input::FULL_GAS; 3]);
        let b = t(vec![Input::FULL_GAS; 5]);
        let d = b.distance_from(&a);
        assert_eq!(d.first_differing_tick, Some(3));
        assert_eq!(d.differing_ticks, 2);
        assert!(d.is_forward_only(2));
    }

    #[test]
    fn prov_lines_round_trip() {
        let r = Prov::root(42, "abcDEF123");
        assert_eq!(Prov::parse_line(&r.to_line()), Some(r));
        let c = Prov::child(Producer::Explorer, TapeHash([7; 32]), 9, "abcDEF123");
        assert_eq!(Prov::parse_line(&c.to_line()), Some(c));
        assert_eq!(Prov::parse_line("PROV\t1\tnonsense\t-\t0\t0\tM"), None);
        assert_eq!(Prov::parse_line("not a prov line"), None);
    }

    #[test]
    fn only_the_synthesizer_roots() {
        assert!(Producer::Synthesizer.is_root());
        for p in [
            Producer::Explorer,
            Producer::SavestateTree,
            Producer::Polisher,
            Producer::TestFixture,
        ] {
            assert!(!p.is_root(), "{:?} must not root a chain", p);
        }
    }
}
