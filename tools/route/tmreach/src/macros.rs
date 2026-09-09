//! The action-macro library (G3). Every macro is an OPEN-LOOP input program
//! over `h` ticks, possibly BASE-RELATIVE (built on the human's own records
//! for those ticks). Macro 0 is the reference tape itself — the identity
//! control of every fan-out.
//!
//! The library's SHAPE is an assumption (COMMON-RULES): sign, magnitude,
//! duration and compound shapes are all varied; unbuildable / no-op macros
//! are counted apart from nulls by the fan-out.

use forkoracle::forksrv::{rec_of, Rec};

#[derive(Clone, Debug)]
pub struct Macro {
    pub id: u16,
    pub description: String,
    pub shape: Shape,
}

#[derive(Clone, Debug)]
pub enum Shape {
    /// The human's own records, unchanged.
    Reference,
    /// Constant steer (i8) + pedals for the whole horizon.
    Hold { steer: i8, gas: bool, brake: bool },
    /// Human's records with steer offset by `dsteer` for the first `ticks`
    /// ticks, then the human's records unchanged.
    BaseSteer { dsteer: i16, ticks: usize },
    /// Human's records with the pedals overridden for the first `ticks`.
    BasePedal { gas: bool, brake: bool, ticks: usize },
    /// Steer ramps linearly from 0 to `to` over `ramp` ticks, then holds; gas.
    Ramp { to: i8, ramp: usize },
    /// +a for `ticks`, then −a for `ticks`, then 0; gas.
    Doublet { a: i8, ticks: usize },
    /// Steer `steer` only if the START state is airborne (|vy| > 2 m/s), else
    /// the reference (a no-op, logged as such); gas held.
    AirSteer { steer: i8 },
    /// SLALOM: steer alternates +a / −a every `half` ticks (period 2·half), gas held
    /// (coordinator, 2026-09-07 06:26Z: 3–4 periods × 2 amplitudes).
    Slalom { a: i8, half: usize },
    /// RUNG 3 (coordinator 2026-09-09 09:26Z) — through-the-piece shapes: brake tap `tap` ticks at the entry, then steer
    /// ramps 0 -> `to` over `ramp` ticks and holds, gas; `counter` > 0: after the ramp holds `hold` ticks, steer flips to
    /// -to/2 for `counter` ticks (counter-steer at the apex), then 0.
    Compound { tap: usize, to: i8, ramp: usize, hold: usize, counter: usize },
    /// throttle lift-off on the piece: steer `steer` held, gas OFF for `off` ticks starting at `at`, then gas again
    LiftOff { steer: i8, at: usize, off: usize },
    /// AIR CONTROL: pedals pitch the car in flight, steer rolls it — hold `gas`/`brake`/`steer` for `ticks`, then gas straight
    Air { steer: i8, gas: bool, brake: bool, ticks: usize },
    /// ATTITUDE at take-off: a small steer offset for the first `ticks` (roll on the ramp), then straight gas
    Attitude { steer: i8, ticks: usize },
}

impl Shape {
    /// The family a shape belongs to (distinct-outcome counts are logged per family).
    pub fn family(&self) -> &'static str {
        match self {
            Shape::Reference => "reference",
            Shape::Hold { .. } => "hold",
            Shape::BaseSteer { .. } => "base-steer",
            Shape::BasePedal { .. } => "base-pedal",
            Shape::Ramp { .. } => "ramp",
            Shape::Doublet { .. } => "doublet",
            Shape::AirSteer { .. } => "air-steer",
            Shape::Slalom { .. } => "slalom",
            Shape::Compound { .. } => "compound",
            Shape::LiftOff { .. } => "lift-off",
            Shape::Air { .. } => "air",
            Shape::Attitude { .. } => "attitude",
        }
    }
}

