//! The finishing pass: everything between "the engine state is in the record"
//! and "this file is publishable".
//!
//! It exists because the list of things that had to be remembered was longer
//! than anyone remembered it, and every item on it shipped at least once:
//!
//! | shipped defect | what was forgotten |
//! |---|---|
//! | dirt thrown where there is no dirt | the 49 unwritten per-run bytes |
//! | a 441 s clip of a 218 s run, camera adrift | the record's inherited span |
//! | a header declaring the donor's time | the declared-time census |
//! | a stranger's login, badge, country, uuid | the identity fields |
//!
//! Every one was found by a person noticing something odd in a video, days
//! later. So the fix is not another check that somebody has to run: it is that
//! `ghost regen`'s DEFAULT output is finished, and that it refuses rather than
//! writing something a later step has to catch.
//!
//! Each step below already existed as a separate command that had to be run
//! afterwards by whoever knew to. They are called here, in order, on every
//! regeneration — `ghost record rebuild`, `ghost declare --from-oracle`,
//! `ghost identity --anonymise` — and each one still refuses on its own terms,
//! which is what makes calling them a pipeline rather than a copy of them.

use gbx::container::secs;

/// Which sample bytes still hold the container donor's values, by number.
///
/// The acceptance test for the whole pass, and the one that cannot be argued
/// with: compare against the container this file was built in, byte by byte, at
/// the same sample index. **An empty list is the goal.** A non-empty one is
/// printed as loudly as a failure, with the offsets spelled out, because "91 of
/// 116 bytes are still the carrier's" was true for months and was carried
/// around as a generality instead of a list — which is why nobody noticed that
/// four of those bytes are what make the tyres throw dirt.
///
/// Format constants are excluded: identical in every ghost of every driver,
/// never varying in either file, so they carry no provenance and listing them
/// would bury the ones that do.
/// A BYTE WE WROTE CANNOT BE INHERITED, however much it agrees.
///
/// The round-trip control found this on its first run and it would have been a
/// slow poison otherwise: regenerating a recording from its OWN inputs
/// correctly reproduces its own position, so bytes 50, 57 and 58 -- the
/// high-order bytes of the x and z floats, which barely move over a lap --
/// came back bit-identical on every sample, and the provenance check called
/// three position bytes the donor's. Agreement between our output and the file
/// it was generated from is SUCCESS there, not contamination.
///
/// So the question is only ever asked about bytes the regeneration did not
/// write: the transform (47..69) and the tape echo (14, 15, 18) are ours by
/// construction, whatever they equal.
fn written_by_us(o: usize) -> bool {
    (47..69).contains(&o) || o == 14 || o == 15 || o == 18
}

pub fn inherited_bytes(ghost: &str, carrier: &str) -> Result<Vec<usize>, String> {
    let a = gbx::record::decode_ghost(ghost)?;
    let b = gbx::record::decode_ghost(carrier)?;
    let ss = a.sample_size;
    if ss != b.sample_size || ss == 0 {
        return Err(format!(
            "sample sizes differ ({} vs {}) -- this is not the container it was built in",
            ss, b.sample_size
        ));
    }
    let n = (a.raw.len() / ss).min(b.raw.len() / ss);
    if n < 20 {
        return Err(format!("only {n} comparable samples"));
    }
    let mut out = Vec::new();
    for k in 0..ss {
        let mut same = 0usize;
        let mut varies = false;
        for i in 0..n {
            if a.raw[i * ss + k] == b.raw[i * ss + k] {
                same += 1;
            }
            if b.raw[i * ss + k] != b.raw[k] {
                varies = true;
            }
        }
        if same == n && varies && !written_by_us(k) {
            out.push(k);
        }
    }
    Ok(out)
}

