//! THE CARDS' ALPHA TEST AGAINST THE CAPTURE — which anisotropic footprint rule reproduces every survive / discard
//! decision of PS 17134 (`TMapOpacityInAlpha.Sample(aniso ×16 ClampEdge, TexCoord0).w ≥ 128/255`) on the captured
//! peel layers.
//!
//! Oracle: the captured `peel_depth` layers of one (sweep, direction, peel) — layer k's depth buffer holds, per pixel,
//! the k-th nearest fragment that SURVIVED the game's alpha test (opaque geometry included). Our A-buffer build dumps
//! every card fragment BEFORE its alpha test (`peel::CARD_DUMP`: pixel, z01, triangle, TexCoord0, the alpha mask, the
//! footprint). A fragment whose depth appears in some captured layer at its pixel survived in the game; one whose
//! depth appears in none was discarded — provided the pixel is FULLY OBSERVED (its last captured layer is the clear
//! value, so no fragment was cut by the layer cap). Every candidate rule is then a pure function of (u, v, footprint,
//! the alpha mip chain) and is scored by its agreement with those labels: nothing is fitted.

use crate::alphatex::{Address, AlphaLevel, AlphaTex, Footprint};
use crate::peel::CardFrag;
use std::collections::HashMap;
use std::path::Path;

pub struct Dump {
    pub res: u32,
    pub res_y: u32,
    pub d: [f32; 3],
    pub frags: Vec<CardFrag>,
}

pub fn read_dump(p: &Path) -> Result<Dump, String> {
    let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if b.len() < 32 || &b[0..8] != b"CFRG0001" {
        return Err(format!("{}: not a card dump", p.display()));
    }
    let n = u64::from_le_bytes(b[8..16].try_into().unwrap()) as usize;
    let res = u32::from_le_bytes(b[16..20].try_into().unwrap());
    let res_y = u32::from_le_bytes(b[20..24].try_into().unwrap());
    let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let d = [f(24), f(28), f(32)];
    let mut frags = Vec::with_capacity(n);
    let mut o = 36;
    for _ in 0..n {
        if o + CardFrag::BYTES > b.len() {
            break;
        }
        frags.push(CardFrag::read(&b[o..o + CardFrag::BYTES]));
        o += CardFrag::BYTES;
    }
    // the A-buffer may be built twice per peel (the exact-layer pass + the sparse build): one record per (pixel, triangle, depth)
    let mut seen = std::collections::HashSet::new();
    frags.retain(|f| seen.insert((f.x, f.y, f.tri, f.z01.to_bits())));
    Ok(Dump { res, res_y, d, frags })
}

pub fn read_mask(p: &Path) -> Result<AlphaTex, String> {
    let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if b.len() < 16 || &b[0..8] != b"CMSK0001" {
        return Err(format!("{}: not a card mask", p.display()));
    }
    let n = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    let flipped = u32::from_le_bytes(b[12..16].try_into().unwrap()) != 0;
    let mut o = 16;
    let mut levels = Vec::with_capacity(n);
    for _ in 0..n {
        let w = u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
        let h = u32::from_le_bytes(b[o + 4..o + 8].try_into().unwrap()) as usize;
        o += 8;
        levels.push(AlphaLevel::with_blocks(w, h, b[o..o + w * h].to_vec()));
        o += w * h;
    }
    Ok(AlphaTex { levels, flipped })
}

/// One candidate rule: the filtered alpha at a fragment.
#[derive(Clone, Copy, Debug)]
pub enum Rule {
    /// D3D11 reference (the port's model): N = ceil(ratio) ≤ 16 taps at (i + ½)/N − ½ along the major axis, trilinear at
    /// log2(major / ratio).
    RefAniso { max_aniso: usize },
    /// Isotropic trilinear at log2(max(|dx|, |dy|)).
    IsoMajor,
    /// Isotropic trilinear at log2(min(|dx|, |dy|)).
    IsoMinor,
    /// Taps N = round(ratio).
    AnisoRound,
    /// Taps N = the next power of two ≥ ratio.
    AnisoPow2,
    /// N = ceil(ratio) taps at the endpoints inclusive: i/(N − 1) − ½.
    AnisoEndpoints,
    /// N = ceil(ratio), the span shortened by one minor length (the taps' centres cover major − minor).
    AnisoShortSpan,
    /// N = ceil(ratio), trilinear at log2(minor) (the minor length itself, no ratio clamp).
    AnisoLodMinor,
    /// N = ceil(ratio), the two nearest levels but the nearer one only (no trilinear blend).
    AnisoNearestLevel,
    /// The level-0 texel under the fragment (the point-sampled mask).
    Point,
    /// N = ceil(ratio), the level of detail from the AVERAGE of the two derivative lengths (some hardware).
    AnisoLodMean,
    /// N = ceil(ratio) with the LOD biased by −½ (a "sharper" vendor path) / +½.
    AnisoBias { bias: f32 },
    /// N = 2·ceil(ratio/2) (an even tap count), reference positions.
    AnisoEven,
    /// Reference taps but the trilinear weight quantised to 1/256 and the taps averaged in 8-bit steps.
    AnisoQuant,
    /// N = ceil(ratio), the level of detail from major / N (the integer tap count) instead of major / ratio.
    AnisoLodOverN,
    /// N = ceil(ratio) + 1 taps, reference positions and lod.
    AnisoPlusOne,
    /// N = 2·ceil(ratio) taps.
    AnisoDouble,
    /// N = ceil(ratio) taps at i/N − ½ (the first tap at the footprint's start, none at its end).
    AnisoNoHalf,
    /// N = ceil(ratio) taps, the span widened by one tap spacing (the taps cover major·(1 + 1/N)).
    AnisoWideSpan,
    /// Always 16 taps over the major axis; the level from major/16 (`lod_from_span`) or from the minor length.
    Fixed16 { lod_from_span: bool },
    /// 2·ceil(ratio) taps at the level of major / (2·ratio) (twice the taps, one level finer).
    DoubleFine,
    /// The reference rule at the sample point moved by (sx, sy) PIXELS along the footprint's derivatives (a pixel-centre
    /// convention difference between the rasterisers would look like this).
    RefShift { sx: f32, sy: f32 },
    /// The reference rule at (u, 1 − v).
    RefVFlip,
    /// The reference rule with the level of detail quantised to 1/q after the log2 (q < 0: truncated instead of rounded) and
    /// the anisotropy ratio quantised to 1/ratio_q (0 = exact) — the sampler's fixed-point arithmetic.
    RefQuantLod { q: f32, ratio_q: f32 },
    /// Taps N = the next power of two ≥ ratio, the level of detail from the minor axis (D3D's).
    Pow2LodMinor,
    /// The reference taps with the vendor's "optimised" trilinear: the two-level blend only when the LOD fraction lies within
    /// `band` of a level boundary (rescaled over that band), the nearer level alone otherwise.
    RefReducedTrilinear { band: f32 },
    /// N = pow2 ≥ ratio with the reduced trilinear.
    Pow2Reduced { band: f32 },
}

