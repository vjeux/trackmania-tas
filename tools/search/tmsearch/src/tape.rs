//! The candidate writer: one base image, and a handful of bit patches per
//! candidate.
//!
//! # This is the search's only contact with the file format
//!
//! `tools/ghost` owns the `0x0309201D` codec. Nothing here parses or writes a
//! packet. What this module does is turn the codec's output into something a
//! search can afford to call 26 000 times a second per core:
//!
//! 1. Ask `ghost` for the whole file with every vehicle field written
//!    **explicitly** (`Encoding::Explicit`). That fixes the bit layout: every
//!    tick then has its own bits at a position that does not depend on any
//!    other tick's values.
//! 2. Find where each tick's bits are by **probing the encoder** -- flip every
//!    steer field, ask `ghost` to encode again, and read the positions off the
//!    difference. Three encodes, once, at startup.
//! 3. A candidate is then `memcpy` of the base image plus one 8-bit and two
//!    1-bit patches per tick.
//!
//! Step 2 is the point. The obvious alternative is to reimplement the encoder's
//! bit arithmetic here, which is exactly the second copy of the codec that this
//! rebuild exists to delete: the search used to carry one, and it had a defect
//! nobody could see (below). Probing cannot drift from the encoder, because the
//! encoder is what it measures. `tests/patcher.rs` closes the loop by asserting
//! that a patched image equals a full re-encode of the same inputs, byte for
//! byte, on random states.
//!
//! # The defect the old copy had, and why this one cannot have it
//!
//! The search's own writer emitted a mode-12 "same as the previous tick" packet
//! as a single bit and marked the tick `Slot::FROZEN`. A write to such a tick
//! was **silently dropped** -- the search could not express those candidates and
//! nothing said so. It never bit only because the affected ticks sat below
//! every resume boundary in use.
//!
//! `Encoding::Explicit` expands those packets, so most of them become
//! ordinary patchable ticks. The ones that genuinely cannot take an 8-bit
//! steering value -- a 32-bit steer field (modes 12 and 13), a packet with no
//! vehicle fields at all (mode 0), a trigger-only packet -- are listed in
//! [`Patcher::unwritable`], printed at startup, and [`Patcher::check_window`]
//! **refuses to search a window containing one**. A limit that is stated is a
//! task; a limit that is silent is a defect.

use forkoracle::inputs::Inputs;
use ghost::container::Container;
use ghost::tape::{Encoding, Tape};

/// Where one tick's inputs live in the base image, as absolute bit positions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    /// An 8-bit steering field with its gas and brake bits: fully writable.
    Analog { steer: usize, gas: usize, brake: usize },
    /// This tick's packet has no 8-bit steering field. `why` names the reason.
    Unwritable { why: &'static str },
}

pub struct Patcher {
    /// The complete candidate file for the template's own inputs, body
    /// uncompressed so it can be patched in place.
    pub base: Vec<u8>,
    slots: Vec<Slot>,
    /// The template's own inputs -- the search's starting point and the
    /// reference every reported distance is measured against.
    pub template: Inputs,
    /// Race time of tick 0. **Check this before interpreting any tick-0 edit**:
    /// most incumbents in this project are countdown-prefixed, so writing
    /// `gas = false` at race 0 there is a one-tick lift inside an existing
    /// hold, not a change of throttle onset.
    pub start_offset_ms: i32,
    /// What the file DECLARES. Never a result: a synthesised tape carries its
    /// template's header, so this is the donor's number until something writes
    /// it. `ghost declare --from-oracle` is what writes it.
    pub declared_ms: Option<i64>,
    /// Ticks this search cannot write, with the reason.
    pub unwritable: Vec<(usize, &'static str)>,
}

impl Patcher {
    pub fn n(&self) -> usize {
        self.slots.len()
    }

