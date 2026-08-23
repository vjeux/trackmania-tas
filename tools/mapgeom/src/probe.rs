//! The check that decides whether the model is worth anything: drop a plumb
//! line from every sample of a run and measure the gap to the surface below.
//!
//! A model that looks plausible and puts the car three metres under the track
//! is worse than no model, and a rendering cannot tell you which one you have.
//! So the model is graded, not admired: for each ghost sample, find the
//! highest triangle whose vertical projection contains the sample and that
//! lies below it, and report the distribution of `sample.y - surface.y` — plus
//! the physics material the car was over, which is a second, independent read
//! on whether the right surface was found.
//!
//! **What a good answer looks like.** The gap is not zero: a ghost's position
//! is the car's origin, which rides above the contact patch. What it should be
//! is *tight and constant* — one narrow mode across the whole run. A bimodal
//! or wandering gap means blocks are being placed at the wrong height, and a
//! large "no surface below" fraction means they are being placed in the wrong
//! place entirely.

use crate::scene::Scene;
use std::collections::BTreeMap;

pub struct Hit {
    pub gap: f32,
    pub material: String,
}

/// A uniform grid over XZ holding triangle references, so a plumb line does
/// not have to test a million triangles.
pub struct Index {
    cell: f32,
    origin: [f32; 2],
    nx: usize,
    nz: usize,
    /// bucket -> (group index, triangle index)
    buckets: Vec<Vec<(u32, u32)>>,
    groups: Vec<(String, Vec<[f32; 3]>, Vec<[u32; 3]>)>,
}

impl Index {
    pub fn build(scene: &Scene, cell: f32) -> Index {
        let (lo, hi) = scene.bounds().unwrap_or(([0.0; 3], [1.0; 3]));
        let nx = (((hi[0] - lo[0]) / cell).ceil() as usize + 1).max(1);
        let nz = (((hi[2] - lo[2]) / cell).ceil() as usize + 1).max(1);
        let mut idx = Index {
            cell,
            origin: [lo[0], lo[2]],
            nx,
            nz,
            buckets: vec![Vec::new(); nx * nz],
            groups: Vec::new(),
        };
        for (name, g) in &scene.groups {
            idx.groups.push((name.clone(), g.verts.clone(), g.tris.clone()));
        }
        for (gi, (_, verts, tris)) in idx.groups.iter().enumerate() {
            for (ti, t) in tris.iter().enumerate() {
                let p = [
                    verts[t[0] as usize],
                    verts[t[1] as usize],
                    verts[t[2] as usize],
                ];
                let minx = p[0][0].min(p[1][0]).min(p[2][0]);
                let maxx = p[0][0].max(p[1][0]).max(p[2][0]);
                let minz = p[0][2].min(p[1][2]).min(p[2][2]);
                let maxz = p[0][2].max(p[1][2]).max(p[2][2]);
                let (i0, i1) = (bucket(minx, lo[0], cell, nx), bucket(maxx, lo[0], cell, nx));
                let (j0, j1) = (bucket(minz, lo[2], cell, nz), bucket(maxz, lo[2], cell, nz));
                for j in j0..=j1 {
                    for i in i0..=i1 {
                        idx.buckets[j * nx + i].push((gi as u32, ti as u32));
                    }
                }
            }
        }
        idx
    }

    pub fn triangle_count(&self) -> usize {
        self.groups.iter().map(|(_, _, t)| t.len()).sum()
    }

    /// Every surface directly under (or over) `(x, z)`, highest first: the
    /// plumb probe this project used to do by driving a car at a spot and
    /// watching where it stopped.
    pub fn column(&self, x: f32, z: f32) -> Vec<(f32, String)> {
        let i = bucket(x, self.origin[0], self.cell, self.nx);
        let j = bucket(z, self.origin[1], self.cell, self.nz);
        let mut out = Vec::new();
        for (gi, ti) in &self.buckets[j * self.nx + i] {
            let (name, verts, tris) = &self.groups[*gi as usize];
            let t = tris[*ti as usize];
            if let Some(y) = height_at(
                verts[t[0] as usize],
                verts[t[1] as usize],
                verts[t[2] as usize],
                x,
                z,
            ) {
                out.push((y, name.clone()));
            }
        }
        out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        out.dedup_by(|a, b| (a.0 - b.0).abs() < 0.002 && a.1 == b.1);
        out
    }

