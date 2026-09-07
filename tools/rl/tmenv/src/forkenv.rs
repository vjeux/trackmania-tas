//! The environment over a live savestate tree.
//!
//! One `ForkEnv` owns one fork server. `reset` returns to the server's own
//! checkpoint — the start of the race — and `step` advances a macro of
//! `k_ticks`, forking a fresh paused engine each time and reading the car's
//! own state per tick out of it.
//!
//! # What a step costs, and why the shape is what it is
//!
//! Agent D measured a `k = 10` branch at **6.953 ms**, of which **4.220 ms is
//! the boundary probe and is flat in `k`**. So a step is dominated by a fixed
//! cost, and `k = 20` buys ten more ticks for about 1.3 ms rather than for
//! another 6.9 ms. The macro length is therefore a throughput knob as much as a
//! control-resolution one, and `EnvCfg::k_ticks` is the thing to sweep first
//! when the rollout rate is the binding constraint.
//!
//! # The one rule
//!
//! Every node probes its own boundary and `advance` refuses to write at or
//! below it. This env never tries: `step` always asks the forest for the floor
//! rather than tracking a tick count of its own. Where the probe lands past the
//! end of the previous macro, the ticks in between were consumed from the
//! **reference tape**, not from the policy — those are counted and reported as
//! `gap_ticks`, because a policy that does not control 3 % of its own ticks is
//! a fact about the measurement, not a detail.

use crate::action::{Act, ActionSpace};
use tmstate::Action;
use crate::core::{Core, CoreCfg, Done, Info};
use branch::{ClockCal, Forest, Handle, TraceCfg, ROOT};
use fk::session::{Checkpoint, Engine, Session};
use fk::tape::Tape;
use forkoracle::forksrv::{rec_of, Rec};
use forkoracle::layout::Row;
use crate::track::Track;
use std::path::{Path, PathBuf};

/// One macro the policy committed, as it landed on the tape.
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub from: usize,
    pub k: usize,
    pub act: Act,
}

/// What a rollout produced.
pub struct Rollout {
    pub spans: Vec<Span>,
    /// Every row the episode saw, in tick order.
    pub trace: Vec<Row>,
    pub done: Option<Done>,
    pub steps: usize,
    /// Gates collected, the env's own reading. Never a result on its own.
    pub gates_hit: usize,
    /// Ticks consumed from the reference because a probe landed past the end of
    /// the previous macro.
    pub gap_ticks: usize,
    /// Ticks the policy wrote twice because a probe landed short.
    pub overlap_ticks: usize,
    /// The race clock at the tick the finish gate was collected, if it was.
    pub finish_ms: Option<i64>,
}

pub struct ForkEnv {
    pub core: Core,
    forest: Forest,
    cur: Handle,
    reference: Vec<Rec>,
    n_ticks: usize,
    spans: Vec<Span>,
    trace: Vec<Row>,
    root_row: Row,
    last_end: usize,
    gap: usize,
    overlap: usize,
    finish_ms: Option<i64>,
    /// How the race clock was labelled for this server.
    pub clock_cal: ClockCal,
    /// Reset-anywhere: kept states, and the nodes no step may release.
    snaps: std::collections::HashMap<u64, Snap>,
    pinned: std::collections::HashSet<Handle>,
    next_snap: u64,
}

/// A kept state, for `reset_to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StateId(pub u64);

/// The longest chunk one `step_ticks` fork runs.
pub const MAX_CHUNK: usize = 50;

struct Snap {
    node: Handle,
    core: Core,
    spans: Vec<Span>,
    trace: Vec<Row>,
    last_end: usize,
    gap: usize,
    overlap: usize,
    finish_ms: Option<i64>,
}

/// Ticks to run past the root before taking the clock calibration probe. The
/// root sits ~0.03 s before the race starts; 50 ticks lands the calibration
/// node ~0.45 s into the race, well inside the regime where the probe is exact.
pub const CLOCK_WARM_TICKS: u64 = 50;

