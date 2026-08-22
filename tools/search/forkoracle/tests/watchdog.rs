//! The early-abort watchdog: its condition language, and the geometry it
//! measures progress with.
//!
//! The evaluator itself lives in `shared/pred_core.rs`, which is
//! `#[path]`-included by BOTH this crate and the LD_PRELOAD shim -- so what a
//! predicate means here is what it means inside the fork child. These tests
//! therefore cover the child's logic without a child.

use forkoracle::pred::{parse_spec, RefLineData};
use forkoracle::pred_core::Pred;

/// The shipped default set. If one of these stops parsing, every fork search
/// silently runs with fewer predicates than it was told to.
const SHIPPED: [&str; 3] = [
    "crash:speeddrop:frac=0.5,win=50,minpeak=15,after=200",
    "stuck:floor:speed=3,need=50,after=250",
    "off:offref:dist=20,need=10,after=200",
];

#[test]
fn the_shipped_predicates_parse() {
    for s in SHIPPED {
        let p = parse_spec(s).unwrap_or_else(|e| panic!("{}: {}", s, e));
        assert!(!p.name.is_empty());
    }
}

/// A typo in a threshold is exactly the mistake that silently kills good
/// candidates, so an unknown key or kind is an error and not a shrug.
#[test]
fn a_typo_is_refused_rather_than_ignored() {
    assert!(parse_spec("crash:speeddrop:frac=0.5,wim=50").is_err(), "unknown key accepted");
    assert!(parse_spec("crash:speedrop:frac=0.5").is_err(), "unknown kind accepted");
    assert!(parse_spec("nokind").is_err());
}

/// The wire round trip. `encode`/`decode` is how a predicate crosses into the
/// fork child; if it loses a field, the child watches for something the driver
/// did not ask for.
#[test]
fn a_predicate_survives_the_crossing_into_the_child() {
    for s in SHIPPED {
        let p = parse_spec(s).unwrap().pred;
        let mut buf = [0u8; forkoracle::pred_core::PRED_BYTES];
        p.encode(&mut buf);
        let q = Pred::decode(&buf);
        let mut buf2 = [0u8; forkoracle::pred_core::PRED_BYTES];
        q.encode(&mut buf2);
        assert_eq!(buf, buf2, "{} did not survive encode -> decode -> encode", s);
    }
}

/// THE PLATEAU. A reference line has stretches of identical points -- the
/// standing start, any stationary tick, a respawn. Tracking the nearest point
/// by hill-descent pins the match at the front of one and reports enormous
/// deviation for a run that is exactly on the line: measured 1123 m of apparent
/// error for the reference against ITSELF, and 0.00 m once the search became an
/// argmin over a window with ties broken to the later index.
#[test]
fn the_reference_line_is_zero_metres_from_itself_across_a_plateau() {
    let mut pts: Vec<[f64; 3]> = Vec::new();
    for _ in 0..40 {
        pts.push([0.0, 0.0, 0.0]); // the standing start: 0.4 s of not moving
    }
    for i in 0..200 {
        pts.push([i as f64 * 0.7, 0.0, 0.0]);
    }
    let line = RefLineData::from_points(&pts);
    for (i, p) in pts.iter().enumerate() {
        let (_, d) = line.nearest(*p);
        // 1 mm, not 0: the line is stored as f32, so a point is at best ~1e-6 m
        // from itself. The defect this pins was 1123 m.
        assert!(d < 1e-3, "point {} is {} m from the line it is on", i, d);
    }
}

#[test]
fn arclength_does_not_advance_while_the_car_is_stationary() {
    let mut pts: Vec<[f64; 3]> = vec![[0.0, 0.0, 0.0]; 10];
    for i in 1..=10 {
        pts.push([i as f64, 0.0, 0.0]);
    }
    let line = RefLineData::from_points(&pts);
    assert_eq!(line.s_at_tick(0), 0.0);
    assert_eq!(line.s_at_tick(9), 0.0, "the standing start accumulated arclength");
    assert!((line.s_at_tick(19) - 10.0).abs() < 1e-5);
    // past the end clamps rather than panicking: `s_at_tick(usize::MAX)` is how
    // the search asks for the line's total length.
    assert!((line.s_at_tick(usize::MAX) - 10.0).abs() < 1e-5);
}

#[test]
fn arclength_is_monotone() {
    let pts: Vec<[f64; 3]> = (0..500).map(|i| {
        let t = i as f64 * 0.02;
        [t.cos() * 50.0, t.sin() * 3.0, t.sin() * 50.0]
    }).collect();
    let line = RefLineData::from_points(&pts);
    for i in 1..line.n {
        assert!(line.s[i] >= line.s[i - 1], "arclength went backwards at {}", i);
    }
}
