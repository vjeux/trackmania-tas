//! Turn the joined table (race clock + key lamps, per video frame) into an
//! input record indexed by RACE time — the thing a simulator can be handed.
//!
//! Two things make this more than a reformat:
//!
//!  * the video plays the run at speeds between about 0.1x and 1x, so several
//!    frames can land on one 10 ms tick, and their lamps must AGREE. A
//!    disagreement is either a misread clock or a transition inside the tick,
//!    and both are reported rather than averaged away.
//!  * the same race time appears in several clips, filmed at different stages
//!    of the run's construction. Where two clips disagree the run CHANGED
//!    between them, and that is a finding, not noise.

use std::io::{BufRead, Write};

pub struct Row {
    pub t: f64,
    pub race_ms: i64,
    pub score: f32,
    pub keys: [bool; 5],
}

/// `MSShh` -> milliseconds. Returns None if any cell is not a digit.
pub fn race_ms(v: &str) -> Option<i64> {
    let d: Vec<u32> = v.chars().map(|c| c.to_digit(10)).collect::<Option<_>>()?;
    if d.len() != 5 {
        return None;
    }
    Some((d[0] as i64) * 60_000 + (d[1] as i64) * 10_000 + (d[2] as i64) * 1000 + (d[3] as i64) * 100 + (d[4] as i64) * 10)
}

pub fn parse(r: impl BufRead, min_score: f32) -> Vec<Row> {
    let mut out = Vec::new();
    for line in r.lines() {
        let line = line.expect("read");
        if line.starts_with('t') || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 12 {
            continue;
        }
        let score: f32 = f[2].parse().unwrap_or(0.0);
        if score < min_score || f[6] != "1" {
            continue;
        }
        let Some(ms) = race_ms(f[1]) else { continue };
        let mut keys = [false; 5];
        for k in 0..5 {
            keys[k] = f[7 + k] == "1";
        }
        out.push(Row { t: f[0].parse().unwrap(), race_ms: ms, score, keys });
    }
    out
}

/// The same record from a plain `lamps` table, placed in race time by an
/// alignment instead of by the clip's own clock: `race_ms = ms0 + (t-t0)*1000*rate`.
/// Used on clips the editor reframed, where the game's clock is somewhere this
/// tool cannot read but the speed readout still is.
pub fn parse_aligned(
    r: impl BufRead,
    rate: f64,
    ms0: f64,
    t0: f64,
    from: f64,
    to: f64,
) -> Vec<Row> {
    let mut out = Vec::new();
    for line in r.lines() {
        let line = line.expect("read");
        if line.starts_with('t') || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 7 || f[1] != "1" {
            continue;
        }
        let t: f64 = f[0].parse().unwrap();
        if t < from || t > to {
            continue;
        }
        let ms = ((ms0 + (t - t0) * 1000.0 * rate) / 10.0).round() as i64 * 10;
        let mut keys = [false; 5];
        for k in 0..5 {
            keys[k] = f[2 + k] == "1";
        }
        out.push(Row { t, race_ms: ms, score: 1.0, keys });
    }
    out
}

/// Two independently recovered records of the same race window, compared tick
/// by tick. Two clips filmed at different playback speeds, read through the
/// same lamps but placed by different instruments, either agree or they do
/// not; nothing here can make them agree by construction.
pub fn compare(
    a: &std::collections::BTreeMap<i64, Tick>,
    b: &std::collections::BTreeMap<i64, Tick>,
    o: &mut impl Write,
) {
    let mut both = 0usize;
    let mut same = 0usize;
    let mut per_key = [0usize; 5];
    for (ms, ta) in a {
        if let Some(tb) = b.get(ms) {
            both += 1;
            if ta.keys == tb.keys {
                same += 1;
            }
            for k in 0..5 {
                if ta.keys[k] == tb.keys[k] {
                    per_key[k] += 1;
                }
            }
        }
    }
    if both == 0 {
        writeln!(o, "# the two records share no tick").unwrap();
        return;
    }
    writeln!(
        o,
        "# {both} shared ticks, all five lamps agree on {} of them ({:.1}%)",
        same,
        100.0 * same as f64 / both as f64
    )
    .unwrap();
    for k in 0..5 {
        writeln!(
            o,
            "#   {:<6} agrees on {:.1}%",
            crate::lamps::NAMES[k],
            100.0 * per_key[k] as f64 / both as f64
        )
        .unwrap();
    }
}

