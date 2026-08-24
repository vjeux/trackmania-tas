//! [`ForkBranch`] — the explorer's `Branch`, on the real engine.
//!
//! # Which backend this is, and why it is the boring one
//!
//! Agent D's `Forest` is a savestate *tree*: a fork child stays alive and
//! becomes a new fork point, so a macro costs one branch instead of a whole
//! prefix. It is the fast answer and it is proven to 50 generations.
//!
//! This is the *other* one — the fallback the design budgets at ~37 min per
//! pass instead of ~6: **every `open` re-simulates the prefix from the fork
//! server's own checkpoint.** It is here first, and it is what the first real
//! runs use, for one reason: it needs no mechanism that has not already been
//! carrying this project's searches for weeks. `run_sampled_segs_ex` is the
//! same call `blind::locate` makes before anything is armed.
//!
//! The explorer cannot tell the difference — that is what `Branch::open`'s
//! `live` hint is for, and `tests/backend_indifference.rs` pins it — so
//! swapping in `Forest` later changes the clock and nothing else.
//!
//! # Two things about this backend that are NOT limitations of the game
//!
//! * **Ticks below the server's own probed boundary belong to the reference
//!   tape.** The fork server stops where it stops; a write below that is a
//!   silent no-op, which is the defect that produced 312 false finishes. So
//!   the explorer's tick 0 *is* the server's boundary, and everything before
//!   it is the synthesized container's own (neutral) input. On a standing
//!   start that is the countdown, and it costs the run nothing but the
//!   boundary's worth of time.
//! * **A trace costs the whole prefix.** `run_sampled_segs_ex` samples from
//!   the resume, so a 10-tick macro at tick 2000 streams 2010 samples. The
//!   explorer only keeps the last `k`. That is this backend's cost, not the
//!   engine's, and `Forest` removes it.

use forkoracle::blind::bounds_from;
use forkoracle::forksrv::{parse_result, write_key, ForkServer, Rec};
use forkoracle::inputs::Inputs;
use forkoracle::layout::{decode_rows, segments, tail_recs, Layout, Row, REC_LEN};
use std::path::{Path, PathBuf};
use tmexplore::action::Input;
use tmexplore::branch::{Advance, Branch, BranchErr, CarState, Handle};
use tmexplore::outcome::Verdict;

/// `lroundf` calls per simulated 10 ms tick, measured on this engine. Used
/// ONLY to bound how far a child runs. Nothing is ever labelled from it.
const LROUNDF_PER_TICK: u64 = 255;

pub struct ForkOpts {
    pub work: PathBuf,
    pub server: PathBuf,
    pub map: PathBuf,
    /// The synthesized container. Its tape is the reference below the
    /// boundary; nothing about it is a driver.
    pub reference_ghost: PathBuf,
    pub shim: PathBuf,
    /// The `lroundf` clock the server forks at. Earlier is better for a cold
    /// start: everything below the resulting boundary is fixed.
    pub checkpoint_clock: u64,
    pub start_offset_ms: i32,
    /// Every position on our own route, for the locator's search bounds. This
    /// is OUR route, derived from the map file — not a recorded line.
    pub route_points: Vec<[f32; 3]>,
    /// Extra ticks a candidate is allowed past its own end, so the child does
    /// not stop the instant the macro does.
    pub tail_margin: u32,
    /// The vehicle state's offset below the server's own base, from an
    /// EARLIER HONEST LOCATE. `None` sweeps for it (~70 s).
    ///
    /// Passed explicitly rather than through `FK_STATE_OFF`, because the
    /// env var is process-global and a worker that needs to fall back to a
    /// real sweep cannot unset it without racing every other worker. The
    /// offset does NOT always hold: this server's heap is bimodal run to run,
    /// and 7 of 12 workers took the shared offset and read a stationary
    /// address. `self_check` catches that; this field is what lets the worker
    /// then do the honest thing instead of dying.
    pub state_off: Option<u64>,
}

