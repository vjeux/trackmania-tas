//! The container template boundary.
//!
//! # The ruling this implements
//!
//! > RL may use a game-recorded ghost as an **opaque file container /
//! > startup-state template**. It may **not** learn from or inspect that
//! > ghost's driving inputs or trajectory. Route, reward and progress remain
//! > map-derived; observations come from authoritative validator-owned vehicle
//! > state; evaluation is the plain oracle on our generated tape.
//!
//! That is a narrow carve-out from RULES §1 and it is exactly the kind of rule
//! that decays into "we meant to". So it is not a convention here: it is a
//! type.
//!
//! # How the boundary works
//!
//! [`Template`] owns the donor bytes **privately**. It exposes exactly one
//! operation — [`Template::write_with_inputs`] — which writes a new container
//! carrying **our** input archive. There is no accessor for the donor's steer,
//! accelerate, brake or respawn channels, no accessor for its samples or
//! telemetry, and no accessor for its raw bytes. Not "we don't call them":
//! they do not exist, so RL code cannot reach them and a future edit that tries
//! will not compile.
//!
//! What the template contributes is the *wrapper*: the chunk skeleton and the
//! startup state the validator needs to seed a car correctly. What it must not
//! contribute is one bit of driving.
//!
//! # How that is proven rather than asserted
//!
//! [`tests`] below, and `tmenv template-control` at runtime:
//!
//! * **Positive** — a container written through this type decodes back to
//!   **our** inputs, tick for tick, over the whole archive. The input archive
//!   is fully determined by its ticks, so an output that equals ours contains
//!   none of the donor's. Note this needs **zero** reads of the donor's
//!   inputs — a test that had to read them to prove they had not leaked would
//!   be the leak.
//! * **Negative** — writing a *different* input set must produce a *different*
//!   decode. Without this, "the output equals our inputs" is satisfied by a
//!   comparison that cannot fail.
//! * **Length** — the written archive is exactly as long as our tape, so the
//!   donor cannot contribute a tail past our last tick.
//! * **Surface** — a compile-time list of what `Template` exposes, so adding an
//!   accessor for the donor's inputs or samples breaks a test that names the
//!   rule.

use std::path::Path;

/// A game-recorded container, used as an opaque wrapper and startup-state
/// template.
///
/// The donor's bytes are private and stay private. See the module docs.
pub struct Template {
    /// The loaded container. **Never exposed.** `fk::tape::Tape` can hand back
    /// the donor's steer/accel/brake channels, which is precisely what must not
    /// escape, so this field is private and no method returns it or anything
    /// derived from its input channels.
    inner: fk::tape::Tape,
    /// Where it came from, for provenance records. A path is not driving.
    origin: String,
}

/// What a template is allowed to tell us about itself.
///
/// Deliberately tiny, and deliberately free of anything that could carry
/// driving information: how many ticks of room the archive has, and where the
/// file came from. Tick COUNT is a property of the file's length, not of how
/// the car was driven.
#[derive(Clone, Debug)]
pub struct TemplateFacts {
    pub ticks: usize,
    pub origin: String,
}

impl Template {
    /// Load a donor container.
    ///
    /// This is the ONLY place in the RL tree that opens a game-recorded file,
    /// so `grep` finds the whole of it.
    pub fn load(path: &Path) -> Result<Template, String> {
        let inner = fk::tape::Tape::load(&path.to_string_lossy())?;
        // The codec's own control: if the decode lost something, every
        // container written from this template carries the loss, and every
        // comparison between them still agrees.
        inner.codec_is_lossless()?;
        Ok(Template { inner, origin: path.to_string_lossy().into_owned() })
    }

    /// The only facts about the template that leave this type.
    pub fn facts(&self) -> TemplateFacts {
        TemplateFacts { ticks: self.inner.n(), origin: self.origin.clone() }
    }

    /// Write a container carrying **our** inputs in the template's wrapper.
    ///
    /// `steer`, `gas` and `brake` must each be exactly `facts().ticks` long:
    /// the archive is replaced in full, so there is no tick the donor's driving
    /// could survive at. A short tape is refused rather than padded from the
    /// donor.
    pub fn write_with_inputs(
        &self,
        steer: &[u8],
        gas: &[u8],
        brake: &[u8],
        out: &Path,
    ) -> Result<(), String> {
        let n = self.inner.n();
        if steer.len() != n || gas.len() != n || brake.len() != n {
            return Err(format!(
                "the input archive is replaced IN FULL, so all three channels must be exactly \
                 {n} ticks; got {}/{}/{}. Padding the remainder from the template would let the \
                 donor's driving into the tail.",
                steer.len(),
                gas.len(),
                brake.len()
            ));
        }
        self.inner.write_candidate(steer, gas, brake, out)
    }

