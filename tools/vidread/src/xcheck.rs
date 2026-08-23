//! Does a clip of the run's CONSTRUCTION show the same run as the finished
//! replay?
//!
//! The clips are filmed at different playback speeds and at different stages
//! of the build, so "same race time" does not imply "same run". The test that
//! decides it is physical rather than cosmetic: read the clip's own race clock
//! and its own speed readout, look up the finished replay's speed at that race
//! time, and compare. Two renderings of the same run agree; a clip of an
//! earlier build does not.
//!
//! The finished replay's trace is the reference because it is the only
//! continuous, uncut, real-time recording of the run in the video.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};

/// race_ms -> km/h, from a `vidread trace` table whose t column is race time.
pub fn load_reference(r: impl BufRead) -> BTreeMap<i64, f64> {
    let mut m = BTreeMap::new();
    for line in r.lines() {
        let line = line.expect("read");
        if line.starts_with('t') || line.starts_with('#') || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 2 || f[1].is_empty() {
            continue;
        }
        let t: f64 = f[0].parse().unwrap();
        m.insert((t * 1000.0).round() as i64, f[1].parse().unwrap());
    }
    m
}

/// Nearest reference sample within `win_ms`.
fn near(m: &BTreeMap<i64, f64>, ms: i64, win_ms: i64) -> Option<f64> {
    let mut best: Option<(i64, f64)> = None;
    for (k, v) in m.range(ms - win_ms..=ms + win_ms) {
        let d = (k - ms).abs();
        if best.map_or(true, |(bd, _)| d < bd) {
            best = Some((d, *v));
        }
    }
    best.map(|(_, v)| v)
}

pub struct Pair {
    pub video_t: f64,
    pub race_ms: i64,
    pub clip_kmh: f64,
    pub ref_kmh: f64,
}

/// `clock` and `speed` are two `vidread read` tables over the SAME frames.
pub fn pairs(
    clock: impl BufRead,
    speed: impl BufRead,
    reference: &BTreeMap<i64, f64>,
    min_clock: f32,
    min_speed: f32,
    win_ms: i64,
) -> Vec<Pair> {
    let sp: Vec<(String, f32)> = speed
        .lines()
        .map(|l| l.expect("read"))
        .filter(|l| !(l.starts_with('t') || l.starts_with('#') || l.is_empty()))
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f[1].to_string(), f[2].parse().unwrap_or(0.0))
        })
        .collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    for line in clock.lines() {
        let line = line.expect("read");
        if line.starts_with('t') || line.starts_with('#') || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        let idx = i;
        i += 1;
        if idx >= sp.len() || f.len() < 3 {
            continue;
        }
        if f[2].parse::<f32>().unwrap_or(0.0) < min_clock || sp[idx].1 < min_speed {
            continue;
        }
        let Some(ms) = crate::keytape::race_ms(f[1]) else { continue };
        let digits: String = sp[idx].0.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() || sp[idx].0.contains('?') {
            continue;
        }
        let Some(rv) = near(reference, ms, win_ms) else { continue };
        out.push(Pair {
            video_t: f[0].parse().unwrap(),
            race_ms: ms,
            clip_kmh: digits.parse().unwrap(),
            ref_kmh: rv,
        });
    }
    out
}

pub fn report(p: &[Pair], near_kmh: f64, o: &mut impl Write) {
    writeln!(o, "video_t\trace_ms\tclip_kmh\treplay_kmh\tdiff").unwrap();
    let mut d: Vec<f64> = Vec::with_capacity(p.len());
    for x in p {
        writeln!(
            o,
            "{:.3}\t{}\t{}\t{}\t{}",
            x.video_t, x.race_ms, x.clip_kmh as i64, x.ref_kmh as i64, (x.clip_kmh - x.ref_kmh) as i64
        )
        .unwrap();
        d.push((x.clip_kmh - x.ref_kmh).abs());
    }
    if d.is_empty() {
        writeln!(o, "# no comparable frames").unwrap();
        return;
    }
    let agree = d.iter().filter(|x| **x <= near_kmh).count();
    d.sort_by(|a, b| a.partial_cmp(b).unwrap());
    writeln!(
        o,
        "# {} comparable frames, median |diff| {:.0} km/h, {:.1}% within {:.0} km/h",
        d.len(),
        d[d.len() / 2],
        100.0 * agree as f64 / d.len() as f64,
        near_kmh
    )
    .unwrap();
}
