//! THE CAR, FOUND BY ASKING THE ENGINE — not by sweeping memory for one.
//!
//! Two objects hold this run's car, and both are needed:
//!
//! * the **validator's own** `CGameVehiclePhy`, reached by a typed pointer walk
//!   from the callback the shim captured before `main` (`VALIDATOR_CAR.md`).
//!   No search, no candidates, nothing to choose between — but it is one tick
//!   AHEAD of the vis state and it does not carry the fields the sampler wants;
//! * the **vis state**, which is what every consumer reads (`segments()`: the
//!   quaternion at −16, the position, the velocity at +12, the wetness at
//!   +180) and what the 3.4 mm agreement with ghost telemetry was measured on.
//!
//! The vis state used to be found by sweeping: fork the engine once per 64 KB
//! window of a 150 MB address space, gather 150 samples in each, and keep the
//! most self-consistent moving float triple. That is **3.6 s per fork server**,
//! it is paid by every worker of every search, and it can be fooled — measured,
//! twice: with the whole world allowed it picks a STATIONARY object 1624 m from
//! the car on 126859 and 1192 m on 145875, both of which pass their own
//! self-consistency test because nothing that never moves can contradict a zero
//! velocity.
//!
//! This finds it in two steps that cannot pick a decoy:
//!
//! 1. read the car's position out of the validator's own object, and scan the
//!    paused parent for that exact 12-byte triple. No forks — one pass over
//!    memory the parent is already holding still.
//! 2. require each candidate to TRACK the validator's car over ticks, which is
//!    what separates the live state from a frozen copy parked at the same
//!    coordinates (that is not hypothetical either: a pointer chain resolved to
//!    exactly such a copy on map 2, 0.1 m from the car and never moving again).
//!
//! Measured against the sweep it replaces: **3.6 s → 0.2 s**, same address.

use crate::forksrv::{ForkServer, Rec};
use crate::layout::Layout;
use crate::procmem;

/// The validator's ownership chain, build 128182. Every offset is checked at
/// its own step; a null, an unreadable word or a participant count that is not
/// exactly 1 is a hard error, never a fallback to a search.
pub struct ValidatorChain {
    pub controller: u64,
    pub sim: u64,
    pub playground: u64,
    pub participant: u64,
    pub vehicle: u64,
    /// The world position triple inside the `CGameVehiclePhy`.
    pub pos: u64,
}

const SIM_IN_CONTROLLER: u64 = 0x1a70;
const PLAYGROUND_IN_SIM: u64 = 0x18;
const PLAYERS_IN_PLAYGROUND: u64 = 0x660;
const NPLAYERS_IN_PLAYGROUND: u64 = 0x668;
const CLASS_IN_PARTICIPANT: u64 = 0x1110;
const VEHICLE_IN_PARTICIPANT: u64 = 0x1118;
const CGAME_VEHICLE_PHY: u32 = 0x032e_2000;
const POS_IN_VEHICLE: u64 = 0x12f0;

/// Walk it. Costs six 8-byte reads of `/proc/<pid>/mem` and no simulation.
pub fn validator_chain(srv: &ForkServer) -> Result<ValidatorChain, String> {
    let pid = srv.pid();
    let word = |a: u64, what: &str| -> Result<u64, String> {
        let b = procmem::read_at(pid, a, 8)
            .ok_or_else(|| format!("{}: cannot read {:#x}", what, a))?;
        let v = u64::from_le_bytes(b[..8].try_into().unwrap());
        if v == 0 {
            return Err(format!("{}: null at {:#x}", what, a));
        }
        Ok(v)
    };
    let (controller, sim) = (srv.validator_controller, srv.validation_sim);
    if controller == 0 || sim == 0 {
        return Err("this server did not capture the validator's callback".into());
    }
    // The captured argument must agree with the object's own field, or the
    // capture is not describing this simulation.
    let bound = word(controller + SIM_IN_CONTROLLER, "controller.sim")?;
    if bound != sim {
        return Err(format!(
            "the captured simulation {:#x} is not the one the controller holds ({:#x})",
            sim, bound
        ));
    }
    let playground = word(sim + PLAYGROUND_IN_SIM, "sim.playground")?;
    let n = procmem::read_at(pid, playground + NPLAYERS_IN_PLAYGROUND, 4)
        .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))
        .ok_or("cannot read the participant count")?;
    if n != 1 {
        return Err(format!("{} participants; a validation has exactly 1", n));
    }
    let players = word(playground + PLAYERS_IN_PLAYGROUND, "playground.players")?;
    let participant = word(players, "players[0]")?;
    let class = procmem::read_at(pid, participant + CLASS_IN_PARTICIPANT, 4)
        .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))
        .ok_or("cannot read the vehicle class id")?;
    if class != CGAME_VEHICLE_PHY {
        return Err(format!(
            "participant's class id is {:#x}, not CGameVehiclePhy ({:#x})",
            class, CGAME_VEHICLE_PHY
        ));
    }
    let vehicle = word(participant + VEHICLE_IN_PARTICIPANT, "participant.vehicle")?;
    Ok(ValidatorChain {
        controller,
        sim,
        playground,
        participant,
        vehicle,
        pos: vehicle + POS_IN_VEHICLE,
    })
}

