//! One fork server on one human ghost, and the two things done with it:
//! walk the reference tape to a savestate, and fan a macro out from it.
//!
//! Everything here is `branch::Forest` over `fk::session::Session`, the way
//! the a7aa56c `tmenv::forkenv` did it (cited: `Rig::new`, `build_at_start`,
//! `control::resolve_car`). The car is resolved from the validator's ownership
//! chain (`fk::validator::ValidatorCar`), never scanned for.

use branch::{Advanced, Forest, Handle, TraceCfg, ROOT};
use fk::session::{Checkpoint, Engine, Session};
use fk::tape::Tape;
use fk::validator::ValidatorCar;
use forkoracle::forksrv::{rec_of, Rec};
use forkoracle::layout::Row;
use std::path::{Path, PathBuf};

/// The earliest `lroundf` count the engine can be stopped at with the
/// validator's ownership chain already built (a7aa56c `control::EARLIEST_CLOCK`). LROUNDF CLOCK ONLY.
pub const EARLIEST_CLOCK: u64 = 11_000;

/// The root for a tape WITHOUT countdown records (`start_offset_ms` near 0).
///
/// MEASURED (Summer 2026 - 01, p00003 19.552, 2026-09-07): a pause inside the
/// pre-race phase of an offset-0 tape gives a DIFFERENT RUN from the plain
/// oracle's. Paused at lroundf 6000/8000/9500/11000/14000 the root had consumed
/// 51/33/95/52/52 records (non-monotone in the clock) with the car still at
/// rest, and the continued run then matched the telemetry only shifted by that
/// many ticks and diverged to 40–125 m by 17 s. The plain oracle on the same
/// file, and on the re-encoded reference, returns the declared 19.552, and
/// hard-left on ticks 0..40 turns it into "wrong simu" -- so in the oracle tick
/// 0 IS race 0. Paused AFTER the race start (30000 → tick 70, 40000 → 108,
/// 90000 → 296) the run matches the telemetry to RMS 0.007 m. A countdown tape
/// (offset −1550: 155 pre-race records) is unaffected at 11000 (44 ghosts).
/// The mechanism is UNKNOWN and is a task; the rule is a description.
pub const OFFSET0_CLOCKS: &[u64] = &[36_000, 60_000, 90_000, 140_000];

/// What one rollout produced.
pub struct Rolled {
    pub rows: Vec<Row>,
    pub exited: bool,
}

pub struct Worker {
    forest: Forest,
    pub tape: Tape,
    /// The root's probed consumed boundary: the first tick the root has NOT
    /// consumed.
    pub root_probe: usize,
    pub root_row: Row,
    pub ghost: PathBuf,
    pub car: ValidatorCar,
    /// Wall time to launch the server, resolve the car and probe.
    pub startup_s: f64,
    /// Race time of a row = its label + this. MEASURED by the identity control
    /// against the ghost's telemetry (`starts::run_on_worker` sets it; +10 under
    /// the lroundf clock, +20 under the tick clock, both a labelling convention
    /// of `forkoracle::layout`, never a shift of the run). Until measured: the
    /// tick hook's own race clock at the root.
    pub label_shift: i64,
    /// The root's race time as the tick hook reports it (sim_ms − race_start), if
    /// in tick mode.
    pub root_race_ms_hook: Option<i64>,
}

/// Race clock of tape tick `t` in ms.
pub fn race_ms(tape: &Tape, t: usize) -> i64 {
    tape.race_ms(t)
}

/// Tape tick whose race clock is `ms` (rounded down to the tick grid).
pub fn tick_of_ms(tape: &Tape, ms: i64) -> usize {
    ((ms - tape.start_offset_ms as i64) / 10).max(0) as usize
}

