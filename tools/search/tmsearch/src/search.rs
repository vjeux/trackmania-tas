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
use forkoracle::pred::GateRecord;
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
    ///
    /// `idx` is the candidate's position in the batch just evaluated, because
    /// some of what the record carries -- the state a gate measured -- belongs
    /// to that evaluation and not to the tape.
    fn provenance(&self, idx: usize, inputs: &Inputs) -> Provenance;

    /// Release the evaluator's resources (a fork server, in practice).
    fn finish(self: Box<Self>) {}
}

/// THE DECOY TEST, run before the first candidate.
///
/// > An objective that can be maximised without achieving the goal is not a
/// > proxy, it is a decoy.
///
/// One map met three in a row. The cheapest question to ask of any objective is
/// *what is the laziest tape that scores well on it*, and the laziest tape the
/// search can write is the one with every editable tick set to no input at all.
/// So that tape is evaluated first, through the same evaluator, and its score
/// is printed beside the incumbent's before anything else happens.
///
/// **In fork mode this is not a parked car and that is the point.** A fork
/// server has already consumed the seed's prefix, so the do-nothing tape is
/// "the incumbent up to the resume boundary, then hands off the wheel" -- which
/// is exactly the laziest tape inside the search's real action space. In plain
/// mode it is the parked car.
///
/// If it OUTRANKS the incumbent the run stops here rather than spending four
/// hours proving it: an objective the lazy tape wins is not measuring what its
/// author thinks.
pub struct Decoy {
    pub nothing: Outcome,
    pub incumbent: Outcome,
    /// The seed's own measured gate state, for the identity control.
    pub incumbent_gate: Option<GateRecord>,
    /// What the identity control said, when one was armed.
    pub identity: Option<Result<String, String>>,
    /// Ticks that were blanked: the search's own action space.
    pub editable: usize,
}

impl Decoy {
    /// The tape with every tick in `[lo, hi)` set to no steering, no throttle
    /// and no brake.
    pub fn do_nothing(seed: &Inputs, lo: usize, hi: usize) -> Inputs {
        let mut s = seed.clone();
        for t in lo..hi.min(s.len()) {
            s.steer[t] = 0;
            s.gas[t] = false;
            s.brake[t] = false;
        }
        s
    }
    pub fn is_decoy(&self) -> bool {
        self.nothing > self.incumbent
    }
    /// Everything that must hold before a single candidate is worth paying
    /// for: the objective is not a decoy, and the servers are measuring the
    /// tape we think they are.
    pub fn ok(&self) -> bool {
        !self.is_decoy() && !matches!(self.identity, Some(Err(_)))
    }
    pub fn report(&self) -> String {
        format!(
            "decoy test: the do-nothing tape ({} editable ticks blanked) scores {}; \
             the incumbent scores {}{}",
            self.editable,
            self.nothing,
            self.incumbent,
            if self.is_decoy() {
                " -- THE DO-NOTHING TAPE WINS. This objective can be maximised without \
                 driving the map: it is a decoy, not a proxy. Nothing was searched."
            } else {
                ""
            }
        )
    }
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
    /// THE SEED IDENTITY CONTROL, in gate mode. Given the state the fork
    /// measured for the seed at the gate, say whether it is the state the
    /// seed's own recording shows there. `Err` stops the run before the first
    /// candidate.
    ///
    /// A closure rather than a comparison here, because what it compares
    /// against is a ghost file and this module knows nothing about ghosts.
    #[allow(clippy::type_complexity)]
    pub check_seed_gate: Option<Arc<dyn Fn(&GateRecord) -> Result<String, String> + Send + Sync>>,
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
    // Worker 0 runs the decoy test before it enters its loop and before the
    // barrier, so the answer is on screen ahead of the first candidate rather
    // than four hours into the run.
    let (dtx, drx) = mpsc::channel::<Decoy>();
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
        let dtx = dtx.clone();
        let (batch, opc, opset) = (cfg.batch, cfg.ops_per_candidate, cfg.opset);
        let (window, stride, every) = (cfg.window, cfg.stride, cfg.full_window_every);
        let (seed, temp_s, migrate) = (cfg.seed, cfg.temp_s, cfg.migrate);
        let (cfg_lo, cfg_hi) = (cfg.lo, cfg.hi);
        let start_for_decoy = start.clone();
        let check_seed = cfg.check_seed_gate.clone();

