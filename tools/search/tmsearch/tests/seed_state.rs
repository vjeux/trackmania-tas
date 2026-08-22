//! THE SEED IDENTITY CONTROL, offline.
//!
//! In gate mode the classic "did the fork reproduce the seed's millisecond"
//! check is unavailable -- the seed is normally aborted by a predicate long
//! before the finish -- and the replacement is stronger: the fork's measured
//! state at the gate must equal the seed's own recorded state there, position,
//! velocity AND attitude.
//!
//! No engine is needed to test the half that does the comparing, and the
//! fixture is a real game recording checked in under `tools/testdata`, anchored
//! on `CARGO_MANIFEST_DIR` so it does not depend on where the test is run from:
//! a missing one is a failure here, not a skip.

use forkoracle::pred::{parse_gate, GateRecord};
use tmsearch::seedstate;

const GHOST: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/human_23013.Ghost.Gbx");
/// The tape's own clock offset: this recording starts 1.58 s before race 0.
const START_OFFSET_MS: i32 = -1580;

/// A box around one point of the fixture's own line, and the true state there.
fn a_gate_on_the_line() -> (forkoracle::pred_core::Gate, GateRecord) {
    let d = gbx::record::decode_ghost(GHOST).expect("the fixture must be readable");
    assert!(d.samples.len() > 300, "fixture has only {} samples", d.samples.len());
    let s = &d.samples[200];
    let spec = format!(
        "xmin={},xmax={},ymin={},ymax={},zmin={},zmax={}",
        s.x - 1.5,
        s.x + 1.5,
        s.y - 1.5,
        s.y + 1.5,
        s.z - 1.5,
        s.z + 1.5
    );
    let gate = parse_gate(&spec, "speed").expect("gate");
    let expect = seedstate::from_ghost(GHOST, &gate, START_OFFSET_MS).expect("the line enters it");
    (gate, expect)
}

/// The positive control: the recording's own state, handed back as if the fork
/// had measured it, must pass -- and pass with room to spare.
#[test]
fn the_recordings_own_state_passes_its_own_control() {
    let (gate, expect) = a_gate_on_the_line();
    let ag = seedstate::check(GHOST, &gate, &expect, START_OFFSET_MS).expect("check");
    assert!(ag.passed(), "{}", ag.report());
    assert!(ag.pos_err < 0.05, "{}", ag.report());
    assert!(ag.ang_err_deg < 1.0, "{}", ag.report());
}

/// AND IT FAILS WHEN THE STATE IS WRONG. A control that cannot fail is not a
/// control: each of the three quantities is perturbed on its own, by an amount
/// far smaller than a wrong object in the engine's memory would be wrong by.
#[test]
fn a_wrong_position_velocity_or_attitude_is_caught() {
    let (gate, truth) = a_gate_on_the_line();

    let mut moved = truth;
    moved.pos[0] += 5.0;
    let ag = seedstate::check(GHOST, &gate, &moved, START_OFFSET_MS).unwrap();
    assert!(!ag.passed(), "5 m of position error passed: {}", ag.report());

    let mut faster = truth;
    faster.vel[2] += 25.0;
    let ag = seedstate::check(GHOST, &gate, &faster, START_OFFSET_MS).unwrap();
    assert!(!ag.passed(), "25 m/s of velocity error passed: {}", ag.report());

    // A quarter turn about the car's own forward axis: the same position, the
    // same velocity, the car on its side. This is the one the millisecond
    // check could never see, and the reason the record carries a quaternion.
    let mut rolled = truth;
    let (w, x, y, z) = (rolled.quat[0], rolled.quat[1], rolled.quat[2], rolled.quat[3]);
    let (c, s) = ((45f32).to_radians().cos(), (45f32).to_radians().sin());
    rolled.quat = [w * c - z * s, x * c + y * s, y * c - x * s, z * c + w * s];
    let ag = seedstate::check(GHOST, &gate, &rolled, START_OFFSET_MS).unwrap();
    assert!(
        !ag.passed(),
        "the car rolled 90 degrees, in the same place at the same speed, passed: {}",
        ag.report()
    );
}

/// A box the tape never enters cannot say what the fork should have measured,
/// and that is an error rather than a silent zero -- it means the gate is
/// somewhere this seed does not go, which is worth knowing before a search
/// spends an afternoon on it.
#[test]
fn a_box_the_seed_never_visits_is_an_error() {
    let gate = parse_gate(
        "xmin=90000,xmax=90010,ymin=0,ymax=10,zmin=0,zmax=10",
        "speed",
    )
    .unwrap();
    let e = seedstate::from_ghost(GHOST, &gate, START_OFFSET_MS).unwrap_err();
    assert!(e.contains("never enters"), "{}", e);
}

/// The minimum speed applies offline exactly as it does in the child: a box
/// the tape only crawls through is not an arrival on either side.
#[test]
fn the_minimum_speed_applies_offline_too() {
    let d = gbx::record::decode_ghost(GHOST).unwrap();
    let s = &d.samples[200];
    let spec = |ms: f64| {
        format!(
            "xmin={},xmax={},ymin={},ymax={},zmin={},zmax={},minspeed={}",
            s.x - 1.5, s.x + 1.5, s.y - 1.5, s.y + 1.5, s.z - 1.5, s.z + 1.5, ms
        )
    };
    let slow = parse_gate(&spec(1.0), "speed").unwrap();
    assert!(seedstate::from_ghost(GHOST, &slow, START_OFFSET_MS).is_ok());
    let impossible = parse_gate(&spec(s.speed_ms + 50.0), "speed").unwrap();
    assert!(seedstate::from_ghost(GHOST, &impossible, START_OFFSET_MS).is_err());
}

/// THE CLOCK SHIFT, which is the trap this control walked into on a real map.
///
/// The child labels a state by the clock it was gathered at; the sampler's own
/// `sample_ms` labels the first record of tick `t` as the END of tick `t - 1`.
/// The two conventions sit one tick apart, and on 228811 the car is doing
/// 118 m/s at the gate, so that one tick is **1.20 m** -- which reads exactly
/// like a wrong car if the shift is assumed to be zero.
///
/// So it is measured. A one-tick shift is reported and allowed; three ticks is
/// not a labelling convention and fails.
#[test]
fn a_one_tick_clock_shift_is_measured_and_a_three_tick_one_is_refused() {
    let (gate, truth) = a_gate_on_the_line();

    let mut late = truth;
    late.tick += 1;
    let ag = seedstate::check(GHOST, &gate, &late, START_OFFSET_MS).unwrap();
    assert_eq!(ag.shift_ticks, -1, "the one-tick shift was not found: {}", ag.report());
    assert!(ag.passed(), "{}", ag.report());
    assert!(
        ag.pos_err_unshifted > ag.pos_err,
        "the shift bought nothing, so this fixture cannot pin the behaviour: {}",
        ag.report()
    );

    let mut much_later = truth;
    much_later.tick += 3;
    let ag = seedstate::check(GHOST, &gate, &much_later, START_OFFSET_MS).unwrap();
    assert!(!ag.passed(), "a three-tick shift passed: {}", ag.report());
}
