//! ROW 10 — the PROBE passes of the game's lightmapper, TRANSCRIBED from the capture
//! (`passcap/pwc-day`, frame 127448 = sweep 0's first direction; shader ids of that frame).
//!
//! The probe volume is a 32×16×32 **3D render target** (RGBA16F, one texel per probe of the block:
//! `x` = cell axis 0, `y` = the height level, `z` = the slice = cell axis 2 — the trailer's block
//! record of map-lightmap.md §3.8). Every probe draw is `Draw(3·n)` with a null VS (1111: `ret`), the
//! geometry shader 1113 turning input triangle `p` into a full-viewport triangle ((−1.01, −1.01),
//! (3.03, −1.01), (−1.01, 3.03)) on render-target array slice `iSliceStart + p` (cb `ShaderG.g_CBufferG`),
//! the viewport (0, 0, 32, 16) and a SCISSOR rect = the block's occupied cell range in (x, y) — for
//! pwc-day (22, 4, 7, 8) with `iSliceStart` 20 and 7 triangles: exactly the baked map's block
//! `min (22, 4, 20) max (29, 12, 27)`.
//!
//! Per direction D the game issues, in the WORLD peel phase only (the fitted phase draws no probes):
//!
//! * after every layer k (the environment block k = 0, then the geometry layers), one
//!   `ProbeGrid_SetILightDir` draw — **PS 17151** (pwc1: 17565) into the RGBA16F volume 17157, no
//!   blend, cb `ShaderP.g_CBufferP {ProbeToShadow float4x3, Padding, OutScale = 1}`, SRVs t0 = the
//!   layer's peel colour (R11G11B10 4096²), t1 = its depth (D16 as R16_UNORM), t2 = `TMapProbeSafetyOffset`
//!   (3D 32×16×32 R16G16B16A16_SNORM 17045: the per-probe position offset in cells), samplers
//!   s0 `SGbxClamp_Point` (ClampEdge, point), s1 `SGbxClamp_Point_Cmp` (ClampEdge, point, comparison
//!   GreaterEqual — the accumulate's pair, dumped at the draw):
//!   ```text
//!   p    = utof(uint3(pixel.x, pixel.y, slice)) + SafetyOffset[x, y, slice]        (cells)
//!   uv   = (p, 1) · ProbeToShadow[:, 0..2]      depth = (p, 1) · ProbeToShadow[:, 2]  (dp4 per register)
//!   vis  = TMapShadow.SampleCmpLevelZero(s1, uv, depth)     (1 where depth ≥ stored: the layer is
//!                                                            at or beyond the probe along D)
//!   rgb  = TMapColorPeeled.Sample(s0, uv)
//!   if (vis − 0.5 < 0) discard;                              (the layer is behind the probe: keep)
//!   a    = (r + g + b > 1e-6) ? 1 : 0
//!   out  = float4(rgb, a) · OutScale
//!   ```
//!   Layers are peeled far-to-near from the probe's viewpoint (k = 0 the sky/terrain, then the
//!   surface nearest the sky, then deeper ones), each passing layer overwrites, so after the last layer
//!   a probe holds the colour of the FIRST surface it sees along D (or the dome) and α = 1 where that
//!   colour is not black (front faces; back faces and the sweep-0 geometry are black).
//!   `ProbeToShadow` = ProbeToWorld · WorldPw01Shadow with ProbeToWorld = 16·cell + block.pos
//!   (pwc-day: pos = (472, −46, −8), the trailer's block record; measured off the cbuffer to 1e-5).
//! * after layer 1 only (the first geometry layer = the surface nearest the sky in every pixel
//!   column) one `ProbeGrid_AddSkyVisibility` draw — **PS 17154** into the R16_FLOAT volume 17160,
//!   blend One/One, cb {ProbeToShadow, OutScale = 4·D.y/N}, s0 `SMapShadow` (ClampEdge, LINEAR
//!   comparison GreaterEqual — the direct-sun PCF sampler): `out = (SampleCmp(uv, depth) < 0.5) ? OutScale
//!   : 0` — the probe is above the topmost geometry along D → it sees the sky → += 4·D.y/N.
//! * after the direction's H-basis draws, two `ProbeGrid` folds — **PS 1112** (`out = TMapInput[x, y,
//!   slice] · ScaleSrc`, `ld` of the 3D source, all 32 slices, no scissor), blend One/One:
//!   17056 += 17157 · (2/N, 2/N, 2/N, 1/N) and 17059 += 17157 · (4·D.y/N) (all four channels); then 17157
//!   is cleared for the next direction. 17056 / 17059 / 17160 run over the whole bake (cleared once in
//!   the first compute frame) and are the three probe images the CPU downloads (§3.9 of
//!   map-lightmap.md).
//!
//! Hardware rules used (the baker's, measured on the other rows): an un-blended RGBA16F store
//! TRUNCATES; a One/One f16 blend truncates the source to f16 then rounds the sum to nearest-even;
//! SampleCmp on a UNORM depth clamps the reference to [0, 1] (D3D11 §7.18); point sampling takes the
//! texel floor(u·W) (ClampEdge: clamped to the edge texel); the linear comparison filter is the
//! 2×2 bilinear blend of the four texels' compare results with the fraction at 8 bits.

use crate::gpufmt::{decode_f16, encode_f16, Rounding};
use crate::passdiff::Buf;

/// A 3D texture as f32 channels, `data[((z·h + y)·w + x)·channels + c]`.
#[derive(Clone, Debug)]
pub struct Volume3 {
    pub w: u32,
    pub h: u32,
    pub d: u32,
    pub channels: u32,
    pub data: Vec<f32>,
}

impl Volume3 {
    pub fn new(w: u32, h: u32, d: u32, channels: u32) -> Volume3 {
        Volume3 { w, h, d, channels, data: vec![0.0; (w * h * d * channels) as usize] }
    }
    #[inline]
    pub fn idx(&self, x: u32, y: u32, z: u32, c: u32) -> usize {
        (((z * self.h + y) * self.w + x) * self.channels + c) as usize
    }
    #[inline]
    pub fn get(&self, x: u32, y: u32, z: u32, c: u32) -> f32 {
        self.data[self.idx(x, y, z, c)]
    }
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, z: u32, c: u32, v: f32) {
        let i = self.idx(x, y, z, c);
        self.data[i] = v;
    }
    /// Texels with any non-zero channel.
    pub fn count_nonzero(&self) -> usize {
        self.data.chunks_exact(self.channels as usize).filter(|p| p.iter().any(|&v| v != 0.0)).count()
    }
}

/// The DXGI formats a probe volume comes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolFmt {
    Rgba16F,
    R16F,
    Rgba16Snorm,
    Rgba32F,
    R32F,
}

