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
//! * **The resume boundary is per worker.** The `lroundf` checkpoint is not a
//!   fixed simulation point: under load the count moves in whole chunks of ~62
//!   calls (~0.24 tick), so servers started together stop at different ticks --
//!   104 of 150 workers stopped one tick later than the master's single
//!   calibration when 150 started at once. Each worker therefore probes its own
//!   server and publishes `max(calibration, probe + 1)`; `probe + 1` because
//!   tick `probe` is already partly consumed. The search takes the MAXIMUM over
//!   workers as its mutation floor -- it must be the maximum, because migration
//!   moves a state made by one worker into another.
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
use forkoracle::layout::{segments, tail_recs, Row, REC_LEN, R_CLOCK, R_POS, R_VEL};
use forkoracle::blind::{bounds_from, locate_blind as locate};
use forkoracle::pred::{outcome, Watch};

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
    let ms = tick * 10 + start_offset_ms as i64;
    (36141.0 + 25.483 * ms as f64).max(1000.0) as u64
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
}

pub struct ForkSetup {
    pub server: PathBuf,
    pub map: PathBuf,
    pub reference_ghost: PathBuf,
    pub key: PathBuf,
    pub shim: PathBuf,
    pub checkpoint_clock: u64,
    /// The boundary the master calibrated against ground truth.
    pub calibrated: usize,
    pub start_offset_ms: i32,
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
        let probe = srv.probe_tick()?;
        let from = s.calibrated.max(probe + 1);

        let steer: Vec<u8> = reference.steer.iter().map(|&v| v as u8).collect();
        let gas: Vec<u8> = reference.gas.iter().map(|&v| v as u8).collect();
        let brake: Vec<u8> = reference.brake.iter().map(|&v| v as u8).collect();
        let lrecs = tail_recs(&steer, &gas, &brake, from);

        let rows: Vec<Row> = (0..watch.refline.n)
            .map(|i| Row {
                time_ms: 0,
                x: watch.refline.xyz[3 * i] as f64,
                y: watch.refline.xyz[3 * i + 1] as f64,
                z: watch.refline.xyz[3 * i + 2] as f64,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                qx: 0.0,
                qy: 0.0,
                qz: 0.0,
                qw: 0.0,
            })
            .collect();
        let bounds = bounds_from(&rows, 200.0);

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

        // Addresses are re-derived in THIS process, every time: the server is
        // PIE and its heap is bimodal, so five consecutive runs give five
        // different addresses. A failure is an abort, never a guess.
        let layout = locate(&mut srv, from, &lrecs, s.start_offset_ms, 1, bounds, false)
            .map_err(|e| format!("the car's state was not located: {}", e))?;

        let ack = srv.arm(&watch.arm_payload(
            layout.clock_bias + s.start_offset_ms as i64,
            R_CLOCK as u32,
            R_POS as u32,
            R_VEL as u32,
            REC_LEN as u32,
            &segments(&layout),
        ));
        if !ack.starts_with("ARMED") {
            return Err(format!("arming the watchdog failed: {}", ack));
        }
        let line_len = watch.refline.s_at_tick(usize::MAX);
        Ok(ForkEval { srv, from, line_len, reference })
    }
}

impl Evaluator for ForkEval {
    fn evaluate(&mut self, cands: &[Inputs]) -> Vec<Outcome> {
        let mut out = Vec::with_capacity(cands.len());
        for c in cands {
            let recs: Vec<Rec> = (self.from..c.len())
                .map(|t| Rec {
                    steer: c.steer[t] as f32 / 127.0,
                    gas: if c.gas[t] { 1.0 } else { 0.0 },
                    brake: if c.brake[t] { 1.0 } else { 0.0 },
                })
                .collect();
            let (j, b) = self.srv.run_watched(self.from, &recs);
            let o = outcome(&j, &b);
            out.push(match o.time {
                Some(ms) => Outcome::Finish { ms },
                None => Outcome::Dnf(Progress::Metres { m: o.progress(), of: self.line_len }),
            });
        }
        out
    }

    fn floor(&self) -> usize {
        self.from
    }

    fn provenance(&self, inputs: &Inputs) -> Provenance {
        Provenance {
            from_fork: true,
            resume_tick: Some(self.from),
            distance: inputs.distance_from(&self.reference),
        }
    }

    fn finish(self: Box<Self>) {
        self.srv.quit();
    }
}