pub struct ForkBranch {
    srv: ForkServer,
    /// The server's own probed boundary + 1: the first tick this worker may
    /// write. The explorer's tick 0.
    pub from: usize,
    layout: Layout,
    segs: Vec<(u64, u32)>,
    /// The reference tape, full length, as the container holds it.
    reference: Inputs,
    /// Ticks of input the container can hold.
    pub capacity: usize,
    /// Handles are bookkeeping only in this backend: there is no live child to
    /// keep, so `open` records the prefix and `advance` replays it.
    parked: std::collections::HashMap<Handle, Vec<Input>>,
    next: Handle,
    pub sim_ticks: u64,
}

impl ForkBranch {
    pub fn start(o: &ForkOpts, reference: Inputs) -> Result<ForkBranch, String> {
        std::fs::create_dir_all(&o.work).map_err(|e| format!("{}: {}", o.work.display(), e))?;
        // THE KEY LIVES OUTSIDE THE WORK DIRECTORY. `ForkServer::start` does
        // `remove_dir_all(dir)` on the way in, so a key written inside it is
        // deleted before the server is told to canonicalize it -- which
        // presents as a bare "No such file or directory" from the launcher and
        // says nothing about which file.
        let keydir = o.work.parent().unwrap_or(std::path::Path::new("/tmp")).join("keys");
        std::fs::create_dir_all(&keydir).map_err(|e| format!("{}: {}", keydir.display(), e))?;
        let key = keydir.join(format!(
            "{}.key.bin",
            o.work.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or("w".into())
        ));
        // The shim is keyed on the tape the server is actually simulating. Key
        // it on the reference and not on anything else: keying it on a
        // template while the server ran a different tape is a real bug this
        // layer has already had (`bad handshake: ERR notfound`).
        write_key(&key, &reference.steer_u8());

        let mut srv = ForkServer::start(
            &o.work,
            &o.server,
            &o.map,
            &o.reference_ghost,
            &key,
            &o.shim,
            o.checkpoint_clock,
        )?;

        // WHERE DID THIS SERVER ACTUALLY STOP? Ask it. A failed probe is a
        // hard abort: a resume cannot be trusted without it, and a fallback
        // here is how the phantom got in.
        let probe = srv.probe_tick()?;
        let from = probe + 1;

        // IS THIS SERVER SIMULATING THE TAPE WE THINK IT IS? Two processes
        // sharing a work directory swap replays, and the result is a real,
        // self-consistent trajectory of a car that drove somewhere else —
        // nothing internal can see it, because nothing about it is
        // inconsistent.
        let gas: Vec<u8> = reference.gas.iter().map(|&v| v as u8).collect();
        let brake: Vec<u8> = reference.brake.iter().map(|&v| v as u8).collect();
        forkoracle::layout::verify_tape(srv.pid(), srv.base, &reference.steer_u8(), &gas, &brake)
            .map_err(|e| format!("this server is not simulating the tape we asked for: {}", e))?;

        // Addresses are re-derived in THIS process, every time: the server is
        // PIE and its heap is bimodal, so consecutive runs give different
        // addresses. A failure is an abort, never a guess.
        let lrecs = tail_recs(&reference.steer_u8(), &gas, &brake, from);
        let rows: Vec<Row> = o
            .route_points
            .iter()
            .map(|p| Row {
                time_ms: 0,
                x: p[0] as f64,
                y: p[1] as f64,
                z: p[2] as f64,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                qx: 0.0,
                qy: 0.0,
                qz: 0.0,
                qw: 0.0,
                wetness: 0.0,
            })
            .collect();
        let bounds = bounds_from(&rows, 300.0);
        // THE CLOCK-FIRST LOCATOR, not the reference-free one.
        //
        // `forkoracle::blind::locate_blind` is what `tmsearch --fork` uses and
        // it did not return here: three runs, **0 evaluations in 440 s, zero
        // errors**, at a load average of 1.8 — nothing failing and nothing
        // attempted. `fk::locate::locate_v2` finds the clock first by its
        // unforgeable +10 signature and then sweeps for the state near it, and
        // it located this exact container on this exact map in 69.3 s at a
        // velocity residual of 0.0482 m/s.
        //
        // 69 s per worker is still most of a fleet start, so `probe_state_off`
        // pays it ONCE and every worker takes the offset through
        // `FK_STATE_OFF`. That is an override taken from a real locate, never a
        // guess — and `self_check` re-reads the trajectory afterwards, so a
        // wrong address is caught by the data.
        let verbose = std::env::var("TMEX_VERBOSE_LOCATE").is_ok();
        let layout = match o.state_off {
            Some(off) => {
                // Take the offset, but still find the CLOCK honestly — it is
                // cheap (0.1 s) and it is what every tick is labelled from.
                let ck = fk::locate::find_clock2(&mut srv, from, &lrecs, o.start_offset_ms, 2000, verbose)?;
                let pos = srv
                    .base
                    .checked_sub(off)
                    .ok_or("the state offset is past the server's base")?;
                Layout { pos, clock: ck.addr, clock_bias: ck.bias, rms: 0.0, max_dev: 0.0 }
            }
            None => fk::locate::locate_v2(
            &mut srv,
            from,
            &lrecs,
            o.start_offset_ms,
            bounds,
                2000,
                4000,
                verbose,
            )
            .map_err(|e| format!("the car's state was not located: {}", e))?,
        };
        let segs = segments(&layout);
        let capacity = reference.len();
        Ok(ForkBranch {
            srv,
            from,
            layout,
            segs,
            reference,
            capacity,
            parked: Default::default(),
            next: 1,
            sim_ticks: 0,
        })
    }