impl Rule {
    pub fn all() -> Vec<(String, Rule)> {
        vec![
            ("ref-aniso16 (N=ceil(ratio), (i+½)/N−½, lod=log2(major/ratio))".into(), Rule::RefAniso { max_aniso: 16 }),
            ("ref-aniso8".into(), Rule::RefAniso { max_aniso: 8 }),
            ("ref-aniso4".into(), Rule::RefAniso { max_aniso: 4 }),
            ("ref-aniso2".into(), Rule::RefAniso { max_aniso: 2 }),
            ("iso-trilinear lod=log2(major)".into(), Rule::IsoMajor),
            ("iso-trilinear lod=log2(minor)".into(), Rule::IsoMinor),
            ("aniso N=round(ratio)".into(), Rule::AnisoRound),
            ("aniso N=pow2≥ratio".into(), Rule::AnisoPow2),
            ("aniso endpoints i/(N−1)−½".into(), Rule::AnisoEndpoints),
            ("aniso short span (major−minor)".into(), Rule::AnisoShortSpan),
            ("aniso lod=log2(minor)".into(), Rule::AnisoLodMinor),
            ("aniso nearest level (no trilinear)".into(), Rule::AnisoNearestLevel),
            ("aniso lod=log2(mean(|dx|,|dy|))".into(), Rule::AnisoLodMean),
            ("aniso lod bias −0.25".into(), Rule::AnisoBias { bias: -0.25 }),
            ("aniso lod bias −0.5".into(), Rule::AnisoBias { bias: -0.5 }),
            ("aniso lod bias −0.75".into(), Rule::AnisoBias { bias: -0.75 }),
            ("aniso lod bias −1.0".into(), Rule::AnisoBias { bias: -1.0 }),
            ("aniso lod bias +0.5".into(), Rule::AnisoBias { bias: 0.5 }),
            ("aniso lod=log2(major/N) N=ceil".into(), Rule::AnisoLodOverN),
            ("aniso N=ceil(ratio)+1".into(), Rule::AnisoPlusOne),
            ("aniso N=2·ceil(ratio)".into(), Rule::AnisoDouble),
            ("aniso taps at i/N − ½ (no half offset)".into(), Rule::AnisoNoHalf),
            ("aniso taps at (i+½)/N − ½, span ×(1+1/N)".into(), Rule::AnisoWideSpan),
            ("ref shifted by (+½, +½) px".into(), Rule::RefShift { sx: 0.5, sy: 0.5 }),
            ("ref shifted by (−½, −½) px".into(), Rule::RefShift { sx: -0.5, sy: -0.5 }),
            ("ref shifted by (+½, −½) px".into(), Rule::RefShift { sx: 0.5, sy: -0.5 }),
            ("ref shifted by (−½, +½) px".into(), Rule::RefShift { sx: -0.5, sy: 0.5 }),
            ("ref shifted by (+½, 0) px".into(), Rule::RefShift { sx: 0.5, sy: 0.0 }),
            ("ref shifted by (0, +½) px".into(), Rule::RefShift { sx: 0.0, sy: 0.5 }),
            ("ref shifted by (−½, 0) px".into(), Rule::RefShift { sx: -0.5, sy: 0.0 }),
            ("ref shifted by (0, −½) px".into(), Rule::RefShift { sx: 0.0, sy: -0.5 }),
            ("ref with v mirrored (1 − v)".into(), Rule::RefVFlip),
            ("aniso N=pow2≥ratio, lod=log2(minor)".into(), Rule::Pow2LodMinor),
            ("ref, reduced trilinear (blend only within ¼ of a level boundary)".into(), Rule::RefReducedTrilinear { band: 0.25 }),
            ("ref, reduced trilinear (½)".into(), Rule::RefReducedTrilinear { band: 0.5 }),
            ("aniso N=pow2≥ratio, reduced trilinear ¼".into(), Rule::Pow2Reduced { band: 0.25 }),
            ("ref, lod quantised to 1/256".into(), Rule::RefQuantLod { q: 256.0, ratio_q: 0.0 }),
            ("ref, lod quantised to 1/64".into(), Rule::RefQuantLod { q: 64.0, ratio_q: 0.0 }),
            ("ref, lod quantised to 1/32".into(), Rule::RefQuantLod { q: 32.0, ratio_q: 0.0 }),
            ("ref, lod 1/256 + ratio 1/16".into(), Rule::RefQuantLod { q: 256.0, ratio_q: 16.0 }),
            ("ref, lod floor to 1/64 (truncated)".into(), Rule::RefQuantLod { q: -64.0, ratio_q: 0.0 }),
            ("aniso N=16 fixed, lod=log2(major/16)".into(), Rule::Fixed16 { lod_from_span: true }),
            ("aniso N=16 fixed, lod=log2(minor)".into(), Rule::Fixed16 { lod_from_span: false }),
            ("aniso N=ceil(ratio), lod=log2(major/ratio) − 1".into(), Rule::AnisoBias { bias: -1.0 }),
            ("aniso N=2·ceil(ratio), lod=log2(major/(2·ratio))".into(), Rule::DoubleFine),
            ("aniso N even".into(), Rule::AnisoEven),
            ("aniso 8-bit weights".into(), Rule::AnisoQuant),
            ("point level 0".into(), Rule::Point),
        ]
    }

    fn lens(fp: &Footprint) -> (f32, f32) {
        let lx = (fp.dx[0] * fp.dx[0] + fp.dx[1] * fp.dx[1]).sqrt();
        let ly = (fp.dy[0] * fp.dy[0] + fp.dy[1] * fp.dy[1]).sqrt();
        (lx.max(ly).max(1e-30), lx.min(ly).max(1e-30))
    }

    fn taps_along(tex: &AlphaTex, u: f32, v: f32, lod: f32, axis: [f32; 2], n: usize, positions: &dyn Fn(usize, usize) -> f32, nearest: bool, quant: bool) -> f32 {
        let n = n.max(1);
        let mut sum = 0.0f32;
        for i in 0..n {
            let s = positions(i, n);
            let a = if nearest { tex.sample_level(lod.round().clamp(0.0, (tex.levels.len() - 1) as f32) as usize, u + axis[0] * s, v + axis[1] * s, Address::ClampEdge) } else if quant { let last = (tex.levels.len() - 1) as f32; let l = lod.clamp(0.0, last); let l0 = l.floor(); let t = ((l - l0) * 256.0).round() / 256.0; let a0 = tex.sample_level(l0 as usize, u + axis[0] * s, v + axis[1] * s, Address::ClampEdge); if t <= 0.0 || l0 >= last { a0 } else { let a1 = tex.sample_level(l0 as usize + 1, u + axis[0] * s, v + axis[1] * s, Address::ClampEdge); a0 + (a1 - a0) * t } } else { tex.sample_lod(u + axis[0] * s, v + axis[1] * s, lod, Address::ClampEdge) };
            sum += if quant { (a * 255.0).round() / 255.0 } else { a };
        }
        sum / n as f32
    }

