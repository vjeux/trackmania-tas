//! The search loop.
//!
//! ```text
//!   pick a bin  ->  open its prefix  ->  hold each action for k ticks
//!        ^                                        |
//!        |            offer every station crossing back to the archive
//!        +----------------------------------------+
//! ```
//!
//! That is the whole algorithm. What makes it work is not the loop; it is what
//! a bin is (`archive.rs`) and what an action is (`action.rs`).
//!
//! # Two rules that are enforced here rather than trusted
//!
//! **A window whose end the CANDIDATE chooses is a decoy the instrument
//! builds.** A state is offered to the archive at a *station crossing* — a
//! place fixed by the route — and at the fixed end of the macro. Never at "the
//! candidate's best tick", which is a window the candidate picks and can win
//! by moving.
//!
//! **A fork answer is never a result.** `Advance::ended` reporting a finish
//! makes a *candidate*, which is written out and handed to the plain oracle.
//! If the oracle disagrees, the candidate is recorded as a phantom and the
//! search continues; nothing enters the bank on a fork's word. 0 of 312
//! fork-reported finishes once survived full re-validation, so this is not a
//! precaution, it is the measured base rate of a lie.

use crate::action::{Alphabet, Input, Macro};
use crate::archive::{Archive, Bands, BinKey, Entry, Policy};
use crate::branch::{Branch, BranchErr, Handle, PlainOracle, Route};
use crate::outcome::{Reached, Verdict};
use crate::rng::Rng;
use crate::trunk::{NodeId, Trunk};

#[derive(Clone, Debug)]
pub struct Cfg {
    pub alphabet: Alphabet,
    /// Ticks per macro.
    pub k: u16,
    /// How many actions to try from a chosen bin per step. `None` = all of
    /// them.
    pub fanout: Option<usize>,
    /// Maximum macros in one expansion rollout. The actual length is drawn
    /// uniformly from `1..=max_rollout`, so the search makes both coarse jumps
    /// and single-macro refinements. See the note on [`Explorer::step`] for
    /// the measurement that put this here.
    pub max_rollout: u32,
    /// Probability that a rollout repeats the previous action instead of
    /// drawing a new one. See the note in [`Explorer::step`].
    pub sticky: f64,
    pub policy: Policy,
    pub bands: Bands,
    pub seed: u64,
    /// Give up on a run this many ticks in.
    pub tick_limit: u32,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            alphabet: Alphabet::Keyboard,
            k: 10,
            fanout: Some(3),
            max_rollout: 30,
            sticky: 0.7,
            policy: Policy::default(),
            bands: Bands::default(),
            seed: 1,
            tick_limit: 6000,
        }
    }
}

/// A finishing tape, before the plain oracle has had its say.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub tape: Vec<Input>,
    /// What the fork said. A hypothesis.
    pub fork_said: Verdict,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    /// Macro advances executed. The unit of cost.
    pub evals: u64,
    /// States offered to the archive.
    pub offers: u64,
    /// States the archive kept.
    pub kept: u64,
    pub bins: u64,
    /// Fork-reported finishes.
    pub fork_finishes: u64,
    /// Fork-reported finishes the plain oracle confirmed.
    pub confirmed: u64,
    /// Fork-reported finishes the plain oracle refused. **Not swept up:
    /// counted, reported, and the tape kept.**
    pub phantoms: u64,
    pub stale_handles: u64,
    pub below_boundary_refusals: u64,
}

pub struct Explorer<'a> {
    pub trunk: Trunk,
    pub archive: Archive,
    pub cfg: Cfg,
    pub stats: Stats,
    /// The best outcome the PLAIN ORACLE has confirmed. Nothing else is
    /// allowed in here.
    pub best: Option<(Reached, Vec<Input>)>,
    /// The best outcome anything has reported, oracle or fork. Reported
    /// separately, and labelled, so the two can never be quoted as one number.
    pub best_seen: Option<Reached>,
    pub phantom_tapes: Vec<Candidate>,
    route: &'a dyn Route,
    rng: Rng,
    actions: Vec<Input>,
}

