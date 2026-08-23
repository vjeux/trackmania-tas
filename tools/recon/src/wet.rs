//! Score a candidate on the wetness readout instead of the speedometer.
//!
//! Speed is a scalar per instant, and that is exactly why it walls: two cars
//! at the same speed can be metres apart, so a search buys tracking with a
//! line that is going somewhere else. The human corridor fixes that where the
//! run follows the human route — and says nothing past the point where it
//! stops, which is most of this run.
//!
//! Wetness reaches there, because it is not a function of the car's state at
//! all. It is a **positional integral**: a function of where the car has BEEN.
//! Two candidates that agree in speed at an instant but crossed different
//! water differ in it, and the difference persists for seconds — it decays at
//! ten points a second, so a soaked car carries the fact for ten. Between two
//! human replays of this map it differs by more than 10 points at 49 % of
//! their shared instants.
//!
//! The comparison is the same shape as the speed one and for the same reason:
//! against the closest VALUE inside a timing window, never the nearest
//! instant. The readout is a step function held for a 50 ms engine sample and
//! sampled by a 60 Hz camera, so a nearest-instant rule scores the sample
//! boundaries and not the run.

use std::collections::BTreeMap;

/// Race milliseconds to percent, 0..100.
pub type Wet = BTreeMap<i64, f64>;

/// A decoded video series: `t<TAB>pct`, seconds and integer percent, blanks
/// for the frames the reader refused.
///
/// The header is REQUIRED, and that is not fussiness. A two-column telemetry
/// file — race milliseconds and a 0..1 fraction — parses perfectly well as this
/// format and yields a series at 9000 seconds reading 1 %, silently. A loader
/// that cannot fail is a loader that returns the wrong units.
pub fn load_video(path: &str) -> Option<Wet> {
    let txt = std::fs::read_to_string(path).ok()?;
    if !txt.starts_with("t\tpct\t") {
        return None;
    }
    let mut m = Wet::new();
    for l in txt.lines() {
        if l.starts_with('#') || l.starts_with("t\t") {
            continue;
        }
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 2 || f[1].is_empty() {
            continue;
        }
        if let (Ok(t), Ok(v)) = (f[0].parse::<f64>(), f[1].parse::<f64>()) {
            m.insert((t * 1000.0).round() as i64, v);
        }
    }
    (!m.is_empty()).then_some(m)
}

/// A simulator or recording series. Two shapes are accepted because two exist:
/// an `fk trace` CSV with a `wetness` column, and the two-column TSV of race
/// milliseconds and a 0..1 fraction that a decoded ghost recording yields.
/// Both are converted to percent here so nothing downstream has to ask which
/// it is holding — this project has been bitten by a unit carried in a
/// variable name before.
pub fn load_series(path: &str) -> Option<Wet> {
    let txt = std::fs::read_to_string(path).ok()?;
    let mut lines = txt.lines().peekable();
    let hdr = *lines.peek()?;
    if hdr.starts_with("time_ms,") {
        let cols: Vec<&str> = hdr.split(',').collect();
        let ct = cols.iter().position(|h| *h == "time_ms")?;
        let cw = cols.iter().position(|h| *h == "wetness")?;
        lines.next();
        let mut m = Wet::new();
        for l in lines {
            let f: Vec<&str> = l.split(',').collect();
            if let (Some(Ok(t)), Some(Ok(v))) =
                (f.get(ct).map(|x| x.parse::<i64>()), f.get(cw).map(|x| x.parse::<f64>()))
            {
                m.insert(t, v * 100.0);
            }
        }
        return (!m.is_empty()).then_some(m);
    }
    let mut m = Wet::new();
    for l in lines {
        if l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() < 2 {
            continue;
        }
        if let (Ok(t), Ok(v)) = (f[0].parse::<f64>(), f[1].parse::<f64>()) {
            m.insert(t.round() as i64, v * 100.0);
        }
    }
    (!m.is_empty()).then_some(m)
}

