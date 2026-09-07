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
/// validator's ownership chain already built (a7aa56c `control::EARLIEST_CLOCK`).
pub const EARLIEST_CLOCK: u64 = 11_000;

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
    /// Launch a server on `ghost`, stop it at the earliest clock, resolve the car
    /// and open a forest. `work` must be private to this worker.
    pub fn start(
        server: &Path,
        map: &Path,
        shim: &Path,
        work: &Path,
        ghost: &Path,
        verbose: bool,
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
        let mut s = Session::start(&engine, tape.clone(), Checkpoint::Clock(EARLIEST_CLOCK))?;
        let probe = s.probe_tick()?;
        let recs = s.tape.tail_records(0);
        let wide = (-1.0e6, 1.0e6, -1.0e6, 1.0e6, -1.0e6, 1.0e6);
        let car = ValidatorCar::locate(&mut s.srv, probe, &recs, s.tape.start_offset_ms, wide, 40_000, verbose)?;
        let dir = work.join("traces");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let cfg = TraceCfg { layout: car.layout().clone(), dir, stride: 1, max: 400_000 };
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
        })
    }

    pub fn n_ticks(&self) -> usize {
        self.tape.n()
    }

    /// Run the reference tape from the root for `ticks` ticks in ONE child and
    /// return the per-tick rows. The flat trajectory of the human's own run.
    pub fn flat(&mut self, ticks: u64) -> Result<Vec<Row>, String> {
        // `k_ticks` places the stop through ~255 lroundf/tick, which drifts over
        // a long run (a 1956-tick request stopped 90 ticks short); walk in
        // chained chunks until the rows reach the requested tick.
        let target = self.root_probe + ticks as usize;
        let mut all: Vec<Row> = Vec::new();
        let mut h = ROOT;
        let mut left = ticks;
        for _ in 0..64 {
            // tree::Node::branch checks `from` against the floor even for an
            // empty write, so a chained chunk is addressed at the floor.
            let from = if h == ROOT { 0 } else { self.forest.floor(h, None)? };
            let (rows, c) = match self.forest.advance_ex(h, &[], from, left.clamp(1, 1500))? {
                Advanced::Paused(rows, c) => (rows, c),
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
            if at >= target || got == 0 || at + 1 >= self.tape.n() {
                break;
            }
            left = (target - at) as u64;
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
            Advanced::Paused(rows, c) => {
                self.forest.release(c);
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
