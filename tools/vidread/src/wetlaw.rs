//! The acceptance gate for a wetness series read off a screen.
//!
//! The run this reader is pointed at **cannot be simulated** — that is the
//! whole problem — so there is no ground truth to score a decode against. What
//! there is instead is a law the channel obeys, measured over human replays
//! that *can* be simulated:
//!
//! * every decrease is an exact integer number of 1/255 units (0 of 283 are
//!   not), so nothing between samples is interpolated;
//! * decreases come in exactly two kinds — gradual dry-out at 1–2 units per
//!   50 ms, and an instant reset to zero when the car leaves the water;
//! * gradual stretches run at 0.098–0.101 per second.
//!
//! On a 0–100 display at 60 Hz that says: a decrease is **at most one point
//! per frame**, or it is a step to zero. A mis-decoded tens digit is a 10–30
//! point step to something that is not zero, and a mis-decoded units digit
//! breaks the per-frame bound. Neither needs the run simulated.
//!
//! The rise side is bounded the same way and from the same data, because a car
//! entering water soaks far faster than it dries and a bound taken from the
//! dry-out rate would reject real wetting.
//!
//! **A gate is a claim like any other**, so this module also carries the two
//! controls that make it one: `envelope`, which reports what the human series
//! actually do (the gate's constants come from there, not from taste), and
//! `corrupt`, which injects exactly the defect the gate exists to catch. A gate
//! that passes a real series and fails a corrupted one in the same batch is
//! evidence; either half alone is not.

use std::io::Write;

/// One instant of a percentage series: a video time, and a reading if the
/// frame was readable.
pub type Series = Vec<(f64, Option<i32>)>;

/// Read a two-column TSV of `t<TAB>pct`, blank `pct` meaning "not readable".
/// A header line is skipped when its first field does not parse as a number.
pub fn load(text: &str, tcol: usize, vcol: usize) -> Series {
    let mut out = Series::new();
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        let Some(t) = f.get(tcol).and_then(|s| s.trim().parse::<f64>().ok()) else { continue };
        let v = f.get(vcol).and_then(|s| s.trim().parse::<i32>().ok());
        out.push((t, v));
    }
    out
}

/// Turn a simulator/telemetry series — race milliseconds and a 0..1 fraction,
/// sampled every 50 ms — into what a HUD would have shown at `fps`.
///
/// Zero-order hold, because the readout displays the last value the engine
/// computed rather than interpolating between samples. Both roundings are
/// offered: which one the game uses is not assumable, and this project has
/// already paid 17 percentage points for assuming one (`fk probe`, rounding).
pub fn from_telemetry(text: &str, fps: f64, truncate: bool) -> Series {
    let mut src: Vec<(f64, f64)> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 2 {
            continue;
        }
        if let (Ok(ms), Ok(v)) = (f[0].parse::<f64>(), f[1].parse::<f64>()) {
            src.push((ms / 1000.0, v));
        }
    }
    src.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out = Series::new();
    if src.is_empty() {
        return out;
    }
    let end = src[src.len() - 1].0;
    let mut j = 0usize;
    let mut i = 0u64;
    loop {
        let t = i as f64 / fps;
        if t > end {
            break;
        }
        while j + 1 < src.len() && src[j + 1].0 <= t {
            j += 1;
        }
        let p = src[j].1 * 100.0;
        out.push((t, Some(if truncate { p.floor() as i32 } else { p.round() as i32 })));
        i += 1;
    }
    out
}

/// What the series actually does, per adjacent readable pair. This is where
/// the gate's constants come from.
pub struct Envelope {
    pub pairs: usize,
    pub drops: usize,
    pub rises: usize,
    pub resets: usize,
    /// Steepest sustained fall and rise, in points per second.
    pub max_drop_rate: f64,
    pub max_rise_rate: f64,
    /// Largest single-frame step in each direction, in points.
    pub max_drop_step: i32,
    pub max_rise_step: i32,
}

