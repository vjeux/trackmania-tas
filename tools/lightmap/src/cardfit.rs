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
            ("aniso lod bias −0.5".into(), Rule::AnisoBias { bias: -0.5 }),
            ("aniso lod bias +0.5".into(), Rule::AnisoBias { bias: 0.5 }),
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
        let survived = layers.iter().any(|l| { let i = (f.y * l.w + f.x) as usize; i < l.data.len() && (l.data[i] - f.z01).abs() <= tol });
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
    // our z01 and the captured differ by a systematic ~1e-4 (the projection's rounding); distinct fronds sit ≥ 5e-3 apart
    let tol: f32 = f("--tol").map(|v| v.parse().unwrap()).unwrap_or(4e-4);
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
        println!("  frag ({}, {}) z01 {:.6} port_pass {} | captured layers {}", fr.x, fr.y, fr.z01, fr.port_pass, vals.join(" "));
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
    let list_misses: usize = f("--list-misses").map(|v| v.parse().unwrap()).unwrap_or(0);
    let mut results: Vec<(String, usize, usize, usize)> = Vec::new();
    for (nm, rule) in Rule::all() {
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
        for s in misses { println!("{s}"); }
        results.push((nm, agree, fp_, fd));
    }
    results.sort_by(|a, b| b.1.cmp(&a.1));
    println!("best: {} ({} of {})", results[0].0, results[0].1, scorable.len());
}