pub fn library_v0() -> Vec<Macro> {
    let mut v = Vec::new();
    let mut push = |description: String, shape: Shape| {
        let id = v.len() as u16;
        v.push(Macro { id, description, shape });
    };
    push("reference: the human's own tape continued (identity control)".into(), Shape::Reference);
    for &steer in &[-127i8, -64, -24, 0, 24, 64, 127] {
        for &(gas, brake, pn) in &[(true, false, "gas"), (false, false, "coast"), (false, true, "brake"), (true, true, "gas+brake")] {
            push(format!("hold steer {steer:+} {pn}"), Shape::Hold { steer, gas, brake });
        }
    }
    for &d in &[-80i16, -32, 32, 80] {
        push(format!("human steer {d:+} for 60 ticks then human"), Shape::BaseSteer { dsteer: d, ticks: 60 });
    }
    push("human with coast for 50 ticks then human".into(), Shape::BasePedal { gas: false, brake: false, ticks: 50 });
    push("human with brake pulse 30 ticks then human".into(), Shape::BasePedal { gas: false, brake: true, ticks: 30 });
    push("human with gas+brake 30 ticks then human".into(), Shape::BasePedal { gas: true, brake: true, ticks: 30 });
    for &to in &[-127i8, 127] {
        push(format!("ramp steer 0 -> {to:+} over 30 ticks then hold, gas"), Shape::Ramp { to, ramp: 30 });
    }
    for &a in &[64i8, 127] {
        for &t in &[40usize, 80] {
            push(format!("doublet {a:+} {t} ticks then {:+} {t} ticks then 0, gas", -(a as i16)), Shape::Doublet { a, ticks: t });
            push(format!("doublet {:+} {t} ticks then {a:+} {t} ticks then 0, gas", -(a as i16)), Shape::Doublet { a: -a, ticks: t });
        }
    }
    for &s in &[-127i8, 127] {
        push(format!("air-control steer {s:+} while airborne at the start (else reference)"), Shape::AirSteer { steer: s });
    }
    // v1 (ids 48..55): the slalom family -- periods 60, 100, 160, 240 ticks (half 30, 50, 80, 120) × amplitudes 64, 127
    for &half in &[30usize, 50, 80, 120] {
        for &a in &[64i8, 127] {
            push(format!("slalom steer ±{a} every {half} ticks (period {}), gas", 2 * half), Shape::Slalom { a, half });
        }
    }
    v
}

/// What building a macro produced.
pub enum Built {
    Recs(Vec<Rec>),
    /// The macro is the reference for this start (AirSteer on the ground).
    NoOp,
}

