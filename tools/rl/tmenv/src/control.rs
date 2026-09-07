//! The controls. Nothing in this crate is believed without one of these.
//!
//! # The two questions, and why both are needed
//!
//! **(A) Is the env's reading of the run true?** Answered by the *plain
//! oracle*: write the tape the env drove, hand the file to the dedicated
//! server, and compare the server's own verdict with what the env thought
//! happened. This check shares **no code** with the fork path — no shim, no
//! fork, no memory readout, no `branch` — so it cannot agree with the env by
//! sharing a bug with it. It is the sharp one.
//!
//! **(B) Is the env's TRAJECTORY the trajectory of that tape?** The verdict is
//! coarse: a run that is wrong everywhere but still collects two checkpoints
//! answers (A) correctly. So the second control re-runs the written tape in one
//! piece — a single child, no per-step forking — and compares position tick for
//! tick against the trajectory the stepped env stitched together.
//!
//! (B) shares the readout with the env and is therefore *not* independent of
//! it; it is a check on the **stepping**, which is the part this crate adds.
//! Saying which axis a check certifies is the whole point of running two.
//!
//! # Both need their negative half
//!
//! A comparison that always says "same" passes (B) on a broken rig, and a
//! verdict check on one tape says nothing about discrimination. So each control
//! is run twice: once on the tape the env drove, and once on a tape with one
//! macro deliberately changed. The perturbed pair must **differ**, and the
//! perturbed env run must **agree with its own** re-simulation. Either half
//! alone is decoration.

use crate::forkenv::Rig;
use branch::{Forest, TraceCfg, ROOT};

use forkoracle::car::Car;
use forkoracle::layout::Row;
use std::path::Path;

/// How two trajectories compare.
#[derive(Clone, Debug)]
pub struct TraceCmp {
    /// Ticks compared (the overlap).
    pub n: usize,
    pub a_len: usize,
    pub b_len: usize,
    /// Largest 3-D position difference over the overlap, metres.
    pub max_pos_err: f64,
    /// Median position difference, metres.
    pub med_pos_err: f64,
    /// First tick whose position differs by more than `tol`.
    pub first_diff: Option<usize>,
    /// Ticks whose race clocks did not line up.
    pub clock_mismatch: usize,
    /// How many of the compared ticks exceed the tolerance. A max alone cannot
    /// tell a transient at the resume boundary from a run that diverged: three
    /// ticks over tolerance and four hundred over it are the same max.
    pub over_tol: usize,
    /// The last tick over tolerance, so a transient (early, then silent) is
    /// distinguishable from a divergence (starts and never stops).
    pub last_diff: Option<usize>,
}

impl TraceCmp {
    /// Coverage: the fraction of the first trace's ticks the second one has at
    /// all. Reported and required separately from exactness, because they are
    /// different failures: a reconstruction that stops early is SHORT, and a
    /// reconstruction that disagrees is WRONG. Folding the first into the
    /// second reads a shorter tape as a disagreement -- measured, twice.
    pub fn coverage(&self) -> f64 {
        if self.a_len == 0 {
            0.0
        } else {
            self.n as f64 / self.a_len as f64
        }
    }

    /// Exact everywhere the two overlap, over a substantial overlap.
    pub fn same(&self, tol: f64) -> bool {
        self.n > 0 && self.max_pos_err <= tol && self.coverage() >= 0.9
    }
}

impl std::fmt::Display for TraceCmp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ticks compared (a={} b={}), max |dp| {:.6} m, median {:.6} m, \
             {} over tol (first {}, last {}), coverage {:.1}%, unmatched {}",
            self.n,
            self.a_len,
            self.b_len,
            self.max_pos_err,
            self.med_pos_err,
            self.over_tol,
            match self.first_diff { Some(t) => t.to_string(), None => "none".into() },
            match self.last_diff { Some(t) => t.to_string(), None => "none".into() },
            self.coverage() * 100.0,
            self.clock_mismatch
        )
    }
}

