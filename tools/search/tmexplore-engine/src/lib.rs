//! The adapters that bind the explorer to the real engine.
//!
//! `tmexplore` is deliberately dependency-free: the archive, the bin key, the
//! alphabet and the policy are logic over three traits. This crate is where
//! those traits meet the game.
//!
//! | trait | owner | implemented here as |
//! |---|---|---|
//! | `PlainOracle` | agent A | [`EngineOracle`] — patch the tape into a container, hand the FILE to the dedicated server |
//! | `Route` | agent B | [`TsvRoute`] — a route file, until B's own format lands |
//! | `Branch` | agent D | *not here*. D owns it, and a second implementation would be a second definition of what a resume means |
//!
//! # What agent A has to hand C, precisely
//!
//! One thing, and this is the whole of the dependency:
//!
//! > **A container with no human provenance, whose input archive is at least
//! > `N` ticks long and every one of those ticks individually patchable.**
//!
//! The length matters and it is easy to miss. `tmsearch::tape::Patcher` writes
//! a candidate by patching bit positions in a base image, so a tape can only
//! be as long as the container's input archive — and the validator only
//! simulates as long as that archive lasts, plus about 200 ticks. A cold-start
//! run is *slow*: the explorer's first finisher on any map will be far slower
//! than a polished one, so a container sized to a good lap is a container the
//! first finisher runs off the end of. `N` should be generous — 12 000 ticks
//! is 120 s — and the explorer pads any shorter tape with neutral input.
//!
//! `Patcher` also *refuses* to search a window containing a tick it cannot
//! write (a 32-bit steer field, a packet with no vehicle fields). For a
//! synthesized container those should not exist at all, and if any do, the
//! explorer needs the list, because a silently-dropped write is a candidate
//! scored on inputs its own file does not contain.

use std::path::{Path, PathBuf};
use tmexplore::action::Input;
use tmexplore::branch::{Progress, Route};
use tmexplore::outcome::Verdict;

pub mod oracle;
pub use oracle::EngineOracle;

/// A route as a tab-separated file, until agent B's own format lands.
///
/// ```text
/// # spacing<TAB>20.0
/// # checkpoints<TAB>4
/// s<TAB>x<TAB>y<TAB>z<TAB>half_width
/// 0.000<TAB>123.5<TAB>9.0<TAB>512.0<TAB>8.0
/// ...
/// ```
///
/// Vertices in increasing `s`. Anything after `#` on the first lines is a
/// header key; the rest are vertices.
pub struct TsvRoute {
    pts: Vec<[f32; 3]>,
    cum: Vec<f32>,
    half: Vec<f32>,
    spacing: f32,
    n_cp: u32,
    grid: std::collections::HashMap<(i32, i32), Vec<u32>>,
    cell: f32,
}

impl TsvRoute {
    pub fn load(p: &Path) -> Result<TsvRoute, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
        let mut pts = Vec::new();
        let mut cum = Vec::new();
        let mut half = Vec::new();
        let mut spacing = 20.0f32;
        let mut n_cp = 0u32;
        for (ln, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix('#') {
                let mut it = rest.split_whitespace();
                match (it.next(), it.next()) {
                    (Some("spacing"), Some(v)) => {
                        spacing = v.parse().map_err(|_| format!("line {}: bad spacing", ln + 1))?
                    }
                    (Some("checkpoints"), Some(v)) => {
                        n_cp = v.parse().map_err(|_| format!("line {}: bad checkpoints", ln + 1))?
                    }
                    _ => {}
                }
                continue;
            }
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 5 {
                // An absent row is not a failure row: say which line, rather
                // than skipping and producing a short route that looks fine.
                return Err(format!("line {}: want 5 fields, got {}", ln + 1, f.len()));
            }
            let n = |i: usize| -> Result<f32, String> {
                f[i].parse().map_err(|_| format!("line {}: field {} is not a number", ln + 1, i))
            };
            cum.push(n(0)?);
            pts.push([n(1)?, n(2)?, n(3)?]);
            half.push(n(4)?);
        }
        if pts.len() < 2 {
            return Err(format!("{}: a route needs at least two vertices", p.display()));
        }
        let cell = 16.0;
        let mut grid: std::collections::HashMap<(i32, i32), Vec<u32>> = Default::default();
        for (i, q) in pts.iter().enumerate() {
            grid.entry(((q[0] / cell).floor() as i32, (q[2] / cell).floor() as i32))
                .or_default()
                .push(i as u32);
        }
        Ok(TsvRoute { pts, cum, half, spacing, n_cp, grid, cell })
    }
}