    pub fn alpha(&self, tex: &AlphaTex, u: f32, v: f32, fp: &Footprint) -> f32 {
        let (major, minor) = Self::lens(fp);
        let centred = |i: usize, n: usize| (i as f32 + 0.5) / n as f32 - 0.5;
        match *self {
            Rule::RefAniso { max_aniso } => {
                let ratio = (major / minor).min(max_aniso as f32).max(1.0);
                Self::taps_along(tex, u, v, (major / ratio).log2(), fp.major_axis(), ratio.ceil() as usize, &centred, false, false)
            }
            Rule::IsoMajor => tex.sample_lod(u, v, major.log2(), Address::ClampEdge),
            Rule::IsoMinor => tex.sample_lod(u, v, minor.log2(), Address::ClampEdge),
            Rule::AnisoRound => {
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u, v, (major / ratio).log2(), fp.major_axis(), ratio.round().max(1.0) as usize, &centred, false, false)
            }
            Rule::AnisoPow2 => {
                let ratio = (major / minor).min(16.0).max(1.0);
                let n = 2f32.powf(ratio.log2().ceil()).max(1.0) as usize;
                Self::taps_along(tex, u, v, (major / n as f32).log2(), fp.major_axis(), n, &centred, false, false)
            }
            Rule::AnisoEndpoints => {
                let ratio = (major / minor).min(16.0).max(1.0);
                let n = ratio.ceil() as usize;
                let pos = |i: usize, n: usize| if n <= 1 { 0.0 } else { i as f32 / (n - 1) as f32 - 0.5 };
                Self::taps_along(tex, u, v, (major / ratio).log2(), fp.major_axis(), n, &pos, false, false)
            }
            Rule::AnisoShortSpan => {
                let ratio = (major / minor).min(16.0).max(1.0);
                let n = ratio.ceil() as usize;
                let k = ((major - minor) / major).max(0.0);
                let ax = fp.major_axis();
                Self::taps_along(tex, u, v, (major / ratio).log2(), [ax[0] * k, ax[1] * k], n, &centred, false, false)
            }
            Rule::AnisoLodMinor => {
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u, v, minor.log2(), fp.major_axis(), ratio.ceil() as usize, &centred, false, false)
            }
            Rule::AnisoNearestLevel => {
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u, v, (major / ratio).log2(), fp.major_axis(), ratio.ceil() as usize, &centred, true, false)
            }
            Rule::Point => tex.sample_level(0, u, v, Address::ClampEdge),
            Rule::AnisoLodMean => {
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u, v, ((major + minor) * 0.5 / ratio).log2(), fp.major_axis(), ratio.ceil() as usize, &centred, false, false)
            }
            Rule::AnisoBias { bias } => {
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u, v, (major / ratio).log2() + bias, fp.major_axis(), ratio.ceil() as usize, &centred, false, false)
            }
            Rule::AnisoEven => {
                let ratio = (major / minor).min(16.0).max(1.0);
                let n = (2.0 * (ratio / 2.0).ceil()).max(1.0) as usize;
                Self::taps_along(tex, u, v, (major / ratio).log2(), fp.major_axis(), n, &centred, false, false)
            }
            Rule::AnisoQuant => {
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u, v, (major / ratio).log2(), fp.major_axis(), ratio.ceil() as usize, &centred, false, true)
            }
            Rule::AnisoLodOverN => {
                let ratio = (major / minor).min(16.0).max(1.0);
                let n = ratio.ceil().max(1.0);
                Self::taps_along(tex, u, v, (major / n).log2(), fp.major_axis(), n as usize, &centred, false, false)
            }
            Rule::AnisoPlusOne => {
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u, v, (major / ratio).log2(), fp.major_axis(), ratio.ceil() as usize + 1, &centred, false, false)
            }
            Rule::AnisoDouble => {
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u, v, (major / ratio).log2(), fp.major_axis(), 2 * ratio.ceil() as usize, &centred, false, false)
            }
            Rule::AnisoNoHalf => {
                let ratio = (major / minor).min(16.0).max(1.0);
                let pos = |i: usize, n: usize| i as f32 / n as f32 - 0.5;
                Self::taps_along(tex, u, v, (major / ratio).log2(), fp.major_axis(), ratio.ceil() as usize, &pos, false, false)
            }
            Rule::AnisoWideSpan => {
                let ratio = (major / minor).min(16.0).max(1.0);
                let n = ratio.ceil() as usize;
                let k = 1.0 + 1.0 / n as f32;
                let ax = fp.major_axis();
                Self::taps_along(tex, u, v, (major / ratio).log2(), [ax[0] * k, ax[1] * k], n, &centred, false, false)
            }
            Rule::Fixed16 { lod_from_span } => {
                let lod = if lod_from_span { (major / 16.0).max(minor.min(major)).log2() } else { minor.log2() };
                Self::taps_along(tex, u, v, lod, fp.major_axis(), 16, &centred, false, false)
            }
            Rule::DoubleFine => {
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u, v, (major / (2.0 * ratio)).log2(), fp.major_axis(), 2 * ratio.ceil() as usize, &centred, false, false)
            }
            Rule::RefShift { sx, sy } => {
                let (u2, v2) = (u + (sx * fp.dx[0] + sy * fp.dy[0]) / fp.w, v + (sx * fp.dx[1] + sy * fp.dy[1]) / fp.h);
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u2, v2, (major / ratio).log2(), fp.major_axis(), ratio.ceil() as usize, &centred, false, false)
            }
            Rule::RefVFlip => {
                let ratio = (major / minor).min(16.0).max(1.0);
                Self::taps_along(tex, u, 1.0 - v, (major / ratio).log2(), fp.major_axis(), ratio.ceil() as usize, &centred, false, false)
            }
            Rule::Pow2LodMinor => {
                let ratio = (major / minor).min(16.0).max(1.0);
                let n = 2f32.powf(ratio.log2().ceil()).max(1.0) as usize;
                Self::taps_along(tex, u, v, minor.log2(), fp.major_axis(), n, &centred, false, false)
            }
            Rule::RefReducedTrilinear { band } | Rule::Pow2Reduced { band } => {
                let ratio = (major / minor).min(16.0).max(1.0);
                let n = if matches!(self, Rule::Pow2Reduced { .. }) { 2f32.powf(ratio.log2().ceil()).max(1.0) as usize } else { ratio.ceil() as usize };
                let lod = (major / ratio).log2();
                // the reduced blend: fraction f → 0 for f < ½ − band/2, 1 for f > ½ + band/2, linear between (a narrowed transition)
                let last = (tex.levels.len() - 1) as f32;
                let l = lod.clamp(0.0, last);
                let l0 = l.floor();
                let f = l - l0;
                let t = ((f - (0.5 - band * 0.5)) / band).clamp(0.0, 1.0);
                let axis = fp.major_axis();
                let mut sum = 0.0f32;
                for i in 0..n.max(1) {
                    let sp = centred(i, n.max(1));
                    let (uu, vv) = (u + axis[0] * sp, v + axis[1] * sp);
                    let a0 = tex.sample_level(l0 as usize, uu, vv, Address::ClampEdge);
                    let a = if t <= 0.0 || l0 >= last { a0 } else if t >= 1.0 { tex.sample_level(l0 as usize + 1, uu, vv, Address::ClampEdge) } else { let a1 = tex.sample_level(l0 as usize + 1, uu, vv, Address::ClampEdge); a0 + (a1 - a0) * t };
                    sum += a;
                }
                sum / n.max(1) as f32
            }
            Rule::RefQuantLod { q, ratio_q } => {
                let mut ratio = (major / minor).min(16.0).max(1.0);
                if ratio_q > 0.0 { ratio = (ratio * ratio_q).round() / ratio_q; }
                let lod = (major / ratio).log2();
                let lod = if q > 0.0 { (lod * q).round() / q } else { (lod * -q).floor() / -q };
                Self::taps_along(tex, u, v, lod, fp.major_axis(), ratio.ceil() as usize, &centred, false, false)
            }
        }
    }
}