    /// Ticks the search may write: the container's capacity minus the boundary.
    pub fn writable_ticks(&self) -> usize {
        self.capacity.saturating_sub(self.from)
    }

    /// Run `tape` (search ticks, i.e. from the boundary) for `want` ticks and
    /// return every sampled state.
    fn simulate(&mut self, tape: &[Input], want: usize) -> Result<(Vec<CarState>, Option<Verdict>), String> {
        let n = tape.len().min(self.writable_ticks());
        let recs: Vec<Rec> = (0..n)
            .map(|i| Rec {
                steer: tape[i].steer as f32 / 127.0,
                gas: if tape[i].gas { 1.0 } else { 0.0 },
                brake: if tape[i].brake { 1.0 } else { 0.0 },
            })
            .collect();
        // Bit 31 of `max` makes the child exit as soon as the sample budget is
        // spent rather than simulating on in silence.
        let samples = (n as u32).saturating_add(4);
        let budget = ((n as u64 + 4) * LROUNDF_PER_TICK).min(u32::MAX as u64) as u32;
        let (json, blob) = self.srv.run_sampled_segs_ex(
            self.from,
            &recs,
            &self.segs,
            1,
            samples | 0x8000_0000,
            (0, 4),
            budget,
        );
        self.sim_ticks += n as u64;
        let (rows, _) = decode_rows(&blob, &self.layout, 0);
        let states: Vec<CarState> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| CarState {
                tick: i as u32 + 1,
                pos: [r.x as f32, r.y as f32, r.z as f32],
                vel: [r.vx as f32, r.vy as f32, r.vz as f32],
                quat: [r.qw as f32, r.qx as f32, r.qy as f32, r.qz as f32],
                // The 48-byte record carries no wheel-contact word. UNKNOWN,
                // and stated as such: reporting "all four wheels down" would be
                // a claim the readout cannot make, and the crude bin key
                // therefore does not use it. Widening the readout is a task,
                // not an impossibility.
                wheels: 0b1111,
                airtime: 0,
                cps: 0,
            })
            .collect();
        let _ = want;
        let (time, cps) = parse_result(&json);
        let ended = match (time, cps) {
            (Some(ms), _) => Some(Verdict::Finish { ms }),
            (None, Some(c)) => Some(Verdict::Dnf { cps: c }),
            _ => None,
        };
        Ok((states, ended))
    }

    pub fn reference(&self) -> &Inputs {
        &self.reference
    }
    pub fn layout(&self) -> &Layout {
        &self.layout
    }
    pub fn record_len(&self) -> usize {
        REC_LEN
    }
}

