//! ROW 12 — the FINALISATION PREP, transcribed from the captured shaders (passcap/pwc-day, capture pwc4/5
//! frame 74490, RenderDoc ids of that frame): the three pixel shaders that turn the four RGBA16F H-basis
//! accumulation targets (`final_00_hbasis_sweep_end`) into the images the gutter dilation (PS 1332,
//! `gpuenc::dilate_ps1332`) and the encode (CS 23025, `gpuenc::encode_ycbcr4`) consume.
//!
//! The chain (draws-frame74490.json, eids 34675 … 34917; every draw a 3-vertex full-screen triangle,
//! VS 522 for 25113/1109, VS 870 for 1034):
//!
//! | eid | PS | in → out | state |
//! |---|---|---|---|
//! | 34675/34695/34715/34735 | 25113 `TMapLightSSAA` | 24752 → 24858, 24749 → 24752, 24852 → 24749, 24855 → 24852 | no blend, cb `DebugShowOverlap = 0` |
//! | 34758/34780/34802/34824 | 1109 `TMapInput` | 24858 → 24911, 24752 → 24914, 24749 → 24917, 24852 → 24920 | blend One/One into cleared targets, cb `ScaleSrc = (2, 2, 2, 0)` |
//! | 34848/34871/34894/34917 | 1034 `TMapInput` | 24911 → 24858, 24914 → 24752, 24917 → 24749, 24920 → 24852 | write mask RGB (7), cb `Raster_ST_Input = (1, 1, 0, 0)` |
//!
//! Every function follows its DXBC instruction by instruction (shaders-frame74490/Pixel_25113.txt,
//! Pixel_1109.txt, Pixel_1034.txt); constants are the bytecode's literals. The targets are RGBA16F and the
//! render-target store of an unblended pixel-shader output TRUNCATES to f16 (the baker's hardware fact,
//! `lmtool dilate-check`); the caller applies the store rounding (`lmtool finalprep-check`).
//!
//! What the capture says (`lmtool finalprep-check passcap/pwc-day`, 2026-09-25):
//!
//! * PS 25113: 4 × 16 777 216 of 16 777 216 values bit-identical — once `div` is modelled as `a × rcp(b)`
//!   (`gpucmp::DivModel::MulRcp`): an IEEE division leaves 17 texels per image whose `w / w` the GPU stores as
//!   0.999512 (= f16 just under 1), the signature of a reciprocal-multiply that lands one f32 ulp under 1.
//! * PS 1109 × 2: the four targets 24911/24914/24917/24920 are NOT empty when the blend adds to them — they hold the
//!   PREVIOUS SWEEP's finalised images (the same chain run at the end of sweep 0: PS 25113 on that sweep's H-basis
//!   MRTs, × 2). With pwc6's banked sweep-0 end (hbasis0..3 at frame 7533 eid 13903) through our 25113 and × 2 as
//!   the prior content, `f16_rtne(prior + f16_rtz(2·src))` reproduces 16 777 193 / 187 / 179 / 176 of 16 777 216
//!   values per image; the 11–20 differing texels sit at the same positions in all four images (a difference between
//!   the two capture RUNS pwc6 and pwc4, not of the kernel). So the × 2 step is where the sweeps ADD UP: the final
//!   lightmap is 2·(C_sweep0 + C_sweep1 + …) per coefficient.
//! * PS 1034: 4 × 16 777 216 of 16 777 216 bit-identical (rgb copied, alpha kept).

use crate::gpufmt::{quantise_f16, Rounding};
use crate::passdiff::Buf;

/// D3D `ld` on a texture2d: out-of-range texel addresses read 0.
#[inline]
fn ld(src: &Buf, x: i64, y: i64) -> [f32; 4] {
    if x < 0 || y < 0 || x >= src.w as i64 || y >= src.h as i64 {
        return [0.0; 4];
    }
    let (x, y) = (x as u32, y as u32);
    let c = |k: u32| if k < src.channels { src.get(x, y, k) } else if k == 3 { 1.0 } else { 0.0 };
    [c(0), c(1), c(2), c(3)]
}