pub fn read_xyz(pid: i32, at: u64) -> Option<[f32; 3]> {
    let b = procmem::read_at(pid, at, 12)?;
    let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    Some([f(0), f(4), f(8)])
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f64 {
    (((a[0] - b[0]) as f64).powi(2) + ((a[1] - b[1]) as f64).powi(2) + ((a[2] - b[2]) as f64).powi(2))
        .sqrt()
}

/// How many ticks a candidate must track the validator's car for.
const TRACK_TICKS: u32 = 24;

/// Does the state at `pos` HOLD the car, rather than merely sit where it is?
///
/// Simulate `TRACK_TICKS` ticks, gathering the candidate and the validator's
/// own car in one sample, and require the candidate to stay within a tick of
/// travel of the car AND to travel the distance the car travels. A frozen copy
/// passes the first and fails the second by the whole distance.
pub fn tracks_the_car(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    pos: u64,
    truth: u64,
) -> Result<f64, String> {
    let keep = ((TRACK_TICKS as usize) + 8).min(recs.len());
    let (_j, blob) = srv.run_sampled_segs_ex(
        probe,
        &recs[..keep],
        &[(pos, 12u32), (truth, 12u32)],
        1,
        TRACK_TICKS | 0x8000_0000,
        (0, 24),
        crate::clock::budget_for_ticks(TRACK_TICKS + 4),
    );
    let recsz = 8 + 24usize;
    let n = blob.len() / recsz;
    if n < 8 {
        return Err(format!("only {} samples", n));
    }
    let xyz = |i: usize, o: usize| -> [f32; 3] {
        let b = &blob[i * recsz + 8 + o..];
        let f = |k: usize| f32::from_le_bytes(b[k..k + 4].try_into().unwrap());
        [f(0), f(4), f(8)]
    };
    let (mut worst, mut worst_bar) = (0.0f64, 0.0f64);
    let (mut cand, mut car) = (0.0f64, 0.0f64);
    for i in 1..n {
        let step = dist(xyz(i - 1, 12), xyz(i, 12));
        car += step;
        cand += dist(xyz(i - 1, 0), xyz(i, 0));
        let d = dist(xyz(i, 0), xyz(i, 12));
        let bar = step + 0.05;
        if d > bar && d - bar > worst - worst_bar {
            worst = d;
            worst_bar = bar;
        }
    }
    if worst > worst_bar {
        return Err(format!(
            "{:.1} m from the car at one sample (a tick of travel is {:.2} m)",
            worst, worst_bar
        ));
    }
    if car > 1.0 && (cand - car).abs() > 0.1 * car {
        return Err(format!(
            "travelled {:.1} m while the car travelled {:.1} m -- not the same object",
            cand, car
        ));
    }
    Ok(worst)
}

/// The whole locate: the clock and the vis state, without a sweep.
pub fn locate_fast(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    verbose: bool,
) -> Result<Layout, String> {
    let t0 = std::time::Instant::now();
    let chain = validator_chain(srv)?;
    let want = read_xyz(srv.pid(), chain.pos).ok_or("cannot read the validator's car")?;
    if !want.iter().all(|v| v.is_finite()) {
        return Err("the validator's car holds a non-finite position".into());
    }
    // WHERE THE CAR IS, IS THE NEEDLE -- but not byte for byte. The vis state
    // holds the same car ONE TICK EARLIER, so its bytes equal the validator's
    // only while the car is stationary (measured: identical at the start line,
    // 0.017 m apart at 1.7 m/s, 0.836 m at 84 m/s). So the scan is for a float
    // triple WITHIN A TICK OF TRAVEL of the car, which is the same test the
    // tracking check applies later, at one instant.
    //
    // It runs in the PARENT: one pass over memory the process is already
    // holding still, no forks at all, where the sweep it replaces pays a fork
    // per 64 KB window. The order is nearest-first from the vis state's measured
    // neighbourhood (~600 KB below the input array); the ORDER is a hint and
    // the tracking test is what decides.
    let hits = scan_near(srv.pid(), want, CAR_MATCH_M, srv.base.saturating_sub(603_616));
    if verbose {
        println!(
            "car {:#x} = ({:.3}, {:.3}, {:.3}); {} triple(s) within {} m of it [{:.2}s]",
            chain.pos,
            want[0],
            want[1],
            want[2],
            hits.len(),
            CAR_MATCH_M,
            t0.elapsed().as_secs_f64()
        );
    }
    if hits.is_empty() {
        return Err("nothing in the parent holds a position within a tick of the car".into());
    }
    // WHICH of them holds the car: tested SEVEN AT A TIME. The candidates are
    // gathered as segments of one sample, so a fork judges seven of them over
    // 24 ticks instead of one -- 64 candidates cost 10 forks, not 64. (Testing
    // them one at a time, each preceded by its own clock hunt, was 5.4 s of the
    // 6 s this whole path used to take.)
    let cands: Vec<u64> = hits.iter().copied().filter(|a| *a != chain.pos).collect();
    // 24 ticks is enough wherever the car is moving. At the start line it is
    // not: over 0.24 s a car at 1.7 m/s travels 0.4 m, which cannot separate
    // the vis state from the physics state (they are a tick apart, and a tick
    // is 17 mm there). So widen ONCE rather than guess -- the car is
    // accelerating, and 96 ticks covers several metres.
    let mut why: Vec<String> = Vec::new();
    let mut passed: Vec<(u64, f64)> = Vec::new();
    for ticks in [TRACK_TICKS, TRACK_TICKS * 4] {
        why.clear();
        for group in cands.chunks(MAX_SEG - 1) {
            match tracks_the_car_batch_over(srv, probe, recs, group, chain.pos, ticks) {
                Ok(mut v) => passed.append(&mut v),
                Err(e) => why.push(e),
            }
        }
        if !passed.is_empty() {
            break;
        }
    }
    passed.sort_by(|a, b| a.1.total_cmp(&b.1));
    if verbose {
        println!(
            "  {} of {} candidates track the car [{:.2}s]",
            passed.len(),
            cands.len(),
            t0.elapsed().as_secs_f64()
        );
    }
    if passed.is_empty() {
        return Err(format!(
            "nothing that holds the car's position tracks the validator's own car: {}",
            why.join("; ")
        ));
    }
    // SEVERAL copies track the car -- the engine keeps render transforms and
    // double buffers of it. The one the sampler wants is the VIS STATE, and
    // what distinguishes it is that the race counter lives beside it (measured
    // -7916, -11268, -14780 from the position on different runs, so the hunt
    // searches both ways). A copy with no counter near it is a copy: the clock
    // hunt is the discriminator here, not merely a lookup.
    for (pos, d) in &passed {
        let Ok((clock, bias)) =
            crate::layout::find_clock(srv, probe, recs, 0, *pos, 16384, 4096, 1)
        else {
            continue;
        };
        if verbose {
            println!(
                "state {:#014x}, clock {:#014x} (bias {:+} ms), tracks the validator's car to \
                 {:.4} m [{:.2}s]",
                pos,
                clock,
                bias,
                d,
                t0.elapsed().as_secs_f64()
            );
        }
        return Ok(Layout { pos: *pos, clock, clock_bias: bias, rms: *d, max_dev: 0.0, cps: 0 });
    }
    Err(format!(
        "{} copies track the car and none has a race counter beside it",
        passed.len()
    ))
}

/// The shim gathers at most this many segments per sample.
const MAX_SEG: usize = 8;

/// [`tracks_the_car`] for up to `MAX_SEG - 1` candidates in ONE fork: they are
/// gathered side by side with the validator's car, so every candidate is judged
/// against the same 24 ticks of the same simulation.
pub fn tracks_the_car_batch(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    cands: &[u64],
    truth: u64,
) -> Result<Vec<(u64, f64)>, String> {
    tracks_the_car_batch_over(srv, probe, recs, cands, truth, TRACK_TICKS)
}

/// As above, over an explicit number of ticks.
pub fn tracks_the_car_batch_over(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    cands: &[u64],
    truth: u64,
    track_ticks: u32,
) -> Result<Vec<(u64, f64)>, String> {
    let mut segs: Vec<(u64, u32)> = cands.iter().map(|a| (*a, 12u32)).collect();
    segs.push((truth, 12u32));
    let reclen = 12 * segs.len();
    let keep = ((track_ticks as usize) + 8).min(recs.len());
    let (_j, blob) = srv.run_sampled_segs_ex(
        probe,
        &recs[..keep],
        &segs,
        1,
        track_ticks | 0x8000_0000,
        (0, reclen as u32),
        crate::clock::budget_for_ticks(track_ticks + 4),
    );
    let recsz = 8 + reclen;
    let n = blob.len() / recsz;
    if n < 8 {
        return Err(format!("only {} samples for {} candidates", n, cands.len()));
    }
    let xyz = |i: usize, k: usize| -> [f32; 3] {
        let b = &blob[i * recsz + 8 + k * 12..];
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        [f(0), f(4), f(8)]
    };
    let truth_k = cands.len();
    let mut ok: Vec<(u64, f64)> = Vec::new();
    let mut why = String::new();
    for (k, addr) in cands.iter().enumerate() {
        let (mut worst, mut worst_bar) = (0.0f64, 0.0f64);
        let (mut cand, mut car) = (0.0f64, 0.0f64);
        // WHICH COPY: the vis state LAGS the validator's CGameVehiclePhy by one
        // tick, and the engine also keeps mirrors that hold the physics state
        // itself. Both track the car; only one is the object every consumer's
        // offsets (quaternion at -16, velocity at +12, wetness at +180) and the
        // clock bias were calibrated on, and picking the other shifts every
        // label by a tick -- which is 0.8 m at racing speed and is exactly the
        // error `fk trace` measures against a ghost's own telemetry.
        //
        // So compare each candidate against the car NOW and against the car ONE
        // SAMPLE AGO: the vis state matches the older one.
        let (mut d_now, mut d_prev) = (0.0f64, 0.0f64);
        for i in 1..n {
            let step = dist(xyz(i - 1, truth_k), xyz(i, truth_k));
            car += step;
            cand += dist(xyz(i - 1, k), xyz(i, k));
            let d = dist(xyz(i, k), xyz(i, truth_k));
            d_now += d;
            d_prev += dist(xyz(i, k), xyz(i - 1, truth_k));
            let bar = step + 0.05;
            if d > bar && d - bar > worst - worst_bar {
                worst = d;
                worst_bar = bar;
            }
        }
        if worst > worst_bar {
            why = format!("{:#x}: {:.1} m off at one sample", addr, worst);
            continue;
        }
        if car > 1.0 && (cand - car).abs() > 0.1 * car {
            why = format!("{:#x}: travelled {:.1} m vs the car's {:.1} m", addr, cand, car);
            continue;
        }
        if car < 1.0 {
            why = format!(
                "the car moved {:.2} m in this window -- too little to tell the vis state from \
                 the physics state; a locate here cannot know which tick it is labelling",
                car
            );
            continue;
        }
        if d_prev >= d_now {
            why = format!(
                "{:#x}: holds the physics state, not the vis state ({:.2} m from the car now vs \
                 {:.2} m from where it was a tick ago)",
                addr,
                d_now / (n - 1) as f64,
                d_prev / (n - 1) as f64
            );
            continue;
        }
        ok.push((*addr, worst));
    }
    if ok.is_empty() {
        return Err(why);
    }
    Ok(ok)
}

/// How far a candidate may sit from the validator's car at the checkpoint: one
/// tick of travel at any speed the game reaches, with slack.
const CAR_MATCH_M: f64 = 2.0;

/// Every 4-byte-aligned float triple in the parent's writable memory within
/// `tol` metres of `want`, regions nearest `hint` first.
///
/// This is the whole of "find the car" now, and it runs in the PARENT: one pass
/// over memory the process is already holding still, no forks at all, where the
/// sweep it replaces pays a fork per 64 KB window. The ORDER is a hint (the vis
/// state sits ~600 KB below the input array on this build); the tracking test
/// is what decides.
fn scan_near(pid: i32, want: [f32; 3], tol: f64, hint: u64) -> Vec<u64> {
    let mut regions: Vec<procmem::Region> = procmem::maps(pid)
        .into_iter()
        .filter(|r| r.perms.starts_with("rw") && r.path != "[vvar]" && r.path != "[vsyscall]")
        .collect();
    regions.sort_by_key(|r| {
        let mid = r.start + (r.end - r.start) / 2;
        (mid as i64 - hint as i64).unsigned_abs()
    });
    let mut out = Vec::new();
    for r in regions {
        let Some(buf) = procmem::read_at(pid, r.start, (r.end - r.start) as usize) else {
            continue;
        };
        let f = |o: usize| f32::from_le_bytes(buf[o..o + 4].try_into().unwrap());
        let mut o = 0usize;
        while o + 12 <= buf.len() {
            // x first: one compare rejects almost everything.
            let x = f(o);
            if (x - want[0]).abs() as f64 <= tol {
                let (y, z) = (f(o + 4), f(o + 8));
                if (y - want[1]).abs() as f64 <= tol && (z - want[2]).abs() as f64 <= tol {
                    let d = (((x - want[0]) as f64).powi(2)
                        + ((y - want[1]) as f64).powi(2)
                        + ((z - want[2]) as f64).powi(2))
                    .sqrt();
                    if d <= tol {
                        out.push(r.start + o as u64);
                    }
                }
            }
            o += 4;
        }
        if out.len() >= 64 {
            break;
        }
    }
    out
}
