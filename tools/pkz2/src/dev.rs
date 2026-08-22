//! How far apart two paths are, as a function of time.
//!
//! A single summary number over a whole run says almost nothing about a
//! desync: a run that tracks perfectly for 100 s and then leaves the map has
//! the same median as one that was never right. What the question needs is the
//! curve, and the first time it crosses each of a few thresholds.
//!
//! The two files are sampled on different grids (a recording every 60 ms, a
//! simulator readout every 10 ms), so each reference sample is compared with
//! the nearest sample of the other path in time, and the time gap used is
//! reported so a lag can never be read as a distance.

use crate::csv::Sample;

fn nearest<'a>(v: &'a [Sample], t: f64) -> &'a Sample {
    let i = v.partition_point(|s| s.t < t);
    let a = v.get(i.saturating_sub(1)).unwrap();
    let b = v.get(i).unwrap_or(a);
    if (a.t - t).abs() <= (b.t - t).abs() { a } else { b }
}

pub fn lag_scan(a: &[Sample], b: &[Sample], lo: f64, hi: f64, step: f64, window: (f64, f64)) {
    println!("{:>10} {:>12} {:>12} {:>8}", "lag_s", "median_m", "p90_m", "n");
    let mut best = (f64::INFINITY, 0.0);
    let mut l = lo;
    while l <= hi + 1e-9 {
        let mut d: Vec<f64> = Vec::new();
        for s in a {
            if s.t < window.0 || s.t > window.1 {
                continue;
            }
            let t = s.t + l;
            if t < b[0].t || t > b[b.len() - 1].t {
                continue;
            }
            let o = nearest(b, t);
            d.push(((s.x - o.x).powi(2) + (s.y - o.y).powi(2) + (s.z - o.z).powi(2)).sqrt());
        }
        if !d.is_empty() {
            d.sort_by(|x, y| x.partial_cmp(y).unwrap());
            let med = d[d.len() / 2];
            println!("{:>10.3} {:>12.3} {:>12.3} {:>8}", l, med, d[d.len() * 9 / 10], d.len());
            if med < best.0 {
                best = (med, l);
            }
        }
        l += step;
    }
    println!("# best lag {:.3} s at median {:.3} m", best.1, best.0);
}

pub fn report(a: &[Sample], b: &[Sample], step: f64) {
    let mut firsts: Vec<(f64, Option<f64>)> = vec![(0.5, None), (2.0, None), (10.0, None), (100.0, None)];
    println!("{:>10} {:>12} {:>10} {:>28} {:>28}", "race_s", "dist_m", "dt_ms", "A (x,y,z)", "B (x,y,z)");
    let mut want = f64::NEG_INFINITY;
    let mut maxgap: f64 = 0.0;
    for s in a {
        if s.t < b[0].t || s.t > b[b.len() - 1].t {
            continue;
        }
        let o = nearest(b, s.t);
        let d = ((s.x - o.x).powi(2) + (s.y - o.y).powi(2) + (s.z - o.z).powi(2)).sqrt();
        maxgap = maxgap.max((o.t - s.t).abs());
        for f in firsts.iter_mut() {
            if f.1.is_none() && d > f.0 {
                f.1 = Some(s.t);
            }
        }
        if s.t < want {
            continue;
        }
        want = if step > 0.0 { s.t + step - 1e-9 } else { f64::NEG_INFINITY };
        println!(
            "{:>10.3} {:>12.3} {:>10.0} {:>28} {:>28}",
            s.t, d, (o.t - s.t) * 1000.0,
            format!("{:.1},{:.1},{:.1}", s.x, s.y, s.z),
            format!("{:.1},{:.1},{:.1}", o.x, o.y, o.z)
        );
    }
    println!("# worst sample-pairing gap {:.0} ms", maxgap * 1000.0);
    for (th, t) in firsts {
        match t {
            Some(t) => println!("# first exceeds {:>6.1} m at {:.3} s", th, t),
            None => println!("# never exceeds {:>6.1} m", th),
        }
    }
}
