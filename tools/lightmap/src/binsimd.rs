//! THE BINNING 16 WIDE (perf engineer 5, PERF 5.3): the per-triangle projection and candidate-centre test of the
//! peel raster's binning (`rows_of` in `build_abuffer_sparse_ranges`) over a block of 16 triangles in AVX-512
//! lanes, from the BVH's blocked position table (`bvh::TriSoa`, 36 bytes per triangle) instead of the 72-byte
//! records. Bit-for-bit the scalar decisions: every lane performs the scalar path's f32 operations in the scalar
//! path's order — `p1 = p0 + e1`, `dot = (a·b + c·d) + e·f`, `(dot − s0) · scale`, the NaN-ignoring `min`/`max`
//! of `f32::min`/`max`, `ceil(miny − 0.5)` / `floor(maxy − 0.5)` — so the projected coordinates handed to the
//! exact centre cull and the row ranges handed to the bands are the ones the scalar code computes.
//! `LMTOOL_BIN_SIMD=0` keeps the scalar binning; `LMTOOL_BIN_SIMD_CHECK=1` runs both and compares every decision.

use crate::bvh::SoaBlock;
use crate::peel::PeelFrame;

/// One block's results: the surviving lanes (a triangle in the depth range with a candidate centre in both axes
/// inside the clip), their projected vertices (x0, y0, x1, y1, x2, y2 — the scalar path's f32 values) and the
/// ceil/floor'd extents before the clip (`cy0 = ceil(miny − 0.5)`, `fy1 = floor(maxy − 0.5)`, same for x) as f32.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct Block16 {
    pub xy: [[f32; 16]; 6],
    pub cy0: [f32; 16],
    pub fy1: [f32; 16],
    pub cx0: [f32; 16],
    pub fx1: [f32; 16],
    pub mask: u16,
}

impl Block16 {
    pub const ZERO: Block16 = Block16 { xy: [[0.0; 16]; 6], cy0: [0.0; 16], fy1: [0.0; 16], cx0: [0.0; 16], fx1: [0.0; 16], mask: 0 };
}

/// Is the 16-wide path available on this machine (AVX-512F) and not switched off (LMTOOL_BIN_SIMD=0)?
pub fn available() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        let on = std::env::var("LMTOOL_BIN_SIMD").map(|v| v != "0").unwrap_or(true);
        on && std::arch::is_x86_feature_detected!("avx512f")
    })
}

/// LMTOOL_BIN_SIMD_CHECK=1: the scalar decisions computed alongside and compared (a panic on the first difference).
pub fn check_on() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_BIN_SIMD_CHECK").map(|v| v == "1").unwrap_or(false))
}

/// The frame's constants as the kernel wants them.
#[derive(Clone, Copy)]
pub struct FrameK {
    pub r: [f32; 3],
    pub u: [f32; 3],
    pub d: [f32; 3],
    pub s0: f32,
    pub t0: f32,
    pub scale: f32,
    pub scale_y: f32,
    pub zmin: f32,
    pub zmax: f32,
    /// The clip rectangle (x0, y0, x1, y1) as f32.
    pub clip: [f32; 4],
}

impl FrameK {
    pub fn new(frame: &PeelFrame, zmin: f32, zmax: f32, clip: (i32, i32, i32, i32)) -> FrameK {
        FrameK { r: frame.r, u: frame.u, d: frame.d, s0: frame.s0, t0: frame.t0, scale: frame.scale, scale_y: frame.scale_y, zmin, zmax, clip: [clip.0 as f32, clip.1 as f32, clip.2 as f32, clip.3 as f32] }
    }
}

/// The block's 16 triangles projected and tested; `lanes` masks the valid triangles of the block (the last block
/// of a range is partial). Safe wrapper: `available()` must have returned true.
#[inline]
pub fn rows_of_16(block: &SoaBlock, k: &FrameK, lanes: u16, out: &mut Block16) {
    debug_assert!(available());
    // SAFETY: `available()` checked avx512f
    unsafe { rows_of_16_avx512(block, k, lanes, out) }
}

