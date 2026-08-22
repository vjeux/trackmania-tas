//! THE IDENTITY CONTROL, in gate mode.
//!
//! # What it replaces, and why it is stronger
//!
//! Every fork search asks one question before it trusts a server: *is this
//! process simulating the tape I think it is?* Outside gate mode the answer is
//! a millisecond -- run the seed, get the time back, compare. In gate mode that
//! is not available: the seed is normally aborted by a predicate long before
//! the finish, and on the map this feature was proven on the seed does not go
//! anywhere near the thing being hunted.
//!
//! The replacement is better. The fork's measured gate state for the SEED is
//! compared against the seed's own recorded telemetry at the same instant --
//! position, velocity **and** the quaternion. One comparison validates the
//! record layout, the car locator, the clock labelling, the box arithmetic and
//! the key, against a recording the engine itself wrote. A millisecond check
//! validates none of those individually: it can pass with the position read out
//! of the wrong object, because the time comes from the validator and not from
//! the memory the gate reads.
//!
//! It must run on the SEED. Reading an incumbent out of the bank instead
//! compares this server's answer against the expected value for a tape that
//! another worker has already improved, and about half of one fleet aborted on
//! a control that was testing the wrong tape.
//!
//! # The tolerance is derived from the data, not chosen
//!
//! The telemetry is on a 50 ms grid and the gate is measured per 10 ms tick, so
//! the comparison interpolates between the two bracketing samples. The bar for
//! each quantity is therefore *a floor, plus a quarter of what that quantity
//! changed across the sample interval the state falls in* -- generous against
//! interpolation error, and still nowhere near the metres and tens of degrees a
//! wrong object in memory is wrong by. All six numbers are printed either way,
//! so a reader can see how much room the answer had rather than only that it
//! passed.

use forkoracle::pred::GateRecord;
use forkoracle::pred_core::{key_eval, Gate};

/// How the fork's measurement compares with the recording.
pub struct Agreement {
    pub race_ms: i64,
    pub measured: GateRecord,
    pub expected: GateRecord,
    pub pos_err: f64,
    pub pos_bar: f64,
    /// SPEED and DIRECTION are checked separately, because the recording
    /// stores them separately and to wildly different precisions.
    ///
    /// A `CSceneVehicleVis` sample keeps the speed as `exp(i16 / 1000)` -- a
    /// tenth of a per cent -- and the velocity DIRECTION as two signed bytes,
    /// a heading of `i8/127 * pi` and a pitch of `i8/127 * pi/2`. That is a
    /// quantisation step of **1.42 degrees**, and at 118 m/s one step is
    /// 1.5 m/s per axis. A single "velocity error in m/s" bar therefore either
    /// fails a perfect measurement at speed or passes a bad one at walking
    /// pace; measured on 228811 the residual was 1.99 m/s with the position
    /// matching to 0.0002 m, which is the encoding and nothing else.
    pub speed_err: f64,
    pub speed_bar: f64,
    pub vdir_err_deg: f64,
    pub vdir_bar_deg: f64,
    pub ang_err_deg: f64,
    pub ang_bar_deg: f64,
    /// The key the recording itself scores at that instant, for context: it is
    /// NOT part of the pass/fail, because a key is a function of the state and
    /// the state is what is being checked.
    pub expected_key: f64,
    /// WHICH TICK THE TWO CLOCKS AGREE AT. The child labels a state by the
    /// clock value it was gathered at; the sampler's own `sample_ms` labels the
    /// first record of tick `t` as the END of tick `t - 1`. So the two
    /// conventions can sit one tick apart, and at 118 m/s one tick is 1.2 m --
    /// which reads exactly like a wrong car if you assume the shift is zero.
    ///
    /// So it is measured rather than assumed: the residual is minimised over
    /// shifts, this is the winner, and it is reported. A shift bigger than one
    /// tick is not a labelling convention and fails.
    pub shift_ticks: i64,
    /// The position residual with no shift at all, so the size of the
    /// convention gap is always on screen next to the verdict.
    pub pos_err_unshifted: f64,
}

/// A shift of more than this many ticks is not a labelling convention -- it is
/// the wrong instant, or the wrong car.
pub const MAX_SHIFT_TICKS: i64 = 1;

