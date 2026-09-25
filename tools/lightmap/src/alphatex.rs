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
    /// Per 8×8 texel block the level's (min, max) alpha — the filtered sample of a footprint whose
    /// touched texels are all ≥ 129 passes the 128/255 test and one whose texels are all ≤ 127 fails it,
    /// so most fragments of a card skip the taps (`AlphaTex::passes`). Built by `with_blocks`.
    pub blocks: Vec<(u8, u8)>,
    pub bw: usize,
    /// Per texel (x, y) the CLASS of the 2×2 neighbourhood a bilinear tap reads at it — texels (x, x+1) ×
    /// (y, y+1), the neighbours clamped to the level: 0 = all ≤ 127 (the tap fails the 128/255 test whatever
    /// its weights), 1 = all ≥ 129 (it passes), 2 = mixed (the weights decide).
    pub cls: Vec<u8>,
    /// The fraction of `cls` that is mixed: when most neighbourhoods straddle the threshold (the coarse
    /// levels of a leaf texture average to ~0.5 everywhere) the early-outs cost more than they save and
    /// `passes_planned` samples directly.
    pub mixed_frac: f32,
}

impl AlphaLevel {
    pub const BLOCK: usize = 8;
    /// The block table of a level.
    pub fn with_blocks(w: usize, h: usize, a: Vec<u8>) -> AlphaLevel {
        let bw = (w + Self::BLOCK - 1) / Self::BLOCK;
        let bh = (h + Self::BLOCK - 1) / Self::BLOCK;
        let mut blocks = vec![(255u8, 0u8); bw * bh];
        for y in 0..h {
            for x in 0..w {
                let v = a[y * w + x];
                let b = &mut blocks[(y / Self::BLOCK) * bw + x / Self::BLOCK];
                b.0 = b.0.min(v);
                b.1 = b.1.max(v);
            }
        }
        let mut cls = vec![2u8; w * h];
        for y in 0..h {
            let y1 = (y + 1).min(h - 1);
            for x in 0..w {
                let x1 = (x + 1).min(w - 1);
                let q = [a[y * w + x], a[y * w + x1], a[y1 * w + x], a[y1 * w + x1]];
                let (mn, mx) = (q.iter().copied().min().unwrap(), q.iter().copied().max().unwrap());
                cls[y * w + x] = if mn >= 129 { 1 } else if mx <= 127 { 0 } else { 2 };
            }
        }
        let mixed_frac = cls.iter().filter(|c| **c == 2).count() as f32 / (w * h).max(1) as f32;
        AlphaLevel { w, h, a, blocks, bw, cls, mixed_frac }
    }
    /// The (min, max) alpha over the texels [x0, x1] × [y0, y1] (inclusive, clamped to the level).
    #[inline]
    pub fn minmax(&self, x0: i64, y0: i64, x1: i64, y1: i64) -> (u8, u8) {
        let (x0, x1) = (x0.clamp(0, self.w as i64 - 1) as usize, x1.clamp(0, self.w as i64 - 1) as usize);
        let (y0, y1) = (y0.clamp(0, self.h as i64 - 1) as usize, y1.clamp(0, self.h as i64 - 1) as usize);
        let (mut mn, mut mx) = (255u8, 0u8);
        for by in y0 / Self::BLOCK..=y1 / Self::BLOCK {
            for bx in x0 / Self::BLOCK..=x1 / Self::BLOCK {
                let b = self.blocks[by * self.bw + bx];
                mn = mn.min(b.0);
                mx = mx.max(b.1);
            }
        }
        (mn, mx)
    }
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
            levels.push(AlphaLevel::with_blocks(mw, mh, a));
        }
        if levels.is_empty() {
            return Err("no mip level decoded".into());
        }
        if alpha_stats_on() { eprintln!("alpha texture {}×{}: per level mixed fraction {:?}", levels[0].w, levels[0].h, levels.iter().map(|l| format!("{}x{}:{:.2}", l.w, l.h, l.mixed_frac)).collect::<Vec<_>>()); }
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

    /// The part of the test that is constant across a triangle (the footprint is): the level of detail,
    /// the two levels it blends and the fraction, the taps' axis and count — `passes_planned` per fragment.
    pub fn plan(&self, fp: &Footprint, aniso: usize) -> TapPlan {
        let (lod, axis, n) = if aniso > 1 { (fp.lod_aniso(aniso), fp.major_axis(), fp.taps(aniso).max(1)) } else { (fp.lod_iso(), [0.0, 0.0], 1) };
        let last = (self.levels.len() - 1) as f32;
        let lc = lod.clamp(0.0, last);
        let l0 = lc.floor();
        let t = lc - l0;
        let two = t > 0.0 && l0 < last;
        // whether the early-outs are worth trying: every tap at every level must find a uniform neighbourhood
        // (and all on one side) for them to decide — with the level's mixed fraction m the chance is about
        // (1 − m)^(taps × levels); below 0.3 the pre-checks cost more than they save (the giant's leaves at
        // levels 4–5: m ≈ 0.3, 2–3 taps, two levels → 8 % of the tests decided early, 92 % paid the checks)
        let m = self.levels[l0 as usize].mixed_frac;
        let checks = (n * if two { 2 } else { 1 }) as i32;
        let try_early = (1.0 - m).powi(checks) >= 0.3;
        TapPlan { lod, l0: l0 as usize, l1: if two { l0 as usize + 1 } else { l0 as usize }, two, t, axis, n, aniso, try_early }
    }

    /// The bilinear sample of one level with ClampEdge addressing — `sample_level`'s arithmetic, the
    /// addressing branches resolved (the plan's levels are known).
    #[inline]
    fn sample_level_clamp(l: &AlphaLevel, u: f32, v: f32) -> f32 {
        let (w, h) = (l.w as i64, l.h as i64);
        let fx = u.clamp(0.0, 1.0) * w as f32 - 0.5;
        let fy = v.clamp(0.0, 1.0) * h as f32 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (((fx - x0) * 256.0).floor() / 256.0, ((fy - y0) * 256.0).floor() / 256.0);
        let (xa, xb) = ((x0 as i64).clamp(0, w - 1) as usize, (x0 as i64 + 1).clamp(0, w - 1) as usize);
        let (ya, yb) = ((y0 as i64).clamp(0, h - 1) as usize, (y0 as i64 + 1).clamp(0, h - 1) as usize);
        let p = |x: usize, y: usize| -> f32 { l.a[y * l.w + x] as f32 / 255.0 };
        (p(xa, ya) * (1.0 - tx) + p(xb, ya) * tx) * (1.0 - ty) + (p(xa, yb) * (1.0 - tx) + p(xb, yb) * tx) * ty
    }

    /// `sample_aniso` / `sample_lod` with the plan's levels and fraction (the same arithmetic per tap:
    /// `a + (b − a)·t`, the taps averaged), ClampEdge.
    #[inline]
    fn sample_planned_clamp(&self, u: f32, v: f32, p: &TapPlan) -> f32 {
        let l0 = &self.levels[p.l0];
        let l1 = &self.levels[p.l1];
        let one = |uu: f32, vv: f32| -> f32 {
            let a = Self::sample_level_clamp(l0, uu, vv);
            if !p.two {
                return a;
            }
            let b = Self::sample_level_clamp(l1, uu, vv);
            a + (b - a) * p.t
        };
        if p.aniso > 1 && p.n > 1 {
            let n = p.n;
            let mut sum = 0.0f32;
            for i in 0..n {
                let s = (i as f32 + 0.5) / n as f32 - 0.5;
                sum += one(u + p.axis[0] * s, v + p.axis[1] * s);
            }
            sum / n as f32
        } else {
            one(u, v)
        }
    }

    /// `passes` with the triangle's plan: the exact early-outs (the 8×8 block min/max over the taps' span at
    /// both levels; then every tap's 2×2 class at both levels — all 1 = pass, all 0 = fail), the filtered
    /// sample only when a tap's neighbourhood straddles the threshold (or the taps disagree). The sampled
    /// answer is `passes_sampled`'s to the bit (the same lod / axis / tap count feed the same arithmetic).
    pub fn passes_planned(&self, u: f32, v: f32, p: &TapPlan, threshold: f32, addr: Address) -> bool {
        let stats = alpha_stats_on();
        if stats { ALPHA_STATS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed); ALPHA_STATS[4].fetch_add(p.n as u64, std::sync::atomic::Ordering::Relaxed); ALPHA_STATS[5].fetch_add(p.l0 as u64, std::sync::atomic::Ordering::Relaxed); }
        let clamp_thr = addr == Address::ClampEdge && threshold > 127.5 / 255.0 && threshold < 128.5 / 255.0;
        if clamp_thr && p.try_early {
            let n = p.n;
            let half = if n > 1 { 0.5 - 0.5 / n as f32 } else { 0.0 };
            let (u0, u1) = ((u - p.axis[0].abs() * half).clamp(0.0, 1.0), (u + p.axis[0].abs() * half).clamp(0.0, 1.0));
            let (v0, v1) = ((v - p.axis[1].abs() * half).clamp(0.0, 1.0), (v + p.axis[1].abs() * half).clamp(0.0, 1.0));
            let mut mn = 255u8;
            let mut mx = 0u8;
            for lv in [p.l0, p.l1] {
                let l = &self.levels[lv];
                let (x0, x1) = ((u0 * l.w as f32 - 0.5).floor() as i64, (u1 * l.w as f32 - 0.5).floor() as i64 + 1);
                let (y0, y1) = ((v0 * l.h as f32 - 0.5).floor() as i64, (v1 * l.h as f32 - 0.5).floor() as i64 + 1);
                let (a, b) = l.minmax(x0, y0, x1, y1);
                mn = mn.min(a);
                mx = mx.max(b);
                if !p.two { break; }
            }
            if mn >= 129 { if stats { ALPHA_STATS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed); } return true; }
            if mx <= 127 { if stats { ALPHA_STATS[1].fetch_add(1, std::sync::atomic::Ordering::Relaxed); } return false; }
            // per tap, per level: the 2×2 class at the tap's bilinear anchor
            let mut all = 3u8; // bit 0 = a class-0 tap seen, bit 1 = class 1; start as "nothing seen"
            let mut seen0 = false;
            let mut seen1 = false;
            let mut mixed = false;
            'taps: for i in 0..n {
                let s = if n > 1 { (i as f32 + 0.5) / n as f32 - 0.5 } else { 0.0 };
                let (uu, vv) = if n > 1 { (u + p.axis[0] * s, v + p.axis[1] * s) } else { (u, v) };
                for lv in [p.l0, p.l1] {
                    let l = &self.levels[lv];
                    let fx = uu.clamp(0.0, 1.0) * l.w as f32 - 0.5;
                    let fy = vv.clamp(0.0, 1.0) * l.h as f32 - 0.5;
                    let xa = (fx.floor() as i64).clamp(0, l.w as i64 - 1) as usize;
                    let ya = (fy.floor() as i64).clamp(0, l.h as i64 - 1) as usize;
                    match l.cls[ya * l.w + xa] {
                        0 => seen0 = true,
                        1 => seen1 = true,
                        _ => { mixed = true; break 'taps; }
                    }
                    if !p.two { break; }
                }
            }
            let _ = &mut all;
            if !mixed {
                if seen1 && !seen0 { if stats { ALPHA_STATS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed); } return true; }
                if seen0 && !seen1 { if stats { ALPHA_STATS[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed); } return false; }
            }
        }
        if stats { ALPHA_STATS[3].fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
        let a = if addr == Address::ClampEdge { self.sample_planned_clamp(u, v, p) } else if p.aniso > 1 { self.sample_aniso(u, v, p.lod, p.axis, p.n, addr) } else { self.sample_lod(u, v, p.lod, addr) };
        a - threshold >= 0.0
    }

    /// The alpha test of PS 17134 at one fragment: the filtered alpha ≥ `threshold` (128/255).
    pub fn passes(&self, u: f32, v: f32, fp: &Footprint, threshold: f32, addr: Address, aniso: usize) -> bool {
        // THE EXACT EARLY-OUT: every tap is a convex combination (8-bit weights, 1 − t exact) of texels of
        // the two levels around the lod, within the taps' span along the major axis ± one texel; when
        // those texels are all ≥ 129/255 the filtered alpha is ≥ 129/255 − ε > 128/255, when all ≤ 127/255
        // it is < 128/255 — the test's answer without the taps (a texel of exactly 128 forces the taps)
        if addr == Address::ClampEdge && threshold > 127.5 / 255.0 && threshold < 128.5 / 255.0 {
            let (lod, axis, n) = if aniso > 1 { (fp.lod_aniso(aniso), fp.major_axis(), fp.taps(aniso).max(1)) } else { (fp.lod_iso(), [0.0, 0.0], 1) };
            let last = (self.levels.len() - 1) as f32;
            let lod = lod.clamp(0.0, last);
            let l0 = lod.floor() as usize;
            let two = lod - lod.floor() > 0.0 && (l0 as f32) < last;
            // the taps' span in texture coordinates (s from −0.5 + 0.5/n to 0.5 − 0.5/n)
            let half = if n > 1 { 0.5 - 0.5 / n as f32 } else { 0.0 };
            let (u0, u1) = ((u - axis[0].abs() * half).clamp(0.0, 1.0), (u + axis[0].abs() * half).clamp(0.0, 1.0));
            let (v0, v1) = ((v - axis[1].abs() * half).clamp(0.0, 1.0), (v + axis[1].abs() * half).clamp(0.0, 1.0));
            let mut mn = 255u8;
            let mut mx = 0u8;
            for lv in [l0, if two { l0 + 1 } else { l0 }] {
                let l = &self.levels[lv.min(self.levels.len() - 1)];
                // texel index range touched by bilinear at any point of the span: floor(f·w − 0.5) .. +1
                let (x0, x1) = ((u0 * l.w as f32 - 0.5).floor() as i64, (u1 * l.w as f32 - 0.5).floor() as i64 + 1);
                let (y0, y1) = ((v0 * l.h as f32 - 0.5).floor() as i64, (v1 * l.h as f32 - 0.5).floor() as i64 + 1);
                let (a, b) = l.minmax(x0, y0, x1, y1);
                mn = mn.min(a);
                mx = mx.max(b);
            }
            if mn >= 129 {
                return true;
            }
            if mx <= 127 {
                return false;
            }
        }
        let a = if aniso > 1 { self.sample_aniso(u, v, fp.lod_aniso(aniso), fp.major_axis(), fp.taps(aniso), addr) } else { self.sample_lod(u, v, fp.lod_iso(), addr) };
        a - threshold >= 0.0
    }

    /// The test without the early-out (for the check below).
    pub fn passes_sampled(&self, u: f32, v: f32, fp: &Footprint, threshold: f32, addr: Address, aniso: usize) -> bool {
        let a = if aniso > 1 { self.sample_aniso(u, v, fp.lod_aniso(aniso), fp.major_axis(), fp.taps(aniso), addr) } else { self.sample_lod(u, v, fp.lod_iso(), addr) };
        a - threshold >= 0.0
    }
}

