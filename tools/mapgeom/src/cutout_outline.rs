//! Exact-outline cutouts (Hugo's tester, 2026-10-24 06:37: "use curves, not 4×4 squares; the cutouts need to be
//! perfect cutouts of the blocks"). The standable grass fill of a block variant is the pak's own fill mesh (the
//! `*FCBGround` bottom clips + the variant's own grass-level collision); its region in the plane MINUS the
//! structure footprint dilated by the inset is traced as POLYGONS: a marching-squares contour of the field
//! `inside(fill) ∧ dist(structure) ≥ inset` on a fine grid (`CONTOUR_STEP`), whose vertices are then SNAPPED
//! onto the fill mesh's own boundary edges wherever the fill boundary is what limits the region — so a curb that
//! is a polyline of the pak mesh comes out as that polyline (Hugo's "perfect": the block's own geometry), and the
//! inset offset around walls/pillars is a smooth curve at the grid resolution. Loops are oriented (outer CCW,
//! holes CW), simplified (collinear points dropped), triangulated by ear clipping with holes bridged to their
//! outer loop, and extruded into the trigger prism (below .. above) + the indicator plane (above the floor).
use std::collections::HashMap;

/// The contour grid step in metres (8 per metre).
pub const CONTOUR_STEP: f32 = 0.125;
/// A contour vertex within this of a fill-boundary edge snaps onto it.
const SNAP: f32 = 0.2;

#[derive(Clone, Debug)]
pub struct Outline {
    /// outer loops (CCW) with their holes (CW)
    pub polygons: Vec<(Vec<[f32; 2]>, Vec<Vec<[f32; 2]>>)>,
    pub area: f32,
    pub vertices: usize,
    /// vertices that lie on a fill-mesh edge (the pak's own curb) vs on the inset offset
    pub on_mesh: usize,
    pub notes: Vec<String>,
}

fn dist_point_seg(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> (f32, [f32; 2]) {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dz * dz;
    let t = if l2 < 1e-12 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / l2).clamp(0.0, 1.0) };
    let q = [a[0] + t * dx, a[1] + t * dz];
    (((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt(), q)
}

fn inside_tri(p: [f32; 2], t: &[[f32; 3]; 3]) -> bool {
    let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
    let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

/// Distance from a point to a triangle's xz projection (0 inside).
fn dist_tri(p: [f32; 2], t: &[[f32; 3]; 3]) -> f32 {
    if inside_tri(p, t) {
        return 0.0;
    }
    let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
    dist_point_seg(p, a, b).0.min(dist_point_seg(p, b, c).0).min(dist_point_seg(p, c, a).0)
}

/// Bucketed triangle lookup on a 2 m grid.
struct Buckets<'a> {
    cell: f32,
    map: HashMap<(i32, i32), Vec<&'a [[f32; 3]; 3]>>,
}

impl<'a> Buckets<'a> {
    fn new(tris: &'a [[[f32; 3]; 3]], cell: f32) -> Self {
        let mut map: HashMap<(i32, i32), Vec<&'a [[f32; 3]; 3]>> = HashMap::new();
        for t in tris {
            let (x0, x1) = (t.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max));
            let (z0, z1) = (t.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[2]).fold(f32::NEG_INFINITY, f32::max));
            for iz in (z0 / cell).floor() as i32..=(z1 / cell).floor() as i32 {
                for ix in (x0 / cell).floor() as i32..=(x1 / cell).floor() as i32 {
                    map.entry((ix, iz)).or_default().push(t);
                }
            }
        }
        Buckets { cell, map }
    }
    fn near(&self, p: [f32; 2], r: f32) -> impl Iterator<Item = &&'a [[f32; 3]; 3]> {
        let (ix0, ix1) = (((p[0] - r) / self.cell).floor() as i32, ((p[0] + r) / self.cell).floor() as i32);
        let (iz0, iz1) = (((p[1] - r) / self.cell).floor() as i32, ((p[1] + r) / self.cell).floor() as i32);
        let mut v: Vec<&&'a [[f32; 3]; 3]> = Vec::new();
        for iz in iz0..=iz1 {
            for ix in ix0..=ix1 {
                if let Some(l) = self.map.get(&(ix, iz)) {
                    v.extend(l.iter());
                }
            }
        }
        v.into_iter()
    }
}