impl ForkEnv {
    /// Build an env on an already-started session.
    ///
    /// `session` must have passed its own identity control (`Session::start`
    /// does it and there is no way to skip it), so the engine inside is
    /// provably running `tape` and not somebody else's.
    pub fn new(
        session: Session,
        engine: &Engine,
        track: std::sync::Arc<Track>,
        acts: ActionSpace,
        cfg: CoreCfg,
        trace_cfg: Option<TraceCfg>,
    ) -> Result<ForkEnv, String> {
        let Session { mut srv, tape, .. } = session;
        let n_ticks = tape.n();
        if cfg.max_ticks > n_ticks {
            return Err(format!(
                "max_ticks {} exceeds the tape's {} ticks: the engine would stop before the cap \
                 and the truncation is indistinguishable from a crash",
                cfg.max_ticks, n_ticks
            ));
        }
        let reference = tape.tail_records(0);
        let probe = srv.probe_tick()?;
        let _ = probe;
        let mut forest = Forest::new(srv, &engine.work, reference.clone(), trace_cfg)?;
        forest.probe_root()?;
        // Label the clock from INSIDE the race, not from the root's probe --
        // see `Forest::calibrate_clock` for the measurement that forced this.
        let clock_cal = forest.calibrate_clock(CLOCK_WARM_TICKS, tape.start_offset_ms)?;

        // The state at the root, captured once: the root never moves, so every
        // reset starts from the same place and there is no reason to pay a fork
        // per episode for it.
        let (rows, h) = forest.advance(ROOT, &[], 0, 1)?;
        let root_row = rows
            .last()
            .cloned()
            .ok_or("the root produced no state rows: the car was not located, so every \
                    observation this env would return is UNMEASURED rather than wrong")?;
        forest.release(h);

        let core = Core::new(cfg, track, acts);
        Ok(ForkEnv {
            core,
            forest,
            cur: ROOT,
            reference,
            n_ticks,
            spans: Vec::new(),
            trace: Vec::new(),
            root_row,
            last_end: 0,
            gap: 0,
            overlap: 0,
            finish_ms: None,
            clock_cal,
            snaps: std::collections::HashMap::new(),
            pinned: std::collections::HashSet::new(),
            next_snap: 1,
        })
    }

    pub fn obs_dim(&self) -> usize {
        self.core.obs_dim()
    }

    pub fn n_actions(&self) -> usize {
        self.core.acts.n()
    }

    /// Back to the root: the start of the race.
    pub fn reset(&mut self) -> Result<Vec<f32>, String> {
        self.leave_cur();
        self.cur = ROOT;
        self.spans.clear();
        self.trace.clear();
        self.last_end = self.forest.floor(ROOT, None)?;
        self.gap = 0;
        self.overlap = 0;
        self.finish_ms = None;
        let t0 = self.last_end;
        Ok(self.core.reset(self.root_row, t0))
    }

    /// Release the current node unless it is the root or a snapshot.
    fn leave_cur(&mut self) {
        if self.cur != ROOT && !self.pinned.contains(&self.cur) {
            self.forest.release(self.cur);
        }
    }

    /// **Reset-anywhere, half one: keep THIS state.**
    ///
    /// The current node -- a live paused engine -- is pinned so no later step
    /// releases it, and everything the core and the bookkeeping know about the
    /// episode so far is copied beside it. Costs nothing now (the node already
    /// exists) and one paused process for as long as the snapshot is held;
    /// `drop_snapshot` frees it. Snapshotting at the root is allowed and cheap.
    pub fn snapshot(&mut self) -> StateId {
        let id = StateId(self.next_snap);
        self.next_snap += 1;
        if self.cur != ROOT {
            self.pinned.insert(self.cur);
        }
        self.snaps.insert(
            id.0,
            Snap {
                node: self.cur,
                core: self.core.clone(),
                spans: self.spans.clone(),
                trace: self.trace.clone(),
                last_end: self.last_end,
                gap: self.gap,
                overlap: self.overlap,
                finish_ms: self.finish_ms,
            },
        );
        id
    }

