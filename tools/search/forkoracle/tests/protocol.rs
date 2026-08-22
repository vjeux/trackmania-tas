//! What the fork server's driver must not get wrong, tested with no server
//! running.
//!
//! Every fixture here is a canned transcript or a synthetic trajectory. They
//! run on any machine, in a second, and they are the checks that would
//! otherwise only be exercised by a search that takes an hour and cannot tell
//! you which part broke.

use forkoracle::forksrv::parse_result;
use forkoracle::layout::{check_rows, sample_ms, tail_recs, Row};

/// THE ASYMMETRIC FIXTURE.
///
/// The server prints two results per file: `ValidatedResult` (what it
/// simulated) and `DeclaredResult` (what the file claims). On a healthy file
/// they are equal -- so a fixture built from a healthy file cannot fail
/// whatever the parser does, and pins nothing. These two disagree on purpose.
const FINISHED_BUT_STALE_HEADER: &str = r#"
{
   "ValidatedResult": {
      "Time": 23081,
      "NbCheckpoints": 3
   },
   "DeclaredResult": {
      "Time": 22963,
      "NbCheckpoints": 3
   },
   "Desc": "race finished.",
   "IsValid": true,
   "FileName": "c0007.Ghost.Gbx"
}
"#;

/// A run that did NOT finish, in a file that still declares a time. This is the
/// shape that turns a validation step into a self-confirmation: read the
/// `"Time"` lines in order and the answer is 22.963 for a tape that DNFs.
const DNF_WITH_A_DECLARED_TIME: &str = r#"
{
   "ValidatedResult": null,
   "DeclaredResult": {
      "Time": 22963,
      "NbCheckpoints": 3
   },
   "Desc": "reached some checkpoints (2 out of 4).",
   "IsValid": false,
   "FileName": "c0008.Ghost.Gbx"
}
"#;

#[test]
fn the_fork_parser_reads_what_the_engine_did_not_what_the_file_claims() {
    let (t, _) = parse_result(FINISHED_BUT_STALE_HEADER);
    assert_eq!(t, Some(23081), "took the file's declaration instead of the simulation");
}

#[test]
fn a_dnf_with_a_declared_time_is_a_dnf() {
    let (t, cps) = parse_result(DNF_WITH_A_DECLARED_TIME);
    assert_eq!(t, None, "a declared time was reported as a finish");
    assert_eq!(cps, Some(2));
}

/// `wrong simu` is the information-free failure: it means the run did not
/// finish and says nothing about how far it got. It must not be mistaken for
/// "reached zero checkpoints and went nowhere" -- on one map, 45 of 200 such
/// runs had driven up to 966 m of 1647 m.
#[test]
fn wrong_simu_is_a_sentinel_not_a_distance() {
    let (t, cps) = parse_result("\"Desc\": \"wrong simu\"\n\"FileName\": \"x.Ghost.Gbx\"\n");
    assert_eq!(t, None);
    assert_eq!(cps, Some(0));
}

#[test]
fn tail_recs_starts_at_the_resume_tick_and_scales_steering() {
    let steer: Vec<u8> = (0..10u8).map(|t| (t as i8 * 12) as u8).collect();
    let gas = vec![1u8; 10];
    let brake = vec![0u8; 10];
    let r = tail_recs(&steer, &gas, &brake, 4);
    assert_eq!(r.len(), 6, "a resume must send only the ticks from `from` on");
    // steer is i8 over 127; tick 4 of the ramp is 48
    assert!((r[0].steer - 48.0 / 127.0).abs() < 1e-6, "steer scaling: {}", r[0].steer);
    assert_eq!(r[0].gas, 1.0);
    assert_eq!(r[0].brake, 0.0);
}

/// Sample 0 of a stream started at probe tick P is the END of tick P-1. An
/// off-by-one here shows up as ~0.3 m of position error and was, once,
/// mistaken for a bad memory slot.
#[test]
fn sample_labelling_is_the_end_of_the_previous_tick() {
    assert_eq!(sample_ms(60, 0, 0), 590);
    assert_eq!(sample_ms(60, 1, 0), 600);
    assert_eq!(sample_ms(60, 0, -1580), -990);
}

fn synthetic(n: usize, bad_quat: bool, bad_vel: bool) -> Vec<Row> {
    // a car going in a straight line at 20 m/s, sampled every 10 ms
    (0..n)
        .map(|i| {
            let t = i as f64 * 0.01;
            Row {
                time_ms: (i as i64) * 10,
                x: 20.0 * t,
                y: 0.0,
                z: 0.0,
                vx: if bad_vel { 200.0 } else { 20.0 },
                vy: 0.0,
                vz: 0.0,
                qw: if bad_quat { 3.0 } else { 1.0 },
                qx: 0.0,
                qy: 0.0,
                qz: 0.0,
            }
        })
        .collect()
}

#[test]
fn the_row_check_passes_a_real_looking_run() {
    let c = check_rows(&synthetic(200, false, false)).expect("a clean run must pass");
    assert!(c.quat_err < 1e-6, "{}", c);
    assert!(c.vel_err < 0.5, "{}", c);
    assert!((c.mean_speed - 20.0).abs() < 0.5, "{}", c);
}

/// The negative controls. A check that only ever passes is decoration.
#[test]
fn the_row_check_refuses_a_non_unit_quaternion() {
    assert!(
        check_rows(&synthetic(200, true, false)).is_err(),
        "|q| = 3 was accepted as a car's orientation"
    );
}

#[test]
fn the_row_check_refuses_a_velocity_that_is_not_the_positions_derivative() {
    assert!(
        check_rows(&synthetic(200, false, true)).is_err(),
        "a velocity ten times the actual motion was accepted"
    );
}

#[test]
fn the_row_check_refuses_a_sample_too_short_to_mean_anything() {
    assert!(check_rows(&synthetic(3, false, false)).is_err());
}