/// The scalar field: positive inside the cutout region. min(fill-inside ? +d_fill_boundary : −d_fill, d_struct − inset)
/// where d_struct is the distance to the nearest structure triangle; approximated as: inside fill → the smaller of
/// (distance to the fill boundary, d_struct − inset); outside fill → negative.
fn field(p: [f32; 2], fill: &Buckets, fill_edges: &[([f32; 2], [f32; 2])], structure: &Buckets, inset: f32) -> f32 {
    let in_fill = fill.near(p, 0.0).any(|t| inside_tri(p, t));
    let d_edge = fill_edges.iter().map(|(a, b)| dist_point_seg(p, *a, *b).0).fold(f32::INFINITY, f32::min);
    let d_struct = structure.near(p, inset + 1.0).map(|t| dist_tri(p, t)).fold(f32::INFINITY, f32::min);
    if !in_fill {
        return -d_edge.max(0.001);
    }
    d_edge.min(d_struct - inset)
}

/// Trace the region's contour loops by marching squares on a `step` grid over the block's footprint box.
pub fn trace(fill_tris: &[[[f32; 3]; 3]], structure_tris: &[[[f32; 3]; 3]], size: (f32, f32), inset: f32, step: f32) -> Outline {
    let fill = Buckets::new(fill_tris, 2.0);
    let structure = Buckets::new(structure_tris, 2.0);
    let fill_loops = crate::floor_finish::boundary_loops(fill_tris);
    let mut fill_edges: Vec<([f32; 2], [f32; 2])> = Vec::new();
    for l in &fill_loops {
        for i in 0..l.len() {
            fill_edges.push((l[i], l[(i + 1) % l.len()]));
        }
    }
    let (nx, nz) = ((size.0 / step).round() as usize + 1, (size.1 / step).round() as usize + 1);
    // sample the field at the grid nodes (slightly inside the footprint box so the box edge is a contour)
    let mut f = vec![0.0f32; nx * nz];
    for iz in 0..nz {
        for ix in 0..nx {
            let p = [ix as f32 * step, iz as f32 * step];
            let v = field(p, &fill, &fill_edges, &structure, inset);
            // the footprint box itself bounds the region: outside it (or on its edge) negative
            let edge = p[0].min(size.0 - p[0]).min(p[1]).min(size.1 - p[1]);
            f[iz * nx + ix] = if edge <= 1e-4 { -0.001 } else { v.min(edge) };
        }
    }
    if std::env::var_os("OUTLINE_DEBUG").is_some() {
        let border_neg = (0..nx).filter(|&ix| f[ix] <= 0.0).count() + (0..nx).filter(|&ix| f[(nz - 1) * nx + ix] <= 0.0).count();
        eprintln!("grid {nx}×{nz}, border nodes negative (bottom+top rows): {border_neg} of {}", 2 * nx);
        for iz in (0..nz).rev().step_by(4) {
            let row: String = (0..nx).step_by(4).map(|ix| { let v = f[iz * nx + ix]; if v > 0.0 { '#' } else if v > -0.01 { '-' } else { '.' } }).collect();
            eprintln!("{row}");
        }
    }
    // marching squares: segments per cell, chained into loops
    let at = |ix: usize, iz: usize| f[iz * nx + ix];
    let interp = |p: [f32; 2], q: [f32; 2], a: f32, b: f32| -> [f32; 2] {
        let t = if (a - b).abs() < 1e-9 { 0.5 } else { a / (a - b) };
        [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]
    };
    let key = |p: [f32; 2]| -> (i64, i64) { ((p[0] * 2000.0).round() as i64, (p[1] * 2000.0).round() as i64) };
    let mut segs: Vec<([f32; 2], [f32; 2])> = Vec::new();
    for iz in 0..nz - 1 {
        for ix in 0..nx - 1 {
            let (p00, p10, p11, p01) = ([ix as f32 * step, iz as f32 * step], [(ix + 1) as f32 * step, iz as f32 * step], [(ix + 1) as f32 * step, (iz + 1) as f32 * step], [ix as f32 * step, (iz + 1) as f32 * step]);
            let (v00, v10, v11, v01) = (at(ix, iz), at(ix + 1, iz), at(ix + 1, iz + 1), at(ix, iz + 1));
            let c = ((v00 > 0.0) as u8) | (((v10 > 0.0) as u8) << 1) | (((v11 > 0.0) as u8) << 2) | (((v01 > 0.0) as u8) << 3);
            if c == 0 || c == 15 {
                continue;
            }
            let e_bottom = || interp(p00, p10, v00, v10);
            let e_right = || interp(p10, p11, v10, v11);
            let e_top = || interp(p01, p11, v01, v11);
            let e_left = || interp(p00, p01, v00, v01);
            // oriented so the positive region is on the LEFT of each segment (CCW outer loops)
            match c {
                1 => segs.push((e_left(), e_bottom())),
                2 => segs.push((e_bottom(), e_right())),
                3 => segs.push((e_left(), e_right())),
                4 => segs.push((e_right(), e_top())),
                5 => {
                    let centre = (v00 + v10 + v11 + v01) / 4.0;
                    if centre > 0.0 { segs.push((e_left(), e_top())); segs.push((e_right(), e_bottom())); } else { segs.push((e_left(), e_bottom())); segs.push((e_right(), e_top())); }
                }
                6 => segs.push((e_bottom(), e_top())),
                7 => segs.push((e_left(), e_top())),
                8 => segs.push((e_top(), e_left())),
                9 => segs.push((e_top(), e_bottom())),
                10 => {
                    let centre = (v00 + v10 + v11 + v01) / 4.0;
                    if centre > 0.0 { segs.push((e_bottom(), e_left())); segs.push((e_top(), e_right())); } else { segs.push((e_top(), e_left())); segs.push((e_bottom(), e_right())); }
                }
                11 => segs.push((e_top(), e_right())),
                12 => segs.push((e_right(), e_left())),
                13 => segs.push((e_right(), e_bottom())),
                14 => segs.push((e_bottom(), e_left())),
                _ => {}
            }
        }
    }
    // the case table above puts the positive region on the RIGHT of each segment; reverse for CCW outer loops
    for s in segs.iter_mut() {
        std::mem::swap(&mut s.0, &mut s.1);
    }
    // chain
    let mut next: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        next.entry(key(s.0)).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut loops: Vec<Vec<[f32; 2]>> = Vec::new();
    for i in 0..segs.len() {
        if used[i] {
            continue;
        }
        let mut lp = vec![segs[i].0];
        let mut cur = i;
        used[i] = true;
        let start = key(segs[i].0);
        loop {
            let end = segs[cur].1;
            if key(end) == start {
                break;
            }
            lp.push(end);
            let Some(cands) = next.get(&key(end)) else { break };
            let Some(&n) = cands.iter().find(|&&j| !used[j]) else { break };
            used[n] = true;
            cur = n;
        }
        if lp.len() >= 3 {
            loops.push(lp);
        }
    }
    if std::env::var_os("OUTLINE_DEBUG").is_some() {
        eprintln!("{} segments → {} raw loops: {:?}", segs.len(), loops.len(), loops.iter().map(|l| (l.len(), crate::floor_finish::polygon_area(l))).collect::<Vec<_>>());
    }
    // snap onto the fill mesh edges where the fill boundary is the limit, then simplify
    let mut on_mesh = 0usize;
    let mut out_loops: Vec<Vec<[f32; 2]>> = Vec::new();
    for lp in loops {
        let mut snapped: Vec<[f32; 2]> = Vec::with_capacity(lp.len());
        for p in &lp {
            let mut best: Option<([f32; 2], f32)> = None;
            for (a, b) in &fill_edges {
                let (d, q) = dist_point_seg(*p, *a, *b);
                if d < SNAP && best.map_or(true, |(_, bd)| d < bd) {
                    best = Some((q, d));
                }
            }
            match best {
                // snap only where the structure is NOT the limit here (the inset band): check the field at the snapped point
                Some((q, _)) if structure.near(q, inset + 0.5).map(|t| dist_tri(q, t)).fold(f32::INFINITY, f32::min) >= inset - 0.05 => {
                    snapped.push(q);
                    on_mesh += 1;
                }
                _ => snapped.push(*p),
            }
        }
        // drop near-duplicates and collinear runs
        let mut simp: Vec<[f32; 2]> = Vec::new();
        for p in snapped {
            if let Some(l) = simp.last() {
                if (l[0] - p[0]).abs() < 1e-3 && (l[1] - p[1]).abs() < 1e-3 {
                    continue;
                }
            }
            simp.push(p);
        }
        let mut i = 0;
        while simp.len() >= 4 && i < simp.len() {
            let (a, b, c) = (simp[(i + simp.len() - 1) % simp.len()], simp[i], simp[(i + 1) % simp.len()]);
            // drop a vertex only when it sits within 0.5 mm of the chord of its neighbours (a straight run); an arc
            // sampled at 0.125 m keeps its points (the chord deviation of a 4 m-radius arc at that step is ~0.5 mm,
            // of a 16 m one ~0.1 mm — so coarse arcs survive, the finest barely: the pak mesh snap wins those)
            let cross = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
            let (d, _) = dist_point_seg(b, a, c);
            if cross.abs() < 1e-7 || d < 0.0005 {
                simp.remove(i);
            } else {
                i += 1;
            }
        }
        if simp.len() >= 3 {
            out_loops.push(simp);
        }
    }
    // on_mesh: recount on the simplified loops (a vertex within 1 mm of a fill edge)
    on_mesh = out_loops.iter().flatten().filter(|p| fill_edges.iter().any(|(a, b)| dist_point_seg(**p, *a, *b).0 < 0.001)).count();
    // outer loops (CCW, positive area) with their holes (CW): a hole belongs to the smallest outer containing it
    let mut outers: Vec<(Vec<[f32; 2]>, Vec<Vec<[f32; 2]>>)> = Vec::new();
    let mut holes: Vec<Vec<[f32; 2]>> = Vec::new();
    for l in out_loops {
        if crate::floor_finish::polygon_area(&l) > 0.0 {
            outers.push((l, Vec::new()));
        } else {
            holes.push(l);
        }
    }
    for h in holes {
        let p = h[0];
        let mut best: Option<(usize, f32)> = None;
        for (i, (o, _)) in outers.iter().enumerate() {
            if point_in_poly(p, o) {
                let a = crate::floor_finish::polygon_area(o);
                if best.map_or(true, |(_, ba)| a < ba) {
                    best = Some((i, a));
                }
            }
        }
        if let Some((i, _)) = best {
            outers[i].1.push(h);
        }
    }
    let area: f32 = outers.iter().map(|(o, hs)| crate::floor_finish::polygon_area(o) + hs.iter().map(|h| crate::floor_finish::polygon_area(h)).sum::<f32>()).sum();
    let vertices = outers.iter().map(|(o, hs)| o.len() + hs.iter().map(|h| h.len()).sum::<usize>()).sum();
    let notes = vec![format!("{} outer loops, {} holes, {vertices} vertices ({on_mesh} on the pak mesh edges), area {area:.1} m², step {step} m, inset {inset} m", outers.len(), outers.iter().map(|(_, h)| h.len()).sum::<usize>())];
    Outline { polygons: outers, area, vertices, on_mesh, notes }
}