impl Branch for ForkBranch {
    fn open(&mut self, prefix: &[Input], _live: Option<Handle>) -> Result<Handle, BranchErr> {
        // The hint is ignored ON PURPOSE in this backend: there is no live
        // child. Ignoring it must never change the answer, and it does not,
        // because the prefix is re-simulated either way.
        let id = self.next;
        self.next += 1;
        self.parked.insert(id, prefix.to_vec());
        Ok(id)
    }

    fn advance(
        &mut self,
        h: Handle,
        from_tick: u32,
        inputs: &[Input],
    ) -> Result<Advance, BranchErr> {
        let prefix = self.parked.remove(&h).ok_or(BranchErr::Stale)?;
        if (from_tick as usize) < prefix.len() {
            return Err(BranchErr::BelowBoundary { asked: from_tick, boundary: prefix.len() as u32 });
        }
        let mut tape = prefix;
        tape.extend_from_slice(inputs);
        let total = tape.len();
        if total > self.writable_ticks() {
            return Err(BranchErr::Other(format!(
                "a tape of {} ticks exceeds the container's {} writable ticks",
                total,
                self.writable_ticks()
            )));
        }
        let (all, ended) = self.simulate(&tape, total).map_err(BranchErr::Other)?;
        // Keep only the states this macro produced. The rest were the prefix
        // being replayed, and they are already in the archive.
        let keep = inputs.len().min(all.len());
        let trace: Vec<CarState> = all[all.len() - keep..]
            .iter()
            .enumerate()
            .map(|(i, s)| CarState { tick: from_tick + i as u32 + 1, ..*s })
            .collect();
        Ok(Advance { trace, handle: None, ended })
    }

    fn close(&mut self, h: Handle) {
        self.parked.remove(&h);
    }

    fn initial_state(&mut self) -> Result<CarState, BranchErr> {
        let (s, _) = self.simulate(&[], 1).map_err(BranchErr::Other)?;
        s.first().copied().map(|mut c| {
            c.tick = 0;
            c
        })
        .ok_or_else(|| BranchErr::Other("the server returned no state at the boundary".into()))
    }

    fn tick_limit(&self) -> u32 {
        self.writable_ticks() as u32
    }
}

/// The reference tape a synthesized container holds: neutral everywhere.
///
/// It is not a driver and it is not a seed. It is the bit layout the candidate
/// writer patches and the inputs the engine consumes below the fork boundary.
pub fn neutral_reference(n: usize) -> Inputs {
    Inputs { steer: vec![0; n], gas: vec![false; n], brake: vec![false; n] }
}


/// Locate the car ONCE, honestly, and return the state slot's offset from the
/// server's own base.
///
/// The sweep costs ~70 s. The slot sits at a fixed offset from the base for a
/// given build, so paying it once and handing the offset to every worker turns
/// a fleet start from 70 s per worker into 70 s total. It is an override taken
/// from a real locate, never a guess — and every worker still runs
/// [`ForkBranch::self_check`] on what it reads back, so a wrong offset is
/// caught by the data rather than assumed away.
///
/// It locates at a checkpoint HALFWAY THROUGH THE TAPE, not at the search's own
/// early boundary, and that is not a detail. `locate_v2`'s discriminator is
/// `d(pos)/dt` against the stored velocity, qualified against 2 % of the mean
/// speed in the window — so a probe where the car is slow gets the tightest
/// threshold and the largest residual, and the same tape on the same map
/// locates or refuses purely on where the checkpoint fell. A standing start is
/// the worst possible place to ask.
pub fn probe_state_offset(o: &ForkOpts, reference: &Inputs, frac: f64) -> Result<u64, String> {
    let work = o.work.parent().unwrap_or(std::path::Path::new("/tmp")).join("locate-probe");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let keydir = work.join("keys");
    std::fs::create_dir_all(&keydir).map_err(|e| e.to_string())?;
    let key = keydir.join("probe.key.bin");
    write_key(&key, &reference.steer_u8());

    let n = reference.len();
    let ckpt = crate::clock_for_tick_public((n as f64 * frac) as i64, o.start_offset_ms);
    let mut srv = ForkServer::start(
        &work.join("srv"),
        &o.server,
        &o.map,
        &o.reference_ghost,
        &key,
        &o.shim,
        ckpt,
    )?;
    let probe = srv.probe_tick()?;
    let gas: Vec<u8> = reference.gas.iter().map(|&v| v as u8).collect();
    let brake: Vec<u8> = reference.brake.iter().map(|&v| v as u8).collect();
    let lrecs = tail_recs(&reference.steer_u8(), &gas, &brake, probe);
    let rows: Vec<Row> = o
        .route_points
        .iter()
        .map(|p| Row {
            time_ms: 0,
            x: p[0] as f64,
            y: p[1] as f64,
            z: p[2] as f64,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            qx: 0.0,
            qy: 0.0,
            qz: 0.0,
            qw: 0.0,
            wetness: 0.0,
        })
        .collect();
    let bounds = bounds_from(&rows, 400.0);
    let l = fk::locate::locate_v2(&mut srv, probe, &lrecs, o.start_offset_ms, bounds, 2000, 4000, true)?;
    let off = srv
        .base
        .checked_sub(l.pos)
        .ok_or("the located state is ABOVE the input array; that is not the layout this expects")?;
    Ok(off)
}