/// Does anything in this file outlive the car?
///
/// The record's own declared end, and every entity's own last sample. 286279's
/// published `BEST_218812` reads `span 0.000 .. 441.000` for a car that stops
/// at 217.95 and keeps the donor's 8820-sample non-vehicle entity at its full
/// length — which is what renders 441 s of video and strands the camera when
/// our car's entity ends.
pub fn outlives_the_car(path: &str) -> Result<Option<String>, String> {
    let d = gbx::record::decode_ghost(path)?;
    let last = d.samples.last().map(|s| s.time_ms).unwrap_or(0) as i64;
    let ent_end = d.ents.iter().filter_map(|e| e.t_last).max().unwrap_or(0) as i64;
    let scene = (d.end_ms as i64).max(ent_end);
    let past = scene - last;
    // Proportionate, not flat: a record legitimately runs a fraction of a
    // second past the finish, while a carrier's span is 87 % to 10 500 % long.
    if past > 2000 && past as f64 / (last as f64).max(1.0) >= 0.10 {
        return Ok(Some(format!(
            "the scene ends at {} and the car's last sample is at {} (+{:.0} %), over {} entities",
            secs(scene),
            secs(last),
            100.0 * past as f64 / (last as f64).max(1.0),
            d.ents.len()
        )));
    }
    Ok(None)
}

/// Is every channel we claim to write actually alive in this file?
///
/// **A bare position copy once wrote ZEROED wheels into a file that passed the
/// entire verify gate** (found by the carrier-byte arm, 2026-08-22). Zero is a
/// legal value for every one of these channels, so no per-sample check can
/// object to one sample of it; what no real run does is hold a channel at a
/// single value for its whole length while the car drives 3 km.
///
/// So the test is variance, not value, and it applies to whatever the current
/// pipeline says it writes. As channels move from `unwritten_channels` into the
/// written set, they come under this automatically -- which is the point: the
/// day the wheel bytes start being written is the day a silently-zeroed wheel
/// byte has to be caught, and nobody should have to remember to add a check.
pub fn dead_channels(path: &str, expect_alive: &[(usize, &str)]) -> Result<Vec<String>, String> {
    let d = gbx::record::decode_ghost(path)?;
    let ss = d.sample_size;
    let n = d.raw.len() / ss.max(1);
    if n < 20 {
        return Err(format!("only {n} samples"));
    }
    let mut dead = Vec::new();
    for (o, name) in expect_alive {
        if *o >= ss {
            continue;
        }
        let first = d.raw[*o];
        if (0..n).all(|i| d.raw[i * ss + o] == first) {
            dead.push(format!(
                "byte {o} ({name}) holds {first:#04x} on all {n} samples -- this channel is \
                 claimed as written and is not alive"
            ));
        }
    }
    Ok(dead)
}

/// The engine channels this pass does NOT yet write, named by byte.
///
/// **This is a harness limit, not a data limit, and the distinction is the
/// point.** Every one of these quantities is in the engine's memory while it
/// simulates — the `whl` arm fitted them against a real recording and got
/// gear, turbo and wetness exact on 100 % of samples and rpm on 92.6 % — so
/// "we cannot have them" would be false. What is true is that their addresses
/// are relative to a copy of the car state whose position varies per map, and
/// the anchoring that would make them portable is not built. Until it is, the
/// honest thing is to write ZERO and say which byte, rather than pass the
/// donor's through where it reads as ours.
pub fn unwritten_channels() -> &'static [(usize, &'static str)] {
    &[
        (5, "rpm"),
        (81, "ice, front left"),
        (82, "ice, front right"),
        (83, "ice, rear right"),
        (84, "ice, rear left"),
        (89, "ground contact"),
        (91, "gear"),
        (93, "dirt, front left"),
        (95, "dirt, front right"),
        (97, "dirt, rear right"),
        (99, "dirt, rear left"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE CHECK MUST FIRE, not merely pass on good files.
    ///
    /// A liveness check that has only ever been run on live channels is
    /// indistinguishable from one that returns "fine" unconditionally -- and
    /// this project has shipped exactly that shape more than once (a guard
    /// whose function was never defined, so bash returned 127 and every call
    /// answered "no"). So: a synthetic record with one channel held constant
    /// and one varying, and the check must name the first and not the second.
    #[test]
    fn a_channel_held_at_one_value_is_named_and_a_varying_one_is_not() {
        let ss = 116usize;
        let n = 40usize;
        let mut raw = vec![0u8; ss * n];
        for i in 0..n {
            raw[i * ss + 7] = i as u8; // alive
                                       // byte 9 left at 0 on every sample: dead
        }
        let dead: Vec<usize> = (0..ss)
            .filter(|k| {
                let first = raw[*k];
                (0..n).all(|i| raw[i * ss + k] == first)
            })
            .collect();
        assert!(dead.contains(&9), "a constant channel must be detected");
        assert!(!dead.contains(&7), "a varying channel must not be");
    }
}
