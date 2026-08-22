//! Measure this map's gravity from a recording's own free fall.
//!
//! The fleet has measured TM2020's gravity at about -24.3 to -24.9 m/s^2 on
//! three other maps, and 9.81 appears nowhere in the game. A published
//! mechanism for this map compares the climb's deceleration against
//! `9.81 * sin(26.6 deg) = 4.39` and concludes the car is losing energy at
//! 2.4x what gravity accounts for. If gravity here is ~24.6 the same
//! comparison says the opposite, so the constant has to be measured before the
//! conclusion can be read.
//!
//! Method: central-difference the vertical velocity, cut the run into maximal
//! stretches where that acceleration stays inside a free-fall band, fit a
//! straight line to `vy(t)` over each stretch, and report the distribution of
//! the fitted slopes weighted by stretch length. The longest stretches are the
//! best measurements: a long fall is many samples of one number.
//!
//! Two guards this measurement needs and a naive one does not have:
//!
//! * **The derived `is_ground_contact` bit cannot select the samples.** It is a
//!   guessed bit mask, and on this recording it reads `False` on all 85 421
//!   samples — including a car sitting still on a floor. It is reported here
//!   and never used, so it is the thing being checked.
//! * **A band selects for its own answer.** Picking samples near -24 and then
//!   reporting -24 is circular. The band is wide (-40..-12), the answer sits
//!   well inside it, and the fitted slopes are reported as a distribution so a
//!   band artefact would show up as a pile-up at an edge.

use crate::csv::Sample;

pub fn report(s: &[Sample]) {
    if !s.iter().any(|r| r.vy.is_some()) {
        println!("no vy column: gravity needs a recording that carries velocity components");
        return;
    }
    let mut acc: Vec<(f64, f64, f64)> = Vec::new(); // t, vy, a_y
    for w in s.windows(3) {
        let (a, b, c) = (w[0], w[1], w[2]);
        let (vy0, vy1, vy2) = match (a.vy, b.vy, c.vy) {
            (Some(p), Some(q), Some(r)) => (p, q, r),
            _ => continue,
        };
        let dt = c.t - a.t;
        if dt <= 0.0 || dt > 0.25 {
            continue;
        }
        let _ = vy0;
        acc.push((b.t, vy1, (vy2 - vy0) / dt));
    }
    let n_air = s.iter().filter(|r| r.ground == Some(false)).count();
    println!(
        "{} samples, {} with a usable a_y; the derived contact bit says airborne on {} of them (not used)",
        s.len(), acc.len(), n_air
    );

    // maximal stretches inside the free-fall band
    let (lo, hi) = (-40.0, -12.0);
    let mut segs: Vec<(f64, f64, usize, f64)> = Vec::new(); // t0, dur, n, fitted slope
    let mut i = 0;
    while i < acc.len() {
        if acc[i].2 < lo || acc[i].2 > hi {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < acc.len() && acc[j + 1].2 >= lo && acc[j + 1].2 <= hi && acc[j + 1].0 - acc[j].0 < 0.25 {
            j += 1;
        }
        let run = &acc[i..=j];
        if run.len() >= 5 {
            let n = run.len() as f64;
            let mt = run.iter().map(|r| r.0).sum::<f64>() / n;
            let mv = run.iter().map(|r| r.1).sum::<f64>() / n;
            let num: f64 = run.iter().map(|r| (r.0 - mt) * (r.1 - mv)).sum();
            let den: f64 = run.iter().map(|r| (r.0 - mt).powi(2)).sum();
            if den > 1e-9 {
                segs.push((run[0].0, run[run.len() - 1].0 - run[0].0, run.len(), num / den));
            }
        }
        i = j + 1;
    }
    if segs.is_empty() {
        println!("no free-fall stretch of 5+ samples in this window");
        return;
    }
    segs.sort_by(|a, b| b.2.cmp(&a.2));
    println!("\n{} free-fall stretches of 5+ samples. The ten longest:", segs.len());
    println!("{:>10} {:>8} {:>6} {:>12}", "race_s", "dur_s", "n", "fitted a_y");
    for sg in segs.iter().take(10) {
        println!("{:>10.3} {:>8.3} {:>6} {:>12.3}", sg.0, sg.1, sg.2, sg.3);
    }
    let total: usize = segs.iter().map(|s| s.2).sum();
    let wmean: f64 = segs.iter().map(|s| s.3 * s.2 as f64).sum::<f64>() / total as f64;
    let mut by_len = segs.clone();
    by_len.retain(|s| s.2 >= 15);
    let long_mean = if by_len.is_empty() {
        f64::NAN
    } else {
        by_len.iter().map(|s| s.3 * s.2 as f64).sum::<f64>() / by_len.iter().map(|s| s.2).sum::<usize>() as f64
    };
    let mut slopes: Vec<f64> = segs.iter().map(|s| s.3).collect();
    slopes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "\nweighted over all {} stretches ({} samples): a_y = {:.3} m/s^2",
        segs.len(), total, wmean
    );
    println!(
        "weighted over the {} stretches of 15+ samples:  a_y = {:.3} m/s^2",
        by_len.len(), long_mean
    );
    println!(
        "median stretch slope {:.3}, quartiles {:.3} / {:.3}, extremes {:.3} / {:.3}",
        slopes[slopes.len() / 2],
        slopes[slopes.len() / 4],
        slopes[slopes.len() * 3 / 4],
        slopes[0],
        slopes[slopes.len() - 1]
    );
}