/// Compare two per-tick trajectories by race clock.
///
/// Matching on the **clock** rather than on the array index is deliberate: two
/// traces sampled by different paths can start at different ticks, and an
/// index-wise comparison of two offset traces reads as a large error
/// everywhere, which looks like a real disagreement and is not one.
pub fn compare(a: &[Row], b: &[Row], tol: f64) -> TraceCmp {
    use std::collections::HashMap;
    let bi: HashMap<i64, &Row> = b.iter().map(|r| (r.time_ms, r)).collect();
    let mut errs: Vec<f64> = Vec::new();
    let mut first = None;
    let mut last = None;
    let mut over = 0usize;
    let mut miss = 0usize;
    for r in a {
        match bi.get(&r.time_ms) {
            None => miss += 1,
            Some(q) => {
                let d = ((r.x - q.x).powi(2) + (r.y - q.y).powi(2) + (r.z - q.z).powi(2)).sqrt();
                if d > tol {
                    over += 1;
                    last = Some((r.time_ms / 10) as usize);
                    if first.is_none() {
                        first = Some((r.time_ms / 10) as usize);
                    }
                }
                errs.push(d);
            }
        }
    }
    errs.sort_by(|x, y| x.partial_cmp(y).unwrap());
    TraceCmp {
        n: errs.len(),
        a_len: a.len(),
        b_len: b.len(),
        max_pos_err: errs.last().copied().unwrap_or(f64::NAN),
        med_pos_err: errs.get(errs.len() / 2).copied().unwrap_or(f64::NAN),
        first_diff: first,
        clock_mismatch: miss,
        over_tol: over,
        last_diff: last,
    }
}

/// Re-simulate a written tape in ONE piece and read its trajectory out.
///
/// A fresh server, checkpointed as early as the engine can be stopped, and a
/// single child running the whole tape. No per-step forking — that is the thing
/// being checked.
///
/// The root walks the same ladder the environment does, because a
/// reconstruction that starts LATER than the run it is checking cannot cover
/// the run's first ticks, and those show up as "clock mismatches" — which reads
/// like a disagreement and is really just a shorter tape. Measured: 28 of 286
/// env ticks unmatched, purely because the reconstruction began 28 ticks late,
/// while every one of the 258 ticks they shared agreed to 0.000000 m.




/// Where the engine ACTUALLY puts the car at the start of the race.
///
/// # Why this is measured rather than read off the map
///
/// The map's `Spawn` waypoint is what the map file says. On Summer 2026 - 01 it
/// is not where the dedicated server puts the validated car: the car begins at
/// (1360.00, 10.00, 1108.75), 389 m away, at rest, facing -z. That is not a
/// readout fault — a sweep of all 2308 mapped memory windows finds no other
/// moving car, the trajectory passes its own self-check, and the plain oracle
/// corroborates it from a completely separate path.
///
/// A route solved from the wrong origin is silently wrong: self-consistent
/// geometry, nonsense reward. So the origin is measured, per map, from the
/// engine, every time.
///
/// The checkpoint has to be early — the car must not have gone far — and it
/// must be late enough that the car has moved, because the locator finds the
/// car by velocity consistency and a parked car is indistinguishable from any
/// other constant region of memory.




/// The measured start of the race.
#[derive(Clone, Copy, Debug)]
pub struct SpawnFix {
    pub pos: [f32; 3],
    /// The race clock of the earliest sample. The car has been rolling for this
    /// long, so the true spawn is slightly behind `pos` — reported rather than
    /// corrected, because a correction would be an estimate presented as a
    /// measurement.
    pub race_ms: i64,
    pub speed: f64,
    pub probe_tick: usize,
}
/// Resolve the controlled car from the validator's own ownership chain.
///
/// # The ONLY way this crate learns where the car is
///
/// Every previous path is deleted: `locate_v2`, the candidate scan and its
/// ranking, `FK_STATE_OFF`, the base-offset rebase, and the cached offsets. They
/// are gone rather than deprecated, because a scanner that is merely
/// discouraged comes back.
///
/// Why they had to go, in one measurement of my own: with the old locator the
/// env's root was **non-deterministic** — three identical repeats gave one
/// outright refusal (every ladder rung, three tries each, landing 1.5–6.8 m
/// downroad) and two different roots. `locate_pos2` returns the single
/// lowest-`verr` candidate, and `fk::locate::locate_candidates`' own comment
/// names a map where the winner is *"some other entity whose position, velocity
/// and quaternion are perfectly self-consistent, moving at 3.8 m/s while the car
/// does 40"*. Self-consistency cannot tell them apart; it was never meant to.
///
/// [`forkoracle::car::locate`] (LOCATE.md) instead DERIVES the car: `validator
/// controller → simulation → playground → participant → the driven
/// CGameVehiclePhy → its dyna body record`, every hop an exact pointer read the
/// engine itself takes, cross-checked (the phy is one the vehicle manager
/// iterates; the body record equals the copy-out bit for bit). Thirty reads of
/// the stopped parent, ~130 µs, no scan, no fallback, no fitted bias.
pub fn resolve_car(
    srv: &mut forkoracle::forksrv::ForkServer,
    _probe: usize,
    _recs: &[forkoracle::forksrv::Rec],
    _off: i32,
    verbose: bool,
) -> Result<Car, String> {
    let car = forkoracle::car::locate(srv)?;
    if verbose {
        println!("car: {car}");
    }
    Ok(car)
}

