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
    /// The driven vehicle's four phy WHEEL BLOCKS (`phy + 0x1780`, 4 × 0xb8: damper, contact point, live contact flag,
    /// material, contact normal — ENV 2026-09-10), or 0 when not carried. Decoded into `Vis::wheel_*`.
    pub wheels: u64,
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
    if l.wheels != 0 {
        v.push((l.wheels, WHEELS_LEN as u32));
    }
    v
}

/// u32 checkpoint counter, present only when `Layout::cps != 0`.
pub const R_CPS: usize = REC_LEN;
/// The gathered record length for this layout.
pub fn rec_len(l: &Layout) -> usize {
    (if l.cps != 0 { REC_LEN + 4 } else { REC_LEN }) + if l.vis != 0 { VIS_LEN } else { 0 } + if l.wheels != 0 { WHEELS_LEN } else { 0 }
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
            vis: {
                let mut v = if l.vis != 0 { Vis::decode(&b[r_vis(l)..r_vis(l) + VIS_LEN], l.car, l.car_slot) } else { Vis::UNKNOWN };
                if l.wheels != 0 {
                    v.decode_wheels(&b[r_wheels(l)..r_wheels(l) + WHEELS_LEN]);
                }
                v
            },
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
        // the record holds the REAL brake; a respawn rides the wire as brake + 2.0 and lands in word 0, so compare
        // against the brake value the wire encoding stands for (2026-09-09)
        if st != want.steer || ga != want.gas || br != want.brake_value() {
            if bad == 0 {
                first = format!(
                    "tick {}: server has ({}, {}, {}), tape says ({}, {}, {})",
                    t, st, ga, br, want.steer, want.gas, want.brake_value()
                );
            }
            bad += 1;
        }
    }
    if bad > 0 {
        // A CONSTANT STEER PREFIX IS THE OTHER CAUSE, and it looks exactly like
        // this: the input locator keys on the tape's steer sequence, so a
        // synthesised tape with steer 0 everywhere matches a zero page and
        // every record reads (0, 0, 0) -- measured 2026-09-29 on a full-gas
        // straight seed, 1000 of 1000 ticks "differ". Name it before the
        // work-directory story, which is the rarer of the two.
        let distinct: std::collections::BTreeSet<u8> = steer.iter().take(200.min(steer.len())).copied().collect();
        let all_zero = buf.chunks(crate::forksrv::STRIDE).take(n).all(|c| c.iter().all(|b| *b == 0));
        if distinct.len() < 2 && all_zero {
            return Err(format!(
                "TAPE MISMATCH: {} of {} ticks differ, and the server-side records read back as all zero while the tape's \
                 first {} ticks hold a single steer value ({:?}): the input locator cannot key on a constant steer prefix. \
                 Give the seed a distinct prefix -- `tmauto synth write` does so by default (--wobble-prefix 25, a \
                 zero-mean +-12 steer key over the first 0.25 s) -- or `ghost tape poke` one in.",
                bad, n, 200.min(steer.len()), distinct.iter().next().copied().unwrap_or(0)
            ));
        }
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
        // RESPAWN HOLD (ENV 2026-09-09): for ~1.0 s after a respawn the readout is frozen -- the position does not move
        // while the velocity triple keeps the re-placed car's speed -- so d(pos)/dt - v == |v| on every held row, and a
        // window forked inside the hold read a median of 16-27 m/s on a correct slot (Poland, INPUT). A held row is not
        // evidence about the layout: skip it.
        if dx == 0.0 && dy == 0.0 && dz == 0.0 && (w[0].vx * w[0].vx + w[0].vy * w[0].vy + w[0].vz * w[0].vz) > 1.0 {
            continue;
        }
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
    // Floor 1.0 m/s: a standing-start car at full gas reads 0.55-0.8 m/s here over
    // its first 2 s (Bear's Valley, A Crumpled Up Piece of Paper, Never Odd or
    // Even, Against the Current -- all identity-exact through the env), and the
    // S01 WR at 95 m/s reads 0.5; a wrong slot reads tens. (ENV 2026-09-07)
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
    pub wheel_contact: [bool; 4],
    pub wheel_material: [u8; 4],
    pub wheel_slip: [f32; 4],
    pub wheel_damper: [f32; 4],
    pub wheel_steer: [f32; 4],
    pub wetness: f32,
    // --- effects (INPUT arm, EFFECTS.md, 2026-09-07; verified 100 % against ghost
    // samples on the Summer 2026 - 07 reactor run and the 18/20 reset/down runs) ---
    /// The raw flags word at +0x88 (bits 18/19/20/24 named below; 4,6,7,8,9,10,
    /// 12,17 unidentified -- likely no-engine/cruise/fragile).
    pub flags_raw: u32,
    /// Boost enum, u32(+0x19c) & 7.
    pub boost_enum: u8,
    /// Reactor boost level, u32(+0x174) & 3 (0 none, 1, 2).
    pub reactor_lvl: u8,
    /// Reactor type, u32(+0x178) & 3: 1 down, 2 up.
    pub reactor_type: u8,
    /// flags bit 19.
    pub reactor_ground_mode: bool,
    /// flags bit 18.
    pub reactor_inputs_x: bool,
    /// Reactor air control, f32 x3 at +0x180.
    pub reactor_air: [f32; 3],
    /// Simulation time coefficient (slow-motion), f32 +0x230; 1.0 normally.
    pub sim_time_coef: f32,
    // --- the phy WHEEL BLOCKS (phy+0x1780 + 0xb8·k; ENV 2026-09-10 21:18Z, identity car with one wheel lifted), gathered as
    // the layout's `wheels` segment; `wheel_live` u8::MAX when the layout does not carry it ---
    /// u32 at +0x30 of each block: 1 = the wheel touches now (the vis `wheel_contact` bit is NOT this — ENV).
    pub wheel_live: [u8; 4],
    /// +0x44..0x4c: the contact normal in the CAR'S LOCAL frame (unit; zero while the wheel is in the air). Local, not
    /// world: on 20's ramp climb (7.2 s, y rising) it still reads (0.00, 1.00, −0.02), and GEN's four-wheel ice-wall ride
    /// read (0, 1, 0.02) — a world normal would be near-horizontal there (coordinator 22:30Z).
    pub wheel_normal: [[f32; 3]; 4],
    /// +0x00: damper length, 0.200 = fully extended (airborne).
    pub wheel_damper_phy: [f32; 4],
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
        ground_contact: false,
        wheel_contact: [false; 4],
        wheel_material: [u8::MAX; 4],
        wheel_slip: [f32::NAN; 4],
        wheel_damper: [f32::NAN; 4],
        wheel_steer: [f32::NAN; 4],
        wetness: f32::NAN,
        flags_raw: 0,
        boost_enum: u8::MAX,
        reactor_lvl: u8::MAX,
        reactor_type: u8::MAX,
        reactor_ground_mode: false,
        reactor_inputs_x: false,
        reactor_air: [f32::NAN; 3],
        sim_time_coef: f32::NAN,
        wheel_live: [u8::MAX; 4],
        wheel_normal: [[f32::NAN; 3]; 4],
        wheel_damper_phy: [f32::NAN; 4],
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
            wheel_contact: [false; 4],
            wheel_material: [0; 4],
            wheel_slip: [0.0; 4],
            wheel_damper: [0.0; 4],
            wheel_steer: [0.0; 4],
            wetness: f(0x328),
            flags_raw: flags,
            boost_enum: (u(0x19c) & 7) as u8,
            reactor_lvl: (u(0x174) & 3) as u8,
            reactor_type: (u(0x178) & 3) as u8,
            reactor_ground_mode: flags & (1 << 19) != 0,
            reactor_inputs_x: flags & (1 << 18) != 0,
            reactor_air: [f(0x180), f(0x184), f(0x188)],
            sim_time_coef: f(0x230),
            wheel_live: [u8::MAX; 4],
            wheel_normal: [[f32::NAN; 3]; 4],
            wheel_damper_phy: [f32::NAN; 4],
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

/// Size of the wheel-blocks segment: four `CGameVehiclePhy` wheel blocks of 0xb8 at `phy + 0x1780`.
pub const WHEELS_LEN: usize = 4 * 0xb8;
pub const WHEEL_BLOCK: usize = 0xb8;
/// Where the wheel-blocks segment starts in the record: after the vis segment.
pub fn r_wheels(l: &Layout) -> usize {
    r_vis(l) + if l.vis != 0 { VIS_LEN } else { 0 }
}

impl Vis {
    /// Fill the wheel-block fields from a gathered `WHEELS_LEN` segment.
    pub fn decode_wheels(&mut self, s: &[u8]) {
        let f = |o: usize| f32::from_le_bytes(s[o..o + 4].try_into().unwrap());
        for k in 0..4 {
            let b = k * WHEEL_BLOCK;
            self.wheel_live[k] = (u32::from_le_bytes(s[b + 0x30..b + 0x34].try_into().unwrap()) != 0) as u8;
            self.wheel_normal[k] = [f(b + 0x44), f(b + 0x48), f(b + 0x4c)];
            self.wheel_damper_phy[k] = f(b);
        }
    }
    /// Surface-relative tilt per wheel: the angle (degrees) between the body's up axis and the wheel's contact normal
    /// while the wheel touches. The normal is in the car's LOCAL frame, so this is acos(n.y) — no quaternion needed (the
    /// arguments are kept for the callers written against the earlier signature; they are ignored). NaN in the air, when
    /// the normal is not yet written (the flag leads it by a tick), or when the layout carries no wheel blocks. The ratified
    /// attitude rule: ≥ 45° while in contact = illegal.
    pub fn wheel_tilt_deg(&self, _qw: f64, _qx: f64, _qy: f64, _qz: f64) -> [f64; 4] {
        self.wheel_tilt()
    }
    pub fn wheel_tilt(&self) -> [f64; 4] {
        let up = [0.0f64, 1.0, 0.0];
        let mut out = [f64::NAN; 4];
        for k in 0..4 {
            if self.wheel_live[k] != 1 {
                continue;
            }
            let n = self.wheel_normal[k];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if !(len > 0.5) {
                continue;
            }
            let dot = (up[0] * n[0] as f64 + up[1] * n[1] as f64 + up[2] * n[2] as f64).clamp(-1.0, 1.0);
            out[k] = dot.acos().to_degrees();
        }
        out
    }
}

impl Vis {
    /// The wheel's contact normal rotated into the WORLD frame by the body quaternion (w, x, y, z).
    pub fn wheel_normal_world(&self, k: usize, qw: f64, qx: f64, qy: f64, qz: f64) -> [f64; 3] {
        let n = self.wheel_normal[k];
        let (vx, vy, vz) = (n[0] as f64, n[1] as f64, n[2] as f64);
        // v' = v + 2 w (q × v) + 2 q × (q × v)
        let (cx, cy, cz) = (qy * vz - qz * vy, qz * vx - qx * vz, qx * vy - qy * vx);
        let (dx, dy, dz) = (qy * cz - qz * cy, qz * cx - qx * cz, qx * cy - qy * cx);
        [vx + 2.0 * (qw * cx + dx), vy + 2.0 * (qw * cy + dy), vz + 2.0 * (qw * cz + dz)]
    }
}