    /// **Reset-anywhere, half two: continue from a kept state.**
    ///
    /// The episode resumes from the snapshot's node -- forking it, so the
    /// snapshot itself survives and can be reset to again -- with the core and
    /// the tape bookkeeping exactly as they were. The next `step` writes from
    /// that node's own probed floor, like any other step.
    pub fn reset_to(&mut self, id: &StateId) -> Result<Vec<f32>, String> {
        if !self.snaps.contains_key(&id.0) {
            return Err(format!("no such snapshot: {:?}", id));
        }
        self.leave_cur();
        let s = &self.snaps[&id.0];
        self.cur = s.node;
        self.core = s.core.clone();
        self.spans = s.spans.clone();
        self.trace = s.trace.clone();
        self.last_end = s.last_end;
        self.gap = s.gap;
        self.overlap = s.overlap;
        self.finish_ms = s.finish_ms;
        Ok(self.core.observe())
    }

    /// Forget a snapshot and, unless another snapshot or the episode still
    /// stands on it, kill its node.
    pub fn drop_snapshot(&mut self, id: &StateId) {
        if let Some(s) = self.snaps.remove(&id.0) {
            let still_used = s.node == ROOT
                || s.node == self.cur
                || self.snaps.values().any(|o| o.node == s.node);
            if !still_used {
                self.pinned.remove(&s.node);
                self.forest.release(s.node);
            } else if !self.snaps.values().any(|o| o.node == s.node) {
                // the episode stands on it: unpin, so leaving it releases it
                self.pinned.remove(&s.node);
            }
        }
    }

    pub fn snapshots(&self) -> usize {
        self.snaps.len()
    }

    /// One macro of the discrete action table, held for `cfg.k_ticks` ticks.
    pub fn step(&mut self, action: usize) -> Result<(Vec<f32>, f32, Option<Done>, Info), String> {
        let a = self.core.acts.get(action);
        let act = Action { steer: a.steer as i8, gas: a.gas != 0, brake: a.brake != 0 };
        let k = self.core.cfg.k_ticks;
        self.step_ticks(&vec![act; k])
    }