impl Worker {
    /// Launch a server on `ghost`, stop it at the earliest tick, resolve the
    /// car and open a forest. `work` must be private to this worker.
    ///
    /// Tick clock (`FK_CLOCK=tick`, the default): the root is race −0.010 s
    /// (`ckpt_for_race_ms(-10)`), the first tick the engine reads an input
    /// for, exact in every process. Lroundf clock: the legacy constants below
    /// and, for an offset-0 tape, the state-checked ladder.
    pub fn start(
        server: &Path,
        map: &Path,
        shim: &Path,
        work: &Path,
        ghost: &Path,
        verbose: bool,
    ) -> Result<Worker, String> {
        let tape = Tape::load(&ghost.to_string_lossy())?;
        if forkoracle::clock::tick_mode() {
            // Race −0.010 s first. An offset-0 tape can refuse it: during the
            // countdown the engine READS records it does not apply (the probe
            // saw 33..96 of them read at race −0.010 on p00003), and
            // `ForkServer::start` treats hook/probe disagreement as a hard error
            // (TICKHOOK.md §4). Later roots, inside the race, agree (probe 49
            // at race 0.500). The ladder is race-time based and the identity
            // control downstream still judges the run.
            let mut last = String::new();
            for ms in [-10i64, 300, 600, 1000, 1500] {
                match Self::start_at(server, map, shim, work, ghost, verbose, forkoracle::clock::ckpt_for_race_ms(ms)) {
                    Ok(w) => return Ok(w),
                    Err(e) => {
                        if !e.contains("tick hook / probe disagreement") {
                            return Err(e);
                        }
                        if verbose {
                            eprintln!("  root at race {} refused: {e}; retrying later", crate::secs(ms));
                        }
                        last = e;
                    }
                }
            }
            return Err(format!("no root agreed with the tick hook: {last}"));
        }
        if tape.start_offset_ms <= -1000 {
            return Self::start_at(server, map, shim, work, ghost, verbose, EARLIEST_CLOCK);
        }
        // An offset-0 tape: the pre-race pause is broken (see OFFSET0_CLOCKS), and
        // how many lroundf calls the pre-race dwell takes is not repeatable
        // (36000 landed once at tick 51 pre-race, 30000 twice at tick 70 in the
        // race). So the STATE decides: a root with the car at rest is pre-race
        // and is thrown away for a later clock.
        let mut last_err = String::new();
        for &c in OFFSET0_CLOCKS {
            match Self::start_at(server, map, shim, work, ghost, verbose, c) {
                Ok(mut w) => {
                    if speed(&w.root_row) > 4.0 {
                        // Mid-race root. One more thing can be wrong: the probe can
                        // run AHEAD of the physics (measured once: probe tick 97
                        // with the car in its tick-93 state, labels 40 ms off), so
                        // a short flat run must sit on the telemetry at exactly
                        // the +10 ms label convention every good run has.
                        let tel = crate::tele::Telemetry::load(&ghost.to_string_lossy())?;
                        let rows = w.flat(300)?;
                        let (shift, cmp) = crate::tele::best_shift(&rows, &tel);
                        if shift == 10 && cmp.rms < 0.05 {
                            return Ok(w);
                        }
                        last_err = format!(
                            "clock {} root tick {}: 300-tick flat run sits on the telemetry at shift {:+} ms (RMS {:.4} m), not the +10 ms convention: probe/physics disagree",
                            c, w.root_probe, shift, cmp.rms
                        );
                        if verbose {
                            eprintln!("  offset-0 tape: {last_err}; retrying later");
                        }
                        drop(w);
                        continue;
                    }
                    last_err = format!("clock {} paused pre-race (root tick {}, car at rest)", c, w.root_probe);
                    if verbose {
                        eprintln!("  offset-0 tape: {last_err}; retrying later");
                    }
                    drop(w);
                }
                Err(e) => last_err = e,
            }
        }
        Err(format!("no root inside the race for an offset-0 tape: {last_err}"))
    }

