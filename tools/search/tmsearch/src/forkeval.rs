//! The fork evaluator: a paused simulator per worker, forked once per
//! candidate, with a per-tick watchdog that stops paying for a candidate the
//! moment it is clearly dead.
//!
//! Six to nine times faster than a full re-simulation, and **a gradient, not a
//! result**. Two facts govern every use of it:
//!
//! * **It is only trustworthy near the reference it checkpointed on.** The
//!   4700/4700 exactness evidence covers tapes that perturb a reference by a
//!   few ticks at 48-99% of the way through a run. Outside that regime it lies:
//!   0 of 312 fork-reported finishes survived a full re-validation of the
//!   byte-identical bitstream, and one tape gave DNF from boundary 170 and
//!   23.622 from boundary 305 -- same inputs, two answers, and the file says
//!   DNF. So every candidate this evaluator scores carries
//!   [`Provenance::distance`], and the guard re-validates anything that is
//!   going to be banked.
//! * **The resume boundary is still measured per worker.** It no longer VARIES
//!   per worker -- the tick hook stops every server at the same tick, and 300
//!   servers started together prove it -- but it is still MEASURED on each one,
//!   because a boundary that is assumed is how the phantom got in. Each worker
//!   probes its own server and publishes `max(calibration, probe)`, where the
//!   probe is the first UNCONSUMED record. The search takes the MAXIMUM over
//!   workers as its mutation floor -- it must be the maximum, because migration
//!   moves a state made by one worker into another.
//!   (When the clock was a count of `lroundf` calls this was a real spread: 104
//!   of 150 workers stopped one tick later than the master's single
//!   calibration.)
//!
//! # Scoring an aborted candidate
//!
//! A candidate the watchdog kills never reaches the validator, so it has no
//! time and no checkpoint count. It is ranked by **progress**: the furthest
//! arclength along the reference line it reached, computed in the child
//! identically whether it was aborted or not. Progress is a maximum over ticks
//! and aborting only removes ticks, so `progress(aborted) <= progress(the same
//! candidate, unarmed)`: arming can only lower a score, and a dead candidate
//! can never displace a live one. Measured 2000/2000, zero violations.

use crate::guard::Provenance;
use forkoracle::inputs::Inputs;
use crate::score::{Outcome, Progress};
use crate::search::Evaluator;
use std::path::{Path, PathBuf};
use forkoracle::forksrv::{ForkServer, Rec};
use forkoracle::ladder::Ladder;
use forkoracle::layout::{segments, tail_recs, REC_LEN, R_CLOCK, R_POS, R_QUAT, R_VEL};

use forkoracle::pred::{outcome, GateRecord, Watch};

/// The clock value the checkpoint should stop at, from the fitted relation
/// `clock = 36141 + 25.483 * race_ms`.
///
/// **That fit is per map.** It was measured on three segment maps of one map;
/// another map fitted `5431 + 26.49 * race_ms`, and using this one there put a
/// requested race 1.200 at race 4.325. The tick a server actually stopped at is
/// always read back with [`ForkServer::probe_tick`] and reported, so a bad
/// estimate costs a checkpoint in the wrong place, never a wrong answer -- but
/// if you are on a new map, measure the fit rather than trusting this.
pub fn clock_for_tick(tick: i64, start_offset_ms: i32) -> u64 {
    // Under the tick hook this is EXACT (the start of the tick that consumes
    // record `tick`); the fit above only describes the legacy lroundf clock.
    forkoracle::clock::ckpt_for_tick(tick, start_offset_ms)
}