#[target_feature(enable = "avx512f")]
unsafe fn rows_of_16_avx512(block: &SoaBlock, k: &FrameK, lanes: u16, out: &mut Block16) {
    use std::arch::x86_64::*;
    let ld = |i: usize| -> __m512 { _mm512_load_ps(block.v[i].as_ptr()) };
    let bc = |v: f32| -> __m512 { _mm512_set1_ps(v) };
    // f32::min / f32::max: the other operand when one is NaN — `min_ps(a, b)` hands back b when either is NaN, so
    // where b is NaN take a
    let fmin = |a: __m512, b: __m512| -> __m512 { let m = _mm512_min_ps(a, b); let bn = _mm512_cmp_ps_mask::<_CMP_UNORD_Q>(b, b); _mm512_mask_blend_ps(bn, m, a) };
    let fmax = |a: __m512, b: __m512| -> __m512 { let m = _mm512_max_ps(a, b); let bn = _mm512_cmp_ps_mask::<_CMP_UNORD_Q>(b, b); _mm512_mask_blend_ps(bn, m, a) };
    // dot(p, v) = (p.x·v.x + p.y·v.y) + p.z·v.z, the scalar order
    let dot = |px: __m512, py: __m512, pz: __m512, v: [f32; 3]| -> __m512 {
        let a = _mm512_mul_ps(px, bc(v[0]));
        let b = _mm512_mul_ps(py, bc(v[1]));
        let s = _mm512_add_ps(a, b);
        let c = _mm512_mul_ps(pz, bc(v[2]));
        _mm512_add_ps(s, c)
    };
    let (p0x, p0y, p0z) = (ld(0), ld(1), ld(2));
    let (p1x, p1y, p1z) = (_mm512_add_ps(p0x, ld(3)), _mm512_add_ps(p0y, ld(4)), _mm512_add_ps(p0z, ld(5)));
    let (p2x, p2y, p2z) = (_mm512_add_ps(p0x, ld(6)), _mm512_add_ps(p0y, ld(7)), _mm512_add_ps(p0z, ld(8)));
    let project = |px: __m512, py: __m512, pz: __m512| -> (__m512, __m512, __m512) {
        let x = _mm512_mul_ps(_mm512_sub_ps(dot(px, py, pz, k.r), bc(k.s0)), bc(k.scale));
        let y = _mm512_mul_ps(_mm512_sub_ps(dot(px, py, pz, k.u), bc(k.t0)), bc(k.scale_y));
        // −dot(p, d): the negation is exact
        let z = _mm512_sub_ps(_mm512_setzero_ps(), dot(px, py, pz, k.d));
        (x, y, z)
    };
    let (x0, y0, z0) = project(p0x, p0y, p0z);
    let (x1, y1, z1) = project(p1x, p1y, p1z);
    let (x2, y2, z2) = project(p2x, p2y, p2z);
    // the depth range: z.min ≥ zmax or z.max < zmin → out
    let zmn = fmin(fmin(z0, z1), z2);
    let zmx = fmax(fmax(z0, z1), z2);
    let z_out = _mm512_cmp_ps_mask::<_CMP_GE_OQ>(zmn, bc(k.zmax)) | _mm512_cmp_ps_mask::<_CMP_LT_OQ>(zmx, bc(k.zmin));
    // (the scalar `>=` / `<` with a NaN operand are false: a NaN depth is "in range" there as here — a NaN z only
    // arises from a NaN position, which the finite tests below reject anyway)
    let miny = fmin(fmin(y0, y1), y2);
    let maxy = fmax(fmax(y0, y1), y2);
    let minx = fmin(fmin(x0, x1), x2);
    let maxx = fmax(fmax(x0, x1), x2);
    // finite: |v| < +inf and not NaN
    let inf = bc(f32::INFINITY);
    let finite = |v: __m512| -> __mmask16 { _mm512_cmp_ps_mask::<_CMP_LT_OQ>(_mm512_abs_ps(v), inf) };
    let fin = finite(miny) & finite(maxy) & finite(minx) & finite(maxx);
    let half = bc(0.5);
    let cy0 = _mm512_roundscale_ps::<{ _MM_FROUND_TO_POS_INF | _MM_FROUND_NO_EXC }>(_mm512_sub_ps(miny, half));
    let fy1 = _mm512_roundscale_ps::<{ _MM_FROUND_TO_NEG_INF | _MM_FROUND_NO_EXC }>(_mm512_sub_ps(maxy, half));
    let cx0 = _mm512_roundscale_ps::<{ _MM_FROUND_TO_POS_INF | _MM_FROUND_NO_EXC }>(_mm512_sub_ps(minx, half));
    let fx1 = _mm512_roundscale_ps::<{ _MM_FROUND_TO_NEG_INF | _MM_FROUND_NO_EXC }>(_mm512_sub_ps(maxx, half));
    // clipped: ry0 = max(cy0, clip.y0), ry1 = min(fy1, clip.y1); empty when ry0 > ry1 (exact on integer-valued f32)
    let ry0 = _mm512_max_ps(cy0, bc(k.clip[1]));
    let ry1 = _mm512_min_ps(fy1, bc(k.clip[3]));
    let rx0 = _mm512_max_ps(cx0, bc(k.clip[0]));
    let rx1 = _mm512_min_ps(fx1, bc(k.clip[2]));
    let rows_ok = _mm512_cmp_ps_mask::<_CMP_LE_OQ>(ry0, ry1);
    let cols_ok = _mm512_cmp_ps_mask::<_CMP_LE_OQ>(rx0, rx1);
    let mask = lanes & !z_out & fin & rows_ok & cols_ok;
    out.mask = mask;
    if mask == 0 {
        return;
    }
    _mm512_store_ps(out.xy[0].as_mut_ptr(), x0);
    _mm512_store_ps(out.xy[1].as_mut_ptr(), y0);
    _mm512_store_ps(out.xy[2].as_mut_ptr(), x1);
    _mm512_store_ps(out.xy[3].as_mut_ptr(), y1);
    _mm512_store_ps(out.xy[4].as_mut_ptr(), x2);
    _mm512_store_ps(out.xy[5].as_mut_ptr(), y2);
    _mm512_store_ps(out.cy0.as_mut_ptr(), cy0);
    _mm512_store_ps(out.fy1.as_mut_ptr(), fy1);
    _mm512_store_ps(out.cx0.as_mut_ptr(), cx0);
    _mm512_store_ps(out.fx1.as_mut_ptr(), fx1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bvh::{TriSoa, WTri};

    /// The 16-wide decisions and coordinates equal the scalar path's on random triangles of every size, position and
    /// depth — including ones straddling the clip and the depth range and sub-pixel ones.
    #[test]
    fn the_lanes_agree_with_the_scalar_projection() {
        if !available() { eprintln!("no avx512f: skipped"); return; }
        let mut seed = 0x1234_5678_9abc_def0u64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % 1_000_000) as f32 / 1_000_000.0 };
        let frame = PeelFrame::new([0.3, -0.8, 0.5], [-100.0, 0.0, -100.0], [900.0, 200.0, 900.0], 4096);
        let (zmin, zmax) = (frame.z_from_z01(0.0), frame.z_from_z01(1.0));
        let clip = (1i32, 1i32, 4094i32, 4094i32);
        let mut tris: Vec<WTri> = Vec::new();
        for i in 0..5000 {
            let scale = match i % 5 { 0 => 0.05, 1 => 0.3, 2 => 2.0, 3 => 40.0, _ => 400.0 };
            let p = [rnd() * 1100.0 - 150.0, rnd() * 260.0 - 30.0, rnd() * 1100.0 - 150.0];
            tris.push(WTri { p0: p, e1: [(rnd() - 0.5) * scale, (rnd() - 0.5) * scale, (rnd() - 0.5) * scale], e2: [(rnd() - 0.5) * scale, (rnd() - 0.5) * scale, (rnd() - 0.5) * scale], inst: 0, tri: i, alpha: u16::MAX, uv0: [[0.0; 2]; 3] });
        }
        let soa = TriSoa::build(&tris);
        let k = FrameK::new(&frame, zmin, zmax, clip);
        let mut out = Block16::ZERO;
        let (mut survivors, mut rejected) = (0, 0);
        for (bi, b) in soa.blocks.iter().enumerate() {
            let n = (tris.len() - bi * 16).min(16);
            let lanes: u16 = if n >= 16 { 0xffff } else { (1u16 << n) - 1 };
            rows_of_16(b, &k, lanes, &mut out);
            for l in 0..n {
                let t = &tris[bi * 16 + l];
                let p0 = t.p0;
                let p1 = [p0[0] + t.e1[0], p0[1] + t.e1[1], p0[2] + t.e1[2]];
                let p2 = [p0[0] + t.e2[0], p0[1] + t.e2[1], p0[2] + t.e2[2]];
                let (x0, y0, z0) = frame.project(p0);
                let (x1, y1, z1) = frame.project(p1);
                let (x2, y2, z2) = frame.project(p2);
                let scalar: Option<(i32, i32)> = (|| {
                    if z0.min(z1).min(z2) >= zmax || z0.max(z1).max(z2) < zmin { return None; }
                    let (miny, maxy) = (y0.min(y1).min(y2), y0.max(y1).max(y2));
                    if !(miny.is_finite() && maxy.is_finite()) { return None; }
                    let ry0 = ((miny - 0.5).ceil() as i64).max(clip.1 as i64) as i32;
                    let ry1 = ((maxy - 0.5).floor() as i64).min(clip.3 as i64) as i32;
                    if ry0 > ry1 { return None; }
                    let (minx, maxx) = (x0.min(x1).min(x2), x0.max(x1).max(x2));
                    if !(minx.is_finite() && maxx.is_finite()) { return None; }
                    let rx0 = ((minx - 0.5).ceil() as i64).max(clip.0 as i64);
                    let rx1 = ((maxx - 0.5).floor() as i64).min(clip.2 as i64);
                    if rx0 > rx1 { return None; }
                    Some((ry0, ry1))
                })();
                let lane_on = (out.mask >> l) & 1 == 1;
                assert_eq!(lane_on, scalar.is_some(), "triangle {}: lane {lane_on} scalar {scalar:?}", bi * 16 + l);
                if lane_on {
                    survivors += 1;
                    assert_eq!([out.xy[0][l], out.xy[1][l], out.xy[2][l], out.xy[3][l], out.xy[4][l], out.xy[5][l]].map(f32::to_bits), [x0, y0, x1, y1, x2, y2].map(f32::to_bits), "the projected coordinates of triangle {}", bi * 16 + l);
                    let ry0 = ((out.cy0[l] as i64).max(clip.1 as i64)) as i32;
                    let ry1 = ((out.fy1[l] as i64).min(clip.3 as i64)) as i32;
                    assert_eq!(Some((ry0, ry1)), scalar);
                } else {
                    rejected += 1;
                }
            }
        }
        assert!(survivors > 500 && rejected > 500, "{survivors} / {rejected}");
    }
}

