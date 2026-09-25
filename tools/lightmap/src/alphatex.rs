//! THE CUT-OUT TEXTURES AS THE GAME SAMPLES THEM (rows 5/6, port engineer D): the vegetation cards'
//! alpha test in the peel — PS 17134 (frame 127448 draw eid 2894 / 2913):
//!
//! ```text
//!   r0.x = TMapOpacityInAlpha_Gbx_Opacity.Sample(SMapOpacityInAlpha_Gbx_Opacity, v1.zw).w   (v1.zw = the card's TexCoord0)
//!   if (r0.x − GbxShadowAlphaThreshold < 0) discard      (GbxShadowAlphaThreshold = 0.501960813999176 = 128/255)
//! ```
//!
//! The texture is the item zip's DDS (BC3, a full mip chain — the tiny library's chains preserve the
//! alpha coverage per level, so the coarse levels carry a boosted alpha) UPLOADED BOTTOM-UP: the GPU
//! textures 14585 / 14579 of the capture equal the DDS files `VegetPalmTreeSugar_D_in0` / `_D` with their
//! rows reversed (100 % of the texels agree flipped, 64 % unflipped) — texture v addresses file row
//! (1 − v)·h. The sample is a filtered one (the material sampler: anisotropic ×16 in the capture's
//! sibling draw eid 394, ClampEdge; mip bias 0): per fragment the level of detail follows the
//! screen-space derivatives of the texture coordinate, so the alpha the threshold sees is the
//! trilinear (or anisotropic) blend of the mip chain at the peel's pixel footprint — a coarse footprint
//! (the 2.4 km world peel: 0.6 m per pixel) reads the boosted coarse levels, a fine one (the fitted peel:
//! 1 cm) the top level bilinearly.
//!
//! `AlphaTex` holds every level's alpha bytes; `sample_lod` is the D3D11 filter (8-bit fractional weights,
//! ClampEdge or Wrap, trilinear between the two nearest levels); `lod_isotropic` / `Footprint` give the
//! level of detail from a triangle's uv Jacobian as the rasteriser's derivatives would.

/// One texture's alpha channel, every mip level (level 0 first), rows in TEXTURE order (v = 0 first —
/// the DDS file's rows reversed when `flipped`).
#[derive(Clone, Debug)]
pub struct AlphaTex {
    pub levels: Vec<AlphaLevel>,
    /// The texture was stored bottom-up in the file (the game's loader reverses the rows).
    pub flipped: bool,
}

#[derive(Clone, Debug)]
pub struct AlphaLevel {
    pub w: usize,
    pub h: usize,
    pub a: Vec<u8>,
}

/// The sampler's addressing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Address {
    Wrap,
    ClampEdge,
}

/// The 16 alphas of one BC3 alpha block (two 8-bit endpoints, 3-bit indices; the 8-value mode when
/// a0 > a1, else the 6-value mode with 0 and 255).
pub fn bc3_alpha_block(block: &[u8]) -> [u8; 16] {
    let (a0, a1) = (block[0] as u32, block[1] as u32);
    let mut pal = [0u8; 8];
    pal[0] = a0 as u8;
    pal[1] = a1 as u8;
    if a0 > a1 {
        for i in 1..7u32 {
            pal[(i + 1) as usize] = (((7 - i) * a0 + i * a1) / 7) as u8;
        }
    } else {
        for i in 1..5u32 {
            pal[(i + 1) as usize] = (((5 - i) * a0 + i * a1) / 5) as u8;
        }
        pal[6] = 0;
        pal[7] = 255;
    }
    let mut bits: u64 = 0;
    for (i, b) in block[2..8].iter().enumerate() {
        bits |= (*b as u64) << (8 * i);
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = pal[((bits >> (3 * i)) & 7) as usize];
    }
    out
}

/// The 16 alphas of one BC1 block: 255, or 0 for the transparent index of the 3-colour mode (c0 ≤ c1).
pub fn bc1_alpha_block(block: &[u8]) -> [u8; 16] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    let mut out = [255u8; 16];
    if c0 <= c1 {
        for i in 0..16 {
            if (bits >> (2 * i)) & 3 == 3 {
                out[i] = 0;
            }
        }
    }
    out
}