pub fn point_in_poly(p: [f32; 2], poly: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Ear-clip a polygon with holes: the holes are bridged into the outer loop (each hole's rightmost vertex joined
/// to the nearest visible outer vertex), then classic ear clipping. Returns triangles as index triples into the
/// returned vertex list.
pub fn triangulate(outer: &[[f32; 2]], holes: &[Vec<[f32; 2]>]) -> (Vec<[f32; 2]>, Vec<[usize; 3]>) {
    let mut poly: Vec<[f32; 2]> = outer.to_vec();
    // bridge holes, largest x first
    let mut hs: Vec<Vec<[f32; 2]>> = holes.to_vec();
    hs.sort_by(|a, b| b.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max).partial_cmp(&a.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max)).unwrap());
    for h in hs {
        // the hole's rightmost vertex
        let (hi, hp) = h.iter().enumerate().max_by(|a, b| a.1[0].partial_cmp(&b.1[0]).unwrap()).map(|(i, p)| (i, *p)).unwrap();
        // the nearest outer vertex to its right (visible enough: closest by distance among those with x >= hp.x, else closest overall)
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in poly.iter().enumerate() {
            let d = (p[0] - hp[0]).powi(2) + (p[1] - hp[1]).powi(2);
            let ok = p[0] >= hp[0] - 1e-3;
            if ok && best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        let bi = match best {
            Some((i, _)) => i,
            None => poly.iter().enumerate().min_by(|a, b| ((a.1[0] - hp[0]).powi(2) + (a.1[1] - hp[1]).powi(2)).partial_cmp(&((b.1[0] - hp[0]).powi(2) + (b.1[1] - hp[1]).powi(2))).unwrap()).map(|(i, _)| i).unwrap(),
        };
        // splice: outer[..=bi], hole from hi around back to hi, bridge back to outer[bi], outer[bi+1..]
        let mut spliced: Vec<[f32; 2]> = Vec::with_capacity(poly.len() + h.len() + 2);
        spliced.extend_from_slice(&poly[..=bi]);
        for k in 0..=h.len() {
            spliced.push(h[(hi + k) % h.len()]);
        }
        spliced.push(poly[bi]);
        spliced.extend_from_slice(&poly[bi + 1..]);
        poly = spliced;
    }
    // ear clipping
    let n = poly.len();
    let mut idx: Vec<usize> = (0..n).collect();
    let mut tris: Vec<[usize; 3]> = Vec::new();
    let area2 = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
    let mut guard = 0usize;
    while idx.len() > 3 && guard < 10 * n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if area2(a, b, c) <= 1e-7 {
                continue; // reflex or degenerate
            }
            let mut ear = true;
            for &j in &idx {
                if j == ia || j == ib || j == ic {
                    continue;
                }
                let p = poly[j];
                if (p[0] - a[0]).abs() < 1e-6 && (p[1] - a[1]).abs() < 1e-6 || (p[0] - b[0]).abs() < 1e-6 && (p[1] - b[1]).abs() < 1e-6 || (p[0] - c[0]).abs() < 1e-6 && (p[1] - c[1]).abs() < 1e-6 {
                    continue; // a bridge duplicate of a corner
                }
                if area2(a, b, p) >= -1e-7 && area2(b, c, p) >= -1e-7 && area2(c, a, p) >= -1e-7 {
                    ear = false;
                    break;
                }
            }
            if ear {
                tris.push([ia, ib, ic]);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            // no ear found (numerical trouble): clip the most convex vertex anyway
            let m = idx.len();
            let (mut best, mut bv) = (0usize, f32::NEG_INFINITY);
            for i in 0..m {
                let v = area2(poly[idx[(i + m - 1) % m]], poly[idx[i]], poly[idx[(i + 1) % m]]);
                if v > bv {
                    bv = v;
                    best = i;
                }
            }
            tris.push([idx[(best + m - 1) % m], idx[best], idx[(best + 1) % m]]);
            idx.remove(best);
        }
    }
    if idx.len() == 3 {
        tris.push([idx[0], idx[1], idx[2]]);
    }
    (poly, tris)
}