    /// `start` with an explicit root `lroundf` count (diagnostics).
    pub fn start_at(
        server: &Path,
        map: &Path,
        shim: &Path,
        work: &Path,
        ghost: &Path,
        verbose: bool,
        clock: u64,
    ) -> Result<Worker, String> {
        let t0 = std::time::Instant::now();
        let engine = Engine {
            server: server.to_path_buf(),
            map: map.to_path_buf(),
            shim: shim.to_path_buf(),
            work: work.to_path_buf(),
            work_is_temporary: false,
        };
        engine.check()?;
        let _ = std::fs::remove_dir_all(work);
        std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
        let tape = Tape::load(&ghost.to_string_lossy())?;
        tape.codec_is_lossless()?;
        let mut s = Session::start(&engine, tape.clone(), Checkpoint::Clock(clock))?;
        let probe = s.probe_tick()?;
        let recs = s.tape.tail_records(0);
        let wide = (-1.0e6, 1.0e6, -1.0e6, 1.0e6, -1.0e6, 1.0e6);
        let car = ValidatorCar::locate(&mut s.srv, probe, &recs, s.tape.start_offset_ms, wide, 40_000, verbose)?;
        let dir = work.join("traces");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let cfg = TraceCfg { layout: car.layout().clone(), dir, stride: 1, max: 400_000 };
        let hook = if s.srv.tick_mode { Some(s.srv.sim_ms as i64 - s.srv.race_start as i64) } else { None };
        let Session { srv, .. } = s;
        let mut forest = Forest::new(srv, work, recs, Some(cfg))?;
        let root_probe = forest.probe_root()?;
        // The root's own state, captured once (a7aa56c ForkEnv::new).
        let (rows, h) = forest.advance(ROOT, &[], 0, 1)?;
        forest.release(h);
        let root_row = rows.first().cloned().ok_or(
            "the root produced no state rows: the car was not located, so nothing here is measured",
        )?;
        Ok(Worker {
            forest,
            tape,
            root_probe,
            root_row,
            ghost: ghost.to_path_buf(),
            car,
            startup_s: t0.elapsed().as_secs_f64(),
            label_shift: 10,
            root_race_ms_hook: hook,
        })
    }

    pub fn n_ticks(&self) -> usize {
        self.tape.n()
    }

    /// Run the reference tape from the root for `ticks` ticks in ONE child and
    /// return the per-tick rows. The flat trajectory of the human's own run.
    pub fn flat(&mut self, ticks: u64) -> Result<Vec<Row>, String> {
        // `k_ticks` places the stop through ~255 lroundf/tick, which drifts over
        // a long run (a 1956-tick request stopped 90 ticks short, another
        // overshot into the finish); walk in chained chunks that each ask for
        // 90 % of what is left, so the target is approached from below. A
        // target at or past the tape's end is reached by running OFF the end
        // (`Advanced::Exited`), which is how the finish crossing is traced.
        let to_end = self.root_probe + ticks as usize + 5 >= self.tape.n();
        let target = (self.root_probe + ticks as usize).min(self.tape.n());
        let mut all: Vec<Row> = Vec::new();
        let mut h = ROOT;
        let mut left = ticks;
        for _ in 0..64 {
            // tree::Node::branch checks `from` against the floor even for an
            // empty write, so a chained chunk is addressed at the floor.
            let from = if h == ROOT { 0 } else { self.forest.floor(h, None)? };
            let ask = if left <= 60 {
                if to_end { left + 400 } else { left }
            } else {
                ((left as f64 * 0.9) as u64).clamp(1, 1500)
            };
            let (rows, c) = match self.forest.advance_ex(h, &[], from, ask)? {
                Advanced::Paused(mut rows, c) => {
                    drop_stale_tail(&mut rows);
                    if std::env::var("TMREACH_SEAMS").is_ok() {
                        eprintln!("seam: chunk rows {} .. {}", rows.first().map(|r| r.time_ms).unwrap_or(0), rows.last().map(|r| r.time_ms).unwrap_or(0));
                    }
                    (rows, c)
                }
                Advanced::Exited(rows) => {
                    all.extend(rows);
                    h = ROOT;
                    break;
                }
            };
            if h != ROOT {
                self.forest.release(h);
            }
            h = c;
            let got = rows.len();
            all.extend(rows);
            let at = self.forest.floor(h, None)?;
            if got == 0 || (at >= target && !to_end) {
                break;
            }
            left = target.saturating_sub(at).max(1) as u64;
        }
        if h != ROOT {
            self.forest.release(h);
        }
        // Chunks may overlap by a tick at the seams; keep the LAST row per clock
        // (a7aa56c rollout_record's rule).
        let mut seen = std::collections::HashMap::new();
        for (i, r) in all.iter().enumerate() {
            seen.insert(r.time_ms, i);
        }
        let mut out: Vec<Row> = all.iter().enumerate().filter(|(i, r)| seen[&r.time_ms] == *i).map(|(_, r)| r.clone()).collect();
        out.sort_by_key(|r| r.time_ms);
        Ok(out)
    }

