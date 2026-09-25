//! The lightmapper's GPU finalisation, TRANSCRIBED from the captured shaders (passcap/pwc-day, frame 74490,
//! RenderDoc capture #5 of the editor's ComputeShadows, 2026-09-25): the per-image HDR max reduction and
//! `LmCompress_HBasis_YCbCr4` (CS 23025) that turns the four 2048² RGBA16F H-basis coefficient images into
//! the three stored R8G8B8A8_UNORM textures — Y4 (2048²: the luma of coefficient 0..3 in R,G,B,A), Cb4 and
//! Cr4 (1024²: the luma-weighted 2×2-average chroma of each coefficient).
//!
//! Every function here follows its DXBC instruction by instruction (same operation order, f32 arithmetic);
//! constants are the bytecode's literals; nothing is inferred. Shader ids are the capture's RenderDoc ids
//! (frame 74490): the disassembly is banked under passcap/pwc-day/shaders-frame74490/.
//!
//! The chain after the last accumulate (eid 34598) and before this encode:
//!   PS 25113 (4 draws, cb DebugShowOverlap = 0) rotates the MRTs: 24858 ← 24752, 24752 ← 24749, 24749 ← 24852,
//!   24852 ← 24855 (image k = C_k now lives in [24858, 24752, 24749, 24852][k]);
//!   PS 1109 (4 draws, cb ScaleSrc (2, 2, 2, 0), blend One/One into cleared targets) = rgb × 2;
//!   PS 1034 (4 draws, write mask RGB) copies them back, keeping each image's ALPHA (the direction count);
//!   PS 1332 × 8 per image = the gutter dilation (transcribed separately);
//!   PS 1317 + PS 1297 chain + CS 1495 = `maxhdr_hbasis` below;
//!   CS 23025 = `encode_ycbcr4` below; then 6 CopyResource to staging (the CPU readback).

use crate::gpufmt::{quantise_f16, Rounding};
use crate::passdiff::Buf;

/// The 4×4 → 512² → 128² → 32² → 8² → 1 max-reduction of |R|,|G|,|B| over one coefficient image
/// (PS 1317: gather4 of the four 2×2 quads of a 4×4 block, `max(abs, abs)` per channel, then the channel max;
/// PS 1297: the same without abs, three times; CS 1495: the 8×8 → 1 groupshared max into buffer[iOut]).
/// Every intermediate lives in an R16_FLOAT target, so the result is the f16 (RTNE) rounding of the plain max;
/// a max commutes with a monotonic rounding, so one rounding at the end is the same value.
pub fn maxhdr_hbasis(img: &Buf) -> f32 {
    let mut m = 0.0f32;
    for y in 0..img.h {
        for x in 0..img.w {
            for c in 0..3 {
                let v = img.get(x, y, c).abs();
                // D3D `max` returns the non-NaN operand; the images carry no NaN, keep the plain max
                if v > m {
                    m = v;
                }
            }
        }
    }
    quantise_f16(m, Rounding::NearestEven)
}

/// FLOAT → UNORM8 as the UAV store does it: clamp to [0, 1], scale by 255, round to nearest (ties to even —
/// D3D11 3.2.3.1 allows ±0.6 ULP; the capture decides which rounding the driver used, see `encode_check`).
#[inline]
pub fn unorm8(v: f32, mode: UnormRounding) -> u8 {
    let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) } * 255.0;
    let r = match mode {
        UnormRounding::NearestEven => v.round_ties_even(),
        UnormRounding::HalfUp => (v + 0.5).floor(),
        UnormRounding::Truncate => v.floor(),
        UnormRounding::Trunc12 => {
            // what the capture's byte thresholds show (WhiteStick's GPU, 6 boundaries checked to ±0.0003/255):
            // the value is first truncated to a 12-bit fixed-point fraction, then rounded to 8 bits with the
            // tie going down: q = floor(v·4096); byte = (q·255 + 2047) >> 12
            let q = ((v / 255.0) * 4096.0).floor().clamp(0.0, 4096.0) as u32;
            return ((q * 255 + 2047) >> 12).min(255) as u8;
        }
    };
    r.clamp(0.0, 255.0) as u8
}