    /// **The general step: one input per tick, 1..=`MAX_CHUNK` of them, one
    /// fork.** Reward is summed over the chunk, `done` is the first
    /// termination inside it, the observation is the state at its end.
    pub fn step_ticks(&mut self, chunk: &[Action]) -> Result<(Vec<f32>, f32, Option<Done>, Info), String> {
        if self.core.done().is_some() {
            return Err("step on a finished episode: call reset".into());
        }
        if chunk.is_empty() || chunk.len() > MAX_CHUNK {
            return Err(format!("a chunk is 1..={MAX_CHUNK} ticks, not {}", chunk.len()));
        }
        let k = chunk.len();
        let from = self.forest.floor(self.cur, None)?;
        // One record more than the chunk, and one tick more of running: the
        // child pauses on an `lroundf` count, i.e. somewhere INSIDE a tick, so
        // the last row it traces is that tick's state part-way through its
        // update. Measured: the final row of an episode 0.538 m from the flat
        // re-simulation of the same tape while every earlier row agreed to
        // 0.000000 m (the earlier partials had been superseded by the next
        // child's completed row). So the last input is held one tick longer,
        // the child runs ~k+1, and only ticks the clock has moved PAST are
        // ingested -- a tick is complete once a row with a later clock exists.
        // The trailing partial is re-traced, completed, by the next step.
        let held = k + 1;
        if from + held > self.n_ticks {
            // Out of tape. Not a crash and not a finish: say which.
            let obs = self.core.observe();
            return Ok((obs, 0.0, Some(Done::TickCap), Info { tick: from, ..Default::default() }));
        }
        if from > self.last_end {
            self.gap += from - self.last_end;
        } else {
            self.overlap += self.last_end - from;
        }

        let mut recs: Vec<Rec> = chunk.iter().map(|a| rec_of(a.steer as u8, a.gas as u8, a.brake as u8)).collect();
        recs.push(*recs.last().unwrap());
        // The child's run budget is k ticks (plus a fraction, so a k = 1 step
        // reliably crosses one tick boundary): the extra RECORD is written so
        // the trailing partial tick runs under our input, but the budget must
        // not grow with it or every step advances k+1 ticks (measured 1.92
        // ticks per k = 1 step before this). `lroundf` counts are ~263 per tick
        // against the crate's 255 estimate, so ticks/step reads ~0.97 k; the
        // tick hook (agentcloud/tickhook) makes this exact later.
        let (all_rows, h) = self.forest.advance_budget(self.cur, &recs, from, k as u64 * branch::LROUNDF_PER_TICK + 32)?;
        self.leave_cur();
        self.cur = h;
        for (i, a) in chunk.iter().enumerate() {
            let k1 = if i + 1 == k { 2 } else { 1 };
            self.spans.push(Span { from: from + i, k: k1, act: Act { steer: a.steer as u8, gas: a.gas as u8, brake: a.brake as u8 } });
        }
        self.last_end = from + held;

        let last_clock = all_rows.last().map(|r| r.time_ms);
        let rows: Vec<Row> = match last_clock {
            Some(lc) if all_rows.len() > 1 => all_rows.iter().filter(|r| r.time_ms < lc).cloned().collect(),
            _ => all_rows.clone(),
        };

        let before = self.core.gates_hit();
        self.trace.extend(rows.iter().cloned());
        let out = self.core.ingest(chunk, &rows);
        if self.finish_ms.is_none()
            && before < self.core.track.n_gates()
            && self.core.gates_hit() >= self.core.track.n_gates()
        {
            self.finish_ms = rows.last().map(|r| r.time_ms);
        }
        Ok(out)
    }

    /// The episode as it happened, with the trace DEDUPED BY TICK, keeping the
    /// LAST row for each.
    ///
    /// Not cosmetic. When a node's probe lands SHORT of the previous macro's
    /// end, the new macro is written over ticks the previous child had already
    /// simulated and traced -- and the new child RE-SIMULATES them with the new
    /// inputs. The superseded rows are a real record of a run that the final
    /// tape does not contain, so keeping them makes the stitched trajectory
    /// disagree with a flat simulation of that tape at exactly those ticks.
    ///
    /// Measured before this fix: 20 of 445 ticks over tolerance, max 0.805 m,
    /// median 0.000000 -- against an instrument whose own noise floor is
    /// exactly 0.000000 m over 824 ticks, so the disagreement was real and it
    /// was ours.
    pub fn rollout_record(&self) -> Rollout {
        let mut seen: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
        for (i, r) in self.trace.iter().enumerate() {
            seen.insert(r.time_ms, i);
        }
        let mut trace: Vec<Row> = self
            .trace
            .iter()
            .enumerate()
            .filter(|(i, r)| seen.get(&r.time_ms) == Some(i))
            .map(|(_, r)| *r)
            .collect();
        trace.sort_by_key(|r| r.time_ms);
        Rollout {
            spans: self.spans.clone(),
            trace,
            done: self.core.done(),
            steps: self.spans.len(),
            gates_hit: self.core.gates_hit(),
            gap_ticks: self.gap,
            overlap_ticks: self.overlap,
            finish_ms: self.finish_ms,
        }
    }