pub fn envelope(s: &Series, max_gap: f64, reset_win: f64, reset_frac: f64) -> Envelope {
    let mut e = Envelope {
        pairs: 0,
        drops: 0,
        rises: 0,
        resets: 0,
        max_drop_rate: 0.0,
        max_rise_rate: 0.0,
        max_drop_step: 0,
        max_rise_step: 0,
    };
    let mut prev: Option<(f64, i32)> = None;
    for (i, (t, val)) in s.iter().enumerate() {
        let Some(val) = *val else { continue };
        if let Some((pt, pv)) = prev {
            let dt = t - pt;
            if dt > 0.0 && dt <= max_gap {
                e.pairs += 1;
                let d = val - pv;
                if d < 0 {
                    e.drops += 1;
                    let reset = s[i..]
                        .iter()
                        .take_while(|(u, _)| *u - pt <= reset_win)
                        .any(|(_, v)| v.is_some_and(|v| v <= (pv as f64 * reset_frac).floor() as i32));
                    if reset {
                        e.resets += 1;
                    } else {
                        e.max_drop_step = e.max_drop_step.max(-d);
                        e.max_drop_rate = e.max_drop_rate.max(-d as f64 / dt);
                    }
                } else if d > 0 {
                    e.rises += 1;
                    e.max_rise_step = e.max_rise_step.max(d);
                    e.max_rise_rate = e.max_rise_rate.max(d as f64 / dt);
                }
            }
        }
        prev = Some((*t, val));
    }
    e
}

/// A single violation, kept so a report can name the instant rather than only
/// count it.
pub struct Violation {
    pub t: f64,
    pub from: i32,
    pub to: i32,
    pub dt: f64,
    pub kind: &'static str,
}

pub struct Gate {
    /// Points per second a gradual dry-out may lose.
    pub drop_rate: f64,
    /// Points per second a car entering water may gain.
    pub rise_rate: f64,
    /// A gap longer than this is not a pair: the law says nothing across it.
    pub max_gap: f64,
    /// One point of slack, for the display's own rounding at a sample edge.
    pub slack: i32,
    /// The channel's own update period. The engine recomputes wetness every
    /// 50 ms and the HUD holds the last value, so at 60 Hz a whole sample's
    /// worth of change lands on ONE frame boundary. A gate that divides by the
    /// frame period instead of the sample period rejects real wetting: on the
    /// three human replays the largest true rise is 4 points, which is 80 pt/s
    /// over a sample and a nonsensical 240 pt/s over a frame.
    pub sample: f64,
    /// A fall that reaches zero within this is the car leaving the water, not
    /// drying. It takes one step or two — 202+53 /255 on one replay — so the
    /// test has to look AHEAD rather than ask whether this step landed on nil.
    pub reset_win: f64,
    /// How far a reset has to get, as a fraction of where it started.
    ///
    /// Zero would be the letter of the law, and it is the wrong test for a
    /// *video*: the HUD box stops being drawn as the readout empties, so the
    /// last frame of a real reset here reads 8, not 0. What separates a reset
    /// from a mis-decoded tens digit is not the destination but the shape —
    /// a reset keeps falling and a bad digit comes straight back up — so the
    /// test is "the fall completed", and the return trip is caught as a rise
    /// by the same gate.
    pub reset_frac: f64,
}