impl Agreement {
    pub fn passed(&self) -> bool {
        self.shift_ticks.abs() <= MAX_SHIFT_TICKS
            && self.pos_err <= self.pos_bar
            && self.speed_err <= self.speed_bar
            && self.vdir_err_deg <= self.vdir_bar_deg
            && self.ang_err_deg <= self.ang_bar_deg
    }
    pub fn report(&self) -> String {
        format!(
            "seed identity control at race {} ({}):\n  \
             position {:.4} m (bar {:.4})   speed {:.4} m/s (bar {:.4})   \
             heading {:.3} deg (bar {:.3})   attitude {:.3} deg (bar {:.3})\n  \
             clock: best fit at a shift of {} tick(s); unshifted the position residual is \
             {:.4} m\n  \
             fork    {}\n  \
             ghost   {}\n  \
             key: the fork measured {:+.4}, the recording scores {:+.4} at that instant",
            crate::report::secs(self.race_ms),
            if self.passed() { "PASS" } else { "FAILED" },
            self.pos_err,
            self.pos_bar,
            self.speed_err,
            self.speed_bar,
            self.vdir_err_deg,
            self.vdir_bar_deg,
            self.ang_err_deg,
            self.ang_bar_deg,
            self.shift_ticks,
            self.pos_err_unshifted,
            self.measured,
            self.expected,
            self.measured.key,
            self.expected_key
        )
    }
}

/// The best key the RECORDING itself reaches inside the gate, and where.
///
/// This is the offline twin of what the child computes, on the same `Gate` and
/// through the same `key_eval` -- there is one implementation of the key and
/// this is it, sampled at 50 ms instead of 10.
pub fn from_ghost(path: &str, gate: &Gate, start_offset_ms: i32) -> Result<GateRecord, String> {
    let d = gbx::record::decode_ghost(path).map_err(|e| format!("{}: {}", path, e))?;
    let mut best: Option<GateRecord> = None;
    for s in &d.samples {
        let pos = [s.x as f32, s.y as f32, s.z as f32];
        let vel = [s.vx as f32, s.vy as f32, s.vz as f32];
        let quat = [s.qw as f32, s.qx as f32, s.qy as f32, s.qz as f32];
        let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
        if speed < gate.minspeed || gate.over(pos) > 0.0 {
            continue;
        }
        let key = key_eval(&gate.prog, pos, vel, quat);
        if !key.is_finite() {
            continue;
        }
        if best.map(|b| key > b.key).unwrap_or(true) {
            // the tape tick, on the same clock the fork's own record uses
            let tick = ((s.time_ms as i64 - start_offset_ms as i64) / 10) as i32;
            best = Some(GateRecord { tick, key, pos, vel, quat });
        }
    }
    best.ok_or_else(|| {
        format!(
            "{}: this recording never enters the gate box above {} m/s, so it cannot say what \
             the fork should have measured. Either the box is somewhere this tape does not go \
             -- which is worth knowing before a search runs on it -- or the recording is not of \
             this tape.",
            path, gate.minspeed
        )
    })
}

/// Compare the fork's measured gate state for the seed against the seed's own
/// telemetry at the same instant.
///
/// `race_ms` is `tick * 10 + start_offset_ms`, the same clock the telemetry's
/// own sample times are on.
pub fn check(
    path: &str,
    gate: &Gate,
    measured: &GateRecord,
    start_offset_ms: i32,
) -> Result<Agreement, String> {
    let d = gbx::record::decode_ghost(path).map_err(|e| format!("{}: {}", path, e))?;
    if d.samples.len() < 2 {
        return Err(format!("{}: {} telemetry samples", path, d.samples.len()));
    }
    let race_ms = measured.tick as i64 * 10 + start_offset_ms as i64;

    // MEASURE THE CLOCK SHIFT, do not assume it. Both conventions are
    // defensible and they differ by one tick; at speed that is over a metre,
    // which is the size of a wrong answer.
    let mut best: Option<(i64, GateRecord, f64)> = None;
    let mut unshifted = f64::NAN;
    for shift in -(MAX_SHIFT_TICKS + 1)..=(MAX_SHIFT_TICKS + 1) {
        let at = race_ms + shift * 10;
        let e = interpolate(&d.samples, at, gate, measured.tick);
        let err = dist3(measured.pos, e.pos);
        if shift == 0 {
            unshifted = err;
        }
        if best.as_ref().map(|(_, _, b)| err < *b).unwrap_or(true) {
            best = Some((shift, e, err));
        }
    }
    let (shift_ticks, expected, pos_err) = best.expect("the shift sweep is never empty");

    let (a, b) = bracket(&d.samples, race_ms + shift_ticks * 10);
    let sample_move = dist3(
        [a.x as f32, a.y as f32, a.z as f32],
        [b.x as f32, b.y as f32, b.z as f32],
    );
    let sample_dspeed = (a.speed_ms - b.speed_ms).abs();
    let sample_vturn = angle_between(
        [a.vx as f32, a.vy as f32, a.vz as f32],
        [b.vx as f32, b.vy as f32, b.vz as f32],
    );
    let sample_turn = quat_angle_deg([a.qw, a.qx, a.qy, a.qz], [b.qw, b.qx, b.qy, b.qz]);

    Ok(Agreement {
        race_ms,
        measured: *measured,
        expected,
        pos_err,
        pos_bar: 0.25 + 0.25 * sample_move,
        speed_err: (measured.speed() - expected.speed()).abs() as f64,
        // the stored speed is `exp(i16/1000)`: a tenth of a per cent, so the
        // bar is relative with a floor for the interpolation.
        speed_bar: 0.25 + 0.002 * expected.speed() as f64 + 0.25 * sample_dspeed,
        vdir_err_deg: angle_between(measured.vel, expected.vel),
        // ONE QUANTISATION STEP of the recording's own velocity heading byte
        // (pi/127 = 1.417 deg), plus room for the interpolation.
        vdir_bar_deg: 1.5 + 0.5 * sample_vturn,
        ang_err_deg: quat_angle_deg(f4(measured.quat), f4(expected.quat)),
        ang_bar_deg: 3.0 + 0.5 * sample_turn,
        expected_key: expected.key as f64,
        shift_ticks,
        pos_err_unshifted: unshifted,
    })
}

