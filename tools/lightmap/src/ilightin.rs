//! ROW 4 — the ILIGHTINPUT CHAIN of a compute frame, transcribed from the captured shaders (passcap/pwc-day,
//! capture pwc2 frame 127448 — the first compute frame, sweep 0; shaders-frame127448/): the pixel shaders
//! that turn the direct-sun accumulation target (16963, RGBA16F 2048², alpha = the raster coverage count in
//! ninths) and the resolved MDiffuse atlas (16969, B8G8R8A8) into the `ILightInput` texture (17095,
//! R11G11B10) and its coverage (17104 / 17309 / 17312, R8_UNORM) that every peel layer samples (SRV1 of
//! PS 17131/17134).
//!
//! The chain (logs/draws-frame127448.json, every draw a 3-vertex full-screen triangle):
//!
//! | eid | PS | in → out | state |
//! |---|---|---|---|
//! | 27 | 17043 `TMapLightSum` | 16963 (the 9-run MDiffuse accumulation of frame 127447) → 16969 B8G8R8A8 | no blend, `DebugShowOverlap = 0` |
//! | 35, 43 | clear 16963 to 0 | | |
//! | 88 … 165 | 1332 × 8 | 16963 ↔ 16966 (a dilation of the cleared target: zeros) | |
//! | 188, 277 | 1034 | 16963 → 17077 (R11G11B10), 17077 → 17095 | copies, `Raster_ST_Input = (1, 1, 0, 0)` |
//! | 476 … 765 | 15187 direct sun × 9 offsets | → 16963 (`sunpass.rs`) | blend One/One |
//! | 791 | 1038 `TMapInput` | 16963 → 17104 R8_UNORM | `ColorMat4` = column 3 → the alpha into every channel |
//! | 812 | 17043 | 16963 → 17095 | blend One/One onto the zeros of eid 277 |
//! | 836 | 1109 | 16969 → 17095 | blend DstCol/Zero, `ScaleSrc = (1, 1, 1, 1)`: 17095 ×= the MDiffuse |
//! | 857 … 927 | 1335 × 8 | (17095, 17104) → (17098, 17309) → (17095, 17312) → … | MRT: R11G11B10 + R8_UNORM |
//!
//! Every function follows its DXBC instruction by instruction (Pixel_17043.txt, Pixel_1038.txt,
//! Pixel_1335.txt in shaders-frame127448/); the store roundings are the caller's (`lmtool ilightin-check`
//! tries the candidates and prints which one the capture agrees with).
//!
//! What the capture says (`lmtool ilightin-check passcap/pwc-day`, 2026-09-25) — hardware facts of WhiteStick's GPU:
//!
//! * `mad` is FUSED (one rounding) and `div` is `a × rcp(b)`: PS 1335's first pass, whose weights are the fractional
//!   R8 coverage, lands 12 582 912 of 12 582 912 values bit-identical only with both (mul + add leaves 271 806 one
//!   R11G11B10 quantum low, the quotient of equal-weighted neighbours sitting exactly on a quantum); passes 2–8 (weights
//!   0/1) are bit-identical either way. The R11G11B10 store truncates; the R8 coverage store rounds to nearest.
//! * eid 27 writes 16969 through a B8G8R8A8_UNORM_SRGB view: rgb sRGB-ENCODED then rounded to nearest (16 776 784 of
//!   16 777 216 bit-identical with the IEC 61966-2-1 curve; 432 values one byte step off = the ROP's encoder vs the
//!   formula at rounding boundaries), alpha stored linear (1.0 where covered).
//! * eid 836 reads 16969 through the same view — `ld` returns the byte DECODED to linear — and the blend multiplies it
//!   into the R11G11B10-truncated resolve, truncating again: 12 321 887 of 12 582 912 values bit-identical with the IEC
//!   decode, 261 025 one quantum low, none beyond. The residual is the GPU's own sRGB→linear table: bounding each
//!   byte's decoded value from the capture (`--fit-srgb`: q ≤ d·L(byte) < q + quantum over the texels sharing the
//!   byte) puts the IEC value outside the bound for 178 of 317 observed (byte, channel) cells, by up to 0.3 % either
//!   way (e.g. L(105) = 0.1416667 where the curve gives 0.1412633); no rounding of the curve to a fixed precision
//!   fits all cells — the table is measured hardware, not derivable. Coverage (PS 1038): 4 194 304 of 4 194 304.

