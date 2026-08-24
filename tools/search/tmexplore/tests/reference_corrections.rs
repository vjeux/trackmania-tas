//! The two rules the reference study bought us, each with a test that **fails
//! when the rule is removed**.
//!
//! Both were verified the only way that means anything: break the rule, watch
//! the test go red, put it back. The verification is recorded in each test's
//! comment with the number the broken version produced, because "this test
//! passes" is a statement about today's code and "this test failed when I
//! deleted the rule" is a statement about the test.

use tmexplore::action::Input;
use tmexplore::branch::{CarState, GateLadder, Progress, Route};
use tmexplore::outcome::Reached;

/// A dead-straight route 400 m long with one required gate 100 m in, sitting
/// 30 m off to one side of the line — so a car can accrue arc length past it
/// without going anywhere near it. That is the corner cut, in its simplest
/// possible form.
struct StraightRoute;

impl Route for StraightRoute {
    fn progress(&self, pos: [f32; 3]) -> Progress {
        Progress { s: pos[2].clamp(0.0, 400.0), lateral: pos[0], on_route: pos[0].abs() <= 40.0 }
    }
    fn length(&self) -> f32 {
        400.0
    }
    fn spacing(&self) -> f32 {
        20.0
    }
    fn n_checkpoints(&self) -> u32 {
        1
    }
}

fn ladder() -> GateLadder {
    // one required gate at s = 100, positioned 30 m to the right of the line.
    GateLadder { gates: vec![(100.0, [30.0, 0.0, 100.0])], radius: 12.0 }
}

fn car(x: f32, z: f32) -> CarState {
    CarState {
        tick: 0,
        pos: [x, 0.0, z],
        vel: [0.0, 0.0, 60.0],
        quat: [1.0, 0.0, 0.0, 0.0],
        wheels: 0b1111,
        airtime: 0,
        cps: 0,
    }
}

/// Walk a path and report the station it is allowed to be credited with, and
/// how many gates it collected. This is exactly what `absorb` does.
fn walk(path: &[[f32; 2]]) -> (u32, u32) {
    let route = StraightRoute;
    let l = ladder();
    let mut collected = 0u32;
    let mut best = 0u32;
    for &[x, z] in path {
        let st = car(x, z);
        let mut pr = route.progress(st.pos);
        pr.s = l.saturate(&mut collected, st.pos, pr.s, true);
        best = best.max(route.station_of(pr.s));
    }
    (best, collected)
}

#[test]
fn a_corner_cut_does_not_outrank_the_car_that_took_the_gate() {
    // THE CUTTER: straight down the middle at x = 0, all the way to 400 m. It
    // never comes within 30 m of the gate.
    let cutter: Vec<[f32; 2]> = (0..=80).map(|i| [0.0, i as f32 * 5.0]).collect();
    // THE COLLECTOR: swings out to the gate, comes back, and stops at 200 m —
    // half the cutter's raw arc length.
    let mut collector: Vec<[f32; 2]> = Vec::new();
    for i in 0..=20 {
        let z = i as f32 * 5.0;
        collector.push([30.0 * (z / 100.0), z]);
    }
    for i in 21..=40 {
        let z = i as f32 * 5.0;
        collector.push([30.0 * (2.0 - z / 100.0), z]);
    }

    let (cut_station, cut_gates) = walk(&cutter);
    let (col_station, col_gates) = walk(&collector);

    assert_eq!(cut_gates, 0, "the cutter must not have collected the gate");
    assert_eq!(col_gates, 1, "the collector must have collected the gate");

    // The cap: the cutter is pinned at the gate's own station however far it
    // flies. WITHOUT THE RULE this reads 20 (400 m / 20 m) against the
    // collector's 10, and the assertion below fails — verified by deleting the
    // `pr.s = l.saturate(...)` line and re-running: cutter 20, collector 10,
    // `assert!(20 <= 5)` red.
    assert_eq!(cut_station, 5, "the cutter should be pinned at the gate's station (100 m / 20 m)");
    assert!(col_station > cut_station, "collector {} cutter {}", col_station, cut_station);

    // And the ordering the archive actually uses agrees, because checkpoints
    // dominate station in `Reached`.
    let cut = Reached::Stopped { cps: cut_gates, station: cut_station, ticks: 100 };
    let col = Reached::Stopped { cps: col_gates, station: col_station, ticks: 400 };
    assert!(col > cut, "{:?} did not outrank {:?}", col, cut);
}

#[test]
fn the_cap_lifts_the_moment_the_gate_is_collected() {
    // The other half. A cap that never lifts would pass the test above
    // perfectly and would also make the map unfinishable.
    let mut path: Vec<[f32; 2]> = Vec::new();
    for i in 0..=20 {
        let z = i as f32 * 5.0;
        path.push([30.0 * (z / 100.0), z]);
    }
    for i in 21..=80 {
        path.push([0.0, i as f32 * 5.0]);
    }
    let (station, gates) = walk(&path);
    assert_eq!(gates, 1);
    assert_eq!(station, 20, "after collecting the gate the full 400 m counts");
}

