//! Where the car's state lives in one server process, as the sampler and every
//! consumer of a sampled record see it.
//!
//! The addresses come from `car::locate` -- the dyna body record the physics
//! step integrates, stamped with the tick loop's own clock -- and nothing here
//! searches for anything. What stays here is the RECORD: the byte layout of one
//! gathered sample, how it decodes into rows, and the whole-run self-checks.

use crate::forksrv::Rec;

/// Where the car's state lives in one particular server process.
///
/// Every address is derived (`car::locate`), and the label is the engine's:
/// `clock` is `sim+0x48`, the simulation time the tick loop writes at the end
/// of every tick, and `clock_bias` is the race start it set in this process,
/// so `clock - clock_bias` is the race time of the state a sample holds.
#[derive(Clone, Debug)]
pub struct Layout {
    /// f32 x,y,z -- the body record's position.
    pub pos: u64,
    /// f32 w,x,y,z -- the body record's attitude.
    pub quat: u64,
    /// f32 vx,vy,vz -- the body record's linear velocity.
    pub vel: u64,
    /// f32 tyre wetness, 0..1 -- `WetnessValue01` of the post-step vis state.
    pub wet: u64,
    /// u32 simulation time of the last finished tick, `sim+0x48`.
    pub clock: u64,
    /// `clock_value - race_ms` of the state a sample holds: the race start.
    pub clock_bias: i64,
    /// Deviation of the located position from a reference, when one was
    /// measured; 0 for a derived layout.
    pub rms: f64,
    pub max_dev: f64,
    /// The engine's own checkpoint counter (u32) at `participant + 0xc70`, or 0
    /// when not carried. Located behaviourally from three real ghosts' split
    /// times (tmenv cpfind, 2026-09-06): it steps at exactly the tick the
    /// validator credits a checkpoint, the finish included; agreement with the
    /// plain oracle 200/200 tapes over 5 containers.
    pub cps: u64,
    /// The driven vehicle's post-step `CSceneVehicleVisState` (`phy + 0x848`,
    /// 0x360 bytes: gear, rpm, wheels, turbo, applied steer -- WHEELS.md), or 0
    /// when not carried. See [`Vis`].
    pub vis: u64,
    /// The driven vehicle's KIND (0 Stadium, 1 Snow, 2 Rally, 3 Desert) from the model
    /// fingerprint (`car::KIND_WORD_IN_PHY`); the slot index when the fingerprint is unknown.
    pub car: u8,
    /// The participant slot the driven vehicle came from (0..3) -- not a kind.
    pub car_slot: u8,
}

/// Offsets within the gathered record, once the segments are concatenated.
pub const R_CLOCK: usize = 0;
pub const R_QUAT: usize = 4; // qw qx qy qz
pub const R_POS: usize = 20; // x y z
pub const R_VEL: usize = 32; // vx vy vz
pub const R_WET: usize = 44; // f32 tyre wetness, 0..1
pub const REC_LEN: usize = 48;

/// The segments the production sampler gathers, in record order: the clock,
/// the attitude, the position, the velocity, the wetness word. Five segments
/// because the body record keeps its quaternion AFTER the position (`+0x30` vs
/// `+0x24`) while the record every consumer decodes puts it first; the sampler
/// concatenates, so the consumers never learned the difference.
pub fn segments(l: &Layout) -> Vec<(u64, u32)> {
    let mut v = vec![(l.clock, 4), (l.quat, 16), (l.pos, 12), (l.vel, 12), (l.wet, 4)];
    if l.cps != 0 {
        v.push((l.cps, 4));
    }
    if l.vis != 0 {
        v.push((l.vis, VIS_LEN as u32));
    }
    v
}

/// u32 checkpoint counter, present only when `Layout::cps != 0`.
pub const R_CPS: usize = REC_LEN;
/// The gathered record length for this layout.
pub fn rec_len(l: &Layout) -> usize {
    (if l.cps != 0 { REC_LEN + 4 } else { REC_LEN }) + if l.vis != 0 { VIS_LEN } else { 0 }
}
/// Where the vis segment starts in the record: after the counter, when present.
pub fn r_vis(l: &Layout) -> usize {
    if l.cps != 0 { R_CPS + 4 } else { REC_LEN }
}

