//! The video's speed against a simulated tape's speed, in race time.
//!
//! This is the whole reconstruction loop in one table: what the run did, and
//! what a candidate tape does, at the same instants. The number that matters
//! is not the mean error over the run — a reconstruction is right until it is
//! wrong, and after that the two cars are in different places and the
//! difference means nothing. What this reports is where it STOPS tracking.

use std::collections::BTreeMap;
use std::io::Write;

/// race_ms -> km/h from an `fk trace` CSV.
pub fn load_engine(path: &str) -> Result<BTreeMap<i64, f64>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut lines = txt.lines();
    let hdr: Vec<&str> = lines.next().ok_or("empty")?.split(',').collect();
    let ct = hdr.iter().position(|h| *h == "time_ms").ok_or("no time_ms")?;
    let cs = hdr.iter().position(|h| *h == "speed_kmh").ok_or("no speed_kmh")?;
    let mut m = BTreeMap::new();
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        if let (Some(Ok(t)), Some(Ok(v))) =
            (f.get(ct).map(|x| x.parse::<i64>()), f.get(cs).map(|x| x.parse::<f64>()))
        {
            m.insert(t, v);
        }
    }
    Ok(m)
}

/// `video` is race_ms -> km/h, the readout the video shows; `engine` is the
/// same for a simulated tape. `tol` is how far apart they may be, and `run` is
/// how many consecutive comparable samples must exceed it before the
/// reconstruction is called diverged (one bad OCR frame is not a divergence).
pub fn report(
    video: &BTreeMap<i64, f64>,
    engine: &BTreeMap<i64, f64>,
    tol: f64,
    run: usize,
    tol_ms: i64,
    o: &mut impl Write,
) {
    let mut bad = 0usize;
    let mut diverged: Option<i64> = None;
    let mut n = 0usize;
    let mut sum = 0.0;
    let mut worst_ok = 0.0f64;
    writeln!(o, "race_ms\tvideo_kmh\tengine_kmh\tdiff").unwrap();
    for (t, v) in video {
        let mut near: Option<(i64, f64)> = None;
        for (u, e) in engine.range(t - tol_ms..=t + tol_ms) {
            let d = (u - t).abs();
            if near.map_or(true, |(bd, _)| d < bd) {
                near = Some((d, *e));
            }
        }
        let Some((_, e)) = near else { continue };
        let d = e - v;
        writeln!(o, "{t}\t{v}\t{:.1}\t{:+.1}", e, d).unwrap();
        if diverged.is_none() {
            n += 1;
            sum += d.abs();
            if d.abs() > tol {
                bad += 1;
                if bad >= run {
                    diverged = Some(*t);
                }
            } else {
                bad = 0;
                worst_ok = worst_ok.max(d.abs());
            }
        }
    }
    match diverged {
        Some(t) => writeln!(
            o,
            "# tracks to race {:.3} s, then {} consecutive samples over {:.0} km/h apart",
            t as f64 / 1000.0,
            run,
            tol
        )
        .unwrap(),
        None => writeln!(o, "# never diverges by more than {:.0} km/h", tol).unwrap(),
    }
    if n > 0 {
        writeln!(
            o,
            "# over the tracking window: {} comparable samples, mean |diff| {:.2} km/h",
            n,
            sum / n as f64
        )
        .unwrap();
    }
}