    /// Build from a template file.
    pub fn build(path: &str) -> Result<Patcher, String> {
        let c = Container::load(path)?;
        let tape = Tape::from_file(path)?;
        // The codec's own control, on the way past: a verbatim re-encode of an
        // unedited tape must reproduce the file's bitstream exactly. If it does
        // not, nothing downstream is worth running.
        tape.verbatim_is_identity()
            .map_err(|e| format!("{}: the codec does not round-trip this file: {}", path, e))?;

        let base = tape.inject_into(&c, Encoding::Explicit)?;
        let a0 = tape.archives.first().ok_or("no input archive in the template")?;
        let n = a0.packets.len();

        let mut slots = vec![Slot::Unwritable { why: "not probed" }; n];
        let mut unwritable = Vec::new();

        // Which ticks CAN carry an 8-bit analog triple, from the packet modes
        // the codec decoded. Modes 2 and 4 are the analog vehicle packet.
        let analog: Vec<usize> = (0..n).filter(|&i| matches!(a0.packets[i].mode, 2 | 4)).collect();
        for i in 0..n {
            if !analog.contains(&i) {
                let why = match a0.packets[i].mode {
                    12 | 13 => "32-bit steering field",
                    0 => "packet carries no vehicle fields",
                    _ => "trigger-only packet",
                };
                slots[i] = Slot::Unwritable { why };
                unwritable.push((i, why));
            }
        }

        // Probe the encoder. Flipping every analog steer field at once gives
        // one run of exactly 8 changed bits per analog tick, in tick order,
        // separated by the bits that did not change -- so the positions can be
        // read straight off the difference, and any surprise is a hard error
        // rather than a guess.
        let steer_bits = probe(&tape, &c, &base, &analog, Field::Steer, 8)?;
        let gas_bits = probe(&tape, &c, &base, &analog, Field::Gas, 1)?;
        let brake_bits = probe(&tape, &c, &base, &analog, Field::Brake, 1)?;

        for (k, &i) in analog.iter().enumerate() {
            slots[i] = Slot::Analog { steer: steer_bits[k], gas: gas_bits[k], brake: brake_bits[k] };
        }

        let template = Inputs {
            steer: a0.packets.iter().map(|p| p.steer_i8()).collect(),
            gas: a0.packets.iter().map(|p| p.accel != 0).collect(),
            brake: a0.packets.iter().map(|p| p.brake != 0).collect(),
        };

        let p = Patcher {
            base,
            slots,
            template,
            start_offset_ms: a0.start_offset_ms,
            declared_ms: c.declared_times().first().map(|t| t.1 as i64),
            unwritable,
        };

        // The identity control: the template's own inputs, patched into the
        // base image, must give the base image back.
        let mut buf = p.base.clone();
        p.apply(&mut buf, &p.template);
        if buf != p.base {
            return Err(format!("{}: patching the template's own inputs changed the file", path));
        }
        Ok(p)
    }

    /// Write `inputs` into `buf`, which must be a clone of [`Patcher::base`].
    #[inline]
    pub fn apply(&self, buf: &mut [u8], inputs: &Inputs) {
        for (i, s) in self.slots.iter().enumerate() {
            if let Slot::Analog { steer, gas, brake } = *s {
                patch_bits(buf, steer, inputs.steer[i] as u8 as u64, 8);
                patch_bits(buf, gas, inputs.gas[i] as u64, 1);
                patch_bits(buf, brake, inputs.brake[i] as u64, 1);
            }
        }
    }

    /// A complete candidate file for these inputs.
    pub fn file(&self, inputs: &Inputs) -> Vec<u8> {
        let mut b = self.base.clone();
        self.apply(&mut b, inputs);
        b
    }