fn getf32(b: &[u8], o: usize) -> f64 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as f64
}

/// One extracted tick.
///
/// `Copy` because the environment carries it as plain state through a hot loop
/// and every scalar in it is a machine word.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub time_ms: i64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub qx: f64,
    pub qy: f64,
    pub qz: f64,
    pub qw: f64,
    /// Tyre wetness, 0..1.
    pub wetness: f64,
    /// The engine's checkpoint counter at this tick; `u32::MAX` when the
    /// layout carries none.
    pub cps: u32,
    /// The vis state (gear, rpm, wheels, ...); `Vis::UNKNOWN` when the layout
    /// carries none.
    pub vis: Vis,
}

/// Decode a gathered sample blob into one row per tick.
///
/// The record is keyed on its whole content, so the engine may emit several
/// samples inside one tick; the last one carries that tick's finished state.
/// The clock makes the stream self-timing: a missing or duplicated tick shows
/// up as a gap rather than silently shifting everything after it.
pub fn decode_rows(blob: &[u8], l: &Layout, label_shift: i64) -> (Vec<Row>, Vec<String>) {
    let rl = rec_len(l);
    let recsz = 8 + rl;
    let m = blob.len() / recsz;
    let mut rows: Vec<Row> = Vec::new();
    let mut warn = Vec::new();
    for i in 0..m {
        let b = &blob[i * recsz + 8..i * recsz + 8 + rl];
        let clk = u32::from_le_bytes(b[R_CLOCK..R_CLOCK + 4].try_into().unwrap()) as i64;
        let t = clk - l.clock_bias + label_shift;
        let row = Row {
            time_ms: t,
            x: getf32(b, R_POS),
            y: getf32(b, R_POS + 4),
            z: getf32(b, R_POS + 8),
            vx: getf32(b, R_VEL),
            vy: getf32(b, R_VEL + 4),
            vz: getf32(b, R_VEL + 8),
            qw: getf32(b, R_QUAT),
            qx: getf32(b, R_QUAT + 4),
            qy: getf32(b, R_QUAT + 8),
            qz: getf32(b, R_QUAT + 12),
            wetness: getf32(b, R_WET),
            cps: if l.cps != 0 { u32::from_le_bytes(b[R_CPS..R_CPS + 4].try_into().unwrap()) } else { u32::MAX },
            vis: if l.vis != 0 { Vis::decode(&b[r_vis(l)..r_vis(l) + VIS_LEN], l.car, l.car_slot) } else { Vis::UNKNOWN },
        };
        match rows.last_mut() {
            Some(last) if last.time_ms == t => *last = row,
            _ => rows.push(row),
        }
    }
    for w in rows.windows(2) {
        if w[1].time_ms - w[0].time_ms != 10 {
            warn.push(format!(
                "clock gap: {} -> {} ms",
                w[0].time_ms, w[1].time_ms
            ));
        }
    }
    (rows, warn)
}

/// Race time of sample `i` of a stream started at boundary tick `probe`.
///
/// Sample 0 is the state at the end of tick `probe - 1`: the resume rewrites
/// tick `probe` onwards, so the first state the child reports is the one the
/// prefix left behind.
pub fn sample_ms(probe: usize, i: usize, start_offset_ms: i32) -> i64 {
    (probe as i64 - 1 + i as i64) * 10 + start_offset_ms as i64
}

/// One tick of input for every tape tick from `from` to the end.
pub fn tail_recs(steer: &[u8], accel: &[u8], brake: &[u8], from: usize) -> Vec<Rec> {
    (from..steer.len())
        .map(|t| crate::forksrv::rec_of(steer[t], accel[t], brake[t]))
        .collect()
}

// --------------------------------------------------------------- self-checks
//
// Two questions no measurement of a simulated trajectory should be trusted
// without, both answered from data the run already produced:
//
//   1. Is the simulator running the tape I asked about?
//   2. Is the thing I read out of it the car?
//
// Question 1 sounds impossible to get wrong and was, in production, wrong 17%
// of the time: two `fk btraj` processes sharing a work directory swap replays,
// so one of them measures the OTHER tape's prefix with its own tail patched in.
// The result is a genuine, self-consistent trajectory of a car that drove
// somewhere else -- no internal consistency test can see it, because nothing
// about it is inconsistent. Only comparing against the tape itself can.

