//! IS THE BIT WE CALL "RESPAWN" ACTUALLY A RESPAWN?
//!
//! The exporter now writes TICK `respawn` actions off state-literal bit 31
//! (`word0 & 0x20`). "The old code called it respawn" is not evidence. The
//! ghost carries a SECOND, independent record of the same run -- the
//! `CPlugEntRecordData` telemetry, one 116-byte vehicle sample every 50 ms --
//! and it is produced by the simulation, not by the input layer. If the bit is
//! the respawn input, the car must teleport right after a tick that carries
//! one, and must not teleport much anywhere else.
//!
//! Measured on the fixture below (a Trial map, 407 s, 8103 telemetry steps):
//!
//! * 8 of its 9 respawn ticks sit within 100 ms of a >= 8 m jump between two
//!   consecutive 50 ms samples (the largest is 186 m).
//! * only 10 of 8103 steps (0.12 %) are >= 8 m at all, so landing on 8 of 9 by
//!   coincidence is not a possibility worth entertaining.
//! * 2 of those 10 jumps are not near a respawn tick -- a fall and a landing.
//!   The correlation is strong, not total, and the test asserts exactly that.
//!
//! WHAT THIS TEST DOES NOT SHOW. `srespawn` (`word0 & 0x1000`) is mapped to
//! TICK's standing-respawn action BY NAME; no run in the corpus separates the
//! two cleanly enough to prove it from telemetry, and this fixture has none.
//! Respawns during the start countdown (the `respawn_m2_...` fixture's are all
//! at race -1.490 s .. -0.950 s) move nothing, so they are not evidence either
//! way.

mod common;

use common::*;
use gbx::record::decode_ghost;
use tmsite::tick::{self, Opts};

const TRIAL: &str = "238835-turtle-trial-angustus/replays/NORETRY_407463_watchable.Ghost.Gbx";
const JUMP_M: f64 = 8.0;
const NEAR_MS: i32 = 100;

#[test]
fn the_respawn_bit_coincides_with_a_teleport_in_the_telemetry() {
    let path = repo_fixture_str(TRIAL);
    let e = tick::export(&Opts { path: path.clone(), archive: 0, raw: false, seed: None }).unwrap();
    assert_eq!(e.respawns.len(), 9, "fixture no longer has the 9 respawns this test is about");

    let d = decode_ghost(&path).unwrap_or_else(|x| panic!("decode telemetry of {}: {}", path, x));
    assert!(d.samples.len() > 8000, "telemetry is {} samples", d.samples.len());
    assert!(
        d.samples.iter().any(|s| s.x != 0.0 || s.z != 0.0),
        "every telemetry position decoded as zero -- the corroboration cannot be made \
         (this is gbx::record, not the exporter)"
    );

    // (sample time, distance from the previous sample)
    let steps: Vec<(i32, f64)> = d
        .samples
        .windows(2)
        .map(|w| {
            let (a, b) = (&w[0], &w[1]);
            let m = (((b.x - a.x) as f64).powi(2) + ((b.y - a.y) as f64).powi(2) + ((b.z - a.z) as f64).powi(2)).sqrt();
            (b.time_ms, m)
        })
        .collect();
    let jumps: Vec<(i32, f64)> = steps.iter().filter(|(_, m)| *m >= JUMP_M).cloned().collect();

    // The control: a jump is RARE. Without this, "every respawn is near a jump"
    // would be satisfied by telemetry that jumps all the time.
    let frac = jumps.len() as f64 / steps.len() as f64;
    assert!(
        frac < 0.005,
        "{} of {} steps are >= {} m ({:.2} %) -- a jump is not rare here, so proximity proves nothing",
        jumps.len(),
        steps.len(),
        JUMP_M,
        frac * 100.0
    );

    let race_ms = |t: usize| t as i32 * 10 + e.start_offset_ms;
    let matched = e
        .respawns
        .iter()
        .filter(|&&t| jumps.iter().any(|(ms, _)| (ms - race_ms(t)).abs() <= NEAR_MS))
        .count();
    assert!(
        matched >= 8,
        "only {} of {} respawn ticks land within {} ms of a >= {} m jump; respawn ticks at {:?}, jumps at {:?}",
        matched,
        e.respawns.len(),
        NEAR_MS,
        JUMP_M,
        e.respawns.iter().map(|&t| race_ms(t)).collect::<Vec<_>>(),
        jumps
    );

    let unexplained: Vec<(i32, f64)> = jumps
        .iter()
        .filter(|(ms, _)| !e.respawns.iter().any(|&t| (ms - race_ms(t)).abs() <= NEAR_MS))
        .cloned()
        .collect();
    assert!(
        unexplained.len() <= 2,
        "{} jumps are not explained by a respawn: {:?}",
        unexplained.len(),
        unexplained
    );
}
