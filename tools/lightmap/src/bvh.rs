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


const PAR_DEPTH: usize = 5;

/// Bounds of a range of `order`, and the centroid bounds.
fn range_bounds(order: &[u32], bounds: &[(V3, V3)], cents: &[V3]) -> (V3, V3, V3, V3) {
    let mut bmin = [f32::MAX; 3];
    let mut bmax = [f32::MIN; 3];
    let mut cmin = [f32::MAX; 3];
    let mut cmax = [f32::MIN; 3];
    for &i in order {
        let (a, b) = bounds[i as usize];
        let c = cents[i as usize];
        for k in 0..3 {
            bmin[k] = bmin[k].min(a[k]);
            bmax[k] = bmax[k].max(b[k]);
            cmin[k] = cmin[k].min(c[k]);
            cmax[k] = cmax[k].max(c[k]);
        }
    }
    (bmin, bmax, cmin, cmax)
}

/// The binned-SAH split of a range: (axis, split value), or None for a leaf.
fn choose_split(order: &[u32], bounds: &[(V3, V3)], cents: &[V3], cmin: V3, cmax: V3) -> Option<(usize, f32)> {
    const BINS: usize = 12;
    let mut best: Option<(f32, usize, f32)> = None;
    for axis in 0..3 {
        let ext = cmax[axis] - cmin[axis];
        if ext <= 1e-6 {
            continue;
        }
        let mut bin_count = [0usize; BINS];
        let mut bin_lo = [[f32::MAX; 3]; BINS];
        let mut bin_hi = [[f32::MIN; 3]; BINS];
        for &i in order {
            let c = cents[i as usize][axis];
            let b = ((((c - cmin[axis]) / ext) * BINS as f32) as usize).min(BINS - 1);
            bin_count[b] += 1;
            let (a, bb) = bounds[i as usize];
            for k in 0..3 {
                bin_lo[b][k] = bin_lo[b][k].min(a[k]);
                bin_hi[b][k] = bin_hi[b][k].max(bb[k]);
            }
        }
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
    best.map(|(_, a, s)| (a, s))
}

/// Partition `order` by `split` on `axis`; returns the split index (never 0 or len).
fn partition(order: &mut [u32], cents: &[V3], axis: usize, split: f32) -> usize {
    let mut i = 0usize;
    let mut j = order.len();
    while i < j {
        if cents[order[i] as usize][axis] < split {
            i += 1;
        } else {
            j -= 1;
            order.swap(i, j);
        }
    }
    if i == 0 || i == order.len() {
        order.len() / 2
    } else {
        i
    }
}

/// Build the subtree over `order` (whose first element is triangle slot
/// `base` of the final leaf order). Node indices in the result are local
/// (root = 0); `first` of a leaf is global.
fn build_rec(order: &mut [u32], bounds: &[(V3, V3)], cents: &[V3], base: usize, depth: usize) -> Vec<Node> {
    let (bmin, bmax, cmin, cmax) = range_bounds(order, bounds, cents);
    let count = order.len();
    let leaf = |first: usize, count: usize| vec![Node { bmin, bmax, first: first as u32, count: count as u32 }];
    if count <= 4 {
        return leaf(base, count);
    }
    let Some((axis, split)) = choose_split(order, bounds, cents, cmin, cmax) else {
        return leaf(base, count);
    };
    let mid = partition(order, cents, axis, split);
    let (left, right) = order.split_at_mut(mid);
    let (ln, rn) = if depth < PAR_DEPTH && count > 20000 {
        std::thread::scope(|sc| {
            let l = sc.spawn(|| build_rec(left, bounds, cents, base, depth + 1));
            let r = build_rec(right, bounds, cents, base + mid, depth + 1);
            (l.join().unwrap(), r)
        })
    } else {
        (build_rec_iter(left, bounds, cents, base), build_rec_iter(right, bounds, cents, base + mid))
    };
    // splice: root, then left at offset 1, then right at offset 1 + ln.len()
    let mut nodes = Vec::with_capacity(1 + ln.len() + rn.len());
    nodes.push(Node { bmin, bmax, first: 1, count: 0 });
    let off_l = 1u32;
    let off_r = 1 + ln.len() as u32;
    for n in ln.iter() {
        nodes.push(Node { bmin: n.bmin, bmax: n.bmax, first: if n.count == 0 { n.first + off_l } else { n.first }, count: n.count });
    }
    for n in rn.iter() {
        nodes.push(Node { bmin: n.bmin, bmax: n.bmax, first: if n.count == 0 { n.first + off_r } else { n.first }, count: n.count });
    }
    // the root's children: left root at 1, right root at off_r — the traversal expects right = left + 1,
    // so lay the two roots out adjacently: swap the left subtree's root to index 1 and the right root to 2
    // by relocating: simplest is to rebuild with an explicit child pair.
    relayout(nodes)
}

/// The traversal wants `right = left + 1`; a spliced subtree has them apart.
/// Re-lay the whole (local) subtree in depth-first order with sibling pairs adjacent.
fn relayout(nodes: Vec<Node>) -> Vec<Node> {
    fn children(nodes: &[Node], i: usize) -> Option<(usize, usize)> {
        let n = &nodes[i];
        if n.count > 0 {
            return None;
        }
        Some((n.first as usize, n.first as usize + 1))
    }
    // the spliced layout encodes children as (first, first + 1) for the SUBTREE roots too, except our
    // fresh root, whose right child sits at off_r: handle the root specially
    let mut out: Vec<Node> = Vec::with_capacity(nodes.len());
    let root = nodes[0];
    let (l0, r0) = (1usize, {
        // find the right root: the first index whose subtree is not reachable from 1 — recorded as off_r
        // (the left subtree occupies indices 1..off_r)
        let mut size_l = 1usize;
        // compute the left subtree's node count by walking it
        let mut stack = vec![1usize];
        let mut visited = 0usize;
        while let Some(i) = stack.pop() {
            visited += 1;
            if let Some((a, b)) = children(&nodes, i) {
                stack.push(a);
                stack.push(b);
            }
        }
        size_l = size_l.max(visited);
        1 + size_l
    });
    out.push(Node { bmin: root.bmin, bmax: root.bmax, first: 1, count: 0 });
    // copy the two subtrees in order with a stack of (src index, dst slot); siblings adjacent by construction
    out.push(nodes[l0]);
    out.push(nodes[r0]);
    let mut work: Vec<(usize, usize)> = vec![(l0, 1), (r0, 2)];
    while let Some((src, dst)) = work.pop() {
        if let Some((a, b)) = children(&nodes, src) {
            let slot = out.len();
            out.push(nodes[a]);
            out.push(nodes[b]);
            out[dst].first = slot as u32;
            work.push((a, slot));
            work.push((b, slot + 1));
        }
    }
    out
}

/// Sequential iterative build of a subtree (the original loop), local node indices.
fn build_rec_iter(order: &mut [u32], bounds: &[(V3, V3)], cents: &[V3], base: usize) -> Vec<Node> {
    struct Task {
        node: usize,
        lo: usize,
        hi: usize,
    }
    let mut nodes: Vec<Node> = Vec::with_capacity(2 * order.len().max(1));
    nodes.push(Node { bmin: [0.0; 3], bmax: [0.0; 3], first: 0, count: 0 });
    let mut stack = vec![Task { node: 0, lo: 0, hi: order.len() }];
    while let Some(Task { node, lo, hi }) = stack.pop() {
        let (bmin, bmax, cmin, cmax) = range_bounds(&order[lo..hi], bounds, cents);
        nodes[node].bmin = bmin;
        nodes[node].bmax = bmax;
        let count = hi - lo;
        if count <= 4 {
            nodes[node].first = (base + lo) as u32;
            nodes[node].count = count as u32;
            continue;
        }
        let Some((axis, split)) = choose_split(&order[lo..hi], bounds, cents, cmin, cmax) else {
            nodes[node].first = (base + lo) as u32;
            nodes[node].count = count as u32;
            continue;
        };
        let mid = lo + partition(&mut order[lo..hi], cents, axis, split);
        let left = nodes.len();
        nodes.push(Node { bmin: [0.0; 3], bmax: [0.0; 3], first: 0, count: 0 });
        nodes.push(Node { bmin: [0.0; 3], bmax: [0.0; 3], first: 0, count: 0 });
        nodes[node].first = left as u32;
        nodes[node].count = 0;
        stack.push(Task { node: left, lo, hi: mid });
        stack.push(Task { node: left + 1, lo: mid, hi });
    }
    nodes
}

impl Bvh {
    /// Binned-SAH build; the top of the tree is split in parallel (the two
    /// children of every node down to `PAR_DEPTH` build on their own threads),
    /// the subtrees below sequentially. Deterministic: the result does not
    /// depend on thread timing.
    pub fn build(mut tris: Vec<WTri>) -> Bvh {
        let n = tris.len();
        let bounds: Vec<(V3, V3)> = tris.iter().map(tri_bounds).collect();
        let cents: Vec<V3> = tris.iter().map(centroid).collect();
        let mut order: Vec<u32> = (0..n as u32).collect();
        let nodes = build_rec(&mut order, &bounds, &cents, 0, 0);
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

    /// Does any triangle's bounding box overlap the box `[lo, hi]`? (probe-volume occupancy)
    pub fn any_in_box(&self, lo: V3, hi: V3) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        let mut stack: [u32; 64] = [0; 64];
        let mut sp = 1usize;
        while sp > 0 {
            sp -= 1;
            let n = &self.nodes[stack[sp] as usize];
            if (0..3).any(|k| n.bmax[k] < lo[k] || n.bmin[k] > hi[k]) {
                continue;
            }
            if n.count > 0 {
                for i in n.first..n.first + n.count {
                    let (a, b) = tri_bounds(&self.tris[i as usize]);
                    if (0..3).all(|k| b[k] >= lo[k] && a[k] <= hi[k]) {
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

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}