use crate::passdiff::Buf;

/// D3D `ld`: out-of-range reads 0; a missing alpha channel reads 1 (the R11G11B10 view's default).
#[inline]
fn ld(src: &Buf, x: i64, y: i64) -> [f32; 4] {
    if x < 0 || y < 0 || x >= src.w as i64 || y >= src.h as i64 {
        return [0.0; 4];
    }
    let (x, y) = (x as u32, y as u32);
    let c = |k: u32| if k < src.channels { src.get(x, y, k) } else if k == 3 { 1.0 } else { 0.0 };
    [c(0), c(1), c(2), c(3)]
}

/// PS 17043 (`TMapLightSum`): the plain normalise — for `r1 = ld(x, y)`:
/// `r2 = (1.95 < w, 0.01 < w)`; `r0.xyz = rgb / w`, `r0.w = 1`; `r1 = r0 & r2.y` (a bitwise and with the
/// all-ones/zero mask: the normalised texel where covered, exact 0 bits — never a NaN — where not);
/// `DebugShowOverlap != 0 && 1.95 < w` selects the grey `(dot(r0.xyz, 1/3), 0, 0, 1)` instead. Returns f32
/// rgba; the caller quantises to the target (B8G8R8A8 at eid 27, R11G11B10 blended at eid 812).
pub fn resolve_ps17043_texel(src: &Buf, x: u32, y: u32, debug_show_overlap: bool) -> [f32; 4] {
    resolve_ps17043_texel_div(src, x, y, debug_show_overlap, crate::gpucmp::DivModel::MulRcp)
}

/// As `resolve_ps17043_texel` with the division model chosen (the captured GPU divides as `a × rcp(b)`).
pub fn resolve_ps17043_texel_div(src: &Buf, x: u32, y: u32, debug_show_overlap: bool, dm: crate::gpucmp::DivModel) -> [f32; 4] {
    let dv = |a: f32, b: f32| crate::gpucmp::div(a, b, dm);
    let r1 = ld(src, x as i64, y as i64);
    let (over, covered) = (1.95 < r1[3], 0.01 < r1[3]);
    let r0 = [dv(r1[0], r1[3]), dv(r1[1], r1[3]), dv(r1[2], r1[3]), 1.0];
    let masked = if covered { r0 } else { [0.0; 4] };
    if over && debug_show_overlap {
        let g = (r0[0] * 0.333333 + r0[1] * 0.333333) + r0[2] * 0.333333;
        return [g, 0.0, 0.0, 1.0];
    }
    masked
}

/// PS 17043 over a target (f32 values, 4 channels).
pub fn resolve_ps17043(src: &Buf, debug_show_overlap: bool) -> Buf {
    let mut out = Buf::new(src.w, src.h, 4);
    let w = src.w;
    out.fill_rows_par(|y, row| {
        for x in 0..w {
            let o = resolve_ps17043_texel(src, x, y, debug_show_overlap);
            for k in 0..4 {
                row[(x * 4 + k) as usize] = o[k as usize];
            }
        }
    });
    out
}