/// The exact first tick a resume may rewrite, calibrated against ground truth.
///
/// The engine does not consume the three input axes at the same instant -- at
/// one checkpoint the steer of tick 2313 was still live while the gas and brake
/// of 2313-2315 had already been taken -- so all three are perturbed at every
/// tick in a window around the page-fault probe, and the answer is the last
/// disagreement with the plain oracle, plus one.
///
/// **The boundary may only move LATER than the probe, never earlier.** The
/// probe is authoritative about what the engine has already consumed. A sweep
/// that finds no disagreement used to return `probe - 6`, and single-tick
/// perturbations near a finish often do not move the interpolated millisecond,
/// so the sweep saw nothing and concluded "all safe": 23 of 100 candidates came
/// back silently wrong. With the clamp, 100/100 at four checkpoint fractions
/// and 600 more across three spans and two seeds.
pub fn calibrate_boundary(
    srv: &mut ForkServer,
    server: &Path,
    map: &Path,
    p: &crate::tape::Patcher,
    work: &Path,
    probe: usize,
    n: usize,
) -> Result<usize, String> {
    let lo = probe.saturating_sub(6);
    let hi = (probe + 10).min(n.saturating_sub(1));
    let mut rows = Vec::new();
    for t in lo..=hi {
        for axis in 0..3u8 {
            let mut c = p.template.clone();
            match axis {
                0 => c.steer[t] = (c.steer[t] as i32 + 90).clamp(-127, 127) as i8,
                1 => c.gas[t] = !c.gas[t],
                _ => c.brake[t] = !c.brake[t],
            }
            let path = work.join(format!("cal{}_{:04}.Ghost.Gbx", axis, t));
            std::fs::write(&path, p.file(&c)).map_err(|e| e.to_string())?;
            let steer: Vec<u8> = c.steer.iter().map(|&v| v as u8).collect();
            let gas: Vec<u8> = c.gas.iter().map(|&v| v as u8).collect();
            let brake: Vec<u8> = c.brake.iter().map(|&v| v as u8).collect();
            let out = srv.run(t, &tail_recs(&steer, &gas, &brake, t));
            rows.push((t, path, forkoracle::forksrv::parse_result(&out).0));
        }
    }
    let files: Vec<&Path> = rows.iter().map(|r| r.1.as_path()).collect();
    let truth = ghost::oracle::validate_many(server, &files, ghost::oracle::MapsMode::One(map), "cal")?;
    let mut by_name = std::collections::HashMap::new();
    for r in &truth {
        by_name.insert(r.file.clone(), r.time_ms);
    }
    let mut last_bad = None;
    for (t, path, fork_ms) in &rows {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if *fork_ms != by_name.get(&name).cloned().unwrap_or(None) {
            last_bad = Some(*t);
        }
    }
    for (_, path, _) in &rows {
        let _ = std::fs::remove_file(path);
    }
    Ok(match last_bad {
        Some(t) => (t + 1).max(probe),
        None => probe,
    })
}

pub struct ForkEval {
    srv: ForkServer,
    /// This worker's own safe resume tick.
    from: usize,
    line_len: f32,
    reference: Inputs,
    /// Armed with a state objective? Then a non-finisher is scored on the gate
    /// and never on the metres ladder: one search, one objective.
    gate: bool,
    /// HOW GOOD THE STATE HAS TO BE before finishing counts as having done the
    /// thing. `NEG_INFINITY` -- the default -- means any arrival counts.
    ///
    /// This exists because *entering a box* is not *doing the thing*. On the
    /// map this feature was proven on, the gate sits on 96 m of boost deck that
    /// every run on the leaderboard drives across: the human world record
    /// clips it with a key of 0.06 and then finishes, which without a bar puts
    /// the seed in the top band and makes it unbeatable by any state hunt --
    /// the exact local optimum the mode exists to escape. The version that
    /// worked had a separate hard-coded launch detector for this; here it is a
    /// bar on the key the operator already wrote.
    gate_min_key: f32,
    /// Is an event clause armed? The band rule differs: with a clause, a run
    /// that finishes without firing has not done the thing.
    fire_armed: bool,
    /// Whether `--after-key` was given. See `GateReport::event`: without this,
    /// an empty after-window scores 0 and outranks every real measurement.
    after_key_armed: bool,
    /// The box itself, so an improvement can say when it is pressed against a
    /// face of it.
    gate_box: forkoracle::pred_core::Gate,
    /// Where the SEED's own state sat in that box, when a seed recording was
    /// given. The migration warning needs somewhere to have migrated FROM.
    gate_seed_pos: Option<[f32; 3]>,
    /// The gate record of each candidate in the LAST batch, so the winner's
    /// whole state travels with it into the bank. Indexed exactly as the batch
    /// was.
    last_gate: Vec<Option<GateRecord>>,
    /// THE SUB-TICK TIMING PLANE, and the per-worker correction it cannot be
    /// used without.
    ///
    /// `Some(off_ms)` when `--plane` is armed: the offset, a whole number of
    /// ticks, from this worker's raw crossing label to race time. The child's
    /// tick labelling moves by ONE WHOLE TICK between fork servers and between
    /// workers of the same run -- measured on 191465, where the same tape read
    /// 13 080.95 on one worker and 13 070.95 on another, and 4 of 56 workers of
    /// one run disagreed with the other 52. A hardcoded correction therefore
    /// puts two scales 10 ms apart into one population, every candidate from an
    /// offset worker looks 10 ms better, and it takes over the global best.
    ///
    /// So each worker calibrates against its OWN run of the incumbent, whose
    /// millisecond the plain oracle has already measured, and a worker whose
    /// calibrated value is not within [`PLANE_TOL_MS`] of it removes itself.
    plane_off_ms: Option<f64>,
    start_offset_ms: i32,
    /// DEEP FORK POINTS. Savestate nodes along the lineage this worker is
    /// editing; a candidate forks from the deepest one that agrees with it
    /// instead of from the server's checkpoint. See `forkoracle::ladder`.
    ladder: Ladder,
    /// The tick each candidate of the LAST batch was actually forked at,
    /// indexed as the batch was, for the provenance record.
    last_from: Vec<usize>,
}

