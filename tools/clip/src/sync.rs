//! `clip sync` -- CHECK a render against the tape it plays, from the picture
//! itself.
//!
//! An overlay is a claim about timing: "at this frame the driver was doing
//! this". `clip alignment` checks the two channels INSIDE a ghost against each
//! other, which says nothing about where the VIDEO starts. Until now that was
//! settled by eye, per clip, against a frame -- or, more often, not settled at
//! all and assumed. This reads the picture.
//!
//! **The signal.** The camera is bolted to the car (FILMING.md §1) and turns
//! with it, so the world slides sideways across the frame at the car's yaw
//! rate. The telemetry carries the car's heading every 50 ms. The picture's
//! horizontal shift per frame and the tape's yaw rate describe ONE run, so
//! they correlate at one lag and nowhere else. (Frame-to-frame change against
//! speed was tried first and is kept as [`Motion::energy`]; it correlates at
//! 0.1-0.4 on these maps -- what the ground looks like matters more than how
//! fast it passes -- and cannot place anything.)
//!
//! **What the lag means, and what it does not.** The chase camera swings behind
//! the car on a spring, so the picture's yaw LAGS the car's yaw: on Tiny Summer
//! 01/02/15 the best lag reads -160, -190, -290 ms while a frame check on 02
//! (the front wheels turning at race 1.950, the wheels being drawn from the
//! ghost with no spring) puts the render at offset 0 to a frame. A low-pass
//! fitted jointly does not separate the two -- a delay and a smoothing look
//! alike to a correlation -- so the lag is NOT the offset to draw at. It is a
//! GUARD: a render that started half a second early, a clip cut at the wrong
//! place, a ghost that is not the one in the picture, a static render with no
//! car in it, all land outside the window a correct clip lands in, or correlate
//! with nothing. [`Fit::check`] is that window; the offset the overlay draws
//! at is the pipeline's nominal one, verified by eye once per pipeline.
//!
//! The frames come out of ffmpeg on a pipe as 192x108 grey, so a 150 s clip
//! costs a few seconds and nothing lands on disk.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::fmt::secs;
use crate::platform::Ff;

pub const W: usize = 192;
pub const H: usize = 108;

/// The chase camera's spring, as measured: a correct clip's yaw lag sits this
/// far BEHIND the nominal offset at most ...
pub const CAMERA_LAG_MAX_MS: i64 = 450;
/// ... and never this far ahead of it.
pub const CAMERA_LEAD_MAX_MS: i64 = 150;
/// Below this the picture is not following the tape at all.
pub const MIN_R: f64 = 0.35;
/// A peak wider than this is a plateau, not a fix.
pub const MAX_WIDTH_MS: i64 = 500;

/// The fitted lag and how much to believe it.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    /// `race_ms = video_ms + lag_ms` -- the sense `clip overlay --offset-ms` uses.
    pub lag_ms: i64,
    /// Spearman correlation at the best lag.
    pub r: f64,
    /// ... and at lag 0, for the reader who assumed zero.
    pub r_at_zero: f64,
    /// Span of the lags whose correlation is within 0.02 of the peak. A sharp
    /// peak is a few frames wide; a plateau means the picture does not say.
    pub width_ms: i64,
    pub frames: usize,
    pub fps: f64,
}

impl Fit {
    /// THE GUARD. The fit must be believable (`r`, `width`) and land where a
    /// correct clip lands relative to `nominal_ms`: between
    /// [`CAMERA_LAG_MAX_MS`] behind it (the spring) and [`CAMERA_LEAD_MAX_MS`]
    /// ahead. Anything else is a refusal with the numbers in it.
    pub fn check(&self, nominal_ms: i64) -> Result<(), String> {
        if self.r < MIN_R {
            return Err(format!(
                "the picture does not follow the tape: rank correlation {:.2} at the best lag \
                 ({:+} ms) -- is this the ghost that was rendered, and is there a car in the frame?",
                self.r, self.lag_ms
            ));
        }
        if self.width_ms > MAX_WIDTH_MS {
            return Err(format!(
                "the timing fit is a plateau {} ms wide (best {:+} ms, r {:.2}) -- the picture \
                 cannot place the tape",
                self.width_ms, self.lag_ms, self.r
            ));
        }
        let d = self.lag_ms - nominal_ms;
        if d < -CAMERA_LAG_MAX_MS || d > CAMERA_LEAD_MAX_MS {
            return Err(format!(
                "the picture's yaw lags the tape by {:+} ms (r {:.2}, width {} ms); a correct render \
                 at offset {:+} ms reads between {:+} and {:+} (the chase camera's spring) -- this clip \
                 does not start where the pipeline says it does",
                self.lag_ms,
                self.r,
                self.width_ms,
                nominal_ms,
                nominal_ms - CAMERA_LAG_MAX_MS,
                nominal_ms + CAMERA_LEAD_MAX_MS
            ));
        }
        Ok(())
    }