    /// The highest surface at or below `p`, within `reach` metres.
    pub fn below(&self, p: [f32; 3], reach: f32) -> Option<Hit> {
        let i = bucket(p[0], self.origin[0], self.cell, self.nx);
        let j = bucket(p[2], self.origin[1], self.cell, self.nz);
        let mut best: Option<(f32, usize)> = None;
        for (gi, ti) in &self.buckets[j * self.nx + i] {
            let (_, verts, tris) = &self.groups[*gi as usize];
            let t = tris[*ti as usize];
            let a = verts[t[0] as usize];
            let b = verts[t[1] as usize];
            let c = verts[t[2] as usize];
            if let Some(y) = height_at(a, b, c, p[0], p[2]) {
                if y <= p[1] + 0.001 && p[1] - y <= reach {
                    if best.map_or(true, |(by, _)| y > by) {
                        best = Some((y, *gi as usize));
                    }
                }
            }
        }
        best.map(|(y, gi)| Hit { gap: p[1] - y, material: self.groups[gi].0.clone() })
    }
}

fn bucket(v: f32, origin: f32, cell: f32, n: usize) -> usize {
    (((v - origin) / cell).floor().max(0.0) as usize).min(n - 1)
}

/// The height of the plane of triangle `abc` at `(x, z)`, if `(x, z)` is
/// inside the triangle's vertical projection.
fn height_at(a: [f32; 3], b: [f32; 3], c: [f32; 3], x: f32, z: f32) -> Option<f32> {
    let d = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
    if d.abs() < 1e-9 {
        return None; // vertical triangle: no roof, no floor
    }
    let w0 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / d;
    let w1 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / d;
    let w2 = 1.0 - w0 - w1;
    const E: f32 = -1e-4;
    if w0 < E || w1 < E || w2 < E {
        return None;
    }
    Some(w0 * a[1] + w1 * b[1] + w2 * c[1])
}

pub struct Report {
    pub samples: usize,
    pub hits: usize,
    pub gaps: Vec<f32>,
    pub materials: BTreeMap<String, usize>,
}

impl Report {
    pub fn of(index: &Index, points: &[[f32; 3]], reach: f32) -> Report {
        let mut r =
            Report { samples: points.len(), hits: 0, gaps: Vec::new(), materials: BTreeMap::new() };
        for p in points {
            if let Some(h) = index.below(*p, reach) {
                r.hits += 1;
                r.gaps.push(h.gap);
                *r.materials.entry(h.material).or_insert(0) += 1;
            }
        }
        r.gaps.sort_by(|a, b| a.partial_cmp(b).unwrap());
        r
    }

    /// How many samples are RESTING: sitting within `max_gap` of a surface,
    /// and the median of those gaps.
    ///
    /// This is the fit criterion, and choosing it correctly matters more than
    /// it looks. Two wrong criteria were tried first and both produce a
    /// confident wrong answer:
    ///
    /// * *"how many samples have anything under them"* picks whichever height
    ///   drops the run onto the stadium FLOOR, because grass is everywhere.
    ///   On 134672 that scored 93 % of samples over a surface at a wandering
    ///   3.2 m.
    /// * *"the largest group of samples sharing a gap"* is degenerate under a
    ///   vertical shift: lowering the whole model by a metre raises every gap
    ///   by a metre and the same samples still share one. It also picked the
    ///   wrong cell row on maps with a deck under the road -- 146612 fitted a
    ///   consistent 2.048 m.
    ///
    /// A car rests CENTIMETRES above what it is on, so the window is anchored
    /// at zero. Measured ride heights on maps this model reproduces run
    /// 0.013 - 0.073 m.
    pub fn resting(&self, max_gap: f32) -> (usize, f32) {
        let n = self.gaps.partition_point(|g| *g <= max_gap);
        if n == 0 {
            return (0, f32::NAN);
        }
        (n, self.gaps[n / 2])
    }

    pub fn pct(&self, q: f64) -> f32 {
        if self.gaps.is_empty() {
            return f32::NAN;
        }
        let i = ((self.gaps.len() - 1) as f64 * q).round() as usize;
        self.gaps[i]
    }
    pub fn median(&self) -> f32 {
        self.pct(0.5)
    }
    /// The half-width of the tightest window holding half the samples — a
    /// spread that is not fooled by the flight phases a plumb probe cannot
    /// see, unlike an rms.
    pub fn tightest_half(&self) -> f32 {
        let n = self.gaps.len();
        if n < 2 {
            return f32::NAN;
        }
        let w = n / 2;
        let mut best = f32::INFINITY;
        for i in 0..=(n - w - 1) {
            best = best.min(self.gaps[i + w] - self.gaps[i]);
        }
        best / 2.0
    }
}