/// Grid spacing of the deep fork points, in ticks, and how many a worker keeps.
///
/// Measured on map 2 (`fk ladder check`, 1000 candidates edited anywhere in a
/// 2432-tick tape): at 100 ticks the ladder forks 96 % of candidates from a
/// node and the average candidate re-simulates 1089 fewer ticks; the cost of
/// making a node is ~12 ms once. A node is a paused engine process whose
/// private pages are the ones its own ticks dirtied (a few MB); 32 of them per
/// worker covers a 3200-tick tape at this spacing.
pub const LADDER_SPACING: usize = 100;
pub const LADDER_CAP: usize = 32;

/// Every worker's ladder statistics, folded together at `finish` so the run
/// can print one line about what the deep fork points did.
static LADDER_TOTALS: std::sync::Mutex<forkoracle::ladder::Stats> =
    std::sync::Mutex::new(forkoracle::ladder::Stats::ZERO);

/// One line about the whole run's deep fork points.
pub fn ladder_report() -> String {
    format!("deep fork points: {}", LADDER_TOTALS.lock().map(|g| *g).unwrap_or(forkoracle::ladder::Stats::ZERO))
}

/// How far a worker's own calibrated crossing of the plane may sit from the
/// millisecond the plain oracle gave the incumbent before the worker is
/// refused.
///
/// The crossing must land inside the incumbent's own millisecond bucket, so
/// anything much over 1 ms means the plane is not measuring the finish this
/// validator is reporting -- a differently-oriented car crossing a body-shaped
/// trigger, which is the documented way this surrogate lies (map 227969: a
/// confident 7 990.7 that validated at 8 004).
const PLANE_TOL_MS: f64 = 2.0;

pub struct ForkSetup {
    pub server: PathBuf,
    pub map: PathBuf,
    pub reference_ghost: PathBuf,
    pub key: PathBuf,
    pub shim: PathBuf,
    pub checkpoint_clock: u64,
    /// The boundary the master calibrated against ground truth.
    pub calibrated: usize,
    /// The key a state must reach before finishing counts as having done the
    /// thing; `NEG_INFINITY` for no bar. See `ForkEval::gate_min_key`.
    pub gate_min_key: f32,
    /// Where the seed's own state sat in the gate box, from its recording.
    pub gate_seed_pos: Option<[f32; 3]>,
    pub start_offset_ms: i32,
    /// The incumbent's millisecond, as the PLAIN ORACLE measured it. The
    /// sub-tick plane is calibrated against this and nothing else.
    pub incumbent_ms: Option<i64>,
}