impl VolFmt {
    pub fn from_dxgi(id: u32) -> Option<VolFmt> {
        match id {
            10 => Some(VolFmt::Rgba16F),
            54 => Some(VolFmt::R16F),
            13 => Some(VolFmt::Rgba16Snorm),
            2 => Some(VolFmt::Rgba32F),
            41 => Some(VolFmt::R32F),
            _ => None,
        }
    }
    pub fn from_name(name: &str) -> Option<VolFmt> {
        let n = name.trim().to_ascii_uppercase();
        let n = n.strip_prefix("DXGI_FORMAT_").unwrap_or(&n);
        match n {
            "R16G16B16A16_FLOAT" => Some(VolFmt::Rgba16F),
            "R16_FLOAT" => Some(VolFmt::R16F),
            "R16G16B16A16_SNORM" => Some(VolFmt::Rgba16Snorm),
            "R32G32B32A32_FLOAT" => Some(VolFmt::Rgba32F),
            "R32_FLOAT" => Some(VolFmt::R32F),
            _ => None,
        }
    }
    pub fn bytes_per_texel(self) -> usize {
        match self {
            VolFmt::Rgba16F | VolFmt::Rgba16Snorm => 8,
            VolFmt::R16F => 2,
            VolFmt::Rgba32F => 16,
            VolFmt::R32F => 4,
        }
    }
    pub fn channels(self) -> u32 {
        match self {
            VolFmt::Rgba16F | VolFmt::Rgba16Snorm | VolFmt::Rgba32F => 4,
            VolFmt::R16F | VolFmt::R32F => 1,
        }
    }
}

/// D3D's SNORM16 decode: −32768 and −32767 both read −1.
#[inline]
pub fn snorm16(v: i16) -> f32 {
    (v as f32 / 32767.0).max(-1.0)
}

/// Read a DDS volume (DX10 header, `depth` slices tightly packed slice after slice, each slice
/// `h` rows of `w` texels). A DDS whose header says depth 1 but whose payload holds `depth_hint`
/// slices is accepted too (an exporter that wrote the slices back to back).
pub fn load_dds_volume(bytes: &[u8], fmt_hint: Option<VolFmt>, depth_hint: u32) -> Result<Volume3, String> {
    if bytes.len() < 128 || &bytes[..4] != b"DDS " {
        return Err("not a DDS file".into());
    }
    let u = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let flags = u(8);
    let h = u(12);
    let w = u(16);
    let mut d = if flags & 0x80_0000 != 0 { u(24).max(1) } else { 1 };
    let pf_flags = u(80);
    let fourcc = &bytes[84..88];
    let mut off = 128usize;
    let mut fmt = fmt_hint;
    if pf_flags & 0x4 != 0 && fourcc == b"DX10" {
        if bytes.len() < 148 {
            return Err("truncated DX10 header".into());
        }
        let dxgi = u(128);
        off = 148;
        if fmt.is_none() {
            fmt = VolFmt::from_dxgi(dxgi);
        }
    }
    let fmt = fmt.ok_or_else(|| "unsupported volume format".to_string())?;
    let bpt = fmt.bytes_per_texel();
    let slice_bytes = (w * h) as usize * bpt;
    let payload = &bytes[off..];
    if d == 1 && depth_hint > 1 && payload.len() >= slice_bytes * depth_hint as usize {
        d = depth_hint;
    }
    if payload.len() < slice_bytes * d as usize {
        return Err(format!("volume payload {} B < {}×{}×{} × {bpt} B", payload.len(), w, h, d));
    }
    let mut v = Volume3::new(w, h, d, fmt.channels());
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let o = ((z * h + y) * w + x) as usize * bpt;
                let px = &payload[o..o + bpt];
                let base = v.idx(x, y, z, 0);
                match fmt {
                    VolFmt::Rgba16F => {
                        for c in 0..4 {
                            v.data[base + c] = decode_f16(u16::from_le_bytes([px[c * 2], px[c * 2 + 1]]));
                        }
                    }
                    VolFmt::R16F => v.data[base] = decode_f16(u16::from_le_bytes([px[0], px[1]])),
                    VolFmt::Rgba16Snorm => {
                        for c in 0..4 {
                            v.data[base + c] = snorm16(i16::from_le_bytes([px[c * 2], px[c * 2 + 1]]));
                        }
                    }
                    VolFmt::Rgba32F => {
                        for c in 0..4 {
                            v.data[base + c] = f32::from_le_bytes(px[c * 4..c * 4 + 4].try_into().unwrap());
                        }
                    }
                    VolFmt::R32F => v.data[base] = f32::from_le_bytes(px[..4].try_into().unwrap()),
                }
            }
        }
    }
    Ok(v)
}

/// One probe draw's state: the cbuffer, the GS slice range, the scissor rect.
#[derive(Clone, Debug)]
pub struct ProbeDraw {
    pub eid: u64,
    /// `g_CBufferP.ProbeToShadow` as the DXBC sees it: register k = HLSL column k of the float4x3
    /// (column_major) = (M[0][k], M[1][k], M[2][k], M[3][k]) — the row-vector convention
    /// `(p, 1) · M` with the translation in row 3. `regs[0]`/`regs[1]` give u/v, `regs[2]` the depth.
    pub regs: [[f32; 4]; 3],
    pub out_scale: f32,
    /// `g_CBufferG.iSliceStart` and the draw's triangle count.
    pub slice_start: u32,
    pub slice_count: u32,
    /// The scissor rect (x, y, w, h) in probe cells; None = the whole 32×16 viewport.
    pub scissor: Option<[u32; 4]>,
}

impl ProbeDraw {
    /// The HLSL matrix's rows (4 × 3) → the DXBC registers (3 × float4).
    pub fn regs_from_rows(rows: &[[f32; 3]; 4]) -> [[f32; 4]; 3] {
        let mut r = [[0f32; 4]; 3];
        for k in 0..3 {
            for i in 0..4 {
                r[k][i] = rows[i][k];
            }
        }
        r
    }
    /// ProbeToShadow = ProbeToWorld · WorldPw01Shadow: given the peel's pw01 (rows, row-vector
    /// convention) recover ProbeToWorld's scale (16 per axis) and translation (the block's `pos`).
    pub fn probe_to_world(rows: &[[f32; 3]; 4], pw01: &[[f32; 4]; 4]) -> Option<([f32; 3], [f32; 3])> {
        // rows 0..2 of ProbeToShadow = diag(s) · pw01[0..2, 0..2] → s per row
        let mut s = [0f32; 3];
        for i in 0..3 {
            let (mut num, mut den) = (0f64, 0f64);
            for j in 0..3 {
                num += rows[i][j] as f64 * pw01[i][j] as f64;
                den += pw01[i][j] as f64 * pw01[i][j] as f64;
            }
            if den == 0.0 {
                return None;
            }
            s[i] = (num / den) as f32;
        }
        // row 3: t · R + T = rows[3]  →  t = (rows[3] − T) · R⁻¹
        let r = [[pw01[0][0] as f64, pw01[0][1] as f64, pw01[0][2] as f64], [pw01[1][0] as f64, pw01[1][1] as f64, pw01[1][2] as f64], [pw01[2][0] as f64, pw01[2][1] as f64, pw01[2][2] as f64]];
        let d = [rows[3][0] as f64 - pw01[3][0] as f64, rows[3][1] as f64 - pw01[3][1] as f64, rows[3][2] as f64 - pw01[3][2] as f64];
        let det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1]) - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0]) + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
        if det.abs() < 1e-30 {
            return None;
        }
        let inv = [
            [(r[1][1] * r[2][2] - r[1][2] * r[2][1]) / det, -(r[0][1] * r[2][2] - r[0][2] * r[2][1]) / det, (r[0][1] * r[1][2] - r[0][2] * r[1][1]) / det],
            [-(r[1][0] * r[2][2] - r[1][2] * r[2][0]) / det, (r[0][0] * r[2][2] - r[0][2] * r[2][0]) / det, -(r[0][0] * r[1][2] - r[0][2] * r[1][0]) / det],
            [(r[1][0] * r[2][1] - r[1][1] * r[2][0]) / det, -(r[0][0] * r[2][1] - r[0][1] * r[2][0]) / det, (r[0][0] * r[1][1] - r[0][1] * r[1][0]) / det],
        ];
        // t · R = d  →  t_j = Σ_k d_k · inv[k][j]
        let mut t = [0f32; 3];
        for j in 0..3 {
            t[j] = (0..3).map(|k| d[k] * inv[k][j]).sum::<f64>() as f32;
        }
        Some((s, t))
    }
}

