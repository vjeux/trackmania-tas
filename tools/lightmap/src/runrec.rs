//! THE RUN RECORD (perf engineer 5, PERF 5.4 — lever 4's geometry side, for engineer 2's fused count): a run
//! of covered pixels of ONE triangle on ONE row, stored as one record instead of a per-pixel (li, CFrag) entry
//! each, and expanded at the scan with EXACTLY the raster's arithmetic — the per-lane edge values of engineer 3's
//! 16-wide row kernel (`t1 = (b−a).x·(py−a.y)` once per row, `p2 = (b−a).y·(qx−a.x)` per lane, `e = t1 − p2`),
//! the barycentrics `e·inv` in the caller's vertex order, and the visit body's depth `(z0·b0 + z1·b1) + z2·b2` —
//! so the (li, z, tri, bias) fragments a run yields are bit for bit the records the per-pixel body would have
//! pushed for those pixels (the test below checks it against the scalar raster's own visits). The count's
//! per-pixel sort is by (z, tri) and its layer logic reads (z, bias) only, so the order the fragments arrive
//! in — list entries first, run fragments after — does not matter.
//!
//! The producer is the block body: a block hands (x0, y, cov, bary) for one triangle in x order along a row;
//! consecutive fully-live blocks of the same triangle and row coalesce into a `Run`. The consumer, at the scan,
//! calls `expand_run` per run (one record fetch and one setup per RUN, not per fragment) and merges the
//! fragments into the pixels' sort.

use crate::bvh::WTri;
use crate::peel::PeelFrame;

/// A run of covered pixels `x0..=x1` of triangle `tri` (the BVH index — the fragments' `tri` key) on row `y`
/// of the frame, with the triangle's depth-bias term (the block body computed it once per triangle).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub tri: u32,
    pub y: u16,
    pub x0: u16,
    pub x1: u16,
    pub bias: f32,
}

/// The triangle's raster setup as the scalar `triangle_clipped_masked` makes it: the oriented vertices (inside
/// positive), the top-left flags, `inv = 1 / |area|`, whether b and c were swapped — and the projected depths in
/// the caller's vertex order.
#[derive(Clone, Copy, Debug)]
pub struct RunSetup {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub c: [f32; 2],
    pub inv: f32,
    pub swapped: bool,
    pub z: [f32; 3],
}

impl RunSetup {
    /// From the triangle's world record and the frame: the same `PeelFrame::project` calls as the band body, the
    /// same orientation as the raster. None for a degenerate triangle (which produces no run).
    pub fn of(t: &WTri, frame: &PeelFrame) -> Option<RunSetup> {
        let p0 = t.p0;
        let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
        let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
        let (x0, y0, z0) = frame.project(p0);
        let (x1, y1, z1) = frame.project(p1);
        let (x2, y2, z2) = frame.project(p2);
        Self::of_projected([[x0, y0], [x1, y1], [x2, y2]], [z0, z1, z2])
    }
    pub fn of_projected(p: [[f32; 2]; 3], z: [f32; 3]) -> Option<RunSetup> {
        // the scalar raster's `edge(p[0], p[1], p[2])`, orientation and `inv`
        let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
        if area == 0.0 || !area.is_finite() {
            return None;
        }
        let (a, b, c, swapped) = if area > 0.0 { (p[0], p[1], p[2], false) } else { (p[0], p[2], p[1], true) };
        let inv = 1.0 / area.abs();
        Some(RunSetup { a, b, c, inv, swapped, z })
    }
}

