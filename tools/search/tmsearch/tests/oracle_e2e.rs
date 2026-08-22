//! End to end, against the real dedicated server.
//!
//! Set `TM_SERVER` to a server directory and these run; without it they skip,
//! and say so. A skipped check is not a passing check -- `cargo test` prints
//! the reason, and CI on a box with an engine should treat a skip as a failure.
//!
//! The fixtures are the two human ghosts and the map in `tools/testdata`, the
//! corpus every crate shares, resolved from this crate's own manifest
//! directory: a fixture path relative to the CWD gives a different answer
//! depending on where you stand, and these pointed at `tools/ghost/testdata`,
//! which the audit merged into the shared corpus. They had been silently
//! skipping ever since -- on a box with no server they skip, and on a box with
//! one they died on a missing file.


use ghost::oracle::{server_dir, validate, MapsMode};
use std::path::{Path, PathBuf};
use tmsearch::guard::{Bank, Provenance};
use forkoracle::inputs::{mutate, Distance, OpSet, Rng};
use tmsearch::score::{Outcome, Progress};
use tmsearch::tape::Patcher;

const GHOST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/human_22730.Ghost.Gbx");
const MAP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/map2.Map.Gbx");
/// What the engine gets when it re-simulates that file's own tape.
const TRUTH_MS: i64 = 22730;

fn server() -> Option<PathBuf> {
    let d = server_dir(None);
    if d.join("TrackmaniaServer").exists() {
        return Some(d);
    }
    // A SKIP IS NOT A PASS. The suite this one replaces reported "6 passed" on
    // any machine without its fixtures -- every check was wrapped in
    // `if !path.exists() { return }` against an absolute path outside the
    // repo, so it was green, in 0.00 s, having asserted nothing. On a box with
    // an engine, set TM_REQUIRE_ENGINE=1 and a missing server is a failure
    // rather than a silence.
    assert!(
        std::env::var("TM_REQUIRE_ENGINE").is_err(),
        "TM_REQUIRE_ENGINE is set and there is no dedicated server at {} (set TM_SERVER)",
        d.display()
    );
    eprintln!("SKIP: no dedicated server at {} (set TM_SERVER)", d.display());
    None
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tmsearch-e2e-{}-{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn nowhere() -> Provenance {
    Provenance {
        from_fork: false,
        resume_tick: None,
        distance: Distance {
            first_diff_tick: None,
            diff_ticks: 0,
            ticks: 0,
            max_steer_delta: 0,
        },
        gate: None,
    }
}

/// THE POSITIVE CONTROL for the whole stack: the template, written back out
/// through the patcher, must re-simulate to the time the original file does.
/// If this fails, nothing else in the suite means anything.
#[test]
fn the_patcher_reproduces_the_template_through_the_real_engine() {
    let Some(srv) = server() else { return };
    let p = Patcher::build(GHOST).unwrap();
    let d = scratch("identity");
    let f = d.join("identity.Ghost.Gbx");
    std::fs::write(&f, p.file(&p.template)).unwrap();
    let r = validate(&srv, &f, MapsMode::One(Path::new(MAP)), "identity").unwrap();
    assert_eq!(r.time_ms, Some(TRUTH_MS), "the rewritten template does not do what the original does");
    let _ = std::fs::remove_dir_all(&d);
}

/// The guard accepts a claim the oracle agrees with...
#[test]
fn the_guard_banks_a_true_claim() {
    let Some(srv) = server() else { return };
    let p = Patcher::build(GHOST).unwrap();
    let d = scratch("guard-true");
    let mut bank = Bank::new(&d, &srv, Path::new(MAP), None).unwrap();
    let b = bank
        .offer(&p, &p.template, Outcome::Finish { ms: TRUTH_MS }, &nowhere())
        .expect("a true claim was refused");
    assert_eq!(b.confirmed, Outcome::Finish { ms: TRUTH_MS });
    assert!(b.path.exists());
    assert_eq!(bank.phantoms, 0);
    let _ = std::fs::remove_dir_all(&d);
}

/// ...AND REFUSES ONE IT DOES NOT. This is the check that makes the other one
/// mean something: a guard that cannot fail is not a guard, and every phantom
/// this project has shipped got through a step that could only pass.
///
/// The claim here is a lie of the exact shape a phantom is -- a finish time the
/// file does not achieve -- and the guard must keep the tape, name it, count
/// it, and refuse.
#[test]
fn the_guard_refuses_a_time_the_tape_does_not_achieve() {
    let Some(srv) = server() else { return };
    let p = Patcher::build(GHOST).unwrap();
    let d = scratch("guard-phantom");
    let mut bank = Bank::new(&d, &srv, Path::new(MAP), None).unwrap();
    let lie = Outcome::Finish { ms: TRUTH_MS - 500 };
    let err = bank
        .offer(&p, &p.template, lie, &nowhere())
        .expect_err("the guard banked a time the tape does not achieve");
    assert_eq!(err.claimed, lie);
    assert_eq!(err.actual, Some(Outcome::Finish { ms: TRUTH_MS }));
    assert!(err.path.file_name().unwrap().to_string_lossy().starts_with("PHANTOM_"));
    assert_eq!(bank.phantoms, 1);
    assert_eq!(bank.confirmed, 0);
    // and nothing was left in the bank pretending to be an improvement
    let bests: Vec<_> = std::fs::read_dir(&d)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("best_"))
        .collect();
    assert!(bests.is_empty(), "a refused claim still produced a best_ file");
    let _ = std::fs::remove_dir_all(&d);
}

/// A DNF claim must not be confirmable as a finish either: the guard compares
/// kinds, not just numbers.
#[test]
fn the_guard_refuses_a_dnf_claim_for_a_tape_that_finishes() {
    let Some(srv) = server() else { return };
    let p = Patcher::build(GHOST).unwrap();
    let d = scratch("guard-kind");
    let mut bank = Bank::new(&d, &srv, Path::new(MAP), None).unwrap();
    let claim = Outcome::Dnf(Progress::Checkpoints { cps: 2, seg_ms: None });
    assert!(
        bank.offer(&p, &p.template, claim, &nowhere()).is_err(),
        "a finishing tape was banked under a DNF claim"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// A mutated candidate: whatever the oracle says about it, the guard's verdict
/// and the oracle's answer are the same statement. This is the loop the search
/// runs thousands of times, once.
#[test]
fn a_mutated_candidate_is_banked_only_under_the_time_it_actually_does() {
    let Some(srv) = server() else { return };
    let p = Patcher::build(GHOST).unwrap();
    let d = scratch("guard-mutated");
    let mut bank = Bank::new(&d, &srv, Path::new(MAP), None).unwrap();
    let mut rng = Rng::new(3);
    let mut s = p.template.clone();
    mutate(&mut s, &mut rng, 1500, 1700, OpSet::Local);

    let f = d.join("probe.Ghost.Gbx");
    std::fs::write(&f, p.file(&s)).unwrap();
    let truth = validate(&srv, &f, MapsMode::One(Path::new(MAP)), "mutated").unwrap();
    let claim = match truth.time_ms {
        Some(ms) => Outcome::Finish { ms },
        None => Outcome::Dnf(Progress::Checkpoints { cps: truth.cps.unwrap_or(0), seg_ms: None }),
    };
    let _ = std::fs::remove_file(&f);

    let banked = bank.offer(&p, &s, claim, &nowhere()).expect("the oracle's own answer was refused");
    assert_eq!(banked.confirmed, claim);
    let _ = std::fs::remove_dir_all(&d);
}
