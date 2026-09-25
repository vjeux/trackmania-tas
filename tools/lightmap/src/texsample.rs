//! Textures as the GPU samples them: DDS containers (the capture's texture exports), the block formats the
//! lightmapper's materials use (BC1, BC3, BC4-alpha) decoded per the D3D11 reference, and a `sample`
//! following the D3D11 texture-filtering pipeline (3.2 / 7.18: address modes, the LOD from the screen-space
//! derivatives, trilinear and anisotropic footprints, bilinear weights at a chosen fixed-point precision).
//!
//! What the spec leaves to the implementation is a parameter here so the capture can decide it (`Sampler`,
//! `Bc1Decode`, `weight_bits`): the block-colour interpolation rounding, the weight precision (≥ 8 bits
//! fractional, 7.18.7), the anisotropic tap placement.

use crate::passdiff::gunzip;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexFmt {
    Bc1,
    Bc3,
    R16Unorm,
    Bgra8,
    Rgba8,
    R8Unorm,
    Rg8Uint,
    Unknown(u32),
}

/// One mip level of one array slice: 8-bit-exact formats keep their bytes (decoded through `lut` on fetch — the
/// identity k/255, or the sRGB curve after `Texture::decode_srgb`), the others hold f32 RGBA.
#[derive(Clone, Debug)]
pub struct Level {
    pub w: u32,
    pub h: u32,
    pub px: Px,
    pub lut: std::sync::Arc<[f32; 256]>,
}

#[derive(Clone, Debug)]
pub enum Px {
    F32(Vec<[f32; 4]>),
    U8(Vec<[u8; 4]>),
}

fn identity_lut() -> std::sync::Arc<[f32; 256]> {
    let mut t = [0f32; 256];
    for (i, v) in t.iter_mut().enumerate() {
        *v = i as f32 / 255.0;
    }
    std::sync::Arc::new(t)
}

impl Level {
    pub fn from_f32(w: u32, h: u32, px: Vec<[f32; 4]>) -> Level {
        Level { w, h, px: Px::F32(px), lut: identity_lut() }
    }
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> [f32; 4] {
        match &self.px {
            Px::F32(v) => v[(y * self.w + x) as usize],
            Px::U8(v) => {
                let p = v[(y * self.w + x) as usize];
                [self.lut[p[0] as usize], self.lut[p[1] as usize], self.lut[p[2] as usize], p[3] as f32 / 255.0]
            }
        }
    }
    pub fn len(&self) -> usize {
        match &self.px { Px::F32(v) => v.len(), Px::U8(v) => v.len() }
    }
    /// Every texel as f32 (a copy).
    pub fn texels(&self) -> Vec<[f32; 4]> {
        (0..self.h).flat_map(|y| (0..self.w).map(move |x| (x, y))).map(|(x, y)| self.get(x, y)).collect()
    }
}

/// A texture: `levels[slice][mip]`.
#[derive(Clone, Debug)]
pub struct Texture {
    pub fmt: TexFmt,
    pub w: u32,
    pub h: u32,
    pub mips: u32,
    pub slices: u32,
    pub levels: Vec<Vec<Level>>,
    /// Whether the container held every mip (the sampler clamps the LOD to the levels present).
    pub complete: bool,
}

/// How the BC1 palette is expanded (D3D11 19.5.1 leaves ±1/255 to the implementation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bc1Decode {
    /// The reference: c5/31, c6/63 exactly, thirds exactly in float.
    Ideal,
    /// 8-bit expansion `(c5 << 3) | (c5 >> 2)`, `(c6 << 2) | (c6 >> 4)`, thirds as `(2a + b) / 3` truncated.
    Expand8Trunc,
    /// 8-bit expansion, thirds as `(2a + b + 1) / 3`.
    Expand8Round,
}

