//! THE BINNING'S PROJECTION SIXTEEN TRIANGLES AT A TIME (perf engineer 3): the sparse raster's binning projects
//! every culled triangle of every peel frame — 87 M per direction on the giant, three vertices each — to find
//! the cells its bounding box touches; per triangle it was ~150 instructions at IPC ≈ 1 (nine dot products,
//! the bounds, the culls). Here sixteen triangles' records are gathered into lanes (p0, e1, e2: nine floats
//! per triangle out of the 72-byte `WTri`) and projected with `PeelFrame::project`'s arithmetic per lane —
//! `p1 = p0 + e1`, `p2 = p0 + e2`, `dot(p, v) = (p.x·v.x + p.y·v.y) + p.z·v.z`, `(dot − s0)·scale`, `−dot(p, d)`
//! — the same IEEE single operations in the same order, no fused multiply-add, so every lane's nine results
//! are the scalar `project`'s to the bit (the test below checks it on random records and frames).

use crate::bvh::WTri;
use crate::peel::PeelFrame;

/// The projections of up to sixteen triangles: `x[v][l]`, `y[v][l]`, `z[v][l]` = vertex v (0 = p0, 1 = p0 + e1,
/// 2 = p0 + e2) of lane l.
#[derive(Clone, Copy)]
pub struct Proj16 {
    pub x: [[f32; 16]; 3],
    pub y: [[f32; 16]; 3],
    pub z: [[f32; 16]; 3],
}

impl Proj16 {
    pub const ZERO: Proj16 = Proj16 { x: [[0.0; 16]; 3], y: [[0.0; 16]; 3], z: [[0.0; 16]; 3] };
}

/// The scalar projection of one record's three vertices, exactly as the raster's band loop computes them.
#[inline]
pub fn project_one(t: &WTri, frame: &PeelFrame) -> ((f32, f32, f32), (f32, f32, f32), (f32, f32, f32)) {
    let p0 = t.p0;
    let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
    let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
    (frame.project(p0), frame.project(p1), frame.project(p2))
}