/// THE IDENTITY CONTROL: the decoded input array in the server's memory must
/// be, tick for tick, the tape we mean to measure.
///
/// `base` is the array the shim located and reported at handshake; the layout
/// is one 32-byte record per tick: `+4` steer, `+8` gas, `+12` brake as f32
/// (`forkoracle::forksrv::STRIDE` documents the whole record).
/// Reading it back through /proc/<pid>/mem costs one 70 KB read and settles the
/// question completely.
pub fn verify_tape(
    pid: i32,
    base: u64,
    steer: &[u8],
    accel: &[u8],
    brake: &[u8],
) -> Result<(), String> {
    let n = steer.len();
    let buf = crate::procmem::read_at(pid, base, n * crate::forksrv::STRIDE)
        .ok_or_else(|| format!("tape check: cannot read {} bytes at {:#x} of pid {}", n * 32, base, pid))?;
    let mut bad = 0usize;
    let mut first = String::new();
    for t in 0..n {
        let o = t * crate::forksrv::STRIDE;
        let g = |k: usize| f32::from_le_bytes(buf[o + k..o + k + 4].try_into().unwrap());
        let (st, ga, br) = (
            g(crate::forksrv::REC_STEER),
            g(crate::forksrv::REC_GAS),
            g(crate::forksrv::REC_BRAKE),
        );
        let want = crate::forksrv::rec_of(steer[t], accel[t], brake[t]);
        if st != want.steer || ga != want.gas || br != want.brake {
            if bad == 0 {
                first = format!(
                    "tick {}: server has ({}, {}, {}), tape says ({}, {}, {})",
                    t, st, ga, br, want.steer, want.gas, want.brake
                );
            }
            bad += 1;
        }
    }
    if bad > 0 {
        return Err(format!(
            "TAPE MISMATCH: {} of {} ticks differ -- the simulator is not running the tape \
             that was asked for (first difference: {}). This is what a shared work directory \
             does; give every run its own --work.",
            bad, n, first
        ));
    }
    Ok(())
}

/// What a whole-run self-check found. All of it is measured over every row of
/// the extracted trajectory, not the 150-sample window the locator used.
#[derive(Debug, Clone)]
pub struct RowCheck {
    pub rows: usize,
    pub quat_err: f64,
    pub vel_err: f64,
    pub gaps: usize,
    pub mean_speed: f64,
}

impl std::fmt::Display for RowCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} rows, |q|-1 p99.5 {:.2e}, |d(pos)/dt - v| median {:.3} m/s, {} clock gaps, \
             mean speed {:.1} m/s",
            self.rows, self.quat_err, self.vel_err, self.gaps, self.mean_speed
        )
    }
}