impl ForkBranch {
    /// **The guard on `FK_STATE_OFF`.** Read a short trajectory back and
    /// require it to be a car: the quaternion normalised, and the position
    /// derivative agreeing with the stored velocity.
    ///
    /// This is the check that makes an offset override safe. A wrong address
    /// holding float triples produces rows that fail both — and a locate that
    /// matches the WRONG thing is far worse than one that fails, because it
    /// answers.
    pub fn self_check(&mut self) -> Result<String, String> {
        // 200 ticks of full throttle: the car has to MOVE for any of this to
        // mean anything, and an empty tape samples nothing (my first version
        // asked for zero ticks and got four states, then reported UNMEASURED —
        // which was the right word for it).
        let tape: Vec<Input> = vec![Input { steer: 0, gas: true, brake: false }; 200];
        let (states, _) = self.simulate(&tape, 200)?;
        if states.len() < 20 {
            return Err(format!(
                "the readout produced {} states; it cannot be checked, so it is UNMEASURED",
                states.len()
            ));
        }
        let mut qerr: f64 = 0.0;
        let mut verrs: Vec<f64> = Vec::new();
        for w in states.windows(2) {
            let (a, b) = (w[0], w[1]);
            let q = b.quat;
            let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
            qerr = qerr.max((n - 1.0).abs() as f64);
            let d = [
                (b.pos[0] - a.pos[0]) * 100.0,
                (b.pos[1] - a.pos[1]) * 100.0,
                (b.pos[2] - a.pos[2]) * 100.0,
            ];
            let e = ((d[0] - a.vel[0]).powi(2) + (d[1] - a.vel[1]).powi(2) + (d[2] - a.vel[2]).powi(2))
                .sqrt() as f64;
            verrs.push(e);
        }
        verrs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = verrs[verrs.len() / 2];
        let moved = {
            let a = states.first().unwrap().pos;
            let b = states.last().unwrap().pos;
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        };
        // A car that never moves passes a velocity-consistency check trivially
        // (0 = 0), so the distance travelled is checked too: this is the same
        // family as a test whose pass condition any outcome satisfies.
        if moved < 1.0 {
            return Err(format!(
                "the located state moved {:.3} m over {} ticks. Either this is not the car, or the \
                 reference tape does not drive -- and a stationary car passes a velocity check for free.",
                moved,
                states.len()
            ));
        }
        if med > 0.5 || qerr > 1e-3 {
            return Err(format!(
                "the readout does not look like a car: |d(pos)/dt - v| median {:.4} m/s (bar 0.5), \
                 |q|-1 max {:.2e} (bar 1e-3)",
                med, qerr
            ));
        }
        Ok(format!(
            "{} states, |d(pos)/dt - v| median {:.4} m/s, |q|-1 max {:.2e}, travelled {:.1} m",
            states.len(),
            med,
            qerr,
            moved
        ))
    }
}