/// PS 1038 (`TMapInput` texture2darray through `SamplerDyna0`, cb `Raster_ST_Input`, `ColorMat4`): the
/// full-screen copy of PS 1034 followed by the 4×4 colour matrix — `o0.k = dp4(sample, ColorMat4[k])`, where
/// `ColorMat4[k]` is DXBC register k = COLUMN k of the HLSL `column_major float4x4` (the draw log prints the
/// HLSL rows; pass them as printed, `cols` are derived here). Returns f32 rgba; the target is R8_UNORM (the
/// R channel stored), the caller quantises.
pub fn mask_ps1038(src: &Buf, st: [f32; 4], color_mat4_rows: [[f32; 4]; 4], w: u32, h: u32) -> Buf {
    let sampled = crate::finalprep::copy_ps1034(src, st, w, h);
    let col = |k: usize| [color_mat4_rows[0][k], color_mat4_rows[1][k], color_mat4_rows[2][k], color_mat4_rows[3][k]];
    let cols = [col(0), col(1), col(2), col(3)];
    let mut out = Buf::new(w, h, 4);
    let sampled = &sampled;
    out.fill_rows_par(|y, row| {
        for x in 0..w {
            let s = [sampled.get(x, y, 0), sampled.get(x, y, 1), sampled.get(x, y, 2), sampled.get(x, y, 3)];
            for k in 0..4 {
                let c = cols[k];
                // dp4: the products summed left to right
                let v = ((s[0] * c[0] + s[1] * c[1]) + s[2] * c[2]) + s[3] * c[3];
                row[(x * 4) as usize + k] = v;
            }
        }
    });
    out
}

/// PS 1335 (`TMapInput` + `TMapCoverage` → MRT0 colour, MRT1 coverage), one texel, in the bytecode's order:
/// `r1 = ld(x, y)` (rgba), `r2.x = cov(x, y)`; if `cov < 1e-4`: `r3 = in(−1,−1)`, `r2.y = cov(−1,−1)`,
/// `r4 = in(0,−1)`, `r2.z = cov(0,−1)`, `r4 = r2.z · r4` (l.10), `r3 = r3 · r2.y + r4` (l.11, a mad),
/// `r2.y = r2.z + r2.y` (l.12); then for (1,−1), (−1,0), (1,0), (−1,1), (0,1): `r3 = in · cov + r3`,
/// `r2.y = cov + r2.y`; then (1,1): `r3 = in · cov + r3`, `r0.x = cov + r2.y`; `r0.y = 1e-4 < r0.x`;
/// `r3 /= r0.x` (all four channels); `o0 = r0.y ? r3 : r1`; `o1 = r0.y ? 1 : r2.x` — else `o0 = r1`,
/// `o1 = 1`. Returns (rgba f32, coverage f32); the caller quantises MRT0 to R11G11B10 and MRT1 to UNORM8.
/// `fma` fuses each `mad` (the driver may, `refactoringAllowed`).
pub fn dilate_ps1335_texel(input: &Buf, coverage: &Buf, x: u32, y: u32, fma: bool) -> ([f32; 4], f32) {
    dilate_ps1335_texel_div(input, coverage, x, y, fma, crate::gpucmp::DivModel::MulRcp)
}