/// The projections of `tris[first..first + n]` (n ≤ 16) into `out`'s lanes 0..n — the 16-lane gather form on
/// the AVX-512 build, the scalar projection lane by lane otherwise.
#[inline]
pub fn project16(tris: &[WTri], first: usize, n: usize, frame: &PeelFrame, out: &mut Proj16) {
    debug_assert!(n <= 16 && first + n <= tris.len());
    #[cfg(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw"))]
    {
        // SAFETY: the build enables avx512f/bw for every function; the gathers read inside `tris`
        unsafe { project16_avx512(tris, first, n, frame, out) }
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx512f", target_feature = "avx512bw")))]
    {
        project16_scalar(tris, first, n, frame, out)
    }
}

pub fn project16_scalar(tris: &[WTri], first: usize, n: usize, frame: &PeelFrame, out: &mut Proj16) {
    for l in 0..n {
        let (a, b, c) = project_one(&tris[first + l], frame);
        out.x[0][l] = a.0; out.y[0][l] = a.1; out.z[0][l] = a.2;
        out.x[1][l] = b.0; out.y[1][l] = b.1; out.z[1][l] = b.2;
        out.x[2][l] = c.0; out.y[2][l] = c.1; out.z[2][l] = c.2;
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw,avx512dq,avx512vl")]
pub unsafe fn project16_avx512(tris: &[WTri], first: usize, n: usize, frame: &PeelFrame, out: &mut Proj16) {
    use std::arch::x86_64::*;
    let n = n.min(16);
    let lanes: __mmask16 = if n >= 16 { u16::MAX } else { ((1u32 << n) - 1) as u16 };
    // element (4-byte) offsets of lane l's record fields: (first + l)·size/4 + field/4
    let stride = (std::mem::size_of::<WTri>() / 4) as i32;
    let iota = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
    let base_idx = _mm512_mullo_epi32(_mm512_add_epi32(_mm512_set1_epi32(first as i32), iota), _mm512_set1_epi32(stride));
    let base = tris.as_ptr() as *const f32;
    let field = |off_bytes: usize| -> __m512 {
        let idx = _mm512_add_epi32(base_idx, _mm512_set1_epi32((off_bytes / 4) as i32));
        // lanes past n read lane 0's record (a valid address) and are ignored
        let idx = _mm512_mask_blend_epi32(lanes, _mm512_set1_epi32((first * stride as usize + off_bytes / 4) as i32), idx);
        _mm512_i32gather_ps::<4>(idx, base)
    };
    let (op0, oe1, oe2) = (std::mem::offset_of!(WTri, p0), std::mem::offset_of!(WTri, e1), std::mem::offset_of!(WTri, e2));
    let p0 = [field(op0), field(op0 + 4), field(op0 + 8)];
    let e1 = [field(oe1), field(oe1 + 4), field(oe1 + 8)];
    let e2 = [field(oe2), field(oe2 + 4), field(oe2 + 8)];
    let p1 = [_mm512_add_ps(p0[0], e1[0]), _mm512_add_ps(p0[1], e1[1]), _mm512_add_ps(p0[2], e1[2])];
    let p2 = [_mm512_add_ps(p0[0], e2[0]), _mm512_add_ps(p0[1], e2[1]), _mm512_add_ps(p0[2], e2[2])];
    // dot(p, v) = (p.x·v.x + p.y·v.y) + p.z·v.z
    let dot = |p: &[__m512; 3], v: [f32; 3]| -> __m512 {
        let a = _mm512_mul_ps(p[0], _mm512_set1_ps(v[0]));
        let b = _mm512_mul_ps(p[1], _mm512_set1_ps(v[1]));
        let c = _mm512_mul_ps(p[2], _mm512_set1_ps(v[2]));
        _mm512_add_ps(_mm512_add_ps(a, b), c)
    };
    let (s0, t0, scale, scale_y) = (_mm512_set1_ps(frame.s0), _mm512_set1_ps(frame.t0), _mm512_set1_ps(frame.scale), _mm512_set1_ps(frame.scale_y));
    let zero = _mm512_setzero_ps();
    for (v, p) in [p0, p1, p2].iter().enumerate() {
        let x = _mm512_mul_ps(_mm512_sub_ps(dot(p, frame.r), s0), scale);
        let y = _mm512_mul_ps(_mm512_sub_ps(dot(p, frame.u), t0), scale_y);
        // the scalar `-dot(p, d)`: a sign flip, 0 − dot would differ for a zero dot (−0 vs +0)
        let z = _mm512_xor_ps(dot(p, frame.d), _mm512_set1_ps(-0.0));
        let _ = zero;
        _mm512_storeu_ps(out.x[v].as_mut_ptr(), x);
        _mm512_storeu_ps(out.y[v].as_mut_ptr(), y);
        _mm512_storeu_ps(out.z[v].as_mut_ptr(), z);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 16-lane projection equals the scalar `PeelFrame::project` of every vertex, bit for bit, on random
    /// records (large and tiny, near and far) and random frames (world-sized and tile-sized).
    #[test]
    fn sixteen_lanes_are_the_scalar_projection() {
        let mut seed = 0x8f1a_2b3c_4d5e_6f70u64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % 2_000_000) as f32 / 1_000_000.0 - 1.0 };
        let mut tris: Vec<WTri> = Vec::new();
        for k in 0..4096 {
            let s = match k % 4 { 0 => 0.3, 1 => 3.0, 2 => 40.0, _ => 500.0 };
            let p0 = [rnd() * 4000.0, rnd() * 600.0, rnd() * 4000.0];
            tris.push(WTri { p0, e1: [rnd() * s, rnd() * s, rnd() * s], e2: [rnd() * s, rnd() * s, rnd() * s], inst: k as u32, tri: k as u32, alpha: u16::MAX, uv0: [[0.0; 2]; 3] });
        }
        for f in 0..8 {
            let d = crate::geometry::norm([rnd(), rnd().abs() + 0.2, rnd()]);
            let half = if f % 2 == 0 { 4200.0 } else { 700.0 + rnd().abs() * 500.0 };
            let frame = PeelFrame::new(d, [-half, -200.0, -half], [half, 900.0, half], if f % 2 == 0 { 4096 } else { 2048 });
            let mut out = Proj16::ZERO;
            let mut first = 0usize;
            while first < tris.len() {
                let n = (tris.len() - first).min(if f == 3 { 5 } else { 16 });
                project16(&tris, first, n, &frame, &mut out);
                for l in 0..n {
                    let (a, b, c) = project_one(&tris[first + l], &frame);
                    for (v, sc) in [a, b, c].iter().enumerate() {
                        assert_eq!(out.x[v][l].to_bits(), sc.0.to_bits(), "frame {f} triangle {} vertex {v} x", first + l);
                        assert_eq!(out.y[v][l].to_bits(), sc.1.to_bits(), "frame {f} triangle {} vertex {v} y", first + l);
                        assert_eq!(out.z[v][l].to_bits(), sc.2.to_bits(), "frame {f} triangle {} vertex {v} z", first + l);
                    }
                }
                first += n;
            }
        }
    }
}