/// THE MICRO COVERAGE, 16 TRIANGLES AT A TIME: for the block's surviving lanes whose clipped candidate box holds at
/// most FOUR centres (w·h ≤ 4 — the giant's leaf canopy: 60–70 % of the pairs), the raster's inside predicate at
/// every candidate, one candidate per pass over all sixteen triangles (the scalar `covers_box` did one triangle's
/// ≤ 16 candidates per pass): per lane the orientation (`area = edge(p0, p1, p2)`, b and c swapped when negative),
/// the top-left flags, and per candidate `e = (b−a).x·(q−a).y − (b−a).y·(q−a).x` — the scalar operations in the
/// scalar order — so lane l's bit p equals `CoverTest::new(p_l).covers_box(rx0, ry0, rx1, ry1)`'s bit p (row-major:
/// p = (y − ry0)·w + (x − rx0)). `small` marks the lanes handled (w·h ≤ 4); `cov[l]` their masks; the clipped
/// ranges come from the block's ceil/floor'd extents clamped to the clip exactly as the scalar path clamps them.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct Micro16 {
    pub rx0: [i32; 16],
    pub ry0: [i32; 16],
    pub rx1: [i32; 16],
    pub ry1: [i32; 16],
    pub cov: [u8; 16],
    pub small: u16,
}

