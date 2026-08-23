//! Separating *the car was in the air* from *the model has nothing there*.
//!
//! `probe` measures the distance from a sample to the nearest surface. A
//! sample with no surface in reach was, until this module existed, counted as
//! a hole in the model — and on a map driven with big air that is simply
//! false. A car twelve metres above a road the model DOES have is not a
//! coverage failure, and a car on the inside of a loop has its road beside it
//! rather than under it.
//!
//! Two things fix that, and neither of them is the model:
//!
//! * **Ask the recording whether the car was touching anything.**
//!   `is_ground_contact` is a DERIVED bit (byte 89 & 0x01) that nothing in
//!   this project had cross-checked, so it is used here *with its control
//!   printed beside it*: `vy` is VERIFIED, and differentiating it gives the
//!   map's own gravity (about −24.6 m/s²) in free flight against near zero
//!   under support. A map where the bit does not split those two populations
//!   is a map where this classification means nothing, and it says so out
//!   loud rather than quietly moving the coverage number.
//! * **Probe along the car's own down axis, not straight down.** The
//!   quaternion is VERIFIED too. On flat ground the two are the same question;
//!   on a loop only one of them is the right one. Its control is also printed:
//!   how much of the run was upright at all.
//!
//! The vertical plumb is kept alongside, because the first corpus run
//! measured it and a before/after comparison has to be like for like.

use crate::geom::from_quat;
use crate::probe::Index;
use gbx::record::Sample;
use std::collections::BTreeMap;

/// Vertical acceleration below this is free flight and nothing else. Measured
/// gravity on these maps is −24.3 to −24.9 m/s²; a car supported by a road
/// reads within a couple of m/s² of zero, so anything under −15 is
/// unambiguous and the boundary samples of a flight fall on the safe side.
pub const FREEFALL: f32 = -15.0;

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
    /// alone. The control on the contact bit.
    pub fn falling(&self) -> bool {
        self.accel_y < FREEFALL
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
            let m = from_quat([s.qx as f32, s.qy as f32, s.qz as f32, s.qw as f32], [0.0; 3]);
            Motion {
                p: [s.x as f32, s.y as f32, s.z as f32],
                contact: s.is_ground_contact,
                accel_y,
                up: [m[3], m[4], m[5]],
            }
        })
        .collect()
}

/// Which of the four things a sample is, judged along the car's own down axis.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// Standing on the model, within `RESTING`.
    Resting,
    /// Supported by something the model has, but further off than a car rests:
    /// a small hop the contact bit did not catch, or geometry in roughly the
    /// right place and not quite the right height.
    Loose,
    /// The recording says airborne. The model owes nothing here.
    Airborne,
    /// The recording says the car was on something and the model has nothing
    /// under it. **This is the coverage failure**, and the only class worth
    /// building geometry for.
    Missing,
}

pub struct Verdict {
    pub classes: Vec<Class>,
    /// gap straight down, or NaN where nothing was within `reach` — the number
    /// the first corpus run reported
    pub gaps: Vec<f32>,
    /// distance along the car's own DOWN axis, or NaN
    pub body: Vec<f32>,
    pub materials: Vec<Option<String>>,
    /// mean vertical acceleration under each value of the contact bit, and how
    /// many samples each covers: the control on the bit
    pub accel_contact: (f32, usize),
    pub accel_air: (f32, usize),
    /// how often the contact bit and the free-fall test agree
    pub bit_vs_freefall: (usize, usize),
    /// the angle between the car's own up axis and world up, per sample, in
    /// degrees: the control on the quaternion the down-axis probe is aimed by,
    /// which on a flat map has to be a couple of degrees
    pub tilt: Vec<f32>,
}

