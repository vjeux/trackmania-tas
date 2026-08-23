//! The candidate generators.
//!
//! `respawn` sweeps the instant a respawn is pressed. It exists because a
//! respawn is the only edit on this map that RESETS the car to a state both
//! the recording and the simulation share -- its crossing state at the last
//! checkpoint.
//!
//! `zigzag` and `pulse` are two readings of the same thing: the manoeuvre the
//! driver uses to climb the ice ramp after CP2, which his own recording shows
//! and which no search on this map had ever expressed. `zigzag` holds full
//! lock for a whole leg; `pulse` is what he actually does.

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

/// A switchback held at full lock for the whole traverse.
pub struct Zig {
    pub brake_from: i64,
    pub brake_ms: i64,
    pub start_ms: i64,
    pub leg_ms: i64,
    /// The reverse leg. The driver's own traverses are NOT symmetric.
    pub leg2_ms: i64,
    pub legs: i64,
    pub first: i64,
    pub gas: bool,
    /// Alternate legs run in REVERSE (brake at rest is reverse gear).
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

/// The manoeuvre as the driver actually drives it.
///
/// Read off his own recording at 282.7-287.0 s, one 60 ms sample at a time:
/// he holds **full lock for about 1.2 s** to turn the car around at the end of
/// a traverse (1.7 -> 26 km/h through the turn), then **releases the steering
/// to zero** and drives the diagonal straight at 27-37 km/h for another
/// ~2.5 s. Steer is neutral for most of every leg.
///
/// That is the difference from `zigzag`, which holds full lock for the whole
/// leg: full lock all the way across a traverse scrubs the speed the traverse
/// exists to carry. His throttle also alternates between legs -- he powers the
/// +z traverses and coasts the -z ones -- and both climb.
///
/// A `descend` phase makes the second attempt expressible: after the climb
/// stalls he coasts back DOWN the ramp (throttle off, brake off, one steer
/// side held) for about 4.5 s to y ~ 120, then climbs again. Our tapes have
/// never done this: they keep fighting on the way down and wedge at the
/// bottom at x = 833, permanently.
pub struct Pulse {
    pub start_ms: i64,
    /// How long full lock is held to turn the car at the start of a leg.
    pub turn_ms: i64,
    /// The whole leg, turn included.
    pub leg_ms: i64,
    pub legs: i64,
    pub first: i64,
    /// `true`: coast alternate legs the way he does. `false`: power all of them.
    pub alt_throttle: bool,
    /// A coast-back down the ramp before the legs start; 0 disables it.
    pub descend_ms: i64,
    pub descend_steer: i64,
}

pub fn pulse(p: &Pulse, fixed: &[Edit]) -> Spec {
    let mut edits: Vec<Edit> = fixed.to_vec();
    let mut t0 = p.start_ms;
    if p.descend_ms > 0 {
        edits.push(e(t0, t0 + p.descend_ms, "accel", 0));
        edits.push(eo(t0, t0 + p.descend_ms, "brake", 0));
        edits.push(eo(t0, t0 + p.descend_ms, "steer", p.descend_steer));
        t0 += p.descend_ms;
    }
    for k in 0..p.legs {
        let side = if k % 2 == 0 { p.first } else { -p.first };
        let turn = p.turn_ms.min(p.leg_ms);
        edits.push(eo(t0, t0 + turn, "steer", side));
        edits.push(eo(t0 + turn, t0 + p.leg_ms, "steer", 0));
        let powered = !p.alt_throttle || k % 2 == 0;
        edits.push(eo(t0, t0 + p.leg_ms, "accel", if powered { 1 } else { 0 }));
        edits.push(eo(t0, t0 + p.leg_ms, "brake", 0));
        t0 += p.leg_ms;
    }
    Spec {
        name: format!(
            "pl_s{}_t{}_l{}_n{}_f{}{}{}",
            p.start_ms, p.turn_ms, p.leg_ms, p.legs, p.first,
            if p.alt_throttle { "_alt" } else { "_g" },
            if p.descend_ms > 0 { format!("_d{}_{}", p.descend_ms, p.descend_steer) } else { String::new() }
        ),
        edits,
    }
}
