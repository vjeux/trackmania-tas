//! The search loop, with a fake oracle.
//!
//! No server and no engine: the evaluator here is a few lines of arithmetic, so
//! these run in milliseconds and pin the parts of the loop that have actually
//! gone wrong in production.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tmsearch::guard::Provenance;
use forkoracle::inputs::{Distance, Inputs};
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
    fn provenance(&self, inputs: &Inputs) -> Provenance {
        Provenance {
            from_fork: true,
            resume_tick: Some(self.floor),
            distance: inputs.distance_from(&self.reference),
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