/// Arithmetic options of the transcription that the D3D spec leaves to the hardware.
#[derive(Clone, Copy, Debug)]
pub struct ProbeOpts {
    /// dp4 as a fused chain (`fma(x, m0, fma(y, m1, fma(z, m2, m3)))`) instead of separate mul/add.
    pub fma: bool,
    /// Clamp the SampleCmp reference to [0, 1] (UNORM depth).
    pub clamp_ref: bool,
    /// The un-blended f16 store rounding (the capture: truncation).
    pub store: Rounding,
    /// Fraction bits of the linear comparison filter's weights (D3D11: ≥ 8).
    pub pcf_frac_bits: u32,
}

impl Default for ProbeOpts {
    fn default() -> Self {
        ProbeOpts { fma: false, clamp_ref: true, store: Rounding::Truncate, pcf_frac_bits: 8 }
    }
}

/// `utof(uint3(x, y, slice)) + SafetyOffset[x, y, slice].xyz` — the probe position in cells.
#[inline]
pub fn probe_point(x: u32, y: u32, z: u32, offsets: Option<&Volume3>) -> [f32; 3] {
    let mut p = [x as f32, y as f32, z as f32];
    if let Some(o) = offsets {
        for c in 0..3 {
            p[c] = o.get(x, y, z, c as u32) + p[c];
        }
    }
    p
}

/// `dp4((p, 1), reg)` for the three registers → (u, v, depth).
#[inline]
pub fn to_shadow(p: [f32; 3], regs: &[[f32; 4]; 3], fma: bool) -> [f32; 3] {
    let mut o = [0f32; 3];
    for k in 0..3 {
        let r = &regs[k];
        o[k] = if fma {
            p[0].mul_add(r[0], p[1].mul_add(r[1], p[2].mul_add(r[2], r[3])))
        } else {
            ((p[0] * r[0] + p[1] * r[1]) + p[2] * r[2]) + r[3]
        };
    }
    o
}

/// Point sampling with ClampEdge: the texel floor(u·W), clamped to the edge.
#[inline]
pub fn texel_point(u: f32, w: u32) -> u32 {
    let t = (u * w as f32).floor();
    if t.is_nan() || t < 0.0 {
        0
    } else if t >= w as f32 {
        w - 1
    } else {
        t as u32
    }
}

/// `SampleCmpLevelZero` with a POINT comparison sampler, GreaterEqual: 1 where `reference ≥ stored`.
#[inline]
pub fn sample_cmp_point_ge<L: crate::lmaccum::LayerRead>(layer: &L, u: f32, v: f32, reference: f32) -> f32 {
    let (w, h) = layer.depth_size();
    let x = texel_point(u, w);
    let y = texel_point(v, h);
    if reference >= layer.depth(x, y) { 1.0 } else { 0.0 }
}

/// `SampleCmpLevelZero` with a LINEAR comparison sampler, GreaterEqual, ClampEdge: the bilinear blend
/// of the four texels' compare results; the weights' fraction quantised to `frac_bits`.
pub fn sample_cmp_linear_ge<L: crate::lmaccum::LayerRead>(layer: &L, u: f32, v: f32, reference: f32, frac_bits: u32) -> f32 {
    let (dw, dh) = layer.depth_size();
    let fx = u * dw as f32 - 0.5;
    let fy = v * dh as f32 - 0.5;
    let x0 = fx.floor();
    let y0 = fy.floor();
    let q = (1u32 << frac_bits) as f32;
    let wx = ((fx - x0) * q).floor() / q;
    let wy = ((fy - y0) * q).floor() / q;
    let clampi = |t: f32, n: u32| -> u32 { if t.is_nan() || t < 0.0 { 0 } else if t >= n as f32 { n - 1 } else { t as u32 } };
    let xa = clampi(x0, dw);
    let xb = clampi(x0 + 1.0, dw);
    let ya = clampi(y0, dh);
    let yb = clampi(y0 + 1.0, dh);
    let c = |x: u32, y: u32| -> f32 { if reference >= layer.depth(x, y) { 1.0 } else { 0.0 } };
    let top = c(xa, ya) * (1.0 - wx) + c(xb, ya) * wx;
    let bot = c(xa, yb) * (1.0 - wx) + c(xb, yb) * wx;
    top * (1.0 - wy) + bot * wy
}

/// Point sample of an RGB target.
#[inline]
pub fn sample_point_rgb<L: crate::lmaccum::LayerRead>(layer: &L, u: f32, v: f32) -> [f32; 3] {
    let (w, h) = layer.color_size();
    let x = texel_point(u, w);
    let y = texel_point(v, h);
    layer.rgb(x, y)
}

/// The (x, y) cells a draw covers: the scissor rect (or the whole viewport).
pub fn draw_rect(d: &ProbeDraw, w: u32, h: u32) -> (u32, u32, u32, u32) {
    match d.scissor {
        Some([x, y, sw, sh]) => (x.min(w), y.min(h), (x + sw).min(w), (y + sh).min(h)),
        None => (0, 0, w, h),
    }
}

/// The value an un-blended RGBA16F store leaves.
#[inline]
fn store_f16(v: f32, r: Rounding) -> f32 {
    decode_f16(encode_f16(v, r))
}

/// The value a One/One f16 blend leaves: the source truncated to f16, the sum rounded to nearest-even.
#[inline]
pub fn blend_add_f16(dst: f32, src: f32) -> f32 {
    let s = decode_f16(encode_f16(src, Rounding::Truncate));
    decode_f16(encode_f16(dst + s, Rounding::NearestEven))
}