impl ForkEval {
    pub fn start(
        work: &Path,
        s: &ForkSetup,
        watch: &Watch,
        reference: Inputs,
    ) -> Result<ForkEval, String> {
        let mut srv = ForkServer::start(
            work,
            &s.server,
            &s.map,
            &s.reference_ghost,
            &s.key,
            &s.shim,
            s.checkpoint_clock,
        )?;

        // WHERE DID THIS SERVER ACTUALLY STOP? Ask it, do not assume the
        // master's answer. A failed probe is a hard abort: a resume cannot be
        // trusted without it, and a fallback here is how the phantom got in.
        // `boundary_tick` is the first unconsumed record, measured by the
        // page-fault probe and required to agree with the tick the engine says
        // it stopped at -- on every worker, every time.
        let probe = srv.boundary_tick(s.start_offset_ms)?;
        let from = s.calibrated.max(probe);

        let steer: Vec<u8> = reference.steer.iter().map(|&v| v as u8).collect();
        let gas: Vec<u8> = reference.gas.iter().map(|&v| v as u8).collect();
        let brake: Vec<u8> = reference.brake.iter().map(|&v| v as u8).collect();
        let lrecs = tail_recs(&steer, &gas, &brake, from);

        // THE IDENTITY CONTROL, and the search never ran it before: is this
        // server simulating the tape we think it is? The decoded input array in
        // its memory is read back and compared tick for tick with the reference
        // we asked for. Two processes sharing a work directory swap replays, and
        // the result is a real, self-consistent trajectory of a car that drove
        // somewhere else -- nothing internal can see it, because nothing about
        // it is inconsistent. One 70 KB read settles it.
        //
        // It must run on the tape the server was STARTED with, not on an
        // incumbent read from the bank: staggered workers read an
        // already-improved incumbent and then abort on a control testing the
        // wrong tape.
        let refsteer: Vec<u8> = reference.steer.iter().map(|&v| v as u8).collect();
        forkoracle::layout::verify_tape(srv.pid(), srv.base, &refsteer, &gas, &brake)
            .map_err(|e| format!("this server is not simulating the tape we asked for: {}", e))?;

        // THE CAR, DERIVED. The dyna body record the physics step integrates,
        // reached by the pointers the engine itself follows (`forkoracle::car`,
        // `LOCATE.md`): thirty reads of the stopped parent, no fork, no scan,
        // nothing to choose between. Addresses are re-derived in THIS process
        // every time -- the server is PIE and its heap is bimodal -- and a
        // failure is an abort with the broken hop's name, never a guess.
        let car = forkoracle::car::locate(&srv)
            .map_err(|e| format!("the car's state was not located: {}", e))?;
        let layout = car.layout();

        // EXIT AT THE FINISH. A candidate that finishes spends 5.8 ms after its
        // last simulated tick on the validator's finish-and-print path, for a
        // number the engine wrote milliseconds earlier. One calibration fork
        // (of a tape that is about to be simulated thousands of times anyway)
        // finds the word that holds it, and every candidate after that leaves
        // as soon as it is written. See `forkoracle::finish` for why the word
        // is measured per server rather than hardcoded.
        //
        // It needs the incumbent's own millisecond, which the master measured
        // with the PLAIN oracle. Without it, or if the word cannot be found,
        // the server keeps the JSON path -- the same answer, more slowly.
        if let Some(ms) = s.incumbent_ms {
            match forkoracle::finish::calibrate(&mut srv, from, &lrecs, ms) {
                Ok((addr, _)) => eprintln!("fork: exit-at-finish armed on {:#x}", addr),
                Err(e) => eprintln!("fork: exit-at-finish not armed ({})", e),
            }
        }

        let ack = srv.arm(&watch.arm_payload(            layout.clock_bias + s.start_offset_ms as i64,
            R_CLOCK as u32,
            R_QUAT as u32,
            R_POS as u32,
            R_VEL as u32,
            REC_LEN as u32,
            &segments(&layout),
        ));
        if !ack.starts_with("ARMED") {
            return Err(format!("arming the watchdog failed: {}", ack));
        }
        // A SHIM THAT IGNORED THE GATE WOULD SCORE EVERY CANDIDATE "never
        // reached it" -- an answer with no tell. The ack says how many key
        // operations it installed, and a mismatch is an abort: it means the
        // .so on disk is older than the driver that is arming it.
        let want = format!("{} key ops", watch.nkops());
        if !ack.contains(&want) {
            return Err(format!(
                "this shim did not install the state objective ({:?}, wanted {:?}). \
                 The libforkshim.so being loaded is older than this binary; rebuild it and \
                 pass the new one with --shim.",
                ack, want
            ));
        }
        let line_len = watch.refline.s_at_tick(usize::MAX);
        // ---- THE PLANE'S PER-WORKER CALIBRATION ----
        //
        // One extra evaluation, of the UNMODIFIED incumbent, on this worker's
        // own server. Its raw crossing label is compared with the millisecond
        // the plain oracle already measured for that same tape; the difference
        // can only be a whole number of ticks, so it is snapped to one, and a
        // residual bigger than the bucket means this plane is not measuring
        // this validator's finish and the worker refuses rather than joining
        // the population on a different scale.
        let plane_off_ms = if watch.plane_x != 0.0 {
            let want = s
                .incumbent_ms
                .ok_or("--plane needs the incumbent's own oracle millisecond to calibrate against")?
                as f64;
            let recs: Vec<Rec> = tail_recs(&steer, &gas, &brake, from);
            let (j, b) = srv.run_watched(from, &recs);
            let o = outcome(&j, &b);
            let raw_ticks = o.cross().ok_or_else(|| {
                format!(
                    "this worker's own run of the incumbent never crossed the timing plane at \
                     x = {}. The plane is not on this run's path.",
                    watch.plane_x
                )
            })?;
            let raw = raw_ticks * 10.0 + s.start_offset_ms as f64;
            let off = 10.0 * ((want - raw) / 10.0).round();
            let residual = raw + off - want;
            if residual.abs() > PLANE_TOL_MS {
                return Err(format!(
                    "the timing plane does not agree with the validator on this worker: the \
                     incumbent crosses x = {} at {:.3} ms (offset {:+.0}) and the plain oracle \
                     says {}. Residual {:.3} ms, tolerance {:.1}.",
                    watch.plane_x, raw, off, want, residual, PLANE_TOL_MS
                ));
            }
            Some(off)
        } else {
            None
        };
        // The deep fork points live in the worker's own directory, beside the
        // server they fork from. Warm: every node carries the watchdog's state
        // up to its own tick, which is what makes a deep fork's summary the
        // root's summary.
        let ladder = Ladder::new(&work.join("ladder"), probe, from, LADDER_SPACING, LADDER_CAP, true)?;
        Ok(ForkEval {
            srv,
            from,
            line_len,
            reference,
            gate: watch.gate.armed,
            gate_min_key: s.gate_min_key,
            fire_armed: watch.fire.armed,
            after_key_armed: watch.fire.armed && watch.fire.after[0].op != forkoracle::pred_core::KOP_END,
            gate_box: watch.gate,
            gate_seed_pos: s.gate_seed_pos,
            last_gate: Vec::new(),
            plane_off_ms,
            start_offset_ms: s.start_offset_ms,
            ladder,
            last_from: Vec::new(),
        })
    }