impl Verdict {
    pub fn of(index: &Index, ms: &[Motion], reach: f32) -> Verdict {
        let mut v = Verdict {
            classes: Vec::with_capacity(ms.len()),
            gaps: Vec::with_capacity(ms.len()),
            body: Vec::with_capacity(ms.len()),
            materials: Vec::with_capacity(ms.len()),
            accel_contact: (0.0, 0),
            accel_air: (0.0, 0),
            bit_vs_freefall: (0, 0),
            tilt: Vec::with_capacity(ms.len()),
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
            v.tilt.push(m.up[1].clamp(-1.0, 1.0).acos().to_degrees());
            let plumb = index.below(m.p, reach);
            v.gaps.push(plumb.as_ref().map_or(f32::NAN, |h| h.gap));
            let down = [-m.up[0], -m.up[1], -m.up[2]];
            let hit = index.along(m.p, down, reach).or(plumb);
            let gap = hit.as_ref().map_or(f32::NAN, |h| h.gap);
            v.materials.push(hit.map(|h| h.material));
            v.body.push(gap);
            v.classes.push(if gap.is_finite() && gap <= RESTING {
                Class::Resting
            } else if gap.is_finite() {
                if m.contact {
                    Class::Loose
                } else {
                    Class::Airborne
                }
            } else if m.contact {
                Class::Missing
            } else {
                Class::Airborne
            });
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

    /// The samples the model is answerable for: the recording says the car was
    /// standing on something.
    pub fn owed(&self) -> usize {
        self.count(Class::Resting) + self.count(Class::Loose) + self.count(Class::Missing)
    }

    /// The honest coverage number: of the samples the model owes, how many it
    /// has a surface for.
    pub fn covered_fraction(&self) -> f32 {
        let owed = self.owed();
        if owed == 0 {
            return f32::NAN;
        }
        (owed - self.count(Class::Missing)) as f32 / owed as f32
    }

    /// The raw number the first corpus run reported: any sample with any
    /// surface straight below within reach, over every sample. Kept so a
    /// before/after comparison against those transcripts is like for like.
    pub fn raw_fraction(&self) -> f32 {
        let hits = self.gaps.iter().filter(|g| g.is_finite()).count();
        hits as f32 / self.gaps.len().max(1) as f32
    }

    pub fn median_gap(&self) -> f32 {
        pct(&sorted(&self.gaps), 0.5)
    }
    pub fn gap_pct(&self, q: f64) -> f32 {
        pct(&sorted(&self.gaps), q)
    }
    /// The ride height along the car's own down axis — the true one, and on a
    /// banked road a smaller number than the plumb gap by exactly the cosine.
    pub fn median_ride(&self) -> f32 {
        pct(&sorted(&self.body), 0.5)
    }
    pub fn ride_pct(&self, q: f64) -> f32 {
        pct(&sorted(&self.body), q)
    }

    /// The half-width of the tightest window holding half the plumb gaps — a
    /// spread that is not fooled by the flight phases a probe cannot see,
    /// unlike an rms.
    pub fn tightest_half(&self) -> f32 {
        let g = sorted(&self.gaps);
        let n = g.len();
        if n < 2 {
            return f32::NAN;
        }
        let w = n / 2;
        (0..=(n - w - 1)).map(|i| g[i + w] - g[i]).fold(f32::INFINITY, f32::min) / 2.0
    }

    fn sorted_tilt(&self) -> Vec<f32> {
        sorted(&self.tilt)
    }

    /// The median tilt of the car, in degrees. The control on the quaternion:
    /// a flat map reads a couple of degrees, and a run mostly on a loop reads
    /// tens.
    pub fn median_tilt(&self) -> f32 {
        pct(&self.sorted_tilt(), 0.5)
    }

    /// What the car was standing on, by material.
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

fn sorted(v: &[f32]) -> Vec<f32> {
    let mut g: Vec<f32> = v.iter().copied().filter(|g| g.is_finite()).collect();
    g.sort_by(|a, b| a.partial_cmp(b).unwrap());
    g
}

fn pct(g: &[f32], q: f64) -> f32 {
    if g.is_empty() {
        return f32::NAN;
    }
    g[(((g.len() - 1) as f64) * q).round() as usize]
}
