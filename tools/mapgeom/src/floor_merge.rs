//! Floor Finish v4.13 (Hugo 12:23: "look into the way FinishPlane is built — maybe improve how you extend the
//! hitbox"): (1) the MAXIMAL-RECTANGLE MERGE of the plane's boxes — boxes with identical (bottom, top, indicator)
//! heights that are adjacent and span-aligned become one: first consecutive z-rows with the same x-span, then
//! x-adjacent rectangles with the same z-span, repeated until nothing merges; the union of volumes is unchanged
//! (checked by rasterising both at 1 m: the per-square set of height intervals must be identical); (2) the SLOPED
//! RING WEDGES live in `outer_ring` (the `Ramp`), this module only rebuilds the mesh from slabs + ramps.
use std::collections::HashMap;

use crate::floor_finish::FloorMesh;
use crate::outer_ring::Ramp;

/// One axis-aligned box of the trigger with the ground height its indicator marks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slab {
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    pub y0: f32,
    pub y1: f32,
    pub ind: f32,
}

fn cm(v: f32) -> i64 {
    (v * 100.0).round() as i64
}

/// The boxes of a mesh built by the floor builders: 8-vertex boxes in `verts`, their anchors in `quads` (same
/// order). Ramps (if any) were appended after the boxes and are not boxes: `n_boxes` says where they stop.
pub fn slabs_of(mesh: &FloorMesh) -> Vec<Slab> {
    let n = mesh.quads.len().min(mesh.verts.len() / 8);
    (0..n)
        .map(|k| {
            let c = &mesh.verts[8 * k..8 * k + 8];
            Slab { x0: c[0][0], x1: c[1][0], z0: c[0][2], z1: c[4][2], y0: c[0][1], y1: c[2][1], ind: mesh.quads[k].4 }
        })
        .collect()
}

/// Merge until stable: z-runs with equal x-span, then x-runs with equal z-span.
pub fn merge(slabs: &[Slab]) -> Vec<Slab> {
    let mut list: Vec<Slab> = slabs.to_vec();
    loop {
        let before = list.len();
        // pass A: same heights + same x-span, consecutive in z
        list.sort_by_key(|s| (cm(s.y0), cm(s.y1), cm(s.ind), cm(s.x0), cm(s.x1), cm(s.z0)));
        list = merge_runs(&list, |s| (cm(s.y0), cm(s.y1), cm(s.ind), cm(s.x0), cm(s.x1)), |s| cm(s.z0), |s| cm(s.z1), |a, b| Slab { z1: b.z1, ..*a });
        // pass B: same heights + same z-span, consecutive in x
        list.sort_by_key(|s| (cm(s.y0), cm(s.y1), cm(s.ind), cm(s.z0), cm(s.z1), cm(s.x0)));
        list = merge_runs(&list, |s| (cm(s.y0), cm(s.y1), cm(s.ind), cm(s.z0), cm(s.z1)), |s| cm(s.x0), |s| cm(s.x1), |a, b| Slab { x1: b.x1, ..*a });
        if list.len() == before {
            break;
        }
    }
    list
}

fn merge_runs<K: PartialEq>(sorted: &[Slab], key: impl Fn(&Slab) -> K, start: impl Fn(&Slab) -> i64, end: impl Fn(&Slab) -> i64, join: impl Fn(&Slab, &Slab) -> Slab) -> Vec<Slab> {
    let mut out: Vec<Slab> = Vec::with_capacity(sorted.len());
    for s in sorted {
        if let Some(last) = out.last_mut() {
            if key(last) == key(s) && end(last) == start(s) {
                *last = join(last, s);
                continue;
            }
        }
        out.push(*s);
    }
    out
}

/// The union signature at `step` m: per square, the XOR of a hash of every (y0, y1, ind) interval covering it.
/// Boxes of equal heights never overlap in plan (they come from disjoint rows), so XOR equality ⇔ set equality.
pub fn raster_signature(slabs: &[Slab], step: f32) -> HashMap<(i64, i64), u64> {
    let mut sig: HashMap<(i64, i64), u64> = HashMap::new();
    for s in slabs {
        let h = {
            let mut v: u64 = 0xcbf2_9ce4_8422_2325;
            for k in [cm(s.y0), cm(s.y1), cm(s.ind)] {
                v ^= k as u64;
                v = v.wrapping_mul(0x0000_0100_0000_01b3);
            }
            v | 1
        };
        let (i0, i1) = ((s.x0 / step).round() as i64, (s.x1 / step).round() as i64);
        let (j0, j1) = ((s.z0 / step).round() as i64, (s.z1 / step).round() as i64);
        for j in j0..j1 {
            for i in i0..i1 {
                *sig.entry((i, j)).or_default() ^= h;
            }
        }
    }
    sig.retain(|_, v| *v != 0);
    sig
}

