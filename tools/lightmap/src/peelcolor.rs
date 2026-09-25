//! THE PEEL'S COLOUR PATH (rows 5/6, port engineer D): PS 17131 / 17134 (items, cards) — the DXBC:
//!
//! ```text
//!   sample r0.xyz, v1.xyxx, TMapILightInput.xyzw, SGbxClamp_Aniso       (v1.xy = the LM uv: TexCoord1 through the chart ST, VS 17130/17133 o1.xy)
//!   max r0.y, r0.y, l(0.00001)
//!   and o0.xyz, r0.xyzx, isfrontface
//! ```
//!
//! The texture is the ILightInput atlas (17095, R11G11B10 2048², one mip level: the dilated
//! sun_direct/w × sRGB⁻¹(MDiffuse8) of the compute frame — engineer A's chain, `ilightin.rs`); the
//! sampler SGbxClamp_Aniso is anisotropic ×16 with ClampEdge addressing. At the fitted peel's
//! magnification (1 cm pixels over 3 cm atlas texels) the filter is one bilinear tap; the world peel
//! (0.6 m pixels) minifies — up to 16 bilinear taps along the footprint's major axis, all on the one
//! level. `AtlasTex::sample` follows D3D11's filter (8-bit fractional weights).
//!
//! THE CHART ST (`chart_st`): VS 17133 `o1.xy = v3.xy × ST.xy + ST.zw` with ST from g_InstanceDatas
//! (the items' vertices carry chart index 0xffff → the instance's own ST). Against the capture's
//! instance buffer (frame 127448, `lmtool lm-st`) the mapping rect (x, y, w, h — layout units of the
//! 2048² atlas) and the model's PreLightGen uv bounds (b0, b1, b2, b3) give it exactly:
//!
//! ```text
//!   S = (w − 1/4) / 2048 / (b2 − b0)           T = (x + 1/8) / 2048 − b0 · S      (same for y with h, b1, b3)
//! ```
//!
//! three items, both axes, within 1e-6 (e.g. the plate: rect (1023, 1, 508, 512), bounds (0.04799,
//! 0.05279, 0.94706, 0.95945) → captured (0.2757549, 0.2756018, 0.4863398, −0.0139989)). The chart's
//! usable span is x + 1/8 … x + w − 1/8 layout pixels: the model's uv bounds map onto the rect with an
//! eighth-pixel inset.

/// A 2048² (or any) RGB f32 texture with one level, sampled like the GPU samples the ILightInput.
#[derive(Clone, Debug)]
pub struct AtlasTex {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<[f32; 3]>,
}

impl AtlasTex {
    pub fn new(w: usize, h: usize, rgb: Vec<[f32; 3]>) -> AtlasTex {
        assert_eq!(rgb.len(), w * h);
        AtlasTex { w, h, rgb }
    }

    /// From a capture buffer (`passdiff::Buf`: 3 or more channels).
    pub fn from_buf(b: &crate::passdiff::Buf) -> AtlasTex {
        let mut rgb = Vec::with_capacity((b.w * b.h) as usize);
        for y in 0..b.h {
            for x in 0..b.w {
                rgb.push([b.get(x, y, 0), b.get(x, y, 1), b.get(x, y, 2)]);
            }
        }
        AtlasTex { w: b.w as usize, h: b.h as usize, rgb }
    }

    #[inline]
    fn texel(&self, x: i64, y: i64) -> [f32; 3] {
        let xc = x.clamp(0, self.w as i64 - 1) as usize;
        let yc = y.clamp(0, self.h as i64 - 1) as usize;
        self.rgb[yc * self.w + xc]
    }

