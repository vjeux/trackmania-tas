//! The search itself: islands of independent incumbents, annealed, over
//! whatever evaluator it is given.
//!
//! # The three ideas, and the failure each one fixes
//!
//! The plain version is a strict hill climber on a scalar that is ABSENT for
//! 57-79% of candidates -- a run that does not finish returns no time. It
//! converges in minutes and then never moves again, from any seed.
//!
//! 1. **A dense objective.** [`crate::score::Outcome`] gives a failure a rank
//!    (checkpoints and a segment-map time, or metres along the reference line),
//!    so a failed run has a gradient. Critically, the true objective is still
//!    the true objective: a finisher always outranks a non-finisher. The naive
//!    "optimise the segment" ladder got that backwards and its fastest-to-CP3
//!    line finished a full second slower on the real map.
//! 2. **Metropolis acceptance.** Improvement-only acceptance is what makes a
//!    hill climber stop. Accepting a slightly worse candidate with probability
//!    `exp(-delta/T)` lets a worker walk out of a local optimum. `T` is in
//!    SECONDS of race time, and it applies **only between two finishers** --
//!    there is no temperature in units of "checkpoints".
//! 3. **Islands.** Every worker keeps its own incumbent and anneals it
//!    independently, so the search holds several distinct lines at once instead
//!    of collapsing onto the first good one. Workers reseed from the global
//!    best with probability `--migrate`.
//!
//! # The startup barrier is not a nicety
//!
//! A fork evaluator can only see an edit at or after the tick its own server
//! stopped at, and **that tick is per worker** -- the `lroundf` checkpoint is
//! not a fixed simulation point, so 135 of 150 workers in one real run stopped
//! past the master's single calibration. An edit below a worker's own resume
//! tick is a silent no-op: invisible to the evaluator, present in the written
//! file, scoring exactly the incumbent's score, accepted at `delta == 0`, and
//! contaminating that worker's lineage for free.
//!
//! So every worker publishes its own floor, a barrier holds the whole fleet
//! until they all have, and the mutation floor is the **maximum** over workers.
//! It must be the maximum and not each worker's own: migration moves a state
//! made by one worker into another.

use crate::guard::{Bank, Provenance};
use forkoracle::inputs::{mutate, Inputs, Op, OpSet, Rng};
use crate::report::{delta, elapsed};
use crate::score::Outcome;
use crate::tape::Patcher;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Barrier, RwLock};
use std::time::Instant;

/// Anything that can put a number on a batch of candidates.
///
/// A BATCH, not a candidate: the plain oracle's cost is dominated by the server
/// launch, so evaluating thirty candidates in one call is roughly thirty times
/// cheaper than thirty calls. The fork evaluator serves the batch one candidate
/// at a time and does not care.
pub trait Evaluator: Send {
    fn evaluate(&mut self, cands: &[Inputs]) -> Vec<Outcome>;

    /// The first tick an edit made by this evaluator's worker is guaranteed to
    /// be visible at. Everything below it is already consumed.
    fn floor(&self) -> usize {
        0
    }

    /// Where this evaluator's answer came from, for the record.
    fn provenance(&self, inputs: &Inputs) -> Provenance;

    /// Release the evaluator's resources (a fork server, in practice).
    fn finish(self: Box<Self>) {}
}

pub struct Config {
    pub workers: usize,
    pub batch: usize,
    pub ops_per_candidate: OpsPerCandidate,
    pub opset: OpSet,
    /// Tick range the search may edit, before the evaluators' own floor is
    /// applied.
    pub lo: usize,
    pub hi: usize,
    pub window: usize,
    pub stride: usize,
    /// Every Nth window is the whole range instead of a sliding one, so a
    /// search that has settled inside one window can still make a global move.
    pub full_window_every: u64,
    pub minutes: f64,
    pub seed: u64,
    /// Metropolis temperature in SECONDS. 0 turns annealing off.
    pub temp_s: f64,
    pub migrate: f64,
    /// Stop the run once a banked result sits more than this many ticks from
    /// the reference the fork server checkpointed on. 0 disables it.
    ///
    /// The fork oracle is exact for a small, late perturbation of its
    /// reference and **lies far from it** -- 0 of 312 fork-reported finishes
    /// survived a plain re-validation once the tape was not that. The guard
    /// means nothing false gets banked either way; what drifts is the fork's
    /// RANKING, so a search that has wandered is burning hours ordering
    /// candidates by a number that no longer means anything. The cure is to
    /// re-anchor: restart with `--start-from` the banked file, which gives the
    /// fork servers a reference the search is actually near.
    pub max_drift: usize,
}

#[derive(Clone, Copy)]
pub enum OpsPerCandidate {
    Exactly(usize),
    UpTo(usize),
}

impl OpsPerCandidate {
    fn draw(&self, rng: &mut Rng) -> usize {
        match *self {
            OpsPerCandidate::Exactly(k) => k,
            OpsPerCandidate::UpTo(k) => rng.range(1, k as i64) as usize,
        }
    }
}