/// QUESTION 2, over the whole run instead of a 150-tick window.
///
/// Three independent things must hold if the rows are the vehicle state:
/// the quaternion is a UNIT quaternion (a structural property of the struct,
/// nothing to do with the velocity test that selected the slot), the position
/// derivative matches the velocity triple, and the clock advances by exactly
/// one tick per row. Two of the three are independent of the signature the
/// locator searched on, which is the point: agreement between independent
/// tests is what makes a reference-free measurement trustworthy.
pub fn check_rows(rows: &[Row]) -> Result<RowCheck, String> {
    if rows.len() < 50 {
        return Err(format!("only {} rows extracted", rows.len()));
    }
    let mut qs: Vec<f64> = Vec::with_capacity(rows.len());
    for r in rows {
        let n = (r.qw * r.qw + r.qx * r.qx + r.qy * r.qy + r.qz * r.qz).sqrt();
        qs.push((n - 1.0).abs());
    }
    qs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // The 99.5th percentile, not the max: one row of a respawn transition is
    // not evidence about the layout, and a record with 31 respawns has 31 of
    // them.
    let qmax: f64 = qs[((qs.len() as f64 - 1.0) * 0.995) as usize];
    let mut verrs: Vec<f64> = Vec::with_capacity(rows.len());
    let mut speed = 0.0;
    let mut n = 0usize;
    let mut gaps = 0usize;
    for w in rows.windows(2) {
        let dt = (w[1].time_ms - w[0].time_ms) as f64 / 1000.0;
        if (w[1].time_ms - w[0].time_ms) != 10 {
            gaps += 1;
            continue;
        }
        let (dx, dy, dz) = (w[1].x - w[0].x, w[1].y - w[0].y, w[1].z - w[0].z);
        verrs.push(
            ((dx / dt - w[0].vx).powi(2) + (dy / dt - w[0].vy).powi(2) + (dz / dt - w[0].vz).powi(2))
                .sqrt(),
        );
        speed += (dx * dx + dy * dy + dz * dz).sqrt() / dt;
        n += 1;
    }
    // MEDIAN, not mean. A respawn moves the car tens of metres in one tick and
    // the mean of |d(pos)/dt - v| over a 31-respawn record is 16 m/s while the
    // typical row is 0.1 -- the mean condemns a perfect measurement.
    verrs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let vmed = if verrs.is_empty() {
        f64::MAX
    } else {
        verrs[verrs.len() / 2]
    };
    let c = RowCheck {
        rows: rows.len(),
        quat_err: qmax,
        vel_err: vmed,
        gaps,
        mean_speed: if n > 0 { speed / n as f64 } else { 0.0 },
    };
    // Thresholds, all with two orders of magnitude of headroom against
    // measured good runs (|q|-1 ~ 1e-7, vel_err ~ 0.1 m/s, 0 gaps). The
    // velocity bound is RELATIVE to the car's own speed: a fixed 2.0 m/s was
    // calibrated on a 90 m/s car and means nothing on a 30 or a 300 m/s one.
    if c.quat_err > 1e-3 {
        return Err(format!("not a unit quaternion (p99.5 |q|-1 = {:.3e}): {}", c.quat_err, c));
    }
    // floor 1.0 m/s (ENV velcheck 2026-09-07: the residual is the solver's per-tick contact-projection
    // correction, ~1.5 mm/tick on tarmac and ~6 mm/tick on a bouncing Rally car = 0.6 m/s at 100 Hz)
    if c.vel_err > (0.02 * c.mean_speed).max(1.0) {
        return Err(format!("position derivative disagrees with the velocity triple: {}", c));
    }
    if c.gaps * 200 > c.rows {
        return Err(format!("clock is not advancing one tick per row: {}", c));
    }
    if c.mean_speed < 1.0 {
        return Err(format!("the car never moves: {}", c));
    }
    Ok(c)
}
/// The engine's `CSceneVehicleVisState` (0x360 bytes at `phy + 0x848`) --
/// gear, rpm, wheels, turbo, applied steer -- decoded per the INPUT arm's
/// `WHEELS.md` §2 (2026-09-06; `fk wheels` reproduces the ghost's own 50 ms
/// samples from it byte for byte: gear/rpm/steer/dampers/contact 100 %,
/// materials 98-100 %, on 7 ghosts × 6 maps × all 4 cars). Wheel order is the
/// ENGINE's: k = 0..3 = FL, FR, RR, RL.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vis {
    /// False when the layout carries no vis segment: every field below is
    /// then meaningless and a consumer must say UNKNOWN, not zero.
    pub known: bool,
    /// The driven vehicle's KIND: 0 Stadium, 1 Snow, 2 Rally, 3 Desert (the model fingerprint;
    /// `Layout::car`) -- a whole-map Rally map reads 2 in slot 0.
    pub car: u8,
    /// The participant slot the vehicle came from (0..3).
    pub car_slot: u8,
    pub gear: u8,
    pub rpm: f32,
    /// The steer the engine applied, after the action-key cap, -1..1.
    pub steer_applied: f32,
    pub gas: f32,
    pub braking: bool,
    pub front_speed: f32,
    pub lateral_speed: f32,
    pub turbo_time: f32,
    pub is_turbo: bool,
    pub ground_contact: bool,
    /// Reactor boost level (0 none, 1, 2) and type (0 none, 1 down, 2 up) -- the INPUT arm's
    /// EFFECTS.md (u32 at vis +0x174 / +0x178, 100 % against ghost telemetry b89 bits 5-6 / 3-4);
    /// decoded from the vis block already gathered (no new read). u8::MAX when unknown.
    pub reactor_lvl: u8,
    pub reactor_type: u8,
    /// IsReactorGroundMode (flags bit 19), ReactorInputsX (bit 18), the boost enum (u32 +0x19c & 7),
    /// reactor air control (+0x180..), simulation time coefficient (+0x230; slow-motion) -- EFFECTS.md.
    pub reactor_ground_mode: bool,
    pub reactor_inputs_x: bool,
    pub boost_enum: u8,
    pub reactor_air: [f32; 3],
    pub sim_time_coef: f32,
    pub wheel_contact: [bool; 4],
    pub wheel_material: [u8; 4],
    pub wheel_slip: [f32; 4],
    pub wheel_damper: [f32; 4],
    pub wheel_steer: [f32; 4],
    pub wetness: f32,
}