/// Rebuild a mesh's boxes from slabs (+ ramps as sloped prisms), keeping `cells`/`notes`.
pub fn rebuild(mesh: &mut FloorMesh, slabs: &[Slab], ramps: &[Ramp], below: f32, above: f32, lift: f32) {
    let quads: [[u32; 4]; 6] = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [2, 3, 7, 6], [1, 2, 6, 5], [0, 4, 7, 3]];
    mesh.verts.clear();
    mesh.tris.clear();
    mesh.quads.clear();
    mesh.sloped.clear();
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for s in slabs {
        let base = mesh.verts.len() as u32;
        mesh.verts.extend_from_slice(&[[s.x0, s.y0, s.z0], [s.x1, s.y0, s.z0], [s.x1, s.y1, s.z0], [s.x0, s.y1, s.z0], [s.x0, s.y0, s.z1], [s.x1, s.y0, s.z1], [s.x1, s.y1, s.z1], [s.x0, s.y1, s.z1]]);
        for q in quads {
            mesh.tris.push([base + q[0], base + q[1], base + q[2]]);
            mesh.tris.push([base + q[0], base + q[2], base + q[3]]);
        }
        mesh.quads.push((s.x0, s.x1, s.z0, s.z1, s.ind));
    }
    for r in ramps {
        let base = mesh.verts.len() as u32;
        let (y0i, y1i, y0o, y1o) = (r.y_inner - below, r.y_inner + above, r.y_outer - below, r.y_outer + above);
        let t = |x: f32, z: f32| r.t(x, z);
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let corners = [(r.x0, r.z0), (r.x1, r.z0), (r.x1, r.z0), (r.x0, r.z0), (r.x0, r.z1), (r.x1, r.z1), (r.x1, r.z1), (r.x0, r.z1)];
        for (k, (x, z)) in corners.iter().enumerate() {
            let tt = t(*x, *z);
            let y = if matches!(k, 2 | 3 | 6 | 7) { lerp(y1i, y1o, tt) } else { lerp(y0i, y0o, tt) };
            mesh.verts.push([*x, y, *z]);
        }
        for q in quads {
            mesh.tris.push([base + q[0], base + q[1], base + q[2]]);
            mesh.tris.push([base + q[0], base + q[2], base + q[3]]);
        }
        // the indicator: the sloped top as one quad, corners (x0,z0) (x0,z1) (x1,z1) (x1,z0) at the envelope + lift
        let ind = |x: f32, z: f32| lerp(r.y_inner, r.y_outer, t(x, z)) + lift;
        mesh.sloped.push([[r.x0, ind(r.x0, r.z0), r.z0], [r.x0, ind(r.x0, r.z1), r.z1], [r.x1, ind(r.x1, r.z1), r.z1], [r.x1, ind(r.x1, r.z0), r.z0]]);
    }
    for v in &mesh.verts {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    mesh.bbox = (lo, hi);
}

/// Merge a built mesh's boxes in place; returns (boxes before, boxes after, union identical at 1 m).
pub fn merge_mesh(mesh: &mut FloorMesh, ramps: &[Ramp], below: f32, above: f32, lift: f32) -> (usize, usize, bool) {
    let slabs = slabs_of(mesh);
    let merged = merge(&slabs);
    let same = raster_signature(&slabs, 1.0) == raster_signature(&merged, 1.0);
    rebuild(mesh, &merged, ramps, below, above, lift);
    mesh.notes.push(format!("maximal-rectangle merge: {} boxes → {} ({} triangles → {}), union at 1 m {}; {} ramps", slabs.len(), merged.len(), 12 * slabs.len(), 12 * merged.len(), if same { "identical" } else { "DIFFERENT" }, ramps.len()));
    (slabs.len(), merged.len(), same)
}
