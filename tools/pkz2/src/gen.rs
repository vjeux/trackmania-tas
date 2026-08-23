//! The candidate generators.
//!
//! `respawn` sweeps the instant a respawn is pressed. It exists because a
//! respawn is the only edit on this map that RESETS the car to a state both
//! the recording and the simulation share -- its crossing state at the last
//! checkpoint -- so it is the only edit that can put a desynced re-simulation
//! back onto the human's line, and the alignment of the press is the whole
//! question.
//!
//! `zigzag` writes a switchback: alternating full-lock steer with the throttle
//! held, optionally after a braking window. It exists because the human's own
//! recording climbs this hill in switchbacks -- five reversals, 45 s, 25-40
//! km/h -- while every search run on this map so far has been optimising a
//! straight-line ballistic rush at 125 km/h that the human never attempts.

use crate::edit::Edit;
use crate::mkcand::Spec;

fn eo(from_ms: i64, to_ms: i64, chan: &str, val: i64) -> Edit {
    Edit { from_ms, to_ms, chan: chan.to_string(), val, optional: true }
}

fn e(from_ms: i64, to_ms: i64, chan: &str, val: i64) -> Edit {
    Edit { from_ms, to_ms, chan: chan.to_string(), val, optional: false }
}

pub fn respawn_sweep(from_ms: i64, to_ms: i64, step_ms: i64, fixed: &[Edit]) -> Vec<Spec> {
    let mut v = Vec::new();
    let mut t = from_ms;
    while t <= to_ms {
        let mut edits: Vec<Edit> = fixed.to_vec();
        edits.push(e(t, t + 10, "respawn", 1));
        v.push(Spec { name: format!("rsp_{}", t), edits });
        t += step_ms;
    }
    v
}

/// A switchback, parameterised the way the terrain is: when to start, how long
/// each traverse lasts, how many of them, which way the first one goes, and
/// how hard to brake on the way in.
pub struct Zig {
    pub brake_from: i64,
    pub brake_ms: i64,
    pub start_ms: i64,
    pub leg_ms: i64,
    /// The reverse leg. The driver's own traverses are NOT symmetric -- his
    /// +z legs and -z legs differ by up to 2x -- so a switchback with one leg
    /// length cannot express the line he drives.
    pub leg2_ms: i64,
    pub legs: i64,
    pub first: i64,
    pub gas: bool,
    /// Alternate legs run in REVERSE.
    ///
    /// The driver's own switchback is not a steering pattern. At each end of
    /// a traverse he comes to 1.8-7.4 km/h and goes back the other way, and
    /// in this game holding the brake at rest is reverse gear. A zig-zag that
    /// only steers cannot express the manoeuvre he uses to climb this hill.
    pub rev: bool,
}

pub fn zigzag(z: &Zig, fixed: &[Edit]) -> Spec {
    let mut edits: Vec<Edit> = fixed.to_vec();
    if z.brake_ms > 0 {
        edits.push(e(z.brake_from, z.brake_from + z.brake_ms, "brake", 1));
        edits.push(e(z.brake_from, z.brake_from + z.brake_ms, "accel", 0));
    }
    if z.gas && !z.rev {
        let span = (z.legs / 2) * (z.leg_ms + z.leg2_ms) + (z.legs % 2) * z.leg_ms;
        edits.push(eo(z.start_ms, z.start_ms + span, "accel", 1));
        edits.push(eo(z.start_ms, z.start_ms + span, "brake", 0));
    }
    let mut t0 = z.start_ms;
    for k in 0..z.legs {
        let side = if k % 2 == 0 { z.first } else { -z.first };
        let len = if k % 2 == 0 { z.leg_ms } else { z.leg2_ms };
        edits.push(eo(t0, t0 + len, "steer", side));
        if z.rev {
            let up = k % 2 == 0;
            edits.push(eo(t0, t0 + len, "accel", if up { 1 } else { 0 }));
            edits.push(eo(t0, t0 + len, "brake", if up { 0 } else { 1 }));
        }
        t0 += len;
    }
    Spec {
        name: format!(
            "zz_b{}_{}_s{}_l{}_{}_n{}_f{}{}",
            z.brake_from, z.brake_ms, z.start_ms, z.leg_ms, z.leg2_ms, z.legs, z.first,
            if z.rev { "_rev" } else if z.gas { "_g" } else { "" }
        ),
        edits,
    }
}