/// PS 25113 (`TMapLightSSAA`): the per-texel resolve of an accumulation target whose alpha counts the
/// coverage — one texel at (x, y). In the bytecode's order:
///
/// * `r1 = ld(x, y)`; if `r1.w < 0.01` (uncovered) the texel is written back as it is (l.3–5);
/// * else if `DebugShowOverlap != 0 && 1.95 < r1.w`: the grey `dot(rgb / w, 1/3)` in R, (0, 0, 1) in GBA (l.7–13);
/// * else if `0.99 < r1.w` (fully covered): `rgba / w` (l.15–17);
/// * else (a partially covered texel, l.19–45): `rgb /= w`; for the four neighbours in the order
///   `icb = (−1, 0), (+1, 0), (0, −1), (0, +1)`: a neighbour with `w > 0.99` is counted, and if the texel
///   one step further in the same direction also has `w > 0.99` the neighbour becomes the candidate
///   (`movc r4`, the LAST qualifying neighbour wins); if exactly one neighbour was counted
///   (`|count − 1| < 0.01`) and the candidate's `w > 0.99`, the output rgb is the candidate's `rgb / w`,
///   else the texel's own `rgb / w`; alpha = 1.
///
/// `div` is the D3D division (IEEE, f32); `ld` out of range reads 0.
pub fn resolve_ps25113_texel(src: &Buf, x: u32, y: u32, debug_show_overlap: bool) -> [f32; 4] {
    resolve_ps25113_texel_div(src, x, y, debug_show_overlap, crate::gpucmp::DivModel::MulRcp)
}

