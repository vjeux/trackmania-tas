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
    /// All four slots, live or not: the finish record is in one of them and it
    /// is not always the live one.
    pub vehicles: Vec<u64>,
    /// The world position triple inside the `CGameVehiclePhy`.
    pub pos: u64,
}

const SIM_IN_CONTROLLER: u64 = 0x1a70;
const PLAYGROUND_IN_SIM: u64 = 0x18;
const PLAYERS_IN_PLAYGROUND: u64 = 0x660;
const NPLAYERS_IN_PLAYGROUND: u64 = 0x668;
const CLASS_IN_PARTICIPANT: u64 = 0x1110;
/// The participant holds FOUR vehicle slots -- Stadium, Snow, Rally, Desert --
/// and only one of them is the car being driven. (From the tm-player project's
/// INPUT arm, `input/WHEELS.md`.) Reading slot 0 unconditionally is right on an
/// ordinary map and FREEZES on a transform map, where the live car is in
/// another slot: the chain then resolves to a parked vehicle that never moves,
/// which is exactly the failure the tracking test below reports as "travelled
/// 0.0 m while the car travelled 40 m".
const VEHICLE_SLOTS: [u64; 4] = [0x1118, 0x1128, 0x1138, 0x1148];
/// A live vehicle's `phy+0x10` is not `0xffffffff`; a parked slot's is.
const LIVE_MARK_IN_VEHICLE: u64 = 0x10;
/// THE CHECKPOINT COUNTER, in the participant: a u32 that increments exactly at
/// the ghosts' split ticks, the finish included, on every server.
///
/// Located behaviourally by the tm-player project's ENV arm and verified
/// against the plain oracle 200/200 (0/1 x138, 2 x30, 3 x19, finish x13) and
/// 1288/1289 over 2453 tapes. `+0xc80`, `+0xc90` and `+0xc94` step at the same
/// instants. This arm's own finish hunt found the same word independently:
/// `participant+0xc70` goes 2 -> 3 exactly one tick after the finish tick on
/// three tapes with three different finish times, which is what made the
/// finish detectable at all.
pub const CP_COUNT_IN_PARTICIPANT: u64 = 0xc70;
/// The vis state -- position, velocity, attitude, wheels -- INSIDE the phy
/// object (tm-player INPUT arm, `input/WHEELS.md`: the binary's own sample
/// writer on it reproduces the ghosts' 116-byte samples). This is the same
/// object the pointer chains reach the long way round, one add from a pointer
/// whose identity is already proven.
#[allow(dead_code)]
const VIS_STATE_IN_VEHICLE: u64 = 0x848;
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
    // WHICH OF THE FOUR SLOTS is being driven: the one whose `phy+0x10` is not
    // the empty marker. Exactly one must qualify -- two would mean this is not
    // the field that says so.
    let mut live: Vec<u64> = Vec::new();
    let mut all: Vec<u64> = Vec::new();
    for s in VEHICLE_SLOTS {
        let Ok(v) = word(participant + s, "vehicle slot") else {
            continue;
        };
        all.push(v);
        let Some(m) = procmem::read_at(pid, v + LIVE_MARK_IN_VEHICLE, 4)
            .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))
        else {
            continue;
        };
        if m != u32::MAX {
            live.push(v);
        }
    }
    let vehicle = match live.len() {
        1 => live[0],
        0 => return Err("none of the four vehicle slots is live".into()),
        n => {
            return Err(format!(
                "{} of the four vehicle slots look live -- phy+0x10 is not the marker on this \
                 build",
                n
            ))
        }
    };
    Ok(ValidatorChain {
        controller,
        sim,
        playground,
        participant,
        vehicle,
        vehicles: all,
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
    // THE ENGINE'S OWN POINTERS FIRST. They end at the vis state by
    // construction, so they cannot name a render copy -- and every one is
    // still checked against the validator's car before it is believed. The
    // scan below is the last resort, for a map whose chains do not resolve.
    // VET EVERY CANDIDATE IN THE PARENT FIRST. The whole batch is gathered into
    // one sample, so a single unreadable address takes the child down with it
    // and the batch reports "0 samples" -- which looks like "no candidate
    // tracks the car" and is nothing of the kind. Reading 40 bytes here is free
    // and cannot crash: `procmem::read_at` fails instead.
    let vet = |v: Vec<u64>| -> Vec<u64> {
        v.into_iter()
            .filter(|a| {
                procmem::read_at(srv.pid(), a.saturating_sub(16), 40)
                    .map(|b| {
                        let f = |o: usize| {
                            f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
                        };
                        // readable, and holding a position near the car rather
                        // than whatever was in a stale slot
                        (16..28).step_by(4).all(|o| f(o).is_finite())
                            && dist([f(16), f(20), f(24)], want) <= CAR_MATCH_M as f64
                    })
                    .unwrap_or(false)
            })
            .collect()
    };
    // THE ENGINE'S OWN POINTERS, IN ORDER, and the first that passes wins.
    //
    // Not ranked: ORDERED. `CAR_CHAINS` is shortest-first and that order is
    // what `fk trace`'s ladder has always used to reach 3 mm; ranking the
    // passing candidates by any measure of self-consistency instead picks a
    // different object on 126859 -- one exactly a tick out of phase, which
    // reads as 21 false positives out of 21 finishers. Two objects can both be
    // self-consistent; only one is the one every calibration was measured on.
    //
    // (`phy+0x848` is the same state by another route -- tm-player's INPUT arm --
    // but its position is a tick out on map 2 and the quaternion at pos-16 is
    // not unit there, so its internal layout is not the one these offsets
    // describe. Left alone rather than guessed at.)
    let mut hits = vet(chain_candidates(srv.pid()));
    let by_chain = hits.len();
    if hits.is_empty() {
        hits = vet(scan_near(srv.pid(), want, CAR_MATCH_M, srv.base.saturating_sub(603_616)));
    }
    if verbose {
        println!(
            "car {:#x} = ({:.3}, {:.3}, {:.3}); {} candidate(s) {} [{:.2}s]",
            chain.pos,
            want[0],
            want[1],
            want[2],
            hits.len(),
            if by_chain > 0 { "from the validator's own vehicle" } else { "from the chains or a scan" },
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
    // ONE CANDIDATE PER FORK, not seven.
    //
    // Batching them was a real speedup and a real bug: the whole batch is one
    // gather, so one candidate that diverges (or one address that goes bad
    // mid-run) decides the sample for all of them, and the failure reads as
    // "nothing tracks the car". There are three chain candidates, not sixty --
    // three forks is 0.1 s and each one judges exactly one object.
    let mut why: Vec<String> = Vec::new();
    let mut passed: Vec<(u64, f64)> = Vec::new();
    for ticks in [TRACK_TICKS, TRACK_TICKS * 4] {
        why.clear();
        for c in &cands {
            match tracks_the_car_batch_over(srv, probe, recs, &[*c], chain.pos, ticks) {
                Ok(mut v) => passed.append(&mut v),
                Err(e) => why.push(e),
            }
        }
        if !passed.is_empty() {
            break;
        }
    }
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
        return Ok(Layout { pos: *pos, clock, clock_bias: bias, rms: *d, max_dev: 0.0 });
    }
    Err(format!(
        "{} copies track the car and none has a race counter beside it",
        passed.len()
    ))
}

/// Judge candidates against the validator's own car over `track_ticks` ticks of
/// one simulation. Called with ONE candidate at a time: gathering several into
/// a single sample was faster and wrong -- one bad address decided the sample
/// for all of them.
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
    let mut segs: Vec<(u64, u32)> = cands.iter().map(|a| (*a - 16, 40u32)).collect();
    segs.push((truth, 12u32));
    let reclen = 40 * cands.len() + 12;
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
        return Err(format!(
            "only {} samples for {} candidates (blob {} bytes, child said {:?})",
            n,
            cands.len(),
            blob.len(),
            _j.chars().take(160).collect::<String>()
        ));
    }
    let xyz = |i: usize, k: usize| -> [f32; 3] {
        // candidate k's record is 40 bytes at (pos-16): q(16) pos(12) vel(12)
        let b = &blob[i * recsz + 8 + k * 40 + 16..];
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        [f(0), f(4), f(8)]
    };
    let vel = |i: usize, k: usize| -> [f32; 3] {
        let b = &blob[i * recsz + 8 + k * 40 + 28..];
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        [f(0), f(4), f(8)]
    };
    let quat = |i: usize, k: usize| -> [f32; 4] {
        let b = &blob[i * recsz + 8 + k * 40..];
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        [f(0), f(4), f(8), f(12)]
    };
    let truth_xyz = |i: usize| -> [f32; 3] {
        let b = &blob[i * recsz + 8 + cands.len() * 40..];
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        [f(0), f(4), f(8)]
    };
    let mut ok: Vec<(u64, f64)> = Vec::new();
    let mut why = String::new();
    for (k, addr) in cands.iter().enumerate() {
        let (mut worst, mut worst_bar) = (0.0f64, 0.0f64);
        let (mut cand, mut car) = (0.0f64, 0.0f64);
        // WHICH COPY: the vis state LAGS the validator's CGameVehiclePhy by one
        // tick, and the engine also keeps copies that hold the position and
        // NOTHING ELSE. Both track the car. Only one is the object every
        // consumer's offsets are calibrated on -- and taking the other is not a
        // near miss, it is a silent disaster: the render copy carries velocity
        // ZERO and a quaternion that is not one, so a speed predicate can never
        // fire and a search runs 5.8x faster finding nothing. (Measured, on
        // this build, at map 2 tick 171: 4 of 8 candidates tripped with the
        // real state, 0 of 8 with the copy.)
        //
        // So the test is the whole state, not the position: it must track the
        // car, lag it by a tick, carry a UNIT QUATERNION at -16, and carry a
        // velocity at +12 that is the derivative of its own position. A
        // position-only copy fails the last two by construction.
        let (mut d_now, mut d_prev) = (0.0f64, 0.0f64);
        let (mut verr, mut qerr) = (0.0f64, 0.0f64);
        for i in 1..n {
            let step = dist(truth_xyz(i - 1), truth_xyz(i));
            car += step;
            cand += dist(xyz(i - 1, k), xyz(i, k));
            let d = dist(xyz(i, k), truth_xyz(i));
            d_now += d;
            d_prev += dist(xyz(i, k), truth_xyz(i - 1));
            let bar = step + 0.05;
            if d > bar && d - bar > worst - worst_bar {
                worst = d;
                worst_bar = bar;
            }
            // the velocity must BE the derivative of the position it sits next
            // to: 100 m/s of travel in 10 ms is 1 m, so the residual is in m/s
            let v = vel(i, k);
            let dp = [
                (xyz(i, k)[0] - xyz(i - 1, k)[0]) as f64 * 100.0,
                (xyz(i, k)[1] - xyz(i - 1, k)[1]) as f64 * 100.0,
                (xyz(i, k)[2] - xyz(i - 1, k)[2]) as f64 * 100.0,
            ];
            verr += ((dp[0] - v[0] as f64).powi(2)
                + (dp[1] - v[1] as f64).powi(2)
                + (dp[2] - v[2] as f64).powi(2))
            .sqrt();
            let q = quat(i, k);
            let norm = ((q[0] as f64).powi(2)
                + (q[1] as f64).powi(2)
                + (q[2] as f64).powi(2)
                + (q[3] as f64).powi(2))
            .sqrt();
            // the WORST sample, not the average: a real attitude is unit to
            // float precision on every tick (measured |q|-1 p99.5 = 1.3e-7),
            // so an object that is unit on average and 0.14 off at the tail is
            // not an attitude, it is four floats that happen to sit there
            qerr = qerr.max((norm - 1.0).abs());
        }
        let m = (n - 1) as f64;
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
        if qerr > 1e-3 {
            why = format!("{:#x}: worst |q|-1 is {:.3e} -- no attitude here", addr, qerr);
            continue;
        }
        if verr / m > 2.0 {
            why = format!(
                "{:#x}: velocity at +12 is not d(pos)/dt ({:.1} m/s residual) -- a position-only \
                 copy, not the vis state",
                addr,
                verr / m
            );
            continue;
        }
        // THE LAG IS AN IDENTITY, NOT AN INEQUALITY.
        //
        // The vis state does not merely sit CLOSER to where the car was a tick
        // ago than to where it is now -- it holds exactly those numbers. Asking
        // only for "closer" accepted, on 126859, an object one tick off from
        // the right one: it passed every other test, `fk trace` accepted its
        // quaternion and velocity, and the trajectory came out 1.2147 m from
        // the reference -- one tick of travel at 128.9 m/s -- which the
        // watchdog then turned into 21 false positives out of 21 finishers.
        //
        // A tick of slack is 1.3 m at that speed and 0.03 m at the start line,
        // so a threshold in metres cannot be right either. The bar is
        // IDENTITY: 5 cm, which is float noise at any speed the game reaches.
        // NO PHASE TEST. The vis state lags the validator's car by a tick on
        // map 2 and does not on 126859, so "closer to where the car was" picks
        // the right object on one map and an object one tick off on the next --
        // which reads as a 1.2 m trajectory error and 21 false positives out of
        // 21 finishers. The chain is what identifies the object; the tests here
        // are what stop a WRONG chain (or a scan hit) being believed.
        let _ = (d_now, d_prev);
        // rank by how well the velocity matches its own derivative: among
        // objects that pass every test, that is the live state
        // rank by how well the velocity matches its own derivative
        ok.push((*addr, verr / m));
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

// -------------------------------------------------------------- the chains
//
// WHICH OBJECT, decided structurally rather than by phase.
//
// Scanning for a float triple near the car finds the car -- and also finds the
// engine's other copies of it, and they are not interchangeable. Two of them
// cost real measurements before this was understood:
//
// * a POSITION-ONLY render copy: same x/y/z, velocity zero, quaternion not
//   unit. Every speed predicate silently never fires on it (4 of 8 candidates
//   tripped with the real state, 0 of 8 with the copy) and a search runs 5.8x
//   faster finding nothing;
// * a copy ONE TICK off from the vis state. It passes the velocity and
//   quaternion checks -- `fk trace`'s own self-check accepts it -- and produces
//   a trajectory 1.2147 m from the reference on 126859, one tick of travel at
//   128.9 m/s, which the watchdog turned into 21 false positives out of 21
//   finishers.
//
// The second one cannot be separated by asking how it sits relative to the
// validator's own car: the phase between the two is NOT the same on every map
// (the vis state lags by a tick on map 2 and does not on 126859), so any rule
// of the form "closer to where the car was" picks the right object on one map
// and the wrong one on the next.
//
// What DOES identify it is the engine's own pointer: these chains end at the
// vis state by construction, they were derived per build with `fk ptr find`,
// and every one of them is checked here against the validator's car before it
// is believed. The scan stays as the last resort for a map whose chains have
// not been derived, and it is now the only thing that can pick a copy.

/// Every chain that reaches a vehicle vis state on build 128182, shortest
/// first. Kept in step with `fk::ptr::CAR_CHAINS`, which is where they are
/// derived; the tracking test below is what decides between them per run.
pub const CAR_CHAINS: &[&str] = &[
    "mod+0x1d56e48:0:+0xd8:+0x4e8",
    "mod+0x1d56e48:0:+0x68:+0x8:+0x4e8",
    "mod+0x1d56e50:0:+0x10:+0x28:+0x4e8",
    "mod+0x1cba348:0:+0x238:+0x140:+0x298:+0x4e8",
    "mod+0x1d58ef0:0:+0x360:+0x48:+0x3c8:+0x4e8",
    "mod+0x1e45148:0:+0x198:+0x38:+0x48:+0x4e8",
    "mod+0x1e59460:0:+0x180:+0x328:+0x328:+0x4e8",
    "mod+0x1cba348:0:+0x2d8:+0x208:+0xc0:+0x6268",
    "mod+0x1d56e48:0:+0x158:+0x6268",
    "mod+0x1d58ef0:0:+0x360:+0x48:+0x258:+0x6268",
];

/// The main module's load address, from `/proc/<pid>/maps`.
fn module_base(pid: i32) -> Option<u64> {
    let s = std::fs::read_to_string(format!("/proc/{}/maps", pid)).ok()?;
    let mut best: Option<(u64, &str)> = None;
    for l in s.lines() {
        let mut it = l.split_whitespace();
        let range = it.next()?;
        let _perms = it.next()?;
        let _off = it.next()?;
        let _dev = it.next()?;
        let _inode = it.next()?;
        let path = it.next().unwrap_or("");
        if path.ends_with("TrackmaniaServer") {
            let start = u64::from_str_radix(range.split('-').next()?, 16).ok()?;
            if best.map(|(b, _)| start < b).unwrap_or(true) {
                best = Some((start, path));
            }
        }
    }
    best.map(|(b, _)| b)
}

/// Walk `mod+0xROOT:0:+0xA:+0xB` — dereference every hop but the last, which
/// is arithmetic.
fn resolve_chain(pid: i32, module: u64, spec: &str) -> Option<u64> {
    let mut it = spec.split(':');
    let root = it.next()?;
    let hex = |s: &str| -> Option<i64> {
        let (neg, s) = match s.as_bytes().first() {
            Some(b'+') => (false, &s[1..]),
            Some(b'-') => (true, &s[1..]),
            _ => (false, s),
        };
        let v = i64::from_str_radix(s.trim_start_matches("0x"), 16).ok()?;
        Some(if neg { -v } else { v })
    };
    let mut a = (module as i64 + hex(root.strip_prefix("mod")?)?) as u64;
    let parts: Vec<&str> = it.collect();
    for (i, p) in parts.iter().enumerate() {
        let o = hex(p)?;
        if i + 1 == parts.len() {
            return Some((a as i64 + o) as u64);
        }
        let at = (a as i64 + o) as u64;
        let b = procmem::read_at(pid, at, 8)?;
        a = u64::from_le_bytes(b[..8].try_into().ok()?);
        if a < 0x1000 {
            return None;
        }
    }
    None
}

/// Every vis state the engine's own pointers reach in this process.
fn chain_candidates(pid: i32) -> Vec<u64> {
    let Some(m) = module_base(pid) else {
        return Vec::new();
    };
    let mut out: Vec<u64> = CAR_CHAINS
        .iter()
        .filter_map(|c| resolve_chain(pid, m, c))
        .map(|s| s + POS_IN_VIS_STATE)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The position's offset inside the vis state the chains end at.
const POS_IN_VIS_STATE: u64 = 0x50;