impl AlphaTex {
    /// Decode a BC1/BC3 (DXT1/DXT5, legacy or DX10 header) DDS's alpha mip chain. `flipped` marks the
    /// game's bottom-up upload (the rows are reversed here, so `a[0..w]` is texture row v = 0).
    pub fn from_dds(dds: &[u8], flipped: bool) -> Result<AlphaTex, String> {
        if dds.len() < 128 || &dds[..4] != b"DDS " {
            return Err("not a DDS".into());
        }
        let u = |o: usize| u32::from_le_bytes([dds[o], dds[o + 1], dds[o + 2], dds[o + 3]]);
        let (h, w, mips) = (u(12) as usize, u(16) as usize, u(28).max(1) as usize);
        let fourcc = &dds[84..88];
        let (bc3, off) = match fourcc {
            b"DXT5" => (true, 128usize),
            b"DXT1" => (false, 128usize),
            b"DX10" => {
                let fmt = u(128);
                // DXGI_FORMAT_BC3_TYPELESS 76, BC3_UNORM 77, BC3_UNORM_SRGB 78; BC1 70/71/72
                match fmt {
                    76..=78 => (true, 148usize),
                    70..=72 => (false, 148usize),
                    f => return Err(format!("DXGI format {f}: not BC1/BC3")),
                }
            }
            f => return Err(format!("{}: not BC1/BC3", String::from_utf8_lossy(f))),
        };
        let bpb = if bc3 { 16 } else { 8 };
        let mut levels = Vec::with_capacity(mips);
        let mut o = off;
        for m in 0..mips {
            let (mw, mh) = ((w >> m).max(1), (h >> m).max(1));
            let (bw, bh) = ((mw + 3) / 4, (mh + 3) / 4);
            if o + bw * bh * bpb > dds.len() {
                break;
            }
            let mut a = vec![0u8; mw * mh];
            for by in 0..bh {
                for bx in 0..bw {
                    let blk = &dds[o + (by * bw + bx) * bpb..o + (by * bw + bx + 1) * bpb];
                    let al = if bc3 { bc3_alpha_block(&blk[..8]) } else { bc1_alpha_block(blk) };
                    for py in 0..4 {
                        for px in 0..4 {
                            let (x, y) = (bx * 4 + px, by * 4 + py);
                            if x < mw && y < mh {
                                let row = if flipped { mh - 1 - y } else { y };
                                a[row * mw + x] = al[py * 4 + px];
                            }
                        }
                    }
                }
            }
            o += bw * bh * bpb;
            levels.push(AlphaLevel { w: mw, h: mh, a });
        }
        if levels.is_empty() {
            return Err("no mip level decoded".into());
        }
        Ok(AlphaTex { levels, flipped })
    }

    pub fn w(&self) -> usize {
        self.levels[0].w
    }
    pub fn h(&self) -> usize {
        self.levels[0].h
    }

    /// One level's bilinear sample (D3D11: the texel coordinate `u·w − 0.5`, its fraction as the weight
    /// with 8 bits — 1/256 steps), addressed per `addr`; the result in 0..1.
    pub fn sample_level(&self, level: usize, u: f32, v: f32, addr: Address) -> f32 {
        let l = &self.levels[level.min(self.levels.len() - 1)];
        let (w, h) = (l.w as i64, l.h as i64);
        let wrap = |x: f32, n: i64| -> f32 {
            match addr {
                Address::Wrap => x.rem_euclid(1.0) * n as f32,
                Address::ClampEdge => x.clamp(0.0, 1.0) * n as f32,
            }
        };
        let fx = wrap(u, w) - 0.5;
        let fy = wrap(v, h) - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (((fx - x0) * 256.0).floor() / 256.0, ((fy - y0) * 256.0).floor() / 256.0);
        let idx = |x: i64, n: i64| -> usize {
            match addr {
                Address::Wrap => x.rem_euclid(n) as usize,
                Address::ClampEdge => x.clamp(0, n - 1) as usize,
            }
        };
        let (xa, xb) = (idx(x0 as i64, w), idx(x0 as i64 + 1, w));
        let (ya, yb) = (idx(y0 as i64, h), idx(y0 as i64 + 1, h));
        let p = |x: usize, y: usize| -> f32 { l.a[y * l.w + x] as f32 / 255.0 };
        (p(xa, ya) * (1.0 - tx) + p(xb, ya) * tx) * (1.0 - ty) + (p(xa, yb) * (1.0 - tx) + p(xb, yb) * tx) * ty
    }