/// The run's fragments, left to right: `f(x, z)` for x in x0..=x1 with z the visit body's depth at pixel (x, y).
/// Scalar reference form: the per-pixel arithmetic of the 16-wide kernel (identical to it lane for lane).
pub fn expand_run_scalar<F: FnMut(u32, f32)>(run: &Run, s: &RunSetup, mut f: F) {
    let py = run.y as f32 + 0.5;
    let es: [([f32; 2], f32, f32); 3] = [(s.a, s.b[0] - s.a[0], s.b[1] - s.a[1]), (s.b, s.c[0] - s.b[0], s.c[1] - s.b[1]), (s.c, s.a[0] - s.c[0], s.a[1] - s.c[1])];
    let t1 = [es[0].1 * (py - es[0].0[1]), es[1].1 * (py - es[1].0[1]), es[2].1 * (py - es[2].0[1])];
    for x in run.x0..=run.x1 {
        let qx = x as f32 + 0.5;
        let mut e = [0f32; 3];
        for k in 0..3 {
            let d = qx - es[k].0[0];
            let p2 = es[k].2 * d;
            e[k] = t1[k] - p2;
        }
        // the weights in the caller's vertex order: a ← e1·inv, b ← e2·inv, c ← e0·inv (b, c swapped back)
        let (wa, wb, wc) = (e[1] * s.inv, e[2] * s.inv, e[0] * s.inv);
        let bc = if s.swapped { [wa, wc, wb] } else { [wa, wb, wc] };
        let z = s.z[0] * bc[0] + s.z[1] * bc[1] + s.z[2] * bc[2];
        f(x as u32, z);
    }
}

/// `expand_run_scalar` sixteen pixels at a time (AVX-512): `g(x0, n, zs)` per block — lanes 0..n of `zs` are the
/// depths of pixels x0..x0+n. Falls back to the scalar form without AVX-512.
pub fn expand_run<G: FnMut(u32, usize, &[f32; 16])>(run: &Run, s: &RunSetup, g: G) {
    if crate::binsimd::available() {
        // SAFETY: avx512f checked
        unsafe { expand_run_avx512(run, s, g) }
    } else {
        let mut zs = [0f32; 16];
        let mut g = g;
        let mut x = run.x0 as u32;
        while x <= run.x1 as u32 {
            let n = (run.x1 as u32 - x + 1).min(16) as usize;
            let sub = Run { x0: x as u16, x1: (x + n as u32 - 1) as u16, ..*run };
            let mut k = 0;
            expand_run_scalar(&sub, s, |_, z| { zs[k] = z; k += 1; });
            g(x, n, &zs);
            x += n as u32;
        }
    }
}