pub struct Report {
    pub readable: usize,
    pub pairs: usize,
    pub violations: Vec<Violation>,
    pub by_kind: Vec<(&'static str, usize)>,
}

impl Report {
    pub fn rate(&self) -> f64 {
        if self.pairs == 0 {
            0.0
        } else {
            self.violations.len() as f64 / self.pairs as f64
        }
    }
}

pub fn check(s: &Series, g: &Gate) -> Report {
    // Did a fall that began at `from` (value `v0`) complete within the reset
    // window? "Complete" is measured against where it started, not against
    // zero: see `Gate::reset_frac`.
    let completes = |ti: f64, from: usize, v0: i32| -> bool {
        let floor = (v0 as f64 * g.reset_frac).floor() as i32;
        // The window carries one sample of the readout's own staleness: the
        // HUD holds the last value the engine computed, so the frame that
        // shows the fall starting may be up to a sample old. The law's 100 ms
        // was measured between telemetry samples; a 60 Hz observer of the same
        // event can see it span 100 ms plus a sample.
        s[from..]
            .iter()
            .take_while(|(t, _)| *t - ti <= g.reset_win + g.sample)
            .any(|(_, v)| v.is_some_and(|v| v <= floor))
    };
    let mut v: Vec<Violation> = Vec::new();
    let mut prev: Option<(f64, i32)> = None;
    let mut pairs = 0usize;
    let mut readable = 0usize;
    for (i, (t, val)) in s.iter().enumerate() {
        let Some(val) = *val else { continue };
        readable += 1;
        if let Some((pt, pv)) = prev {
            let dt = t - pt;
            if dt > 0.0 && dt <= g.max_gap {
                pairs += 1;
                let d = val - pv;
                let span = dt.max(g.sample);
                if d < 0 && !completes(pt, i, pv) {
                    let allowed = (g.drop_rate * span).ceil() as i32 + g.slack;
                    if -d > allowed {
                        v.push(Violation { t: *t, from: pv, to: val, dt, kind: "step-down" });
                    }
                } else if d > 0 {
                    let allowed = (g.rise_rate * span).ceil() as i32 + g.slack;
                    if d > allowed {
                        v.push(Violation { t: *t, from: pv, to: val, dt, kind: "step-up" });
                    }
                }
            }
        }
        prev = Some((*t, val));
    }
    let mut by: Vec<(&'static str, usize)> = Vec::new();
    for k in ["step-down", "step-up"] {
        let n = v.iter().filter(|x| x.kind == k).count();
        by.push((k, n));
    }
    Report { readable, pairs, violations: v, by_kind: by }
}

/// The negative control: replace one decoded digit with a wrong one, at
/// `rate` of the readable frames. This is exactly the defect the gate claims
/// to catch — a tens digit becomes a 10–30 point step with no reset, a units
/// digit breaks the per-frame bound — so a gate that does not light up here is
/// not measuring anything.
pub fn corrupt(s: &Series, rate: f64, seed: u64) -> Series {
    let mut st = seed | 1;
    let mut next = || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        st
    };
    s.iter()
        .map(|(t, v)| {
            let Some(v) = v else { return (*t, None) };
            if (next() % 1_000_000) as f64 / 1_000_000.0 >= rate {
                return (*t, Some(*v));
            }
            let text = format!("{v}");
            let k = (next() as usize) % text.len();
            let mut b: Vec<u8> = text.into_bytes();
            let old = b[k] - b'0';
            let mut d = (next() % 9) as u8;
            if d >= old {
                d += 1;
            }
            b[k] = b'0' + d;
            let w: i32 = String::from_utf8(b).unwrap().parse().unwrap();
            (*t, Some(w.min(100)))
        })
        .collect()
}

pub fn print_envelope(name: &str, e: &Envelope, o: &mut impl Write) {
    writeln!(
        o,
        "{name}\tpairs {}\tdrops {}\trises {}\tresets {}\tmax_drop {} pt ({:.1} pt/s)\tmax_rise {} pt ({:.1} pt/s)",
        e.pairs, e.drops, e.rises, e.resets, e.max_drop_step, e.max_drop_rate, e.max_rise_step, e.max_rise_rate
    )
    .unwrap();
}

pub fn print_report(name: &str, r: &Report, show: usize, o: &mut impl Write) {
    writeln!(
        o,
        "{name}\treadable {}\tpairs {}\tviolations {} ({:.2} %)\t{}",
        r.readable,
        r.pairs,
        r.violations.len(),
        100.0 * r.rate(),
        r.by_kind.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join("  ")
    )
    .unwrap();
    for v in r.violations.iter().take(show) {
        writeln!(o, "  {:.4}\t{} -> {}\tdt {:.4}\t{}", v.t, v.from, v.to, v.dt, v.kind).unwrap();
    }
}