/// The cutout's trigger prism(s) and indicator quads from the outline: a closed prism per polygon (top, bottom,
/// side walls along every loop), plus the top face again as the indicator mesh. Returns (verts, tris) for the
/// trigger and (verts, tris) for the indicator (lifted).
pub fn extrude(outline: &Outline, below: f32, above: f32, lift: f32) -> ((Vec<[f32; 3]>, Vec<[u32; 3]>), (Vec<[f32; 3]>, Vec<[u32; 3]>)) {
    let mut tv: Vec<[f32; 3]> = Vec::new();
    let mut tt: Vec<[u32; 3]> = Vec::new();
    let mut iv: Vec<[f32; 3]> = Vec::new();
    let mut it: Vec<[u32; 3]> = Vec::new();
    let (y0, y1) = (-below, above);
    for (outer, holes) in &outline.polygons {
        let (pts, tris) = triangulate(outer, holes);
        let base = tv.len() as u32;
        let n = pts.len() as u32;
        for p in &pts {
            tv.push([p[0], y0, p[1]]);
        }
        for p in &pts {
            tv.push([p[0], y1, p[1]]);
        }
        for t in &tris {
            // bottom faces down, top faces up
            tt.push([base + t[0] as u32, base + t[2] as u32, base + t[1] as u32]);
            tt.push([base + n + t[0] as u32, base + n + t[1] as u32, base + n + t[2] as u32]);
        }
        // walls along each original loop
        let mut wall = |lp: &[[f32; 2]], ccw: bool| {
            let b0 = tv.len() as u32;
            for p in lp {
                tv.push([p[0], y0, p[1]]);
            }
            for p in lp {
                tv.push([p[0], y1, p[1]]);
            }
            let m = lp.len() as u32;
            for i in 0..m {
                let j = (i + 1) % m;
                let (a0, b0i, a1, b1) = (b0 + i, b0 + j, b0 + m + i, b0 + m + j);
                if ccw {
                    tt.push([a0, b0i, b1]);
                    tt.push([a0, b1, a1]);
                } else {
                    tt.push([a0, b1, b0i]);
                    tt.push([a0, a1, b1]);
                }
            }
        };
        wall(outer, true);
        for h in holes {
            wall(h, true); // holes are CW: the same winding order yields inward-facing walls
        }
        let ib = iv.len() as u32;
        for p in &pts {
            iv.push([p[0], lift, p[1]]);
        }
        for t in &tris {
            it.push([ib + t[0] as u32, ib + t[1] as u32, ib + t[2] as u32]);
        }
    }
    ((tv, tt), (iv, it))
}
