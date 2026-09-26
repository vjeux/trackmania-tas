//! THE INSTANCE HIERARCHY (perf engineer 5, PERF 5.2): the peel frames' triangle lists from a tree per MODEL
//! instead of a projection per triangle.
//!
//! Today every peel frame projects every culled triangle (the BVH's leaf ranges meeting the frame: 87 M of the
//! giant's 27 M per direction across its ten frames, a 72-byte record each — a 6 GB stream, 12 % of a
//! direction) to find the rows it touches. The scene is 10 977 instances of 627 models (the 9 216 zone tiles a
//! few meshes), so the geometry is described far more compactly by the models: ONE tree over each model's
//! triangles in MODEL space (built once per bake, shared by its instances), and per (frame, instance) an
//! affine map model → pixel (the instance transform composed with the frame's projection) under which a node's
//! box projects to a pixel rectangle in three multiply-adds per axis. The traversal descends while a node's
//! rectangle is larger than the consumer's grain (a raster band, a 64×64 tile) and emits JOBS — contiguous
//! ranges of the instance's triangles (laid out in the tree's leaf order, world-space copies of the BVH's own
//! records) with a conservative pixel rectangle — for the binning to place into its bands or tiles.
//!
//! EXACTNESS: the binning only decides which bands walk which triangles; the raster's per-triangle work
//! (projection, edge tests, the fragments) is unchanged and still exact per visited triangle. A job's rectangle
//! is CONSERVATIVE — the f64 image of the node's model-space box under the affine map, widened by `MARGIN_PX`
//! against the f32 rounding of the per-vertex path (world position through `apply`, then `PeelFrame::project`;
//! worst case ≈ 0.02 px on a 4096² frame, see `check_frame`) — so every band a triangle has a covered centre in
//! receives a job holding it; a band that receives a triangle without a covered centre visits nothing. The
//! triangle records are copies of `bvh.tris` (the same f32 positions), and each carries its BVH index, so the
//! fragments' `tri` keys — the (z, tri) order of the count and the sparse CSR — are the ones of today.
//! `LMTOOL_INSTHIER_CHECK=1` verifies, per frame, that every triangle's exact projected bbox lies inside the
//! rectangle of the job holding it (and reports the largest excursion of the bound).

use crate::bvh::{Bvh, WTri};
use crate::geometry::{Scene, V3, DECOR_INST};
use crate::peel::PeelFrame;

/// The widening of a job's rectangle, in pixels, over the f64 image of the node box (the f32 rounding of the
/// vertex path is two orders of magnitude below it on a 4096² frame of a 10 km scene).
pub const MARGIN_PX: f64 = 0.25;

#[derive(Clone, Copy, Debug)]
struct TNode {
    lo: V3,
    hi: V3,
    /// Leaf: the first slot of its range in the tree's leaf order; inner: the left child (right = left + 1).
    first: u32,
    /// Leaf: the triangle count (> 0); inner: 0.
    count: u32,
}

/// A tree over one model's triangles in model space; `order` = the model's triangle indices in leaf order.
pub struct ModelTree {
    nodes: Vec<TNode>,
    pub order: Vec<u32>,
}

/// A contiguous range of `InstHier::tris` whose triangles' projected bboxes lie inside `[x0, x1] × [y0, y1]`
/// (pixel-centre indices, inclusive, clipped to the frame's clip rectangle) and whose depth range meets the
/// frame's; `inst` = the instance (DECOR_INST for the decoration).
#[derive(Clone, Copy, Debug)]
pub struct GeomJob {
    pub first: u32,
    pub count: u32,
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub inst: u32,
}

pub struct InstHier {
    pub trees: Vec<ModelTree>,
    /// Per instance: its first triangle in `tris` (u32::MAX = the instance has no triangles in the BVH).
    pub inst_base: Vec<u32>,
    /// Every instance's triangles in its model tree's leaf order (world-space copies of the BVH's records), then
    /// the decoration's.
    pub tris: Vec<WTri>,
    /// Per entry of `tris`: its index in `bvh.tris` (the fragments' `tri` key).
    pub bvh_id: Vec<u32>,
    /// The decoration's range in `tris` (one job per triangle: a few huge triangles).
    pub decor_first: u32,
    pub decor_count: u32,
}

const LEAF: usize = 8;