/// As `dilate_ps1335_texel` with the division model chosen (`gpucmp::DivModel`).
pub fn dilate_ps1335_texel_div(input: &Buf, coverage: &Buf, x: u32, y: u32, fma: bool, dm: crate::gpucmp::DivModel) -> ([f32; 4], f32) {
    let (xi, yi) = (x as i64, y as i64);
    let r1 = ld(input, xi, yi);
    let cov = ld(coverage, xi, yi)[0];
    if !(cov < 0.0001) {
        return (r1, 1.0);
    }
    let mad = |a: f32, b: f32, c: f32| if fma { a.mul_add(b, c) } else { a * b + c };
    let n = |dx: i64, dy: i64| (ld(input, xi + dx, yi + dy), ld(coverage, xi + dx, yi + dy)[0]);
    let (i00, c00) = n(-1, -1);
    let (i10, c10) = n(0, -1);
    let mut r3 = [c10 * i10[0], c10 * i10[1], c10 * i10[2], c10 * i10[3]];
    r3 = [mad(i00[0], c00, r3[0]), mad(i00[1], c00, r3[1]), mad(i00[2], c00, r3[2]), mad(i00[3], c00, r3[3])];
    let mut wsum = c10 + c00;
    for &(dx, dy) in &[(1i64, -1i64), (-1, 0), (1, 0), (-1, 1), (0, 1)] {
        let (i, c) = n(dx, dy);
        r3 = [mad(i[0], c, r3[0]), mad(i[1], c, r3[1]), mad(i[2], c, r3[2]), mad(i[3], c, r3[3])];
        wsum = c + wsum;
    }
    let (i11, c11) = n(1, 1);
    r3 = [mad(i11[0], c11, r3[0]), mad(i11[1], c11, r3[1]), mad(i11[2], c11, r3[2]), mad(i11[3], c11, r3[3])];
    let total = c11 + wsum;
    let any = 0.0001 < total;
    let dv = |a: f32| crate::gpucmp::div(a, total, dm);
    let div = [dv(r3[0]), dv(r3[1]), dv(r3[2]), dv(r3[3])];
    if any { (div, 1.0) } else { (r1, cov) }
}

/// PS 1335 over a target: (colour f32 rgb, coverage f32).
/// The captured GPU FUSES every `mad` and divides as `a × rcp(b)` (pass 1 of the capture, whose weights are fractional,
/// lands 12 582 912 of 12 582 912 values bit-identical only with both; plain mul+add leaves 271 806 one quantum low):
/// this entry point uses that model.
pub fn dilate_ps1335(input: &Buf, coverage: &Buf) -> (Buf, Buf) {
    dilate_ps1335_div(input, coverage, true, crate::gpucmp::DivModel::MulRcp)
}

/// As `dilate_ps1335` with the division model chosen.
pub fn dilate_ps1335_div(input: &Buf, coverage: &Buf, fma: bool, dm: crate::gpucmp::DivModel) -> (Buf, Buf) {
    let mut oc = Buf::new(input.w, input.h, 3);
    let mut ow = Buf::new(input.w, input.h, 1);
    // (the colour rows in parallel, then the coverage rows from the same texel function — the texel evaluated once per
    // output rather than one task writing rows of two buffers)
    let wd = input.w;
    oc.fill_rows_par(|y, row| {
        for x in 0..wd {
            let (c, _) = dilate_ps1335_texel_div(input, coverage, x, y, fma, dm);
            row[(x * 3) as usize] = c[0];
            row[(x * 3 + 1) as usize] = c[1];
            row[(x * 3 + 2) as usize] = c[2];
        }
    });
    ow.fill_rows_par(|y, row| {
        for x in 0..wd {
            let (_, w) = dilate_ps1335_texel_div(input, coverage, x, y, fma, dm);
            row[x as usize] = w;
        }
    });
    (oc, ow)
}

/// How a render-target UNORM8 store converts a float (the typed-UAV store was measured as `Trunc12`,
/// `gpuenc::UnormRounding`; the RT store is measured by `lmtool ilightin-check`).
pub fn unorm8_rt(v: f32, mode: crate::gpuenc::UnormRounding) -> f32 {
    crate::gpuenc::unorm8(v, mode) as f32 / 255.0
}

/// Quantise a 3-channel f32 buffer to the R11G11B10 target.
pub fn quantise_r11(b: &Buf, r: crate::gpufmt::Rounding) -> Buf {
    let mut out = Buf::new(b.w, b.h, 3);
    let w = b.w;
    out.fill_rows_par(|y, row| {
        for x in 0..w {
            let q = crate::gpufmt::quantise_r11g11b10([b.get(x, y, 0), b.get(x, y, 1), b.get(x, y, 2)], r);
            row[(x * 3) as usize] = q[0];
            row[(x * 3 + 1) as usize] = q[1];
            row[(x * 3 + 2) as usize] = q[2];
        }
    });
    out
}