/// The angle between two vectors, in degrees. 0 when either is standing still,
/// because a direction is not defined there and the speed check covers it.
fn angle_between(a: [f32; 3], b: [f32; 3]) -> f64 {
    let d = |v: [f32; 3]| (v[0] as f64 * v[0] as f64 + v[1] as f64 * v[1] as f64 + v[2] as f64 * v[2] as f64).sqrt();
    let (na, nb) = (d(a), d(b));
    if na < 1e-6 || nb < 1e-6 {
        return 0.0;
    }
    let dot = a[0] as f64 * b[0] as f64 + a[1] as f64 * b[1] as f64 + a[2] as f64 * b[2] as f64;
    (dot / (na * nb)).clamp(-1.0, 1.0).acos().to_degrees()
}

fn f4(q: [f32; 4]) -> [f64; 4] {
    [q[0] as f64, q[1] as f64, q[2] as f64, q[3] as f64]
}

fn dist3(x: [f32; 3], y: [f32; 3]) -> f64 {
    (((x[0] - y[0]) as f64).powi(2) + ((x[1] - y[1]) as f64).powi(2) + ((x[2] - y[2]) as f64).powi(2))
        .sqrt()
}

/// The two samples an instant falls between.
fn bracket(
    samples: &[gbx::record::Sample],
    race_ms: i64,
) -> (&gbx::record::Sample, &gbx::record::Sample) {
    let mut lo = 0usize;
    for (i, s) in samples.iter().enumerate() {
        if (s.time_ms as i64) <= race_ms {
            lo = i;
        }
    }
    let hi = (lo + 1).min(samples.len() - 1);
    (&samples[lo], &samples[hi])
}

/// The recorded state at an instant between two 50 ms samples.
fn interpolate(
    samples: &[gbx::record::Sample],
    race_ms: i64,
    gate: &Gate,
    label: i32,
) -> GateRecord {
    let (a, b) = bracket(samples, race_ms);
    let span = (b.time_ms - a.time_ms) as f64;
    let u = if span > 0.0 {
        ((race_ms - a.time_ms as i64) as f64 / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let lerp = |x: f64, y: f64| x + u * (y - x);
    let pos = [lerp(a.x, b.x) as f32, lerp(a.y, b.y) as f32, lerp(a.z, b.z) as f32];
    let vel = [lerp(a.vx, b.vx) as f32, lerp(a.vy, b.vy) as f32, lerp(a.vz, b.vz) as f32];
    // nlerp, with the near quaternion flipped onto the same hemisphere: q and
    // -q are the same rotation and a raw lerp between them collapses to zero.
    let qa = [a.qw, a.qx, a.qy, a.qz];
    let mut qb = [b.qw, b.qx, b.qy, b.qz];
    if dot4(qa, qb) < 0.0 {
        qb = [-qb[0], -qb[1], -qb[2], -qb[3]];
    }
    let mut q = [0.0f64; 4];
    for i in 0..4 {
        q[i] = lerp(qa[i], qb[i]);
    }
    let n = dot4(q, q).sqrt();
    let quat = if n > 0.0 {
        [(q[0] / n) as f32, (q[1] / n) as f32, (q[2] / n) as f32, (q[3] / n) as f32]
    } else {
        [1.0, 0.0, 0.0, 0.0]
    };
    GateRecord { tick: label, key: key_eval(&gate.prog, pos, vel, quat), pos, vel, quat }
}

fn dot4(a: [f64; 4], b: [f64; 4]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// The rotation angle between two unit quaternions, in degrees.
fn quat_angle_deg(a: [f64; 4], b: [f64; 4]) -> f64 {
    let na = dot4(a, a).sqrt();
    let nb = dot4(b, b).sqrt();
    if na <= 0.0 || nb <= 0.0 {
        return 180.0;
    }
    let c = (dot4(a, b) / (na * nb)).abs().clamp(0.0, 1.0);
    2.0 * c.acos().to_degrees()
}