/// **PS 17151** `ProbeGrid_SetILightDir` for one layer: overwrite the probes the layer lies beyond.
/// Returns the number of probes written.
pub fn probe_set_ilightdir<L: crate::lmaccum::LayerRead>(target: &mut Volume3, d: &ProbeDraw, layer: &L, offsets: Option<&Volume3>, o: ProbeOpts) -> usize {
    assert_eq!(target.channels, 4);
    let (x0, y0, x1, y1) = draw_rect(d, target.w, target.h);
    // every probe writes its own slot: the slices in parallel (a block of the giant's grid is 458 k probes per
    // layer per direction — serial, it was the transcribed probes' whole cost)
    let z0 = d.slice_start;
    let z1 = (d.slice_start + d.slice_count).min(target.d);
    if z1 <= z0 || y1 <= y0 || x1 <= x0 {
        return 0;
    }
    let (w, h, ch) = (target.w as usize, target.h as usize, target.channels as usize);
    let tp = target.data.as_mut_ptr() as usize;
    let per_slice: Vec<usize> = crate::pool::pool().map((z1 - z0) as usize, |zi| {
        let z = z0 + zi as u32;
        let mut written = 0usize;
        for y in y0..y1 {
            for x in x0..x1 {
                let p = probe_point(x, y, z, offsets);
                let s = to_shadow(p, &d.regs, o.fma);
                let reference = if o.clamp_ref { s[2].clamp(0.0, 1.0) } else { s[2] };
                let vis = sample_cmp_point_ge(layer, s[0], s[1], reference);
                let rgb = sample_point_rgb(layer, s[0], s[1]);
                if vis - 0.5 < 0.0 {
                    continue; // discard: the layer is behind the probe, the previous layer's value stays
                }
                let sum = (rgb[0] + rgb[1]) + rgb[2];
                let a = if 1e-6f32 < sum { 1.0 } else { 0.0 };
                let out = [rgb[0] * d.out_scale, rgb[1] * d.out_scale, rgb[2] * d.out_scale, a * d.out_scale];
                let base = ((z as usize * h + y as usize) * w + x as usize) * ch;
                for c in 0..4 {
                    // SAFETY: the slices are disjoint; every probe's four channels belong to this slice
                    unsafe { *(tp as *mut f32).add(base + c) = store_f16(out[c], o.store); }
                }
                written += 1;
            }
        }
        written
    });
    per_slice.iter().sum()
}

/// **PS 17154** `ProbeGrid_AddSkyVisibility`: `+= OutScale` (One/One f16) where the 2×2 PCF against the
/// layer's depth is < 0.5, i.e. the probe is not behind the surface nearest the sky along D.
pub fn probe_add_sky_visibility<L: crate::lmaccum::LayerRead>(target: &mut Volume3, d: &ProbeDraw, layer: &L, offsets: Option<&Volume3>, o: ProbeOpts) -> usize {
    probe_add_sky_visibility_logged(target, d, layer, offsets, o, None)
}

/// `probe_add_sky_visibility`, logging the non-zero adds (flat probe index of channel 0, src) in order when
/// `log` is given — the direction-range split replays them (an add of 0 leaves an f16 value as it is).
pub fn probe_add_sky_visibility_logged<L: crate::lmaccum::LayerRead>(target: &mut Volume3, d: &ProbeDraw, layer: &L, offsets: Option<&Volume3>, o: ProbeOpts, log: Option<&mut Vec<(u32, f32)>>) -> usize {
    let (x0, y0, x1, y1) = draw_rect(d, target.w, target.h);
    let z0 = d.slice_start;
    let z1 = (d.slice_start + d.slice_count).min(target.d);
    if z1 <= z0 || y1 <= y0 || x1 <= x0 {
        return 0;
    }
    // the slices in parallel (every probe blends its own slot); the logged adds of every slice in the serial
    // order (z, y, x, channel), concatenated in slice order
    let (w, h, ch) = (target.w as usize, target.h as usize, target.channels as usize);
    let tp = target.data.as_mut_ptr() as usize;
    let logging = log.is_some();
    let per_slice: Vec<(usize, Vec<(u32, f32)>)> = crate::pool::pool().map((z1 - z0) as usize, |zi| {
        let z = z0 + zi as u32;
        let mut added = 0usize;
        let mut sub: Vec<(u32, f32)> = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                let p = probe_point(x, y, z, offsets);
                let s = to_shadow(p, &d.regs, o.fma);
                let reference = if o.clamp_ref { s[2].clamp(0.0, 1.0) } else { s[2] };
                let pcf = sample_cmp_linear_ge(layer, s[0], s[1], reference, o.pcf_frac_bits);
                // `lt r0.x, r0.x, 0.5` then `and o0, r0.xxxx, OutScale`: the value or 0, on every channel
                let src = if pcf < 0.5 { d.out_scale } else { 0.0 };
                let base = ((z as usize * h + y as usize) * w + x as usize) * ch;
                if src != 0.0 {
                    added += 1;
                    if logging { for c in 0..ch { sub.push(((base + c) as u32, src)); } }
                }
                for c in 0..ch {
                    // SAFETY: the slices are disjoint
                    unsafe {
                        let slot = (tp as *mut f32).add(base + c);
                        *slot = blend_add_f16(*slot, src);
                    }
                }
            }
        }
        (added, sub)
    });
    let mut added = 0usize;
    if let Some(l) = log {
        for (a, sub) in per_slice { added += a; l.extend(sub); }
    } else {
        for (a, _) in per_slice { added += a; }
    }
    added
}

/// **PS 1112** the fold: `target += src[x, y, slice] · scale` (One/One f16) over `slice_start..+count`
/// slices and the whole 32×16 face (no scissor at the folds).
pub fn probe_fold(target: &mut Volume3, src: &Volume3, scale: [f32; 4], slice_start: u32, slice_count: u32) {
    assert_eq!(target.channels, 4);
    assert_eq!(src.channels, 4);
    assert!(target.w == src.w && target.h == src.h && target.d == src.d, "probe_fold: the volumes differ in size");
    let z_end = (slice_start + slice_count).min(target.d);
    if z_end <= slice_start { return; }
    // THE FOLD IN PARALLEL OVER THE SLICES (E6 2026-09-29, the PERF row — baker-7's giant profile: the two folds per direction were
    // 37 s per sweep, serial, while every other stage runs on the pool): a slice's texels are its own contiguous run of the data
    // (`idx` = ((z·h + y)·w + x)·4 + c), so each slice job blends its run alone — the same `blend_add_f16` on the same operands in
    // the same order per texel, hence the same bytes; the result of one texel never depends on another.
    let per_slice = (target.w * target.h * 4) as usize;
    let tp = target.data.as_mut_ptr() as usize;
    let sp = src.data.as_ptr() as usize;
    let _: Vec<()> = crate::pool::pool().map((z_end - slice_start) as usize, |zi| {
        let z = (slice_start as usize + zi) * per_slice;
        // SAFETY: the slice runs [z, z + per_slice) are disjoint across jobs and inside both volumes (the sizes were asserted equal).
        let t = unsafe { std::slice::from_raw_parts_mut((tp as *mut f32).add(z), per_slice) };
        let s = unsafe { std::slice::from_raw_parts((sp as *const f32).add(z), per_slice) };
        for (i, (dst, src_v)) in t.iter_mut().zip(s.iter()).enumerate() {
            let v = *src_v * scale[i & 3];
            *dst = blend_add_f16(*dst, v);
        }
    });
}