/// Quantise a buffer's channel 0 to UNORM8 (an R8_UNORM target).
pub fn quantise_unorm8(b: &Buf, mode: crate::gpuenc::UnormRounding) -> Buf {
    let mut out = Buf::new(b.w, b.h, 1);
    let w = b.w;
    out.fill_rows_par(|y, row| {
        for x in 0..w {
            row[x as usize] = unorm8_rt(b.get(x, y, 0), mode);
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(w: u32, h: u32, ch: u32, f: impl Fn(u32, u32, u32) -> f32) -> Buf {
        let mut b = Buf::new(w, h, ch);
        for y in 0..h { for x in 0..w { for c in 0..ch { b.set(x, y, c, f(x, y, c)); } } }
        b
    }

    #[test]
    fn resolve_17043_masks_uncovered_texels_to_exact_zero() {
        let b = buf(2, 1, 4, |x, _, c| if x == 0 { [0.0, 0.3, 0.0, 0.0][c as usize] } else { [0.9, 0.6, 0.3, 0.7778][c as usize] });
        assert_eq!(resolve_ps17043_texel(&b, 0, 0, false), [0.0; 4]);
        // the division is a × rcp(b) (the captured GPU's): within one f32 ulp of the exact quotient
        let r = resolve_ps17043_texel(&b, 1, 0, false);
        let rcp = 1.0f32 / 0.7778;
        assert_eq!(r, [0.9 * rcp, 0.6 * rcp, 0.3 * rcp, 1.0]);
        for k in 0..3 { assert!((r[k] - [0.9f32, 0.6, 0.3][k] / 0.7778).abs() <= 2.0 * f32::EPSILON * r[k]); }
    }

    #[test]
    fn mask_1038_takes_the_column_of_the_colour_matrix() {
        // the draw log's ColorMat4 rows: only row 3 = (1, 1, 1, 1) → column k = (0, 0, 0, 1) → every output = alpha
        let rows = [[0.0; 4], [0.0; 4], [0.0; 4], [1.0; 4]];
        let b = buf(2, 2, 4, |x, y, c| if c == 3 { 0.25 * (1 + x + 2 * y) as f32 } else { 0.9 });
        let m = mask_ps1038(&b, [1.0, 1.0, 0.0, 0.0], rows, 2, 2);
        assert_eq!(m.get(1, 1, 0), 1.0);
        assert_eq!(m.get(0, 0, 0), 0.25);
        assert_eq!(m.get(1, 0, 2), 0.5);
    }

    #[test]
    fn dilate_1335_fills_an_uncovered_texel_from_its_covered_neighbours_and_sets_coverage() {
        let input = buf(3, 3, 3, |x, y, c| if (x, y) == (0, 0) { [0.8, 0.4, 0.2][c as usize] } else if (x, y) == (2, 2) { [0.2, 0.4, 0.8][c as usize] } else { 0.0 });
        let cov = buf(3, 3, 1, |x, y, _| if (x, y) == (0, 0) { 1.0 } else if (x, y) == (2, 2) { 0.5 } else { 0.0 });
        let (c, w) = dilate_ps1335_texel(&input, &cov, 1, 1, false);
        // (0.8·1 + 0.2·0.5) / 1.5, …
        assert!((c[0] - 0.9 / 1.5).abs() < 1e-6 && (c[2] - 0.6 / 1.5).abs() < 1e-6);
        assert_eq!(w, 1.0);
        // a covered texel is copied, coverage 1
        assert_eq!(dilate_ps1335_texel(&input, &cov, 2, 2, false), ([0.2, 0.4, 0.8, 1.0], 1.0));
        // an uncovered texel without covered neighbours keeps its own colour and coverage
        let (c0, w0) = dilate_ps1335_texel(&input, &cov, 2, 0, false);
        assert_eq!((c0, w0), ([0.0, 0.0, 0.0, 1.0], 0.0));
    }
}
