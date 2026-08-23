//! The search loop, with a fake oracle.
//!
//! No server and no engine: the evaluator here is a few lines of arithmetic, so
//! these run in milliseconds and pin the parts of the loop that have actually
//! gone wrong in production.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tmsearch::guard::Provenance;
use forkoracle::inputs::Inputs;
use tmsearch::score::Outcome;
use tmsearch::search::Evaluator;

/// An evaluator that records the lowest tick at which any candidate it was
/// given differs from the reference, and pretends every candidate finishes at
/// a time derived from its steering.
struct Spy {
    floor: usize,
    reference: Inputs,
    lowest_edit: Arc<AtomicUsize>,
}

impl Evaluator for Spy {
    fn evaluate(&mut self, cands: &[Inputs]) -> Vec<Outcome> {
        for c in cands {
            if let Some(t) = c.distance_from(&self.reference).first_diff_tick {
                self.lowest_edit.fetch_min(t, Ordering::SeqCst);
            }
        }
        cands
            .iter()
            .map(|c| Outcome::Finish { ms: 20000 + c.steer.iter().map(|&s| s as i64).sum::<i64>() })
            .collect()
    }
    fn floor(&self) -> usize {
        self.floor
    }
    fn provenance(&self, _idx: usize, inputs: &Inputs) -> Provenance {
        Provenance {
            from_fork: true,
            resume_tick: Some(self.floor),
            distance: inputs.distance_from(&self.reference),
            gate: None,
            gate_edge: None,
        }
    }
}

fn flat(n: usize) -> Inputs {
    Inputs { steer: vec![0; n], gas: vec![true; n], brake: vec![false; n] }
}

/// THE PHANTOM FIX, tested directly.
///
/// Each fork worker's server stops where it stops, so each has its own resume
/// tick. An edit below a worker's own tick is a silent no-op: invisible to the
/// evaluator, present in the written file, scoring exactly the incumbent's
/// score, accepted at `delta == 0`. The mutation floor must therefore be the
/// MAXIMUM over workers -- not each worker's own, because migration moves a
/// state made by one worker into another -- and no worker may start before
/// every floor is known.
///
/// The old loop calibrated one boundary in the master and reused it: in one
/// real run 135 of 150 workers stopped past it.
#[test]
fn no_worker_ever_edits_below_the_highest_resume_tick() {
    let n = 600;
    let start = flat(n);
    let lowest = Arc::new(AtomicUsize::new(usize::MAX));
    let floors = [100usize, 170, 181, 140];

    let cfg = tmsearch::search::Config {
        workers: floors.len(),
        batch: 4,
        ops_per_candidate: tmsearch::search::OpsPerCandidate::Exactly(1),
        opset: forkoracle::inputs::OpSet::Wide,
        lo: 0,
        hi: n,
        window: 120,
        stride: 60,
        full_window_every: 4,
        minutes: 0.02,
        seed: 5,
        temp_s: 0.0,
        migrate: 0.2,
        max_drift: 0,
        check_seed_gate: None,
    };

    let seen = Arc::clone(&lowest);
    let reference = start.clone();
    let banked: Arc<Mutex<Vec<Outcome>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&banked);

    tmsearch::search::run_with_sink(
        &cfg,
        start.clone(),
        Outcome::Finish { ms: 20000 },
        move |o, _, _| {
            sink.lock().unwrap().push(o);
            Ok(o)
        },
        move |wi| {
            Ok(Spy {
                floor: floors[wi],
                reference: reference.clone(),
                lowest_edit: Arc::clone(&seen),
            })
        },
    );

    let lo = lowest.load(Ordering::SeqCst);
    assert!(lo != usize::MAX, "the fake oracle was never called");
    assert!(
        lo >= 181,
        "a candidate was edited at tick {}, below the highest worker resume tick (181). \
         Those edits are silently dropped by the engine and are exactly how a phantom \
         improvement is banked.",
        lo
    );
    assert!(!banked.lock().unwrap().is_empty(), "nothing was ever offered to the bank");
}

/// A worker that cannot start must not hang the fleet on the barrier.
#[test]
fn a_worker_that_fails_to_start_does_not_wedge_the_others() {
    let n = 300;
    let start = flat(n);
    let reference = start.clone();
    let lowest = Arc::new(AtomicUsize::new(usize::MAX));
    let cfg = tmsearch::search::Config {
        workers: 3,
        batch: 2,
        ops_per_candidate: tmsearch::search::OpsPerCandidate::Exactly(1),
        opset: forkoracle::inputs::OpSet::Local,
        lo: 0,
        hi: n,
        window: 100,
        stride: 50,
        full_window_every: 8,
        minutes: 0.01,
        seed: 1,
        temp_s: 0.0,
        migrate: 0.0,
        max_drift: 0,
        check_seed_gate: None,
    };
    let seen = Arc::clone(&lowest);
    // worker 1 refuses to start
    tmsearch::search::run_with_sink(
        &cfg,
        start,
        Outcome::Finish { ms: 20000 },
        |o, _, _| Ok(o),
        move |wi| {
            if wi == 1 {
                return Err("no fork server on this worker".to_string());
            }
            Ok(Spy { floor: 10, reference: reference.clone(), lowest_edit: Arc::clone(&seen) })
        },
    );
    // reaching here at all is the assertion: the barrier did not deadlock.
    assert!(lowest.load(Ordering::SeqCst) != usize::MAX, "the surviving workers never ran");
}