    /// The tape the engine actually ran: the reference with every span applied
    /// in order.
    ///
    /// **This is the tape the control re-simulates.** Applying the spans in
    /// order is not a convenience — it is what the engine's own memory did,
    /// since a later macro whose probe landed short overwrites records an
    /// earlier one wrote and had not yet been consumed.
    pub fn faithful_tape(&self, tape: &Tape) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let (mut s, mut g, mut b) =
            (tape.steer.clone(), tape.accel.clone(), tape.brake.clone());
        for sp in &self.spans {
            for t in sp.from..(sp.from + sp.k).min(s.len()) {
                s[t] = sp.act.steer;
                g[t] = sp.act.gas;
                b[t] = sp.act.brake;
            }
        }
        (s, g, b)
    }

    /// The tape to BANK: the driven prefix, then a stop tail.
    ///
    /// After the episode ends the reference tape is still full throttle, so a
    /// faithful tape keeps driving under somebody else's inputs and the oracle
    /// would answer a question about that drive rather than about the policy's.
    /// The tail is steer 0, no gas, brake — inputs the policy is answerable for.
    pub fn banked_tape(&self, tape: &Tape) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let n = tape.n();
        let (mut s, mut g, mut b) = (vec![0u8; n], vec![0u8; n], vec![0u8; n]);
        // Before the first span the engine consumed the reference; that prefix
        // is part of what the policy inherited and is reproduced verbatim.
        let first = self.spans.first().map(|x| x.from).unwrap_or(0);
        for t in 0..first.min(n) {
            s[t] = tape.steer[t];
            g[t] = tape.accel[t];
            b[t] = tape.brake[t];
        }
        for sp in &self.spans {
            for t in sp.from..(sp.from + sp.k).min(n) {
                s[t] = sp.act.steer;
                g[t] = sp.act.gas;
                b[t] = sp.act.brake;
            }
        }
        for t in self.last_end.min(n)..n {
            s[t] = 0;
            g[t] = 0;
            b[t] = 1;
        }
        (s, g, b)
    }

    pub fn live_nodes(&self) -> usize {
        self.forest.live_nodes()
    }
}

/// Everything needed to stand an env up on a box, in one place.
pub struct Rig {
    pub engine: Engine,
    pub tape: Tape,
    pub reference_path: PathBuf,
}

impl Rig {
    /// `reference` is the varied-steer container the server forks on.
    pub fn new(
        server: &Path,
        map: &Path,
        shim: &Path,
        work: &Path,
        reference: &Path,
    ) -> Result<Rig, String> {
        let engine = Engine {
            server: server.to_path_buf(),
            map: map.to_path_buf(),
            shim: shim.to_path_buf(),
            work: work.to_path_buf(),
            work_is_temporary: false,
        };
        engine.check()?;
        let tape = Tape::load(&reference.to_string_lossy())?;
        tape.codec_is_lossless()?;
        Ok(Rig { engine, tape, reference_path: reference.to_path_buf() })
    }

    /// Start a server checkpointed at the start of the race.
    pub fn session(&self, at_tick: i64) -> Result<Session, String> {
        Session::start(&self.engine, self.tape.clone(), Checkpoint::Tick(at_tick))
    }

    /// Start a server stopped at a raw `lroundf` count.
    ///
    /// The tick form goes through a line fitted on three segment maps and lands
    /// tens of ticks away from where it was aimed; asking for tick 1 on this map
    /// stops at tick 95. When the question is *how early can the engine be
    /// stopped*, the fitted line is the wrong instrument and the raw count is
    /// the right one.
    pub fn session_clock(&self, clock: u64) -> Result<Session, String> {
        Session::start(&self.engine, self.tape.clone(), Checkpoint::Clock(clock))
    }
}

/// How to pick the root, and how hard to look for the car.
#[derive(Clone, Debug)]
pub struct RootCfg {
    /// The raw `lroundf` count to stop the engine at. One value: the resolver
    /// reads a pointer rather than searching for a moving car, so there is
    /// nothing for a ladder to work around.
    pub clock: u64,
    pub verbose: bool,
    /// Require the reset state to BE the start: `(position, tolerance m,
    /// max speed m/s)`.
    ///
    /// Still needed WITH the resolver. Resolving which object the car is and
    /// knowing the container seeded it at the map's start line are different
    /// claims; this is the second one.
    pub require_start: Option<([f32; 3], f32, f64)>,
    /// The latest RACE time (ms) the write floor may sit at: the reference
    /// tape owns at most this much of every episode. A server whose root
    /// probe lands later is restarted, up to `max_root_tries` times.
    pub max_root_floor_ms: i64,
    pub max_root_tries: usize,
}

