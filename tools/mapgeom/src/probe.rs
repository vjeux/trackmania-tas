//! The check that decides whether the model is worth anything: drop a plumb
//! line from every sample of a run and measure the gap to the surface below.
//!
//! A model that looks plausible and puts the car three metres under the track
//! is worse than no model, and a rendering cannot tell you which one you have.
//! So the model is graded, not admired: for each ghost sample, find the
//! highest triangle whose vertical projection contains the sample and that
//! lies below it, and report `sample.y - surface.y` plus the physics material
//! the car was over.
//!
//! This module answers only *what is under this point*. Deciding what a
//! missing answer MEANS — a hole in the model, or a car in the air — needs the
//! recording's own dynamics and lives in `coverage`.
//!
//! **What a good answer looks like.** The gap is not zero: a ghost's position
//! is the car's origin, which rides above the contact patch. What it should be
//! is *tight and constant* — one narrow mode across the whole run. A bimodal
//! or wandering gap means blocks are being placed at the wrong height.

use crate::scene::Scene;


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
    /// Build the plumb/ray index over a scene.
    ///
    /// **`NotCollidable` and `OffZone` triangles are left out.** They are real
    /// geometry and a render should show them, but a car cannot rest on one,
    /// and a probe that counts them reports coverage the game does not have.
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
            if !crate::scene::is_collidable(name) {
                continue;
            }
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

    /// The first surface a ray from `p` in direction `d` meets, within `reach`
    /// metres.
    ///
    /// A vertical plumb line asks *what is under this point*, which is the
    /// wrong question for a car on a loop or a wall ride: the surface holding
    /// it up is beside it, or above it. Firing along the car's own **down**
    /// axis asks *what is this car standing on*, which is the question the
    /// model is being graded against, and it is the same question on flat
    /// ground.
    pub fn along(&self, p: [f32; 3], d: [f32; 3], reach: f32) -> Option<Hit> {
        // reach is metres and a cell is 32 m, so the ray cannot leave the 3x3
        // of columns around its origin before it runs out.
        let ci = bucket(p[0], self.origin[0], self.cell, self.nx) as i32;
        let cj = bucket(p[2], self.origin[1], self.cell, self.nz) as i32;
        let mut best: Option<(f32, usize)> = None;
        for dj in -1..=1 {
            for di in -1..=1 {
                let (i, j) = (ci + di, cj + dj);
                if i < 0 || j < 0 || i >= self.nx as i32 || j >= self.nz as i32 {
                    continue;
                }
                for (gi, ti) in &self.buckets[j as usize * self.nx + i as usize] {
                    let (_, verts, tris) = &self.groups[*gi as usize];
                    let t = tris[*ti as usize];
                    if let Some(dist) = ray_tri(
                        p,
                        d,
                        verts[t[0] as usize],
                        verts[t[1] as usize],
                        verts[t[2] as usize],
                    ) {
                        if dist <= reach && best.map_or(true, |(bd, _)| dist < bd) {
                            best = Some((dist, *gi as usize));
                        }
                    }
                }
            }
        }
        best.map(|(d, gi)| Hit { gap: d, material: self.groups[gi].0.clone() })
    }

    /// The nearest point of any triangle to `p`, within `radius` metres, and
    /// what material it belongs to.
    ///
    /// This is the question that separates *the model has nothing here* from
    /// *the model has this, a metre to the left*: a plumb line that finds
    /// nothing under a car sitting 0.4 m from the edge of a road is measuring
    /// a road that is too narrow, not a road that is absent, and the two want
    /// completely different work.
    pub fn nearest(&self, p: [f32; 3], radius: f32) -> Option<(f32, String)> {
        let rings = (radius / self.cell).ceil() as i32;
        let ci = bucket(p[0], self.origin[0], self.cell, self.nx) as i32;
        let cj = bucket(p[2], self.origin[1], self.cell, self.nz) as i32;
        let mut best: Option<(f32, usize)> = None;
        for dj in -rings..=rings {
            for di in -rings..=rings {
                let (i, j) = (ci + di, cj + dj);
                if i < 0 || j < 0 || i >= self.nx as i32 || j >= self.nz as i32 {
                    continue;
                }
                for (gi, ti) in &self.buckets[j as usize * self.nx + i as usize] {
                    let (_, verts, tris) = &self.groups[*gi as usize];
                    let t = tris[*ti as usize];
                    let d = point_tri_dist(
                        p,
                        verts[t[0] as usize],
                        verts[t[1] as usize],
                        verts[t[2] as usize],
                    );
                    if d <= radius && best.map_or(true, |(bd, _)| d < bd) {
                        best = Some((d, *gi as usize));
                    }
                }
            }
        }
        best.map(|(d, gi)| (d, self.groups[gi].0.clone()))
    }
}

/// Möller–Trumbore, two-sided: the distance along `d` from `p` to triangle
/// `abc`, or `None` if the ray misses. Two-sided because the collision hull of
/// a road is a shell and which way its normals face is not the question.
fn ray_tri(p: [f32; 3], d: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Option<f32> {
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let h = cross(d, e2);
    let det = dot(e1, h);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(p, a);
    let u = inv * dot(s, h);
    if !(-1e-5..=1.000_01).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = inv * dot(d, q);
    if v < -1e-5 || u + v > 1.000_01 {
        return None;
    }
    let t = inv * dot(e2, q);
    if t >= 0.0 {
        Some(t)
    } else {
        None
    }
}

/// Distance from a point to a triangle. Projects onto the plane, and when the
/// projection falls outside, takes the nearest of the three edges — the exact
/// distance, not a bounding-box estimate, because the answer decides whether a
/// surface is a hand's width away or genuinely absent.
fn point_tri_dist(p: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let n = cross(ab, ac);
    let nn = dot(n, n);
    if nn > 1e-18 {
        let ap = sub(p, a);
        let w = dot(cross(ab, ap), n) / nn;
        let v = dot(cross(ap, ac), n) / nn;
        let u = 1.0 - v - w;
        if u >= 0.0 && v >= 0.0 && w >= 0.0 {
            return (dot(ap, n) / nn.sqrt()).abs();
        }
    }
    seg_dist(p, a, b).min(seg_dist(p, b, c)).min(seg_dist(p, c, a))
}

fn seg_dist(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
    let ab = sub(b, a);
    let d = dot(ab, ab);
    let t = if d > 1e-18 { (dot(sub(p, a), ab) / d).clamp(0.0, 1.0) } else { 0.0 };
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
    dot(sub(p, q), sub(p, q)).sqrt()
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
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