/// The serial form of `probe_fold` (the pre-2026-09-29 loop), kept for the equality test below.
#[cfg(test)]
fn probe_fold_serial(target: &mut Volume3, src: &Volume3, scale: [f32; 4], slice_start: u32, slice_count: u32) {
    for z in slice_start..(slice_start + slice_count).min(target.d) {
        for y in 0..target.h {
            for x in 0..target.w {
                for c in 0..4 {
                    let s = src.get(x, y, z, c) * scale[c as usize];
                    let cur = target.get(x, y, z, c);
                    target.set(x, y, z, c, blend_add_f16(cur, s));
                }
            }
        }
    }
}

/// f16-for-f16 comparison over a volume: (values compared, bit-identical, within 1 f16 ulp, worse,
/// max |Δ|, the worst texel (x, y, z, c, ours, theirs)).
#[derive(Clone, Debug, Default)]
pub struct VolCompare {
    pub n: usize,
    pub exact: usize,
    pub ulp1: usize,
    pub worse: usize,
    pub max_abs: f32,
    pub worst: Option<(u32, u32, u32, u32, f32, f32)>,
    /// Texels non-zero on one side only.
    pub only_ours: usize,
    pub only_theirs: usize,
}

pub fn compare_volumes(ours: &Volume3, theirs: &Volume3, channels: u32) -> VolCompare {
    let mut r = VolCompare::default();
    assert_eq!((ours.w, ours.h, ours.d), (theirs.w, theirs.h, theirs.d));
    for z in 0..ours.d {
        for y in 0..ours.h {
            for x in 0..ours.w {
                let mut nz_o = false;
                let mut nz_t = false;
                for c in 0..channels {
                    let a = ours.get(x, y, z, c);
                    let b = theirs.get(x, y, z, c);
                    nz_o |= a != 0.0;
                    nz_t |= b != 0.0;
                    r.n += 1;
                    let ha = encode_f16(a, Rounding::NearestEven);
                    let hb = encode_f16(b, Rounding::NearestEven);
                    if ha == hb || (a == b) {
                        r.exact += 1;
                    } else if (ha as i32 - hb as i32).abs() == 1 && (ha ^ hb) & 0x8000 == 0 {
                        r.ulp1 += 1;
                    } else {
                        r.worse += 1;
                    }
                    let d = (a - b).abs();
                    if d > r.max_abs {
                        r.max_abs = d;
                        r.worst = Some((x, y, z, c, a, b));
                    }
                }
                if nz_o && !nz_t {
                    r.only_ours += 1;
                }
                if nz_t && !nz_o {
                    r.only_theirs += 1;
                }
            }
        }
    }
    r
}