        handles.push(std::thread::spawn(move || {
            let mut ev = match make(wi) {
                Ok(e) => Box::new(e),
                Err(e) => {
                    eprintln!("worker {}: ABORT, {}", wi, e);
                    // Still meet BOTH barriers, or the whole fleet hangs
                    // waiting for a worker that will never arrive.
                    ready.wait();
                    ready.wait();
                    return;
                }
            };
            // Publish this worker's own floor, then wait for everyone: the
            // fleet's mutation floor is the MAXIMUM and nobody may act on
            // their own.
            floor.fetch_max(ev.floor(), Ordering::SeqCst);
            ready.wait();
            let flo = floor.load(Ordering::SeqCst).max(cfg_lo).min(n);
            let fhi = cfg_hi.min(n);

            // THE DECOY TEST, on the laziest tape the search can write: every
            // tick the fleet may edit blanked. One evaluation, before any
            // candidate, and it runs BETWEEN the barriers because it has to use
            // the same floor the search does -- a probe that edits below the
            // fleet floor is measuring edits the engine silently drops.
            if wi == 0 {
                let nothing = Decoy::do_nothing(&start_for_decoy, flo, fhi);
                let outs = ev.evaluate(&[nothing, start_for_decoy.clone()]);
                if outs.len() == 2 {
                    // THE INCUMBENT'S OWN BAND, measured rather than assumed.
                    // Published before the second barrier, so no other worker
                    // can start from the placeholder the run was seeded with.
                    {
                        let mut g = best.write().unwrap();
                        if outs[1] > g.outcome {
                            g.outcome = outs[1];
                        }
                    }
                    let gate = ev.provenance(1, &start_for_decoy).gate;
                    // THE SEED IDENTITY CONTROL, on the seed, on the state
                    // this server actually measured for it.
                    let identity = check_seed.as_ref().map(|f| match gate {
                        Some(g) => f(&g),
                        None => Err(
                            "seed identity control: the fork never measured the seed inside \
                             the gate, so there is nothing to check it against. Either the box \
                             is not on the seed's line or this shim is not arming the gate."
                                .to_string(),
                        ),
                    });
                    let d = Decoy {
                        nothing: outs[0],
                        incumbent: outs[1],
                        incumbent_gate: gate,
                        identity,
                        editable: fhi.saturating_sub(flo),
                    };
                    // The verdict is acted on HERE, before the barrier the
                    // other workers are waiting on: a master that decided
                    // afterwards would be racing a fleet that had already
                    // started spending.
                    if !d.ok() {
                        stop.store(true, Ordering::Relaxed);
                    }
                    let _ = dtx.send(d);
                }
            }
            drop(dtx);
            ready.wait();
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

                let prov = ev.provenance(bi, &best_inputs);
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
    drop(dtx);
    let mut incumbent = start_outcome;
    ready.wait();
    ready.wait();
    // THE DECOY TEST'S ANSWER, before anything is searched.
    match drx.recv() {
        Ok(d) => {
            eprintln!("{}", d.report());
            match &d.identity {
                Some(Ok(r)) => eprintln!("{}", r),
                Some(Err(e)) => eprintln!(
                    "{}\nSTOPPING before the first candidate: nothing these servers measure \
                     means anything until that is explained.",
                    e
                ),
                None => {}
            }
            if !d.ok() {
                for h in handles {
                    let _ = h.join();
                }
                return start_outcome;
            }
            // The incumbent's real band, which worker 0 has already published.
            if d.incumbent > incumbent {
                incumbent = d.incumbent;
            }
        }
        Err(_) => eprintln!(
            "decoy test: worker 0 never reported -- it failed to start, so nothing below \
             this line was measured against a lazy tape"
        ),
    }
    eprintln!(
        "all {} workers up; mutating ticks [{}, {})",
        cfg.workers,
        floor.load(Ordering::SeqCst).max(cfg.lo).min(n),
        cfg.hi.min(n)
    );

    let mut total = 0u64;
    let mut fin = 0u64;
    let mut last_print = Instant::now();

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
