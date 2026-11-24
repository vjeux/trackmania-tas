//! THE ERROR-BOUNDED DECIMATION (Giant Fall 2026 x4, map 21 Egypt, 2026-11-24).
//!
//! At x4 the LOD guard keeps ~8 MB of Egypt's drivable curves at their nearest level (Nadeo's
//! coarse levels are chords 0.6–1.4 m off the hull there, and its intermediate levels are chords
//! too), and the map lands at 30.9 MB against the 25 MiB upload cap. This makes OUR OWN coarse
//! level of such a visual: interior vertices are removed by half-edge collapse while every
//! removed vertex (and every vertex an earlier collapse removed under the same triangles) stays
//! within `tol` (3-D distance) of the new surface. The guard then reads the result against the
//! hull like any other level, and puts back the nearest level of any part that strays.
//!
//! What is never moved: a vertex on an open edge (the outline stays exact), a vertex whose
//! position carries two different attribute sets (a UV, normal or lightmap-chart seam), a vertex
//! of a non-manifold fan. A collapse never flips a triangle (normals within 60°) nor makes one
//! degenerate. Vertices with identical bytes in every stream are welded first (lossless).
//! The result depends on the geometry and the attribute EQUALITIES only, so a bake copy (the
//! lightmap charts stretched, the same equalities) decimates exactly as its shipped twin.

use std::collections::{BTreeMap, BinaryHeap, HashMap};

use crate::static_item::merged::{sub_visual, visual_triangles};
use crate::static_item::vstream::Elem;
use crate::static_item::visual::CPlugVisualIndexedTriangles;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

/// The distance from `p` to triangle (a, b, c) (Ericson, Real-Time Collision Detection 5.1.5).
pub fn point_triangle(p: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return len(ap);
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return len(bp);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return len(sub(p, [a[0] + v * ab[0], a[1] + v * ab[1], a[2] + v * ab[2]]));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return len(cp);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return len(sub(p, [a[0] + w * ac[0], a[1] + w * ac[1], a[2] + w * ac[2]]));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let bc = sub(c, b);
        return len(sub(p, [b[0] + w * bc[0], b[1] + w * bc[1], b[2] + w * bc[2]]));
    }
    let denom = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denom, vc * denom);
    len(sub(p, [a[0] + ab[0] * v + ac[0] * w, a[1] + ab[1] * v + ac[1] * w, a[2] + ab[2] * v + ac[2] * w]))
}

/// The bytes of every stream element and both tangent arrays at vertex `i` (the weld key).
fn vertex_key(v: &CPlugVisualIndexedTriangles, i: usize) -> Vec<u8> {
    let mut k = Vec::new();
    if let Some(s) = v.stream() {
        for e in &s.elems {
            match e {
                Elem::Float2(a) => a.get(i).iter().for_each(|x| x.iter().for_each(|f| k.extend(f.to_bits().to_le_bytes()))),
                Elem::Float3(a) => a.get(i).iter().for_each(|x| x.iter().for_each(|f| k.extend(f.to_bits().to_le_bytes()))),
                Elem::Float4(a) => a.get(i).iter().for_each(|x| x.iter().for_each(|f| k.extend(f.to_bits().to_le_bytes()))),
                Elem::Word(a) => a.get(i).iter().for_each(|w| k.extend(w.to_le_bytes())),
                Elem::Raw { size, bytes } => k.extend(bytes.chunks(*size.max(&1)).nth(i).unwrap_or(&[])),
            }
        }
    }
    if let (Some((a, b)), Some(m)) = (v.tangents.as_ref(), v.main.as_ref()) {
        let per = (((!(m.flags() >> 17)) & 8) | 4) as usize;
        for t in [a, b] {
            k.extend(t.chunks(per).nth(i).unwrap_or(&[]));
        }
    }
    if let Some(m) = v.main.as_ref() {
        for s in &m.tex_coord_sets {
            if let Some((uv, a, b)) = s.coords.get(i) {
                k.extend(uv[0].to_bits().to_le_bytes());
                k.extend(uv[1].to_bits().to_le_bytes());
                k.extend(a.unwrap_or(i32::MIN).to_le_bytes());
                k.extend(b.unwrap_or(i32::MIN).to_le_bytes());
            }
            if let Some(u) = s.u01.get(i) {
                k.extend(u.to_bits().to_le_bytes());
            }
        }
    }
    k
}