    /// The trilinear sample at `lod` (D3D11: the two levels around it blended by its fraction; ≤ 0 =
    /// the top level alone, beyond the chain = the last level).
    pub fn sample_lod(&self, u: f32, v: f32, lod: f32, addr: Address) -> f32 {
        let last = (self.levels.len() - 1) as f32;
        let lod = lod.clamp(0.0, last);
        let l0 = lod.floor();
        let t = lod - l0;
        let a = self.sample_level(l0 as usize, u, v, addr);
        if t <= 0.0 || l0 >= last {
            return a;
        }
        let b = self.sample_level(l0 as usize + 1, u, v, addr);
        a + (b - a) * t
    }

    /// The anisotropic sample (D3D11 reference behaviour): `n` taps spread along the footprint's major
    /// axis, each a trilinear sample at the minor-axis level of detail; `axis` = the major axis step per
    /// tap in texture coordinates (the whole footprint spans `−0.5..0.5` of it).
    pub fn sample_aniso(&self, u: f32, v: f32, lod: f32, axis: [f32; 2], n: usize, addr: Address) -> f32 {
        let n = n.max(1);
        if n == 1 {
            return self.sample_lod(u, v, lod, addr);
        }
        let mut sum = 0.0f32;
        for i in 0..n {
            let s = (i as f32 + 0.5) / n as f32 - 0.5;
            sum += self.sample_lod(u + axis[0] * s, v + axis[1] * s, lod, addr);
        }
        sum / n as f32
    }

    /// The alpha test of PS 17134 at one fragment: the filtered alpha ≥ `threshold` (128/255).
    pub fn passes(&self, u: f32, v: f32, fp: &Footprint, threshold: f32, addr: Address, aniso: usize) -> bool {
        let a = if aniso > 1 { self.sample_aniso(u, v, fp.lod_aniso(aniso), fp.major_axis(), fp.taps(aniso), addr) } else { self.sample_lod(u, v, fp.lod_iso(), addr) };
        a - threshold >= 0.0
    }
}

/// A triangle's texture-coordinate footprint per pixel: the screen-space derivatives of (u, v) in
/// TEXELS of the top level — constant across a triangle under an orthographic projection (the
/// rasteriser's per-quad differences equal the plane's gradient).
#[derive(Clone, Copy, Debug)]
pub struct Footprint {
    /// ∂(u·w, v·h)/∂x and ∂(u·w, v·h)/∂y in texels per pixel.
    pub dx: [f32; 2],
    pub dy: [f32; 2],
    /// The top level's size (to turn the major axis back into texture coordinates).
    pub w: f32,
    pub h: f32,
}