impl Default for RootCfg {
    fn default() -> Self {
        RootCfg {
            clock: crate::control::EARLIEST_CLOCK,
            verbose: false,
            require_start: None,
            max_root_floor_ms: 120,
            max_root_tries: 6,
        }
    }
}

/// Is this reset state physically coherent, independent of whether it is the
/// answer we want?
///
/// A layout on the wrong address reads out a smooth-looking nothing. Measured,
/// once: position (1085.23, 0.69, 5.33) at race 1155135.336 s and 496.01 m/s.
/// These clauses are about internal consistency — a unit quaternion, a speed a
/// car can reach, a race clock matching the tick the engine says it stopped at
/// — and deliberately NOT about agreeing with the measured start, which is the
/// acceptance control's job and would be circular here.
/// `start_offset_ms` is the countdown: race time of tick `t` is
/// `t * 10 + start_offset_ms`, and a game-recorded container is countdown-
/// prefixed so it is NEGATIVE. The synthetic containers had 0, so an earlier
/// version of this check compared the race clock against `probe * 10` and
/// fired on the first real container -- correctly, on a wrong assumption of
/// mine rather than on a wrong state. Exactly what a control is for.
pub fn reset_is_coherent(r: &Row, probe: usize, start_offset_ms: i32) -> Result<(), String> {
    let q = (r.qx * r.qx + r.qy * r.qy + r.qz * r.qz + r.qw * r.qw).sqrt();
    if !(0.99..=1.01).contains(&q) {
        return Err(format!("|q| = {q:.4}, not a unit quaternion"));
    }
    let v = (r.vx * r.vx + r.vy * r.vy + r.vz * r.vz).sqrt();
    if !v.is_finite() || v > 200.0 {
        return Err(format!("speed {v:.1} m/s"));
    }
    if ![r.x, r.y, r.z].iter().all(|c| c.is_finite() && c.abs() < 1.0e5) {
        return Err(format!("position ({:.1}, {:.1}, {:.1})", r.x, r.y, r.z));
    }
    let expect = probe as i64 * 10 + start_offset_ms as i64;
    if (r.time_ms - expect).abs() > 200 {
        return Err(format!(
            "race clock {} ms but the engine stopped at tick {} with a {} ms countdown (~{} ms)",
            r.time_ms, probe, start_offset_ms, expect
        ));
    }
    Ok(())
}