    /// Refuse a search window that contains a tick this search cannot write.
    ///
    /// The alternative -- carrying on and dropping those writes -- is a search
    /// that cannot express certain candidates while reporting that it explored
    /// them.
    pub fn check_window(&self, lo: usize, hi: usize) -> Result<(), String> {
        let bad: Vec<String> = self
            .unwritable
            .iter()
            .filter(|(t, _)| *t >= lo && *t < hi)
            .map(|(t, w)| format!("tick {} ({})", t, w))
            .collect();
        if bad.is_empty() {
            return Ok(());
        }
        Err(format!(
            "the search window [{}, {}) contains {} tick(s) this search cannot write: {}. \
             Move the window, or teach the writer that packet mode -- do not search \
             a window whose edits are silently dropped.",
            lo,
            hi,
            bad.len(),
            bad.join(", ")
        ))
    }
}

enum Field {
    Steer,
    Gas,
    Brake,
}

/// Encode the tape once with `field` flipped on every tick in `at`, and read
/// the bit position of each field off the difference from `base`.
fn probe(
    tape: &Tape,
    c: &Container,
    base: &[u8],
    at: &[usize],
    field: Field,
    width: usize,
) -> Result<Vec<usize>, String> {
    let mut t = tape.clone();
    {
        let a0 = t.archives.first_mut().ok_or("no input archive")?;
        for &i in at {
            let p = &mut a0.packets[i];
            match field {
                Field::Steer => p.steer ^= 0xFF,
                Field::Gas => p.accel ^= 1,
                Field::Brake => p.brake ^= 1,
            }
        }
    }
    let flipped = t.inject_into(c, Encoding::Explicit)?;
    if flipped.len() != base.len() {
        return Err(format!(
            "flipping a vehicle field changed the file length ({} -> {}); the base image \
             is not a fixed bit layout and cannot be patched in place",
            base.len(),
            flipped.len()
        ));
    }

    // Runs of consecutive differing bits, in order.
    let mut runs: Vec<(usize, usize)> = Vec::with_capacity(at.len());
    let mut run: Option<(usize, usize)> = None;
    for byte in 0..base.len() {
        let d = base[byte] ^ flipped[byte];
        if d == 0 {
            if let Some(r) = run.take() {
                runs.push(r);
            }
            continue;
        }
        for k in 0..8 {
            // The codec is LSB-first: bit `byte*8 + k` is `1 << k`.
            let bit = byte * 8 + k;
            if d & (1u8 << k) != 0 {
                match &mut run {
                    Some((s, l)) if *s + *l == bit => *l += 1,
                    _ => {
                        if let Some(r) = run.take() {
                            runs.push(r);
                        }
                        run = Some((bit, 1));
                    }
                }
            } else if let Some(r) = run.take() {
                runs.push(r);
            }
        }
    }
    if let Some(r) = run.take() {
        runs.push(r);
    }

    if runs.len() != at.len() {
        let mut hist: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
        for (_, l) in &runs {
            *hist.entry(*l).or_insert(0) += 1;
        }
        return Err(format!(
            "probing the encoder found {} changed field(s) where {} ticks were flipped -- \
             the bit layout is not what a patcher assumes; refusing to guess. \
             run lengths: {:?}",
            runs.len(),
            at.len(),
            hist
        ));
    }
    for (i, (_, l)) in runs.iter().enumerate() {
        if *l != width {
            return Err(format!(
                "probe {}: a {}-bit field changed {} bits; refusing to guess",
                i, width, l
            ));
        }
    }
    Ok(runs.into_iter().map(|(s, _)| s).collect())
}

/// Write `n` bits of `v` at absolute bit position `pos`.
///
/// LSB-first, matching `ghost::bits::BitWriter`: bit `i` of the value lands at
/// `pos + i`, and bit `b` of the stream is `1 << (b & 7)` of byte `b >> 3`.
/// (Getting this backwards is not a subtle failure -- the probe below reports
/// run lengths that are not the field width and refuses to build a patcher.)
#[inline]
fn patch_bits(buf: &mut [u8], pos: usize, v: u64, n: usize) {
    for i in 0..n {
        let b = pos + i;
        let mask = 1u8 << (b & 7);
        let byte = &mut buf[b >> 3];
        if (v >> i) & 1 == 1 {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
    }
}
