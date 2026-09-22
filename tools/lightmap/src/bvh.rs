//! A bounding-volume hierarchy over world-space triangles, for the baker's
//! visibility rays. Binned SAH build, iterative traversal, any-hit and
//! closest-hit queries. Plain f32, no SIMD: the baker is throughput-bound on
//! many cores, not on one.

use crate::geometry::{cross, dot, sub, V3};

#[derive(Clone, Copy, Debug)]
pub struct WTri {
    pub p0: V3,
    pub e1: V3,
    pub e2: V3,
    /// Which item (instance) the triangle belongs to (self-hit filtering).
    pub inst: u32,
}

#[derive(Clone, Copy, Debug)]
struct Node {
    bmin: V3,
    bmax: V3,
    /// Leaf: first triangle index; inner: left child index (right = left + 1).
    first: u32,
    /// Leaf: triangle count (> 0); inner: 0.
    count: u32,
}

pub struct Bvh {
    pub tris: Vec<WTri>,
    nodes: Vec<Node>,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub tri: u32,
}

fn tri_bounds(t: &WTri) -> (V3, V3) {
    let p1 = [t.p0[0] + t.e1[0], t.p0[1] + t.e1[1], t.p0[2] + t.e1[2]];
    let p2 = [t.p0[0] + t.e2[0], t.p0[1] + t.e2[1], t.p0[2] + t.e2[2]];
    let mut lo = t.p0;
    let mut hi = t.p0;
    for p in [p1, p2] {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

fn centroid(t: &WTri) -> V3 {
    let (lo, hi) = tri_bounds(t);
    [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5]
}

fn surface(lo: V3, hi: V3) -> f32 {
    let d = sub(hi, lo);
    let d = [d[0].max(0.0), d[1].max(0.0), d[2].max(0.0)];
    2.0 * (d[0] * d[1] + d[1] * d[2] + d[2] * d[0])
}

impl Bvh {
    pub fn build(mut tris: Vec<WTri>) -> Bvh {
        let n = tris.len();
        let mut nodes: Vec<Node> = Vec::with_capacity(2 * n.max(1));
        let bounds: Vec<(V3, V3)> = tris.iter().map(tri_bounds).collect();
        let cents: Vec<V3> = tris.iter().map(centroid).collect();
        let mut order: Vec<u32> = (0..n as u32).collect();
        // recursive build over index ranges
        struct Task {
            node: usize,
            lo: usize,
            hi: usize,
        }
        let root = Node { bmin: [0.0; 3], bmax: [0.0; 3], first: 0, count: 0 };
        nodes.push(root);
        let mut stack = vec![Task { node: 0, lo: 0, hi: n }];
        while let Some(Task { node, lo, hi }) = stack.pop() {
            let mut bmin = [f32::MAX; 3];
            let mut bmax = [f32::MIN; 3];
            let mut cmin = [f32::MAX; 3];
            let mut cmax = [f32::MIN; 3];
            for &i in &order[lo..hi] {
                let (a, b) = bounds[i as usize];
                let c = cents[i as usize];
                for k in 0..3 {
                    bmin[k] = bmin[k].min(a[k]);
                    bmax[k] = bmax[k].max(b[k]);
                    cmin[k] = cmin[k].min(c[k]);
                    cmax[k] = cmax[k].max(c[k]);
                }
            }
            nodes[node].bmin = bmin;
            nodes[node].bmax = bmax;
            let count = hi - lo;
            if count <= 4 {
                nodes[node].first = lo as u32;
                nodes[node].count = count as u32;
                continue;
            }
            // binned SAH over the centroid bounds, 12 bins per axis
            const BINS: usize = 12;
            let mut best: Option<(f32, usize, f32)> = None; // (cost, axis, split)
            for axis in 0..3 {
                let ext = cmax[axis] - cmin[axis];
                if ext <= 1e-6 {
                    continue;
                }
                let mut bin_count = [0usize; BINS];
                let mut bin_lo = [[f32::MAX; 3]; BINS];
                let mut bin_hi = [[f32::MIN; 3]; BINS];
                for &i in &order[lo..hi] {
                    let c = cents[i as usize][axis];
                    let b = (((c - cmin[axis]) / ext) * BINS as f32) as usize;
                    let b = b.min(BINS - 1);
                    bin_count[b] += 1;
                    let (a, bb) = bounds[i as usize];
                    for k in 0..3 {
                        bin_lo[b][k] = bin_lo[b][k].min(a[k]);
                        bin_hi[b][k] = bin_hi[b][k].max(bb[k]);
                    }
                }
                // sweep
                let mut left_area = [0f32; BINS];
                let mut left_n = [0usize; BINS];
                let (mut lo_acc, mut hi_acc, mut n_acc) = ([f32::MAX; 3], [f32::MIN; 3], 0usize);
                for b in 0..BINS - 1 {
                    n_acc += bin_count[b];
                    for k in 0..3 {
                        lo_acc[k] = lo_acc[k].min(bin_lo[b][k]);
                        hi_acc[k] = hi_acc[k].max(bin_hi[b][k]);
                    }
                    left_area[b] = if n_acc > 0 { surface(lo_acc, hi_acc) } else { 0.0 };
                    left_n[b] = n_acc;
                }
                let (mut lo_acc, mut hi_acc, mut n_acc) = ([f32::MAX; 3], [f32::MIN; 3], 0usize);
                for b in (1..BINS).rev() {
                    n_acc += bin_count[b];
                    for k in 0..3 {
                        lo_acc[k] = lo_acc[k].min(bin_lo[b][k]);
                        hi_acc[k] = hi_acc[k].max(bin_hi[b][k]);
                    }
                    let right_area = if n_acc > 0 { surface(lo_acc, hi_acc) } else { 0.0 };
                    let ln = left_n[b - 1];
                    if ln == 0 || n_acc == 0 {
                        continue;
                    }
                    let cost = left_area[b - 1] * ln as f32 + right_area * n_acc as f32;
                    if best.map_or(true, |(c, _, _)| cost < c) {
                        best = Some((cost, axis, cmin[axis] + ext * (b as f32 / BINS as f32)));
                    }
                }
            }
            let (axis, split) = match best {
                Some((_, a, s)) => (a, s),
                None => {
                    // all centroids coincide: leaf
                    nodes[node].first = lo as u32;
                    nodes[node].count = count as u32;
                    continue;
                }
            };
            // partition
            let slice = &mut order[lo..hi];
            let mut i = 0usize;
            let mut j = slice.len();
            while i < j {
                if cents[slice[i] as usize][axis] < split {
                    i += 1;
                } else {
                    j -= 1;
                    slice.swap(i, j);
                }
            }
            let mut mid = lo + i;
            if mid == lo || mid == hi {
                mid = (lo + hi) / 2;
            }
            let left = nodes.len();
            nodes.push(Node { bmin: [0.0; 3], bmax: [0.0; 3], first: 0, count: 0 });
            nodes.push(Node { bmin: [0.0; 3], bmax: [0.0; 3], first: 0, count: 0 });
            nodes[node].first = left as u32;
            nodes[node].count = 0;
            stack.push(Task { node: left, lo, hi: mid });
            stack.push(Task { node: left + 1, lo: mid, hi });
        }
        // reorder triangles into leaf order
        let reordered: Vec<WTri> = order.iter().map(|&i| tris[i as usize]).collect();
        tris = reordered;
        Bvh { tris, nodes }
    }

    #[inline]
    fn ray_box(o: V3, inv: V3, bmin: V3, bmax: V3, tmax: f32) -> bool {
        let mut t0 = 0f32;
        let mut t1 = tmax;
        for k in 0..3 {
            let a = (bmin[k] - o[k]) * inv[k];
            let b = (bmax[k] - o[k]) * inv[k];
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            t0 = t0.max(lo);
            t1 = t1.min(hi);
            if t0 > t1 {
                return false;
            }
        }
        true
    }

    /// Möller–Trumbore; returns t or None. Two-sided.
    #[inline]
    fn ray_tri(o: V3, d: V3, t: &WTri, tmax: f32) -> Option<f32> {
        let pvec = cross(d, t.e2);
        let det = dot(t.e1, pvec);
        if det.abs() < 1e-9 {
            return None;
        }
        let inv = 1.0 / det;
        let tvec = sub(o, t.p0);
        let u = dot(tvec, pvec) * inv;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let qvec = cross(tvec, t.e1);
        let v = dot(d, qvec) * inv;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let tt = dot(t.e2, qvec) * inv;
        if tt > 1e-5 && tt < tmax {
            Some(tt)
        } else {
            None
        }
    }

    /// Is anything within `tmax` along the ray (skipping triangles of
    /// instance `skip_inst` closer than `skip_dist`, i.e. the surface itself)?
    pub fn occluded(&self, o: V3, d: V3, tmax: f32, skip_inst: u32, skip_dist: f32) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        let inv = [1.0 / d[0], 1.0 / d[1], 1.0 / d[2]];
        let mut stack: [u32; 64] = [0; 64];
        let mut sp = 0usize;
        stack[sp] = 0;
        sp += 1;
        while sp > 0 {
            sp -= 1;
            let n = &self.nodes[stack[sp] as usize];
            if !Self::ray_box(o, inv, n.bmin, n.bmax, tmax) {
                continue;
            }
            if n.count > 0 {
                for i in n.first..n.first + n.count {
                    let t = &self.tris[i as usize];
                    if let Some(tt) = Self::ray_tri(o, d, t, tmax) {
                        if t.inst == skip_inst && tt < skip_dist {
                            continue;
                        }
                        return true;
                    }
                }
            } else {
                stack[sp] = n.first;
                stack[sp + 1] = n.first + 1;
                sp += 2;
            }
        }
        false
    }

    pub fn closest(&self, o: V3, d: V3, tmax: f32) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = [1.0 / d[0], 1.0 / d[1], 1.0 / d[2]];
        let mut best: Option<Hit> = None;
        let mut tmax = tmax;
        let mut stack: [u32; 64] = [0; 64];
        let mut sp = 0usize;
        stack[sp] = 0;
        sp += 1;
        while sp > 0 {
            sp -= 1;
            let n = &self.nodes[stack[sp] as usize];
            if !Self::ray_box(o, inv, n.bmin, n.bmax, tmax) {
                continue;
            }
            if n.count > 0 {
                for i in n.first..n.first + n.count {
                    if let Some(tt) = Self::ray_tri(o, d, &self.tris[i as usize], tmax) {
                        tmax = tt;
                        best = Some(Hit { t: tt, tri: i });
                    }
                }
            } else {
                stack[sp] = n.first;
                stack[sp + 1] = n.first + 1;
                sp += 2;
            }
        }
        best
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}