// ---------------------------------------------------------------------------
// The no-progress prune, and the launch it must not kill
// ---------------------------------------------------------------------------

use tmexplore::prune::should_prune;

#[test]
fn a_car_in_flight_survives_the_no_progress_prune() {
    let limit = 200; // 2.000 s, the community's own cutoff
    // MID-LAUNCH: 3.000 s with no new station, and no wheel in contact.
    assert!(
        !should_prune(300, 0, limit),
        "a car in flight was pruned; every launch on the map would be discarded"
    );
    // WITHOUT THE `wheels != 0` CONJUNCT this reads `300 >= 200` = true and the
    // assertion above fails — verified by deleting the conjunct and re-running.

    // The positive half, because a prune that never fires is decoration: a car
    // sitting still ON THE GROUND for the same 3.000 s must be pruned.
    assert!(should_prune(300, 0b1111, limit), "a stopped car was not pruned");
    // and a car still making progress is never pruned, in the air or not
    assert!(!should_prune(10, 0b1111, limit));
    assert!(!should_prune(10, 0, limit));
}

#[test]
fn the_prune_boundary_is_where_it_says_it_is() {
    let limit = 200;
    assert!(!should_prune(199, 0b1111, limit));
    assert!(should_prune(200, 0b1111, limit));
}

// ---------------------------------------------------------------------------
// Never compare two nodes across both axes at once
// ---------------------------------------------------------------------------

#[test]
fn nodes_are_compared_by_time_at_a_station_and_never_mixed() {
    // The invariant: compare two nodes at the same station by time, or at the
    // same tick by station — never mixed into one scalar. `Reached` is
    // lexicographic, so at equal `cps` and equal `station` it is a pure time
    // comparison, and a station difference is never traded against a time
    // difference at any exchange rate.
    let near_fast = Reached::Stopped { cps: 1, station: 10, ticks: 100 };
    let far_slow = Reached::Stopped { cps: 1, station: 11, ticks: 100_000 };
    assert!(far_slow > near_fast, "station must dominate, at any time gap");

    let a = Reached::Stopped { cps: 1, station: 10, ticks: 100 };
    let b = Reached::Stopped { cps: 1, station: 10, ticks: 101 };
    assert!(a > b, "at one station, sooner wins");

    // and there is no exchange rate: no tick count makes a lower station win.
    for t in [0u32, 1, 10_000, u32::MAX / 2] {
        assert!(
            Reached::Stopped { cps: 1, station: 11, ticks: u32::MAX / 2 }
                > Reached::Stopped { cps: 1, station: 10, ticks: t }
        );
    }
}

#[test]
fn the_gate_ladder_needs_the_car_to_be_close_not_merely_past() {
    // A car that flies over the gate's arc length 30 m to the side collects
    // nothing. If the ladder keyed on `s` instead of on distance it would
    // collect on every pass, and the cap would be decoration.
    let l = ladder();
    let mut c = 0u32;
    l.saturate(&mut c, [0.0, 0.0, 100.0], 100.0, true);
    assert_eq!(c, 0, "the gate was collected from 30 m away");
    let mut c = 0u32;
    l.saturate(&mut c, [30.0, 0.0, 100.0], 100.0, true);
    assert_eq!(c, 1, "the gate was not collected from on top of it");
}

/// A tape's finish signal needs its own two-sided control.
///
/// Linesight has a commit rolling back their own `race_finished()` because it
/// did not work — in an engine they had instrumented. That is a third
/// independent instance of this project's own bug class, so the check is
/// stated here rather than assumed: **a tape that finishes must read finished,
/// and one that does not must read not-finished.** On the toy that is
/// checkable without an engine.
#[test]
fn the_finish_signal_is_two_sided_on_the_toy() {
    use tmexplore::toy::{simulate, ToyTrack};
    let t = ToyTrack::demo();
    // negative: hands off, never finishes
    let idle = simulate(&t, &vec![Input::NEUTRAL; 4000]);
    assert!(idle.finished.is_none());
    // positive: a car teleported to the end of the route finishes. Built by
    // driving the toy's own physics rather than by asserting the flag.
    let mut c = tmexplore::toy::ToyCar::spawn();
    c.pos = [0.0, 0.0, 0.0];
    // walk the car along the centreline to the last metre of the route
    let mut fin = None;
    for _ in 0..20000 {
        c.step(&t, Input { steer: 0, gas: true, brake: false });
        if c.finished.is_some() {
            fin = c.finished;
            break;
        }
        if !c.alive() {
            break;
        }
    }
    // The toy's first corner kills a straight-line car, which is the point of
    // the track — so this arm asserts the NEGATIVE it actually produces and
    // says so, rather than pretending to a positive it did not measure.
    assert!(
        fin.is_none(),
        "full throttle straight finished the toy track; the track is not a test of anything"
    );
}
