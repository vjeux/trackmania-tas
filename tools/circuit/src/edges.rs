//! Where the tarmac ends, read off the LIDAR intensity: asphalt returns
//! little light, paint, kerbs and grass return a lot. At every station the
//! transverse profile is walked outwards from the centreline; the first
//! sample clearly brighter than the tarmac core is the edge line.

use crate::tiff::Raster;
use crate::track::Track;

#[derive(Clone, Debug)]
pub struct Edges {
    /// Lateral offset of the left edge (positive, metres from the centreline).
    pub left: Vec<f64>,
    /// Lateral offset of the right edge (positive; the edge is at -right).
    pub right: Vec<f64>,
    /// Width of the bright band just outside each edge (kerb / painted
    /// verge), 0 when the tarmac meets grass directly.
    pub kerb_left: Vec<f64>,
    pub kerb_right: Vec<f64>,
    /// Stations where an edge could not be read and was interpolated.
    pub guessed: usize,
}

const STEP: f64 = 0.25;
const MIN_HALF: f64 = 4.0;
const MAX_HALF: f64 = 16.0;

/// The tarmac core level: median intensity within +-2.5 m of the centre.
fn core_level(profile: &[(f64, f64)]) -> f64 {
    let mut v: Vec<f64> = profile.iter().filter(|(o, _)| o.abs() <= 2.5).map(|(_, i)| *i).collect();
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Walk from the centre outwards along `side` (+1 left, -1 right); the edge
/// is where the profile first crosses the threshold, interpolated between
/// samples. None when it never does within MAX_HALF.
fn find_edge(profile: &[(f64, f64)], side: f64, threshold: f64) -> Option<f64> {
    let mut prev: Option<(f64, f64)> = None;
    let mut k = 0.0;
    while k <= MAX_HALF {
        let off = k * side;
        let v = profile.iter().find(|(o, _)| (o - off).abs() < 1e-6).map(|(_, v)| *v)?;
        if k >= MIN_HALF {
            if let Some((po, pv)) = prev {
                if v >= threshold && pv < threshold {
                    let t = (threshold - pv) / (v - pv);
                    return Some(po + t * (k - po));
                }
            }
        }
        prev = Some((k, v));
        k += STEP;
    }
    None
}

/// The first painted line: the first point past MIN_HALF where the profile
/// rises `rise` above the core AND falls back by at least `dip` within 3 m
/// (a white edge line with asphalt beyond it — a kerb followed by grass
/// never falls back, and is left to the threshold edge). Measured at Abbey
/// (station 340): the track's white line is a 55 -> 98 bump at +5.5 m with a
/// 4 m asphalt run-off at 73..80 beyond it; the threshold edge ran to the
/// grass at 10.4 m and made the track 18.4 m wide. Any real circuit with
/// asphalt run-offs has this everywhere.
fn find_line(profile: &[(f64, f64)], side: f64, core: f64, rise: f64, dip: f64) -> Option<f64> {
    let at = |k: f64| profile.iter().find(|(o, _)| (o - k * side).abs() < 1e-6).map(|(_, v)| *v);
    let mut k = MIN_HALF;
    while k <= MAX_HALF {
        let v = at(k)?;
        if v >= core + rise {
            // the peak over the next 3 m, then the minimum after the peak
            let mut peak = v;
            let mut peak_k = k;
            let mut q = k + STEP;
            while q <= k + 3.0 {
                match at(q) {
                    Some(w) if w > peak => {
                        peak = w;
                        peak_k = q;
                    }
                    Some(_) => {}
                    None => break,
                }
                q += STEP;
            }
            let mut fell = false;
            let mut q = peak_k + STEP;
            while q <= k + 3.0 {
                match at(q) {
                    Some(w) if w <= peak - dip => {
                        fell = true;
                        break;
                    }
                    Some(_) => {}
                    None => break,
                }
                q += STEP;
            }
            if fell {
                // the edge is the foot of the rise: interpolate the core+rise crossing
                let pv = at(k - STEP).unwrap_or(v);
                let t = if v > pv { ((core + rise - pv) / (v - pv)).clamp(0.0, 1.0) } else { 1.0 };
                return Some(k - STEP + t * STEP);
            }
            // a plateau that never fell back: a kerb into grass, not a line
            return None;
        }
        k += STEP;
    }
    None
}

/// A step between two asphalts: the track and a flush run-off differ by
/// only 10..20 intensity units (Maggotts: track 80, run-off 92; Chapel
/// exit: 62 vs 81) and carry no visible line at 1 m. Between MIN_HALF and
/// `limit`, the position of the largest jump between the mean of the 2 m
/// before and the 2 m after; Some only when that jump is at least `min_step`
/// and the two sides are each flat (std below a third of the step).
fn find_step(profile: &[(f64, f64)], side: f64, limit: f64, min_step: f64) -> Option<f64> {
    let at = |k: f64| profile.iter().find(|(o, _)| (o - k * side).abs() < 1e-6).map(|(_, v)| *v);
    let stats = |from: f64, to: f64| -> Option<(f64, f64)> {
        let mut v = Vec::new();
        let mut k = from;
        while k < to - 1e-9 {
            v.push(at(k)?);
            k += STEP;
        }
        if v.is_empty() {
            return None;
        }
        let m = v.iter().sum::<f64>() / v.len() as f64;
        let sd = (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64).sqrt();
        Some((m, sd))
    };
    let mut best: Option<(f64, f64)> = None;
    let mut k = MIN_HALF;
    while k + 2.0 <= limit {
        let (Some((m0, s0)), Some((m1, s1))) = (stats(k - 2.0, k), stats(k, k + 2.0)) else {
            k += STEP;
            continue;
        };
        let jump = m1 - m0;
        if jump >= min_step && s0 < jump / 3.0 && s1 < jump / 3.0 && best.map(|(_, j)| jump > j).unwrap_or(true) {
            best = Some((k, jump));
        }
        k += STEP;
    }
    best.map(|(k, _)| k)
}

/// How far the bright band extends past `edge` before the profile falls
/// back below `threshold` (paint/kerb width); capped at 4 m.
fn bright_band(profile: &[(f64, f64)], side: f64, edge: f64, threshold: f64) -> f64 {
    let mut k = (edge / STEP).ceil() * STEP + STEP;
    let start = k;
    while k <= edge + 4.0 {
        let off = k * side;
        let v = match profile.iter().find(|(o, _)| (o - off).abs() < 1e-6) {
            Some((_, v)) => *v,
            None => break,
        };
        if v < threshold {
            break;
        }
        k += STEP;
    }
    (k - start).max(0.0)
}

impl Edges {
    pub fn from_intensity(tr: &Track, inten: &Raster) -> Edges {
        let n = tr.len();
        let mut left = vec![f64::NAN; n];
        let mut right = vec![f64::NAN; n];
        let mut kl = vec![0.0; n];
        let mut kr = vec![0.0; n];
        for i in 0..n {
            let mut profile: Vec<(f64, f64)> = Vec::new();
            let mut k = -(MAX_HALF + 1.0);
            while k <= MAX_HALF + 1.0 {
                let p = tr.offset(i, k);
                if let Some(v) = inten.sample(p[0], p[1]) {
                    profile.push((k, v));
                }
                k += STEP;
            }
            let core = core_level(&profile);
            // a core this bright is not tarmac: the Wellington footbridge deck
            // (stations 1679..1732 read 134..698) -- interpolate across it
            if core.is_nan() || core > 150.0 {
                continue;
            }
            // Paint/kerb/grass are all well above the asphalt; the threshold
            // sits above the core by a margin that scales with the core so a
            // brighter asphalt (Luffield ~115) is not mistaken for its edge.
            let threshold = core + (60.0f64).max(0.8 * core);
            for (side, edge, kerb) in [(1.0, &mut left, &mut kl), (-1.0, &mut right, &mut kr)] {
                let thr_edge = find_edge(&profile, side, threshold);
                let line_edge = find_line(&profile, side, core, 25.0, 15.0);
                // the painted line wins when the threshold edge is a run-off
                // further out; a line beyond the threshold edge is paint on
                // the kerb/run-off and means nothing. With neither a line nor
                // a threshold edge inside 9.5 m, a step between two asphalts
                // is the edge (a half-width past 9.5 m on one side is a
                // run-off: the whole track is 12..15 m wide).
                let mut e = match (thr_edge, line_edge) {
                    (Some(t), Some(l)) if t - l > 1.0 => Some(l),
                    (Some(t), _) => Some(t),
                    (None, l) => l,
                };
                if e.map(|x| x > 9.5).unwrap_or(true) {
                    if let Some(st) = find_step(&profile, side, e.unwrap_or(MAX_HALF), 8.0) {
                        e = Some(st);
                    }
                }
                if let Some(e) = e {
                    edge[i] = e;
                    kerb[i] = bright_band(&profile, side, e, threshold);
                }
            }
        }
        let guessed = left.iter().chain(right.iter()).filter(|v| v.is_nan()).count();
        // A road the intensity cannot read (a concrete or freshly painted
        // surface, a road under trees): a plain 9 m road, flagged.
        let readable = left.iter().filter(|v| !v.is_nan()).count().min(right.iter().filter(|v| !v.is_nan()).count());
        if readable * 2 <= n {
            println!("edges: only {readable} of {n} stations readable -- a 9 m road is assumed");
            return Edges { left: vec![4.5; n], right: vec![4.5; n], kerb_left: vec![0.0; n], kerb_right: vec![0.0; n], guessed: n };
        }
        // A run-off that merges with the track through a gap in the kerb
        // (Chapel exit onto Hangar: 60 m where the rejoin asphalt is the
        // track's own shade and the white line is invisible at 1 m) shows
        // as one side wandering off its own running median while the width
        // grows past what any part of this circuit measures (15.9 m).
        // That side is a gap, filled from its neighbours.
        let (mut left, mut right) = (left, right);
        let bulges = knock_out_bulges(&mut left, &mut right, 16.0, 1.5, 60);
        if bulges > 0 {
            println!("edges: {bulges} run-off bulges knocked out (one side off its running median with the width past 16 m)");
        }
        let left = fill_and_smooth(&left, 5.0 / tr.ds, tr.closed);
        let right = fill_and_smooth(&right, 5.0 / tr.ds, tr.closed);
        let kl = crate::track::smooth(&kl, 3.0 / tr.ds, tr.closed);
        let kr = crate::track::smooth(&kr, 3.0 / tr.ds, tr.closed);
        Edges { left, right, kerb_left: kl, kerb_right: kr, guessed }
    }
}

/// Running median over ±`half` stations of a cyclic series (NaNs skipped).
fn running_median(v: &[f64], half: usize) -> Vec<f64> {
    let n = v.len();
    (0..n)
        .map(|i| {
            let mut w: Vec<f64> = (0..=2 * half).map(|k| v[(i + n + k - half) % n]).filter(|x| !x.is_nan()).collect();
            if w.is_empty() {
                return f64::NAN;
            }
            w.sort_by(|a, b| a.partial_cmp(b).unwrap());
            w[w.len() / 2]
        })
        .collect()
}

/// Where the width exceeds `max_width` and one side sits more than `dev`
/// beyond its own running median (±`half` stations), that side becomes a
/// gap. Returns how many stations were knocked out.
fn knock_out_bulges(left: &mut [f64], right: &mut [f64], max_width: f64, dev: f64, half: usize) -> usize {
    let ml = running_median(left, half);
    let mr = running_median(right, half);
    let mut n = 0;
    for i in 0..left.len() {
        if left[i].is_nan() || right[i].is_nan() || left[i] + right[i] <= max_width {
            continue;
        }
        let (dl, dr) = (left[i] - ml[i], right[i] - mr[i]);
        if dl > dev && dl >= dr {
            left[i] = f64::NAN;
            n += 1;
        } else if dr > dev {
            right[i] = f64::NAN;
            n += 1;
        }
    }
    n
}

/// Replace NaN gaps by linear interpolation around the loop, knock out
/// single-station spikes with a 7-wide median, then Gaussian-smooth.
fn fill_and_smooth(v: &[f64], sigma: f64, closed: bool) -> Vec<f64> {
    let n = v.len();
    let mut out = v.to_vec();
    // fill gaps
    let valid: Vec<usize> = (0..n).filter(|&i| !v[i].is_nan()).collect();
    if valid.len() <= n / 2 {
        // too few readings to interpolate between: the median of what there
        // is (or 4.5 m) everywhere
        let mut w: Vec<f64> = valid.iter().map(|&i| v[i]).collect();
        w.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let m = if w.is_empty() { 4.5 } else { w[w.len() / 2] };
        return vec![m; n];
    }
    for i in 0..n {
        if !v[i].is_nan() {
            continue;
        }
        // nearest valid before and after (cyclic)
        let mut a = i;
        let mut da = 0;
        while v[a].is_nan() {
            a = (a + n - 1) % n;
            da += 1;
        }
        let mut b = i;
        let mut db = 0;
        while v[b].is_nan() {
            b = (b + 1) % n;
            db += 1;
        }
        let t = da as f64 / (da + db) as f64;
        out[i] = v[a] * (1.0 - t) + v[b] * t;
    }
    // median of 7
    let med: Vec<f64> = (0..n)
        .map(|i| {
            let mut w: Vec<f64> = (-3i64..=3).map(|k| out[((i as i64 + k) % n as i64 + n as i64) as usize % n]).collect();
            w.sort_by(|a, b| a.partial_cmp(b).unwrap());
            w[3]
        })
        .collect();
    crate::track::smooth(&med, sigma, closed)
}