impl Micro16 {
    pub const ZERO: Micro16 = Micro16 { rx0: [0; 16], ry0: [0; 16], rx1: [0; 16], ry1: [0; 16], cov: [0; 16], small: 0 };
}

#[inline]
pub fn micro_16(blk: &Block16, k: &FrameK, out: &mut Micro16) {
    debug_assert!(available());
    // SAFETY: avx512f checked by `available()`
    unsafe { micro_16_avx512(blk, k, out) }
}

#[target_feature(enable = "avx512f")]
unsafe fn micro_16_avx512(blk: &Block16, k: &FrameK, out: &mut Micro16) {
    use std::arch::x86_64::*;
    let ld = |v: &[f32; 16]| -> __m512 { _mm512_load_ps(v.as_ptr()) };
    let bc = |v: f32| -> __m512 { _mm512_set1_ps(v) };
    // the clipped candidate ranges: max/min with the clip on the integer-valued extents (exact), then to i32
    let rx0f = _mm512_max_ps(ld(&blk.cx0), bc(k.clip[0]));
    let rx1f = _mm512_min_ps(ld(&blk.fx1), bc(k.clip[2]));
    let ry0f = _mm512_max_ps(ld(&blk.cy0), bc(k.clip[1]));
    let ry1f = _mm512_min_ps(ld(&blk.fy1), bc(k.clip[3]));
    let rx0 = _mm512_cvtps_epi32(rx0f);
    let rx1 = _mm512_cvtps_epi32(rx1f);
    let ry0 = _mm512_cvtps_epi32(ry0f);
    let ry1 = _mm512_cvtps_epi32(ry1f);
    _mm512_store_si512(out.rx0.as_mut_ptr() as *mut _, rx0);
    _mm512_store_si512(out.rx1.as_mut_ptr() as *mut _, rx1);
    _mm512_store_si512(out.ry0.as_mut_ptr() as *mut _, ry0);
    _mm512_store_si512(out.ry1.as_mut_ptr() as *mut _, ry1);
    let one = _mm512_set1_epi32(1);
    let w = _mm512_add_epi32(_mm512_sub_epi32(rx1, rx0), one);
    let h = _mm512_add_epi32(_mm512_sub_epi32(ry1, ry0), one);
    let n = _mm512_mullo_epi32(w, h);
    let small: __mmask16 = blk.mask & _mm512_cmple_epi32_mask(n, _mm512_set1_epi32(4)) & _mm512_cmpge_epi32_mask(n, one);
    out.small = small;
    out.cov = [0; 16];
    if small == 0 {
        return;
    }
    // the orientation per lane: area = edge(p0, p1, p2) = (x1−x0)·(y2−y0) − (y1−y0)·(x2−x0)
    let (x0, y0, x1, y1, x2, y2) = (ld(&blk.xy[0]), ld(&blk.xy[1]), ld(&blk.xy[2]), ld(&blk.xy[3]), ld(&blk.xy[4]), ld(&blk.xy[5]));
    let area = _mm512_sub_ps(_mm512_mul_ps(_mm512_sub_ps(x1, x0), _mm512_sub_ps(y2, y0)), _mm512_mul_ps(_mm512_sub_ps(y1, y0), _mm512_sub_ps(x2, x0)));
    let zero = _mm512_setzero_ps();
    // a degenerate triangle (area 0 or not finite) covers nothing: its lanes report an empty mask
    let degenerate = _mm512_cmp_ps_mask::<_CMP_EQ_OQ>(area, zero) | _mm512_cmp_ps_mask::<_CMP_UNORD_Q>(area, area) | _mm512_cmp_ps_mask::<_CMP_EQ_OQ>(_mm512_abs_ps(area), bc(f32::INFINITY));
    let swapped = _mm512_cmp_ps_mask::<_CMP_LT_OQ>(area, zero);
    // (a, b, c) = (p0, p1, p2) or (p0, p2, p1)
    let (ax, ay) = (x0, y0);
    let bx = _mm512_mask_blend_ps(swapped, x1, x2);
    let by = _mm512_mask_blend_ps(swapped, y1, y2);
    let cx = _mm512_mask_blend_ps(swapped, x2, x1);
    let cy = _mm512_mask_blend_ps(swapped, y2, y1);
    // the three edges a→b, b→c, c→a: (b−a).x, (b−a).y and the top-left flag dy < 0 || (dy == 0 && dx > 0)
    let edges: [(__m512, __m512, __m512, __m512); 3] = [(ax, ay, bx, by), (bx, by, cx, cy), (cx, cy, ax, ay)];
    let mut dxs = [zero; 3];
    let mut dys = [zero; 3];
    let mut tls: [__mmask16; 3] = [0; 3];
    for e in 0..3 {
        let (eax, eay, ebx, eby) = edges[e];
        dxs[e] = _mm512_sub_ps(ebx, eax);
        dys[e] = _mm512_sub_ps(eby, eay);
        tls[e] = _mm512_cmp_ps_mask::<_CMP_LT_OQ>(dys[e], zero) | (_mm512_cmp_ps_mask::<_CMP_EQ_OQ>(dys[e], zero) & _mm512_cmp_ps_mask::<_CMP_GT_OQ>(dxs[e], zero));
    }
    let half = bc(0.5);
    let mut cov: [u8; 16] = [0; 16];
    // candidate p (row-major in the lane's box): i = p mod w, j = p div w, for w ∈ {1, 2, 3, 4}: p < w → (p, 0);
    // else p − w < w → (p − w, 1); else (p − 2w, 2) — up to 4 candidates and 4 rows for w = 1 (p = j)
    for p in 0..4i32 {
        let pv = _mm512_set1_epi32(p);
        let live = small & _mm512_cmplt_epi32_mask(pv, n);
        if live == 0 { continue; }
        // j = p / w, i = p − j·w via compares: j = (p ≥ w) + (p ≥ 2w) + (p ≥ 3w)
        let w2 = _mm512_add_epi32(w, w);
        let w3 = _mm512_add_epi32(w2, w);
        let j = _mm512_add_epi32(_mm512_add_epi32(_mm512_maskz_mov_epi32(_mm512_cmpge_epi32_mask(pv, w), one), _mm512_maskz_mov_epi32(_mm512_cmpge_epi32_mask(pv, w2), one)), _mm512_maskz_mov_epi32(_mm512_cmpge_epi32_mask(pv, w3), one));
        let i = _mm512_sub_epi32(pv, _mm512_mullo_epi32(j, w));
        // q = (x as f32 + 0.5, y as f32 + 0.5)
        let qx = _mm512_add_ps(_mm512_cvtepi32_ps(_mm512_add_epi32(rx0, i)), half);
        let qy = _mm512_add_ps(_mm512_cvtepi32_ps(_mm512_add_epi32(ry0, j)), half);
        let mut inside: __mmask16 = live & !degenerate;
        for e in 0..3 {
            let (eax, eay, _, _) = edges[e];
            let t1 = _mm512_mul_ps(dxs[e], _mm512_sub_ps(qy, eay));
            let t2 = _mm512_mul_ps(dys[e], _mm512_sub_ps(qx, eax));
            let ev = _mm512_sub_ps(t1, t2);
            let gt = _mm512_cmp_ps_mask::<_CMP_GT_OQ>(ev, zero);
            let on = _mm512_cmp_ps_mask::<_CMP_EQ_OQ>(ev, zero) & tls[e];
            inside &= gt | on;
        }
        let mut m = inside as u32;
        while m != 0 {
            let l = m.trailing_zeros() as usize;
            m &= m - 1;
            cov[l] |= 1 << p;
        }
    }
    out.cov = cov;
}