/// How the UAV typed store converts float → UNORM8.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnormRounding {
    NearestEven,
    HalfUp,
    Truncate,
    /// truncate to 12 fractional bits, then round to 8 (ties down) — the captured store's behaviour
    Trunc12,
}

#[inline]
fn dp4(a: [f32; 4], b: [f32; 4], fma: bool) -> f32 {
    // dp4: the D3D reference sums the four products left to right
    if fma {
        let mut s = a[0] * b[0];
        s = a[1].mul_add(b[1], s);
        s = a[2].mul_add(b[2], s);
        a[3].mul_add(b[3], s)
    } else {
        ((a[0] * b[0] + a[1] * b[1]) + a[2] * b[2]) + a[3] * b[3]
    }
}

#[inline]
fn mad(a: f32, b: f32, c: f32, fma: bool) -> f32 {
    if fma { a.mul_add(b, c) } else { a * b + c }
}

const K_Y: [f32; 4] = [0.256788, 0.504129, 0.097906, 0.062745];
const K_CB: [f32; 4] = [-0.148224, -0.290992, 0.439216, 0.501961];
const K_CR: [f32; 4] = [0.439216, -0.367788, -0.071427, 0.501961];

/// Arithmetic options that the bytecode leaves to the implementation (fused mad/dp4, the UNORM rounding).
#[derive(Clone, Copy, Debug)]
pub struct EncodeOpts {
    pub fma: bool,
    pub unorm: UnormRounding,
    /// The typed UAV store converts the f32 through an f16 (RTNE) before the UNORM8 conversion — what the
    /// capture's thresholds show (errors up to half an f16 ULP of the value, sharp per byte).
    pub f16_store: bool,
    /// The encoded colour c (sqrt / signed sqrt, before the YCbCr matrix) rounded to f16 first.
    pub f16_c: bool,
}

impl Default for EncodeOpts {
    fn default() -> Self {
        EncodeOpts { fma: false, unorm: UnormRounding::Trunc12, f16_store: false, f16_c: false }
    }
}

/// The value the UAV store quantises: optionally through f16 first.
#[inline]
pub fn store_value(v: f32, o: EncodeOpts) -> f32 {
    if o.f16_store { quantise_f16(v, Rounding::NearestEven) } else { v }
}

/// The three encoded textures: Y4 (w×h, 4 channels), Cb4 and Cr4 (w/2×h/2, 4 channels), bytes 0..255.
pub struct YCbCr4 {
    pub y4: Vec<[u8; 4]>,
    pub cb4: Vec<[u8; 4]>,
    pub cr4: Vec<[u8; 4]>,
    pub w: u32,
    pub h: u32,
}

