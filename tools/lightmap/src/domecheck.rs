//! THE SKY DOME'S LAST QUANTUM — `lmtool dome-check PASSCAP [--frame 127448] [--direction 0] [--filter f32|f16|f16sum]
//! [--weights floor|round|f32] [--rsq exact|approx] [--top N]`: PS 16774 (Pixel_16774.txt, 29 instructions) transcribed
//! instruction by instruction on the game's own inputs of the frame's environment render — the dome mesh e001051 rasterised
//! in the world peel's frustum (`domemesh`), the two BC6H gradient textures (env/frame127448/textures/e001051_16801/16803.dds,
//! sampler Mirror U / ClampEdge V / linear), the ShaderP g_CBuffer and SceneP constants of the draw (logs/draws-frame127448)
//! — quantised to R11G11B10 by truncation and compared with the captured environment layer (peel_color layer 0 of the
//! direction) on every pixel the captured depth marks as the dome. The switches are the study: the texture filter's
//! arithmetic (f32; every product/sum rounded to f16; the weighted sum rounded to f16 once), the bilinear weights' precision,
//! the reciprocal square root.

use crate::passdiff::Buf;

/// The draw's constants (eid 1051 of frame 127448 — read from the draws log by `dome_check`).
#[derive(Clone, Debug)]
pub struct DomeConsts {
    pub scale_grad0: f32,
    pub scale_grad1: f32,
    pub sun_power: f32,
    pub sun_is_visible: bool,
    pub pow_scale: [f32; 4],
    pub rgb1: [f32; 3],
    pub rgb2: [f32; 3],
    pub fog_intens: f32,
    pub global_scale: f32,
    pub light_dir: [f32; 3],
    pub light_rgb: [f32; 3],
    pub fog_rgb: [f32; 3],
    pub eye: [f32; 3],
    pub light_dir_angle: f32,
    pub force_x: f32,
    pub invert_y: bool,
}