fn dxgi_fmt(id: u32) -> TexFmt {
    match id {
        70 | 71 | 72 => TexFmt::Bc1,
        76 | 77 | 78 => TexFmt::Bc3,
        56 | 54 => TexFmt::R16Unorm,
        87 | 90 | 91 => TexFmt::Bgra8,
        27 | 28 | 29 => TexFmt::Rgba8,
        60 | 61 => TexFmt::R8Unorm,
        48 | 50 => TexFmt::Rg8Uint,
        other => TexFmt::Unknown(other),
    }
}

fn block_bytes(f: TexFmt) -> Option<usize> {
    match f {
        TexFmt::Bc1 => Some(8),
        TexFmt::Bc3 => Some(16),
        _ => None,
    }
}

fn pixel_bytes(f: TexFmt) -> usize {
    match f {
        TexFmt::R16Unorm => 2,
        TexFmt::Bgra8 | TexFmt::Rgba8 => 4,
        TexFmt::R8Unorm => 1,
        TexFmt::Rg8Uint => 2,
        _ => 0,
    }
}

fn level_bytes(f: TexFmt, w: u32, h: u32) -> usize {
    match block_bytes(f) {
        Some(b) => (w.div_ceil(4) * h.div_ceil(4)) as usize * b,
        None => (w * h) as usize * pixel_bytes(f),
    }
}

/// Load a DDS (optionally gzipped) and decode every level of every slice it holds.
pub fn load_dds(path: &std::path::Path, bc1: Bc1Decode) -> Result<Texture, String> {
    let mut bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        bytes = gunzip(&bytes)?;
    }
    parse_dds(&bytes, bc1).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn parse_dds(b: &[u8], bc1: Bc1Decode) -> Result<Texture, String> {
    if b.len() < 128 || &b[..4] != b"DDS " {
        return Err("not a DDS".into());
    }
    let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let flags = u(8);
    let h = u(12);
    let w = u(16);
    let mips = if flags & 0x20000 != 0 { u(28).max(1) } else { 1 };
    let pf_flags = u(80);
    let fourcc = &b[84..88];
    let (fmt, off, slices) = if pf_flags & 4 != 0 && fourcc == b"DX10" {
        let dxgi = u(128);
        let arr = u(140).max(1);
        (dxgi_fmt(dxgi), 148usize, arr)
    } else if pf_flags & 4 != 0 {
        let f = match fourcc {
            b"DXT1" => TexFmt::Bc1,
            b"DXT5" => TexFmt::Bc3,
            _ => TexFmt::Unknown(u(84)),
        };
        (f, 128usize, 1)
    } else {
        let bits = u(88);
        let rmask = u(92);
        let f = match (bits, rmask) {
            (16, 0xffff) => TexFmt::R16Unorm,
            (32, 0x00ff0000) => TexFmt::Bgra8,
            (32, 0x000000ff) => TexFmt::Rgba8,
            (8, 0xff) => TexFmt::R8Unorm,
            _ => TexFmt::Unknown(0),
        };
        (f, 128usize, 1)
    };
    if let TexFmt::Unknown(id) = fmt {
        return Err(format!("unsupported DDS format {id}"));
    }
    let mut levels = Vec::with_capacity(slices as usize);
    let mut o = off;
    let mut complete = true;
    for _ in 0..slices {
        let mut lv = Vec::with_capacity(mips as usize);
        let (mut lw, mut lh) = (w, h);
        for _ in 0..mips {
            let n = level_bytes(fmt, lw, lh);
            if o + n > b.len() {
                complete = false;
                break;
            }
            lv.push(decode_level(fmt, &b[o..o + n], lw, lh, bc1));
            o += n;
            lw = (lw / 2).max(1);
            lh = (lh / 2).max(1);
        }
        levels.push(lv);
    }
    // the container may hold fewer mips than the texture has (RenderDoc exports the top level of arrays)
    let full_chain = 32 - (w.max(h)).leading_zeros();
    if mips < full_chain {
        complete = false;
    }
    Ok(Texture { fmt, w, h, mips, slices, levels, complete })
}