/// CS 23025 `LmCompress_HBasis_YCbCr4`, one 8×8 thread group at a time (the chroma pass reads the group's
/// shared Y/Cb/Cr of the 2×2 block at (x, y), (x+1, y), (x, y+1), (x+1, y+1) for even x, y).
///
/// `imgs[k]` = coefficient image k (t0..t3, RGBA16F as f32), `maxhdr[k]` = g_In_MaxHdrHBasis[k] (the
/// reduction above), `mood_max_hdr` = cb Mood_MaxHdr.
pub fn encode_ycbcr4(imgs: [&Buf; 4], maxhdr: [f32; 4], mood_max_hdr: f32, o: EncodeOpts) -> YCbCr4 {
    let (w, h) = (imgs[0].w, imgs[0].h);
    assert!(w % 8 == 0 && h % 8 == 0, "the thread groups are 8×8");
    let mut y4 = vec![[0u8; 4]; (w * h) as usize];
    let mut cb4 = vec![[0u8; 4]; (w * h / 4) as usize];
    let mut cr4 = vec![[0u8; 4]; (w * h / 4) as usize];
    // 0-4: r0.y = 1 / max(MaxHdr[0] / Mood_MaxHdr, 1)  (the whole encode scales down when C0 exceeds the mood's max)
    let inv = 1.0f32 / (maxhdr[0] / mood_max_hdr).max(1.0);
    let mut g0 = [[0f32; 4]; 64];
    let mut g1 = [[0f32; 4]; 64];
    let mut g2 = [[0f32; 4]; 64];
    for gy in 0..h / 8 {
        for gx in 0..w / 8 {
            // pass 1: every thread encodes its texel's four coefficients and stores Y4
            for ty in 0..8u32 {
                for tx in 0..8u32 {
                    let (x, y) = (gx * 8 + tx, gy * 8 + ty);
                    let tid = (ty * 8 + tx) as usize;
                    let mut yv = [0f32; 4];
                    let mut cbv = [0f32; 4];
                    let mut crv = [0f32; 4];
                    for k in 0..4 {
                        // 16-17 / 33-34 / 50-51 (and 4 for k = 0): m_k = r0.y * MaxHdr[k]
                        let m = inv * maxhdr[k];
                        let p = [imgs[k].get(x, y, 0), imgs[k].get(x, y, 1), imgs[k].get(x, y, 2)];
                        let mut c = [0f32; 3];
                        if k == 0 {
                            // 8-11: div, max 0, sqrt, min 1
                            for i in 0..3 {
                                let v = p[i] / m;
                                let v = v.max(0.0);
                                let v = v.sqrt();
                                c[i] = v.min(1.0);
                            }
                        } else {
                            // 19-28: div, sign = (0 < v) - (v < 0), sqrt(|v|) * sign, * 0.5 + 0.5, max -1, min 1
                            for i in 0..3 {
                                let v = p[i] / m;
                                let s = ((0.0 < v) as i32 - (v < 0.0) as i32) as f32;
                                let r = v.abs().sqrt() * s;
                                let r = mad(r, 0.5, 0.5, o.fma);
                                let r = r.max(-1.0);
                                c[i] = r.min(1.0);
                            }
                        }
                        if o.f16_c {
                            for i in 0..3 { c[i] = quantise_f16(c[i], Rounding::NearestEven); }
                        }
                        let c4 = [c[0], c[1], c[2], 1.0];
                        yv[k] = dp4(K_Y, c4, o.fma);
                        cbv[k] = dp4(K_CB, c4, o.fma);
                        crv[k] = dp4(K_CR, c4, o.fma);
                    }
                    g0[tid] = yv;
                    g1[tid] = cbv;
                    g2[tid] = crv;
                    // 70-71: Y4 = max(Y, 0) stored UNORM
                    let mut out = [0u8; 4];
                    for k in 0..4 {
                        out[k] = unorm8(store_value(yv[k].max(0.0), o), o.unorm);
                    }
                    y4[(y * w + x) as usize] = out;
                }
            }
            // pass 2 (after the group barrier): the even-even threads average the 2×2 block's chroma, weighted
            // by each texel's Y normalised by the block max
            for ty in (0..8u32).step_by(2) {
                for tx in (0..8u32).step_by(2) {
                    let (x, y) = (gx * 8 + tx, gy * 8 + ty);
                    let tid = (ty * 8 + tx) as usize;
                    let (ta, tb, tc) = (tid + 1, tid + 8, tid + 9);
                    let r3 = g0[tid];
                    let r1 = g0[ta];
                    let r5 = g0[tb];
                    let r6 = g0[tc];
                    let mut cb_out = [0u8; 4];
                    let mut cr_out = [0u8; 4];
                    for k in 0..4 {
                        // 81-84: r7 = max(max(r1, r3), max(r5, r6)); r7 = max(r7, 0)
                        let mx = r1[k].max(r3[k]).max(r5[k].max(r6[k])).max(0.0);
                        // 85-88: the four weights (a division by zero yields inf/NaN exactly as the GPU does)
                        let w3 = r3[k] / mx;
                        let w1 = r1[k] / mx;
                        let w5 = r5[k] / mx;
                        let w6 = r6[k] / mx;
                        // 89-91: r7 = r1 + r3; r7 = r5 + r7; r7 = r6 + r7
                        let mut sum = w1 + w3;
                        sum = w5 + sum;
                        sum = w6 + sum;
                        // 92-98: Cb = r1*Cb[ta]; += r3*Cb[tid]; += r5*Cb[tb]; += r6*Cb[tc]
                        let mut cb = w1 * g1[ta][k];
                        cb = mad(w3, g1[tid][k], cb, o.fma);
                        cb = mad(w5, g1[tb][k], cb, o.fma);
                        cb = mad(w6, g1[tc][k], cb, o.fma);
                        // 99-100: has = 0 < sum; Cb = saturate(Cb / sum)
                        let has = 0.0 < sum;
                        let cb = (cb / sum).clamp(0.0, 1.0);
                        let cb = if cb.is_nan() { 0.0 } else { cb };
                        // 101-108: the same for Cr
                        let mut cr = w1 * g2[ta][k];
                        cr = mad(w3, g2[tid][k], cr, o.fma);
                        cr = mad(w5, g2[tb][k], cr, o.fma);
                        cr = mad(w6, g2[tc][k], cr, o.fma);
                        let cr = (cr / sum).clamp(0.0, 1.0);
                        let cr = if cr.is_nan() { 0.0 } else { cr };
                        // 110-113: has ? value : 0.5
                        cb_out[k] = unorm8(store_value(if has { cb } else { 0.5 }, o), o.unorm);
                        cr_out[k] = unorm8(store_value(if has { cr } else { 0.5 }, o), o.unorm);
                    }
                    let oi = ((y / 2) * (w / 2) + x / 2) as usize;
                    cb4[oi] = cb_out;
                    cr4[oi] = cr_out;
                }
            }
        }
    }
    YCbCr4 { y4, cb4, cr4, w, h }
}