#[target_feature(enable = "avx512f")]
unsafe fn expand_run_avx512<G: FnMut(u32, usize, &[f32; 16])>(run: &Run, s: &RunSetup, mut g: G) {
    use std::arch::x86_64::*;
    let py = run.y as f32 + 0.5;
    let es: [([f32; 2], f32, f32); 3] = [(s.a, s.b[0] - s.a[0], s.b[1] - s.a[1]), (s.b, s.c[0] - s.b[0], s.c[1] - s.b[1]), (s.c, s.a[0] - s.c[0], s.a[1] - s.c[1])];
    let ax = [_mm512_set1_ps(es[0].0[0]), _mm512_set1_ps(es[1].0[0]), _mm512_set1_ps(es[2].0[0])];
    let dy = [_mm512_set1_ps(es[0].2), _mm512_set1_ps(es[1].2), _mm512_set1_ps(es[2].2)];
    let t1v = [_mm512_set1_ps(es[0].1 * (py - es[0].0[1])), _mm512_set1_ps(es[1].1 * (py - es[1].0[1])), _mm512_set1_ps(es[2].1 * (py - es[2].0[1]))];
    let half = _mm512_set1_ps(0.5);
    let iota = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
    let invv = _mm512_set1_ps(s.inv);
    let (wi_b, wi_c) = if s.swapped { (2usize, 1usize) } else { (1, 2) };
    let zv = [_mm512_set1_ps(s.z[0]), _mm512_set1_ps(s.z[1]), _mm512_set1_ps(s.z[2])];
    let mut zs = [0f32; 16];
    let mut x = run.x0 as i32;
    while x <= run.x1 as i32 {
        let n = ((run.x1 as i32 - x + 1).min(16)) as usize;
        let qx = _mm512_add_ps(_mm512_cvtepi32_ps(_mm512_add_epi32(_mm512_set1_epi32(x), iota)), half);
        let mut e = [_mm512_setzero_ps(); 3];
        for k in 0..3 {
            let d = _mm512_sub_ps(qx, ax[k]);
            let p2 = _mm512_mul_ps(dy[k], d);
            e[k] = _mm512_sub_ps(t1v[k], p2);
        }
        let mut bc = [_mm512_setzero_ps(); 3];
        bc[0] = _mm512_mul_ps(e[1], invv);
        bc[wi_b] = _mm512_mul_ps(e[2], invv);
        bc[wi_c] = _mm512_mul_ps(e[0], invv);
        // z = (z0·b0 + z1·b1) + z2·b2, the visit body's order
        let z = _mm512_add_ps(_mm512_add_ps(_mm512_mul_ps(zv[0], bc[0]), _mm512_mul_ps(zv[1], bc[1])), _mm512_mul_ps(zv[2], bc[2]));
        _mm512_storeu_ps(zs.as_mut_ptr(), z);
        g(x as u32, n, &zs);
        x += 16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster;

    /// Random triangles of every size: on each covered row, the run over the covered interval expands to exactly the
    /// per-pixel fragments the scalar visit body computes (`raster::triangle_clipped_masked`'s barycentrics, then
    /// `z0·b0 + z1·b1 + z2·b2`), pixel by pixel and bit by bit — in the scalar and the 16-wide form.
    #[test]
    fn a_run_expands_to_the_visit_bodys_fragments() {
        let mut seed = 0x5eed_1234_abcdu64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % 1_000_000) as f32 / 1_000_000.0 };
        let (w, h) = (256u32, 192u32);
        let mut runs_checked = 0usize;
        for k in 0..4000 {
            let scale = match k % 4 { 0 => 3.0, 1 => 20.0, 2 => 90.0, _ => 300.0 };
            let (cx, cy) = (rnd() * 300.0 - 20.0, rnd() * 230.0 - 20.0);
            let mut p = [[0.0f32; 2]; 3];
            for v in p.iter_mut() { *v = [cx + (rnd() - 0.5) * scale, cy + (rnd() - 0.5) * scale]; }
            if k % 7 == 0 { p[1][1] = p[0][1]; }
            if k % 13 == 0 { p[0] = [p[0][0].round() + 0.5, p[0][1].round() + 0.5]; }
            let z = [rnd() * 500.0, rnd() * 500.0, rnd() * 500.0];
            let Some(s) = RunSetup::of_projected(p, z) else { continue };
            // the scalar body's fragments per row
            let mut rows: std::collections::BTreeMap<u32, Vec<(u32, f32)>> = std::collections::BTreeMap::new();
            raster::triangle_clipped_masked(w, h, p, (0, 0, w as i32 - 1, h as i32 - 1), None, |x, y, bc| {
                let zz = z[0] * bc[0] + z[1] * bc[1] + z[2] * bc[2];
                rows.entry(y).or_default().push((x, zz));
            });
            for (y, frags) in rows {
                // the covered set on a row is an interval (each edge's inside set is a half-line in x)
                let (x0, x1) = (frags[0].0, frags[frags.len() - 1].0);
                assert_eq!(frags.len() as u32, x1 - x0 + 1, "row {y} of triangle {k} is not an interval");
                let run = Run { tri: k as u32, y: y as u16, x0: x0 as u16, x1: x1 as u16, bias: 0.0 };
                let mut got: Vec<(u32, f32)> = Vec::new();
                expand_run_scalar(&run, &s, |x, zz| got.push((x, zz)));
                assert_eq!(got.len(), frags.len());
                for (g, f) in got.iter().zip(frags.iter()) {
                    assert_eq!(g.0, f.0);
                    assert_eq!(g.1.to_bits(), f.1.to_bits(), "triangle {k} pixel ({}, {y}): run z {} vs body z {}", g.0, g.1, f.1);
                }
                let mut got16: Vec<(u32, f32)> = Vec::new();
                expand_run(&run, &s, |x0, n, zs| { for l in 0..n { got16.push((x0 + l as u32, zs[l])); } });
                assert_eq!(got16.len(), frags.len());
                for (g, f) in got16.iter().zip(frags.iter()) {
                    assert_eq!((g.0, g.1.to_bits()), (f.0, f.1.to_bits()), "16-wide: triangle {k} pixel ({}, {y})", g.0);
                }
                runs_checked += 1;
            }
        }
        assert!(runs_checked > 20000, "{runs_checked}");
    }
}
