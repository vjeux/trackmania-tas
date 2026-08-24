//! The parallel search: one shared archive, one fork server per worker.
//!
//! # Why the archive is shared and the simulators are not
//!
//! An archive search's whole value is that a state found by one worker is
//! available to every other. Islands with private archives throw that away —
//! the frontier is exactly the thing you want pooled. So the archive, the
//! trunk and the statistics live behind one mutex, and each worker owns its
//! own simulator, which is the only object that cannot be shared (a fork
//! server is a process with a pipe and a work directory it locks).
//!
//! The lock is held to pick a bin and to absorb a rollout, and **never while
//! simulating** — the rollout runs unlocked against the worker's own engine
//! and comes back as a list of `(action, trace)` to be folded in. On a 65 ms
//! evaluation that makes the critical section a rounding error.
//!
//! # The one rule this file adds
//!
//! **Whenever the search reaches a station it has never reached before, the
//! tape that did it goes to the plain oracle.** Not a sample, not a
//! milestone — every new furthest station. That is what makes a failed run a
//! measured statement ("station 143 of 220, cps 1, on a file the server read")
//! instead of a number out of a log, and it is cheap because a new furthest
//! station is rare by construction.

use crate::action::{Alphabet, Input, Macro};
use crate::archive::{Archive, BinKey, Entry, Policy};
use crate::branch::{Branch, BranchErr, CarState, GateLadder, Handle, PlainOracle, Route};
use crate::explore::Cfg;
use crate::outcome::{Reached, Verdict};
use crate::rng::Rng;
use crate::trunk::{NodeId, Trunk};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

/// Everything the workers share.
pub struct Shared {
    pub trunk: Trunk,
    pub archive: Archive,
    /// Confirmed by the plain oracle. Nothing else goes in here.
    pub best: Option<(Reached, Vec<Input>)>,
    /// Best the FORK has reported. Kept apart from `best` so the two can never
    /// be quoted as one number.
    pub best_forked: Option<Reached>,
    /// Every plain-oracle answer, in order: (station that triggered it, ticks,
    /// verdict). The audit trail of what was actually measured.
    pub confirmations: Vec<(u32, u32, Verdict)>,
    pub kept: u64,
    pub offers: u64,
}

pub struct Counters {
    pub evals: AtomicU64,
    pub opens: AtomicU64,
    pub oracle_calls: AtomicU64,
    pub errors: AtomicU64,
    pub stop: AtomicBool,
}

impl Default for Counters {
    fn default() -> Self {
        Counters {
            evals: AtomicU64::new(0),
            opens: AtomicU64::new(0),
            oracle_calls: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            stop: AtomicBool::new(false),
        }
    }
}

pub struct Progress0 {
    /// Highest station any worker has reached, atomically, so a worker can see
    /// a new record without taking the lock.
    pub furthest: AtomicU64,
}