/// Build an environment rooted as close to the start of the race as the
/// instrument can be made to reach.
///
/// **One implementation, called by both binaries.** The root selection is
/// exactly the kind of fact this project keeps stating twice and then finding
/// the two statements disagree; there is one of it, here.
///
/// Returns the env, its `Rig` (which owns the `Engine` and must outlive it) and
/// the reference `Tape`.
#[allow(clippy::too_many_arguments)]
/// Build an environment rooted at the start of the race.
///
/// **One path, no ladder, no retries, no fallback.** All three existed because
/// the value-based locator could not find a car that had barely moved, so the
/// env walked later and later checkpoints hoping one would resolve. The
/// resolver does not look for a moving car — it reads the validator's own
/// ownership chain — so the constraint is gone and with it the
/// non-determinism: three identical repeats of the old path gave one outright
/// refusal and two different roots.
///
/// `require_start` is still enforced. The resolver says *which object* the car
/// is; it does not say the container seeded that object at the map's start
/// line, and that is a different claim needing its own check.
pub fn build_at_start(
    server: &Path,
    map: &Path,
    shim: &Path,
    work: &Path,
    reference: &Path,
    track: std::sync::Arc<Track>,
    acts: ActionSpace,
    cfg: CoreCfg,
    root: &RootCfg,
) -> Result<(ForkEnv, Rig, Tape), String> {
    let rig = Rig::new(server, map, shim, work, reference)?;
    let tape = rig.tape.clone();
    let dir = work.join("traces");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // The root's probe is not a tick label (see `Forest::calibrate_clock`),
    // but it IS the write floor: records below it may already have been read
    // ahead, so the env never writes them and they stay the reference's. How
    // many that is varies per server (0 to 113 measured at the same instant
    // on a synthesized container), so a server whose floor would give the
    // reference more than `max_root_floor_ms` of the RACE is thrown away and
    // started again -- cheap, once, at startup -- rather than silently handing
    // the first second to a tape the policy does not control.
    //
    // Judged in race time, not tick index: a game-recorded template is
    // countdown-prefixed (`start_offset_ms` < 0), its root probe is
    // legitimately ~156 -- the countdown ticks the engine consumed -- and the
    // reference then owns nothing of the race at all.
    let floor_race_ms = |probe: usize, tape: &Tape| probe as i64 * 10 + tape.start_offset_ms as i64;
    let mut s = rig.session_clock(root.clock)?;
    let mut probe = s.probe_tick()?;
    let mut tries = 1usize;
    while floor_race_ms(probe, &s.tape) > root.max_root_floor_ms && tries < root.max_root_tries {
        if root.verbose {
            eprintln!(
                "  root probe {probe} = race {:.3} > {:.3} (try {tries}): the pre-race read-ahead got \
                 that far on this server; restarting it",
                floor_race_ms(probe, &s.tape) as f64 / 1000.0,
                root.max_root_floor_ms as f64 / 1000.0
            );
        }
        drop(s);
        s = rig.session_clock(root.clock)?;
        probe = s.probe_tick()?;
        tries += 1;
    }
    if floor_race_ms(probe, &s.tape) > root.max_root_floor_ms {
        return Err(format!(
            "the root probe is {probe} (race {:.3}) on {tries} consecutive servers (limit race {:.3}): \
             that much of every tape would belong to the reference. Refusing rather than training on it.",
            floor_race_ms(probe, &s.tape) as f64 / 1000.0,
            root.max_root_floor_ms as f64 / 1000.0
        ));
    }
    let refrecs = s.tape.tail_records(0);
    let car = crate::control::resolve_car(
        &mut s.srv,
        probe,
        &refrecs,
        s.tape.start_offset_ms,
        root.verbose,
    )?;
    let tcfg = TraceCfg { layout: car.layout().clone(), dir, stride: 1, max: 200_000 };
    let mut env = ForkEnv::new(s, &rig.engine, track, acts, cfg, Some(tcfg))?;
    let row = {
        env.reset()?;
        env.core.last_row()
    };
    reset_is_coherent(&row, probe, tape.start_offset_ms)?;

    if let Some((want, tol, vmax)) = root.require_start {
        let pos = [row.x as f32, row.y as f32, row.z as f32];
        let d = crate::geom::norm(crate::geom::sub(pos, want));
        let v = (row.vx * row.vx + row.vy * row.vy + row.vz * row.vz).sqrt();
        if d > tol || v > vmax {
            return Err(format!(
                "the env resets {d:.2} m from the required start at {v:.2} m/s (want <= {tol:.1} m, \
                 <= {vmax:.1} m/s). The car is resolved from validator ownership, so this is not a \
                 locator ambiguity: the CONTAINER seeds the vehicle here."
            ));
        }
    }
    if root.verbose {
        let c = env.clock_cal;
        eprintln!(
            "  env root: lroundf {}, root probe {probe} (write floor; {tries} server start{}), car from \
             validator ownership; clock bias {} from an in-race probe ({} at raw {}; the root's probe \
             implied {}), root row race {:.3}",
            root.clock,
            if tries == 1 { "" } else { "s" },
            c.bias,
            c.probe,
            c.raw_clock,
            c.old_bias,
            row.time_ms as f64 / 1000.0
        );
    }
    Ok((env, rig, tape))
}
