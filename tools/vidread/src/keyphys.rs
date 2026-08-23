//! Do the recovered key states explain the speed the video shows?
//!
//! This is the only test of the input recovery that does not depend on another
//! reading of the same pixels. The lamps come from one corner of the screen,
//! the speed from another, and the race time that joins them from a third
//! instrument again — so if gas-on ticks accelerate and brake-on ticks
//! decelerate, three independent readings are agreeing about the same car.
//!
//! It is a weak test in one direction and a strong one in the other: a car in
//! the air or on its roof does not accelerate under gas, so gas-on ticks that
//! lose speed prove nothing. Brake-on ticks that GAIN speed, on the other
//! hand, are hard to explain and are counted separately.

use crate::keytape::Tick;
use std::collections::BTreeMap;
use std::io::Write;

pub struct Stats {
    pub n: usize,
    pub mean_dv: f64,
}

fn stats(v: &[f64]) -> Stats {
    if v.is_empty() {
        return Stats { n: 0, mean_dv: 0.0 };
    }
    Stats { n: v.len(), mean_dv: v.iter().sum::<f64>() / v.len() as f64 }
}

/// `speed` is race_ms -> km/h. `window_ms` is how far ahead the change in
/// speed is measured; a single 10 ms tick is below the readout's 1 km/h
/// quantisation, so the difference is taken over a longer arm.
pub fn report(
    rec: &BTreeMap<i64, Tick>,
    speed: &BTreeMap<i64, f64>,
    window_ms: i64,
    tol_ms: i64,
    o: &mut impl Write,
) {
    let at = |ms: i64| -> Option<f64> {
        let mut best: Option<(i64, f64)> = None;
        for (k, v) in speed.range(ms - tol_ms..=ms + tol_ms) {
            let d = (k - ms).abs();
            if best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, *v));
            }
        }
        best.map(|(_, v)| v)
    };
    let mut gas = Vec::new();
    let mut brake = Vec::new();
    let mut coast = Vec::new();
    for (ms, t) in rec {
        let (Some(a), Some(b)) = (at(*ms), at(ms + window_ms)) else { continue };
        let dv = b - a;
        if t.keys[2] || t.keys[0] {
            brake.push(dv);
        } else if t.keys[1] {
            gas.push(dv);
        } else {
            coast.push(dv);
        }
    }
    let g = stats(&gas);
    let b = stats(&brake);
    let c = stats(&coast);
    writeln!(o, "state\tticks\tmean d(kmh) over {window_ms} ms").unwrap();
    writeln!(o, "gas\t{}\t{:+.2}", g.n, g.mean_dv).unwrap();
    writeln!(o, "brake\t{}\t{:+.2}", b.n, b.mean_dv).unwrap();
    writeln!(o, "neither\t{}\t{:+.2}", c.n, c.mean_dv).unwrap();
    let wrong = brake.iter().filter(|x| **x > 2.0).count();
    writeln!(
        o,
        "# {} of {} brake ticks GAIN more than 2 km/h over the window",
        wrong,
        b.n.max(1)
    )
    .unwrap();
}