fn tri_bounds(p: [V3; 3]) -> (V3, V3) {
    let mut lo = p[0];
    let mut hi = p[0];
    for q in [p[1], p[2]] {
        for k in 0..3 {
            lo[k] = lo[k].min(q[k]);
            hi[k] = hi[k].max(q[k]);
        }
    }
    (lo, hi)
}

impl ModelTree {
    /// A median-split tree (the longest centroid axis, the range's median) with leaves of ≤ `LEAF` triangles.
    pub fn build(tris: &[crate::geometry::Tri]) -> ModelTree {
        let n = tris.len();
        let bounds: Vec<(V3, V3)> = tris.iter().map(|t| tri_bounds(t.p)).collect();
        let cents: Vec<V3> = bounds.iter().map(|(lo, hi)| [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5]).collect();
        let mut order: Vec<u32> = (0..n as u32).collect();
        let mut nodes: Vec<TNode> = Vec::with_capacity(2 * n / LEAF.max(1) + 2);
        if n == 0 {
            return ModelTree { nodes, order };
        }
        nodes.push(TNode { lo: [0.0; 3], hi: [0.0; 3], first: 0, count: 0 });
        let mut stack: Vec<(usize, usize, usize)> = vec![(0, 0, n)];
        while let Some((node, lo_i, hi_i)) = stack.pop() {
            let mut lo = [f32::MAX; 3];
            let mut hi = [f32::MIN; 3];
            let mut clo = [f32::MAX; 3];
            let mut chi = [f32::MIN; 3];
            for &i in &order[lo_i..hi_i] {
                let (a, b) = bounds[i as usize];
                let c = cents[i as usize];
                for k in 0..3 {
                    lo[k] = lo[k].min(a[k]);
                    hi[k] = hi[k].max(b[k]);
                    clo[k] = clo[k].min(c[k]);
                    chi[k] = chi[k].max(c[k]);
                }
            }
            nodes[node].lo = lo;
            nodes[node].hi = hi;
            let count = hi_i - lo_i;
            let axis = (0..3).max_by(|&a, &b| (chi[a] - clo[a]).partial_cmp(&(chi[b] - clo[b])).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(0);
            if count <= LEAF || chi[axis] - clo[axis] <= 0.0 {
                nodes[node].first = lo_i as u32;
                nodes[node].count = count as u32;
                continue;
            }
            let mid = lo_i + count / 2;
            order[lo_i..hi_i].select_nth_unstable_by(count / 2, |&a, &b| cents[a as usize][axis].partial_cmp(&cents[b as usize][axis]).unwrap_or(std::cmp::Ordering::Equal));
            let left = nodes.len();
            nodes.push(TNode { lo: [0.0; 3], hi: [0.0; 3], first: 0, count: 0 });
            nodes.push(TNode { lo: [0.0; 3], hi: [0.0; 3], first: 0, count: 0 });
            nodes[node].first = left as u32;
            nodes[node].count = 0;
            stack.push((left, lo_i, mid));
            stack.push((left + 1, mid, hi_i));
        }
        ModelTree { nodes, order }
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

/// The affine map model → (x_px, y_px, z_port) of one (instance, frame): row k of `a` maps a model point to
/// axis k as `a[k]·v + b[k]` (f64; the bound of a box is `b + Σ min/max(a_i·lo_i, a_i·hi_i)`).
struct Affine {
    a: [[f64; 3]; 3],
    b: [f64; 3],
}

impl Affine {
    fn new(xf: &mapgeom::geom::Xform, frame: &PeelFrame) -> Affine {
        let m: Vec<f64> = xf.iter().map(|v| *v as f64).collect();
        let axes: [(V3, f64, f64); 3] = [(frame.r, frame.s0 as f64, frame.scale as f64), (frame.u, frame.t0 as f64, frame.scale_y as f64), (frame.d, 0.0, -1.0)];
        let mut a = [[0f64; 3]; 3];
        let mut b = [0f64; 3];
        for (k, (axis, off, scale)) in axes.iter().enumerate() {
            let r = [axis[0] as f64, axis[1] as f64, axis[2] as f64];
            // column i of the 3×3 dotted with the axis
            for i in 0..3 {
                a[k][i] = (m[3 * i] * r[0] + m[3 * i + 1] * r[1] + m[3 * i + 2] * r[2]) * scale;
            }
            b[k] = ((m[9] * r[0] + m[10] * r[1] + m[11] * r[2]) - off) * scale;
        }
        Affine { a, b }
    }
    /// The image of a box: (min, max) per axis.
    #[inline]
    fn bound(&self, lo: V3, hi: V3) -> ([f64; 3], [f64; 3]) {
        let mut mn = self.b;
        let mut mx = self.b;
        for k in 0..3 {
            for i in 0..3 {
                let p = self.a[k][i] * lo[i] as f64;
                let q = self.a[k][i] * hi[i] as f64;
                mn[k] += p.min(q);
                mx[k] += p.max(q);
            }
        }
        (mn, mx)
    }
}

/// The candidate pixel-centre range of a projected interval `[mn, mx]` (as `raster::bounds` derives it from the
/// vertices: centres x + 0.5 in [mn, mx]), widened by the margin, clipped to `[c0, c1]`; None when empty.
#[inline]
fn centre_range(mn: f64, mx: f64, c0: i32, c1: i32) -> Option<(i32, i32)> {
    let lo = ((mn - MARGIN_PX - 0.5).ceil()).max(c0 as f64);
    let hi = ((mx + MARGIN_PX - 0.5).floor()).min(c1 as f64);
    if !(lo.is_finite() && hi.is_finite()) || lo > hi {
        return None;
    }
    Some((lo as i32, hi as i32))
}

impl InstHier {
    /// The trees (one per model that has instances), the instance-major triangle array in tree order, the BVH
    /// index of every entry. `tri_base[ii]` = instance ii's first triangle in `world_tris` order (the peel's
    /// `tri_base`); an instance whose model was left out of the BVH (flat tiles with LMTOOL_TILES_CAST=0) has
    /// `tri_base[ii + 1] == tri_base[ii]` and gets no entries.
    pub fn build(scene: &Scene, bvh: &Bvh, tri_base: &[u32]) -> InstHier {
        let t0 = std::time::Instant::now();
        let n_models = scene.models.len();
        // which models are instanced (the others get an empty tree)
        let mut used = vec![false; n_models];
        for inst in &scene.instances { used[inst.model] = true; }
        let trees: Vec<ModelTree> = crate::pool::pool().map(n_models, |mi| if used[mi] { ModelTree::build(&scene.models[mi].tris) } else { ModelTree { nodes: Vec::new(), order: Vec::new() } });
        let n_inst = scene.instances.len();
        let mut inst_base = vec![u32::MAX; n_inst];
        let mut acc = 0u32;
        for ii in 0..n_inst {
            let next = if ii + 1 < n_inst { tri_base[ii + 1] } else { tri_base[ii] + scene.models[scene.instances[ii].model].tris.len() as u32 };
            let n = next - tri_base[ii];
            if n > 0 {
                inst_base[ii] = acc;
                acc += n;
            }
        }
        let n_item_tris = acc as usize;
        let decor_count = scene.decor.len() as u32;
        let total = n_item_tris + decor_count as usize;
        let mut tris: Vec<WTri> = Vec::with_capacity(total);
        let mut bvh_id: Vec<u32> = Vec::with_capacity(total);
        // SAFETY: every slot is written exactly once below
        unsafe { tris.set_len(total); bvh_id.set_len(total); }
        {
            let (tp, bp) = (tris.as_mut_ptr() as usize, bvh_id.as_mut_ptr() as usize);
            let (trees, inst_base) = (&trees, &inst_base);
            crate::pool::pool().run(n_inst, |ii| {
                let base = inst_base[ii];
                if base == u32::MAX { return; }
                let inst = &scene.instances[ii];
                let tree = &trees[inst.model];
                for (k, &mt) in tree.order.iter().enumerate() {
                    let world_i = tri_base[ii] + mt;
                    let bi = bvh.perm[world_i as usize];
                    // SAFETY: disjoint ranges per instance
                    unsafe {
                        std::ptr::write((tp as *mut WTri).add(base as usize + k), bvh.tris[bi as usize]);
                        *(bp as *mut u32).add(base as usize + k) = bi;
                    }
                }
            });
        }
        // the decoration: world_tris lists it after every instance
        let decor_world_base = tri_base.last().map(|b| *b + scene.models[scene.instances[n_inst - 1].model].tris.len() as u32).unwrap_or(0);
        for di in 0..decor_count {
            let bi = bvh.perm[(decor_world_base + di) as usize];
            tris[n_item_tris + di as usize] = bvh.tris[bi as usize];
            bvh_id[n_item_tris + di as usize] = bi;
        }
        if std::env::var_os("LMTOOL_PROFILE").is_some() {
            let nodes: usize = trees.iter().map(|t| t.nodes.len()).sum();
            eprintln!("insthier: {} models ({} instanced) → {} tree nodes, {} instance triangles + {} decoration, {:.2}s", n_models, used.iter().filter(|u| **u).count(), nodes, n_item_tris, decor_count, t0.elapsed().as_secs_f32());
        }
        InstHier { trees, inst_base, tris, bvh_id, decor_first: n_item_tris as u32, decor_count }
    }

    /// The jobs of one frame: per instance the tree descended until a node's pixel rectangle is at most
    /// `grain` pixels in both axes (or a leaf), nodes outside the clip rectangle or the depth range
    /// `[zmin, zmax)` skipped. Jobs come out in instance order, then tree (leaf) order — a fixed order per frame.
    pub fn frame_jobs(&self, scene: &Scene, frame: &PeelFrame, zmin: f32, zmax: f32, clip: (i32, i32, i32, i32), grain: i32) -> Vec<GeomJob> {
        let n_inst = scene.instances.len();
        let per: Vec<Vec<GeomJob>> = crate::pool::pool().map(n_inst, |ii| {
            let mut out: Vec<GeomJob> = Vec::new();
            let base = self.inst_base[ii];
            if base == u32::MAX { return out; }
            let inst = &scene.instances[ii];
            let tree = &self.trees[inst.model];
            if tree.nodes.is_empty() { return out; }
            let af = Affine::new(&inst.xf, frame);
            let (zmin, zmax) = (zmin as f64, zmax as f64);
            let mut stack: Vec<u32> = vec![0];
            while let Some(ni) = stack.pop() {
                let n = &tree.nodes[ni as usize];
                let (mn, mx) = af.bound(n.lo, n.hi);
                // the depth range as `rows_of` tests it (z.min ≥ zmax or z.max < zmin → out), with the margin in metres
                // scaled from the pixel margin is not meaningful — use a relative slack on the depth instead
                let zs = (mx[2].abs().max(mn[2].abs()) * 1e-5).max(1e-3);
                if mn[2] - zs >= zmax || mx[2] + zs < zmin {
                    continue;
                }
                let Some((x0, x1)) = centre_range(mn[0], mx[0], clip.0, clip.2) else { continue };
                let Some((y0, y1)) = centre_range(mn[1], mx[1], clip.1, clip.3) else { continue };
                if n.count > 0 || (x1 - x0 + 1 <= grain && y1 - y0 + 1 <= grain) {
                    // a leaf, or small enough: one job over the node's whole range
                    let (first, count) = range_of(&tree.nodes, ni as usize);
                    out.push(GeomJob { first: base + first, count, x0, y0, x1, y1, inst: ii as u32 });
                } else {
                    stack.push(n.first + 1);
                    stack.push(n.first);
                }
            }
            out
        });
        let mut jobs: Vec<GeomJob> = Vec::with_capacity(per.iter().map(|v| v.len()).sum::<usize>() + self.decor_count as usize);
        for v in per { jobs.extend(v); }
        // the decoration: one job per triangle from its own exact projection (a few hundred huge triangles)
        for k in 0..self.decor_count {
            let i = (self.decor_first + k) as usize;
            let t = &self.tris[i];
            let p0 = t.p0;
            let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
            let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
            let (x0, y0, z0) = frame.project(p0);
            let (x1, y1, z1) = frame.project(p1);
            let (x2, y2, z2) = frame.project(p2);
            if z0.min(z1).min(z2) >= zmax || z0.max(z1).max(z2) < zmin { continue; }
            let Some((cx0, cx1)) = centre_range(x0.min(x1).min(x2) as f64, x0.max(x1).max(x2) as f64, clip.0, clip.2) else { continue };
            let Some((cy0, cy1)) = centre_range(y0.min(y1).min(y2) as f64, y0.max(y1).max(y2) as f64, clip.1, clip.3) else { continue };
            jobs.push(GeomJob { first: i as u32, count: 1, x0: cx0, y0: cy0, x1: cx1, y1: cy1, inst: DECOR_INST });
        }
        jobs
    }

    /// THE CHECK (LMTOOL_INSTHIER_CHECK=1): every triangle of every job projects inside the job's rectangle
    /// (its exact candidate-centre bbox from `PeelFrame::project`, as `raster::bounds` derives it), and every
    /// triangle the frame's depth range holds with a candidate centre inside the clip is in some job. Returns
    /// (jobs, triangles in jobs, triangles the frame holds, violations, the largest excursion of a projected
    /// vertex beyond a job's f64 bound in pixels — the margin's safety factor is MARGIN_PX / that).
    pub fn check_frame(&self, scene: &Scene, frame: &PeelFrame, zmin: f32, zmax: f32, clip: (i32, i32, i32, i32), jobs: &[GeomJob]) -> (usize, usize, usize, usize, f64) {
        let mut in_jobs = vec![false; self.tris.len()];
        let mut violations = 0usize;
        let mut n_job_tris = 0usize;
        let mut worst = 0f64;
        for j in jobs {
            let inst_af = if j.inst != DECOR_INST { Some(Affine::new(&scene.instances[j.inst as usize].xf, frame)) } else { None };
            for i in j.first..j.first + j.count {
                n_job_tris += 1;
                in_jobs[i as usize] = true;
                let t = &self.tris[i as usize];
                let p0 = t.p0;
                let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
                let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
                let pr = [frame.project(p0), frame.project(p1), frame.project(p2)];
                let (minx, maxx) = (pr.iter().map(|q| q.0).fold(f32::MAX, f32::min), pr.iter().map(|q| q.0).fold(f32::MIN, f32::max));
                let (miny, maxy) = (pr.iter().map(|q| q.1).fold(f32::MAX, f32::min), pr.iter().map(|q| q.1).fold(f32::MIN, f32::max));
                let rx0 = ((minx - 0.5).ceil() as i64).max(clip.0 as i64);
                let rx1 = ((maxx - 0.5).floor() as i64).min(clip.2 as i64);
                let ry0 = ((miny - 0.5).ceil() as i64).max(clip.1 as i64);
                let ry1 = ((maxy - 0.5).floor() as i64).min(clip.3 as i64);
                if rx0 <= rx1 && ry0 <= ry1 && (rx0 < j.x0 as i64 || rx1 > j.x1 as i64 || ry0 < j.y0 as i64 || ry1 > j.y1 as i64) {
                    violations += 1;
                    if violations <= 5 { eprintln!("insthier check: triangle {i} (bvh {}) bbox x {rx0}..{rx1} y {ry0}..{ry1} outside job x {}..{} y {}..{} (inst {})", self.bvh_id[i as usize], j.x0, j.x1, j.y0, j.y1, j.inst); }
                }
                // the excursion of the f32 vertex path beyond the f64 node bound (the margin's safety factor)
                if let Some(af) = &inst_af {
                    let inst = &scene.instances[j.inst as usize];
                    let tree = &self.trees[inst.model];
                    let mt = tree.order[(i - self.inst_base[j.inst as usize]) as usize] as usize;
                    let mtri = &scene.models[inst.model].tris[mt];
                    for (k, q) in pr.iter().enumerate() {
                        let (mn, mx) = af.bound(mtri.p[k], mtri.p[k]);
                        let ex = (mn[0] - q.0 as f64).max(q.0 as f64 - mx[0]).max(mn[1] - q.1 as f64).max(q.1 as f64 - mx[1]);
                        if ex > worst { worst = ex; }
                    }
                }
            }
        }
        // every triangle the frame holds is in a job
        let mut held = 0usize;
        for (i, t) in self.tris.iter().enumerate() {
            let p0 = t.p0;
            let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
            let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
            let (x0, y0, z0) = frame.project(p0);
            let (x1, y1, z1) = frame.project(p1);
            let (x2, y2, z2) = frame.project(p2);
            if z0.min(z1).min(z2) >= zmax || z0.max(z1).max(z2) < zmin { continue; }
            let (minx, maxx, miny, maxy) = (x0.min(x1).min(x2), x0.max(x1).max(x2), y0.min(y1).min(y2), y0.max(y1).max(y2));
            if !(minx.is_finite() && maxx.is_finite() && miny.is_finite() && maxy.is_finite()) { continue; }
            let rx0 = ((minx - 0.5).ceil() as i64).max(clip.0 as i64);
            let rx1 = ((maxx - 0.5).floor() as i64).min(clip.2 as i64);
            let ry0 = ((miny - 0.5).ceil() as i64).max(clip.1 as i64);
            let ry1 = ((maxy - 0.5).floor() as i64).min(clip.3 as i64);
            if rx0 > rx1 || ry0 > ry1 { continue; }
            held += 1;
            if !in_jobs[i] {
                violations += 1;
                if violations <= 5 { eprintln!("insthier check: triangle {i} (bvh {}, inst {}) held by the frame (x {rx0}..{rx1} y {ry0}..{ry1}) but in no job", self.bvh_id[i], t.inst); }
            }
        }
        (jobs.len(), n_job_tris, held, violations, worst)
    }
}

/// The whole leaf range under a node: its leftmost leaf's first .. its rightmost leaf's end.
fn range_of(nodes: &[TNode], i: usize) -> (u32, u32) {
    let mut lo = i;
    while nodes[lo].count == 0 { lo = nodes[lo].first as usize; }
    let mut hi = i;
    while nodes[hi].count == 0 { hi = nodes[hi].first as usize + 1; }
    let first = nodes[lo].first;
    (first, nodes[hi].first + nodes[hi].count - first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_tree_orders_every_triangle_once_and_its_leaves_tile_the_range() {
        let mut seed = 99u64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % 10_000) as f32 / 10_000.0 };
        let tris: Vec<crate::geometry::Tri> = (0..1000).map(|_| {
            let p = [rnd() * 50.0, rnd() * 50.0, rnd() * 50.0];
            crate::geometry::Tri { p: [p, [p[0] + rnd(), p[1], p[2] + rnd()], [p[0], p[1] + rnd(), p[2] + rnd()]], n: [[0.0, 1.0, 0.0]; 3], uv: [[0.0; 2]; 3], uv0: [[0.0; 2]; 3], mat: 0, alpha: u16::MAX, diff: u16::MAX }
        }).collect();
        let t = ModelTree::build(&tris);
        let mut seen = t.order.clone();
        seen.sort();
        assert_eq!(seen, (0..1000u32).collect::<Vec<_>>());
        // every leaf's box holds its triangles; the root's range is everything
        for n in &t.nodes {
            if n.count > 0 {
                for &i in &t.order[n.first as usize..(n.first + n.count) as usize] {
                    let (lo, hi) = tri_bounds(tris[i as usize].p);
                    for k in 0..3 { assert!(lo[k] >= n.lo[k] && hi[k] <= n.hi[k]); }
                }
            }
        }
        assert_eq!(range_of(&t.nodes, 0), (0, 1000));
    }

    #[test]
    fn the_affine_bound_holds_every_corner() {
        let xf: mapgeom::geom::Xform = [0.8, 0.6, 0.0, -0.6, 0.8, 0.0, 0.0, 0.0, 1.0, 100.0, 5.0, -30.0];
        let frame = PeelFrame::new([0.3, -0.9, 0.2], [0.0, 0.0, 0.0], [500.0, 100.0, 500.0], 1024);
        let af = Affine::new(&xf, &frame);
        let (lo, hi) = ([-3.0f32, 0.0, -2.0], [4.0f32, 7.0, 1.5]);
        let (mn, mx) = af.bound(lo, hi);
        for c in 0..8 {
            let v = [if c & 1 == 0 { lo[0] } else { hi[0] }, if c & 2 == 0 { lo[1] } else { hi[1] }, if c & 4 == 0 { lo[2] } else { hi[2] }];
            let (x, y, z) = frame.project(mapgeom::geom::apply(&xf, v));
            assert!(x as f64 >= mn[0] - 1e-3 && x as f64 <= mx[0] + 1e-3, "x {x} in [{}, {}]", mn[0], mx[0]);
            assert!(y as f64 >= mn[1] - 1e-3 && y as f64 <= mx[1] + 1e-3);
            assert!(z as f64 >= mn[2] - 1e-3 && z as f64 <= mx[2] + 1e-3);
        }
    }
}