    /// One bilinear tap (D3D11: texel coordinate u·w − 0.5, the fraction as an 8-bit weight, ClampEdge).
    pub fn sample_bilinear(&self, u: f32, v: f32) -> [f32; 3] {
        let fx = u.clamp(0.0, 1.0) * self.w as f32 - 0.5;
        let fy = v.clamp(0.0, 1.0) * self.h as f32 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let tx = ((fx - x0) * 256.0).floor() / 256.0;
        let ty = ((fy - y0) * 256.0).floor() / 256.0;
        let (xa, ya) = (x0 as i64, y0 as i64);
        let (a, b, c, d) = (self.texel(xa, ya), self.texel(xa + 1, ya), self.texel(xa, ya + 1), self.texel(xa + 1, ya + 1));
        let mut out = [0f32; 3];
        for k in 0..3 {
            out[k] = (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty;
        }
        out
    }

    /// The anisotropic sample of a one-level texture: `taps` bilinear taps spread along the footprint's
    /// major axis (`axis` = the whole extent in texture coordinates), averaged.
    pub fn sample_aniso(&self, u: f32, v: f32, axis: [f32; 2], taps: usize) -> [f32; 3] {
        let n = taps.max(1);
        if n == 1 {
            return self.sample_bilinear(u, v);
        }
        let mut sum = [0f32; 3];
        for i in 0..n {
            let s = (i as f32 + 0.5) / n as f32 - 0.5;
            let c = self.sample_bilinear(u + axis[0] * s, v + axis[1] * s);
            for k in 0..3 {
                sum[k] += c[k];
            }
        }
        [sum[0] / n as f32, sum[1] / n as f32, sum[2] / n as f32]
    }
}

/// The chart ST of an instance from its layout rect (2048-unit layout: x, y, w, h) and the model's
/// PreLightGen uv bounds — `uv_lm = uv1 × ST.xy + ST.zw` (see the module doc for the derivation).
pub fn chart_st(rect: [i32; 4], bounds: [f32; 4], atlas: f32) -> [f32; 4] {
    let (x, y, w, h) = (rect[0] as f32, rect[1] as f32, rect[2] as f32, rect[3] as f32);
    let (du, dv) = ((bounds[2] - bounds[0]).max(1e-9), (bounds[3] - bounds[1]).max(1e-9));
    // S = (w − ¼) · (1/atlas) · rcp(b_hi − b_lo) — a RECIPROCAL multiply, not a division: the captured 4096 tile
    // instances' S.y are bit-identical only under this order (the division forms match 1384 of 4096; S.x matches under
    // every order); T = (x + ⅛)/atlas − b_lo·S with the product and the subtraction separate (the fused form misses 23)
    let sx = (w - 0.25) * (1.0 / atlas) * (1.0 / du);
    let sy = (h - 0.25) * (1.0 / atlas) * (1.0 / dv);
    [sx, sy, (x + 0.125) / atlas - bounds[0] * sx, (y + 0.125) / atlas - bounds[1] * sy]
}

/// The peel pixel shader's colour: the atlas at the LM uv (one bilinear tap when the footprint is a
/// texel or less, else the anisotropic taps), G floored at 1e-5, zero for a back face.
pub fn peel_color(atlas: &AtlasTex, uv_lm: [f32; 2], axis: [f32; 2], taps: usize, front_face: bool) -> [f32; 3] {
    if !front_face {
        return [0.0; 3];
    }
    let s = atlas.sample_aniso(uv_lm[0], uv_lm[1], axis, taps);
    [s[0], s[1].max(1e-5), s[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chart_st_reproduces_the_captured_instance_buffer() {
        // frame 127448 (lmtool lm-st): the plate AC16236083, the palm AV16236014, the wall AC16236052
        let cases: [([i32; 4], [f32; 4], [f32; 4]); 3] = [
            ([1023, 1, 508, 512], [0.04799, 0.05279, 0.94706, 0.95945], [0.2757549, 0.2756018, 0.4863398, -0.0139989]),
            ([511, 1, 510, 512], [0.00100, 0.00100, 0.99713, 0.99900], [0.2498691, 0.2503787, 0.2493229, 0.0002989]),
            ([1, 1, 508, 2042], [0.00100, 0.00100, 0.25050, 0.99900], [0.9936858, 0.9989461, -0.0004444, -0.0004496]),
        ];
        for (rect, b, want) in cases {
            let st = chart_st(rect, b, 2048.0);
            for k in 0..4 {
                assert!((st[k] - want[k]).abs() < 2e-5, "rect {rect:?}: ST {st:?} vs captured {want:?}");
            }
        }
    }

    #[test]
    fn bilinear_reads_texel_centres_and_clamps_at_the_edge() {
        let mut rgb = vec![[0.0f32; 3]; 4];
        rgb[1] = [1.0, 2.0, 3.0]; // texel (1, 0)
        let t = AtlasTex::new(2, 2, rgb);
        let c = t.sample_bilinear(0.75, 0.25);
        assert!((c[0] - 1.0).abs() < 1e-6 && (c[1] - 2.0).abs() < 1e-6);
        // half-way between texels (0,0) and (1,0): the mean
        let m = t.sample_bilinear(0.5, 0.25);
        assert!((m[2] - 1.5).abs() < 1e-6, "{m:?}");
        // beyond the edge: the last column
        let e = t.sample_bilinear(1.5, 0.25);
        assert!((e[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_back_face_is_black_and_g_is_floored() {
        let t = AtlasTex::new(1, 1, vec![[0.5, 0.0, 0.25]]);
        assert_eq!(peel_color(&t, [0.5, 0.5], [0.0, 0.0], 1, false), [0.0; 3]);
        let c = peel_color(&t, [0.5, 0.5], [0.0, 0.0], 1, true);
        assert!((c[0] - 0.5).abs() < 1e-6 && (c[1] - 1e-5).abs() < 1e-9 && (c[2] - 0.25).abs() < 1e-6);
    }
}