impl Route for TsvRoute {
    fn progress(&self, pos: [f32; 3]) -> Progress {
        let cx = (pos[0] / self.cell).floor() as i32;
        let cz = (pos[2] / self.cell).floor() as i32;
        let mut best = (f32::INFINITY, usize::MAX);
        for dx in -1..=1 {
            for dz in -1..=1 {
                if let Some(l) = self.grid.get(&(cx + dx, cz + dz)) {
                    for &i in l {
                        let i = i as usize;
                        let d = (pos[0] - self.pts[i][0]).powi(2)
                            + (pos[1] - self.pts[i][1]).powi(2)
                            + (pos[2] - self.pts[i][2]).powi(2);
                        if d < best.0 {
                            best = (d, i);
                        }
                    }
                }
            }
        }
        if best.1 == usize::MAX {
            for i in 0..self.pts.len() {
                let d = (pos[0] - self.pts[i][0]).powi(2)
                    + (pos[1] - self.pts[i][1]).powi(2)
                    + (pos[2] - self.pts[i][2]).powi(2);
                if d < best.0 {
                    best = (d, i);
                }
            }
        }
        let i = best.1;
        let j = (i + 1).min(self.pts.len() - 1);
        let (tx, tz) = (self.pts[j][0] - self.pts[i][0], self.pts[j][2] - self.pts[i][2]);
        let (dx, dz) = (pos[0] - self.pts[i][0], pos[2] - self.pts[i][2]);
        let lat_abs = ((pos[0] - self.pts[i][0]).powi(2) + (pos[2] - self.pts[i][2]).powi(2)).sqrt();
        let lateral = if tx * dz - tz * dx < 0.0 { lat_abs } else { -lat_abs };
        Progress { s: self.cum[i], lateral, on_route: lat_abs <= self.half[i] }
    }
    fn length(&self) -> f32 {
        *self.cum.last().unwrap()
    }
    fn spacing(&self) -> f32 {
        self.spacing
    }
    fn n_checkpoints(&self) -> u32 {
        self.n_cp
    }
}

/// Pad a tape with neutral input to exactly `n` ticks. See the module header:
/// the container's archive length is a hard bound on how long a run may be.
pub fn pad_to(tape: &[Input], n: usize) -> Vec<Input> {
    let mut v = tape.to_vec();
    v.resize(n, Input::NEUTRAL);
    v
}

/// Where the pieces live on disk, in one place.
pub struct Paths {
    pub server: PathBuf,
    pub map: PathBuf,
    pub template: PathBuf,
    pub work: PathBuf,
}

/// Turn a `SimResult` into a `Verdict`, refusing to read a declaration as a
/// simulation.
///
/// This is three lines and it has its own function because a sixth copy of
/// this parser, with no sanity bound on the time it accepts, was one of the
/// defects found in the last audit of this layer. The rule: `time_ms` is what
/// the server SIMULATED and `declared_ms` is what the FILE CLAIMS, and a
/// careless parser reads the second one as a finish on a DNF.
pub fn verdict_of(r: &ghost::oracle::SimResult) -> Verdict {
    match r.time_ms {
        Some(ms) => Verdict::Finish { ms },
        None => Verdict::Dnf { cps: r.cps.unwrap_or(0) },
    }
}