/// As `resolve_ps25113_texel`, with the division model chosen (`gpucmp::DivModel`).
pub fn resolve_ps25113_texel_div(src: &Buf, x: u32, y: u32, debug_show_overlap: bool, dm: crate::gpucmp::DivModel) -> [f32; 4] {
    let dv = |a: f32, b: f32| crate::gpucmp::div(a, b, dm);
    const ICB: [(i64, i64); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    let r1 = ld(src, x as i64, y as i64);
    if r1[3] < 0.01 {
        return r1;
    }
    if debug_show_overlap && 1.95 < r1[3] {
        let r2 = [dv(r1[0], r1[3]), dv(r1[1], r1[3]), dv(r1[2], r1[3])];
        // dp3 with l(0.333333, 0.333333, 0.333333): the D3D reference sums the products left to right
        let g = (r2[0] * 0.333333 + r2[1] * 0.333333) + r2[2] * 0.333333;
        return [g, 0.0, 0.0, 1.0];
    }
    if 0.99 < r1[3] {
        return [dv(r1[0], r1[3]), dv(r1[1], r1[3]), dv(r1[2], r1[3]), dv(r1[3], r1[3])];
    }
    let own = [dv(r1[0], r1[3]), dv(r1[1], r1[3]), dv(r1[2], r1[3])];
    let mut r4 = [0.0f32; 4];
    let mut count = 0.0f32;
    for &(dx, dy) in &ICB {
        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
        let r5 = ld(src, nx, ny);
        if 0.99 < r5[3] {
            count += 1.0;
            let t = ld(src, nx + dx, ny + dy)[3];
            if 0.99 < t {
                r4 = r5;
            }
        }
    }
    let cond = (count - 1.0).abs() < 0.01 && 0.99 < r4[3];
    // l.43: the candidate's rgb / w is computed regardless (a NaN when r4.w = 0, never selected then)
    let cand = [dv(r4[0], r4[3]), dv(r4[1], r4[3]), dv(r4[2], r4[3])];
    let rgb = if cond { cand } else { own };
    [rgb[0], rgb[1], rgb[2], 1.0]
}

/// PS 25113 over a whole target; `store` = the RGBA16F render-target rounding (truncation on the captured
/// GPU for an unblended store).
pub fn resolve_ps25113(src: &Buf, debug_show_overlap: bool, store: Rounding) -> Buf {
    let mut out = Buf::new(src.w, src.h, 4);
    for y in 0..src.h {
        for x in 0..src.w {
            let o = resolve_ps25113_texel(src, x, y, debug_show_overlap);
            for k in 0..4 {
                out.set(x, y, k, quantise_f16(o[k as usize], store));
            }
        }
    }
    out
}

/// PS 1109 (`TMapInput`, cb `ScaleSrc`): `o0 = ld(x, y) × ScaleSrc` (l.0–3). The draw blends One/One into a
/// target cleared to 0, so the stored value is the blend's rounding of `src + 0` = the source itself rounded
/// to the target: `store` (the blended f16 store rounds the SUM to nearest even, the source having been
/// truncated first — with ×2 of an f16 value both are exact, so the rounding never bites here).
pub fn scale_ps1109(src: &Buf, scale: [f32; 4], src_round: Rounding, sum_round: Rounding) -> Buf {
    let mut out = Buf::new(src.w, src.h, 4);
    for y in 0..src.h {
        for x in 0..src.w {
            let v = ld(src, x as i64, y as i64);
            for k in 0..4 {
                let s = quantise_f16(v[k] * scale[k], src_round);
                out.set(x, y, k as u32, quantise_f16(s + 0.0, sum_round));
            }
        }
    }
    out
}

/// PS 1109's other use (frame 127448 eid 836): blend DstCol/Zero — the target is MULTIPLIED by the shader's
/// output `ld(src) × ScaleSrc`; `dst` is the target's content before the draw, the result is `store`-rounded
/// per channel (`R11G11B10` there — the caller quantises, this returns f32).
pub fn multiply_ps1109(dst: &Buf, src: &Buf, scale: [f32; 4]) -> Buf {
    let mut out = Buf::new(dst.w, dst.h, dst.channels);
    for y in 0..dst.h {
        for x in 0..dst.w {
            let s = ld(src, x as i64, y as i64);
            for k in 0..dst.channels {
                out.set(x, y, k, s[k as usize] * scale[k as usize] * dst.get(x, y, k));
            }
        }
    }
    out
}

/// PS 1034 (`TMapInput` texture2darray, `SamplerDyna0`, cb `Raster_ST_Input`): the full-screen copy —
/// `uv = v1 × ST.xy + ST.zw` sampled at slice 0. VS 870 gives `v1 = (x/W, y/H)` at the pixel centre, so with
/// `ST = (1, 1, 0, 0)` the sample lands on the texel centre of a same-sized input and the bilinear weights are
/// (1, 0): the texel itself. Returns the sampled values (all four channels); the draw's write mask (RGB in the
/// finalisation, RGBA in frame 127448's copies) is applied by the caller through `write_masked`.
pub fn copy_ps1034(src: &Buf, st: [f32; 4], w: u32, h: u32) -> Buf {
    let mut out = Buf::new(w, h, 4);
    for y in 0..h {
        for x in 0..w {
            // VS 870: o1 = (ndc.x · 0.5 + 0.5, ndc.y · −0.5 + 0.5) at the pixel centre = ((x + 0.5)/W, (y + 0.5)/H)
            let u = ((x as f32 + 0.5) / w as f32) * st[0] + st[2];
            let v = ((y as f32 + 0.5) / h as f32) * st[1] + st[3];
            let s = sample_bilinear_clamp(src, u, v);
            for k in 0..4 {
                out.set(x, y, k, s[k as usize]);
            }
        }
    }
    out
}

/// A bilinear sample of `src` at normalised (u, v), clamp addressing, no mips (the copies' inputs have one
/// level); the weights come out exactly (1, 0) on texel centres.
pub fn sample_bilinear_clamp(src: &Buf, u: f32, v: f32) -> [f32; 4] {
    let fx = u * src.w as f32 - 0.5;
    let fy = v * src.h as f32 - 0.5;
    let x0 = fx.floor();
    let y0 = fy.floor();
    let (tx, ty) = (fx - x0, fy - y0);
    let cl = |a: f32, n: u32| (a.max(0.0).min(n as f32 - 1.0)) as i64;
    let (xa, xb) = (cl(x0, src.w), cl(x0 + 1.0, src.w));
    let (ya, yb) = (cl(y0, src.h), cl(y0 + 1.0, src.h));
    let p00 = ld(src, xa, ya);
    let p10 = ld(src, xb, ya);
    let p01 = ld(src, xa, yb);
    let p11 = ld(src, xb, yb);
    let mut o = [0.0f32; 4];
    for k in 0..4 {
        let top = p00[k] * (1.0 - tx) + p10[k] * tx;
        let bot = p01[k] * (1.0 - tx) + p11[k] * tx;
        o[k] = top * (1.0 - ty) + bot * ty;
    }
    o
}

/// Apply a render-target write mask: channels whose bit is set in `mask` take `src`'s value (after `store`
/// rounding to f16), the others keep `dst`'s.
pub fn write_masked(dst: &Buf, src: &Buf, mask: u32, store: Rounding) -> Buf {
    let mut out = dst.clone();
    for y in 0..dst.h {
        for x in 0..dst.w {
            for k in 0..dst.channels.min(4) {
                if mask & (1 << k) != 0 {
                    out.set(x, y, k, quantise_f16(src.get(x, y, k), store));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf4(w: u32, h: u32, f: impl Fn(u32, u32) -> [f32; 4]) -> Buf {
        let mut b = Buf::new(w, h, 4);
        for y in 0..h {
            for x in 0..w {
                let v = f(x, y);
                for k in 0..4 {
                    b.set(x, y, k, v[k as usize]);
                }
            }
        }
        b
    }

    #[test]
    fn resolve_uncovered_texel_is_copied_covered_texel_is_normalised() {
        let b = buf4(1, 2, |_, y| if y == 0 { [0.5, 0.25, 0.125, 0.005] } else { [2.0, 1.0, 0.5, 1.0009766] });
        assert_eq!(resolve_ps25113_texel(&b, 0, 0, false), [0.5, 0.25, 0.125, 0.005]);
        let r = resolve_ps25113_texel(&b, 0, 1, false);
        assert_eq!(r, [2.0 / 1.0009766, 1.0 / 1.0009766, 0.5 / 1.0009766, 1.0]);
    }

    #[test]
    fn partial_texel_takes_the_single_qualifying_neighbour_two_deep() {
        // row y = 1: [full 3.0 | full 3.0 | partial | 0 | 0]; the partial texel at x = 2 has one full neighbour
        // (x = 1) whose next texel (x = 0) is full too → it takes the neighbour's rgb / w
        let b = buf4(5, 3, |x, y| {
            if y != 1 { return [0.0; 4]; }
            match x { 0 | 1 => [3.0, 1.5, 0.75, 1.0], 2 => [0.5, 0.5, 0.5, 0.5], _ => [0.0; 4] }
        });
        assert_eq!(resolve_ps25113_texel(&b, 2, 1, false), [3.0, 1.5, 0.75, 1.0]);
        // with a second full neighbour (x = 3 full, x = 4 full) the count is 2 → its own rgb / w
        let b2 = buf4(5, 3, |x, y| {
            if y != 1 { return [0.0; 4]; }
            match x { 0 | 1 | 3 | 4 => [3.0, 1.5, 0.75, 1.0], 2 => [0.5, 0.5, 0.5, 0.5], _ => [0.0; 4] }
        });
        assert_eq!(resolve_ps25113_texel(&b2, 2, 1, false), [1.0, 1.0, 1.0, 1.0]);
        // a full neighbour whose next texel is NOT full counts but does not become the candidate → own rgb / w
        let b3 = buf4(5, 3, |x, y| {
            if y != 1 { return [0.0; 4]; }
            match x { 1 => [3.0, 1.5, 0.75, 1.0], 2 => [0.5, 0.5, 0.5, 0.5], _ => [0.0; 4] }
        });
        assert_eq!(resolve_ps25113_texel(&b3, 2, 1, false), [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn scale_and_copy_are_exact_on_f16_values() {
        let b = buf4(4, 4, |x, y| [x as f32 * 0.125, y as f32 * 0.25, 0.3330078, 0.7778320]);
        let s = scale_ps1109(&b, [2.0, 2.0, 2.0, 0.0], Rounding::Truncate, Rounding::NearestEven);
        assert_eq!(s.get(3, 2, 0), 0.75);
        assert_eq!(s.get(3, 2, 2), 0.6660156);
        assert_eq!(s.get(3, 2, 3), 0.0);
        let c = copy_ps1034(&s, [1.0, 1.0, 0.0, 0.0], 4, 4);
        for y in 0..4 { for x in 0..4 { for k in 0..4 { assert_eq!(c.get(x, y, k), s.get(x, y, k)); } } }
        let m = write_masked(&b, &c, 7, Rounding::Truncate);
        assert_eq!(m.get(3, 2, 0), 0.75);
        assert_eq!(m.get(3, 2, 3), 0.7778320);
    }
}

/// PS 1109 × `ScaleSrc` blended One/One onto a target that already holds the previous sweep's finalised image
/// (frame 74490 eids into 24911 / 24914 / 24917 / 24920): `f16_rtne(dst + f16_rtz(src × scale))` per channel —
/// the lightmap is 2·Σ_sweeps C_k.
pub fn add_scaled_ps1109(dst: &Buf, src: &Buf, scale: [f32; 4]) -> Buf {
    let mut out = Buf::new(src.w, src.h, 4);
    for y in 0..src.h {
        for x in 0..src.w {
            let v = ld(src, x as i64, y as i64);
            for k in 0..4 {
                let s = quantise_f16(v[k] * scale[k], Rounding::Truncate);
                out.set(x, y, k as u32, quantise_f16(dst.get(x, y, k as u32) + s, Rounding::NearestEven));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// THE SWEEPS' FINALISATION AS A LIBRARY — from a bake's own H-basis accumulation targets to the finalised (×2-added)
// coefficient images `e2e::finalise_tail` consumes (E; the sequence and the rounding are the inline chain's, which
// reproduces the captured finalisation frame 74490 pass by pass)
// ---------------------------------------------------------------------------------------------------------------

/// An H-basis accumulation target (the four RGBA16F MRTs of one sweep) as passdiff buffers.
pub fn mrt_bufs(hb: &crate::lmaccum::HbTargets) -> [Buf; 4] {
    let one = |m: usize| -> Buf {
        let mut b = Buf::new(hb.w, hb.h, 4);
        for i in 0..(hb.w * hb.h) as usize {
            for c in 0..4 {
                b.data[i * 4 + c] = hb.mrt[m][i][c];
            }
        }
        b
    };
    [one(0), one(1), one(2), one(3)]
}

/// The finalised images of a bake: per coefficient image Σ over the sweeps of 2 · PS 25113(the sweep's MRT) — the game
/// adds each sweep's resolved image × 2 (PS 1109) into the previous sweep's finalised targets (RGBA16F: the source
/// truncated, the sum RTNE); the alpha channel carries the LAST sweep's resolve alpha (1 where covered) for the tail's
/// PS 1034 target.
pub fn finalise_sweeps(sweeps: &[crate::lmaccum::HbTargets]) -> [Buf; 4] {
    let (w, h) = sweeps.first().map(|s| (s.w, s.h)).unwrap_or((2048, 2048));
    // the four MRTs are independent: one thread each (perf 8)
    let out: Vec<Buf> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..4).map(|m| sc.spawn(move || {
            let mut acc = Buf::new(w, h, 4);
            let mut last: Option<Buf> = None;
            for hb in sweeps {
                let res = resolve_ps25113(&mrt_bufs(hb)[m], false, Rounding::Truncate);
                acc = add_scaled_ps1109(&acc, &res, [2.0, 2.0, 2.0, 0.0]);
                last = Some(res);
            }
            if let Some(res) = last {
                for i in 0..(w * h) as usize {
                    acc.data[i * 4 + 3] = res.data[i * 4 + 3];
                }
            }
            acc
        })).collect();
        hs.into_iter().map(|h| h.join().expect("finalise")).collect()
    });
    let mut it = out.into_iter();
    [it.next().unwrap(), it.next().unwrap(), it.next().unwrap(), it.next().unwrap()]
}

/// BlueBay Day's `Mood_MaxHdr` as the captured encode cbuffer carries it.
pub const MOOD_MAX_HDR_BLUEBAY_DAY: f32 = 7.519885063171387;
