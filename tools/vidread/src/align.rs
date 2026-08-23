//! Where in the run is this clip, and is it even the same run?
//!
//! The video shows the run being BUILT: dozens of clips, at playback speeds
//! between about 0.1x and 1x, most of them from earlier states of the tape.
//! Only one recording is continuous, real-time and final — the replay at the
//! end — so that one is the reference, and every other clip is placed against
//! it by its own speed readout.
//!
//! The alignment is a search over (rate, offset). Its value is not only the
//! placement: a clip of an EARLIER build has no placement that fits, and the
//! score says so. Two things make the answer trustworthy rather than a
//! best-effort argmax — the runner-up score at a clearly different offset is
//! reported beside the winner, and on clips whose own race clock is legible
//! the placement is checked against the clock, which is a wholly separate
//! instrument reading wholly different pixels.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};

pub struct Obs {
    pub t: f64,
    pub kmh: f64,
}

/// Read a `vidread read` speed table: video time and integer km/h.
pub fn load_clip(r: impl BufRead, min_score: f32) -> Vec<Obs> {
    let mut v = Vec::new();
    for line in r.lines() {
        let line = line.expect("read");
        if line.starts_with('t') || line.starts_with('#') || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 3 || f[2].parse::<f32>().unwrap_or(0.0) < min_score || f[1].contains('?') {
            continue;
        }
        let d: String = f[1].chars().filter(|c| c.is_ascii_digit()).collect();
        if d.is_empty() {
            continue;
        }
        v.push(Obs { t: f[0].parse().unwrap(), kmh: d.parse().unwrap() });
    }
    v
}

pub struct Fit {
    pub t0: f64,
    pub rate: f64,
    pub offset_ms: f64,
    pub score: f64,
    pub n: usize,
    pub runner: f64,
}

/// Best (rate, offset) placing `clip` on the reference, plus the best score
/// found at an offset more than `apart_ms` away from it.
pub fn fit(
    clip: &[Obs],
    reference: &BTreeMap<i64, f64>,
    rates: (f64, f64, f64),
    off: (f64, f64, f64),
    win_ms: i64,
    near_kmh: f64,
) -> Option<Fit> {
    if clip.len() < 20 {
        return None;
    }
    let t0 = clip[0].t;
    let mut best: Option<Fit> = None;
    let mut all: Vec<(f64, f64)> = Vec::new(); // (offset, score)
    let mut rate = rates.0;
    while rate <= rates.1 + 1e-9 {
        let mut o = off.0;
        while o <= off.1 + 1e-9 {
            let mut hit = 0.0f64;
            let mut n = 0usize;
            for c in clip {
                let ms = (o + (c.t - t0) * 1000.0 * rate).round() as i64;
                if ms < 0 {
                    continue;
                }
                let mut nearest: Option<(i64, f64)> = None;
                for (k, v) in reference.range(ms - win_ms..=ms + win_ms) {
                    let d = (k - ms).abs();
                    if nearest.map_or(true, |(bd, _)| d < bd) {
                        nearest = Some((d, *v));
                    }
                }
                if let Some((_, v)) = nearest {
                    n += 1;
                    if (v - c.kmh).abs() <= near_kmh {
                        hit += 1.0;
                    }
                }
            }
            if n >= clip.len() / 2 {
                let s = hit / n as f64;
                all.push((o, s));
                if best.as_ref().map_or(true, |b| s > b.score) {
                    best = Some(Fit { t0, rate, offset_ms: o, score: s, n, runner: 0.0 });
                }
            }
            o += off.2;
        }
        rate += rates.2;
    }
    let b = best.as_mut()?;
    b.runner = all
        .iter()
        .filter(|(o, _)| (o - b.offset_ms).abs() > 1500.0)
        .map(|(_, s)| *s)
        .fold(0.0, f64::max);
    best
}

pub fn print(name: &str, f: &Option<Fit>, o: &mut impl Write) {
    match f {
        Some(f) => writeln!(
            o,
            "{name}\tclip_t0 {:.4}\trate {:.3}\trace_start {:.3}\tfit {:.1}%\trunner-up {:.1}%\tframes {}",
            f.t0,
            f.rate,
            f.offset_ms / 1000.0,
            100.0 * f.score,
            100.0 * f.runner,
            f.n
        )
        .unwrap(),
        None => writeln!(o, "{name}\tno fit").unwrap(),
    }
}