/// THE LABEL SHIFT between the derivation's clock convention and the game's.
///
/// `Car::layout()` labels a row `[sim+0x48] − race_start`: the simulation time
/// the tick loop stamped on the FINISHED tick whose state memory holds. The
/// game stamps the same state -- in its telemetry samples, its tape record
/// times and its finish times -- and the env's rows are labelled the GAME's
/// way, so that DATA's telemetry-derived labels, LEARN's tape indexing and the
/// env agree on the tick with no lag constant anywhere. The shift between the
/// two is MEASURED by `tmenv threeway` (env row T vs the ghost's own telemetry
/// sample T vs fk regen's dump-truth row T, at Δt = 0 and ±10 ms) and written
/// here; the control is the guard, not this constant.
pub const GAME_LABEL_SHIFT_MS: i64 = 0;

/// The layout the env gathers with: the derived car's, plus the engine's
/// checkpoint counter and the driven vehicle's vis state, labelled the game's
/// way (`GAME_LABEL_SHIFT_MS`).
pub fn env_layout(car: &Car) -> forkoracle::layout::Layout {
    let mut l = car.layout_with_engine();
    l.clock_bias -= GAME_LABEL_SHIFT_MS;
    l
}

/// Re-simulate a written tape in ONE piece and read its trajectory out.
///
/// A fresh server, stopped as early as the engine can be stopped, one child
/// running the whole tape, and the car resolved from validator ownership. No
/// per-step forking — that is what CONTROL B checks — and no scan.
pub fn flat_trace(
    server: &Path,
    map: &Path,
    shim: &Path,
    work: &Path,
    candidate: &Path,
    ticks: u64,
) -> Result<Vec<Row>, String> {
    let rig = Rig::new(server, map, shim, work, candidate)?;
    let mut s = rig.session_root()?;
    let probe = s.probe_tick()?;
    let reference = s.tape.tail_records(0);
    let car = resolve_car(&mut s.srv, probe, &reference, s.tape.start_offset_ms, false)?;
    let dir = work.join("flat-traces");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let cfg = TraceCfg { layout: env_layout(&car), dir, stride: 1, max: 400_000 };
    let fk::session::Session { srv, .. } = s;
    let mut f = Forest::new(srv, work, reference, Some(cfg))?;
    f.probe_root()?;
    let (rows, h) = f.advance(ROOT, &[], 0, ticks)?;
    f.release(h);
    forkoracle::layout::check_rows(&rows).map_err(|e| format!("the reconstruction failed its own checks: {e}"))?;
    Ok(rows)
}

/// The root checkpoint: the start of the tick at race −10 ms, before its input
/// record is read -- the first tick the engine reads by INDEX (everything
/// earlier reads record 0; `forkoracle::clock::record_read_at`). Under the tick
/// hook this is the same simulation point in every process: the car at rest on
/// the line with the whole tape still unread.
pub const EARLIEST_CLOCK: u64 = forkoracle::clock::RACE_CLOCK_BIAS as u64 - 1;

/// Where the validator actually puts the car at the start of the race.
///
/// Resolved, not searched. Returns the first sampled row and the whole opening
/// trajectory so a caller can bank it.
pub fn measure_spawn(
    server: &Path,
    map: &Path,
    shim: &Path,
    work: &Path,
    reference: &Path,
) -> Result<(SpawnFix, Vec<Row>), String> {
    let rig = Rig::new(server, map, shim, work, reference)?;
    let mut s = rig.session_root()?;
    let probe = s.probe_tick()?;
    let recs = s.tape.tail_records(0);
    let car = resolve_car(&mut s.srv, probe, &recs, s.tape.start_offset_ms, false)?;
    let dir = work.join("spawn-traces");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let cfg = TraceCfg { layout: env_layout(&car), dir, stride: 1, max: 4_000 };
    let fk::session::Session { srv, .. } = s;
    let mut f = Forest::new(srv, work, recs, Some(cfg))?;
    f.probe_root()?;
    let (rows, h) = f.advance(ROOT, &[], 0, 200)?;
    f.release(h);
    let first = *rows.first().ok_or("no state rows at the start: the spawn is UNMEASURED")?;
    if let Err(e) = forkoracle::layout::check_rows(&rows) {
        // A map that SWITCHES CAR AT THE START LINE (Fall 2024 - 14: a
        // GateGameplayDesert4m on the start block; 8 of 407 pool maps): at the
        // root the participant's live slot is still the Stadium car (+0x10 = 0)
        // and by race 0.5 s it reads -1 while the Desert slot reads 1 -- so a
        // flat 200-tick trace of the ROOT car is a car that stops moving at the
        // switch. Its position at the root is still the spawn, which is what
        // this measures; the env follows the live slot per step from there.
        let never_moves = e.contains("never moves");
        if never_moves && !crate::track::car_switch_blocks(map).is_empty() {
            eprintln!("tmenv: measure_spawn: the root car stops moving on a car-switch map ({e}); taking the root position as the spawn");
        } else {
            return Err(format!("the resolved readout failed its own checks ({e}); UNMEASURED"));
        }
    }
    let speed = (first.vx * first.vx + first.vy * first.vy + first.vz * first.vz).sqrt();
    Ok((
        SpawnFix {
            pos: [first.x as f32, first.y as f32, first.z as f32],
            race_ms: first.time_ms,
            speed,
            probe_tick: probe,
        },
        rows,
    ))
}