/// An evaluator whose objective is maximised by DOING NOTHING: the fewer
/// inputs, the better the score. This is the shape of every decoy -- a
/// quantity that goes up when the car does less -- and it is what the
/// startup test exists to catch before four hours are spent on it.
struct Lazy {
    /// `true` for the decoy; `false` for the honest twin, which rewards
    /// steering instead.
    rewards_doing_nothing: bool,
    calls: Arc<AtomicUsize>,
}

impl Evaluator for Lazy {
    fn evaluate(&mut self, cands: &[Inputs]) -> Vec<Outcome> {
        self.calls.fetch_add(cands.len(), Ordering::SeqCst);
        cands
            .iter()
            .map(|c| {
                let effort: f64 = c.steer.iter().map(|&s| (s as f64).abs()).sum();
                let key = if self.rewards_doing_nothing { -effort } else { effort };
                Outcome::Gate(tmsearch::score::GateState::Reached { key })
            })
            .collect()
    }
    fn provenance(&self, _idx: usize, inputs: &Inputs) -> Provenance {
        Provenance {
            from_fork: true,
            resume_tick: Some(0),
            distance: inputs.distance_from(inputs),
            gate: None,
            gate_edge: None,
        }
    }
}

fn lazy_cfg(n: usize) -> tmsearch::search::Config {
    tmsearch::search::Config {
        workers: 2,
        batch: 4,
        ops_per_candidate: tmsearch::search::OpsPerCandidate::Exactly(1),
        opset: forkoracle::inputs::OpSet::Wide,
        lo: 0,
        hi: n,
        window: 100,
        stride: 50,
        full_window_every: 8,
        minutes: 0.02,
        seed: 7,
        temp_s: 0.0,
        migrate: 0.0,
        max_drift: 0,
        check_seed_gate: None,
    }
}

/// THE DECOY TEST, and it must fire BEFORE the first candidate.
///
/// > An objective that can be maximised without achieving the goal is not a
/// > proxy, it is a decoy.
///
/// Here the objective is a pure decoy: the blank tape wins it outright. The run
/// must stop having offered nothing to the bank, and it must have paid for only
/// the two evaluations the test itself costs -- not a batch, and certainly not
/// a search.
#[test]
fn an_objective_the_do_nothing_tape_wins_stops_before_the_first_candidate() {
    let n = 400;
    let start = Inputs { steer: vec![40; n], gas: vec![true; n], brake: vec![false; n] };
    let calls = Arc::new(AtomicUsize::new(0));
    let banked: Arc<Mutex<Vec<Outcome>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&banked);
    let c = Arc::clone(&calls);

    let out = tmsearch::search::run_with_sink(
        &lazy_cfg(n),
        start,
        Outcome::Gate(tmsearch::score::GateState::Missed { miss_m: f64::INFINITY }),
        move |o, _, _| {
            sink.lock().unwrap().push(o);
            Ok(o)
        },
        move |_wi| Ok(Lazy { rewards_doing_nothing: true, calls: Arc::clone(&c) }),
    );

    assert!(banked.lock().unwrap().is_empty(), "a decoy objective banked a result");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the run did not stop at the decoy test: it evaluated {} candidates",
        calls.load(Ordering::SeqCst)
    );
    assert!(matches!(out, Outcome::Gate(tmsearch::score::GateState::Missed { .. })));
}

/// The positive control for the test above. The same machinery, the same seed,
/// the same evaluator with its sign the other way up: an objective the blank
/// tape LOSES must run normally. Without this the test above passes for a
/// search that never starts at all.
#[test]
fn an_objective_the_do_nothing_tape_loses_runs_normally() {
    let n = 400;
    let start = Inputs { steer: vec![40; n], gas: vec![true; n], brake: vec![false; n] };
    let calls = Arc::new(AtomicUsize::new(0));
    let banked: Arc<Mutex<Vec<Outcome>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&banked);
    let c = Arc::clone(&calls);

    tmsearch::search::run_with_sink(
        &lazy_cfg(n),
        start,
        Outcome::Gate(tmsearch::score::GateState::Missed { miss_m: f64::INFINITY }),
        move |o, _, _| {
            sink.lock().unwrap().push(o);
            Ok(o)
        },
        move |_wi| Ok(Lazy { rewards_doing_nothing: false, calls: Arc::clone(&c) }),
    );

    assert!(
        calls.load(Ordering::SeqCst) > 100,
        "the honest twin only evaluated {} candidates -- the decoy test stopped a good run",
        calls.load(Ordering::SeqCst)
    );
    assert!(!banked.lock().unwrap().is_empty(), "nothing was ever offered to the bank");
}

/// The do-nothing tape is the search's own action space, not the whole tape:
/// everything below the resume floor is already consumed by the engine and
/// cannot be blanked, so blanking it would measure a tape nothing can write.
#[test]
fn the_do_nothing_tape_only_blanks_what_the_search_may_edit() {
    let n = 100;
    let seed = Inputs { steer: vec![40; n], gas: vec![true; n], brake: vec![true; n] };
    let d = tmsearch::search::Decoy::do_nothing(&seed, 30, 70);
    for t in 0..30 {
        assert_eq!(d.steer[t], 40, "tick {} was blanked below the floor", t);
        assert!(d.gas[t]);
    }
    for t in 30..70 {
        assert_eq!(d.steer[t], 0);
        assert!(!d.gas[t]);
        assert!(!d.brake[t]);
    }
    for t in 70..n {
        assert_eq!(d.steer[t], 40, "tick {} was blanked above the window", t);
    }
}
