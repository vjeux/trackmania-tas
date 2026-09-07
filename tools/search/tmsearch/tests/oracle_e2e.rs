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
use tmsearch::score::{GateState, Outcome, Progress};
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
        gate_edge: None,
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
        .offer(&p, &p.template, Outcome::fin(TRUTH_MS), &nowhere())
        .expect("a true claim was refused");
    assert_eq!(b.confirmed, Outcome::fin(TRUTH_MS));
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
    let lie = Outcome::fin(TRUTH_MS - 500);
    let err = bank
        .offer(&p, &p.template, lie, &nowhere())
        .expect_err("the guard banked a time the tape does not achieve");
    assert_eq!(err.claimed, lie);
    assert_eq!(err.actual, Some(Outcome::fin(TRUTH_MS)));
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
/// and the oracle's answer are the same statement -- with the one exception
/// the guard makes on purpose. This template's tape ends one tick after its
/// own finish, so a mutant that is SLOWER finishes after the tape's last
/// record; the oracle still prints a time for it, and that time is not a
/// result (SEARCH.md §3): the guard must refuse it as `PHANTOM_pastend_*`. A
/// mutant that finishes inside its tape is banked under the oracle's time.
/// This is the loop the search runs thousands of times, once.
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
        Some(ms) => Outcome::fin(ms),
        None => Outcome::Dnf(Progress::Checkpoints { cps: truth.cps.unwrap_or(0), seg_ms: None }),
    };
    let _ = std::fs::remove_file(&f);

    let end_ms = p.start_offset_ms as i64 + 10 * p.n() as i64;
    match truth.time_ms {
        Some(ms) if ms > end_ms => {
            let err = bank
                .offer(&p, &s, claim, &nowhere())
                .expect_err("a finish after the tape's own end was banked");
            assert!(
                err.path.file_name().unwrap().to_string_lossy().starts_with("PHANTOM_pastend_"),
                "refused, but not as a past-the-end finish: {}",
                err.path.display()
            );
        }
        _ => {
            let banked = bank.offer(&p, &s, claim, &nowhere()).expect("the oracle's own answer was refused");
            assert_eq!(banked.confirmed, claim);
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// A gate record, for the two tests below. The numbers are a plausible state;
/// what matters is that one travels with the claim into the bank.
fn a_gate_state() -> forkoracle::pred::GateRecord {
    forkoracle::pred::GateRecord {
        tick: 2013,
        key: 57.2294,
        pos: [56.08, 50.08, 709.18],
        vel: [-102.40, -1.89, -11.45],
        quat: [0.4215, 0.0297, -0.9062, -0.0165],
    }
}

fn with_gate() -> Provenance {
    let mut p = nowhere();
    p.gate = Some(a_gate_state());
    p
}

/// BAND 2 IS A TIME AND IS HELD TO A TIME'S STANDARD. "It reached the gate AND
/// finished" is the one gate band that carries a millisecond, so the guard must
/// refuse it on exactly the terms it refuses any other false time -- otherwise
/// a state objective would be a way to put an unchecked number in the bank.
#[test]
fn the_guard_refuses_a_gate_finish_the_tape_does_not_achieve() {
    let Some(srv) = server() else { return };
    let p = Patcher::build(GHOST).unwrap();
    let d = scratch("guard-gate-phantom");
    let mut bank = Bank::new(&d, &srv, Path::new(MAP), None).unwrap();
    let lie = Outcome::Gate(GateState::Finished { ms: TRUTH_MS - 500 });
    let err = bank
        .offer(&p, &p.template, lie, &with_gate())
        .expect_err("the guard banked a gate finish the tape does not achieve");
    assert_eq!(err.actual, Some(Outcome::fin(TRUTH_MS)));
    assert_eq!(bank.phantoms, 1);
    assert_eq!(bank.confirmed, 0);
    let _ = std::fs::remove_dir_all(&d);
}

/// AND A STATE IS BANKED AS A STATE. Bands 0 and 1 carry no millisecond, so
/// there is nothing for the oracle to contradict -- what the bank must do
/// instead is write the measurement down beside the tape, in the units it was
/// measured in, so the claim can be checked by hand. The banked file must NOT
/// be named as a time.
#[test]
fn a_state_is_banked_with_its_measurement_beside_it() {
    let Some(srv) = server() else { return };
    let p = Patcher::build(GHOST).unwrap();
    let d = scratch("guard-gate-state");
    let mut bank = Bank::new(&d, &srv, Path::new(MAP), None).unwrap();
    let claim = Outcome::Gate(GateState::Reached { key: 57.2294 });
    let b = bank.offer(&p, &p.template, claim, &with_gate()).expect("a state claim was refused");
    assert_eq!(b.confirmed, claim, "the bank changed a state into something else");
    assert!(b.path.exists());
    let name = b.path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.contains("gate"), "{} does not say it is a state", name);
    assert!(
        !name.contains("22_730"),
        "{} is named as a time the search never claimed",
        name
    );
    let side = b.path.with_extension("state.json");
    let text = std::fs::read_to_string(&side).expect("the measurement was not written beside it");
    for want in ["gate_tick", "\"key\"", "quat", "body_right", "709.18"] {
        assert!(text.contains(want), "the sidecar is missing {}: {}", want, text);
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// A FAILURE COMES BACK ON THE LADDER THE SEARCH RANKS ON.
///
/// The plain oracle only ever reports checkpoints. A fork search ranks failures
/// by METRES along the reference line, and a plain search with segment maps
/// ranks them by checkpoints WITH a time. Handing either of those back as a
/// bare `Checkpoints { cps, seg_ms: None }` -- which is what the bank used to
/// do -- returns a value from a different ladder, so `confirmed > incumbent`
/// compares two unrelated numbers and the improvement is confirmed, written to
/// disk, and never adopted. The search then reports improvements all afternoon
/// while its incumbent never moves.
///
/// Found by the state objective walking into the same wall: 49 confirmations,
/// zero adopted.
#[test]
fn a_failure_is_banked_on_the_ladder_the_search_ranks_on() {
    let Some(srv) = server() else { return };
    let p = Patcher::build(GHOST).unwrap();
    let d = scratch("guard-ladder");

    // a tape that does not finish: full lock, held, from well before the end
    let mut s = p.template.clone();
    for t in 1200..1400 {
        s.steer[t] = 127;
    }
    let f = d.join("probe.Ghost.Gbx");
    std::fs::write(&f, p.file(&s)).unwrap();
    let truth = validate(&srv, &f, MapsMode::One(Path::new(MAP)), "ladder").unwrap();
    let _ = std::fs::remove_file(&f);
    assert!(truth.time_ms.is_none(), "the probe tape finishes, so it cannot pin this");

    let mut bank = Bank::new(&d, &srv, Path::new(MAP), None).unwrap();
    let claim = Outcome::Dnf(Progress::Metres { m: 1234.5, of: 1998.0 });
    let b = bank.offer(&p, &s, claim, &nowhere()).expect("a failure claim was refused");
    assert_eq!(
        b.confirmed, claim,
        "the bank returned a failure on a different ladder from the one the search ranks on"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// What a verdict looks like when two of them are compared: the same claim
/// must get the same answer whichever way it was certified.
fn shape(r: &Result<tmsearch::guard::Banked, tmsearch::guard::Phantom>) -> String {
    match r {
        Ok(b) => format!("OK {}", b.confirmed),
        Err(ph) => format!(
            "PHANTOM claimed {} actual {} kind {}",
            ph.claimed,
            ph.actual.map(|a| a.to_string()).unwrap_or_else(|| "none".into()),
            ph.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .split('_')
                .nth(1)
                .unwrap_or("")
                .to_string()
        ),
    }
}

/// BATCHED CERTIFICATION (PERF.md §7): `offer_many` on N mutated tapes must
/// return, claim for claim, the verdict `offer` returns one launch at a time --
/// confirmed with the same time, or refused for the same reason with the same
/// oracle answer. `TM_CERT_N` sets N (default 30; the proof run used 500), and
/// the wall time of both ways is printed.
///
/// The claims are the tapes' own oracle times where the oracle finished them
/// (so most confirm) and a lie of 22.000 on every third tape (so the phantom
/// path is exercised in the same batch); a DNF is claimed as a DNF.
#[test]
fn offer_many_gives_every_claim_the_verdict_offer_gives_it_alone() {
    let Some(srv) = server() else { return };
    let n: usize = std::env::var("TM_CERT_N").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
    let p = Patcher::build(GHOST).unwrap();
    let mut rng = Rng::new(11);
    let mut tapes = Vec::with_capacity(n);
    for _ in 0..n {
        let mut s = p.template.clone();
        // one to three local edits in the last third: finishes, DNFs and
        // past-the-end finishes all occur
        for _ in 0..(1 + (rng.next_u64() % 3) as usize) {
            mutate(&mut s, &mut rng, 1500, 2400, OpSet::Local);
        }
        tapes.push(s);
    }
    // The truth, in one launch, to build the claims.
    let d = scratch("cert-truth");
    let files: Vec<PathBuf> = tapes
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let f = d.join(format!("t{}.Ghost.Gbx", i));
            std::fs::write(&f, p.file(s)).unwrap();
            f
        })
        .collect();
    let refs: Vec<&Path> = files.iter().map(|f| f.as_path()).collect();
    let truth = ghost::oracle::validate_many(&srv, &refs, MapsMode::One(Path::new(MAP)), "cert-truth").unwrap();
    let claims: Vec<Outcome> = files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            let r = truth.iter().find(|r| r.file == name).expect("the oracle read every file");
            match r.time_ms {
                Some(_) if i % 3 == 2 => Outcome::fin(22000),
                Some(ms) => Outcome::fin(ms),
                None => Outcome::Dnf(Progress::Checkpoints { cps: r.cps.unwrap_or(0), seg_ms: None }),
            }
        })
        .collect();
    let _ = std::fs::remove_dir_all(&d);

    // ONE AT A TIME.
    let d1 = scratch("cert-one");
    let mut one = Bank::new(&d1, &srv, Path::new(MAP), None).unwrap();
    let t = std::time::Instant::now();
    let single: Vec<String> = tapes
        .iter()
        .zip(&claims)
        .map(|(s, c)| shape(&one.offer(&p, s, *c, &nowhere())))
        .collect();
    let t_one = t.elapsed();

    // ALL AT ONCE (in the search's batches of CERT_BATCH).
    let d2 = scratch("cert-many");
    let mut many = Bank::new(&d2, &srv, Path::new(MAP), None).unwrap();
    let t = std::time::Instant::now();
    let mut batched: Vec<String> = Vec::with_capacity(n);
    for chunk in tapes.iter().zip(&claims).collect::<Vec<_>>().chunks(tmsearch::search::CERT_BATCH) {
        let cs: Vec<(&forkoracle::inputs::Inputs, Outcome, &Provenance)> =
            chunk.iter().map(|(s, c)| (*s, **c, &NOWHERE)).collect();
        batched.extend(many.offer_many(&p, &cs).iter().map(shape));
    }
    let t_many = t.elapsed();

    let mut differ = 0;
    for (i, (a, b)) in single.iter().zip(&batched).enumerate() {
        if a != b {
            differ += 1;
            if differ <= 5 {
                eprintln!("claim {}: alone -> {} | batched -> {}", i, a, b);
            }
        }
    }
    let confirmed = single.iter().filter(|s| s.starts_with("OK")).count();
    let pastend = single.iter().filter(|s| s.contains("kind pastend")).count();
    eprintln!(
        "certification: {} claims ({} confirmed, {} phantoms of which {} finished after their own tape); \
         one at a time {:.1} s, batched by {} {:.1} s ({:.1}x); {} verdicts differ",
        n,
        confirmed,
        n - confirmed,
        pastend,
        t_one.as_secs_f64(),
        tmsearch::search::CERT_BATCH,
        t_many.as_secs_f64(),
        t_one.as_secs_f64() / t_many.as_secs_f64().max(1e-9),
        differ
    );
    assert_eq!(differ, 0, "batched certification changed {} verdict(s)", differ);
    assert_eq!(one.confirmed, many.confirmed);
    assert_eq!(one.phantoms, many.phantoms);
    let _ = std::fs::remove_dir_all(&d1);
    let _ = std::fs::remove_dir_all(&d2);
}

static NOWHERE: Provenance = Provenance {
    from_fork: false,
    resume_tick: None,
    distance: Distance { first_diff_tick: None, diff_ticks: 0, ticks: 0, max_steer_delta: 0 },
    gate: None,
    gate_edge: None,
};

/// A FINISH AFTER THE TAPE'S OWN LAST RECORD IS REFUSED, whatever the plain
/// oracle said its time was (SEARCH.md §3, PERF.md §1): the template braked
/// over its last ticks finishes a few ticks after its own end, the oracle
/// reports a time for it, and the guard must keep it as `PHANTOM_pastend_*`
/// rather than bank a millisecond that depends on the batch it was in.
#[test]
fn the_guard_refuses_a_finish_after_the_tapes_own_end() {
    let Some(srv) = server() else { return };
    let p = Patcher::build(GHOST).unwrap();
    let n = p.n();
    let mut s = p.template.clone();
    for t in n - 30..n {
        // brake, no gas, straight, over the last 0.3 s
        s.steer[t] = 0;
        s.gas[t] = false;
        s.brake[t] = true;
    }
    let d = scratch("guard-pastend");
    let f = d.join("late.Ghost.Gbx");
    std::fs::write(&f, p.file(&s)).unwrap();
    let truth = validate(&srv, &f, MapsMode::One(Path::new(MAP)), "pastend").unwrap();
    let end_ms = p.start_offset_ms as i64 + 10 * n as i64;
    let Some(ms) = truth.time_ms else {
        eprintln!("SKIP: braking the last 0.3 s made the template a DNF, not a late finish");
        let _ = std::fs::remove_dir_all(&d);
        return;
    };
    assert!(ms > end_ms, "the braked template finished at {} inside its tape (end {}); the fixture no longer makes a late finish", ms, end_ms);
    let mut bank = Bank::new(&d, &srv, Path::new(MAP), None).unwrap();
    let err = bank
        .offer(&p, &s, Outcome::fin(ms), &nowhere())
        .expect_err("the guard banked a finish that happened after the tape's last record");
    assert!(
        err.path.file_name().unwrap().to_string_lossy().starts_with("PHANTOM_pastend_"),
        "refused, but not as a past-the-end finish: {}",
        err.path.display()
    );
    assert_eq!(err.actual, None);
    assert_eq!(bank.phantoms, 1);
    assert_eq!(bank.confirmed, 0);
    let _ = std::fs::remove_dir_all(&d);
}