pub struct Labelled {
    pub frag: CardFrag,
    pub survived: bool,
}

/// Label the fragments against the captured layers (depth buffers in the same z01 convention, the clear value of the
/// last layer marks the fully observed pixels). Returns the labelled fragments of fully observed pixels and the counts
/// (fragments in all, at unobserved pixels, survived, discarded).
pub fn label(frags: &[CardFrag], layers: &[crate::passdiff::Buf], clear: f32, tol: f32) -> (Vec<Labelled>, [usize; 4]) {
    let mut out = Vec::new();
    let mut counts = [0usize; 4];
    let last = layers.last();
    for f in frags {
        counts[0] += 1;
        let observed = last.map(|l| { let i = (f.y * l.w + f.x) as usize; i < l.data.len() && l.data[i] == clear }).unwrap_or(false);
        if !observed {
            counts[1] += 1;
            continue;
        }
        let survived = layers.iter().any(|l| { let i = (f.y * l.w + f.x) as usize; i < l.data.len() && (l.data[i] - f.zq).abs() <= tol });
        if survived { counts[2] += 1; } else { counts[3] += 1; }
        out.push(Labelled { frag: *f, survived });
    }
    (out, counts)
}

/// `lmtool card-fit DUMP.bin PASSCAP [--tol 1e-6] [--sweep S] [--peel P] [--list-misses N]`
pub fn card_fit(a: Vec<String>) {
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1).cloned());
    let dump_path = std::path::PathBuf::from(&a[1]);
    let root = std::path::PathBuf::from(&a[2]);
    // the fragment's `zq` = z01 + the item draws' depth bias, quantised to the D16 target — the value the game's buffer holds;
    // the match tolerance is half a D16 step (the stored value differs from ours only through its own rounding)
    let tol: f32 = f("--tol").map(|v| v.parse().unwrap()).unwrap_or(1.5 / 65535.0);
    let dump = read_dump(&dump_path).unwrap_or_else(|e| panic!("{e}"));
    let name = dump_path.file_name().unwrap().to_string_lossy().to_string();
    // cardfrags-s{sweep}-d{di}-p{pi}.bin
    let parse_tag = |tag: char| -> Option<u32> { name.split(['-', '.']).find(|s| s.starts_with(tag) && s[1..].chars().all(|c| c.is_ascii_digit())).and_then(|s| s[1..].parse().ok()) };
    let sweep = f("--sweep").map(|v| v.parse().unwrap()).or_else(|| parse_tag('s')).unwrap_or(0);
    let peel = f("--peel").map(|v| v.parse().unwrap()).or_else(|| parse_tag('p')).unwrap_or(0);
    let masks: HashMap<u32, AlphaTex> = {
        let mut m = HashMap::new();
        let used: std::collections::BTreeSet<u32> = dump.frags.iter().map(|f| f.mask).collect();
        for k in used {
            let p = dump_path.parent().unwrap().join(format!("cardmask-{k}.bin"));
            match read_mask(&p) { Ok(t) => { m.insert(k, t); } Err(e) => eprintln!("{e}") }
        }
        m
    };
    println!("{name}: {} card fragments ({}×{} frame, direction ({:.4}, {:.4}, {:.4})), {} alpha masks ({})", dump.frags.len(), dump.res, dump.res_y, dump.d[0], dump.d[1], dump.d[2], masks.len(), masks.iter().map(|(k, t)| format!("#{k} {}×{} {} levels", t.w(), t.h(), t.levels.len())).collect::<Vec<_>>().join(", "));
    // the captured layers: the manifest's peel_depth entries of (sweep, the direction with our vector, peel)
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).unwrap_or_else(|e| panic!("MANIFEST.json: {e}"));
    let mut m = crate::passdiff::read_manifest(&txt).unwrap_or_else(|e| panic!("{e}"));
    crate::passdiff::reindex_directions(&mut m);
    let dir_of = |e: &crate::passdump::Entry| e.dir.map(|v| (0..3).all(|k| (v[k] - dump.d[k]).abs() < 2e-3)).unwrap_or(false);
    let dir_index: Option<u32> = m.passes.iter().find(|e| e.pass == "peel_depth" && e.sweep.unwrap_or(0) == sweep && dir_of(e)).and_then(|e| e.direction);
    let Some(di) = dir_index else { println!("no captured peel_depth entry of sweep {sweep} with the dump's direction"); return };
    let mut es: Vec<&crate::passdump::Entry> = m.passes.iter().filter(|e| e.pass == "peel_depth" && e.sweep.unwrap_or(0) == sweep && e.direction == Some(di) && e.peel == Some(peel) && e.width == dump.res).collect();
    es.sort_by_key(|e| (e.layer.unwrap_or(0), e.eid_last.unwrap_or(0)));
    // one entry per layer (the last snapshot of a layer index)
    let mut by_layer: std::collections::BTreeMap<u32, &crate::passdump::Entry> = std::collections::BTreeMap::new();
    for e in es { by_layer.insert(e.layer.unwrap_or(0), e); }
    if by_layer.is_empty() { println!("no captured peel_depth layers of sweep {sweep} direction {di} peel {peel} at {}²", dump.res); return }
    let mut layers: Vec<crate::passdiff::Buf> = Vec::new();
    let mut clear = 0.0f32;
    for (k, e) in &by_layer {
        match crate::passdiff::load_entry(&root, e) {
            Ok(b) => { clear = crate::passdiff::depth_clear(e, &b); if *k == 0 { println!("layer 0: {} ({}×{}); frustum half {:?}", e.file, b.w, b.h, e.frustum.as_ref().map(|f| f.half)); } layers.push(b); }
            Err(err) => println!("layer {k}: {err}"),
        }
    }
    // the clear value = the most common value of the last layer (the pixels no fragment reached)
    if let Some(last) = layers.last() { let mut h: HashMap<u32, usize> = HashMap::new(); for v in &last.data { *h.entry(v.to_bits()).or_default() += 1; } clear = f32::from_bits(*h.iter().max_by_key(|(_, n)| **n).unwrap().0); }
    println!("{} captured layers (indices {:?}), clear value of the last layer {clear}", layers.len(), by_layer.keys().collect::<Vec<_>>());
    // a look at the depth conventions: the first fragments' z01 against the captured layers' values at their pixel
    for fr in dump.frags.iter().take(6) {
        let vals: Vec<String> = layers.iter().map(|l| format!("{:.6}", l.data[(fr.y * l.w + fr.x) as usize])).collect();
        println!("  frag ({}, {}) z01 {:.6} zq {:.6} port_pass {} | captured layers {}", fr.x, fr.y, fr.z01, fr.zq, fr.port_pass, vals.join(" "));
    }
    // the depth offset between our z01 and the captured (the nearest layer value within 2e-3): a histogram in 1e-5 bins
    {
        let mut h: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
        for fr in &dump.frags {
            let mut best: Option<f32> = None;
            for l in &layers { let i = (fr.y * l.w + fr.x) as usize; if i < l.data.len() { let d = l.data[i] - fr.zq; if d.abs() < 2e-3 && best.map_or(true, |b: f32| d.abs() < b.abs()) { best = Some(d); } } }
            if let Some(d) = best { *h.entry((d * 65535.0).round() as i32).or_default() += 1; }
        }
        let top: Vec<String> = { let mut v: Vec<(i32, usize)> = h.iter().map(|(k, n)| (*k, *n)).collect(); v.sort_by(|a, b| b.1.cmp(&a.1)); v.iter().take(8).map(|(k, n)| format!("{k:+} steps: {n}")).collect() };
        println!("captured − ours (zq) depth offsets in D16 steps (nearest layer within 2e-3), the 8 most common: {}", top.join(", "));
    }
    let (lab, counts) = label(&dump.frags, &layers, clear, tol);
    println!("fragments {}: {} at pixels not fully observed (the layer cap), {} survived in the game, {} discarded (depth match tol {tol})", counts[0], counts[1], counts[2], counts[3]);
    // the port's own answer as recorded
    let (mut agree, mut fp_, mut fd) = (0usize, 0usize, 0usize);
    for l in &lab { let p = l.frag.port_pass != 0; if p == l.survived { agree += 1 } else if p { fp_ += 1 } else { fd += 1 } }
    println!("the bake's recorded answer: {agree} agree, {fp_} passed-but-discarded, {fd} discarded-but-survived of {}", lab.len());
    // the fragments whose footprint is missing (no texture) cannot be scored
    let scorable: Vec<&Labelled> = lab.iter().filter(|l| masks.contains_key(&l.frag.mask) && (l.frag.fp_dx != [0.0; 2] || l.frag.fp_dy != [0.0; 2])).collect();
    println!("scorable (a texture and a footprint): {}", scorable.len());
    // THE GROUPS: fragments sharing a pixel and a stored depth (coincident two-sided / duplicate card triangles) are one
    // depth sample in the game's buffer — kept iff ANY of them passed; a rule is scored per group (OR of its fragments)
    let mut groups: HashMap<(u32, u32, i32), Vec<usize>> = HashMap::new();
    for (i, l) in scorable.iter().enumerate() { groups.entry((l.frag.x, l.frag.y, (l.frag.zq * 65535.0).round() as i32)).or_default().push(i); }
    let multi = groups.values().filter(|v| v.len() > 1).count();
    println!("{} depth groups (pixel, D16 depth) of the {} scorable fragments; {multi} groups hold more than one fragment (coincident card triangles)", groups.len(), scorable.len());
    let list_misses: usize = f("--list-misses").map(|v| v.parse().unwrap()).unwrap_or(0);
    let mut results: Vec<(String, usize, usize, usize)> = Vec::new();
    // THE PEEL TEST (PS 17134 lines 4–7): sample_c GreaterEqual of the fragment's UNBIASED shadow-space depth against the
    // previous layer's STORED depth (biased by DepthBias 1 + SlopeScaled 1.0, D16) — a fragment closer than the previous
    // layer's bias behind it is peeled away with that layer, alpha or not. Per group: the game's previous layer = the largest
    // captured depth below the group's stored depth; predicted kept = alpha passes AND z01 ≥ that depth.
    let prev_layer = |x: u32, y: u32, zq: f32| -> f32 {
        let mut best = 0.0f32;
        for l in &layers { let d = l.data[(y * l.w + x) as usize]; if d < zq - tol && d > best { best = d; } }
        best
    };
    for (nm, rule) in Rule::all() {
        {
            let (mut g_agree, mut g_fp, mut g_fd, mut peeled) = (0usize, 0usize, 0usize, 0usize);
            for (&(x, y, zs), idx) in &groups {
                let survived = scorable[idx[0]].survived;
                let zq = zs as f32 / 65535.0;
                let prev = prev_layer(x, y, zq);
                let any_pass = idx.iter().any(|&i| { let l = scorable[i]; let tex = &masks[&l.frag.mask]; let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 }; rule.alpha(tex, l.frag.u, l.frag.v, &fp) - crate::peel::ALPHA_THRESHOLD >= 0.0 && l.frag.z01 >= prev });
                let alpha_only = idx.iter().any(|&i| { let l = scorable[i]; let tex = &masks[&l.frag.mask]; let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 }; rule.alpha(tex, l.frag.u, l.frag.v, &fp) - crate::peel::ALPHA_THRESHOLD >= 0.0 });
                if alpha_only && !any_pass { peeled += 1; }
                if any_pass == survived { g_agree += 1 } else if any_pass { g_fp += 1 } else { g_fd += 1 }
            }
            println!("{nm} [per depth group, WITH the peel test]: {g_agree} agree / {g_fp} pass-but-discarded / {g_fd} discard-but-kept  ({:.3} % agree); {peeled} alpha-passing groups peeled away by the previous layer's bias", 100.0 * g_agree as f64 / groups.len().max(1) as f64);
            if matches!(rule, Rule::RefAniso { max_aniso: 16 }) {
                // the remaining misses (with the peel test) by the group's best alpha margin and by texture / anisotropy
                let mut hp = [0usize; 6]; let mut hd = [0usize; 6];
                let bin = |d: f32| if d < 0.005 { 0 } else if d < 0.01 { 1 } else if d < 0.02 { 2 } else if d < 0.05 { 3 } else if d < 0.1 { 4 } else { 5 };
                let mut by_tex: HashMap<u32, (usize, usize)> = HashMap::new();
                let mut by_ratio = [(0usize, 0usize); 5];
                for (&(x, y, zs), idx) in &groups {
                    let survived = scorable[idx[0]].survived;
                    let zq = zs as f32 / 65535.0;
                    let prev = prev_layer(x, y, zq);
                    let mut best_a = f32::MIN; let mut any_pass = false; let mut ratio_max = 0f32;
                    for &i in idx { let l = scorable[i]; let tex = &masks[&l.frag.mask]; let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 }; let a = rule.alpha(tex, l.frag.u, l.frag.v, &fp); let (ma, mi) = Rule::lens(&fp); ratio_max = ratio_max.max(ma / mi); if l.frag.z01 >= prev { best_a = best_a.max(a); if a - crate::peel::ALPHA_THRESHOLD >= 0.0 { any_pass = true; } } }
                    let e = by_tex.entry(scorable[idx[0]].frag.mask).or_default(); e.0 += 1;
                    let rb = if ratio_max < 2.0 { 0 } else if ratio_max < 4.0 { 1 } else if ratio_max < 8.0 { 2 } else if ratio_max < 16.0 { 3 } else { 4 };
                    by_ratio[rb].0 += 1;
                    if any_pass != survived {
                        e.1 += 1; by_ratio[rb].1 += 1;
                        let d = if best_a == f32::MIN { 1.0 } else { (best_a - crate::peel::ALPHA_THRESHOLD).abs() };
                        if any_pass { hp[bin(d)] += 1 } else { hd[bin(d)] += 1 }
                    }
                }
                println!("    remaining misses by |best alpha − threshold| <0.005 / <0.01 / <0.02 / <0.05 / <0.1 / ≥0.1: pass-but-discarded {hp:?}, discard-but-kept {hd:?}");
                println!("    groups / misses by texture {:?}; by anisotropy ratio <2 / <4 / <8 / <16 / ≥16 {:?}", by_tex, by_ratio);
                // the far pass-but-discarded misses that survive the peel model: the pixel's game layers and our fragments
                let mut shown = 0;
                for (&(x, y, zs), idx) in &groups {
                    if shown >= 8 { break; }
                    let survived = scorable[idx[0]].survived;
                    if survived { continue; }
                    let zq = zs as f32 / 65535.0;
                    let prev = prev_layer(x, y, zq);
                    let mut best_a = f32::MIN;
                    for &i in idx { let l = scorable[i]; let tex = &masks[&l.frag.mask]; let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 }; let a = rule.alpha(tex, l.frag.u, l.frag.v, &fp); if l.frag.z01 >= prev { best_a = best_a.max(a); } }
                    if best_a < crate::peel::ALPHA_THRESHOLD + 0.05 { continue; }
                    let vals: Vec<String> = layers.iter().map(|l| { let d = l.data[(y * l.w + x) as usize]; if d >= 0.999 || d <= 0.0 { "—".into() } else { format!("{}", (d * 65535.0).round() as i32) } }).collect();
                    let mut ours: Vec<(i32, i32, u32, bool)> = dump.frags.iter().filter(|f| f.x == x && f.y == y).map(|f| ((f.zq * 65535.0).round() as i32, (f.z01 * 65535.0).round() as i32, f.tri, f.port_pass != 0)).collect();
                    ours.sort(); ours.dedup();
                    println!("    far miss after the peel model at ({x}, {y}): group zq {zs} best alpha {best_a:.3}, prev layer {}; game layers [{}]; ours (zq z01 tri α) {:?}", (prev * 65535.0).round() as i32, vals.join(" "), ours.iter().take(10).collect::<Vec<_>>());
                    if shown < 2 {
                        // the 9×9 neighbourhood: does the game hold this depth (±3 steps) somewhere nearby? per pixel the nearest layer offset in steps or '.'
                        for dy in -4i32..=4 {
                            let mut row = String::new();
                            for dx in -4i32..=4 {
                                let (xx, yy) = ((x as i32 + dx) as u32, (y as i32 + dy) as u32);
                                let mut best: Option<i32> = None;
                                for l in &layers { let d = l.data[(yy * l.w + xx) as usize]; if d > 0.0 && d < 0.999 { let o = ((d - zq) * 65535.0).round() as i32; if best.map_or(true, |b: i32| o.abs() < b.abs()) { best = Some(o); } } }
                                row += &match best { Some(o) if o.abs() <= 3 => format!("{:>6}", "HIT"), Some(o) => format!("{o:>+6}"), None => format!("{:>6}", ".") };
                            }
                            println!("        {row}");
                        }
                    }
                    shown += 1;
                }
                let mut shown = 0;
                for (&(x, y, zs), idx) in &groups {
                    if shown >= 8 { break; }
                    let survived = scorable[idx[0]].survived;
                    let zq = zs as f32 / 65535.0;
                    let prev = prev_layer(x, y, zq);
                    let alpha_only = idx.iter().any(|&i| { let l = scorable[i]; let tex = &masks[&l.frag.mask]; let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 }; rule.alpha(tex, l.frag.u, l.frag.v, &fp) - crate::peel::ALPHA_THRESHOLD >= 0.0 });
                    let z01 = scorable[idx[0]].frag.z01;
                    if !(survived && alpha_only && z01 < prev) { continue; }
                    let vals: Vec<String> = layers.iter().map(|l| { let d = l.data[(y * l.w + x) as usize]; if d >= 0.999 || d <= 0.0 { "—".into() } else { format!("{}", (d * 65535.0).round() as i32) } }).collect();
                    let ours: Vec<String> = dump.frags.iter().filter(|f| f.x == x && f.y == y).map(|f| format!("t{} z01 {} zq {}{}", f.tri, (f.z01 * 65535.0).round() as i32, (f.zq * 65535.0).round() as i32, if f.port_pass != 0 { "" } else { " (α✗)" })).take(8).collect();
                    println!("    kept-by-the-game but peeled in the model at ({x}, {y}): group zq {zs}, z01 {} (bias {} steps), prev layer {}; game layers [{}]; ours [{}]", (z01 * 65535.0).round() as i32, zs - (z01 * 65535.0).round() as i32, (prev * 65535.0).round() as i32, vals.join(" "), ours.join(", "));
                    shown += 1;
                }
            }
        }
        {
            let (mut g_agree, mut g_fp, mut g_fd) = (0usize, 0usize, 0usize);
            for (_, idx) in &groups {
                let survived = scorable[idx[0]].survived;
                let any_pass = idx.iter().any(|&i| { let l = scorable[i]; let tex = &masks[&l.frag.mask]; let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 }; rule.alpha(tex, l.frag.u, l.frag.v, &fp) - crate::peel::ALPHA_THRESHOLD >= 0.0 });
                if any_pass == survived { g_agree += 1 } else if any_pass { g_fp += 1 } else { g_fd += 1 }
            }
            println!("{nm} [per depth group]: {g_agree} agree / {g_fp} pass-but-discarded / {g_fd} discard-but-kept  ({:.3} % agree)", 100.0 * g_agree as f64 / groups.len().max(1) as f64);
            if matches!(rule, Rule::RefAniso { max_aniso: 16 }) {
                // where the pass-but-discarded groups sit: per triangle (misses / groups), the worst 12
                let mut per_tri: HashMap<u32, (usize, usize)> = HashMap::new();
                for (_, idx) in &groups {
                    let survived = scorable[idx[0]].survived;
                    let any_pass = idx.iter().any(|&i| { let l = scorable[i]; let tex = &masks[&l.frag.mask]; let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 }; rule.alpha(tex, l.frag.u, l.frag.v, &fp) - crate::peel::ALPHA_THRESHOLD >= 0.0 });
                    for &i in idx { let e = per_tri.entry(scorable[i].frag.tri).or_default(); e.0 += 1; if any_pass && !survived { e.1 += 1; } }
                }
                let mut v: Vec<(u32, usize, usize)> = per_tri.iter().map(|(t, (n, m))| (*t, *n, *m)).collect();
                v.sort_by(|a, b| b.2.cmp(&a.2));
                println!("    pass-but-discarded by triangle (tri: misses/fragments), worst 12: {}", v.iter().take(12).map(|(t, n, m)| format!("{t}: {m}/{n}")).collect::<Vec<_>>().join(", "));
                let whole: usize = v.iter().filter(|(_, n, m)| m == n && *n >= 4).count();
                let tris_with_miss = v.iter().filter(|(_, _, m)| *m > 0).count();
                println!("    triangles with a miss: {tris_with_miss} of {}; triangles ENTIRELY missed (≥ 4 fragments, all pass-but-discarded): {whole}", v.len());
                // the pass-but-discarded groups' pixel neighbourhood: does the game have a fragment of ANY depth within ±1 px at that depth?
                let mut near = 0usize; let mut total = 0usize;
                for (&(x, y, zs), idx) in &groups {
                    let survived = scorable[idx[0]].survived;
                    let any_pass = idx.iter().any(|&i| { let l = scorable[i]; let tex = &masks[&l.frag.mask]; let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 }; rule.alpha(tex, l.frag.u, l.frag.v, &fp) - crate::peel::ALPHA_THRESHOLD >= 0.0 });
                    if any_pass && !survived {
                        total += 1;
                        let z = zs as f32 / 65535.0;
                        let hit = layers.iter().any(|l| { let mut h = false; for dy in -1i32..=1 { for dx in -1i32..=1 { let (xx, yy) = (x as i32 + dx, y as i32 + dy); if xx >= 0 && yy >= 0 && (xx as u32) < l.w && (yy as u32) < l.h { let i = (yy as u32 * l.w + xx as u32) as usize; if (l.data[i] - z).abs() <= 3.0 / 65535.0 { h = true; } } } } h });
                        if hit { near += 1; }
                    }
                }
                println!("    of the {total} pass-but-discarded groups, {near} have that depth in the game's layers within ±1 pixel (an edge / coverage difference), {} do not", total - near);
                // the worst triangle's misses: our zq against the game's layer values at the pixel (in D16 steps), first 10
                if let Some(&(wt, _, _)) = v.first() {
                    let mut shown = 0;
                    for (&(x, y, zs), idx) in &groups {
                        if shown >= 10 { break; }
                        if !idx.iter().any(|&i| scorable[i].frag.tri == wt) { continue; }
                        let survived = scorable[idx[0]].survived;
                        let any_pass = idx.iter().any(|&i| { let l = scorable[i]; let tex = &masks[&l.frag.mask]; let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 }; rule.alpha(tex, l.frag.u, l.frag.v, &fp) - crate::peel::ALPHA_THRESHOLD >= 0.0 });
                        if !(any_pass && !survived) { continue; }
                        let fr = &scorable[idx[0]].frag;
                        let vals: Vec<String> = layers.iter().map(|l| { let d = l.data[(y * l.w + x) as usize]; if d >= 0.999 { "—".into() } else { format!("{:+}", ((d - zs as f32 / 65535.0) * 65535.0).round() as i32) } }).collect();
                        let others: Vec<String> = dump.frags.iter().filter(|f| f.x == x && f.y == y && f.tri != wt).map(|f| format!("tri {} zq{:+}", f.tri, ((f.zq - zs as f32 / 65535.0) * 65535.0).round() as i32)).take(6).collect();
                        println!("    tri {wt} miss at ({x}, {y}) z01 {:.5} zq step {zs} slope-bias {:+} steps, uv ({:.4}, {:.4}); game layers rel. to zq (steps): [{}]; our other fragments there: [{}]", fr.z01, ((fr.zq - fr.z01) * 65535.0).round() as i32, fr.u, fr.v, vals.join(" "), others.join(", "));
                        shown += 1;
                    }
                }
            }
        }
        let (mut agree, mut fp_, mut fd) = (0usize, 0usize, 0usize);
        let mut misses: Vec<String> = Vec::new();
        for l in &scorable {
            let tex = &masks[&l.frag.mask];
            let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 };
            let a = rule.alpha(tex, l.frag.u, l.frag.v, &fp);
            let pass = a - crate::peel::ALPHA_THRESHOLD >= 0.0;
            if pass == l.survived { agree += 1 } else {
                if pass { fp_ += 1 } else { fd += 1 }
                if misses.len() < list_misses { misses.push(format!("    ({}, {}) z01 {:.6} tri {} uv ({:.5}, {:.5}) fp dx {:?} dy {:?} alpha {:.4} → {} but the game {}", l.frag.x, l.frag.y, l.frag.z01, l.frag.tri, l.frag.u, l.frag.v, l.frag.fp_dx, l.frag.fp_dy, a, if pass { "pass" } else { "discard" }, if l.survived { "kept it" } else { "discarded it" })); }
            }
        }
        println!("{nm}: {agree} agree / {fp_} pass-but-discarded / {fd} discard-but-kept  ({:.3} % agree)", 100.0 * agree as f64 / scorable.len().max(1) as f64);
        if matches!(rule, Rule::RefAniso { max_aniso: 16 }) {
            // the misses by their distance from the threshold (|alpha − 128/255|): precision noise sits within 0.02, a rule error further out
            let mut hp = [0usize; 6]; let mut hd = [0usize; 6];
            let bin = |d: f32| if d < 0.005 { 0 } else if d < 0.01 { 1 } else if d < 0.02 { 2 } else if d < 0.05 { 3 } else if d < 0.1 { 4 } else { 5 };
            for l in &scorable {
                let tex = &masks[&l.frag.mask];
                let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 };
                let a = rule.alpha(tex, l.frag.u, l.frag.v, &fp);
                let pass = a - crate::peel::ALPHA_THRESHOLD >= 0.0;
                if pass != l.survived { let b = bin((a - crate::peel::ALPHA_THRESHOLD).abs()); if pass { hp[b] += 1 } else { hd[b] += 1 } }
            }
            println!("    misses by |alpha − threshold| bins <0.005 / <0.01 / <0.02 / <0.05 / <0.1 / ≥0.1: pass-but-discarded {hp:?}, discard-but-kept {hd:?}");
            // the texels under a few far misses (alpha ≥ threshold + 0.05, game discarded): the min / max alpha over the footprint's
            // window at the two levels around the lod — if every texel there is opaque no sampling rule discards, the uv itself differs
            let mut shown = 0;
            for l in &scorable {
                if shown >= 6 { break; }
                let tex = &masks[&l.frag.mask];
                let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 };
                let a = rule.alpha(tex, l.frag.u, l.frag.v, &fp);
                if l.survived || a < crate::peel::ALPHA_THRESHOLD + 0.05 { continue; }
                let (major, minor) = Rule::lens(&fp);
                let lod = (major / (major / minor).min(16.0).max(1.0)).log2().max(0.0);
                let mut desc = Vec::new();
                for lv in [lod.floor() as usize, (lod.floor() as usize + 1).min(tex.levels.len() - 1)] {
                    let lev = &tex.levels[lv];
                    let scale = (1u32 << lv) as f32;
                    let (cx, cy) = (l.frag.u * lev.w as f32, l.frag.v * lev.h as f32);
                    let r = (major / scale * 0.5 + 1.0).ceil() as i64;
                    let (x0, x1, y0, y1) = ((cx as i64 - r).max(0), (cx as i64 + r).min(lev.w as i64 - 1), (cy as i64 - r).max(0), (cy as i64 + r).min(lev.h as i64 - 1));
                    let (mn, mx) = lev.minmax(x0, y0, x1, y1);
                    let centre = lev.a[(cy as usize).min(lev.h - 1) * lev.w + (cx as usize).min(lev.w - 1)];
                    desc.push(format!("L{lv} ({}×{}) window x {x0}..{x1} y {y0}..{y1}: alpha min {mn} max {mx}, centre texel {centre}", lev.w, lev.h));
                }
                println!("    far miss ({}, {}) tri {} mask {} uv ({:.5}, {:.5}) major {major:.2} minor {minor:.2} lod {lod:.2} ours {a:.3}: {}", l.frag.x, l.frag.y, l.frag.tri, l.frag.mask, l.frag.u, l.frag.v, desc.join("; "));
                shown += 1;
            }
            // and by the footprint's anisotropy ratio (1–2, 2–4, 4–8, 8–16, >16) and by texture
            let mut hr = [(0usize, 0usize); 5];
            for l in &scorable {
                let tex = &masks[&l.frag.mask];
                let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 };
                let (major, minor) = Rule::lens(&fp);
                let r = major / minor;
                let b = if r < 2.0 { 0 } else if r < 4.0 { 1 } else if r < 8.0 { 2 } else if r < 16.0 { 3 } else { 4 };
                hr[b].0 += 1;
                let a = rule.alpha(tex, l.frag.u, l.frag.v, &fp);
                if (a - crate::peel::ALPHA_THRESHOLD >= 0.0) != l.survived { hr[b].1 += 1; }
            }
            println!("    fragments / misses by anisotropy ratio <2 / <4 / <8 / <16 / ≥16: {hr:?}");
            let mut ht: HashMap<u32, (usize, usize)> = HashMap::new();
            for l in &scorable { let tex = &masks[&l.frag.mask]; let fp = Footprint { dx: l.frag.fp_dx, dy: l.frag.fp_dy, w: tex.w() as f32, h: tex.h() as f32 }; let a = rule.alpha(tex, l.frag.u, l.frag.v, &fp); let e = ht.entry(l.frag.mask).or_default(); e.0 += 1; if (a - crate::peel::ALPHA_THRESHOLD >= 0.0) != l.survived { e.1 += 1; } }
            println!("    fragments / misses by texture: {:?}", ht);
        }
        for s in misses { println!("{s}"); }
        results.push((nm, agree, fp_, fd));
    }
    results.sort_by(|a, b| b.1.cmp(&a.1));
    println!("best: {} ({} of {})", results[0].0, results[0].1, scorable.len());
}