/// Run the search until `budget` macro advances or `stop` is set.
///
/// `make_branch(i)` builds worker `i`'s simulator. It is called on the worker's
/// own thread because a fork server must be owned by the thread that talks to
/// its pipe.
#[allow(clippy::too_many_arguments)]
pub fn run<B, O, F>(
    route: &(dyn Route),
    cfg: &Cfg,
    threads: usize,
    budget: u64,
    make_branch: F,
    oracle: &O,
    ladder: &GateLadder,
    shared: &Mutex<Shared>,
    counters: &Counters,
    on_event: &(dyn Fn(&str) + Sync),
) where
    B: Branch,
    O: PlainOracle + Sync,
    F: Fn(usize) -> Result<B, String> + Sync + Send,
{
    let actions = cfg.alphabet.actions();
    let make_branch = &make_branch;
    // THE ROOT IS SEEDED BY THE FIRST WORKER THAT COMES UP, not by worker 0.
    // Worker 0 failed its self-check on a 12-worker run, the archive stayed
    // empty, and five healthy workers then spun on `pick() -> None` for 203 s
    // reporting 0 evals and no errors of their own. A fleet must not have a
    // member whose failure is silently everybody's.
    let root_seeded = AtomicBool::new(false);
    let root_seeded = &root_seeded;
    std::thread::scope(|sc| {
        for wi in 0..threads {
            let actions = actions.clone();
            sc.spawn(move || {
                let mut br = match make_branch(wi) {
                    Ok(b) => b,
                    Err(e) => {
                        on_event(&format!("worker {}: {}", wi, e));
                        counters.errors.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                };
                if !root_seeded.swap(true, Ordering::SeqCst) {
                    // Seed the root exactly once, and do it from a worker so
                    // the initial state comes from the same instrument every
                    // other state comes from.
                    //
                    // When the tree carries a seed tape, the root is that
                    // tape's END state, and its gates and cursor are walked
                    // along the whole trace — a root that claimed zero gates
                    // would put the cap back at the first checkpoint and undo
                    // the run it was seeded from.
                    let seed_tape = {
                        let s = shared.lock().unwrap();
                        let n = s.trunk.end_tick(Trunk::ROOT);
                        s.trunk.inputs_to(Trunk::ROOT, n)
                    };
                    if !seed_tape.is_empty() {
                        match br.open(&[], None).and_then(|h| br.advance(h, 0, &seed_tape)) {
                            Ok(adv) => {
                                let mut s = shared.lock().unwrap();
                                let mut collected = 0u32;
                                let mut cursor = 0u32;
                                absorb(&mut s, route, ladder, &mut collected, &mut cursor, Trunk::ROOT, 0, &adv.trace);
                                on_event(&format!(
                                    "seeded from a banked tape: {} ticks, {} gates collected, station {}",
                                    seed_tape.len(),
                                    collected,
                                    s.archive.max_station
                                ));
                            }
                            Err(e) => on_event(&format!("could not replay the seed tape: {:?}", e)),
                        }
                    } else {
                    match br.initial_state() {
                        Ok(st) => {
                            // THE ROOT GOES THROUGH THE LADDER TOO. It did not,
                            // and on this map the spawn's nearest route vertex
                            // is at s = 1483 m, so the parked car at tick 0 was
                            // archived at station 74 of 97 and became the best
                            // entry in the archive. Every headline number after
                            // that was about a car that had not moved.
                            let (mut pr, cur) = route.progress_from(st.pos, 0);
                            let mut c0 = 0u32;
                            pr.s = ladder.saturate(&mut c0, st.pos, pr.s, pr.on_route);
                            let mut s = shared.lock().unwrap();
                            let key = BinKey::of(&st, &pr, route, &s.archive.bands);
                            s.archive.offer(
                                key,
                                Entry {
                                    node: Trunk::ROOT,
                                    ticks: 0,
                                    state: st,
                                    progress: pr,
                                    live: None,
                                    visits: 0,
                                    seen: 1,
                                    cursor: cur,
                                },
                            );
                        }
                        Err(e) => {
                            root_seeded.store(false, Ordering::SeqCst);
                            on_event(&format!("worker {} could not read the initial state: {:?}", wi, e));
                        }
                    }
                    }
                }
                let mut rng = Rng::new(cfg.seed.wrapping_mul(0x9E3779B9).wrapping_add(wi as u64));
                worker(
                    wi, route, cfg, &actions, budget, &mut br, oracle, ladder, shared, counters,
                    on_event, &mut rng,
                );
            });
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn worker<B: Branch, O: PlainOracle>(
    _wi: usize,
    route: &dyn Route,
    cfg: &Cfg,
    actions: &[Input],
    budget: u64,
    br: &mut B,
    oracle: &O,
    ladder: &GateLadder,
    shared: &Mutex<Shared>,
    counters: &Counters,
    on_event: &dyn Fn(&str),
    rng: &mut Rng,
) {
    let policy: Policy = cfg.policy;
    loop {
        if counters.stop.load(Ordering::Relaxed) || counters.evals.load(Ordering::Relaxed) >= budget
        {
            return;
        }
        // ---- locked: choose where to spend the next rollout ----
        let picked = {
            let mut s = shared.lock().unwrap();
            match s.archive.pick(&policy, rng) {
                None => None,
                Some(k) => s.archive.get(&k).map(|e| (k, e.node, e.ticks, e.live, e.state.cps, e.cursor)),
            }
        };
        let (_key, node0, ticks0, live0, cps0, cur0) = match picked {
            Some(p) => p,
            None => {
                std::thread::yield_now();
                continue;
            }
        };
        if ticks0 + cfg.k as u32 > cfg.tick_limit {
            continue;
        }
        let prefix0 = {
            let s = shared.lock().unwrap();
            s.trunk.inputs_to(node0, ticks0)
        };

        // ---- unlocked: simulate ----
        let mut first_order: Vec<Input> = actions.to_vec();
        rng.shuffle(&mut first_order);
        let n_first = cfg.fanout.unwrap_or(first_order.len()).min(first_order.len());

        for first in first_order.into_iter().take(n_first) {
            if counters.stop.load(Ordering::Relaxed)
                || counters.evals.load(Ordering::Relaxed) >= budget
            {
                return;
            }
            let steps = 1 + rng.below(cfg.max_rollout.max(1) as u64) as u32;
            let mut prefix = prefix0.clone();
            let mut ticks = ticks0;
            let mut prev = first;
            let mut live: Option<Handle> = None;
            let mut chain: Vec<(Input, Vec<CarState>)> = Vec::new();
            let mut ended: Option<Verdict> = None;

            counters.opens.fetch_add(1, Ordering::Relaxed);
            let opened = match br.open(&prefix, live0) {
                Ok(h) => Some(h),
                Err(BranchErr::Stale) => br.open(&prefix, None).ok(),
                Err(e) => {
                    counters.errors.fetch_add(1, Ordering::Relaxed);
                    on_event(&format!("open: {:?}", e));
                    None
                }
            };
            live = opened;
            if live.is_none() {
                continue;
            }

            for j in 0..steps {
                if ticks + cfg.k as u32 > cfg.tick_limit {
                    break;
                }
                let act = if j == 0 {
                    first
                } else if rng.f64() < cfg.sticky {
                    prev
                } else {
                    actions[rng.below(actions.len() as u64) as usize]
                };
                prev = act;
                let h = match live.take() {
                    Some(h) => h,
                    None => match br.open(&prefix, None) {
                        Ok(h) => h,
                        Err(e) => {
                            counters.errors.fetch_add(1, Ordering::Relaxed);
                            on_event(&format!("reopen: {:?}", e));
                            break;
                        }
                    },
                };
                let inputs = vec![act; cfg.k as usize];
                let adv = match br.advance(h, ticks, &inputs) {
                    Ok(a) => a,
                    Err(e) => {
                        counters.errors.fetch_add(1, Ordering::Relaxed);
                        on_event(&format!("advance: {:?}", e));
                        break;
                    }
                };
                counters.evals.fetch_add(1, Ordering::Relaxed);
                let consumed = adv.trace.len() as u32;
                if consumed == 0 {
                    break;
                }
                prefix.extend_from_slice(&inputs[..consumed as usize]);
                ticks += consumed;
                chain.push((act, adv.trace));
                live = adv.handle;
                ended = adv.ended;
                if ended.is_some() {
                    break;
                }
            }
            if let Some(h) = live {
                br.close(h);
            }

            // ---- locked: fold the rollout in ----
            let (new_furthest, tape_for_oracle) = {
                let mut s = shared.lock().unwrap();
                let before = s.archive.max_station;
                let mut node = node0;
                let mut t = ticks0;
                let mut collected = cps0;
                let mut cursor = cur0;
                let mut best_tape: Option<(u32, u32, Vec<Input>)> = None;
                for (act, trace) in &chain {
                    let child = s.trunk.push(node, Macro { input: *act, k: cfg.k });
                    absorb(&mut s, route, ladder, &mut collected, &mut cursor, child, t, trace);
                    t += trace.len() as u32;
                    node = child;
                }
                let after = s.archive.max_station;
                if after > before {
                    // The tape that reached it: this rollout's whole prefix.
                    best_tape = Some((after, t, s.trunk.inputs_to(node, t)));
                }
                if let Some(Verdict::Finish { ms }) = ended {
                    let r = Reached::Finished { ms };
                    if s.best_forked.map(|b| r > b).unwrap_or(true) {
                        s.best_forked = Some(r);
                    }
                    best_tape = Some((s.archive.max_station, t, s.trunk.inputs_to(node, t)));
                }
                (after > before, best_tape)
            };
            let _ = new_furthest;

            // ---- unlocked: the plain oracle has the last word ----
            if let Some((station, t, tape)) = tape_for_oracle {
                counters.oracle_calls.fetch_add(1, Ordering::Relaxed);
                match oracle.confirm(&tape) {
                    Ok(v) => {
                        let mut s = shared.lock().unwrap();
                        s.confirmations.push((station, t, v));
                        let r = match v {
                            Verdict::Finish { ms } => Reached::Finished { ms },
                            Verdict::Dnf { cps } => Reached::Stopped { cps, station, ticks: t },
                        };
                        let better = s.best.as_ref().map(|(b, _)| r > *b).unwrap_or(true);
                        if better {
                            s.best = Some((r, tape));
                            on_event(&format!(
                                "*** PLAIN ORACLE: {}   (station {} of {}, {} evals)",
                                r,
                                station,
                                route.n_stations(),
                                counters.evals.load(Ordering::Relaxed)
                            ));
                        }
                    }
                    Err(e) => {
                        counters.errors.fetch_add(1, Ordering::Relaxed);
                        on_event(&format!("oracle: {}", e));
                    }
                }
            }
        }
    }
}

/// Offer a trace's station crossings and its fixed end to the archive.
///
/// The offer points are the route's station boundaries and the macro's end —
/// both fixed by something other than the candidate. **A window whose end the
/// candidate chooses is a decoy the instrument builds**, and "the candidate's
/// own best tick" is exactly such a window.
fn absorb(
    s: &mut Shared,
    route: &dyn Route,
    ladder: &GateLadder,
    collected: &mut u32,
    cursor: &mut u32,
    child: NodeId,
    from_tick: u32,
    trace: &[CarState],
) {
    let mut last_station: Option<u32> = None;
    for (i, st) in trace.iter().enumerate() {
        let n_ticks = from_tick + i as u32 + 1;
        let (mut pr, cur) = route.progress_from(st.pos, *cursor);
        *cursor = cur;
        // THE CAP, applied before anything reads `s`: a car that has not
        // collected the next required gate cannot be credited past that gate.
        pr.s = ladder.saturate(collected, st.pos, pr.s, pr.on_route);
        let mut st = *st;
        st.cps = *collected;
        let st = &st;
        let station = route.station_of(pr.s);
        let is_end = i + 1 == trace.len();
        let crossed = last_station.map(|l| station != l).unwrap_or(true);
        last_station = Some(station);
        if !crossed && !is_end {
            continue;
        }
        let key = BinKey::of(st, &pr, route, &s.archive.bands);
        s.offers += 1;
        if s.archive.offer(
            key,
            Entry {
                node: child,
                ticks: n_ticks,
                state: *st,
                progress: pr,
                live: None,
                visits: 0,
                seen: 1,
                cursor: cur,
            },
        ) {
            s.kept += 1;
        }
    }
}

impl Shared {
    /// A search that starts from a banked tape of our own rather than from the
    /// grid. See [`crate::trunk::Trunk::with_seed`].
    pub fn seeded(route: &dyn Route, cfg: &Cfg, seed: Vec<Input>) -> Shared {
        let mut s = Shared::new(route, cfg);
        s.trunk = Trunk::with_seed(seed);
        s
    }

    pub fn new(route: &dyn Route, cfg: &Cfg) -> Shared {
        let mut bands = cfg.bands;
        bands.station_m = route.spacing();
        Shared {
            trunk: Trunk::new(),
            archive: Archive::new(bands, route.n_stations()),
            best: None,
            best_forked: None,
            confirmations: Vec::new(),
            kept: 0,
            offers: 0,
        }
    }

    /// The failure report the brief asks for: **furthest station reached on our
    /// own route, and the checkpoint count**, from the plain oracle, so
    /// "stuck at station 143 of 220, always" is a debuggable statement about a
    /// place on the map.
    pub fn report(&self, route: &dyn Route) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "furthest station reached: {} of {} ({:.0} m of {:.0} m)\n",
            self.archive.max_station,
            route.n_stations(),
            self.archive.max_station as f32 * route.spacing(),
            route.length()
        ));
        s.push_str(&format!("archive: {} bins, {} offers, {} kept\n", self.archive.len(), self.offers, self.kept));
        match &self.best {
            Some((r, _)) => s.push_str(&format!("best CONFIRMED by the plain oracle: {}\n", r)),
            None => s.push_str("best CONFIRMED by the plain oracle: nothing yet\n"),
        }
        match self.best_forked {
            Some(r) => s.push_str(&format!("best the FORK reported (a hypothesis, not a result): {}\n", r)),
            None => s.push_str("the fork has reported no finish\n"),
        }
        s.push_str(&format!("plain-oracle answers: {}\n", self.confirmations.len()));
        let mut cps_hist: std::collections::BTreeMap<u32, u32> = Default::default();
        for (_, _, v) in &self.confirmations {
            let c = match v {
                Verdict::Finish { .. } => u32::MAX,
                Verdict::Dnf { cps } => *cps,
            };
            *cps_hist.entry(c).or_insert(0) += 1;
        }
        for (c, n) in cps_hist {
            if c == u32::MAX {
                s.push_str(&format!("  finishes: {}\n", n));
            } else {
                s.push_str(&format!("  cps {}: {} tapes\n", c, n));
            }
        }
        s.push_str("best speed WE have ever reached at a station (m/s) -- self-referential, every 20 stations:\n");
        for (i, v) in self.archive.best_speed_at.iter().enumerate() {
            if i % 20 == 0 && v.is_finite() {
                s.push_str(&format!("  st {:>4}  {:>6.1}\n", i, v));
            }
        }
        s
    }
}

/// Convenience: the alphabet the lead asked to start with.
pub fn three_action() -> Alphabet {
    Alphabet::ThreeGas
}