/// Keep only rows that sit on a locally monotone clock: a misread clock jumps.
/// A row survives if at least `need` of the 8 rows around it are consistent
/// with it under a common, non-negative playback rate.
pub fn monotone_filter(rows: &[Row], half: usize, tol_ms: f64) -> Vec<usize> {
    let mut keep = Vec::new();
    for i in 0..rows.len() {
        let lo = i.saturating_sub(half);
        let hi = (i + half + 1).min(rows.len());
        // local rate from the median of pairwise slopes against row i
        let mut slopes: Vec<f64> = Vec::new();
        for j in lo..hi {
            if j == i {
                continue;
            }
            let dt = rows[j].t - rows[i].t;
            if dt.abs() > 1e-6 {
                slopes.push((rows[j].race_ms - rows[i].race_ms) as f64 / (dt * 1000.0));
            }
        }
        if slopes.len() < 4 {
            continue;
        }
        slopes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let rate = slopes[slopes.len() / 2];
        if !(0.0..=1.5).contains(&rate) {
            continue;
        }
        let agree = (lo..hi)
            .filter(|&j| {
                let pred = rows[i].race_ms as f64 + rate * (rows[j].t - rows[i].t) * 1000.0;
                (pred - rows[j].race_ms as f64).abs() <= tol_ms
            })
            .count();
        if agree * 2 >= hi - lo {
            keep.push(i);
        }
    }
    keep
}

pub struct Tick {
    pub keys: [bool; 5],
    pub votes: usize,
    pub conflicts: usize,
    pub first_t: f64,
}

/// Collapse to one entry per 10 ms tick, counting disagreements.
pub fn collapse(rows: &[Row], keep: &[usize]) -> std::collections::BTreeMap<i64, Tick> {
    let mut m: std::collections::BTreeMap<i64, Tick> = Default::default();
    for &i in keep {
        let r = &rows[i];
        let e = m.entry(r.race_ms).or_insert(Tick {
            keys: r.keys,
            votes: 0,
            conflicts: 0,
            first_t: r.t,
        });
        if e.votes > 0 && e.keys != r.keys {
            e.conflicts += 1;
        }
        e.votes += 1;
    }
    m
}

pub fn print(m: &std::collections::BTreeMap<i64, Tick>, o: &mut impl Write) {
    writeln!(o, "race_ms\tbrake\tup\tdown\tleft\tright\tvotes\tconflicts\tvideo_t").unwrap();
    let mut conflicted = 0;
    for (ms, t) in m {
        if t.conflicts > 0 {
            conflicted += 1;
        }
        writeln!(
            o,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}",
            ms,
            t.keys[0] as u8,
            t.keys[1] as u8,
            t.keys[2] as u8,
            t.keys[3] as u8,
            t.keys[4] as u8,
            t.votes,
            t.conflicts,
            t.first_t
        )
        .unwrap();
    }
    let span = match (m.keys().next(), m.keys().next_back()) {
        (Some(a), Some(b)) => (*b - *a) as f64 / 1000.0,
        _ => 0.0,
    };
    writeln!(
        o,
        "# {} ticks over a {:.3} s race-time span, {} of them with disagreeing frames",
        m.len(),
        span,
        conflicted
    )
    .unwrap();
}

