//! Where a recorded path teleports.
//!
//! A respawn is the one thing in a run that moves the car without driving it,
//! and on this map the driver used 110 of them. Every one is a hard reset of
//! the car's state to its own crossing state at the last checkpoint — which
//! means it is also the one place a divergence between a recording and a
//! re-simulation of its tape can be WIPED, provided both cars hold the same
//! checkpoints when it fires.
//!
//! That makes the respawn census the first thing to know about a desync on a
//! map like this, and nothing in the arm's tooling had it: find every sample
//! pair whose separation is impossible at the speed the car is doing, and
//! print where it went and where it came back.

use crate::csv::Sample;

pub fn report(s: &[Sample], min_jump: f64) {
    println!(
        "{:>10} {:>9} {:>26} {:>26} {:>9} {:>8}",
        "race_s", "jump_m", "from (x,y,z)", "to (x,y,z)", "v_before", "v_after"
    );
    let mut n = 0;
    for w in s.windows(2) {
        let (a, b) = (w[0], w[1]);
        let dt = b.t - a.t;
        if dt <= 0.0 || dt > 0.5 {
            continue;
        }
        let d = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
        // what the speed column says the car could have covered, with slack
        let plausible = (a.v.max(b.v) / 3.6) * dt * 1.6 + 1.0;
        if d > plausible.max(min_jump) {
            n += 1;
            println!(
                "{:>10.3} {:>9.1} {:>26} {:>26} {:>9.1} {:>8.1}",
                b.t, d,
                format!("{:.1},{:.1},{:.1}", a.x, a.y, a.z),
                format!("{:.1},{:.1},{:.1}", b.x, b.y, b.z),
                a.v, b.v
            );
        }
    }
    println!("{} teleports", n);
}