/// [`flat_trace`] that reports where the ENGINE ended the run instead of
/// failing there: one fork from the root, `ticks` ticks or the run's end,
/// whichever comes first. For asking how far past its tape a run goes.
pub fn flat_trace_or_end(
    server: &Path,
    map: &Path,
    shim: &Path,
    work: &Path,
    candidate: &Path,
    ticks: u64,
) -> Result<(Vec<Row>, bool), String> {
    let rig = Rig::new(server, map, shim, work, candidate)?;
    let mut s = rig.session_root()?;
    let probe = s.probe_tick()?;
    let reference = s.tape.tail_records(0);
    let car = resolve_car(&mut s.srv, probe, &reference, s.tape.start_offset_ms, false)?;
    let dir = work.join("flat-traces");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let cfg = TraceCfg { layout: env_layout(&car), dir, stride: 1, max: 400_000 };
    let fk::session::Session { srv, .. } = s;
    let mut f = Forest::new(srv, work, reference, Some(cfg))?;
    f.probe_root()?;
    match f.advance_or_end(ROOT, &[], 0, ticks)? {
        branch::Advanced::Node(rows, h) => {
            f.release(h);
            Ok((rows, false))
        }
        branch::Advanced::RunEnded(rows) => Ok((rows, true)),
    }
}

/// One row of the wheels control: race clock + the raw vis state.
pub struct VisTick {
    pub clock: u32,
    pub vis: Vec<u8>,
}

/// Gather the LIVE vis state (`Layout.vis`) per tick for a whole run of the
/// candidate, from the root, in one fork. For `tmenv wheels-control`.
pub fn gather_vis(
    server: &Path,
    map: &Path,
    shim: &Path,
    work: &Path,
    candidate: &Path,
    ticks: u32,
) -> Result<(Vec<VisTick>, i64, u8), String> {
    let rig = Rig::new(server, map, shim, work, candidate)?;
    let mut s = rig.session_root()?;
    let probe = s.probe_tick()?;
    let reference = s.tape.tail_records(0);
    let car = resolve_car(&mut s.srv, probe, &reference, s.tape.start_offset_ms, false)?;
    let l = env_layout(&car);
    if l.vis == 0 {
        return Err("the validator car resolved without a vis state".into());
    }
    let segs = vec![(l.clock, 4u32), (l.vis, forkoracle::layout::VIS_LEN as u32)];
    let recs = s.tape.tail_records(probe);
    let rows = fk::locate::gather_ticks(&mut s.srv, probe, &recs, &segs, ticks, 400_000, (0, 4));
    let out = rows
        .into_iter()
        .map(|t| VisTick { clock: t.clock, vis: t.rec[4..4 + forkoracle::layout::VIS_LEN].to_vec() })
        .collect();
    Ok((out, l.clock_bias, l.car))
}

/// The root stop for a tape: the start of the FIRST tick the engine reads by
/// index. With a countdown prefix (`start_offset_ms <= -10`) that is race
/// -10 ms ([`EARLIEST_CLOCK`]); a tape whose record 0 IS race 0 has no record
/// for race -10 (index -1), so its root is the start of race 0. Measured: a
/// rank-10000 Summer 2026 - 02 ghost with start_offset 0 failed the tick-hook
/// / probe control at 999 ("about to read tape tick -1").
pub fn root_clock_for(start_offset_ms: i32) -> u64 {
    if start_offset_ms <= -10 {
        EARLIEST_CLOCK
    } else {
        forkoracle::clock::ckpt_for_race_ms(start_offset_ms as i64)
    }
}
