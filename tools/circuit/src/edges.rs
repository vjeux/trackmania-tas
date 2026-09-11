//! Where the tarmac ends, read off the LIDAR intensity: asphalt returns
//! little light, paint, kerbs and grass return a lot. At every station the
//! transverse profile is walked outwards from the centreline; the first
//! sample clearly brighter than the tarmac core is the edge line.

use crate::tiff::Raster;
use crate::track::{smooth_periodic, Track};

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
            if core.is_nan() {
                continue;
            }
            // Paint/kerb/grass are all well above the asphalt; the threshold
            // sits above the core by a margin that scales with the core so a
            // brighter asphalt (Luffield ~115) is not mistaken for its edge.
            let threshold = core + (60.0f64).max(0.8 * core);
            if let Some(e) = find_edge(&profile, 1.0, threshold) {
                left[i] = e;
                kl[i] = bright_band(&profile, 1.0, e, threshold);
            }
            if let Some(e) = find_edge(&profile, -1.0, threshold) {
                right[i] = e;
                kr[i] = bright_band(&profile, -1.0, e, threshold);
            }
        }
        let guessed = left.iter().chain(right.iter()).filter(|v| v.is_nan()).count();
        let left = fill_and_smooth(&left, 5.0 / tr.ds);
        let right = fill_and_smooth(&right, 5.0 / tr.ds);
        let kl = smooth_periodic(&kl, 3.0 / tr.ds);
        let kr = smooth_periodic(&kr, 3.0 / tr.ds);
        Edges { left, right, kerb_left: kl, kerb_right: kr, guessed }
    }
}

/// Replace NaN gaps by linear interpolation around the loop, knock out
/// single-station spikes with a 7-wide median, then Gaussian-smooth.
fn fill_and_smooth(v: &[f64], sigma: f64) -> Vec<f64> {
    let n = v.len();
    let mut out = v.to_vec();
    // fill gaps
    let valid: Vec<usize> = (0..n).filter(|&i| !v[i].is_nan()).collect();
    assert!(valid.len() > n / 2, "edges: most stations unreadable");
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
    smooth_periodic(&med, sigma)
}