impl VolCompare {
    pub fn line(&self) -> String {
        let worst = match self.worst {
            Some((x, y, z, c, a, b)) if self.max_abs > 0.0 => format!("; worst ({x},{y},{z}) c{c}: ours {a} game {b}"),
            _ => String::new(),
        };
        format!("{} values: {} bit-identical, {} within 1 f16 ulp, {} beyond (max |Δ| {:.6}); texels non-zero only ours {}, only game {}{}", self.n, self.exact, self.ulp1, self.worse, self.max_abs, self.only_ours, self.only_theirs, worst)
    }
    pub fn closed(&self) -> bool {
        self.worse == 0 && self.ulp1 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn depth_buf(w: u32, h: u32, v: f32) -> Buf {
        let mut b = Buf::new(w, h, 1);
        for y in 0..h {
            for x in 0..w {
                b.set(x, y, 0, v);
            }
        }
        b
    }

    #[test]
    fn probe_to_world_recovers_the_block_pos_from_the_cbuffer() {
        // frame 127448 eid 2758 (direction 0 of sweep 0) and the world peel's pw01 of that direction
        let rows = [[-0.005694863386452198, 0.0014758179895579815, -0.0020974765066057444], [0.0, -0.03598681837320328, -0.0007108112331479788], [0.00211304216645658, 0.003977479413151741, -0.005652911961078644], [0.5601815581321716, 0.4556904137134552, 0.9431222677230835]];
        let pw01 = [[-0.0003559289616532624, 9.223862434737384e-05, -0.00013109228166285902, 0.0], [0.0, -0.002249176148325205, -4.4425702071748674e-05, 0.0], [0.00013206513540353626, 0.0002485924633219838, -0.00035330699756741524, 0.0], [0.7292365431785583, 0.31068041920661926, 1.0001277923583984, 1.0]];
        let (s, t) = ProbeDraw::probe_to_world(&rows, &pw01).unwrap();
        for k in 0..3 {
            assert!((s[k] - 16.0).abs() < 1e-4, "scale {s:?}");
        }
        // the baked map's block record: pos (472, −46, −8) = O + 480·(1, 0, 0) − 8 with O = (0, −38, 0)
        assert!((t[0] - 472.0).abs() < 1e-3 && (t[1] + 46.0).abs() < 1e-3 && (t[2] + 8.0).abs() < 1e-3, "translation {t:?}");
    }

    #[test]
    fn regs_are_the_matrix_columns() {
        let rows = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0], [10.0, 11.0, 12.0]];
        let r = ProbeDraw::regs_from_rows(&rows);
        assert_eq!(r[0], [1.0, 4.0, 7.0, 10.0]);
        assert_eq!(r[2], [3.0, 6.0, 9.0, 12.0]);
        // (p, 1) · M for p = (1, 0, 0): row 0 + row 3
        let s = to_shadow([1.0, 0.0, 0.0], &r, false);
        assert_eq!(s, [11.0, 13.0, 15.0]);
    }

    #[test]
    fn point_sampling_takes_floor_and_clamps_to_the_edge() {
        assert_eq!(texel_point(0.0, 4096), 0);
        assert_eq!(texel_point(0.5, 4096), 2048);
        assert_eq!(texel_point(1.0, 4096), 4095);
        assert_eq!(texel_point(-0.2, 4096), 0);
        assert_eq!(texel_point(1.7, 4096), 4095);
    }

    #[test]
    fn set_ilightdir_overwrites_only_where_the_layer_is_beyond_the_probe() {
        // a 4×4 peel, identity-like mapping: u = x/32, v = y/16, depth = z/32 (regs carry the scale)
        let regs = [[1.0 / 32.0, 0.0, 0.0, 0.0], [0.0, 1.0 / 16.0, 0.0, 0.0], [0.0, 0.0, 1.0 / 32.0, 0.0]];
        let d = ProbeDraw { eid: 0, regs, out_scale: 1.0, slice_start: 0, slice_count: 32, scissor: Some([0, 0, 32, 16]) };
        let mut color = Buf::new(4, 4, 3);
        for y in 0..4 {
            for x in 0..4 {
                color.set(x, y, 0, 0.25);
                color.set(x, y, 1, 0.5);
                color.set(x, y, 2, 0.75);
            }
        }
        let depth = depth_buf(4, 4, 0.5); // the layer at z01 0.5 → probes with z/32 ≥ 0.5 (z ≥ 16) pass
        let mut t = Volume3::new(32, 16, 32, 4);
        let n = probe_set_ilightdir(&mut t, &d, &crate::lmaccum::LayerTargets { color: &color, depth: &depth }, None, ProbeOpts::default());
        assert_eq!(n, 32 * 16 * 16);
        assert_eq!(t.get(0, 0, 15, 0), 0.0);
        assert_eq!([t.get(0, 0, 16, 0), t.get(0, 0, 16, 1), t.get(0, 0, 16, 2), t.get(0, 0, 16, 3)], [0.25, 0.5, 0.75, 1.0]);
        // a black layer beyond the probe writes (0, 0, 0, 0): alpha needs a non-black colour
        let black = Buf::new(4, 4, 3);
        let n2 = probe_set_ilightdir(&mut t, &d, &crate::lmaccum::LayerTargets { color: &black, depth: &depth_buf(4, 4, 0.25) }, None, ProbeOpts::default());
        assert_eq!(n2, 32 * 16 * 24);
        assert_eq!(t.get(0, 0, 16, 3), 0.0);
        assert_eq!(t.get(0, 0, 7, 3), 0.0);
    }

    #[test]
    fn sky_visibility_adds_out_scale_where_the_pcf_fails_and_blends_in_f16() {
        let regs = [[1.0 / 32.0, 0.0, 0.0, 0.0], [0.0, 1.0 / 16.0, 0.0, 0.0], [0.0, 0.0, 1.0 / 32.0, 0.0]];
        let scale = 4.0 * 0.11707823723554611 / 256.0; // 4·D.y/N of frame 127448's direction 0
        let d = ProbeDraw { eid: 0, regs, out_scale: scale, slice_start: 0, slice_count: 32, scissor: None };
        let depth = depth_buf(4, 4, 0.5);
        let mut t = Volume3::new(32, 16, 32, 1);
        let n = probe_add_sky_visibility(&mut t, &d, &crate::lmaccum::LayerTargets { color: &depth, depth: &depth }, None, ProbeOpts::default());
        assert_eq!(n, 32 * 16 * 16); // z < 16: the probe is before the surface → sees the sky
        // the stored value is the source truncated to f16 (0.0018293474 → 0.0018291473), as the capture shows
        assert_eq!(t.get(0, 0, 0, 0), 0.0018291473388671875);
        assert_eq!(t.get(0, 0, 20, 0), 0.0);
        // a second add rounds the f16 sum to nearest-even
        probe_add_sky_visibility(&mut t, &d, &crate::lmaccum::LayerTargets { color: &depth, depth: &depth }, None, ProbeOpts::default());
        assert_eq!(t.get(0, 0, 0, 0), decode_f16(encode_f16(2.0 * 0.0018291473388671875, Rounding::NearestEven)));
    }

    #[test]
    fn fold_parallel_equals_serial_to_the_bit() {
        // a volume of many f16-representable and non-representable values, folded twice with the two production scales
        let (w, h, d) = (40u32, 16u32, 32u32);
        let mut src = Volume3::new(w, h, d, 4);
        let mut t_par = Volume3::new(w, h, d, 4);
        let mut t_ser = Volume3::new(w, h, d, 4);
        let mut seed = 0x9E37_79B9u32;
        for i in 0..src.data.len() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let v = (seed >> 8) as f32 / (1u32 << 24) as f32 * 3.0 - 0.5;
            src.data[i] = if seed & 7 == 0 { 0.0 } else { v };
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let start = decode_f16(encode_f16((seed >> 8) as f32 / (1u32 << 24) as f32, Rounding::NearestEven));
            t_par.data[i] = start;
            t_ser.data[i] = start;
        }
        for scale in [[2.0 / 1032.0, 2.0 / 1032.0, 2.0 / 1032.0, 1.0 / 1032.0], [0.0018293f32, 0.0018293, 0.0018293, 0.0018293], [-0.0007f32, -0.0007, -0.0007, -0.0007]] {
            probe_fold(&mut t_par, &src, scale, 0, d);
            probe_fold_serial(&mut t_ser, &src, scale, 0, d);
        }
        // a partial slice range too
        probe_fold(&mut t_par, &src, [0.01, 0.01, 0.01, 0.005], 5, 9);
        probe_fold_serial(&mut t_ser, &src, [0.01, 0.01, 0.01, 0.005], 5, 9);
        assert!(t_par.data.iter().zip(t_ser.data.iter()).all(|(a, b)| a.to_bits() == b.to_bits()), "the parallel fold differs from the serial one");
    }

    #[test]
    fn fold_scales_and_accumulates_all_slices() {
        let mut src = Volume3::new(32, 16, 32, 4);
        src.set(25, 8, 23, 0, 0.40625);
        src.set(25, 8, 23, 1, 0.5390625);
        src.set(25, 8, 23, 2, 0.71875);
        src.set(25, 8, 23, 3, 1.0);
        let mut t = Volume3::new(32, 16, 32, 4);
        probe_fold(&mut t, &src, [1.0 / 128.0, 1.0 / 128.0, 1.0 / 128.0, 1.0 / 256.0], 0, 32);
        // frame 127448 eid 7150's captured texel (25, 8, 23)
        assert_eq!([t.get(25, 8, 23, 0), t.get(25, 8, 23, 1), t.get(25, 8, 23, 2), t.get(25, 8, 23, 3)], [0.003173828125, 0.00421142578125, 0.005615234375, 0.00390625]);
        let mut t2 = Volume3::new(32, 16, 32, 4);
        let s = 0.001829347456805408f32;
        probe_fold(&mut t2, &src, [s, s, s, s], 0, 32);
        assert_eq!([t2.get(25, 8, 23, 0), t2.get(25, 8, 23, 3)], [0.0007429122924804688, 0.0018291473388671875]);
    }

    #[test]
    fn snorm16_decodes_both_minimum_codes_to_minus_one() {
        assert_eq!(snorm16(-32768), -1.0);
        assert_eq!(snorm16(-32767), -1.0);
        assert_eq!(snorm16(32767), 1.0);
        assert_eq!(snorm16(0), 0.0);
    }
}

// ───────────────────────────── the CPU download (0x14022d8f0) ─────────────────────────────

