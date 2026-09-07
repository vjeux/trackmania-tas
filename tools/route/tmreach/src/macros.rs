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
    }
    Built::Recs(out)
}

pub fn macros_tsv(lib: &[Macro]) -> String {
    let mut s = String::from("macro_id\tdescription\tshape\n");
    for m in lib {
        s.push_str(&format!("{}\t{}\t{:?}\n", m.id, m.description, m.shape));
    }
    s
}
