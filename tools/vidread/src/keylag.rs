//! Do the recovered key states line up with the video's own acceleration, and
//! do they line up HERE rather than anywhere?
//!
//! `keyphys` asks whether brake ticks decelerate. That is a real question but a
//! weak one: a record shifted by half a second, or a record of a different
//! part of the run, can pass it, because braking and slowing are correlated
//! over long stretches whatever the alignment.
//!
//! This asks the sharper question. Slide the recovered record against the
//! video's speed trace over a range of lags and, at each one, measure how far
//! apart the braking and non-braking ticks' accelerations are. A record that
//! is genuinely this run's inputs, placed correctly, separates them best at
//! lag zero and worse either side. A record placed by luck separates them the
//! same everywhere — the curve is flat, and flatness is the null.
//!
//! The lag axis is its own control: no shuffling is needed, because every lag
//! but one IS a shuffle of the same data against the same trace.

use crate::keytape::Tick;
use std::collections::BTreeMap;
use std::io::Write;

fn accel_at(speed: &BTreeMap<i64, f64>, ms: i64, window_ms: i64, tol_ms: i64) -> Option<f64> {
    let at = |t: i64| -> Option<f64> {
        let mut best: Option<(i64, f64)> = None;
        for (k, v) in speed.range(t - tol_ms..=t + tol_ms) {
            let d = (k - t).abs();
            if best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, *v));
            }
        }
        best.map(|(_, v)| v)
    };
    Some(at(ms + window_ms)? - at(ms)?)
}

/// For each lag, the mean acceleration of non-braking ticks minus that of
/// braking ticks — positive means braking ticks lose speed relative to the
/// rest, which is the sign physics requires.
/// How many CONTIGUOUS blocks the chosen channel is on for. This is the
/// test's real sample size: twenty-one consecutive braking ticks are one
/// event observed twenty-one times, not twenty-one observations, and a single
/// block slid along a trace will find whatever large deceleration is nearby
/// and report it as a peak. Counting blocks is what stops that reading as
/// evidence.
fn blocks(rec: &BTreeMap<i64, Tick>, on: impl Fn(&Tick) -> bool) -> (usize, i64) {
    let mut n = 0;
    let mut prev = false;
    let mut prev_ms = i64::MIN;
    let (mut first, mut last) = (i64::MAX, i64::MIN);
    for (ms, t) in rec {
        let v = on(t);
        if v {
            if !prev || ms - prev_ms > 10 {
                n += 1;
            }
            first = first.min(*ms);
            last = last.max(*ms);
        }
        prev = v;
        prev_ms = *ms;
    }
    (n, if n > 0 { last - first } else { 0 })
}

pub fn report(
    rec: &BTreeMap<i64, Tick>,
    speed: &BTreeMap<i64, f64>,
    lag_max: i64,
    lag_step: i64,
    window_ms: i64,
    tol_ms: i64,
    o: &mut impl Write,
) {
    let (nb, spread) = blocks(rec, |t| t.keys[0] || t.keys[2]);
    writeln!(
        o,
        "# the braking ticks form {nb} contiguous block(s) spanning {:.3} s of race time",
        spread as f64 / 1000.0
    )
    .unwrap();
    writeln!(o, "lag_ms\tbrake_n\tfree_n\tbrake_dv\tfree_dv\tseparation").unwrap();
    let mut best = (i64::MIN, f64::MIN);
    let mut rows: Vec<(i64, f64)> = Vec::new();
    let mut lag = -lag_max;
    while lag <= lag_max {
        let (mut bn, mut fnn) = (0usize, 0usize);
        let (mut bs, mut fs) = (0.0, 0.0);
        for (ms, t) in rec {
            let Some(dv) = accel_at(speed, ms + lag, window_ms, tol_ms) else { continue };
            if t.keys[0] || t.keys[2] {
                bn += 1;
                bs += dv;
            } else {
                fnn += 1;
                fs += dv;
            }
        }
        if bn >= 10 && fnn >= 10 {
            let (b, f) = (bs / bn as f64, fs / fnn as f64);
            let sep = f - b;
            writeln!(o, "{lag}\t{bn}\t{fnn}\t{:+.3}\t{:+.3}\t{:+.3}", b, f, sep).unwrap();
            rows.push((lag, sep));
            if sep > best.1 {
                best = (lag, sep);
            }
        }
        lag += lag_step;
    }
    if rows.is_empty() {
        writeln!(o, "# no lag has enough braking AND non-braking ticks to compare").unwrap();
        return;
    }
    // SPREAD, not count, is the sample size. Nine blocks inside four tenths of
    // a second are one braking EVENT chopped up by the frames the reader could
    // not place -- and one event slid along a trace finds whatever large
    // deceleration is nearby and reports it as a peak.
    if nb < 3 || spread < 1500 {
        writeln!(
            o,
            "# UNDERPOWERED: {nb} block(s) inside {:.3} s. That is ONE braking event,\n\
             # and one event has one degree of freedom against the trace, so a peak\n\
             # anywhere -- at a lag of zero as much as anywhere else -- is as likely\n\
             # to be the nearest large deceleration as it is to be this run's own\n\
             # braking. Not evidence, either way.",
            spread as f64 / 1000.0
        )
        .unwrap();
    }
    let far: Vec<f64> =
        rows.iter().filter(|(l, _)| (l - best.0).abs() >= 400).map(|(_, s)| *s).collect();
    writeln!(o, "# best separation {:+.3} km/h per {window_ms} ms, at lag {} ms", best.1, best.0)
        .unwrap();
    if far.len() >= 5 {
        let mean = far.iter().sum::<f64>() / far.len() as f64;
        let sd = (far.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / far.len() as f64).sqrt();
        writeln!(
            o,
            "# lags 400 ms or more away: mean {:+.3}, sd {:.3} over {} lags -- the peak is {:.1} sd above them",
            mean,
            sd,
            far.len(),
            if sd > 1e-9 { (best.1 - mean) / sd } else { f64::NAN }
        )
        .unwrap();
    }
}