/// Build the records for ticks `from..from+h` on top of the reference records
/// `base` (exactly h of them). `airborne` is the start state's.
pub fn build(m: &Macro, base: &[(u8, u8, u8)], airborne: bool) -> Built {
    let h = base.len();
    let mut out = Vec::with_capacity(h);
    match &m.shape {
        Shape::Reference => return Built::Recs(base.iter().map(|&(s, g, b)| rec_of(s, g, b)).collect()),
        Shape::Hold { steer, gas, brake } => {
            for _ in 0..h {
                out.push(rec_of(*steer as u8, *gas as u8, *brake as u8));
            }
        }
        Shape::BaseSteer { dsteer, ticks } => {
            for (i, &(s, g, b)) in base.iter().enumerate() {
                let st = if i < *ticks { ((s as i8) as i16 + dsteer).clamp(-127, 127) as i8 as u8 } else { s };
                out.push(rec_of(st, g, b));
            }
        }
        Shape::BasePedal { gas, brake, ticks } => {
            for (i, &(s, g, b)) in base.iter().enumerate() {
                if i < *ticks {
                    out.push(rec_of(s, *gas as u8, *brake as u8));
                } else {
                    out.push(rec_of(s, g, b));
                }
            }
        }
        Shape::Ramp { to, ramp } => {
            for i in 0..h {
                let f = (i as f64 / *ramp as f64).min(1.0);
                let st = (f * *to as f64).round() as i8;
                out.push(rec_of(st as u8, 1, 0));
            }
        }
        Shape::Doublet { a, ticks } => {
            for i in 0..h {
                let st: i8 = if i < *ticks { *a } else if i < 2 * ticks { -(*a as i16) as i8 } else { 0 };
                out.push(rec_of(st as u8, 1, 0));
            }
        }
        Shape::AirSteer { steer } => {
            if !airborne {
                return Built::NoOp;
            }
            for _ in 0..h {
                out.push(rec_of(*steer as u8, 1, 0));
            }
        }
        Shape::Slalom { a, half } => {
            for i in 0..h {
                let st: i8 = if (i / half) % 2 == 0 { *a } else { -(*a as i16) as i8 };
                out.push(rec_of(st as u8, 1, 0));
            }
        }
        Shape::Compound { tap, to, ramp, hold, counter } => {
            for i in 0..h {
                let brake = (i < *tap) as u8;
                let st: i8 = if i < *ramp {
                    ((*to as f64) * (i as f64 + 1.0) / (*ramp as f64)).round() as i8
                } else if i < ramp + hold {
                    *to
                } else if i < ramp + hold + counter {
                    (-(*to as i16) / 2) as i8
                } else if *counter > 0 {
                    0
                } else {
                    *to
                };
                out.push(rec_of(st as u8, 1, brake));
            }
        }
        Shape::LiftOff { steer, at, off } => {
            for i in 0..h {
                let gas = !(i >= *at && i < at + off) as u8;
                out.push(rec_of(*steer as u8, gas, 0));
            }
        }
        Shape::Air { steer, gas, brake, ticks } => {
            for i in 0..h {
                if i < *ticks {
                    out.push(rec_of(*steer as u8, *gas as u8, *brake as u8));
                } else {
                    out.push(rec_of(0, 1, 0));
                }
            }
        }
        Shape::Attitude { steer, ticks } => {
            for i in 0..h {
                out.push(rec_of(if i < *ticks { *steer as u8 } else { 0 }, 1, 0));
            }
        }
    }
    Built::Recs(out)
}


/// RUNG 3 macros (compound through-the-piece shapes, lift-off, air control, take-off attitude) — added to the fan with
/// `--compound`; 40 shapes.
pub fn library_compound(start_id: u16) -> Vec<Macro> {
    let mut v = Vec::new();
    let mut id = start_id;
    let mut push = |description: String, shape: Shape| {
        id += 1;
        v.push(Macro { id, description, shape });
    };
    for &to in &[-127i8, -80, 80, 127] {
        for &tap in &[0usize, 15] {
            push(format!("compound: brake tap {tap} then ramp to {to:+} over 30, hold, gas"), Shape::Compound { tap, to, ramp: 30, hold: 200, counter: 0 });
            push(format!("compound: brake tap {tap}, ramp to {to:+} over 30, hold 40, counter-steer 30, gas"), Shape::Compound { tap, to, ramp: 30, hold: 40, counter: 30 });
        }
    }
    for &steer in &[-100i8, 0, 100] {
        push(format!("lift-off: steer {steer:+}, gas off ticks 20-50 then gas"), Shape::LiftOff { steer, at: 20, off: 30 });
        push(format!("lift-off: steer {steer:+}, gas off ticks 0-40 then gas"), Shape::LiftOff { steer, at: 0, off: 40 });
    }
    for &(g, b, pn) in &[(true, false, "gas (nose down)"), (false, true, "brake (nose up)"), (false, false, "coast")] {
        for &steer in &[-64i8, 0, 64] {
            push(format!("air: {pn}, steer {steer:+} for 80 ticks then gas straight"), Shape::Air { steer, gas: g, brake: b, ticks: 80 });
        }
    }
    for &steer in &[-24i8, -12, 12, 24] {
        push(format!("attitude: steer {steer:+} for 30 ticks at take-off then gas straight"), Shape::Attitude { steer, ticks: 30 });
    }
    v
}
pub fn macros_tsv(lib: &[Macro]) -> String {
    let mut s = String::from("macro_id\tdescription\tshape\n");
    for m in lib {
        s.push_str(&format!("{}\t{}\t{:?}\n", m.id, m.description, m.shape));
    }
    s
}