/// LMTOOL_ALPHA_STATS=1: [tests, block early-outs, class early-outs, full samples, Σ taps, Σ l0].
pub static ALPHA_STATS: [std::sync::atomic::AtomicU64; 6] = [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];
pub fn alpha_stats_on() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("LMTOOL_ALPHA_STATS").is_some())
}
pub fn alpha_stats_report() {
    if alpha_stats_on() {
        // (the per-level mixed fractions are printed once at load: `AlphaTex::from_dds`)
        let g = |i: usize| ALPHA_STATS[i].load(std::sync::atomic::Ordering::Relaxed);
        eprintln!("alpha stats: {} tests, {} block early-outs, {} class early-outs, {} full samples; mean taps {:.2}, mean l0 {:.2}", g(0), g(1), g(2), g(3), g(4) as f64 / g(0).max(1) as f64, g(5) as f64 / g(0).max(1) as f64);
        for c in &ALPHA_STATS { c.store(0, std::sync::atomic::Ordering::Relaxed); }
    }
}

/// The per-triangle constants of the alpha test (`AlphaTex::plan`).
#[derive(Clone, Copy, Debug)]
pub struct TapPlan {
    pub lod: f32,
    pub l0: usize,
    pub l1: usize,
    pub two: bool,
    /// The blend fraction between `l0` and `l1` (`sample_lod`'s `lod − floor(lod)` after the clamp).
    pub t: f32,
    pub axis: [f32; 2],
    pub n: usize,
    pub aniso: usize,
    /// Try the exact early-outs before sampling (see `plan`).
    pub try_early: bool,
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
mod early_out_tests {
    use super::*;