/// The three probe accumulators at the end of the bake and how the CPU turns them into the stored
/// probe images (client decompile `FUN_14022d8f0`, "ProbeGrid download"; `client-re/decomp2`):
///
/// * `colour` = the fold 17056 (Σ over both sweeps of 2/N · the first surface's colour, α = Σ 1/N ·
///   [front face]); `max0` = the largest rgb channel over ALL probes → `frame_info[0].scale`;
///   image 0 = `t12[(int)(v / max0 · 4095 + 0.5)]` (the client's 4096-entry linear → sRGB byte table) for the
///   block's valid probes; a valid probe whose three bytes are 0 is written (1, 1, 1);
/// * `valid` = `colour.a ≥ 0.5` → the trailer's cell4 mask bits;
/// * `skyvis` = the R16F volume 17160 (Σ over the upward sweep-0 directions of 4·D.y/N · [nothing above
///   the probe]) → image 1 = `clamp(round(255 · v), 0, 255)`, no scale;
/// * `updown` = the fold 17059 (Σ 4·D.y/N · colour, signed) → `max2` = max |rgb| over the VALID probes
///   → `frame_info[1].scale`; image 2 = `clamp(round(127 · sign(v) · sqrt(|v| / max2)), −127, 127)`
///   per channel (the fourth byte 0) — a signed byte image, stored + 127 (its zero is 127);
/// * `frame_info[2].scale` and image 3 belong to the local-light probe pass (not in this bake: 1e-5).
///
/// The stored atlas (`probe_atlases`): 21×21 for this block's 7×7 tiles in a 3×3 grid; pixels no tile
/// covers hold 128 (images 0, 1, 3) / 127 (image 2); the four WEBPs = libwebp preset DEFAULT, RGB import at
/// quality 91 for images 0, 2, 3 and Y-only (U = V = 128) at quality 80 for image 1 — byte-identical to the
/// pwc6 save's four blobs (`lmtool final-check --probes`).
pub struct ProbeDownload {
    pub max0: f32,
    pub max2: f32,
    /// Per probe (x, y, z) of the block range: image 0 rgb bytes, the validity, image 1 byte (None
    /// without the sky-visibility volume), image 2 signed bytes.
    pub probes: Vec<((u32, u32, u32), [u8; 3], bool, Option<u8>, [i8; 3])>,
}

/// `lroundf` as the client's `FUN_1418f6954` (round half away from zero).
#[inline]
pub fn lround(v: f32) -> i32 {
    v.round() as i32
}

pub fn download_probes(colour: &Volume3, updown: &Volume3, skyvis: Option<&Volume3>, range: ([u32; 3], [u32; 3])) -> ProbeDownload {
    let (lo, hi) = range;
    let mut max0 = 0f32;
    for z in 0..colour.d { for y in 0..colour.h { for x in 0..colour.w { for c in 0..3 { max0 = max0.max(colour.get(x, y, z, c)); } } } }
    let valid = |x: u32, y: u32, z: u32| colour.get(x, y, z, 3) >= 0.5;
    let mut max2 = 0f32;
    for z in 0..colour.d { for y in 0..colour.h { for x in 0..colour.w { if valid(x, y, z) { for c in 0..3 { max2 = max2.max(updown.get(x, y, z, c).abs()); } } } } }
    let t12 = crate::filecheck::srgb_encode_table();
    let mut probes = Vec::new();
    for y in lo[1]..hi[1] {
        for z in lo[2]..hi[2] {
            for x in lo[0]..hi[0] {
                let ok = valid(x, y, z);
                let mut rgb = [0u8; 3];
                let mut sq = [0i8; 3];
                let mut sky = None;
                if ok {
                    for c in 0..3 {
                        let v = colour.get(x, y, z, c) / max0;
                        // the client's LUT: the 4096-entry linear → sRGB byte table indexed by (int)(v · 4095 + 0.5)
                        rgb[c as usize] = t12[((v.clamp(0.0, 1.0) * 4095.0 + 0.5) as i32).clamp(0, 4095) as usize];
                        let u = updown.get(x, y, z, c) / max2;
                        let s = if u < 0.0 { -1.0 } else { 1.0 };
                        let b = lround(u.abs().sqrt() * s * 127.0);
                        sq[c as usize] = b.clamp(-127, 127) as i8;
                    }
                    if rgb == [0, 0, 0] {
                        rgb = [1, 1, 1];
                    }
                    if let Some(sv) = skyvis {
                        // linear: round(255 · v) (byte-identical WEBP against the pwc6 save)
                        sky = Some(lround(sv.get(x, y, z, 0) * 255.0).clamp(0, 255) as u8);
                    }
                }
                probes.push(((x, y, z), rgb, ok, sky, sq));
            }
        }
    }
    ProbeDownload { max0, max2, probes }
}

#[cfg(test)]
mod download_tests {
    use super::*;

    #[test]
    fn the_download_scales_and_bytes_follow_the_decompile() {
        let mut colour = Volume3::new(32, 16, 32, 4);
        let mut updown = Volume3::new(32, 16, 32, 4);
        let mut sky = Volume3::new(32, 16, 32, 1);
        // an open probe: colour (0.4, 0.5, 1.0) valid, updown (0.2, −0.1, 0.5), sky 0.999
        colour.set(22, 4, 20, 0, 0.4); colour.set(22, 4, 20, 1, 0.5); colour.set(22, 4, 20, 2, 1.0); colour.set(22, 4, 20, 3, 1.0);
        updown.set(22, 4, 20, 0, 0.2); updown.set(22, 4, 20, 1, -0.1); updown.set(22, 4, 20, 2, 0.5);
        sky.set(22, 4, 20, 0, 0.999);
        // an invalid probe (α 0.3) with a brighter updown that must NOT enter max2
        colour.set(23, 4, 20, 0, 0.1); colour.set(23, 4, 20, 3, 0.3);
        updown.set(23, 4, 20, 2, 0.9);
        let dl = download_probes(&colour, &updown, Some(&sky), ([22, 4, 20], [24, 5, 21]));
        assert_eq!(dl.max0, 1.0);
        assert_eq!(dl.max2, 0.5);
        let open = &dl.probes[0];
        assert_eq!(open.0, (22, 4, 20));
        assert!(open.2);
        // through the 4096-entry table: t12[(int)(0.4·4095 + 0.5)] = 170, t12[2048] = 188, t12[4095] = 255
        assert_eq!(open.1, [170, 188, 255]);
        assert_eq!(open.3, Some(255)); // round(0.999·255) = 254.7 → 255
        // signed sqrt: 127·√(0.4) = 80.3 → 80; −127·√(0.2) = −56.8 → −57; 127·1 = 127
        assert_eq!(open.4, [80, -57, 127]);
        let closed = &dl.probes[1];
        assert!(!closed.2);
        assert_eq!(closed.1, [0, 0, 0]);
        assert_eq!(closed.3, None);
    }

    #[test]
    fn a_valid_black_probe_is_written_one_one_one() {
        let mut colour = Volume3::new(32, 16, 32, 4);
        colour.set(0, 0, 0, 3, 1.0);
        colour.set(1, 0, 0, 0, 0.5);
        colour.set(1, 0, 0, 3, 1.0);
        let updown = Volume3::new(32, 16, 32, 4);
        let dl = download_probes(&colour, &updown, None, ([0, 0, 0], [2, 1, 1]));
        assert_eq!(dl.probes[0].1, [1, 1, 1]);
    }
}