/// A decoded gradient texture (the BC6H_UFLOAT texels as the f16 values the decoder yields).
pub struct GradTex {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 3]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterArith {
    F32,
    /// Every product and sum rounded to f16 (RNE).
    F16Each,
    /// The four products summed in f32, the result rounded to f16.
    F16Sum,
    /// Each product rounded to f16, the sum in f32, the result rounded to f16.
    F16Prod,
    /// Two x-lerps (p0 + t·(p1 − p0)) rounded to f16, the y-lerp rounded to f16.
    F16Lerp,
    /// The f32 result rounded to f16 with the two x-lerps in f32 and the y-lerp fused.
    F16Fma,
    /// The four products summed in f32, the result TRUNCATED to f16.
    F16SumRtz,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeightPrec {
    Floor8,
    Round8,
    F32,
    /// 9 fractional bits, truncated.
    Floor9,
    /// 8 fractional bits of the coordinate fraction, ROUNDED to nearest even.
    Even8,
    /// The texel coordinate (u·w) itself quantised to 1/256 (round to nearest even) BEFORE the −0.5 texel-centre shift.
    Coord8,
    /// The texel coordinate quantised to 1/256 by truncation before the shift.
    Coord8Floor,
}

#[derive(Clone, Copy, Debug)]
pub struct Study {
    pub filter: FilterArith,
    pub weights: WeightPrec,
    pub rsq_approx: bool,
    /// The texel centre convention: coordinate·size − 0.5 (D3D) — kept for completeness.
    pub half: f32,
    /// With `WeightPrec::Even8`: the number of fractional bits (8 = 1/256).
    pub frac_bits: u32,
}

fn f16r(v: f32) -> f32 {
    crate::gpufmt::quantise_f16(v, crate::gpufmt::Rounding::NearestEven)
}

impl GradTex {
    /// `sample_indexable` with SMapGradientV: Mirror U, ClampEdge V, linear, one mip.
    pub fn sample(&self, uv: [f32; 2], st: &Study) -> [f32; 3] {
        // address: u mirrored on [0, 1], v clamped
        let m = uv[0].rem_euclid(2.0);
        let u = if m > 1.0 { 2.0 - m } else { m };
        let v = uv[1].clamp(0.0, 1.0);
        let scale = (1u32 << st.frac_bits) as f32;
        let q8 = |c: f32| -> f32 { let s = c * scale; let f = s.floor(); let d = s - f; (if d > 0.5 { f + 1.0 } else if d < 0.5 { f } else if (f as i64) % 2 == 0 { f } else { f + 1.0 }) / scale };
        let (cu, cv) = match st.weights { WeightPrec::Coord8 => (q8(u * self.w as f32), q8(v * self.h as f32)), WeightPrec::Coord8Floor => ((u * self.w as f32 * 256.0).floor() / 256.0, (v * self.h as f32 * 256.0).floor() / 256.0), _ => (u * self.w as f32, v * self.h as f32) };
        let fx = cu - st.half;
        let fy = cv - st.half;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (mut tx, mut ty) = (fx - x0, fy - y0);
        match st.weights {
            WeightPrec::Floor8 => { tx = (tx * 256.0).floor() / 256.0; ty = (ty * 256.0).floor() / 256.0; }
            WeightPrec::Round8 => { tx = (tx * 256.0).round() / 256.0; ty = (ty * 256.0).round() / 256.0; }
            WeightPrec::F32 => {}
            WeightPrec::Floor9 => { tx = (tx * 512.0).floor() / 512.0; ty = (ty * 512.0).floor() / 512.0; }
            WeightPrec::Even8 => { tx = q8(tx); ty = q8(ty); }
            WeightPrec::Coord8 | WeightPrec::Coord8Floor => {}
        }
        let xi = |x: f32| -> usize {
            // mirror addressing of the texel index
            let n = self.w as i64;
            let mut i = x as i64;
            let period = 2 * n;
            i = i.rem_euclid(period);
            if i >= n { i = period - 1 - i; }
            i as usize
        };
        let yi = |y: f32| -> usize { (y.max(0.0) as usize).min(self.h - 1) };
        let (xa, xb) = (xi(x0), xi(x0 + 1.0));
        let (ya, yb) = (yi(y0), yi(y0 + 1.0));
        let p = |x: usize, y: usize| -> [f32; 3] { self.px[y * self.w + x] };
        let (p00, p10, p01, p11) = (p(xa, ya), p(xb, ya), p(xa, yb), p(xb, yb));
        let (w00, w10, w01, w11) = ((1.0 - tx) * (1.0 - ty), tx * (1.0 - ty), (1.0 - tx) * ty, tx * ty);
        let mut out = [0f32; 3];
        for k in 0..3 {
            out[k] = match st.filter {
                FilterArith::F32 => (p00[k] * (1.0 - tx) + p10[k] * tx) * (1.0 - ty) + (p01[k] * (1.0 - tx) + p11[k] * tx) * ty,
                FilterArith::F16Each => {
                    let a = f16r(f16r(p00[k] * (1.0 - tx)) + f16r(p10[k] * tx));
                    let b = f16r(f16r(p01[k] * (1.0 - tx)) + f16r(p11[k] * tx));
                    f16r(f16r(a * (1.0 - ty)) + f16r(b * ty))
                }
                FilterArith::F16Sum => f16r(p00[k] * w00 + p10[k] * w10 + p01[k] * w01 + p11[k] * w11),
                FilterArith::F16Prod => f16r(f16r(p00[k] * w00) + f16r(p10[k] * w10) + f16r(p01[k] * w01) + f16r(p11[k] * w11)),
                FilterArith::F16Lerp => { let a = f16r(p00[k] + tx * (p10[k] - p00[k])); let b = f16r(p01[k] + tx * (p11[k] - p01[k])); f16r(a + ty * (b - a)) }
                FilterArith::F16Fma => { let a = p00[k] + tx * (p10[k] - p00[k]); let b = p01[k] + tx * (p11[k] - p01[k]); f16r(ty.mul_add(b - a, a)) }
                FilterArith::F16SumRtz => crate::gpufmt::quantise_f16(p00[k] * w00 + p10[k] * w10 + p01[k] * w01 + p11[k] * w11, crate::gpufmt::Rounding::Truncate),
            };
        }
        out
    }
}

/// PS 16774 on one pixel's interpolated attributes: `uv` = v1 (o1 of VS 16773), `view` = v2 (world − eye).
pub fn ps_16774(uv: [f32; 2], view: [f32; 3], c: &DomeConsts, grad0: &GradTex, grad1: &GradTex, st: &Study) -> [f32; 3] {
    // 0-2: r0.xyz = normalize(v2) via dp3 + rsq + mul
    let len2 = view[0] * view[0] + view[1] * view[1] + view[2] * view[2];
    // the GPU's rsq: 1/sqrt correctly rounded (the exact form); the approximate form = the value one ulp below (a stand-in
    // for MUFU.RSQ's bias, the study)
    let rsq = if st.rsq_approx { let r = 1.0 / len2.sqrt(); f32::from_bits(r.to_bits() - 1) } else { 1.0 / len2.sqrt() };
    let n = [rsq * view[0], rsq * view[1], rsq * view[2]];
    // 3-4: r1 = TMapGradientV(uv) · ScaleGrad0
    let g0 = grad0.sample(uv, st);
    let mut r1 = [g0[0] * c.scale_grad0, g0[1] * c.scale_grad0, g0[2] * c.scale_grad0];
    // 5-9: if 1e-6 < ScaleGrad1: r1 = mad(ScaleGrad1, TMapGradientV1(uv), r1)
    if 0.000001 < c.scale_grad1 {
        let g1 = grad1.sample(uv, st);
        for k in 0..3 {
            r1[k] = c.scale_grad1.mul_add(g1[k], r1[k]);
        }
    }
    // 10-12: r0.x = log2(max(0, n · −LightDir))
    let d = n[0] * -c.light_dir[0] + n[1] * -c.light_dir[1] + n[2] * -c.light_dir[2];
    let d = d.max(0.0);
    let lg = d.log2(); // −inf at 0
    // 13-17: the sun disc: exp2(lg · SunPower) · SunPower · LightDirRgb + r1, kept only when SunIsVisible
    let sun = (lg * c.sun_power).exp2() * c.sun_power;
    let mut r0 = [0f32; 3];
    for k in 0..3 {
        let with_sun = sun.mul_add(c.light_rgb[k], r1[k]);
        r0[k] = if c.sun_is_visible { with_sun } else { r1[k] };
    }
    // 18-22: the two atmo lobes: r1.xy = exp2(lg · PowScale.xz) · PowScale.yw; r0 = mad(r1.x, Rgb1, r0); r0 = mad(r1.y, Rgb2, r0)
    let l1 = (lg * c.pow_scale[0]).exp2() * c.pow_scale[1];
    let l2 = (lg * c.pow_scale[2]).exp2() * c.pow_scale[3];
    for k in 0..3 {
        r0[k] = l1.mul_add(c.rgb1[k], r0[k]);
    }
    for k in 0..3 {
        r0[k] = l2.mul_add(c.rgb2[k], r0[k]);
    }
    // 23-25: w = sat(1 − FogIntens); r0 = mad(w, r0 − Fog, Fog)
    let w = (1.0 - c.fog_intens).clamp(0.0, 1.0);
    for k in 0..3 {
        let t = r0[k] - c.fog_rgb[k];
        r0[k] = w.mul_add(t, c.fog_rgb[k]);
    }
    // 26-27: · GlobalScale, min 16375
    for k in 0..3 {
        r0[k] = (r0[k] * c.global_scale).min(16375.0);
    }
    r0
}

/// The per-channel comparison in R11G11B10 storage steps.
#[derive(Default, Debug)]
pub struct QuantaStats {
    pub n: usize,
    pub exact: [usize; 3],
    pub within1: [usize; 3],
    pub worse: [usize; 3],
    pub plus1: [usize; 3],
    pub minus1: [usize; 3],
}

/// Storage step index of a channel value in its UF11/UF10 encoding (the mantissa count from zero, as an ordering key).
pub fn r11_steps(v: f32, bits10: bool) -> i64 {
    // pack the single channel through the R11G11B10 packer and read the field back as an integer
    let packed = crate::gpufmt::pack_r11g11b10(if bits10 { [0.0, 0.0, v] } else { [v, 0.0, 0.0] }, crate::gpufmt::Rounding::Truncate);
    if bits10 { (packed >> 22) as i64 } else { (packed & 0x7ff) as i64 }
}

pub fn compare_quanta(ours: [f32; 3], game: [f32; 3], s: &mut QuantaStats) {
    s.n += 1;
    for k in 0..3 {
        let (a, b) = (r11_steps(ours[k], k == 2), r11_steps(game[k], k == 2));
        let d = a - b;
        if d == 0 { s.exact[k] += 1; }
        if d.abs() <= 1 { s.within1[k] += 1; } else { s.worse[k] += 1; }
        if d == 1 { s.plus1[k] += 1; }
        if d == -1 { s.minus1[k] += 1; }
    }
}

pub fn load_grad(passcap: &std::path::Path, frame: u32, id: u32) -> Result<GradTex, String> {
    let p = passcap.join(format!("env/frame{frame}/textures/e001051_{id}.dds"));
    let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    // the DDS header: 4 ('DDS ') + 124 + the DX10 header (20) for BC6H
    let (w, h) = (u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize, u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize);
    let fourcc = &bytes[84..88];
    let off = if fourcc == b"DX10" { 128 + 20 } else { 128 };
    let px = crate::bc6h::decode_image(&bytes[off..], w, h, false);
    Ok(GradTex { w, h, px })
}

pub fn grad_summary(g: &GradTex) -> String {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &g.px { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
    format!("{}×{}: min {:?} max {:?}", g.w, g.h, lo, hi)
}

/// `Buf` view helper for the comparison (the captured R11G11B10 layer decodes to f32 exactly).
pub fn buf_rgb(b: &Buf, x: u32, y: u32) -> [f32; 3] {
    [b.get(x, y, 0), b.get(x, y, 1), b.get(x, y, 2)]
}
