//! Separating *the car was in the air* from *the model has nothing there*.
//!
//! `probe` measures the gap from a sample down to the nearest surface within
//! `reach` metres. A sample with no surface in reach is reported as a miss —
//! and until this module existed, every such miss was counted as a hole in the
//! model. On a map driven with big air that is simply false: a car twelve
//! metres above a road the model DOES have is not a coverage failure.
//!
//! The discriminator has to come from somewhere the model cannot influence, so
//! it comes from the ghost:
//!
//! * **`vy`** is VERIFIED in the recording (`gbx::record`). Differentiate it
//!   and a free flight reads the map's own gravity — about −24.6 m/s² — while
//!   a car being held up by a road reads near zero. That is a physical
//!   measurement of *airborne*, made without the model.
//! * **`is_ground_contact`** is a DERIVED bit (byte 89 & 0x01) that nothing in
//!   this project had cross-checked. It is used as the classifier here, and
//!   the free-fall measurement above is run beside it as the control: this
//!   module reports the mean vertical acceleration under each value of the
//!   bit, so a map where the bit means something else says so out loud.
//!
//! The second thing a vertical plumb line cannot judge is a car that is not
//! upright. On a loop or a wall ride the surface the car is on is beside it,
//! not under it, so those samples are counted separately rather than being
//! blamed on the model. The car's own up axis comes out of the recording's
//! quaternion.

use crate::geom::from_quat;
use crate::probe::Index;
use gbx::record::Sample;
use std::collections::BTreeMap;

/// Vertical acceleration below this is free flight and nothing else. Measured
/// gravity on these maps is −24.3 to −24.9 m/s²; a car supported by a road
/// reads within a couple of m/s² of zero, so anything under −15 is
/// unambiguous and the boundary samples of a flight fall on the safe side.
pub const FREEFALL: f32 = -15.0;

/// How far the car's own up axis may lie from world up before a plumb line
/// stops being the right question. 60° admits every banked road measured here
/// and excludes loops and wall rides.
pub const UPRIGHT_COS: f32 = 0.5;

/// How close to a surface a sample has to be to count as RESTING on it.
/// Measured ride heights on maps this model reproduces run 0.013 - 0.073 m.
pub const RESTING: f32 = 0.25;

/// What one ghost sample is, before the model is consulted at all.
pub struct Motion {
    pub p: [f32; 3],
    /// The recording's own contact bit.
    pub contact: bool,
    /// Vertical acceleration, m/s², from the recording's `vy`. NaN at the ends.
    pub accel_y: f32,
    /// The car's up axis, from the recording's quaternion.
    pub up: [f32; 3],
}

impl Motion {
    /// The free-fall test: this sample is unsupported, measured from `vy`
    /// alone.
    pub fn falling(&self) -> bool {
        self.accel_y < FREEFALL
    }
    pub fn upright(&self) -> bool {
        self.up[1] >= UPRIGHT_COS
    }
}

/// Read a recording's samples into the form the coverage classifier needs.
pub fn motions(samples: &[Sample]) -> Vec<Motion> {
    let n = samples.len();
    (0..n)
        .map(|i| {
            let s = &samples[i];
            let accel_y = if i == 0 || i + 1 >= n {
                f32::NAN
            } else {
                let dt = (samples[i + 1].time_ms - samples[i - 1].time_ms) as f64 / 1000.0;
                if dt <= 0.0 {
                    f32::NAN
                } else {
                    ((samples[i + 1].vy - samples[i - 1].vy) / dt) as f32
                }
            };
            let m = from_quat(
                [s.qx as f32, s.qy as f32, s.qz as f32, s.qw as f32],
                [0.0; 3],
            );
            Motion {
                p: [s.x as f32, s.y as f32, s.z as f32],
                contact: s.is_ground_contact,
                accel_y,
                up: [m[3], m[4], m[5]],
            }
        })
        .collect()
}

/// Which of the four things a sample is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// On the model, within `RESTING`.
    Resting,
    /// Supported by something the model has, but further off than a car rests.
    /// Either a small hop the contact bit did not catch, or geometry in
    /// roughly the right place and not the right height.
    Loose,
    /// The recording says airborne. The model owes nothing here.
    Airborne,
    /// The recording says the car was on something, and the model has nothing
    /// under it. **This is the coverage failure**, and the only class worth
    /// building geometry for.
    Missing,
    /// The car was not upright — a loop or a wall ride. A vertical plumb line
    /// cannot judge these either way.
    Tilted,
}

