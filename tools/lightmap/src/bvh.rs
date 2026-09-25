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
    /// The triangle's index in its model (the hit → uv → texel lookup of the bounce passes).
    pub tri: u32,
    /// The cut-out mask (index into `Scene`'s mask list as `world_tris` orders it; u16::MAX = opaque)
    /// and the TexCoord0 corners it is sampled with.
    pub alpha: u16,
    pub uv0: [[f32; 2]; 3],
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
    /// `world_tris` index → index into `tris` (the build reorders).
    pub perm: Vec<u32>,
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

/// THE PARALLEL BUILD without re-layouts: the top of the tree (depth < TOP_DEPTH) is a complete binary
/// tree laid out breadth-first (node i's children at 2i + 1, 2i + 2 — siblings adjacent, as the traversal
/// wants), its splits partitioning `order` level by level (every node of a level in parallel); each top
/// leaf's range then builds its subtree with `build_rec_iter` (all subtrees in parallel), and the subtrees
/// are appended after the top tree with one index fix-up each — no walk of the whole tree per level.
/// Deterministic: the splits do not depend on thread timing, and the triangle order equals the
/// recursive build's (the same binned-SAH split sequence).
const TOP_DEPTH: usize = 7;

fn build_top_down(order: &mut [u32], bounds: &[(V3, V3)], cents: &[V3]) -> Vec<Node> {
    let n_top = (1usize << (TOP_DEPTH + 1)) - 1;
    // a top node: its range in `order` (lo, hi) and whether it is a leaf of the top tree (its subtree
    // is built below) or was split
    #[derive(Clone, Copy)]
    enum Top {
        Unused,
        Range(usize, usize),
    }
    let mut top = vec![Top::Unused; n_top];
    top[0] = Top::Range(0, order.len());
    let mut nodes = vec![Node { bmin: [0.0; 3], bmax: [0.0; 3], first: 0, count: 0 }; n_top];
    // per level: split every live node of the level in parallel; `order` is partitioned in disjoint ranges
    let order_ptr = order.as_mut_ptr() as usize;
    let order_len = order.len();
    let t_top = std::time::Instant::now();
    for depth in 0..TOP_DEPTH {
        let (a, b) = ((1usize << depth) - 1, (1usize << (depth + 1)) - 1);
        let live: Vec<usize> = (a..b).filter(|&i| matches!(top[i], Top::Range(..))).collect();
        if live.is_empty() {
            break;
        }
        // (node, bmin, bmax, Option<(mid, axis-split done)>): None = the node stays a leaf of the top tree
        let results: Vec<(usize, V3, V3, Option<usize>)> = crate::pool::pool().map(live.len(), |k| {
            let i = live[k];
            let Top::Range(lo, hi) = top[i] else { unreachable!() };
            // SAFETY: the live nodes of one level own disjoint ranges of `order`
            let slice: &mut [u32] = unsafe { std::slice::from_raw_parts_mut((order_ptr as *mut u32).add(lo), hi - lo) };
            let _ = order_len;
            let (bmin, bmax, cmin, cmax) = range_bounds(slice, bounds, cents);
            let count = hi - lo;
            if count <= 4 {
                return (i, bmin, bmax, None);
            }
            let Some((axis, split)) = choose_split(slice, bounds, cents, cmin, cmax) else { return (i, bmin, bmax, None) };
            let mid = partition(slice, cents, axis, split);
            (i, bmin, bmax, Some(lo + mid))
        });
        for (i, bmin, bmax, mid) in results {
            nodes[i].bmin = bmin;
            nodes[i].bmax = bmax;
            let Top::Range(lo, hi) = top[i] else { unreachable!() };
            match mid {
                Some(m) => {
                    nodes[i].first = (2 * i + 1) as u32;
                    nodes[i].count = 0;
                    top[2 * i + 1] = Top::Range(lo, m);
                    top[2 * i + 2] = Top::Range(m, hi);
                    top[i] = Top::Unused; // split: no longer a top leaf
                    // (keep the range for bookkeeping: not needed further)
                }
                None => {
                    // a top-tree leaf: a real leaf when small, else its subtree builds below
                    nodes[i].first = lo as u32;
                    nodes[i].count = (hi - lo) as u32;
                }
            }
        }
    }
    // the ranges left at the bottom of the top tree: the small ones (≤ 4) are leaves (their bounds set
    // here — the level loop never visited the last level), the big ones build subtrees below
    for i in 0..n_top {
        if let Top::Range(lo, hi) = top[i] {
            if hi - lo <= 4 && nodes[i].count == 0 {
                let (bmin, bmax, _, _) = range_bounds(&order[lo..hi], bounds, cents);
                nodes[i] = Node { bmin, bmax, first: lo as u32, count: (hi - lo) as u32 };
            }
        }
    }
    let sub_roots: Vec<(usize, usize, usize)> = (0..n_top).filter_map(|i| match top[i] { Top::Range(lo, hi) if hi - lo > 4 => Some((i, lo, hi)), _ => None }).collect();
    let t_sub = std::time::Instant::now();
    let subtrees: Vec<Vec<Node>> = crate::pool::pool().map(sub_roots.len(), |k| {
        let (_, lo, hi) = sub_roots[k];
        // SAFETY: disjoint ranges
        let slice: &mut [u32] = unsafe { std::slice::from_raw_parts_mut((order_ptr as *mut u32).add(lo), hi - lo) };
        build_rec_iter(slice, bounds, cents, lo)
    });
    // append: each subtree's root replaces its top slot, its other nodes follow at `base`
    let t_app = std::time::Instant::now();
    if std::env::var_os("LMTOOL_PROFILE").is_some() { eprintln!("bvh build: top {:.2}s, {} subtrees {:.2}s (largest {} triangles)", (t_sub - t_top).as_secs_f32(), sub_roots.len(), (t_app - t_sub).as_secs_f32(), sub_roots.iter().map(|(_, lo, hi)| hi - lo).max().unwrap_or(0)); }
    let total: usize = subtrees.iter().map(|v| v.len().saturating_sub(1)).sum();
    nodes.reserve(total);
    for (k, sub) in subtrees.into_iter().enumerate() {
        let (slot, _, _) = sub_roots[k];
        let base = nodes.len();
        let fix = |nd: &Node| -> Node {
            if nd.count == 0 { Node { bmin: nd.bmin, bmax: nd.bmax, first: nd.first - 1 + base as u32, count: 0 } } else { *nd }
        };
        nodes[slot] = fix(&sub[0]);
        for nd in &sub[1..] {
            nodes.push(fix(nd));
        }
    }
    nodes
}