#[cfg(test)]
mod micro_tests {
    use super::*;
    use crate::bvh::{TriSoa, WTri};
    use crate::raster::CoverTest;

    /// The 16-triangle micro coverage equals `CoverTest::covers_box` lane by lane, and the clipped ranges equal the
    /// scalar path's, over random small triangles (sub-pixel to a few pixels) around a frame.
    #[test]
    fn the_micro_lanes_agree_with_covers_box() {
        if !available() { return; }
        let mut seed = 0xfeed_f00d_1234u64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % 1_000_000) as f32 / 1_000_000.0 };
        let frame = PeelFrame::new([0.2, -0.9, 0.4], [0.0, 0.0, 0.0], [1000.0, 200.0, 1000.0], 2048);
        let (zmin, zmax) = (frame.z_from_z01(0.0), frame.z_from_z01(1.0));
        let clip = (1i32, 1i32, 2046i32, 2046i32);
        let mut tris: Vec<WTri> = Vec::new();
        for i in 0..8000 {
            let scale = match i % 3 { 0 => 0.3, 1 => 0.8, _ => 1.6 };
            let p = [rnd() * 1000.0, rnd() * 200.0, rnd() * 1000.0];
            tris.push(WTri { p0: p, e1: [(rnd() - 0.5) * scale, (rnd() - 0.5) * scale, (rnd() - 0.5) * scale], e2: [(rnd() - 0.5) * scale, (rnd() - 0.5) * scale, (rnd() - 0.5) * scale], inst: 0, tri: i, alpha: u16::MAX, uv0: [[0.0; 2]; 3] });
        }
        let soa = TriSoa::build(&tris);
        let k = FrameK::new(&frame, zmin, zmax, clip);
        let mut blk = Block16::ZERO;
        let mut mic = Micro16::ZERO;
        let (mut small, mut covered) = (0, 0);
        for (bi, b) in soa.blocks.iter().enumerate() {
            let n = (tris.len() - bi * 16).min(16);
            let lanes: u16 = if n >= 16 { 0xffff } else { (1u16 << n) - 1 };
            rows_of_16(b, &k, lanes, &mut blk);
            micro_16(&blk, &k, &mut mic);
            for l in 0..n {
                if (blk.mask >> l) & 1 == 0 { continue; }
                let ry0 = ((blk.cy0[l] as i64).max(clip.1 as i64)) as i32;
                let ry1 = ((blk.fy1[l] as i64).min(clip.3 as i64)) as i32;
                let rx0 = ((blk.cx0[l] as i64).max(clip.0 as i64)) as i32;
                let rx1 = ((blk.fx1[l] as i64).min(clip.2 as i64)) as i32;
                assert_eq!((mic.rx0[l], mic.ry0[l], mic.rx1[l], mic.ry1[l]), (rx0, ry0, rx1, ry1), "ranges of triangle {}", bi * 16 + l);
                let (w, h) = (rx1 - rx0 + 1, ry1 - ry0 + 1);
                let is_small = w * h <= 4;
                assert_eq!((mic.small >> l) & 1 == 1, is_small, "small flag of triangle {}", bi * 16 + l);
                if is_small {
                    small += 1;
                    let p = [[blk.xy[0][l], blk.xy[1][l]], [blk.xy[2][l], blk.xy[3][l]], [blk.xy[4][l], blk.xy[5][l]]];
                    let expect = match CoverTest::new(p) { Some(c) => c.covers_box(rx0, ry0, rx1, ry1) as u8, None => 0 };
                    assert_eq!(mic.cov[l], expect, "coverage of triangle {} box {w}×{h}", bi * 16 + l);
                    if expect != 0 { covered += 1; }
                }
            }
        }
        assert!(small > 1000 && covered > 300, "{small} / {covered}");
    }
}