/// Read back a record this module printed.
pub fn load_record(path: &str) -> std::collections::BTreeMap<i64, Tick> {
    let txt = std::fs::read_to_string(path).expect("record");
    let mut m: std::collections::BTreeMap<i64, Tick> = Default::default();
    for line in txt.lines().skip(1) {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        let mut keys = [false; 5];
        for k in 0..5 {
            keys[k] = f[1 + k] == "1";
        }
        m.insert(
            f[0].parse().unwrap(),
            Tick {
                keys,
                votes: f[6].parse().unwrap_or(1),
                conflicts: f[7].parse().unwrap_or(0),
                first_t: f[8].parse().unwrap_or(0.0),
            },
        );
    }
    m
}

/// The same comparison over a sweep of tick shifts. A record placed by a
/// slightly wrong rate or offset is not wrong, it is displaced: if one shift
/// makes all five channels agree and its neighbours do not, that is a
/// measurement of the displacement AND evidence that both records read the
/// same run.
pub fn compare_shifts(
    a: &std::collections::BTreeMap<i64, Tick>,
    b: &std::collections::BTreeMap<i64, Tick>,
    max_ticks: i64,
    o: &mut impl Write,
) {
    writeln!(o, "shift_ms\tshared\tall5\tbrake\tup\tdown\tleft\tright").unwrap();
    for s in -max_ticks..=max_ticks {
        let d = s * 10;
        let mut both = 0usize;
        let mut same = 0usize;
        let mut per = [0usize; 5];
        for (ms, ta) in a {
            if let Some(tb) = b.get(&(ms + d)) {
                both += 1;
                if ta.keys == tb.keys {
                    same += 1;
                }
                for k in 0..5 {
                    if ta.keys[k] == tb.keys[k] {
                        per[k] += 1;
                    }
                }
            }
        }
        if both < 10 {
            continue;
        }
        let p = |x: usize| 100.0 * x as f64 / both as f64;
        writeln!(
            o,
            "{d}\t{both}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.1}",
            p(same),
            p(per[0]),
            p(per[1]),
            p(per[2]),
            p(per[3]),
            p(per[4])
        )
        .unwrap();
    }
}

/// The record as input EVENTS -- press/release with a race timestamp, which is
/// how a TAS input script is written and how a human reads one. Gaps in the
/// record (ticks the video does not show) are printed as gaps rather than
/// silently bridged: an event list that pretends to be continuous is a
/// different claim from one that says where it stops looking.
pub fn events(m: &std::collections::BTreeMap<i64, Tick>, o: &mut impl Write) {
    let names = ["brake", "gas", "brake2", "left", "right"];
    let mut prev: Option<(i64, [bool; 5])> = None;
    for (ms, t) in m {
        match prev {
            None => {
                writeln!(o, "{ms} record starts").unwrap();
                for k in [1usize, 2, 3, 4] {
                    if t.keys[k] {
                        writeln!(o, "{ms} press {}", names[k]).unwrap();
                    }
                }
            }
            Some((pms, pk)) => {
                if ms - pms > 10 {
                    writeln!(o, "{pms} gap until {ms}").unwrap();
                }
                for k in [1usize, 2, 3, 4] {
                    if t.keys[k] != pk[k] {
                        writeln!(o, "{ms} {} {}", if t.keys[k] { "press" } else { "release" }, names[k])
                            .unwrap();
                    }
                }
            }
        }
        prev = Some((*ms, t.keys));
    }
    if let Some((pms, _)) = prev {
        writeln!(o, "{pms} record ends").unwrap();
    }
}

/// The record as an event list that can be SPLICED into another script: the
/// events, plus an explicit statement of the window they are authoritative
/// over. Everything outside that window is somebody else's business, and a
/// consumer that silently extends the record past its last observed tick is
/// making a claim the video does not support.
pub fn window(m: &std::collections::BTreeMap<i64, Tick>) -> Option<(i64, i64)> {
    Some((*m.keys().next()?, *m.keys().next_back()?))
}