impl Vis {
    pub const UNKNOWN: Vis = Vis {
        known: false,
        car: u8::MAX,
        car_slot: u8::MAX,
        gear: u8::MAX,
        rpm: f32::NAN,
        steer_applied: f32::NAN,
        gas: f32::NAN,
        braking: false,
        front_speed: f32::NAN,
        lateral_speed: f32::NAN,
        turbo_time: f32::NAN,
        is_turbo: false,
        reactor_lvl: u8::MAX,
        reactor_type: u8::MAX,
        reactor_ground_mode: false,
        reactor_inputs_x: false,
        boost_enum: u8::MAX,
        reactor_air: [f32::NAN; 3],
        sim_time_coef: f32::NAN,
        ground_contact: false,
        wheel_contact: [false; 4],
        wheel_material: [u8::MAX; 4],
        wheel_slip: [f32::NAN; 4],
        wheel_damper: [f32::NAN; 4],
        wheel_steer: [f32::NAN; 4],
        wetness: f32::NAN,
    };

    /// Decode a gathered 0x360-byte vis state.
    pub fn decode(s: &[u8], car: u8, car_slot: u8) -> Vis {
        let f = |o: usize| f32::from_le_bytes(s[o..o + 4].try_into().unwrap());
        let u = |o: usize| u32::from_le_bytes(s[o..o + 4].try_into().unwrap());
        let flags = u(0x88);
        let mut v = Vis {
            known: true,
            car,
            car_slot,
            gear: (u(0x1a4) & 0xf) as u8,
            rpm: f(0x198),
            steer_applied: f(0x10),
            gas: f(0x14),
            braking: u(0x20) != 0,
            front_speed: f(0x74),
            lateral_speed: f(0x78),
            turbo_time: f(0x1ac),
            is_turbo: flags & (1 << 24) != 0,
            ground_contact: flags & (1 << 20) != 0,
            reactor_lvl: u(0x174).min(3) as u8,
            reactor_type: u(0x178).min(3) as u8,
            reactor_ground_mode: flags & (1 << 19) != 0,
            reactor_inputs_x: flags & (1 << 18) != 0,
            boost_enum: (u(0x19c) & 7) as u8,
            reactor_air: [f(0x180), f(0x184), f(0x188)],
            sim_time_coef: f(0x230),
            wheel_contact: [false; 4],
            wheel_material: [0; 4],
            wheel_slip: [0.0; 4],
            wheel_damper: [0.0; 4],
            wheel_steer: [0.0; 4],
            wetness: f(0x328),
        };
        for k in 0..4 {
            let w = 0xa8 + 44 * k;
            let wf = u(w + 0x28);
            v.wheel_contact[k] = wf & 2 == 0;
            v.wheel_material[k] = s[w + 0x10];
            v.wheel_slip[k] = f(w + 0x14);
            v.wheel_damper[k] = f(w);
            v.wheel_steer[k] = f(w + 0x0c);
        }
        v
    }
}

/// Size of the vis state segment.
pub const VIS_LEN: usize = 0x360;