impl<'a> Explorer<'a> {
    pub fn new(route: &'a dyn Route, cfg: Cfg) -> Explorer<'a> {
        let mut bands = cfg.bands;
        bands.station_m = route.spacing();
        let archive = Archive::new(bands, route.n_stations());
        let actions = cfg.alphabet.actions();
        let rng = Rng::new(cfg.seed);
        Explorer {
            trunk: Trunk::new(),
            archive,
            cfg,
            stats: Stats::default(),
            best: None,
            best_seen: None,
            phantom_tapes: Vec::new(),
            route,
            rng,
            actions,
        }
    }

    /// Put the car at tick 0 into the archive.
    pub fn seed_root<B: Branch>(&mut self, br: &mut B) -> Result<(), BranchErr> {
        let st = br.initial_state()?;
        let pr = self.route.progress(st.pos);
        let key = BinKey::of(&st, &pr, self.route, &self.archive.bands);
        self.archive.offer(
            key,
            Entry {
                node: Trunk::ROOT,
                ticks: 0,
                state: st,
                progress: pr,
                live: None,
                visits: 0,
                seen: 1,
            },
        );
        Ok(())
    }

    /// **The decoy test, printed before the first candidate.**
    ///
    /// > An objective that can be maximised without achieving the goal is not
    /// > a proxy, it is a decoy.
    ///
    /// The laziest tape the explorer can produce is the neutral action held for
    /// the whole run: hands off the wheel from tick 0. Its outcome is measured
    /// through the same oracle and printed next to everything else. It cannot
    /// catch every decoy — it catches the family where *doing less scores
    /// more*, which is the family a sign error or a badly placed objective
    /// produces — and saying which family it catches is the point.
    pub fn do_nothing_outcome<O: PlainOracle>(&self, oracle: &O) -> Result<Verdict, String> {
        let tape = vec![Input::NEUTRAL; self.cfg.tick_limit as usize];
        oracle.confirm(&tape)
    }

    /// One step: choose a bin and expand it.
    ///
    /// # Why an expansion is a ROLLOUT and not one macro
    ///
    /// The first version expanded a chosen bin by exactly one macro, and it
    /// went nowhere: **200 004 evaluations, 6 bins, furthest station 0.** The
    /// reason is worth writing down because it is a property of any archive
    /// search and it is invisible until you run one.
    ///
    /// A bin is coarse on purpose — 20 m of track, 5 m/s of speed. From a
    /// standing start, ten ticks of full throttle move the car 0.11 m and
    /// reach 2.2 m/s, which is the *same bin*. The archive keeps the earlier
    /// arrival, so the deeper state is rejected; the policy then picks that
    /// same bin again, and the search re-expands tick 10 forever. The archive
    /// was not wrong to reject it — a later arrival in the same bin *is* worse
    /// — the expansion was too short to leave the bin it started in.
    ///
    /// So an expansion is a rollout: the chosen action, then a random walk of
    /// up to `max_rollout` further macros, absorbing every station crossing
    /// along the way. The rollout length is itself random in `1..=max_rollout`
    /// so the search makes both coarse jumps and fine refinements, rather than
    /// only ever moving in 300-tick lumps.
    pub fn step<B: Branch, O: PlainOracle>(&mut self, br: &mut B, oracle: &O) -> StepReport {
        let mut rep = StepReport::default();
        let key = match self.archive.pick(&self.cfg.policy, &mut self.rng) {
            Some(k) => k,
            None => return rep,
        };
        let (node0, ticks0, live0) = match self.archive.get(&key) {
            Some(e) => (e.node, e.ticks, e.live),
            None => return rep,
        };
        if ticks0 + self.cfg.k as u32 > self.cfg.tick_limit {
            rep.hit_tick_limit = true;
            return rep;
        }
        let prefix0 = self.trunk.inputs_to(node0, ticks0);

        let mut order: Vec<Input> = self.actions.clone();
        self.rng.shuffle(&mut order);
        let n = self.cfg.fanout.unwrap_or(order.len()).min(order.len());

        for first in order.into_iter().take(n) {
            let mut node = node0;
            let mut ticks = ticks0;
            let mut prefix = prefix0.clone();
            let steps = 1 + self.rng.below(self.cfg.max_rollout.max(1) as u64) as u32;
            let mut prev = first;
            let mut last_key: Option<BinKey> = None;
            // The archived handle is a HINT: it is offered to the backend
            // once, at the start of the rollout, and verified there. If the
            // backend has no live tree, or the hint is stale, `open` falls
            // back to re-simulating the prefix -- which is the entire
            // difference between D's yes and D's no.
            let mut live: Option<Handle> = match br.open(&prefix0, live0) {
                Ok(h) => Some(h),
                Err(BranchErr::Stale) => {
                    self.stats.stale_handles += 1;
                    br.open(&prefix0, None).ok()
                }
                Err(e) => {
                    rep.errors.push(format!("{:?}", e));
                    None
                }
            };
            if live.is_none() {
                continue;
            }

            for j in 0..steps {
                if ticks + self.cfg.k as u32 > self.cfg.tick_limit {
                    rep.hit_tick_limit = true;
                    break;
                }
                // STICKY exploration. A uniform random walk over the alphabet
                // is not a driver: to hold the throttle for 300 ticks it must
                // draw `gas` thirty times running, which is 2^-30, so the car
                // dithers and never builds speed. Measured, before this line
                // existed: 300 015 evaluations, furthest station 14 of 56, and
                // the FASTEST route found to station 4 (80 m of straight) took
                // 15.170 — a car that simply held full throttle would be there
                // in about 3.1.
                //
                // Repeating the previous action with probability `sticky` is
                // the same idea as the macro itself, applied one level up: a
                // driver holds an input, and the hold length should be drawn,
                // not fixed. Expected hold is k/(1 - sticky) ticks.
                let act = if j == 0 {
                    first
                } else if self.rng.f64() < self.cfg.sticky {
                    prev
                } else {
                    self.actions[self.rng.below(self.actions.len() as u64) as usize]
                };
                prev = act;
                // Only touch `open` when we do NOT already hold a live handle.
                // Calling it every macro costs a hash of the whole prefix and
                // turns a rollout into O(n^2) -- measured as a 22x throughput
                // collapse, 88 000 evals/s down to 3 971.
                let h = match live.take() {
                    Some(h) => h,
                    None => match br.open(&prefix, None) {
                        Ok(h) => h,
                        Err(e) => {
                            rep.errors.push(format!("{:?}", e));
                            break;
                        }
                    },
                };
                let inputs = vec![act; self.cfg.k as usize];
                let adv = match br.advance(h, ticks, &inputs) {
                    Ok(a) => a,
                    Err(BranchErr::BelowBoundary { asked, boundary }) => {
                        self.stats.below_boundary_refusals += 1;
                        rep.errors
                            .push(format!("refused: tick {} <= boundary {}", asked, boundary));
                        br.close(h);
                        break;
                    }
                    Err(e) => {
                        rep.errors.push(format!("{:?}", e));
                        br.close(h);
                        break;
                    }
                };
                self.stats.evals += 1;
                rep.expanded += 1;

                let child = self.trunk.push(node, Macro { input: act, k: self.cfg.k });
                let end_key = self.absorb(child, ticks, &adv.trace, &mut rep);

                let consumed = adv.trace.len() as u32;
                let ended = adv.ended;
                if let Some(Verdict::Finish { ms }) = ended {
                    self.stats.fork_finishes += 1;
                    let tape = self.trunk.inputs_to(child, ticks + consumed);
                    self.judge(tape, ms, oracle, &mut rep);
                }

                // park or release the handle
                if let Some(hh) = adv.handle {
                    if ended.is_none() {
                        live = Some(hh);
                        last_key = end_key;
                    } else {
                        br.close(hh);
                        live = None;
                    }
                } else {
                    live = None;
                }

                node = child;
                ticks += consumed;
                prefix.extend_from_slice(&inputs[..consumed as usize]);

                if ended.is_some() {
                    break;
                }
            }
            // A handle parked at the end of a rollout is offered to the bin
            // that final state defined, so the next expansion of that bin
            // forks instead of re-simulating. It is only ever a hint: entries
            // get replaced, and `open` verifies the handle's tick and prefix
            // before believing it.
            match (live, last_key) {
                (Some(hh), Some(k)) => self.archive.set_live(&k, Some(hh)),
                (Some(hh), None) => br.close(hh),
                _ => {}
            }
        }
        self.stats.bins = self.archive.len() as u64;
        rep
    }

    /// A fork said it finished. The plain oracle decides whether it did.
    fn judge<O: PlainOracle>(
        &mut self,
        tape: Vec<Input>,
        claimed: i64,
        oracle: &O,
        rep: &mut StepReport,
    ) {
        rep.candidates.push(Candidate { tape: tape.clone(), fork_said: Verdict::Finish { ms: claimed } });
        match oracle.confirm(&tape) {
            Ok(Verdict::Finish { ms: real }) => {
                self.stats.confirmed += 1;
                let r = Reached::Finished { ms: real };
                self.note_seen(r);
                if self.best.as_ref().map(|(b, _)| r > *b).unwrap_or(true) {
                    self.best = Some((r, tape));
                }
                rep.confirmed.push(real);
            }
            Ok(Verdict::Dnf { cps }) => {
                self.stats.phantoms += 1;
                self.phantom_tapes
                    .push(Candidate { tape, fork_said: Verdict::Finish { ms: claimed } });
                rep.phantoms.push((claimed, cps));
            }
            Err(e) => rep.errors.push(format!("oracle: {}", e)),
        }
    }

    /// Offer the trace's station crossings and its fixed end to the archive.
    fn absorb(
        &mut self,
        child: NodeId,
        from_tick: u32,
        trace: &[crate::branch::CarState],
        rep: &mut StepReport,
    ) -> Option<BinKey> {
        let mut last_station: Option<u32> = None;
        let mut end_key = None;
        for (i, st) in trace.iter().enumerate() {
            let n_ticks = from_tick + i as u32 + 1;
            let pr = self.route.progress(st.pos);
            let station = self.route.station_of(pr.s);
            let is_end = i + 1 == trace.len();
            let crossed = last_station.map(|l| station != l).unwrap_or(true);
            last_station = Some(station);
            if !crossed && !is_end {
                continue;
            }
            let key = BinKey::of(st, &pr, self.route, &self.archive.bands);
            self.stats.offers += 1;
            let kept = self.archive.offer(
                key,
                Entry {
                    node: child,
                    ticks: n_ticks,
                    state: *st,
                    progress: pr,
                    live: None,
                    visits: 0,
                    seen: 1,
                },
            );
            if kept {
                self.stats.kept += 1;
                rep.kept += 1;
                if is_end {
                    end_key = Some(key);
                }
            }
            self.note_seen(Reached::Stopped { cps: st.cps, station, ticks: n_ticks });
        }
        end_key
    }

    fn note_seen(&mut self, r: Reached) {
        if self.best_seen.map(|b| r > b).unwrap_or(true) {
            self.best_seen = Some(r);
        }
    }

    /// The self-referential diagnostics. **This is what replaces "the human
    /// does 82 m/s here", which this project may not say.** Best speed ever
    /// observed at a station, across all our own runs, and the histogram of
    /// where our states pile up.
    pub fn diagnostics(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "archive {} bins, furthest station {} of {}\n",
            self.archive.len(),
            self.archive.max_station,
            self.route.n_stations()
        ));
        s.push_str("station histogram (states binned per band of stations):\n");
        for (lo, c) in self.archive.station_histogram(20) {
            let bar = "#".repeat(((c as f64).sqrt() as usize).min(60));
            s.push_str(&format!("  st {:>5}  {:>8}  {}\n", lo, c, bar));
        }
        s.push_str("best speed we have ever reached at a station (m/s), every 10 stations:\n");
        for (i, v) in self.archive.best_speed_at.iter().enumerate() {
            if i % 10 == 0 && v.is_finite() {
                s.push_str(&format!("  st {:>5}  {:>7.1}\n", i, v));
            }
        }
        s
    }
}

#[derive(Clone, Debug, Default)]
pub struct StepReport {
    pub expanded: u32,
    pub kept: u32,
    pub candidates: Vec<Candidate>,
    /// Times the plain oracle agreed with.
    pub confirmed: Vec<i64>,
    /// (what the fork claimed, what the oracle's checkpoint count really was)
    pub phantoms: Vec<(i64, u32)>,
    pub errors: Vec<String>,
    pub hit_tick_limit: bool,
}