/// `lmtool card-mask-check cardmask-K.bin CAPTURED.dds[.gz]`: our alpha mip chain (the item zip's DDS, rows reversed) against
/// the GPU texture the capture dumped, level by level — is the game's chain the file's, or regenerated?
pub fn card_mask_check(a: Vec<String>) {
    let ours = read_mask(Path::new(&a[1])).unwrap_or_else(|e| panic!("{e}"));
    let bytes = crate::prepass::read_maybe_gz(Path::new(&a[2])).unwrap_or_else(|e| panic!("{e}"));
    let cap = AlphaTex::from_dds(&bytes, false).unwrap_or_else(|e| panic!("{e}"));
    println!("ours: {} levels ({}×{}), captured: {} levels ({}×{})", ours.levels.len(), ours.w(), ours.h(), cap.levels.len(), cap.w(), cap.h());
    for (k, (o, c)) in ours.levels.iter().zip(cap.levels.iter()).enumerate() {
        if o.w != c.w || o.h != c.h { println!("level {k}: size {}×{} vs {}×{}", o.w, o.h, c.w, c.h); continue; }
        let n = o.a.len();
        let same = o.a.iter().zip(c.a.iter()).filter(|(x, y)| x == y).count();
        let flipped_same = (0..o.h).map(|y| { let yy = o.h - 1 - y; (0..o.w).filter(|&x| o.a[y * o.w + x] == c.a[yy * o.w + x]).count() }).sum::<usize>();
        let mut hist = [0usize; 5];
        for (x, y) in o.a.iter().zip(c.a.iter()) { let d = (*x as i32 - *y as i32).unsigned_abs() as usize; hist[d.min(4)] += 1; }
        let (mo, mc) = (o.a.iter().map(|&v| v as f64).sum::<f64>() / n as f64, c.a.iter().map(|&v| v as f64).sum::<f64>() / n as f64);
        let (ge_o, ge_c) = (o.a.iter().filter(|&&v| v >= 128).count(), c.a.iter().filter(|&&v| v >= 128).count());
        println!("level {k} ({}×{}): {same} of {n} identical as stored ({} flipped); |Δ| histogram 0/1/2/3/≥4 {:?}; mean alpha ours {mo:.2} captured {mc:.2}; ≥128: ours {ge_o} captured {ge_c}", o.w, o.h, flipped_same, hist);
    }
}