    /// A live paused node roughly `ticks` ticks past the root, on the reference
    /// tape, with the rows it produced on the way. Where it actually stopped is
    /// its own probe: `floor(node)`.
    pub fn node_after(&mut self, ticks: u64) -> Result<(Vec<Row>, Handle), String> {
        self.forest.advance(ROOT, &[], 0, ticks.max(1))
    }

    /// The first tick a macro may be written at `h`.
    pub fn floor(&self, h: Handle) -> Result<usize, String> {
        self.forest.floor(h, None)
    }

    /// Fork `h`, write `recs` from `from`, run `k` ticks, read the rows, and
    /// destroy the child. One rollout. `exited` = the child ran the race to its
    /// end (finish, or out of tape) instead of pausing.
    pub fn rollout(&mut self, h: Handle, recs: &[Rec], from: usize, k: u64) -> Result<Rolled, String> {
        match self.forest.advance_ex(h, recs, from, k)? {
            Advanced::Paused(mut rows, c) => {
                self.forest.release(c);
                drop_stale_tail(&mut rows);
                Ok(Rolled { rows, exited: false })
            }
            Advanced::Exited(rows) => Ok(Rolled { rows, exited: true }),
        }
    }

    /// Like `rollout` but keep the child alive (for chained macros / explore).
    pub fn rollout_keep(&mut self, h: Handle, recs: &[Rec], from: usize, k: u64) -> Result<(Vec<Row>, Handle), String> {
        self.forest.advance(h, recs, from, k)
    }

    /// The fork server's own verdict on `h` continued with `tail` to the end of
    /// the tape: the validator's CP count / time. A CLAIM, never a result — the
    /// plain oracle on the written file is the result.
    pub fn fork_verdict(&mut self, h: Handle, tail: &[Rec], from: usize) -> Result<branch::ForkAnswer, String> {
        self.forest.finish(h, tail, from)
    }

    pub fn release(&mut self, h: Handle) {
        self.forest.release(h)
    }

    pub fn live_nodes(&self) -> usize {
        self.forest.live_nodes()
    }

    /// The reference records for ticks `from..from+k` (macro 0: the human's
    /// own tape continued).
    pub fn reference_recs(&self, from: usize, k: usize) -> Vec<Rec> {
        (from..(from + k).min(self.tape.n()))
            .map(|t| rec_of(self.tape.steer[t], self.tape.accel[t], self.tape.brake[t]))
            .collect()
    }
}

/// Speed of a row, m/s.
pub fn speed(r: &Row) -> f64 {
    (r.vx * r.vx + r.vy * r.vy + r.vz * r.vz).sqrt()
}

pub fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

pub fn pos(r: &Row) -> [f64; 3] {
    [r.x, r.y, r.z]
}

/// THE LAST ROW OF A PAUSED CHILD IS STALE. The shim stops the engine inside
/// `lroundf`, after the race clock has stepped and before the physics has
/// integrated the tick: the final sample carries clock T with the position of
/// T−10. Measured: a 19.552 run paused at race 19.340 ended with two rows of
/// identical position, and the identity control read it as a 1.21 m error (one
/// tick at 121 m/s). The row is dropped; the state at T comes from the next
/// child, whose first sample is a full tick.
/// Under the TICK clock the pause is at the tick function's entry, the last
/// sample is a whole tick, and dropping it opened a one-row gap at every seam
/// (measured: 14950 → 14970); so this is the lroundf clock's fix only.
pub fn drop_stale_tail(rows: &mut Vec<Row>) {
    if !forkoracle::clock::tick_mode() && rows.len() >= 2 {
        rows.pop();
    }
}

impl Worker {
    /// TRUE race time of a row (ms): label + the measured shift.
    pub fn race_of(&self, r: &Row) -> i64 {
        r.time_ms + self.label_shift
    }
    /// The engine label that carries race time `ms`.
    pub fn label_of_race(&self, ms: i64) -> i64 {
        ms - self.label_shift
    }
}