struct Best {
    inputs: Inputs,
    outcome: Outcome,
}

struct Report {
    outcome: Outcome,
    inputs: Inputs,
    op: Option<Op>,
    prov: Provenance,
    evals: u64,
    finished: u64,
}

/// Run the search, banking through `sink`.
///
/// `sink` is handed every candidate that beats the global incumbent and returns
/// the outcome that was actually CONFIRMED -- which is not necessarily the one
/// it was offered. Returning `Err` stops the run: that is how the guard refuses
/// a phantom, and the incumbent is left where it was.
///
/// `make` builds one evaluator per worker, on that worker's own thread, because
/// a fork server belongs to the process that started it.
pub fn run_with_sink<E, F, S>(
    cfg: &Config,
    start: Inputs,
    start_outcome: Outcome,
    mut sink: S,
    make: F,
) -> Outcome
where
    E: Evaluator + 'static,
    F: Fn(usize) -> Result<E, String> + Send + Sync + 'static,
    S: FnMut(Outcome, &Inputs, &Provenance) -> Result<Outcome, ()>,
{
    let n = start.len();
    let best = Arc::new(RwLock::new(Best { inputs: start.clone(), outcome: start_outcome }));
    let stop = Arc::new(AtomicBool::new(false));
    let wincount = Arc::new(AtomicU64::new(0));
    let floor = Arc::new(AtomicUsize::new(cfg.lo.min(n)));
    let ready = Arc::new(Barrier::new(cfg.workers + 1));
    let make = Arc::new(make);

    let (tx, rx) = mpsc::channel::<Report>();
    let t0 = Instant::now();
    let mut handles = Vec::new();

    for wi in 0..cfg.workers {
        let (best, stop, wincount, floor, ready, make) = (
            Arc::clone(&best),
            Arc::clone(&stop),
            Arc::clone(&wincount),
            Arc::clone(&floor),
            Arc::clone(&ready),
            Arc::clone(&make),
        );
        let tx = tx.clone();
        let (batch, opc, opset) = (cfg.batch, cfg.ops_per_candidate, cfg.opset);
        let (window, stride, every) = (cfg.window, cfg.stride, cfg.full_window_every);
        let (seed, temp_s, migrate) = (cfg.seed, cfg.temp_s, cfg.migrate);
        let (cfg_lo, cfg_hi) = (cfg.lo, cfg.hi);

        handles.push(std::thread::spawn(move || {
            let mut ev = match make(wi) {
                Ok(e) => Box::new(e),
                Err(e) => {
                    eprintln!("worker {}: ABORT, {}", wi, e);
                    // Still meet the barrier, or the whole fleet hangs waiting
                    // for a worker that will never arrive.
                    ready.wait();
                    return;
                }
            };
            // Publish this worker's own floor, then wait for everyone.
            floor.fetch_max(ev.floor(), Ordering::SeqCst);
            ready.wait();
            let flo = floor.load(Ordering::SeqCst).max(cfg_lo).min(n);
            let fhi = cfg_hi.min(n);
            if fhi <= flo + 1 {
                eprintln!("worker {}: nothing to search in [{}, {})", wi, flo, fhi);
                return;
            }
            let nwin = ((fhi - flo).saturating_sub(window) / stride.max(1)).max(1);

            let mut rng = Rng::new(seed ^ ((wi as u64 + 1) << 32));
            let (mut cur, mut cur_out) = {
                let g = best.read().unwrap();
                (g.inputs.clone(), g.outcome)
            };

            while !stop.load(Ordering::Relaxed) {
                if migrate > 0.0 && rng.unit() < migrate {
                    let g = best.read().unwrap();
                    if g.outcome > cur_out {
                        cur = g.inputs.clone();
                        cur_out = g.outcome;
                    }
                }
                let wc = wincount.fetch_add(1, Ordering::Relaxed);
                let (lo, hi) = if every > 0 && wc % every == every - 1 {
                    (flo, fhi)
                } else {
                    let k = (wc as usize) % nwin;
                    (flo + k * stride, (flo + k * stride + window).min(fhi))
                };

                let mut cands = Vec::with_capacity(batch);
                let mut ops = Vec::with_capacity(batch);
                for _ in 0..batch {
                    let mut s = cur.clone();
                    let mut op = None;
                    for _ in 0..opc.draw(&mut rng) {
                        op = Some(mutate(&mut s, &mut rng, lo, hi, opset));
                    }
                    cands.push(s);
                    ops.push(op);
                }
                let outs = ev.evaluate(&cands);
                let evals = outs.len() as u64;
                let finished = outs.iter().filter(|o| o.is_finish()).count() as u64;

                let mut bi = usize::MAX;
                let mut best_out: Option<Outcome> = None;
                for (i, o) in outs.iter().enumerate() {
                    if best_out.map(|b| *o > b).unwrap_or(true) {
                        best_out = Some(*o);
                        bi = i;
                    }
                }
                if bi == usize::MAX {
                    continue;
                }
                let bo = best_out.unwrap();
                let best_inputs = cands[bi].clone();
                let best_op = ops[bi].clone();

                // METROPOLIS. An improvement is always taken. A regression is
                // taken with probability exp(-delta/T) -- and only when both
                // sides are finishers, because that is the only case where
                // `delta` is a number of seconds.
                let accept = if bo >= cur_out {
                    true
                } else if temp_s > 0.0 {
                    match bo.delta_ms(&cur_out) {
                        Some(d) => rng.unit() < (d as f64 / (temp_s * 1000.0)).exp(),
                        None => false,
                    }
                } else {
                    false
                };
                if accept {
                    cur = best_inputs.clone();
                    cur_out = bo;
                }

                let prov = ev.provenance(&best_inputs);
                if tx
                    .send(Report { outcome: bo, inputs: best_inputs, op: best_op, prov, evals, finished })
                    .is_err()
                {
                    break;
                }
            }
            ev.finish();
        }));
    }
    drop(tx);
    ready.wait();
    eprintln!(
        "all {} workers up; mutating ticks [{}, {})",
        cfg.workers,
        floor.load(Ordering::SeqCst).max(cfg.lo).min(n),
        cfg.hi.min(n)
    );

    let mut total = 0u64;
    let mut fin = 0u64;
    let mut last_print = Instant::now();
    let mut incumbent = start_outcome;

    for rep in rx {
        total += rep.evals;
        fin += rep.finished;

        let better = {
            let g = best.read().unwrap();
            rep.outcome > g.outcome
        };
        if better {
            // THE GUARD. The claim goes to the sink -- in production, the plain
            // oracle -- before it goes anywhere else, and a refusal rolls the
            // global incumbent back rather than trusting the search's own
            // arithmetic.
            match sink(rep.outcome, &rep.inputs, &rep.prov) {
                Ok(confirmed) => {
                    let mut g = best.write().unwrap();
                    if confirmed > g.outcome {
                        let prev = g.outcome;
                        g.outcome = confirmed;
                        g.inputs = rep.inputs.clone();
                        incumbent = confirmed;
                        drop(g);
                        eprintln!(
                            "*** {} (was {})  {}  {}  evals={}  op={}",
                            confirmed,
                            prev,
                            match (confirmed.finish_ms(), prev.finish_ms()) {
                                (Some(a), Some(p)) => delta(a - p),
                                _ => String::new(),
                            },
                            rep.prov,
                            total,
                            rep.op.as_ref().map(|o| o.to_string()).unwrap_or_default()
                        );
                        if cfg.max_drift > 0
                            && rep.prov.from_fork
                            && rep.prov.distance.diff_ticks > cfg.max_drift
                        {
                            stop.store(true, Ordering::Relaxed);
                            eprintln!(
                                "stopping: this result is {} ticks from the reference the fork \
                                 servers checkpointed on, past --max-drift {}. It is CONFIRMED -- \
                                 the plain oracle says so -- but the fork's ranking is not \
                                 trustworthy this far out. Re-anchor: restart with --start-from \
                                 {}.",
                                rep.prov.distance.diff_ticks,
                                cfg.max_drift,
                                "the file just banked"
                            );
                        }
                    }
                }
                Err(()) => {
                    // The incumbent is untouched: nothing unconfirmed ever
                    // reached it.
                    stop.store(true, Ordering::Relaxed);
                    eprintln!(
                        "stopping: a banked claim did not survive the plain oracle. \
                         The PHANTOM_ file and the incumbent before it are both kept."
                    );
                }
            }
        }

        if last_print.elapsed().as_secs_f64() > 20.0 {
            let el = t0.elapsed().as_secs_f64();
            eprintln!(
                "evals {:>9}  finish {:.0}%  best {}  {:.0} eval/s  {}",
                total,
                100.0 * fin as f64 / total.max(1) as f64,
                incumbent,
                total as f64 / el,
                elapsed(el)
            );
            last_print = Instant::now();
        }
        if t0.elapsed().as_secs_f64() / 60.0 >= cfg.minutes {
            stop.store(true, Ordering::Relaxed);
        }
    }
    for h in handles {
        let _ = h.join();
    }
    eprintln!(
        "DONE best={} evals={} in {}",
        incumbent,
        total,
        elapsed(t0.elapsed().as_secs_f64())
    );
    incumbent
}

/// The production entry point: the sink is [`Bank`], so the only way out of the
/// search is through the plain oracle.
pub fn run<E, F>(
    cfg: &Config,
    patcher: Arc<Patcher>,
    start: Inputs,
    start_outcome: Outcome,
    bank: &mut Bank,
    make: F,
) -> Outcome
where
    E: Evaluator + 'static,
    F: Fn(usize) -> Result<E, String> + Send + Sync + 'static,
{
    let p = Arc::clone(&patcher);
    let out = run_with_sink(
        cfg,
        start,
        start_outcome,
        |o, inputs, prov| match bank.offer(&p, inputs, o, prov) {
            Ok(b) => Ok(b.confirmed),
            Err(_) => Err(()),
        },
        make,
    );
    eprintln!("{}", bank.summary());
    out
}