pub struct Verdict {
    pub classes: Vec<Class>,
    /// gap below each sample, or NaN where nothing was within `reach`
    pub gaps: Vec<f32>,
    pub materials: Vec<Option<String>>,
    /// mean vertical acceleration under each value of the contact bit, and how
    /// many samples each covers: the control on the bit
    pub accel_contact: (f32, usize),
    pub accel_air: (f32, usize),
    /// how often the contact bit and the free-fall test disagree
    pub bit_vs_freefall: (usize, usize),
}

impl Verdict {
    pub fn of(index: &Index, ms: &[Motion], reach: f32) -> Verdict {
        let mut v = Verdict {
            classes: Vec::with_capacity(ms.len()),
            gaps: Vec::with_capacity(ms.len()),
            materials: Vec::with_capacity(ms.len()),
            accel_contact: (0.0, 0),
            accel_air: (0.0, 0),
            bit_vs_freefall: (0, 0),
        };
        let (mut sc, mut sa) = (0.0f64, 0.0f64);
        for m in ms {
            if m.accel_y.is_finite() {
                if m.contact {
                    sc += m.accel_y as f64;
                    v.accel_contact.1 += 1;
                } else {
                    sa += m.accel_y as f64;
                    v.accel_air.1 += 1;
                }
                v.bit_vs_freefall.1 += 1;
                if m.contact != m.falling() {
                    v.bit_vs_freefall.0 += 1;
                }
            }
            let hit = index.below(m.p, reach);
            let (gap, mat) = match &hit {
                Some(h) => (h.gap, Some(h.material.clone())),
                None => (f32::NAN, None),
            };
            let class = if !m.upright() {
                Class::Tilted
            } else if gap.is_finite() && gap <= RESTING {
                Class::Resting
            } else if !m.contact {
                Class::Airborne
            } else if gap.is_finite() {
                Class::Loose
            } else {
                Class::Missing
            };
            v.classes.push(class);
            v.gaps.push(gap);
            v.materials.push(mat);
        }
        if v.accel_contact.1 > 0 {
            v.accel_contact.0 = (sc / v.accel_contact.1 as f64) as f32;
        }
        if v.accel_air.1 > 0 {
            v.accel_air.0 = (sa / v.accel_air.1 as f64) as f32;
        }
        v
    }

    pub fn count(&self, c: Class) -> usize {
        self.classes.iter().filter(|k| **k == c).count()
    }

    /// The samples the model is answerable for: upright, and the recording
    /// says the car was on something.
    pub fn owed(&self) -> usize {
        self.count(Class::Resting) + self.count(Class::Loose) + self.count(Class::Missing)
    }

    /// The honest coverage number: of the samples the model is answerable for,
    /// how many it has a surface for.
    pub fn covered_fraction(&self) -> f32 {
        let owed = self.owed();
        if owed == 0 {
            return f32::NAN;
        }
        (self.count(Class::Resting) + self.count(Class::Loose)) as f32 / owed as f32
    }

    /// The raw number the earlier transcripts report: any sample with any
    /// surface within reach, over every sample. Kept so a before/after
    /// comparison against those transcripts is like for like.
    pub fn raw_fraction(&self) -> f32 {
        let hits = self.gaps.iter().filter(|g| g.is_finite()).count();
        hits as f32 / self.gaps.len().max(1) as f32
    }

    pub fn median_gap(&self) -> f32 {
        self.pct(0.5)
    }

    /// A quantile of the gap, over the samples that had a surface.
    pub fn pct(&self, q: f64) -> f32 {
        let g = self.sorted_gaps();
        if g.is_empty() {
            return f32::NAN;
        }
        g[(((g.len() - 1) as f64) * q).round() as usize]
    }

    /// The half-width of the tightest window holding half the gaps — a spread
    /// that is not fooled by the flight phases a plumb probe cannot see,
    /// unlike an rms.
    pub fn tightest_half(&self) -> f32 {
        let g = self.sorted_gaps();
        let n = g.len();
        if n < 2 {
            return f32::NAN;
        }
        let w = n / 2;
        let mut best = f32::INFINITY;
        for i in 0..=(n - w - 1) {
            best = best.min(g[i + w] - g[i]);
        }
        best / 2.0
    }

    fn sorted_gaps(&self) -> Vec<f32> {
        let mut g: Vec<f32> = self.gaps.iter().copied().filter(|g| g.is_finite()).collect();
        g.sort_by(|a, b| a.partial_cmp(b).unwrap());
        g
    }

    /// What the car was over, by material, for the samples the model answers.
    pub fn materials(&self) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for (c, m) in self.classes.iter().zip(&self.materials) {
            if matches!(c, Class::Resting | Class::Loose) {
                if let Some(m) = m {
                    *out.entry(m.clone()).or_insert(0) += 1;
                }
            }
        }
        out
    }
}
