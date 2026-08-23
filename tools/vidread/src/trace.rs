//! A `vidread read` table turned into a numeric series, with the readings that
//! cannot be true removed.
//!
//! Two filters, and they are different in kind. The score filter drops a
//! reading the matcher itself is unsure of. The plausibility filter drops a
//! reading the matcher was CONFIDENT about and that the car cannot have done —
//! the failure mode that matters, because a confident wrong digit is invisible
//! in the score column.

use std::io::{BufRead, Write};

pub struct Sample {
    pub t: f64,
    pub v: Option<f64>,
    pub score: f32,
}

pub fn parse(r: impl BufRead, min_score: f32) -> Vec<Sample> {
    let mut out = Vec::new();
    for line in r.lines() {
        let line = line.expect("read");
        if line.starts_with('t') || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 3 {
            continue;
        }
        let t: f64 = f[0].parse().unwrap();
        let score: f32 = f[2].parse().unwrap_or(0.0);
        let digits: String = f[1].chars().filter(|c| c.is_ascii_digit()).collect();
        let bad = f[1].contains('?') || digits.is_empty();
        // A blank must be a LEADING blank: "1_2" is a misread, not a number.
        let shape_ok = !f[1].trim_end_matches(|c: char| c.is_ascii_digit()).contains('_')
            || f[1].starts_with('_');
        let leading_blanks = f[1].len() - f[1].trim_start_matches('_').len();
        let shape_ok = shape_ok && f[1][leading_blanks..].chars().all(|c| c.is_ascii_digit());
        let v = (!bad && shape_ok && score >= min_score).then(|| digits.parse::<f64>().unwrap());
        out.push(Sample { t, v, score });
    }
    out
}

/// Drop any reading that differs from the median of its neighbours by more
/// than `tol`. `half` is the half-width of the neighbourhood, in samples.
pub fn despike(s: &mut [Sample], half: usize, tol: f64) -> usize {
    let orig: Vec<Option<f64>> = s.iter().map(|x| x.v).collect();
    let mut dropped = 0;
    for i in 0..s.len() {
        if orig[i].is_none() {
            continue;
        }
        let lo = i.saturating_sub(half);
        let hi = (i + half + 1).min(orig.len());
        let mut near: Vec<f64> =
            (lo..hi).filter(|&j| j != i).filter_map(|j| orig[j]).collect();
        if near.len() < 3 {
            continue;
        }
        near.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = near[near.len() / 2];
        if (orig[i].unwrap() - med).abs() > tol {
            s[i].v = None;
            dropped += 1;
        }
    }
    dropped
}

pub fn print(s: &[Sample], o: &mut impl Write) {
    let mut have = 0;
    writeln!(o, "t\tkmh").unwrap();
    for x in s {
        match x.v {
            Some(v) => {
                have += 1;
                writeln!(o, "{:.4}\t{}", x.t, v as i64).unwrap();
            }
            None => writeln!(o, "{:.4}\t", x.t).unwrap(),
        }
    }
    writeln!(o, "# {have} of {} frames read", s.len()).unwrap();
}