fn decode_level(fmt: TexFmt, d: &[u8], w: u32, h: u32, bc1: Bc1Decode) -> Level {
    let n = (w * h) as usize;
    match fmt {
        TexFmt::Bc1 | TexFmt::Bc3 => {
            // BC1 with an 8-bit palette expansion is byte-exact (U8); the ideal decode and BC3's alpha are not (F32)
            let u8_exact = fmt == TexFmt::Bc1 && bc1 != Bc1Decode::Ideal;
            let mut pf = if u8_exact { Vec::new() } else { vec![[0.0f32; 4]; n] };
            let mut pu = if u8_exact { vec![[0u8; 4]; n] } else { Vec::new() };
            let bw = w.div_ceil(4);
            let bs = block_bytes(fmt).unwrap();
            for by in 0..h.div_ceil(4) {
                for bx in 0..bw {
                    let blk = &d[((by * bw + bx) as usize) * bs..];
                    let (col, alpha) = if fmt == TexFmt::Bc3 { (decode_bc1_block(&blk[8..16], bc1, false), Some(decode_bc4_block(&blk[..8]))) } else { (decode_bc1_block(&blk[..8], bc1, true), None) };
                    for j in 0..4 {
                        for i in 0..4 {
                            let (x, y) = (bx * 4 + i, by * 4 + j);
                            if x < w && y < h {
                                let mut c = col[(j * 4 + i) as usize];
                                if let Some(a) = &alpha {
                                    c[3] = a[(j * 4 + i) as usize];
                                }
                                if u8_exact {
                                    pu[(y * w + x) as usize] = [(c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8, (c[3] * 255.0).round() as u8];
                                } else {
                                    pf[(y * w + x) as usize] = c;
                                }
                            }
                        }
                    }
                }
            }
            if u8_exact { Level { w, h, px: Px::U8(pu), lut: identity_lut() } } else { Level::from_f32(w, h, pf) }
        }
        TexFmt::R16Unorm => Level::from_f32(w, h, (0..n).map(|i| [u16::from_le_bytes([d[i * 2], d[i * 2 + 1]]) as f32 / 65535.0, 0.0, 0.0, 1.0]).collect()),
        TexFmt::Bgra8 => Level { w, h, px: Px::U8((0..n).map(|i| [d[i * 4 + 2], d[i * 4 + 1], d[i * 4], d[i * 4 + 3]]).collect()), lut: identity_lut() },
        TexFmt::Rgba8 => Level { w, h, px: Px::U8((0..n).map(|i| [d[i * 4], d[i * 4 + 1], d[i * 4 + 2], d[i * 4 + 3]]).collect()), lut: identity_lut() },
        TexFmt::R8Unorm => Level { w, h, px: Px::U8((0..n).map(|i| [d[i], 0, 0, 255]).collect()), lut: identity_lut() },
        TexFmt::Rg8Uint => Level::from_f32(w, h, (0..n).map(|i| [d[i * 2] as f32, d[i * 2 + 1] as f32, 0.0, 1.0]).collect()),
        TexFmt::Unknown(_) => Level::from_f32(w, h, vec![[0.0; 4]; n]),
    }
}

/// One BC1 block (8 bytes) → 16 RGBA texels (row-major). `one_bit_alpha`: the 3-colour mode's fourth
/// index is transparent black (BC1); inside BC3 the colour block is always 4-colour.
pub fn decode_bc1_block(b: &[u8], mode: Bc1Decode, one_bit_alpha: bool) -> [[f32; 4]; 16] {
    let c0 = u16::from_le_bytes([b[0], b[1]]);
    let c1 = u16::from_le_bytes([b[2], b[3]]);
    let bits = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
    let split = |c: u16| -> [u32; 3] { [((c >> 11) & 31) as u32, ((c >> 5) & 63) as u32, (c & 31) as u32] };
    let (a, bb) = (split(c0), split(c1));
    let expand = |v: u32, bits5: bool| -> f32 {
        match mode {
            Bc1Decode::Ideal => if bits5 { v as f32 / 31.0 } else { v as f32 / 63.0 },
            _ => {
                let e = if bits5 { (v << 3) | (v >> 2) } else { (v << 2) | (v >> 4) };
                e as f32 / 255.0
            }
        }
    };
    let ea = [expand(a[0], true), expand(a[1], false), expand(a[2], true)];
    let eb = [expand(bb[0], true), expand(bb[1], false), expand(bb[2], true)];
    let third = |x: f32, y: f32| -> f32 {
        // (2x + y) / 3
        match mode {
            Bc1Decode::Ideal => (2.0 * x + y) / 3.0,
            Bc1Decode::Expand8Trunc => {
                let (xi, yi) = ((x * 255.0).round() as u32, (y * 255.0).round() as u32);
                ((2 * xi + yi) / 3) as f32 / 255.0
            }
            Bc1Decode::Expand8Round => {
                let (xi, yi) = ((x * 255.0).round() as u32, (y * 255.0).round() as u32);
                ((2 * xi + yi + 1) / 3) as f32 / 255.0
            }
        }
    };
    let half = |x: f32, y: f32| -> f32 {
        match mode {
            Bc1Decode::Ideal => (x + y) / 2.0,
            _ => {
                let (xi, yi) = ((x * 255.0).round() as u32, (y * 255.0).round() as u32);
                ((xi + yi) / 2) as f32 / 255.0
            }
        }
    };
    let four = c0 > c1 || !one_bit_alpha;
    let pal: [[f32; 4]; 4] = if four {
        [
            [ea[0], ea[1], ea[2], 1.0],
            [eb[0], eb[1], eb[2], 1.0],
            [third(ea[0], eb[0]), third(ea[1], eb[1]), third(ea[2], eb[2]), 1.0],
            [third(eb[0], ea[0]), third(eb[1], ea[1]), third(eb[2], ea[2]), 1.0],
        ]
    } else {
        [
            [ea[0], ea[1], ea[2], 1.0],
            [eb[0], eb[1], eb[2], 1.0],
            [half(ea[0], eb[0]), half(ea[1], eb[1]), half(ea[2], eb[2]), 1.0],
            [0.0, 0.0, 0.0, 0.0],
        ]
    };
    let mut out = [[0.0f32; 4]; 16];
    for i in 0..16 {
        out[i] = pal[((bits >> (2 * i)) & 3) as usize];
    }
    out
}

/// One BC4 block (the BC3 alpha block, 8 bytes) → 16 alpha values.
pub fn decode_bc4_block(b: &[u8]) -> [f32; 16] {
    let a0 = b[0] as u32;
    let a1 = b[1] as u32;
    let mut bits = 0u64;
    for i in 0..6 {
        bits |= (b[2 + i] as u64) << (8 * i);
    }
    let pal: [f32; 8] = if a0 > a1 {
        [a0 as f32, a1 as f32, (6.0 * a0 as f32 + a1 as f32) / 7.0, (5.0 * a0 as f32 + 2.0 * a1 as f32) / 7.0, (4.0 * a0 as f32 + 3.0 * a1 as f32) / 7.0, (3.0 * a0 as f32 + 4.0 * a1 as f32) / 7.0, (2.0 * a0 as f32 + 5.0 * a1 as f32) / 7.0, (a0 as f32 + 6.0 * a1 as f32) / 7.0]
    } else {
        [a0 as f32, a1 as f32, (4.0 * a0 as f32 + a1 as f32) / 5.0, (3.0 * a0 as f32 + 2.0 * a1 as f32) / 5.0, (2.0 * a0 as f32 + 3.0 * a1 as f32) / 5.0, (a0 as f32 + 4.0 * a1 as f32) / 5.0, 0.0, 255.0]
    };
    let mut out = [0.0f32; 16];
    for i in 0..16 {
        out[i] = pal[((bits >> (3 * i)) & 7) as usize] / 255.0;
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Address {
    Wrap,
    Clamp,
    Mirror,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Point,
    Linear,
}

/// A D3D11 sampler state, plus the implementation choices the spec leaves open.
#[derive(Clone, Copy, Debug)]
pub struct Sampler {
    pub min_mag: Filter,
    pub mip: Option<Filter>,
    /// 1 = isotropic.
    pub max_aniso: u32,
    pub address_u: Address,
    pub address_v: Address,
    pub lod_bias: f32,
    pub min_lod: f32,
    pub max_lod: f32,
    /// Fractional bits of the bilinear weights (D3D11 7.18.7: at least 8); `None` = exact f32.
    pub weight_bits: Option<u32>,
}

impl Sampler {
    pub fn trilinear(address: Address) -> Sampler {
        Sampler { min_mag: Filter::Linear, mip: Some(Filter::Linear), max_aniso: 1, address_u: address, address_v: address, lod_bias: 0.0, min_lod: 0.0, max_lod: f32::MAX, weight_bits: Some(8) }
    }
    pub fn bilinear_no_mip(address: Address) -> Sampler {
        Sampler { min_mag: Filter::Linear, mip: None, max_aniso: 1, address_u: address, address_v: address, lod_bias: 0.0, min_lod: 0.0, max_lod: f32::MAX, weight_bits: Some(8) }
    }
}

/// Wrap/clamp/mirror an integer texel coordinate into [0, n).
#[inline]
fn address(i: i64, n: u32, mode: Address) -> u32 {
    let n = n as i64;
    match mode {
        Address::Wrap => i.rem_euclid(n) as u32,
        Address::Clamp => i.clamp(0, n - 1) as u32,
        Address::Mirror => {
            let p = i.rem_euclid(2 * n);
            (if p < n { p } else { 2 * n - 1 - p }) as u32
        }
    }
}

/// Quantise a bilinear fraction to `bits` fractional bits (truncation — 7.18.7's minimum precision is a
/// floor to the grid; `None` keeps the f32).
#[inline]
fn quant_frac(f: f32, bits: Option<u32>) -> f32 {
    match bits {
        Some(b) => ((f * (1u32 << b) as f32).floor()) / (1u32 << b) as f32,
        None => f,
    }
}

/// A bilinear (or point) fetch from one level at texture coordinates (u, v) in [0, 1) texture units.
pub fn fetch_level(lv: &Level, s: &Sampler, u: f32, v: f32) -> [f32; 4] {
    // a NaN coordinate: the hardware addresses texel 0 (what the pad's LUT lookups at |NaN| return in the capture)
    let (u, v) = (if u.is_nan() { 0.0 } else { u }, if v.is_nan() { 0.0 } else { v });
    let fx = u * lv.w as f32 - 0.5;
    let fy = v * lv.h as f32 - 0.5;
    if s.min_mag == Filter::Point {
        // point: the texel containing (u, v)
        let x = address((u * lv.w as f32).floor() as i64, lv.w, s.address_u);
        let y = address((v * lv.h as f32).floor() as i64, lv.h, s.address_v);
        return lv.get(x, y);
    }
    let (x0, y0) = (fx.floor(), fy.floor());
    let tx = quant_frac(fx - x0, s.weight_bits);
    let ty = quant_frac(fy - y0, s.weight_bits);
    let (xa, xb) = (address(x0 as i64, lv.w, s.address_u), address(x0 as i64 + 1, lv.w, s.address_u));
    let (ya, yb) = (address(y0 as i64, lv.h, s.address_v), address(y0 as i64 + 1, lv.h, s.address_v));
    let (p00, p10, p01, p11) = (lv.get(xa, ya), lv.get(xb, ya), lv.get(xa, yb), lv.get(xb, yb));
    let mut o = [0.0f32; 4];
    for k in 0..4 {
        let top = p00[k] * (1.0 - tx) + p10[k] * tx;
        let bot = p01[k] * (1.0 - tx) + p11[k] * tx;
        o[k] = top * (1.0 - ty) + bot * ty;
    }
    o
}

/// The D3D11 LOD of a footprint given the texture-space derivatives (in texels of level 0) — isotropic:
/// log2(max(|ddx|, |ddy|)); anisotropic: log2(major / ratio) with ratio = min(major / minor, maxAniso),
/// and the number of taps along the major axis (7.18.11).
pub fn lod_and_ratio(ddx: [f32; 2], ddy: [f32; 2], max_aniso: u32) -> (f32, f32, [f32; 2]) {
    let lx = (ddx[0] * ddx[0] + ddx[1] * ddx[1]).sqrt();
    let ly = (ddy[0] * ddy[0] + ddy[1] * ddy[1]).sqrt();
    let (major, minor, axis) = if lx >= ly { (lx, ly, ddx) } else { (ly, lx, ddy) };
    if max_aniso <= 1 {
        return (major.max(f32::MIN_POSITIVE).log2(), 1.0, axis);
    }
    let ratio = if minor > 0.0 { (major / minor).min(max_aniso as f32).max(1.0) } else { max_aniso as f32 };
    ((major / ratio).max(f32::MIN_POSITIVE).log2(), ratio, axis)
}

/// `sample` of a texture2d(array): (u, v) and the per-pixel derivatives of (u, v) in texture units.
/// The LOD is computed on level 0's size, clamped to the sampler's and the container's range; a
/// fractional LOD blends two levels when the mip filter is linear; anisotropy takes `ceil(ratio)` taps
/// evenly spread over the major axis (the reference's placement — hardware differs, the capture decides).
pub fn sample(tex: &Texture, slice: u32, s: &Sampler, uv: [f32; 2], ddx: [f32; 2], ddy: [f32; 2]) -> [f32; 4] {
    let slice = slice.min(tex.slices - 1);
    let levels = &tex.levels[slice as usize];
    let (w, h) = (tex.w as f32, tex.h as f32);
    let (lod, ratio, axis) = lod_and_ratio([ddx[0] * w, ddx[1] * h], [ddy[0] * w, ddy[1] * h], s.max_aniso);
    let lod = (lod + s.lod_bias).clamp(s.min_lod, s.max_lod).clamp(0.0, (levels.len() - 1) as f32);
    let taps = if s.max_aniso > 1 { ratio.ceil().max(1.0) as u32 } else { 1 };
    let mut acc = [0.0f32; 4];
    for t in 0..taps {
        // taps at offsets (t + 0.5)/taps − 0.5 along the major axis (in texels of level 0 → texture units)
        let f = if taps > 1 { (t as f32 + 0.5) / taps as f32 - 0.5 } else { 0.0 };
        let p = [uv[0] + axis[0] / w * f, uv[1] + axis[1] / h * f];
        let c = match s.mip {
            None => fetch_level(&levels[0], s, p[0], p[1]),
            Some(Filter::Point) => fetch_level(&levels[lod.round().min((levels.len() - 1) as f32) as usize], s, p[0], p[1]),
            Some(Filter::Linear) => {
                let l0 = lod.floor();
                let fr = lod - l0;
                let a = fetch_level(&levels[l0 as usize], s, p[0], p[1]);
                if fr > 0.0 && (l0 as usize + 1) < levels.len() {
                    let b = fetch_level(&levels[l0 as usize + 1], s, p[0], p[1]);
                    let mut o = [0.0f32; 4];
                    for k in 0..4 {
                        o[k] = a[k] * (1.0 - fr) + b[k] * fr;
                    }
                    o
                } else {
                    a
                }
            }
        };
        for k in 0..4 {
            acc[k] += c[k] / taps as f32;
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bc1_block_decodes_its_palette() {
        // c0 = pure red (31,0,0), c1 = pure blue (0,0,31), indices: row 0 all 0, row 1 all 1, row 2 all 2, row 3 all 3
        let c0: u16 = 31 << 11;
        let c1: u16 = 31;
        let mut b = [0u8; 8];
        b[..2].copy_from_slice(&c0.to_le_bytes());
        b[2..4].copy_from_slice(&c1.to_le_bytes());
        let bits: u32 = 0b11111111_10101010_01010101_00000000;
        b[4..8].copy_from_slice(&bits.to_le_bytes());
        let t = decode_bc1_block(&b, Bc1Decode::Ideal, true);
        assert_eq!(t[0], [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(t[4], [0.0, 0.0, 1.0, 1.0]);
        assert!((t[8][0] - 2.0 / 3.0).abs() < 1e-6 && (t[8][2] - 1.0 / 3.0).abs() < 1e-6);
        assert!((t[12][0] - 1.0 / 3.0).abs() < 1e-6 && (t[12][2] - 2.0 / 3.0).abs() < 1e-6);
        // c0 <= c1 → the 3-colour mode with transparent black at index 3
        let t3 = decode_bc1_block(&[c1.to_le_bytes()[0], c1.to_le_bytes()[1], c0.to_le_bytes()[0], c0.to_le_bytes()[1], b[4], b[5], b[6], b[7]], Bc1Decode::Ideal, true);
        assert_eq!(t3[12], [0.0, 0.0, 0.0, 0.0]);
        assert!((t3[8][0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn addressing_modes() {
        assert_eq!(address(-1, 8, Address::Wrap), 7);
        assert_eq!(address(8, 8, Address::Wrap), 0);
        assert_eq!(address(-1, 8, Address::Clamp), 0);
        assert_eq!(address(9, 8, Address::Clamp), 7);
        assert_eq!(address(-1, 8, Address::Mirror), 0);
        assert_eq!(address(8, 8, Address::Mirror), 7);
        assert_eq!(address(16, 8, Address::Mirror), 0);
    }

    #[test]
    fn bilinear_at_texel_centres_is_exact_and_lod_picks_the_level() {
        let lv0 = Level::from_f32(2, 2, vec![[0.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [1.0, 1.0, 0.0, 1.0]]);
        let lv1 = Level::from_f32(1, 1, vec![[0.5, 0.5, 0.0, 1.0]]);
        let tex = Texture { fmt: TexFmt::Rgba8, w: 2, h: 2, mips: 2, slices: 1, levels: vec![vec![lv0, lv1]], complete: true };
        let s = Sampler::trilinear(Address::Clamp);
        // no derivatives → level 0, the texel centre (0.75, 0.25) = texel (1, 0)
        assert_eq!(sample(&tex, 0, &s, [0.75, 0.25], [0.0, 0.0], [0.0, 0.0]), [1.0, 0.0, 0.0, 1.0]);
        // a footprint of two texels per pixel → LOD 1 → the 1×1 level
        assert_eq!(sample(&tex, 0, &s, [0.5, 0.5], [1.0, 0.0], [0.0, 1.0]), [0.5, 0.5, 0.0, 1.0]);
        // the centre of level 0 blends the four texels equally
        let c = sample(&tex, 0, &s, [0.5, 0.5], [0.0, 0.0], [0.0, 0.0]);
        assert!((c[0] - 0.5).abs() < 1e-6 && (c[1] - 0.5).abs() < 1e-6);
    }
}

impl Texture {
    /// Decode the colour channels of every level from sRGB to linear — what a `_UNORM_SRGB` shader resource view
    /// does before filtering (the alpha channel stays linear).
    pub fn decode_srgb(&mut self) {
        let mut t = [0f32; 256];
        for (i, v) in t.iter_mut().enumerate() {
            *v = crate::gpufmt::srgb_to_linear(i as f32 / 255.0);
        }
        let lut = std::sync::Arc::new(t);
        for sl in &mut self.levels {
            for lv in sl {
                match &mut lv.px {
                    Px::U8(_) => lv.lut = lut.clone(),
                    Px::F32(v) => {
                        for p in v.iter_mut() {
                            for k in 0..3 {
                                p[k] = crate::gpufmt::srgb_to_linear(p[k]);
                            }
                        }
                    }
                }
            }
        }
    }
}