#[cfg(test)]
mod build_tests {
    use super::*;

    fn tri(p: V3, e1: V3, e2: V3) -> WTri {
        WTri { p0: p, e1, e2, inst: 0, tri: 0, alpha: u16::MAX, uv0: [[0.0; 2]; 3] }
    }

    #[test]
    fn parallel_build_matches_the_recursive_one() {
        // a few thousand random small triangles; every query must return the same hit either way
        let mut seed = 12345u64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % 10_000) as f32 / 10_000.0 };
        let tris: Vec<WTri> = (0..5000).map(|_| { let p = [rnd() * 100.0, rnd() * 100.0, rnd() * 100.0]; tri(p, [rnd() * 2.0, rnd() * 2.0, 0.0], [0.0, rnd() * 2.0, rnd() * 2.0]) }).collect();
        let bounds: Vec<(V3, V3)> = tris.iter().map(tri_bounds).collect();
        let cents: Vec<V3> = tris.iter().map(centroid).collect();
        let mut o1: Vec<u32> = (0..tris.len() as u32).collect();
        let mut o2 = o1.clone();
        let n_old = build_rec(&mut o1, &bounds, &cents, 0, 0);
        let n_new = build_top_down(&mut o2, &bounds, &cents);
        assert_eq!(o1, o2, "the triangle order");
        let t1: Vec<WTri> = o1.iter().map(|&i| tris[i as usize]).collect();
        let b_old = Bvh { tris: t1.clone(), nodes: n_old, perm: vec![0; tris.len()] };
        let b_new = Bvh { tris: t1, nodes: n_new, perm: vec![0; tris.len()] };
        for k in 0..2000 {
            let o = [rnd() * 100.0, rnd() * 100.0, rnd() * 100.0];
            let d = crate::geometry::norm([rnd() - 0.5, rnd() - 0.5, rnd() - 0.5]);
            let a = b_old.occluded(o, d, 50.0, u32::MAX, 0.0);
            let b = b_new.occluded(o, d, 50.0, u32::MAX, 0.0);
            assert_eq!(a, b, "query {k}");
        }
    }
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
    pub fn build(tris: Vec<WTri>) -> Bvh {
        let n = tris.len();
        // the per-triangle bounds and centroids, the reorder and the permutation in parallel chunks (27 M
        // triangles on the giants: each serial pass was half a second)
        let chunk = (n / (crate::pool::pool().threads * 4).max(1)).max(65_536);
        let n_chunks = (n + chunk - 1) / chunk;
        let mut bounds: Vec<(V3, V3)> = vec![([0.0; 3], [0.0; 3]); n];
        let mut cents: Vec<V3> = vec![[0.0; 3]; n];
        {
            let (bp, cp) = (bounds.as_mut_ptr() as usize, cents.as_mut_ptr() as usize);
            let tris = &tris;
            crate::pool::pool().run(n_chunks, |ci| {
                for i in ci * chunk..((ci + 1) * chunk).min(n) {
                    // SAFETY: disjoint index ranges per task
                    unsafe {
                        *(bp as *mut (V3, V3)).add(i) = tri_bounds(&tris[i]);
                        *(cp as *mut V3).add(i) = centroid(&tris[i]);
                    }
                }
            });
        }
        let mut order: Vec<u32> = (0..n as u32).collect();
        let nodes = build_top_down(&mut order, &bounds, &cents);
        let mut reordered: Vec<WTri> = Vec::with_capacity(n);
        // original index → position in `tris` (the build reorders the triangles; a caller that numbers
        // triangles in `world_tris` order — the peel's own-triangle exclusion — maps through this)
        let mut perm = vec![0u32; n];
        {
            // SAFETY: `reordered` is filled at every index exactly once before set_len; `perm` likewise
            unsafe { reordered.set_len(n); }
            let (rp, pp) = (reordered.as_mut_ptr() as usize, perm.as_mut_ptr() as usize);
            let (tris, order) = (&tris, &order);
            crate::pool::pool().run(n_chunks, |ci| {
                for new_i in ci * chunk..((ci + 1) * chunk).min(n) {
                    let old_i = order[new_i] as usize;
                    unsafe {
                        std::ptr::write((rp as *mut WTri).add(new_i), tris[old_i]);
                        *(pp as *mut u32).add(old_i) = new_i as u32;
                    }
                }
            });
        }
        drop(tris);
        Bvh { tris: reordered, nodes, perm }
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