    #[test]
    fn the_early_out_never_changes_the_answer() {
        let mut seed = 987654321u64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % 100_000) as f32 / 100_000.0 };
        // a 64×64 texture with blotches around the threshold, and its 2× down-sampled chain
        let w = 64usize;
        let mut a0 = vec![0u8; w * w];
        for y in 0..w { for x in 0..w { let d = (((x as f32 - 32.0).powi(2) + (y as f32 - 32.0).powi(2)).sqrt() / 20.0).min(1.0); a0[y * w + x] = ((1.0 - d) * 255.0 + (rnd() - 0.5) * 60.0).clamp(0.0, 255.0) as u8; } }
        let mut levels = vec![AlphaLevel::with_blocks(w, w, a0.clone())];
        let mut cur = a0; let mut cw = w;
        while cw > 1 {
            let nw = cw / 2;
            let mut n = vec![0u8; nw * nw];
            for y in 0..nw { for x in 0..nw { let s = cur[2 * y * cw + 2 * x] as u32 + cur[2 * y * cw + 2 * x + 1] as u32 + cur[(2 * y + 1) * cw + 2 * x] as u32 + cur[(2 * y + 1) * cw + 2 * x + 1] as u32; n[y * nw + x] = (s / 4) as u8; } }
            levels.push(AlphaLevel::with_blocks(nw, nw, n.clone()));
            cur = n; cw = nw;
        }
        let tex = AlphaTex { levels, flipped: false };
        // and a second texture hovering around the threshold everywhere (its levels are mostly mixed: the
        // direct sampling path)
        let mut b0 = vec![0u8; w * w];
        for y in 0..w { for x in 0..w { b0[y * w + x] = (128.0 + (rnd() - 0.5) * 40.0 + ((x + y) % 2) as f32 * 3.0) as u8; } }
        let mut levels2 = vec![AlphaLevel::with_blocks(w, w, b0.clone())];
        let mut cur = b0; let mut cw = w;
        while cw > 1 {
            let nw = cw / 2;
            let mut n = vec![0u8; nw * nw];
            for y in 0..nw { for x in 0..nw { let s = cur[2 * y * cw + 2 * x] as u32 + cur[2 * y * cw + 2 * x + 1] as u32 + cur[(2 * y + 1) * cw + 2 * x] as u32 + cur[(2 * y + 1) * cw + 2 * x + 1] as u32; n[y * nw + x] = (s / 4) as u8; } }
            levels2.push(AlphaLevel::with_blocks(nw, nw, n.clone()));
            cur = n; cw = nw;
        }
        let tex2 = AlphaTex { levels: levels2, flipped: false };
        assert!(tex2.levels[0].mixed_frac > 0.5 && tex.levels[0].mixed_frac < 0.5, "{} {}", tex2.levels[0].mixed_frac, tex.levels[0].mixed_frac);
        let thr = 0.501_960_813_999_176f32;
        for tex in [&tex, &tex2] {
        for _ in 0..20000 {
            let (u, v) = (rnd(), rnd());
            let fp = Footprint { dx: [(rnd() - 0.5) * 6.0, (rnd() - 0.5) * 6.0], dy: [(rnd() - 0.5) * 6.0, (rnd() - 0.5) * 6.0], w: w as f32, h: w as f32 };
            for aniso in [1usize, 16] {
                assert_eq!(tex.passes(u, v, &fp, thr, Address::ClampEdge, aniso), tex.passes_sampled(u, v, &fp, thr, Address::ClampEdge, aniso), "u {u} v {v} fp {fp:?} aniso {aniso}");
                let plan = tex.plan(&fp, aniso);
                assert_eq!(tex.passes_planned(u, v, &plan, thr, Address::ClampEdge), tex.passes_sampled(u, v, &fp, thr, Address::ClampEdge, aniso), "planned: u {u} v {v} fp {fp:?} aniso {aniso}");
            }
        }
        }
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
        AlphaTex { levels: vec![AlphaLevel::with_blocks(4, 4, a0), AlphaLevel::with_blocks(2, 2, vec![128; 4])], flipped: false }
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
