//! Label a readout's glyph alphabet without reading it.
//!
//! Hand-labelling needs a legible frame per glyph, and on a 10x15 box over
//! backgrounds from a dark tunnel to a white wall there may not be one. Three
//! facts make the eye unnecessary here:
//!
//! * the same glyph drawn on two frames is the same bitmap, so **clustering**
//!   the boxes recovers the alphabet up to naming;
//! * the field is a percentage that **counts down through every value** as the
//!   tyres dry, so a descending run spells its own labels; and
//! * the channel's law — every decrease is 1 or 2 units per 50 ms, or a reset
//!   to zero — says which descending runs are real.
//!
//! So: cluster, then solve the naming as a constraint problem against the law.
//! What is left for a human is a sanity glance at ten averaged bitmaps, not a
//! reading of two thousand noisy ones.
//!
//! The clustering is deliberately crude — one pass, fixed radius, correlation
//! as the metric. A cluster count other than ten is the finding, not a
//! parameter to tune until it is ten: eleven means one glyph renders two ways
//! (sub-pixel phase, or the field shifting), and nine means two glyphs are
//! being merged and no labelling can separate them afterwards.

use crate::digits::Patch;

pub struct Cluster {
    pub mean: Vec<f32>,
    pub n: usize,
    /// (frame index, cell index) of every member.
    pub members: Vec<(u64, usize)>,
}

pub struct Clusters {
    pub w: usize,
    pub h: usize,
    pub c: Vec<Cluster>,
}

impl Clusters {
    pub fn new(w: usize, h: usize) -> Clusters {
        Clusters { w, h, c: Vec::new() }
    }

    fn dot(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    /// Assign one patch, creating a cluster if nothing is close enough.
    pub fn add(&mut self, p: &Patch, at: (u64, usize), radius: f32) {
        let mut best = (usize::MAX, -2.0f32);
        for (i, c) in self.c.iter().enumerate() {
            let s = Self::dot(&p.v, &c.mean);
            if s > best.1 {
                best = (i, s);
            }
        }
        if best.1 >= radius {
            let c = &mut self.c[best.0];
            let n = c.n as f32;
            for i in 0..c.mean.len() {
                c.mean[i] = (c.mean[i] * n + p.v[i]) / (n + 1.0);
            }
            // Re-normalise so the metric stays a correlation.
            let norm = c.mean.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
            for x in c.mean.iter_mut() {
                *x /= norm;
            }
            c.n += 1;
            c.members.push(at);
        } else {
            self.c.push(Cluster { mean: p.v.clone(), n: 1, members: vec![at] });
        }
    }

    /// Drop clusters with fewer than `min` members: a glyph seen twice in two
    /// thousand frames is a compression artefact, not a glyph.
    pub fn prune(&mut self, min: usize) -> usize {
        let before = self.c.len();
        self.c.retain(|c| c.n >= min);
        before - self.c.len()
    }

    pub fn print_ascii(&self, o: &mut impl std::io::Write) {
        let ramp: Vec<char> = " .:-=+*#%@".chars().collect();
        for (i, c) in self.c.iter().enumerate() {
            let (lo, hi) = c.mean.iter().fold((f32::MAX, f32::MIN), |a, v| (a.0.min(*v), a.1.max(*v)));
            writeln!(o, "# cluster {i}: {} members", c.n).unwrap();
            for y in 0..self.h {
                let mut s = String::new();
                for x in 0..self.w {
                    let v = ((c.mean[y * self.w + x] - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0);
                    s.push(ramp[(v * (ramp.len() - 1) as f32).round() as usize]);
                }
                writeln!(o, "{s}").unwrap();
            }
        }
    }
}
