//! **The archive must not care whether a fork child can stay alive.**
//!
//! Agent D is measuring exactly that: yes gives a savestate tree (~6 min per
//! search pass), no gives prefix re-simulation (~37 min). Both work, and the
//! design only holds if the *answer* is the same either way — otherwise the
//! fallback is not a fallback, it is a different system.
//!
//! So: the same search, the same seed, the same budget, against two backends
//! that differ only in whether a handle survives. The archives must be
//! identical, bin for bin.
//!
//! And the negative half, because a test only one backend can fail is not a
//! comparison: a backend whose handle points at the WRONG state must be
//! caught, not silently believed. That is the real fork defect in miniature —
//! a snapshot at the right tick of the wrong run — and `open` verifies the
//! prefix rather than trusting the tick.

use tmexplore::action::Alphabet;
use tmexplore::archive::Policy;
use tmexplore::branch::{Branch, Route};
use tmexplore::explore::{Cfg, Explorer};
use tmexplore::toy::{ToyOracle, ToySim, ToyTrack};

fn cfg(seed: u64) -> Cfg {
    Cfg {
        alphabet: Alphabet::Keyboard,
        k: 10,
        fanout: Some(3),
        max_rollout: 20,
        sticky: 0.7,
        policy: Policy::default(),
        bands: Default::default(),
        seed,
        tick_limit: 3000,
    }
}

fn run(tree: bool, seed: u64, evals: u64) -> (usize, u32, Vec<(u32, u32)>) {
    let track = ToyTrack::demo();
    let mut sim = ToySim::new(&track, tree, 3000);
    let oracle = ToyOracle::new(&track);
    let mut ex = Explorer::new(&track, cfg(seed));
    ex.seed_root(&mut sim).unwrap();
    while ex.stats.evals < evals {
        ex.step(&mut sim, &oracle);
    }
    // a fingerprint of the archive: every bin's station and arrival tick,
    // sorted. Two runs that explored the same states agree on it exactly.
    let mut fp: Vec<(u32, u32)> = ex.archive.iter().map(|(k, e)| (k.station, e.ticks)).collect();
    fp.sort_unstable();
    (ex.archive.len(), ex.archive.max_station, fp)
}

#[test]
fn a_savestate_tree_and_prefix_resimulation_produce_the_same_archive() {
    let (n_tree, far_tree, fp_tree) = run(true, 11, 40_000);
    let (n_flat, far_flat, fp_flat) = run(false, 11, 40_000);
    assert_eq!(n_tree, n_flat, "different bin counts");
    assert_eq!(far_tree, far_flat, "different furthest station");
    assert_eq!(fp_tree, fp_flat, "the two backends explored different states");
    // and the run has to have gone somewhere, or this compares two empty sets
    assert!(far_tree > 3, "the search did not move; this test would pass on nothing");
    assert!(n_tree > 200, "only {} bins", n_tree);
}

#[test]
fn different_seeds_explore_differently() {
    // The other half. If the fingerprint were constant, the test above would
    // pass against a backend that did nothing at all.
    let (_, _, a) = run(true, 11, 20_000);
    let (_, _, b) = run(true, 12, 20_000);
    assert_ne!(a, b);
}

#[test]
fn a_handle_is_verified_against_its_prefix_not_just_its_tick() {
    // The real fork defect, in miniature: a snapshot parked at the right tick
    // of the WRONG run. If `open` trusted the tick, the search would evaluate
    // a run nobody asked for and answer honestly about it.
    let track = ToyTrack::demo();
    let mut sim = ToySim::new(&track, true, 3000);
    let a = vec![tmexplore::action::Input { steer: 0, gas: true, brake: false }; 40];
    let b = vec![tmexplore::action::Input { steer: 127, gas: true, brake: false }; 40];

    let ha = sim.open(&a, None).unwrap();
    let adv = sim.advance(ha, 40, &a[..10]).unwrap();
    let h_after_a = adv.handle.expect("a tree backend parks a handle");

    // Same length, different history. Offering A's handle for B's prefix must
    // NOT be honoured.
    let hb = sim.open(&b, Some(h_after_a)).unwrap();
    let from_b = sim.advance(hb, 40, &b[..10]).unwrap();

    // ...and the honest answer is the one a cold open gives.
    let hb2 = sim.open(&b, None).unwrap();
    let cold = sim.advance(hb2, 40, &b[..10]).unwrap();
    assert_eq!(
        from_b.trace.last().unwrap().pos,
        cold.trace.last().unwrap().pos,
        "the stale handle was believed: this is the fork defect"
    );

    // positive half: a handle offered for its OWN prefix is honoured, and the
    // answer is still right.
    let a50: Vec<_> = a.iter().chain(a[..10].iter()).cloned().collect();
    let hc = sim.open(&a50, None).unwrap();
    let cold_a = sim.advance(hc, 50, &a[..10]).unwrap();
    let ha2 = sim.open(&a, None).unwrap();
    let adv2 = sim.advance(ha2, 40, &a[..10]).unwrap();
    let hh = adv2.handle.unwrap();
    let hd = sim.open(&a50, Some(hh)).unwrap();
    let warm_a = sim.advance(hd, 50, &a[..10]).unwrap();
    assert_eq!(cold_a.trace.last().unwrap().pos, warm_a.trace.last().unwrap().pos);
}

#[test]
fn advance_refuses_a_write_below_the_consumed_boundary() {
    // The forward-only rule, enforced in the type rather than in discipline.
    let track = ToyTrack::demo();
    let mut sim = ToySim::new(&track, true, 3000);
    let a = vec![tmexplore::action::Input { steer: 0, gas: true, brake: false }; 40];
    let h = sim.open(&a, None).unwrap();
    let e = sim.advance(h, 20, &a[..5]).unwrap_err();
    assert_eq!(
        e,
        tmexplore::branch::BranchErr::BelowBoundary { asked: 20, boundary: 40 }
    );
    // positive half: at the boundary it is allowed.
    let h = sim.open(&a, None).unwrap();
    assert!(sim.advance(h, 40, &a[..5]).is_ok());
}

#[test]
fn the_route_and_the_track_agree_about_where_the_finish_is() {
    let t = ToyTrack::demo();
    assert!(t.n_stations() > 40);
    assert_eq!(t.station_of(0.0), 0);
    assert_eq!(t.station_of(t.length()), (t.length() / t.spacing()) as u32);
}