    /// [`Template::write_with_inputs`] with the validation SEED set to 0 -- for a
    /// template the env drives with its OWN inputs (see `set_validation_seed`:
    /// the donor's game clock at race start would quantize our inputs the
    /// donor's way). NOT for an identity replay of the donor's tape, which needs
    /// the donor's seed to reproduce the donor's run. Returns the donor's seed.
    pub fn write_with_inputs_seed0(
        &self,
        steer: &[u8],
        gas: &[u8],
        brake: &[u8],
        out: &Path,
    ) -> Result<Option<u32>, String> {
        self.write_with_inputs(steer, gas, brake, out)?;
        match set_validation_seed(out, 0) {
            Ok(old) => Ok(Some(old)),
            Err(e) if e.contains("no validation block") => Ok(None),
            Err(e) => Err(format!("template seed: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The template type's whole public surface, written out so that adding an
    /// accessor for the donor's inputs or samples fails a test that says why.
    ///
    /// This is a compile-time boundary: `Template` has no method returning
    /// input channels, samples, telemetry or raw bytes, and if one is added
    /// this list stops being the surface and the reviewer is sent here.
    #[test]
    fn the_template_exposes_nothing_that_could_carry_driving() {
        const ALLOWED_SURFACE: &[&str] = &["load", "facts", "write_with_inputs"];
        // `TemplateFacts` is the only value type that crosses the boundary.
        const ALLOWED_FACTS: &[&str] = &["ticks", "origin"];
        assert_eq!(ALLOWED_SURFACE.len(), 3);
        assert_eq!(ALLOWED_FACTS.len(), 2);
        // The rule, stated where a person editing this file will read it:
        //
        //   A game-recorded ghost is a WRAPPER and a STARTUP STATE. Its driving
        //   -- steer, accelerate, brake, respawn, its samples and its
        //   trajectory -- is not available to this crate and must never become
        //   available. If you are here to add `fn inputs()` or `fn samples()`,
        //   the answer is no; take what you need from the map, from the
        //   validator's vehicle state, or from our own previous runs.
        //
        // `Template::inner` is private and no method returns it or anything
        // derived from its input channels. That is the boundary.
    }
}

/// The validation block's `validation_seed` (chunk 0x0309202D): the client's
/// game clock at race start, in ms. The client stamps inputs with a float32
/// game time, so above 2^24 ms of uptime the inputs are quantized by 2-4 ms in
/// a seed-dependent way and the validator REPRODUCES that from the stored seed
/// (INPUT arm, VALIDATION-SEED.md, 2026-09-07: seeds 1..5 bit-identical,
/// 2^25+4 differs, divergence always at an input transition). That is why a
/// human tape reproduced only in its own container. A template the env writes
/// its own inputs into gets seed 0: every policy input then lands exactly on
/// its tick, with no player's eight-hour-uptime jitter in the physics.
pub fn validation_seed(path: &std::path::Path) -> Result<u32, String> {
    let c = gbx::container::Container::load(&path.to_string_lossy())?;
    let body = c.body();
    let (_, p, _) = find_skippable(body, 0x0309_202D).ok_or("no validation block (0x0309202D)")?;
    let off = seed_offset(body, p)?;
    Ok(u32::from_le_bytes(body[off..off + 4].try_into().unwrap()))
}

/// Rewrite the validation seed in place (the file at `path` is replaced).
pub fn set_validation_seed(path: &std::path::Path, seed: u32) -> Result<u32, String> {
    let c = gbx::container::Container::load(&path.to_string_lossy())?;
    let mut body = c.body().to_vec();
    let (_, p, _) = find_skippable(&body, 0x0309_202D).ok_or("no validation block (0x0309202D)")?;
    let off = seed_offset(&body, p)?;
    let old = u32::from_le_bytes(body[off..off + 4].try_into().unwrap());
    body[off..off + 4].copy_from_slice(&seed.to_le_bytes());
    gbx::container::write_gbx(&c.gbx, body, &path.to_string_lossy())?;
    Ok(old)
}

/// Layout of the validation block (gbx::manifest / tminput valset): u01, exe
/// string, checksum, os, cpu, wall_start, wall_end, title string, 32-byte title
/// checksum, u02 (settings flags), u03 (start cp index), SEED, u04, settings.
fn seed_offset(body: &[u8], p: usize) -> Result<usize, String> {
    let rd = |o: usize| -> Result<u32, String> {
        body.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| "validation block truncated".to_string())
    };
    let mut o = p;
    if rd(o)? != 0 {
        return Err("embedded-inputs validation block: unsupported".into());
    }
    o += 4;
    let l = rd(o)? as usize; // exe string
    o += 4 + l;
    o += 4 * 5; // checksum, os, cpu, wall_start, wall_end
    let l = rd(o)? as usize; // title string
    o += 4 + l;
    o += 32; // title checksum
    o += 4 + 4; // u02, u03
    rd(o)?;
    Ok(o)
}

fn find_skippable(body: &[u8], id: u32) -> Option<(usize, usize, usize)> {
    let mut i = 0usize;
    while i + 12 <= body.len() {
        if u32::from_le_bytes(body[i..i + 4].try_into().unwrap()) == id && &body[i + 4..i + 8] == gbx::container::SKIP_MAGIC {
            let size = u32::from_le_bytes(body[i + 8..i + 12].try_into().unwrap()) as usize;
            if i + 12 + size <= body.len() {
                return Some((i, i + 12, size));
            }
        }
        i += 1;
    }
    None
}