/// Compare an encoded texture with a captured one (values 0..1 from the DDS loader → bytes).
pub fn compare_u8(ours: &[[u8; 4]], theirs: &Buf) -> (usize, usize, u32, [usize; 4]) {
    let n = ours.len();
    let mut diff = 0usize;
    let mut maxd = 0u32;
    let mut per_ch = [0usize; 4];
    for (i, o) in ours.iter().enumerate() {
        for c in 0..4 {
            let t = (theirs.data[i * 4 + c] * 255.0).round() as i32;
            let d = (o[c] as i32 - t).unsigned_abs();
            if d != 0 {
                diff += 1;
                per_ch[c] += 1;
                maxd = maxd.max(d);
            }
        }
    }
    (n * 4, diff, maxd, per_ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unorm8_rounding() {
        let ne = UnormRounding::NearestEven;
        assert_eq!(unorm8(0.0, ne), 0);
        assert_eq!(unorm8(1.0, ne), 255);
        assert_eq!(unorm8(2.0, ne), 255);
        assert_eq!(unorm8(-1.0, ne), 0);
        assert_eq!(unorm8(0.5, ne), 128); // 127.5 → ties to even = 128
        assert_eq!(unorm8(0.5, UnormRounding::HalfUp), 128);
        assert_eq!(unorm8(0.5, UnormRounding::Truncate), 127);
        assert_eq!(unorm8(f32::NAN, ne), 0);
    }

    #[test]
    fn maxhdr_is_the_f16_max_of_abs_rgb() {
        let mut b = Buf::new(8, 8, 4);
        b.set(3, 5, 1, -2.3339);
        b.set(0, 0, 3, 100.0); // alpha is not part of the max
        let m = maxhdr_hbasis(&b);
        assert!((m - 2.333984375).abs() < 1e-6, "{m}");
    }

    #[test]
    fn encode_neutral_chroma_where_black() {
        let imgs: Vec<Buf> = (0..4).map(|_| Buf::new(8, 8, 4)).collect();
        let e = encode_ycbcr4([&imgs[0], &imgs[1], &imgs[2], &imgs[3]], [1.0; 4], 7.5, EncodeOpts::default());
        // black C0 → c = 0 → Y = 16/255; signed images at 0 → c = 0.5 → Y = 0.5·(0.2568+0.5041+0.0979) + 0.0627
        let ne = UnormRounding::NearestEven;
        assert_eq!(e.y4[0][0], unorm8(0.062745, ne));
        assert_eq!(e.y4[0][1], unorm8(0.5 * (0.256788 + 0.504129 + 0.097906) + 0.062745, ne));
        // the chroma block has positive Y everywhere → weighted average of identical values
        assert_eq!(e.cb4[0][0], unorm8(0.501961, ne));
    }
}
