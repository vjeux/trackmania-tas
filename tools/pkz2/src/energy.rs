//! Energy bookkeeping along a path.
//!
//! `ord_`'s rule for this map, and it is the right one: to test whether a car
//! has power, do not look at where it is — difference its kinetic plus
//! potential energy over a stretch. A car with a live engine makes energy; a
//! coasting car cannot. The arithmetic only works with the map's own gravity,
//! which is why `pkz2 gravity` exists and why `--g` has no innocent default.
//!
//! Columns:
//!   dE/dt   the rate the car's total mechanical energy per kilogram changes.
//!           Positive anywhere is an engine. Zero is a frictionless coast.
//!   a_path  the along-path acceleration, from the speed column.
//!   g_slope the part of a_path that the slope's gravity alone accounts for,
//!           computed from the path's own rise over its own run, so it needs
//!           no assumption about which surface the car is on.
//!   resid   a_path - (-g_slope): what is left after gravity. Negative is
//!           drag/friction, positive is thrust.

use crate::csv::Sample;

pub fn report(s: &[Sample], g: f64, step: f64) {
    println!("# gravity {:.3} m/s^2   {} samples   {:.3}..{:.3} s", g, s.len(), s[0].t, s[s.len() - 1].t);
    println!(
        "{:>9} {:>9} {:>8} {:>9} {:>8} {:>10} {:>9} {:>9} {:>9} {:>8} {:>4}{:>4}",
        "race_s", "x", "y", "z", "kmh", "E J/kg", "dE/dt", "a_path", "g_slope", "resid", "gas", "brk"
    );
    let mut want = f64::NEG_INFINITY;
    let mut e_first = f64::NAN;
    let mut e_last = f64::NAN;
    let mut y_first = f64::NAN;
    let mut y_last = f64::NAN;
    for i in 0..s.len() {
        let r = s[i];
        let vms = r.v / 3.6;
        let e = 0.5 * vms * vms + g * r.y;
        if e_first.is_nan() {
            e_first = e;
            y_first = r.y;
        }
        e_last = e;
        y_last = r.y;
        if i == 0 || i + 1 == s.len() {
            continue;
        }
        let (a, b) = (s[i - 1], s[i + 1]);
        let dt = b.t - a.t;
        if dt <= 0.0 {
            continue;
        }
        let ea = 0.5 * (a.v / 3.6).powi(2) + g * a.y;
        let eb = 0.5 * (b.v / 3.6).powi(2) + g * b.y;
        let dedt = (eb - ea) / dt;
        let a_path = (b.v / 3.6 - a.v / 3.6) / dt;
        let run = ((b.x - a.x).powi(2) + (b.z - a.z).powi(2)).sqrt();
        let rise = b.y - a.y;
        let sin_theta = if run + rise.abs() > 1e-9 { rise / (run * run + rise * rise).sqrt() } else { 0.0 };
        let g_slope = g * sin_theta;
        if r.t < want {
            continue;
        }
        want = if step > 0.0 { r.t + step - 1e-9 } else { f64::NEG_INFINITY };
        println!(
            "{:>9.3} {:>9.2} {:>8.2} {:>9.2} {:>8.1} {:>10.2} {:>9.2} {:>9.3} {:>9.3} {:>8.3} {:>4}{:>4}",
            r.t, r.x, r.y, r.z, r.v, e, dedt, a_path, g_slope, a_path + g_slope,
            r.gas.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "-".into()),
            r.brake.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "-".into()),
        );
    }
    println!(
        "# TOTAL  dE = {:+.2} J/kg over {:.3} s   (dy {:+.3} m)   {}",
        e_last - e_first,
        s[s.len() - 1].t - s[0].t,
        y_last - y_first,
        if e_last - e_first > 0.0 { "ENERGY MADE -> an engine is running" } else { "energy lost -> consistent with a coast" }
    );
}