    /// A finisher, with the sub-tick crossing attached when a plane is armed.
    ///
    /// A finisher whose crossing the child did not see gets `None` and is
    /// therefore ordered by its millisecond -- which puts it BELOW every
    /// candidate with a measured crossing at the same millisecond. That is the
    /// conservative direction: a candidate nobody measured finely never
    /// displaces one that was.
    fn finish_outcome(&self, ms: i64, o: &forkoracle::pred::Outcome) -> Outcome {
        let us = match (self.plane_off_ms, o.cross()) {
            (Some(off), Some(t)) => {
                Some(((t * 10.0 + self.start_offset_ms as f64 + off) * 1000.0).round() as i64)
            }
            _ => None,
        };
        Outcome::Finish { ms, us }
    }
}

impl Evaluator for ForkEval {
    fn evaluate(&mut self, cands: &[Inputs]) -> Vec<Outcome> {
        let mut out = Vec::with_capacity(cands.len());
        self.last_gate.clear();
        self.last_from.clear();
        // ONE node per batch at most, at the deepest grid tick below the prefix
        // the whole batch shares -- every candidate is the worker's incumbent
        // with edits inside one window, so that prefix is the incumbent up to
        // the earliest edit. Making it costs the prefix the first candidate
        // would have simulated anyway plus a fork and a probe; every candidate
        // after that starts where the edits start.
        if let Some(first) = cands.first() {
            self.ladder.prepare(&mut self.srv, first, Ladder::common_prefix(cands));
        }
        for c in cands {
            let (j, b, at) = self.ladder.run_watched(&mut self.srv, c);
            self.last_from.push(at);
            let o = outcome(&j, &b);
            self.last_gate.push(o.gate());
            out.push(match (o.time, self.gate) {
                (Some(ms), false) => self.finish_outcome(ms, &o),
                // THE STATE OBJECTIVE, three bands straight off the
                // measurement. A record means the car was inside the box, no
                // record means it was not, and there is no third case for a
                // caller to invent. The banding rule itself is
                // `score::gate_outcome`, in one place, testable without a
                // server.
                (t, true) => crate::score::gate_outcome(
                    t,
                    o.gate().map(|g| g.key as f64),
                    o.gate_miss().map(|m| m as f64),
                    self.gate_min_key as f64,
                    o.event(self.fire_armed, self.after_key_armed),
                ),
                (None, false) => {
                    Outcome::Dnf(Progress::Metres { m: o.progress(), of: self.line_len })
                }
            });
        }
        out
    }

    fn floor(&self) -> usize {
        self.from
    }

    fn provenance(&self, idx: usize, inputs: &Inputs) -> Provenance {
        let g = self.last_gate.get(idx).copied().flatten();
        Provenance {
            from_fork: true,
            resume_tick: Some(self.last_from.get(idx).copied().unwrap_or(self.from)),
            distance: inputs.distance_from(&self.reference),
            gate: g,
            gate_edge: match (g, self.gate_seed_pos) {
                (Some(r), Some(seed)) => self.gate_box.migration(seed, r.pos),
                _ => None,
            },
        }
    }

    fn finish(self: Box<Self>) {
        let this = *self;
        if let Ok(mut g) = LADDER_TOTALS.lock() {
            g.add(&this.ladder.stats());
        }
        // The nodes die with the ladder, before the server they were forked
        // from is told to quit.
        drop(this.ladder);
        this.srv.quit();
    }
}