/// The four stored probe atlases (RGB triplets, `w × h`) from a download and the block's tile table
/// (`tiles[level − min.y]` = the tile's (x, y) in the atlas, None for a missing level).
///
/// FUN_14022d8f0's per-probe branch (decomp2/14022d8f0.c, read 2026-09-28 by E4): a VALID probe (α ≥ 0.5) writes its
/// colour bytes (an all-zero triple becomes (1, 1, 1)), its sky byte and its signed up/down bytes; an INVALID one is
/// WRITTEN too — `pcVar12[-1..2] = 0` (colour 0, sky 0) and its up/down pixel `= 0` (→ 127 after the packer's +127) — never
/// left at the buffer's initial value. The port skipped invalid probes and left the 128 / 127 atlas init there: V4's
/// "grey 128 inside items / under the island tiles" (probe-column-48e1-vs-5ff2.md); pwc-day's byte-exact save had no
/// invalid in-tile probe. Pixels no tile covers keep the init (128 / 127: the pwc6 save's own bytes there).
pub fn probe_atlases(dl: &ProbeDownload, block_min: [u32; 3], tiles: &[Option<(u32, u32)>], w: u32, h: u32) -> [Vec<u8>; 4] {
    probe_atlases_with_lamp(dl, None, block_min, tiles, w, h)
}

/// `probe_atlases` with the fourth image's bytes (`lamp[i]` for `dl.probes[i]`, `download_lamp_probes`): the lamp colour probe
/// image (the download's third source texture, param_1[2]); None = the lamp pass did not run → zeros, as before.
pub fn probe_atlases_with_lamp(dl: &ProbeDownload, lamp: Option<&[[u8; 3]]>, block_min: [u32; 3], tiles: &[Option<(u32, u32)>], w: u32, h: u32) -> [Vec<u8>; 4] {
    let n = (w * h * 3) as usize;
    let mut imgs = [vec![128u8; n], vec![128u8; n], vec![127u8; n], vec![128u8; n]];
    for (pi, ((x, y, z), rgb, ok, sky, sq)) in dl.probes.iter().enumerate() {
        let Some(Some((tx, ty))) = tiles.get((*y - block_min[1]) as usize) else { continue };
        let (px, py) = (tx + (x - block_min[0]), ty + (z - block_min[2]));
        if px >= w || py >= h {
            continue;
        }
        let o = ((py * w + px) * 3) as usize;
        if !*ok {
            imgs[0][o..o + 3].copy_from_slice(&[0, 0, 0]);
            imgs[1][o..o + 3].copy_from_slice(&[0, 0, 0]);
            imgs[2][o..o + 3].copy_from_slice(&[127, 127, 127]);
            imgs[3][o..o + 3].copy_from_slice(&[0, 0, 0]);
            continue;
        }
        imgs[0][o..o + 3].copy_from_slice(rgb);
        let s = sky.unwrap_or(0);
        imgs[1][o..o + 3].copy_from_slice(&[s, s, s]);
        for c in 0..3 {
            imgs[2][o + c] = (sq[c] as i32 + 127) as u8;
        }
        imgs[3][o..o + 3].copy_from_slice(&lamp.and_then(|l| l.get(pi).copied()).unwrap_or([0, 0, 0]));
    }
    imgs
}

/// THE LAMP COLOUR PROBE IMAGE's download (E4, 2026-09-28): the fourth stored probe image = the colour probe pass's volume
/// (PS 9605, `localdrive::FrameOut::lamp_probe`) through the same encode as image 0 — `maxl` = the largest rgb channel over the
/// VALID probes (α ≥ 0.5; like max2, unlike max0: the editor's stpad word 0.279297 is its brightest valid probe's value, byte 255,
/// while our brightest probe overall sits inside a lamp post at 0.54) → `frame_info[2]`'s scale word (1e-5 when nothing was lit,
/// the lamp-less saves' word), byte = `t12[(int)(v / maxl · 4095 + 0.5)]` for a VALID probe (the invalid ones are written 0 like
/// the other images; the editor's stpad image 3 reads 0 at every masked probe). Returns (maxl, the bytes in `download_probes`'
/// probe order for `range`). The encode is the image-0 rule by analogy — FUN_14022d8f0 only READS this texture (param_1[2], 3
/// bytes per probe, through the byte→linear LUT DAT_141a64360); the falsifier is the editor's image 3 (stpad Night: 424 lamps,
/// 2 929 unsaturated lit probes at a value ratio of 0.995 with 67 % within ±10 %, scale 0.2793).
pub fn download_lamp_probes(lamp: &Volume3, colour: &Volume3, range: ([u32; 3], [u32; 3])) -> (f32, Vec<[u8; 3]>) {
    let (lo, hi) = range;
    let mut maxl = 0f32;
    for z in 0..lamp.d { for y in 0..lamp.h { for x in 0..lamp.w {
        if x < colour.w && y < colour.h && z < colour.d && colour.get(x, y, z, 3) >= 0.5 { for c in 0..3 { maxl = maxl.max(lamp.get(x, y, z, c)); } }
    } } }
    let scale = if maxl > 0.0 { maxl } else { 1e-5 };
    let t12 = crate::filecheck::srgb_encode_table();
    let mut out = Vec::new();
    for y in lo[1]..hi[1] {
        for z in lo[2]..hi[2] {
            for x in lo[0]..hi[0] {
                let ok = colour.get(x, y, z, 3) >= 0.5;
                let mut rgb = [0u8; 3];
                if ok && x < lamp.w && y < lamp.h && z < lamp.d {
                    for c in 0..3u32 {
                        let v = lamp.get(x, y, z, c) / scale;
                        rgb[c as usize] = t12[((v.clamp(0.0, 1.0) * 4095.0 + 0.5) as i32).clamp(0, 4095) as usize];
                    }
                }
                out.push(rgb);
            }
        }
    }
    (scale, out)
}

/// The atlas pixels a block's tiles cover (the block's `min`/`max` cell range over its tile table): the merge of the
/// per-block atlases in `ProbeBake::finish` copies exactly these (the blocks' tiles are disjoint in the atlas).
pub fn probe_tile_pixels(block_min: [u32; 3], block_max: [u32; 3], tiles: &[Option<(u32, u32)>], w: u32, h: u32) -> Vec<usize> {
    let mut out = Vec::new();
    for t in tiles.iter().flatten() {
        for z in block_min[2]..block_max[2] {
            for x in block_min[0]..block_max[0] {
                let (px, py) = (t.0 + (x - block_min[0]), t.1 + (z - block_min[2]));
                if px < w && py < h {
                    out.push((py * w + px) as usize);
                }
            }
        }
    }
    out
}

/// The four probe WEBPs as the client writes them (needs libwebp): images 0, 2, 3 RGB import at quality 91,
/// image 1 Y-only at quality 80.
pub fn encode_probe_atlases(imgs: &[Vec<u8>; 4], w: u32, h: u32) -> Option<[Vec<u8>; 4]> {
    let grey: Vec<u8> = imgs[1].chunks(3).map(|c| c[0]).collect();
    Some([
        crate::webpenc::encode_rgb(&imgs[0], w, h, 91.0)?,
        crate::webpenc::encode_grey(&grey, w, h, 80.0)?,
        crate::webpenc::encode_rgb(&imgs[2], w, h, 91.0)?,
        crate::webpenc::encode_rgb(&imgs[3], w, h, 91.0)?,
    ])
}