impl Footprint {
    /// From a triangle's pixel positions and TexCoord0 corners and the top level's size.
    pub fn of_triangle(px: [[f32; 2]; 3], uv: [[f32; 2]; 3], w: usize, h: usize) -> Footprint {
        let (ax, ay) = (px[1][0] - px[0][0], px[1][1] - px[0][1]);
        let (bx, by) = (px[2][0] - px[0][0], px[2][1] - px[0][1]);
        let det = ax * by - bx * ay;
        if det.abs() < 1e-12 {
            // an edge-on triangle: an infinite footprint (the coarsest level)
            return Footprint { dx: [1e9, 1e9], dy: [1e9, 1e9], w: w as f32, h: h as f32 };
        }
        let (du1, dv1) = ((uv[1][0] - uv[0][0]) * w as f32, (uv[1][1] - uv[0][1]) * h as f32);
        let (du2, dv2) = ((uv[2][0] - uv[0][0]) * w as f32, (uv[2][1] - uv[0][1]) * h as f32);
        // (u, v) = uv0 + s·(du1, dv1) + t·(du2, dv2) with (s, t) the barycentric coordinates along the two
        // edges; (s, t) as a function of (x, y): the inverse of [[ax, bx], [ay, by]]
        let inv = 1.0 / det;
        let (dsdx, dsdy) = (by * inv, -bx * inv);
        let (dtdx, dtdy) = (-ay * inv, ax * inv);
        Footprint { dx: [du1 * dsdx + du2 * dtdx, dv1 * dsdx + dv2 * dtdx], dy: [du1 * dsdy + du2 * dtdy, dv1 * dsdy + dv2 * dtdy], w: w as f32, h: h as f32 }
    }
    fn len_dx(&self) -> f32 {
        (self.dx[0] * self.dx[0] + self.dx[1] * self.dx[1]).sqrt()
    }
    fn len_dy(&self) -> f32 {
        (self.dy[0] * self.dy[0] + self.dy[1] * self.dy[1]).sqrt()
    }
    /// The isotropic level of detail: log2 of the larger derivative length (D3D11 3.2.3).
    pub fn lod_iso(&self) -> f32 {
        let m = self.len_dx().max(self.len_dy()).max(1e-30);
        m.log2()
    }
    /// The anisotropic level of detail: the major length over the (clamped) anisotropy ratio.
    pub fn lod_aniso(&self, max_aniso: usize) -> f32 {
        let (lx, ly) = (self.len_dx(), self.len_dy());
        let (major, minor) = (lx.max(ly).max(1e-30), lx.min(ly).max(1e-30));
        let ratio = (major / minor).min(max_aniso as f32).max(1.0);
        (major / ratio).log2()
    }
    /// The number of taps along the major axis: ceil(ratio), at most `max_aniso`.
    pub fn taps(&self, max_aniso: usize) -> usize {
        let (lx, ly) = (self.len_dx(), self.len_dy());
        let ratio = (lx.max(ly) / lx.min(ly).max(1e-30)).min(max_aniso as f32).max(1.0);
        ratio.ceil() as usize
    }
    /// The major axis in texture coordinates (the full footprint extent along it).
    pub fn major_axis(&self) -> [f32; 2] {
        let a = if self.len_dx() >= self.len_dy() { self.dx } else { self.dy };
        [a[0] / self.w, a[1] / self.h]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_level_tex() -> AlphaTex {
        // level 0: 4×4, the left half opaque; level 1: 2×2 all 128
        let mut a0 = vec![0u8; 16];
        for y in 0..4 {
            for x in 0..2 {
                a0[y * 4 + x] = 255;
            }
        }
        AlphaTex { levels: vec![AlphaLevel { w: 4, h: 4, a: a0 }, AlphaLevel { w: 2, h: 2, a: vec![128; 4] }], flipped: false }
    }

    #[test]
    fn bc3_alpha_blocks_decode_both_modes() {
        // 8-value mode: a0 = 255 > a1 = 0, all indices 0 → 255
        let blk = [255u8, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(bc3_alpha_block(&blk), [255u8; 16]);
        // index 1 everywhere (bits 001 repeated): 0b001001001... → a1 = 0
        let mut b = [255u8, 0, 0, 0, 0, 0, 0, 0];
        let mut bits: u64 = 0;
        for i in 0..16 { bits |= 1u64 << (3 * i); }
        b[2..8].copy_from_slice(&bits.to_le_bytes()[..6]);
        assert_eq!(bc3_alpha_block(&b), [0u8; 16]);
        // 6-value mode: a0 = 0 ≤ a1 = 255, index 7 → 255, index 6 → 0, index 2 → (4·0 + 1·255)/5 = 51
        let mut c = [0u8, 255, 0, 0, 0, 0, 0, 0];
        let mut bits: u64 = 0;
        for i in 0..16 { bits |= 2u64 << (3 * i); }
        c[2..8].copy_from_slice(&bits.to_le_bytes()[..6]);
        assert_eq!(bc3_alpha_block(&c)[0], 51);
    }

    #[test]
    fn bilinear_at_texel_centres_reads_the_texel_and_between_them_the_mean() {
        let t = two_level_tex();
        // texel (1, 0) centre: u = 1.5/4
        assert!((t.sample_level(0, 1.5 / 4.0, 0.5 / 4.0, Address::ClampEdge) - 1.0).abs() < 1e-6);
        assert!(t.sample_level(0, 2.5 / 4.0, 0.5 / 4.0, Address::ClampEdge).abs() < 1e-6);
        // half-way between texels 1 and 2: 0.5
        assert!((t.sample_level(0, 2.0 / 4.0, 0.5 / 4.0, Address::ClampEdge) - 0.5).abs() < 1e-6);
        // the clamp: u beyond 1 reads the last column (transparent); wrap reads the first (opaque)
        assert!(t.sample_level(0, 1.2, 0.1, Address::ClampEdge).abs() < 1e-6);
        assert!((t.sample_level(0, 1.125, 0.1, Address::Wrap) - 1.0).abs() < 1e-6, "wrap: u = 1.125 is texel 0's centre again");
    }

    #[test]
    fn trilinear_blends_the_two_levels_by_the_lod_fraction() {
        let t = two_level_tex();
        let a0 = t.sample_lod(1.5 / 4.0, 0.5 / 4.0, 0.0, Address::ClampEdge);
        let a1 = t.sample_lod(1.5 / 4.0, 0.5 / 4.0, 1.0, Address::ClampEdge);
        let ah = t.sample_lod(1.5 / 4.0, 0.5 / 4.0, 0.5, Address::ClampEdge);
        assert!((a0 - 1.0).abs() < 1e-6 && (a1 - 128.0 / 255.0).abs() < 1e-6);
        assert!((ah - (a0 + a1) * 0.5).abs() < 1e-6);
        // negative and over-range lods clamp to the chain
        assert!((t.sample_lod(1.5 / 4.0, 0.5 / 4.0, -3.0, Address::ClampEdge) - a0).abs() < 1e-6);
        assert!((t.sample_lod(1.5 / 4.0, 0.5 / 4.0, 9.0, Address::ClampEdge) - a1).abs() < 1e-6);
    }

    #[test]
    fn the_footprint_of_a_screen_aligned_quad_is_its_texel_density() {
        // a triangle 100 px wide mapping u 0..1 of a 200-texel texture: 2 texels per pixel along x → lod 1
        let px = [[0.0f32, 0.0], [100.0, 0.0], [0.0, 100.0]];
        let uv = [[0.0f32, 0.0], [1.0, 0.0], [0.0, 1.0]];
        let fp = Footprint::of_triangle(px, uv, 200, 200);
        assert!((fp.dx[0] - 2.0).abs() < 1e-5 && fp.dx[1].abs() < 1e-5, "{fp:?}");
        assert!((fp.dy[1] - 2.0).abs() < 1e-5 && fp.dy[0].abs() < 1e-5, "{fp:?}");
        assert!((fp.lod_iso() - 1.0).abs() < 1e-5);
        assert_eq!(fp.taps(16), 1);
        // squeezed 4× in y: anisotropy 4, the aniso lod follows the minor axis
        let px2 = [[0.0f32, 0.0], [100.0, 0.0], [0.0, 25.0]];
        let fp2 = Footprint::of_triangle(px2, uv, 200, 200);
        assert!((fp2.lod_iso() - 3.0).abs() < 1e-5, "{}", fp2.lod_iso());
        assert!((fp2.lod_aniso(16) - 1.0).abs() < 1e-5, "{}", fp2.lod_aniso(16));
        assert_eq!(fp2.taps(16), 4);
        // the anisotropy ratio is capped
        assert!((fp2.lod_aniso(2) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn a_flipped_decode_reverses_the_rows() {
        // a 4×4 DXT5 DDS: one alpha block with rows 255,255,0,0 (a0 = 255 > a1 = 0: index 0 = 255, index 1 = 0)
        let mut dds = vec![0u8; 128 + 16];
        dds[..4].copy_from_slice(b"DDS ");
        dds[4..8].copy_from_slice(&124u32.to_le_bytes());
        dds[12..16].copy_from_slice(&4u32.to_le_bytes());
        dds[16..20].copy_from_slice(&4u32.to_le_bytes());
        dds[28..32].copy_from_slice(&1u32.to_le_bytes());
        dds[84..88].copy_from_slice(b"DXT5");
        dds[128] = 255;
        dds[129] = 0;
        let mut bits: u64 = 0;
        for i in 8..16 { bits |= 1u64 << (3 * i); }
        dds[130..136].copy_from_slice(&bits.to_le_bytes()[..6]);
        let plain = AlphaTex::from_dds(&dds, false).unwrap();
        assert_eq!(&plain.levels[0].a[0..4], &[255, 255, 255, 255]);
        assert_eq!(&plain.levels[0].a[12..16], &[0, 0, 0, 0]);
        let flipped = AlphaTex::from_dds(&dds, true).unwrap();
        assert_eq!(&flipped.levels[0].a[0..4], &[0, 0, 0, 0]);
        assert_eq!(&flipped.levels[0].a[12..16], &[255, 255, 255, 255]);
    }
}