    /// One phrase for a report column: `yaw lag -190 ms (r 0.73, width 110 ms)`.
    pub fn summary(&self) -> String {
        format!("yaw lag {:+} ms (r {:.2}, width {} ms)", self.lag_ms, self.r, self.width_ms)
    }

    /// The marker's compact spelling: `guard/lag-190ms/r0.73`.
    pub fn compact(&self) -> String {
        format!("guard/lag{:+}ms/r{:.2}", self.lag_ms, self.r)
    }
}

/// What the picture does from one frame to the next.
pub struct Motion {
    /// Mean absolute change of the grey frame (frame 0 gets frame 1's).
    pub energy: Vec<f64>,
    /// Horizontal shift of the world, px of the 192-wide frame (see [`yaw_shift`]).
    pub yaw_px: Vec<f64>,
    pub fps: f64,
}

/// Decode the video small and grey and read both motion series off it.
pub fn picture_motion(ff: &Ff, video: &Path) -> Result<Motion, String> {
    let fps = ff.probe_fps(video)?;
    let args: Vec<String> = vec![
        "-v".into(),
        "error".into(),
        "-i".into(),
        ff.arg_path(video)?,
        "-an".into(),
        "-vf".into(),
        format!("scale={W}:{H}"),
        "-pix_fmt".into(),
        "gray".into(),
        "-f".into(),
        "rawvideo".into(),
        "-".into(),
    ];
    let mut child = Command::new(&ff.ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", ff.ffmpeg.display()))?;
    let mut so = child.stdout.take().ok_or("no stdout on ffmpeg")?;
    let mut prev = vec![0u8; W * H];
    let mut cur = vec![0u8; W * H];
    let mut energy: Vec<f64> = Vec::new();
    let mut yaw_px: Vec<f64> = Vec::new();
    loop {
        let mut got = 0usize;
        while got < W * H {
            let n = so.read(&mut cur[got..]).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            got += n;
        }
        if got < W * H {
            break;
        }
        if !energy.is_empty() {
            let sum: u64 = cur.iter().zip(prev.iter()).map(|(a, b)| (*a as i64 - *b as i64).unsigned_abs()).sum();
            energy.push(sum as f64 / (W * H) as f64);
            yaw_px.push(yaw_shift(&prev, &cur));
        } else {
            energy.push(0.0);
            yaw_px.push(0.0);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    let outp = child.wait_with_output().map_err(|e| e.to_string())?;
    if !outp.status.success() {
        return Err(format!(
            "ffmpeg failed ({}): {}",
            outp.status,
            String::from_utf8_lossy(&outp.stderr).trim()
        ));
    }
    if energy.len() < 30 {
        return Err(format!("{}: only {} frames decoded", video.display(), energy.len()));
    }
    if energy.len() > 1 {
        energy[0] = energy[1];
        yaw_px[0] = yaw_px[1];
    }
    Ok(Motion { energy, yaw_px, fps })
}

/// Horizontal shift of the picture between consecutive frames, in pixels of
/// the 192-wide frame: the camera's yaw rate, as the picture shows it.
///
/// The top 60 % of the frame is the world (the car sits in the bottom 40 % and
/// does not move relative to its own camera); its column-sum profile is a 1-D
/// signature that a yaw shifts sideways. The shift is the one, in +-20 pixels,
/// that best overlays the previous frame's profile.
pub fn yaw_shift(prev: &[u8], cur: &[u8]) -> f64 {
    const R: i64 = 20;
    let rows = H * 6 / 10;
    let prof = |f: &[u8]| -> Vec<f64> {
        let mut p = vec![0.0; W];
        for y in 0..rows {
            for x in 0..W {
                p[x] += f[y * W + x] as f64;
            }
        }
        p
    };
    let (a, b) = (prof(prev), prof(cur));
    let mut best = (0i64, f64::INFINITY);
    for s in -R..=R {
        let mut sad = 0.0;
        let mut n = 0usize;
        for x in 0..W as i64 {
            let xs = x + s;
            if xs < 0 || xs >= W as i64 {
                continue;
            }
            sad += (a[x as usize] - b[xs as usize]).abs();
            n += 1;
        }
        let sad = sad / n as f64;
        if sad < best.1 {
            best = (s, sad);
        }
    }
    best.0 as f64
}

/// The car's speed (m/s) at each telemetry sample, by race ms.
pub fn speed_series(ghost: &str) -> Result<Vec<(i64, f64)>, String> {
    let d = gbx::record::decode_ghost(ghost)?;
    let v: Vec<(i64, f64)> = d
        .samples
        .iter()
        .map(|s| (s.time_ms as i64, s.speed_ms as f64))
        .collect();
    if v.len() < 20 {
        return Err(format!("{ghost}: only {} telemetry samples", v.len()));
    }
    Ok(v)
}

/// The car's yaw rate (rad/s) over each telemetry interval, by the race ms at
/// the interval's middle. Wrapped, so a heading crossing +-pi is not a spin.
pub fn yaw_rate_series(ghost: &str) -> Result<Vec<(i64, f64)>, String> {
    let d = gbx::record::decode_ghost(ghost)?;
    let mut v = Vec::new();
    for w in d.samples.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let dt = (b.time_ms - a.time_ms) as f64 / 1000.0;
        if dt <= 0.0 {
            continue;
        }
        let mut dy = (b.yaw - a.yaw) as f64;
        while dy > std::f64::consts::PI {
            dy -= 2.0 * std::f64::consts::PI;
        }
        while dy < -std::f64::consts::PI {
            dy += 2.0 * std::f64::consts::PI;
        }
        v.push((((a.time_ms + b.time_ms) / 2) as i64, dy / dt));
    }
    if v.len() < 20 {
        return Err(format!("{ghost}: only {} telemetry intervals", v.len()));
    }
    Ok(v)
}

/// Linear interpolation of a sorted `(ms, value)` series; `None` outside it.
fn at(series: &[(i64, f64)], ms: i64) -> Option<f64> {
    let (first, last) = (series.first()?.0, series.last()?.0);
    if ms < first || ms > last {
        return None;
    }
    let i = series.partition_point(|(t, _)| *t <= ms);
    if i == 0 {
        return Some(series[0].1);
    }
    if i >= series.len() {
        return Some(series[series.len() - 1].1);
    }
    let (t0, v0) = series[i - 1];
    let (t1, v1) = series[i];
    if t1 == t0 {
        return Some(v0);
    }
    Some(v0 + (v1 - v0) * (ms - t0) as f64 / (t1 - t0) as f64)
}

/// Average ranks (ties share their mean rank).
fn ranks(v: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..v.len()).collect();
    idx.sort_by(|a, b| v[*a].total_cmp(&v[*b]));
    let mut r = vec![0.0; v.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && v[idx[j + 1]] == v[idx[i]] {
            j += 1;
        }
        let mean = (i + j) as f64 / 2.0 + 1.0;
        for k in i..=j {
            r[idx[k]] = mean;
        }
        i = j + 1;
    }
    r
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        let (dx, dy) = (a - mx, b - my);
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    if sxx <= 0.0 || syy <= 0.0 {
        return 0.0;
    }
    sxy / (sxx * syy).sqrt()
}

/// Spearman correlation between a per-frame picture series and a telemetry
/// series shifted by `lag_ms`, over the frames both cover.
fn spearman_at(frames: &[f64], fps: f64, tele: &[(i64, f64)], lag_ms: i64) -> Option<f64> {
    let mut xs = Vec::with_capacity(frames.len());
    let mut ys = Vec::with_capacity(frames.len());
    for (f, e) in frames.iter().enumerate() {
        let video_ms = (f as f64 * 1000.0 / fps).round() as i64;
        if let Some(v) = at(tele, video_ms + lag_ms) {
            xs.push(*e);
            ys.push(v);
        }
    }
    if xs.len() < 30 {
        return None;
    }
    let (rx, ry) = (ranks(&xs), ranks(&ys));
    Some(pearson(&rx, &ry))
}

/// The fit over lags in `[-span_ms, +span_ms]`, 10 ms apart. Sign-agnostic in
/// the telemetry (the picture's shift and the heading may be measured in
/// opposite senses): the better of `+tele` and `-tele` is reported.
pub fn fit(frames: &[f64], fps: f64, tele: &[(i64, f64)], span_ms: i64) -> Result<Fit, String> {
    let neg: Vec<(i64, f64)> = tele.iter().map(|(t, v)| (*t, -*v)).collect();
    let mut best: Option<Fit> = None;
    for series in [tele, &neg[..]] {
        let mut curve: Vec<(i64, f64)> = Vec::new();
        let mut lag = -span_ms;
        while lag <= span_ms {
            if let Some(r) = spearman_at(frames, fps, series, lag) {
                curve.push((lag, r));
            }
            lag += 10;
        }
        if curve.is_empty() {
            continue;
        }
        let top = curve.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        let r_at_zero = curve.iter().find(|(l, _)| *l == 0).map(|(_, r)| *r).unwrap_or(f64::NAN);
        let near: Vec<i64> = curve.iter().filter(|(_, r)| *r >= top.1 - 0.02).map(|(l, _)| *l).collect();
        let width_ms = near.iter().max().unwrap() - near.iter().min().unwrap();
        let f = Fit { lag_ms: top.0, r: top.1, r_at_zero, width_ms, frames: frames.len(), fps };
        if best.as_ref().map(|b| f.r > b.r).unwrap_or(true) {
            best = Some(f);
        }
    }
    best.ok_or_else(|| "the video and the telemetry share no instants at any lag".into())
}

/// Decode, read, fit the yaw; print what was measured.
pub fn run(ff: &Ff, ghost: &Path, video: &Path, span_ms: i64) -> Result<Fit, String> {
    let m = picture_motion(ff, video)?;
    let yaw = yaw_rate_series(&ghost.to_string_lossy())?;
    let f = fit(&m.yaw_px, m.fps, &yaw, span_ms)?;
    println!(
        "sync: {} frames at {} fps ({}s) against {} telemetry intervals to race {}s: {}; at lag 0 r {:.2}",
        f.frames,
        m.fps,
        secs(f.frames as f64 / m.fps),
        yaw.len(),
        secs(yaw.last().map(|s| s.0).unwrap_or(0) as f64 / 1000.0),
        f.summary(),
        f.r_at_zero
    );
    Ok(f)
}

/// Diagnostic (`clip sync --all`): every fit this module can make, printed.
pub fn run_all(ff: &Ff, ghost: &Path, video: &Path, span_ms: i64) -> Result<(), String> {
    let m = picture_motion(ff, video)?;
    let g = ghost.to_string_lossy();
    let fs = fit(&m.energy, m.fps, &speed_series(&g)?, span_ms)?;
    println!("  energy ~ speed : {}", fs.summary());
    let fy = fit(&m.yaw_px, m.fps, &yaw_rate_series(&g)?, span_ms)?;
    println!("  yaw px ~ yaw   : {}", fy.summary());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A heading that wanders like a lap (turns of both signs, straights), and
    /// a picture whose shift is that heading's rate seen through a spring and
    /// moved by a known number of frames.
    fn synthetic(shift_frames: i64, spring_ms: f64) -> (Vec<f64>, Vec<(i64, f64)>) {
        let fps = 30.0;
        // yaw rate at 50 ms: bursts of turning, zero on the straights
        let mut rate = Vec::new();
        for i in 0..1200 {
            let t = i as f64 * 0.05;
            let phase = (t / 4.0).floor() as i64 % 4;
            let v = match phase {
                0 => 0.0,
                1 => 1.5 * ((t * 3.1).sin()).max(0.0),
                2 => -1.2 * ((t * 2.3).cos()).abs(),
                _ => 0.3 * (t * 5.0).sin(),
            };
            rate.push(((t * 1000.0).round() as i64, v));
        }
        // the camera: first-order spring on the rate
        let mut cam = Vec::with_capacity(rate.len());
        let mut s = 0.0;
        for (t, v) in &rate {
            let a = 50.0 / (spring_ms + 50.0);
            s += a * (v - s);
            cam.push((*t, s));
        }
        let mut px = Vec::new();
        for f in 0..1800i64 {
            let video_ms = (f as f64 * 1000.0 / fps).round() as i64;
            let race_ms = video_ms - (shift_frames as f64 * 1000.0 / fps).round() as i64;
            let v = at(&cam, race_ms).unwrap_or(0.0);
            // quantised to whole pixels like the real measure, sign flipped like a camera might
            px.push(-(v * 6.0).round());
        }
        (px, rate)
    }

    /// A KNOWN SHIFT IS RECOVERED, and a camera spring shows up as a LAG behind
    /// it (never ahead), which is what `check`'s asymmetric window encodes.
    #[test]
    fn a_known_shift_is_recovered_and_the_spring_only_ever_lags() {
        for shift in [0i64, 3, -6, 15] {
            let want = -(shift as f64 * 1000.0 / 30.0).round() as i64;
            let (px, rate) = synthetic(shift, 0.0);
            let f = fit(&px, 30.0, &rate, 1500).unwrap();
            assert!((f.lag_ms - want).abs() <= 40, "no spring, shift {shift}: fitted {:+}, wanted {want:+} (r {:.2})", f.lag_ms, f.r);
            assert!(f.r > 0.9, "r {:.2}", f.r);
            assert!(f.width_ms <= 200, "width {} ms", f.width_ms);
            let (px, rate) = synthetic(shift, 200.0);
            let f = fit(&px, 30.0, &rate, 1500).unwrap();
            let d = f.lag_ms - want;
            assert!(d <= 20 && d >= -CAMERA_LAG_MAX_MS, "spring 200 ms, shift {shift}: lag {:+} vs offset {want:+}", f.lag_ms);
            assert!(f.check(want).is_ok(), "{:?}", f.check(want));
        }
    }

    #[test]
    fn the_guard_refuses_weak_wide_and_off_fits() {
        let good = Fit { lag_ms: -190, r: 0.73, r_at_zero: 0.5, width_ms: 110, frames: 500, fps: 30.0 };
        assert!(good.check(0).is_ok());
        let weak = Fit { r: 0.2, ..good.clone() };
        assert!(weak.check(0).unwrap_err().contains("does not follow"));
        let wide = Fit { width_ms: 900, ..good.clone() };
        assert!(wide.check(0).unwrap_err().contains("plateau"));
        // a render that started half a second early
        let early = Fit { lag_ms: -700, ..good.clone() };
        let e = early.check(0).unwrap_err();
        assert!(e.contains("-700 ms"), "{e}");
        // ... and one that started late (the picture AHEAD of the tape)
        let late = Fit { lag_ms: 400, ..good.clone() };
        assert!(late.check(0).is_err());
        // an offset the pipeline EXPECTS moves the window with it
        assert!(early.check(-500).is_ok());
    }

    #[test]
    fn ranks_share_ties_and_interpolation_stays_inside() {
        assert_eq!(ranks(&[10.0, 30.0, 20.0]), vec![1.0, 3.0, 2.0]);
        assert_eq!(ranks(&[5.0, 5.0, 1.0]), vec![2.5, 2.5, 1.0]);
        let s = vec![(0, 0.0), (50, 10.0), (100, 20.0)];
        assert_eq!(at(&s, 25), Some(5.0));
        assert_eq!(at(&s, 100), Some(20.0));
        assert_eq!(at(&s, 101), None);
        assert_eq!(at(&s, -1), None);
    }

    /// The shift finder reads a sideways move of the world off two frames.
    #[test]
    fn a_sideways_shift_of_the_world_is_read_back() {
        let mut a = vec![0u8; W * H];
        for y in 0..H {
            for x in 0..W {
                a[y * W + x] = (((x / 7) % 3) * 90 + (y % 5) * 3) as u8;
            }
        }
        for s in [-5i64, 0, 4] {
            let mut b = vec![0u8; W * H];
            for y in 0..H {
                for x in 0..W as i64 {
                    let src = x - s;
                    if src >= 0 && src < W as i64 {
                        b[y * W + x as usize] = a[y * W + src as usize];
                    }
                }
            }
            let got = yaw_shift(&a, &b);
            assert!((got - s as f64).abs() <= 1.0, "shift {s}: read {got}");
        }
    }
}