/// Shift a series in race time. The wetness gate's own control: the gate must
/// be able to FIRE, and a candidate that satisfies it is only evidence once a
/// deliberately wrong series has been refused by the same code on the same
/// tape. Two seconds is far more than the 40 ms by which two real runs' resets
/// differ and far less than the gaps between the video's readable stretches.
pub fn shift(w: &Wet, ms: i64) -> Wet {
    w.iter().map(|(t, v)| (t + ms, *v)).collect()
}

/// Assert a value across a race-time window at `fps`, for instants the reader
/// could not read.
///
/// **This is not a reading and it must never be presented as one.** It exists
/// because the HUD stops drawing the readout when there is nothing to draw, so
/// the longest dry stretch of the run — which is exactly where the search is
/// working — comes back as an absence. An absence is a refusal; an assertion
/// is a claim, and a claim needs its supports named at the call site.
pub fn assert_band(w: &Wet, from_ms: i64, to_ms: i64, pct: f64, fps: f64) -> Wet {
    let mut out = w.clone();
    let step = (1000.0 / fps).round() as i64;
    let mut t = from_ms;
    while t <= to_ms {
        out.entry(t).or_insert(pct);
        t += step;
    }
    out
}

/// The race time at which `eng` stops reproducing the video's wetness, or
/// `None` if it never does.
///
/// `run` consecutive disagreements are required before the verdict, so one
/// unlucky frame at a step edge cannot end a candidate — the same rule the
/// speed scorer uses, for the same reason.
pub fn departs(video: &Wet, eng: &Wet, tol: f64, run: usize, match_ms: i64, from_ms: i64) -> Option<i64> {
    let mut bad = 0usize;
    let mut last_ok = from_ms;
    for (t, v) in video.range(from_ms..) {
        let mut near: Option<f64> = None;
        for (_, e) in eng.range(t - match_ms..=t + match_ms) {
            if near.map_or(true, |b: f64| (e - v).abs() < (b - v).abs()) {
                near = Some(*e);
            }
        }
        let Some(e) = near else { continue };
        if (e - v).abs() > tol {
            bad += 1;
            if bad >= run {
                return Some(last_ok);
            }
        } else {
            bad = 0;
            last_ok = *t;
        }
    }
    None
}

/// Every shared instant, for a report rather than a gate.
pub struct Agreement {
    pub shared: usize,
    pub within: usize,
    pub mean_abs: f64,
    /// The last instant before the run of disagreements that ends the match,
    /// and the first instant of that run. They are different numbers and the
    /// distinction matters: the gate uses the first, a reader wants the
    /// second, and a report that prints one under the other's name invites
    /// exactly the misreading it caused here.
    pub last_agreed: Option<i64>,
    pub first_break: Option<i64>,
    /// Video value, matched engine value, per shared instant.
    pub rows: Vec<(i64, f64, f64)>,
}

pub fn agreement(video: &Wet, eng: &Wet, tol: f64, run: usize, match_ms: i64) -> Agreement {
    let mut shared = 0usize;
    let mut within = 0usize;
    let mut sum = 0.0;
    let mut rows = Vec::new();
    let mut bad = 0usize;
    let mut last_ok = None;
    let mut run_start = None;
    let mut broke: Option<(Option<i64>, i64)> = None;
    for (t, v) in video {
        let mut near: Option<f64> = None;
        for (_, e) in eng.range(t - match_ms..=t + match_ms) {
            if near.map_or(true, |b: f64| (e - v).abs() < (b - v).abs()) {
                near = Some(*e);
            }
        }
        let Some(e) = near else { continue };
        shared += 1;
        sum += (e - v).abs();
        rows.push((*t, *v, e));
        if (e - v).abs() <= tol {
            within += 1;
            bad = 0;
            run_start = None;
            last_ok = Some(*t);
        } else {
            if bad == 0 {
                run_start = Some(*t);
            }
            bad += 1;
            if bad == run && broke.is_none() {
                broke = Some((last_ok, run_start.unwrap()));
            }
        }
    }
    Agreement {
        shared,
        within,
        mean_abs: if shared > 0 { sum / shared as f64 } else { f64::NAN },
        last_agreed: broke.map(|b| b.0.unwrap_or(0)),
        first_break: broke.map(|b| b.1),
        rows,
    }
}