fn tri_normal3(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Option<[f32; 3]> {
    let n = cross(sub(b, a), sub(c, a));
    let l = len(n);
    (l > 1e-9).then(|| [n[0] / l, n[1] / l, n[2] / l])
}

/// A collapse candidate in the heap: the smallest error first, then the lowest position (a total
/// order, so the result is deterministic).
#[derive(PartialEq)]
struct Cand {
    err: f32,
    v: u32,
    u: u32,
    stamp: u32,
}
impl Eq for Cand {}
impl Ord for Cand {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.err.total_cmp(&self.err).then(o.v.cmp(&self.v)).then(o.u.cmp(&self.u))
    }
}
impl PartialOrd for Cand {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

/// The mesh at POSITION level: a topological vertex is a position (exact bits); its attribute
/// copies (a UV, normal or lightmap-chart seam through it) move together, each onto the copy of
/// the target it shares a face with — so a seam is only ever shortened ALONG itself.
struct Mesh {
    /// per position: its point
    pos: Vec<[f32; 3]>,
    /// per attribute vertex: its position
    pos_of: Vec<u32>,
    /// faces as attribute-vertex corners
    tris: Vec<[u32; 3]>,
    alive: Vec<bool>,
    /// per position: the live faces using any copy of it
    pf: Vec<Vec<u32>>,
    /// per face: the original points it stands for (removed positions)
    carried: Vec<Vec<[f32; 3]>>,
    stamp: Vec<u32>,
    open_ok: bool,
    /// the ORIGINAL surface, for the two-sided check (every new face within tol of it)
    orig: Orig,
    tol: f32,
    /// positions another visual of the item also draws (never moved: a slit against it)
    locked: Vec<bool>,
}

/// The original triangles in a uniform 3-D grid: is a point within `r` of the original surface?
struct Orig {
    tris: Vec<[[f32; 3]; 3]>,
    cell: f32,
    grid: HashMap<(i32, i32, i32), Vec<u32>>,
}

impl Orig {
    fn new(tris: Vec<[[f32; 3]; 3]>, cell: f32) -> Orig {
        let mut grid: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            let lo = [0, 1, 2].map(|k| (t[0][k].min(t[1][k]).min(t[2][k]) / cell).floor() as i32);
            let hi = [0, 1, 2].map(|k| (t[0][k].max(t[1][k]).max(t[2][k]) / cell).floor() as i32);
            if (hi[0] - lo[0] + 1) as i64 * (hi[1] - lo[1] + 1) as i64 * (hi[2] - lo[2] + 1) as i64 > 200_000 {
                continue;
            }
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        grid.entry((x, y, z)).or_default().push(i as u32);
                    }
                }
            }
        }
        Orig { tris, cell, grid }
    }

    fn near(&self, p: [f32; 3], r: f32) -> bool {
        let lo = [0, 1, 2].map(|k| ((p[k] - r) / self.cell).floor() as i32);
        let hi = [0, 1, 2].map(|k| ((p[k] + r) / self.cell).floor() as i32);
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    if let Some(ts) = self.grid.get(&(x, y, z)) {
                        if ts.iter().any(|t| {
                            let t = &self.tris[*t as usize];
                            point_triangle(p, t[0], t[1], t[2]) <= r
                        }) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

/// Points over a triangle at about `spacing` apart (its corners, edges and inside; at most 91).
fn samples(a: [f32; 3], b: [f32; 3], c: [f32; 3], spacing: f32) -> Vec<[f32; 3]> {
    let longest = len(sub(a, b)).max(len(sub(b, c))).max(len(sub(c, a)));
    let n = ((longest / spacing).ceil() as usize).clamp(1, 12);
    let mut out = Vec::new();
    for i in 0..=n {
        for j in 0..=(n - i) {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            let w = 1.0 - u - v;
            out.push([a[0] * w + b[0] * u + c[0] * v, a[1] * w + b[1] * u + c[1] * v, a[2] * w + b[2] * u + c[2] * v]);
        }
    }
    out
}

impl Mesh {
    fn fpos(&self, t: u32) -> [u32; 3] {
        self.tris[t as usize].map(|a| self.pos_of[a as usize])
    }

    fn point(&self, a: u32) -> [f32; 3] {
        self.pos[self.pos_of[a as usize] as usize]
    }

    /// The neighbour positions of `p`, each with the number of its live faces on that edge.
    fn edges(&self, p: u32) -> BTreeMap<u32, u32> {
        let mut c = BTreeMap::new();
        for &t in &self.pf[p as usize] {
            for w in self.fpos(t) {
                if w != p {
                    *c.entry(w).or_default() += 1;
                }
            }
        }
        c
    }

    /// Where `p` may go: any neighbour inside one closed manifold fan; along an open edge (one
    /// open manifold fan, TINY_LOD_DECIMATE_OPEN=1) only its two open neighbours; else nowhere.
    fn targets(&self, p: u32) -> Vec<u32> {
        let fan = self.pf[p as usize].len();
        if fan < 2 || self.locked[p as usize] {
            return Vec::new();
        }
        let e = self.edges(p);
        if e.values().any(|c| *c > 2) {
            return Vec::new();
        }
        let ends: Vec<u32> = e.iter().filter(|(_, c)| **c == 1).map(|(w, _)| *w).collect();
        match ends.len() {
            0 if e.len() == fan && fan >= 3 => e.into_keys().collect(),
            2 if self.open_ok && e.len() == fan + 1 => ends,
            _ => Vec::new(),
        }
    }

    /// Collapsing position `p` into `q`: the faces on the edge (removed) and every other face of
    /// `p` with its new corners; None when not allowed (the link condition: `p` and `q` share
    /// only the edge's opposite positions; every copy of `p` must share a face with exactly one
    /// copy of `q`, else a seam would tear).
    fn plan(&self, p: u32, q: u32) -> Option<(Vec<u32>, Vec<(u32, [u32; 3])>)> {
        let np: std::collections::BTreeSet<u32> = self.edges(p).into_keys().collect();
        let nq: std::collections::BTreeSet<u32> = self.edges(q).into_keys().collect();
        let both: Vec<u32> = self.pf[p as usize].iter().copied().filter(|t| self.fpos(*t).contains(&q)).collect();
        let opposite: std::collections::BTreeSet<u32> = both.iter().flat_map(|t| self.fpos(*t)).filter(|w| *w != p && *w != q).collect();
        if opposite.is_empty() || opposite.len() > 2 || np.intersection(&nq).count() != opposite.len() {
            return None;
        }
        let mut map: BTreeMap<u32, u32> = BTreeMap::new();
        for &t in &both {
            let (tri, fp) = (self.tris[t as usize], self.fpos(t));
            let a = tri[fp.iter().position(|w| *w == p)?];
            let b = tri[fp.iter().position(|w| *w == q)?];
            if *map.entry(a).or_insert(b) != b {
                return None;
            }
        }
        let mut newt = Vec::new();
        for &t in &self.pf[p as usize] {
            if both.contains(&t) {
                continue;
            }
            let (tri, fp) = (self.tris[t as usize], self.fpos(t));
            let k = fp.iter().position(|w| *w == p)?;
            let mut nt = tri;
            nt[k] = *map.get(&tri[k])?;
            newt.push((t, nt));
        }
        Some((both, newt))
    }

    /// The error of collapsing `p` into `q` (the farthest carried point from the new faces),
    /// None when not allowed (see `plan`; no face may flip by more than 60°).
    fn cost(&self, p: u32, q: u32) -> Option<f32> {
        let (_, newt) = self.plan(p, q)?;
        if newt.is_empty() {
            return None;
        }
        for (t, nt) in &newt {
            let old = self.tris[*t as usize];
            let n0 = tri_normal3(self.point(old[0]), self.point(old[1]), self.point(old[2]))?;
            let n1 = tri_normal3(self.point(nt[0]), self.point(nt[1]), self.point(nt[2]))?;
            if dot(n0, n1) < 0.5 {
                return None;
            }
        }
        let mut worst = 0.0f32;
        // the guard reads a surface the car can stand on VERTICALLY (a 3-D error d is d / |n.y|
        // of it): such faces are held to tol × |n.y| — the error is returned divided by it
        let mut factor = 1.0f32;
        for (t, nt) in &newt {
            let _ = t;
            if let Some(n) = tri_normal3(self.point(nt[0]), self.point(nt[1]), self.point(nt[2])) {
                if n[1].abs() >= 0.3 {
                    factor = factor.min(n[1].abs());
                }
            }
        }
        let mut check = |x: [f32; 3]| {
            let d = newt.iter().map(|(_, t)| point_triangle(x, self.point(t[0]), self.point(t[1]), self.point(t[2]))).fold(f32::INFINITY, f32::min);
            worst = worst.max(d);
        };
        check(self.pos[p as usize]);
        for &t in &self.pf[p as usize] {
            for x in &self.carried[t as usize] {
                check(*x);
            }
        }
        Some(worst / factor)
            .filter(|_| {
                // TWO-SIDED: every new face, sampled about every half metre, within tol of the
                // ORIGINAL surface (a collapse may otherwise bridge a gap or fold a fin over,
                // which the removed points alone do not see)
                let r = self.tol * factor;
                newt.iter().all(|(_, t)| samples(self.point(t[0]), self.point(t[1]), self.point(t[2]), 0.5).into_iter().all(|s| self.orig.near(s, r)))
            })
    }

    fn best(&self, p: u32) -> Option<(f32, u32)> {
        self.targets(p).into_iter().filter_map(|q| self.cost(p, q).map(|e| (e, q))).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
    }

    fn collapse(&mut self, p: u32, q: u32) -> bool {
        let Some((both, newt)) = self.plan(p, q) else { return false };
        let mut pts: Vec<[f32; 3]> = vec![self.pos[p as usize]];
        for t in self.pf[p as usize].clone() {
            pts.append(&mut self.carried[t as usize]);
        }
        for &t in &both {
            self.alive[t as usize] = false;
            for w in self.fpos(t) {
                if w != p {
                    self.pf[w as usize].retain(|x| *x != t);
                }
            }
        }
        let mut kept = Vec::new();
        for (t, nt) in &newt {
            self.tris[*t as usize] = *nt;
            self.pf[q as usize].push(*t);
            kept.push(*t);
        }
        self.pf[p as usize].clear();
        self.pf[q as usize].sort_unstable();
        self.pf[q as usize].dedup();
        for x in pts {
            let nearest = kept.iter().copied().min_by(|a, b| {
                let ta = self.tris[*a as usize];
                let tb = self.tris[*b as usize];
                let da = point_triangle(x, self.point(ta[0]), self.point(ta[1]), self.point(ta[2]));
                let db = point_triangle(x, self.point(tb[0]), self.point(tb[1]), self.point(tb[2]));
                da.total_cmp(&db).then(a.cmp(b))
            });
            if let Some(t) = nearest {
                self.carried[t as usize].push(x);
            }
        }
        true
    }
}

/// One visual decimated within `tol`, with the vertex counts before and after; None when nothing
/// could be removed (or the visual is not a plain indexed float3 mesh).
pub fn decimate(v: &CPlugVisualIndexedTriangles, tol: f32) -> Option<(CPlugVisualIndexedTriangles, usize, usize)> {
    let (mut out, b, a) = decimate_many(&[v], &std::collections::HashSet::new(), tol);
    out.pop().flatten().map(|o| (o, b, a))
}

fn decimatable(v: &CPlugVisualIndexedTriangles) -> bool {
    let (pts, idx) = visual_triangles(v);
    let n = pts.len();
    // texcoords kept OUTSIDE the stream (main.tex_coord_sets) take part in the weld key and are
    // compacted by `sub_visual` when they hold one entry per vertex; any other shape (or a
    // skin) leaves the visual as it is
    n >= 3 && idx.len() >= 3 && idx.iter().all(|i| (*i as usize) < n) && v.main.as_ref().map(|m| m.skin.is_none() && m.tex_coord_sets.iter().all(|s| s.coords.len() == n && (s.u01.is_empty() || s.u01.len() == n))).unwrap_or(false)
}

/// SEVERAL visuals of one item decimated TOGETHER within `tol`: one position-level mesh, so an
/// edge two visuals share (a deck's side against its border's) is an inner edge and moves in
/// both at once, and an open edge is only the outline of them all. A position in `locked` (drawn
/// by another visual of the item) never moves. Per input visual: its decimation (None: left as
/// it is); and the vertex counts before and after over the visuals decimated.
pub fn decimate_many(vs: &[&CPlugVisualIndexedTriangles], locked: &std::collections::HashSet<[u32; 3]>, tol: f32) -> (Vec<Option<CPlugVisualIndexedTriangles>>, usize, usize) {
    let none = || (vs.iter().map(|_| None).collect::<Vec<_>>(), 0, 0);
    // 1. per visual: weld identical vertices (every stream's bytes equal: lossless); one global
    // attribute index space (visual k's vertices from offsets[k])
    let ok: Vec<bool> = vs.iter().map(|v| decimatable(v)).collect();
    let mut offsets = Vec::new();
    let mut gpts: Vec<[f32; 3]> = Vec::new();
    let mut gcanon: Vec<u32> = Vec::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    let mut extra_locked: std::collections::HashSet<[u32; 3]> = locked.clone();
    for (k, v) in vs.iter().enumerate() {
        let (pts, idx) = visual_triangles(v);
        let off = gpts.len() as u32;
        offsets.push(off);
        if !ok[k] {
            // a visual that cannot be decimated still draws its positions: they stay put
            extra_locked.extend(pts.iter().map(|p| p.map(f32::to_bits)));
            gpts.extend(pts.iter().copied());
            gcanon.extend((0..pts.len() as u32).map(|i| off + i));
            continue;
        }
        let mut first: HashMap<Vec<u8>, u32> = HashMap::new();
        for i in 0..pts.len() {
            let c = *first.entry(vertex_key(v, i)).or_insert(i as u32);
            gcanon.push(off + c);
        }
        gpts.extend(pts.iter().copied());
        for t in idx.chunks_exact(3) {
            faces.push([gcanon[(off + t[0]) as usize], gcanon[(off + t[1]) as usize], gcanon[(off + t[2]) as usize]]);
        }
    }
    offsets.push(gpts.len() as u32);
    let n = gpts.len();
    // 2. positions (shared across the visuals)
    let mut pid: HashMap<[u32; 3], u32> = HashMap::new();
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut pos_of = vec![0u32; n];
    for i in 0..n {
        let id = *pid.entry(gpts[i].map(f32::to_bits)).or_insert_with(|| {
            pos.push(gpts[i]);
            (pos.len() - 1) as u32
        });
        pos_of[i] = id;
    }
    let np = pos.len();
    let lock: Vec<bool> = pos.iter().map(|p| extra_locked.contains(&p.map(f32::to_bits))).collect();
    // 3. faces (degenerate at position level: out)
    let tris: Vec<[u32; 3]> = faces
        .into_iter()
        .filter(|tri| {
            let fp = tri.map(|a| pos_of[a as usize]);
            fp[0] != fp[1] && fp[1] != fp[2] && fp[0] != fp[2]
        })
        .collect();
    if tris.is_empty() {
        return none();
    }
    let mut pf: Vec<Vec<u32>> = vec![Vec::new(); np];
    for (ti, t) in tris.iter().enumerate() {
        for a in t {
            pf[pos_of[*a as usize] as usize].push(ti as u32);
        }
    }
    let nt = tris.len();
    // an open edge of them all may still meet ANOTHER item's surface: off unless
    // TINY_LOD_DECIMATE_OPEN=1 (the chord then stays within `tol` of the old outline)
    let open_ok = std::env::var("TINY_LOD_DECIMATE_OPEN").map(|v| v == "1").unwrap_or(false);
    let orig = Orig::new(tris.iter().map(|t| t.map(|a| pos[pos_of[a as usize] as usize])).collect(), 4.0);
    let mut m = Mesh { pos, pos_of, tris, alive: vec![true; nt], pf, carried: vec![Vec::new(); nt], stamp: vec![0; np], open_ok, orig, tol, locked: lock };
    // 4. greedy collapses, smallest error first
    let mut heap = BinaryHeap::new();
    for p in 0..np as u32 {
        if let Some((err, q)) = m.best(p) {
            if err <= tol {
                heap.push(Cand { err, v: p, u: q, stamp: 0 });
            }
        }
    }
    let mut removed = 0usize;
    while let Some(c) = heap.pop() {
        if m.pf[c.v as usize].is_empty() || c.stamp != m.stamp[c.v as usize] {
            continue;
        }
        // the fan may have changed since: re-cost before collapsing
        match m.best(c.v) {
            Some((err, q)) if err <= tol && q == c.u && (err - c.err).abs() < 1e-6 => {}
            Some((err, q)) if err <= tol => {
                m.stamp[c.v as usize] += 1;
                heap.push(Cand { err, v: c.v, u: q, stamp: m.stamp[c.v as usize] });
                continue;
            }
            _ => continue,
        }
        let ring: Vec<u32> = m.edges(c.v).into_keys().collect();
        if !m.collapse(c.v, c.u) {
            continue;
        }
        removed += 1;
        for w in ring {
            m.stamp[w as usize] += 1;
            if let Some((err, q)) = m.best(w) {
                if err <= tol {
                    heap.push(Cand { err, v: w, u: q, stamp: m.stamp[w as usize] });
                }
            }
        }
    }
    if std::env::var("TINY_LOD_DECIMATE_DEBUG").is_ok() {
        eprintln!("decimate: {} visual(s), {n} vertices, {np} positions, {nt} triangles, {removed} positions removed", vs.len());
    }
    if removed == 0 {
        return none();
    }
    // 5. per visual: its live faces (original order) back in its own indices, the unused
    // vertices dropped
    let (mut before, mut after) = (0usize, 0usize);
    let mut outs = Vec::new();
    for (k, v) in vs.iter().enumerate() {
        if !ok[k] {
            outs.push(None);
            continue;
        }
        let (lo, hi) = (offsets[k], offsets[k + 1]);
        let idx: Vec<u32> = m.tris.iter().zip(&m.alive).filter(|(t, a)| **a && t[0] >= lo && t[0] < hi).flat_map(|(t, _)| t.iter().map(move |x| x - lo)).collect();
        let n_k = (hi - lo) as usize;
        let mut o = (*v).clone();
        let Some(ib) = o.index_buffer.as_mut() else {
            outs.push(None);
            continue;
        };
        if idx.len() == ib.indices.len() && idx == ib.indices {
            outs.push(None);
            continue;
        }
        ib.indices = idx;
        let keep = vec![true; o.index_buffer.as_ref().map(|b| b.indices.len() / 3).unwrap_or(0)];
        match sub_visual(&o, &keep) {
            Ok(o) => {
                let a = o.main.as_ref().map(|mm| mm.count.max(0) as usize).unwrap_or(0);
                before += n_k;
                after += a;
                if std::env::var("TINY_LOD_DECIMATE_DEBUG").is_ok() {
                    let (dev, missing, cells) = vertical_dev(v, &o, 0.5);
                    eprintln!("decimate check: {n_k} -> {a} vertices: top surface max {dev:.3} m off over {cells} cells, {missing} cells lost");
                }
                outs.push(Some(o));
            }
            Err(_) => outs.push(None),
        }
    }
    (outs, before, after)
}

/// The top-most surface of two visuals on an xz lattice: the largest vertical gap where both
/// draw, how many cells `a` draws that `b` lost, and the cells compared.
pub fn vertical_dev(a: &CPlugVisualIndexedTriangles, b: &CPlugVisualIndexedTriangles, step: f32) -> (f32, usize, usize) {
    let field = |v: &CPlugVisualIndexedTriangles| {
        let (p, idx) = visual_triangles(v);
        let mut f: HashMap<(i32, i32), f32> = HashMap::new();
        for t in idx.chunks_exact(3) {
            let (Some(x), Some(y), Some(z)) = (p.get(t[0] as usize), p.get(t[1] as usize), p.get(t[2] as usize)) else { continue };
            crate::lodguard::raster(*x, *y, *z, step, &mut |c, h| {
                let e = f.entry(c).or_insert(f32::NEG_INFINITY);
                if h > *e {
                    *e = h;
                }
            });
        }
        f
    };
    let (fa, fb) = (field(a), field(b));
    let (mut dev, mut missing, mut cells) = (0.0f32, 0usize, 0usize);
    for (c, ya) in &fa {
        match fb.get(c) {
            Some(yb) => {
                cells += 1;
                dev = dev.max((ya - yb).abs());
            }
            None => missing += 1,
        }
    }
    (dev, missing, cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_triangle_distances() {
        let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        assert!((point_triangle([0.2, 0.5, 0.2], a, b, c) - 0.5).abs() < 1e-6);
        assert!((point_triangle([-1.0, 0.0, 0.0], a, b, c) - 1.0).abs() < 1e-6);
        assert!((point_triangle([1.0, 0.0, 1.0], a, b, c) - 0.5f32.sqrt()).abs() < 1e-5);
    }
}
