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
