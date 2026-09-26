//! Per-collection, per-mood lighting parameters for `lmtool bake --mood auto`,
//! in the baker's K = 1 units (fb0 = 255 · max):
//!
//! ```text
//! E = ambient + up·(0.5 + 0.5·n.y) + sky·skyVis + sun·max(0, n·L)·sunVis
//! ```
//!
//! FITTED rows come from `lmtool moodfit` against an editor bake (q = 4) of a
//! tiny map of that collection and mood (2026-09-22): the sun direction by the
//! per-texel correlation / shadow agreement over the largest charts, the four
//! colour terms by per-channel least squares (`--regressor 1`). DERIVED rows
//! have no editor bake (the lightmapper crashes or saves nothing on the
//! WhiteShore / GreenCoast tiny copies): they take the nearest fitted row and
//! scale its terms by the ratio of the pack's `Mood.MoodSetting.xml` colours
//! (`LAmbient`, `LDirSun`); their sun direction is the fitted family's.
//!
//! The pack XML (Latitude, DayTime01) reproduces the Stadium Day elevation
//! with el = asin(cos φ · cos(360°·(t − ½))) (φ 45°, t 0.6 → 34.9°, fitted 35°)
//! but not BlueBay Sunrise64 (φ 20°, t 0.515 → 69°, fitted 45°): the 64×64
//! decorations' moods do not carry the XML's time. Directions stay fitted.

#[derive(Clone, Copy, Debug)]
pub struct MoodParams {
    pub collection: &'static str,
    pub mood: &'static str,
    /// "fitted" | "derived"
    pub confidence: &'static str,
    pub sun_az: f32,
    pub sun_el: f32,
    pub ambient: [f32; 3],
    pub up: [f32; 3],
    pub sky: [f32; 3],
    pub sun: [f32; 3],
    /// Point-light scale (frame 1), K = 1 units per unit intensity.
    pub light_k: f32,
    /// The decoration constant P (16384 on Stadium); the item base itself comes from `base_rule`.
    pub base: u32,
    /// The template chunk file name (collection-mood) in the template bank.
    pub template: &'static str,
}

const fn m(collection: &'static str, mood: &'static str, confidence: &'static str, sun_az: f32, sun_el: f32, ambient: [f32; 3], up: [f32; 3], sky: [f32; 3], sun: [f32; 3], base: u32, template: &'static str) -> MoodParams {
    MoodParams { collection, mood, confidence, sun_az, sun_el, ambient, up, sky, sun, light_k: 0.27, base, template }
}

pub const MOODS: &[MoodParams] = &[
    // BlueBay Sunrise64 — Tiny 16 editor bake (fix16): texel corr 0.28 at (80, 42.5), shadow agreement peak (75, 50); r² 0.22
    m("BlueBay", "Sunrise", "fitted", 77.5, 45.0, [0.232, 0.206, 0.211], [0.086, 0.097, 0.128], [0.114, 0.126, 0.169], [0.131, 0.087, 0.073], 4096, "BlueBay-Sunrise"),
    // BlueBay Day64 — Tiny 11 editor bake (fix-bluebay-20260922): a nearly shadowless bake (sun term ≈ 0.03); shadow-agreement peak (230, 35); r² 0.21
    m("BlueBay", "Day", "fitted", 230.0, 35.0, [0.306, 0.361, 0.368], [0.090, 0.116, 0.195], [0.048, 0.058, 0.112], [0.030, 0.034, 0.048], 4096, "BlueBay-Day"),
    // RedIsland Day — Tiny 02 editor bake (lightmap-wip-20260912): (95, 52.5), r² 0.30; the sky term fitted ≈ 0 (folded into ambient/up)
    m("RedIsland", "Day", "fitted", 95.0, 52.5, [0.275, 0.197, 0.198], [0.087, 0.119, 0.171], [0.010, 0.010, 0.012], [0.092, 0.069, 0.060], 4096, "RedIsland-Day"),
    // Stadium 48x48Screen155Day — Tiny 05 editor bake (lightmap-wip-20260912): (197.5, 35) — the XML model gives el 34.9°; r² 0.13
    m("Stadium", "Day", "fitted", 197.5, 35.0, [0.247, 0.271, 0.278], [0.189, 0.151, 0.174], [0.031, 0.043, 0.036], [0.045, 0.043, 0.042], 16384, "Stadium-Day"),
    // WhiteShore Day — no editor bake (the lightmapper crashes on the tiny copy); BlueBay Day terms, the XML colours are identical (SkyFactor 0.5)
    m("WhiteShore", "Day", "derived", 230.0, 35.0, [0.306, 0.361, 0.368], [0.090, 0.116, 0.195], [0.024, 0.029, 0.056], [0.030, 0.034, 0.048], 4096, "WhiteShore-Day"),
    // GreenCoast Day — no editor bake (nothing saved after the compute); BlueBay Day terms (identical XML)
    m("GreenCoast", "Day", "derived", 230.0, 35.0, [0.306, 0.361, 0.368], [0.090, 0.116, 0.195], [0.048, 0.058, 0.112], [0.030, 0.034, 0.048], 4096, "GreenCoast-Day"),
    // RedIsland Sunrise — no editor bake; BlueBay Sunrise terms × XML ratios (LDirSun 1.8/1.29/0.27 vs 1.9/1.39/0.40 → 0.95/0.93/0.68)
    m("RedIsland", "Sunrise", "derived", 77.5, 45.0, [0.232, 0.206, 0.211], [0.086, 0.097, 0.128], [0.114, 0.126, 0.169], [0.124, 0.081, 0.050], 4096, "RedIsland-Sunrise"),
    // WhiteShore Sunset — no editor bake; BlueBay Sunrise terms × XML ratios: LAmbient (0.396,0.361,0.515)/(0.407,0.458,0.546) = (0.97,0.79,0.94),
    // LDirSun (2.84,1.04,0.53)/(1.9,1.39,0.40) = (1.49,0.75,1.33); the sun mirrored to the west and lower (a sunset)
    m("WhiteShore", "Sunset", "derived", 282.5, 20.0, [0.225, 0.163, 0.198], [0.083, 0.077, 0.120], [0.111, 0.100, 0.159], [0.195, 0.065, 0.097], 4096, "WhiteShore-Sunset"),
];

/// The header's mood string, normalised: "Day64" / "48x48Screen155Day" → "Day".
pub fn normalise_mood(s: &str) -> &'static str {
    let l = s.to_ascii_lowercase();
    if l.contains("sunrise") {
        "Sunrise"
    } else if l.contains("sunset") {
        "Sunset"
    } else if l.contains("night") {
        "Night"
    } else {
        "Day"
    }
}

pub fn lookup(collection: &str, mood: &str) -> Option<&'static MoodParams> {
    let mood = normalise_mood(mood);
    MOODS.iter().find(|p| p.collection.eq_ignore_ascii_case(collection) && p.mood == mood)
}

pub fn table() -> String {
    let mut s = String::from("collection  mood     conf     az     el   ambient              up                   sky                  sun\n");
    for p in MOODS {
        s.push_str(&format!(
            "{:<11} {:<8} {:<8} {:>5.1} {:>5.1}  {:.3},{:.3},{:.3}  {:.3},{:.3},{:.3}  {:.3},{:.3},{:.3}  {:.3},{:.3},{:.3}\n",
            p.collection, p.mood, p.confidence, p.sun_az, p.sun_el, p.ambient[0], p.ambient[1], p.ambient[2], p.up[0], p.up[1], p.up[2], p.sky[0], p.sky[1], p.sky[2], p.sun[0], p.sun[1], p.sun[2]
        ));
    }
    s
}

/// The chart→object base (the object index of item 0) of a map — `--base auto`.
///
/// Objects are [decoration][authored blocks][the game's generated blocks][items];
/// the item base is
///
/// ```text
/// base = P + N_authored + (S_x·S_z − replaced) + G
/// ```
///
/// * P = 16384 on the Stadium decorations (48x48Screen155*: objects 0..3 are
///   the decoration's own charts; NoStadium48x48*: none), 0 on the terrain
///   collections;
/// * N_authored = the unbaked blocks, free and embedded custom blocks included
///   (the tiny 05's 46 water tiles count: 19120 = 16384 + 46 + 48² + 386);
/// * S_x·S_z ground tiles generated for the map grid — the baked records in the
///   file do NOT count (01 ×2 carries 1886 Sea records: base 128² exactly); on a
///   terrain collection an authored block replaces its column's tile (17 ×2,
///   04 ×2: one kept block → S² exactly), on Stadium it does not (25 ×2: 16384 +
///   1 + 96² + 7);
/// * G = the game's other generated pieces: pillars/walls under elevated Stadium
///   blocks (7 under 25 ×2's kept platform, 386 around the tiny 05's 46 water
///   tiles, 2108 around 05 ×2's 604 pool tiles) — map-specific. The game's block
///   list gives it: `/mapblocks2?list=baked` total = S² + G (`--baked-total N`),
///   or pass `--base-extra G`.
///
/// A Nadeo map's baked chunk IS the game's generated set, so there base = P +
/// unbaked + baked (25/25 sources). Measured by the giant child's `lmtool
/// itembase` on the giant editor bakes (2026-09-23) and `lmtool basecheck`.
pub struct BaseRule {
    pub deco_const: u32,
    pub authored: u32,
    pub custom_blocks: u32,
    pub ground_cols: u32,
    pub replaced: u32,
    pub extra: u32,
    pub stadium: bool,
}

impl BaseRule {
    pub fn base(&self) -> u32 {
        self.deco_const + self.authored + self.ground_cols - self.replaced + self.extra
    }
}

/// `authored`: (grid cell x, grid cell z, name) of every UNBAKED block. `baked_total`:
/// the game's own baked-block count (S² + G) when known — it replaces the S² − replaced + G estimate.
pub fn base_rule<'a>(envir: &str, decoration: &str, size: [i32; 3], authored: impl Iterator<Item = (i32, i32, &'a str)>, extra: u32, baked_total: Option<u32>) -> BaseRule {
    let d = decoration.to_ascii_lowercase();
    let stadium = envir.eq_ignore_ascii_case("stadium") || d.contains("stadium") || d.contains("48x48");
    let (sx, sz) = (size[0].max(1) as u32, size[2].max(1) as u32);
    let ground_cols = sx * sz;
    let mut covered = std::collections::HashSet::new();
    let (mut n_auth, mut n_custom) = (0u32, 0u32);
    for (cx, cz, name) in authored {
        if name.ends_with("_CustomBlock") {
            n_custom += 1;
        }
        n_auth += 1;
        if cx >= 0 && cz >= 0 && (cx as u32) < sx && (cz as u32) < sz {
            covered.insert((cx as u32, cz as u32));
        }
    }
    let replaced = if stadium { 0 } else { (covered.len() as u32).min(ground_cols) };
    match baked_total {
        Some(t) => BaseRule { deco_const: if stadium { 16384 } else { 0 }, authored: n_auth, custom_blocks: n_custom, ground_cols: t, replaced: 0, extra: 0, stadium },
        None => BaseRule { deco_const: if stadium { 16384 } else { 0 }, authored: n_auth, custom_blocks: n_custom, ground_cols, replaced, extra, stadium },
    }
}

/// The lighting mood NAME of a map by the QUARTER of its DayTime word (chunk 0x03043056) —
/// Night [0, ¼), Sunrise [¼, ½), Day [½, ¾), Sunset [¾, 1) — else (0xffffffff = default) the
/// decoration's own mood. This is the editor's day-time SLIDER rule (0x1407b1d50: the slider is a
/// key whose quarters are the four moods, stored as key_to_time(slider)); it names the right mood at
/// every mood's DEFAULT word (the 25 Summer sources: 0.306/0.3175 → Sunrise; 0.504/0.607 → Day;
/// 0.8075/0.854 → Sunset — Tiny 16's "Sunrise64" decoration bakes as a SUNSET at 0.854) but the
/// word is a TIME, and for a custom word the game's mood = `blend_weights` (a blend) with the name
/// of `nearest_mood_name` (the nearest DayTime01 to the blend key).
pub fn effective_mood(decoration_mood: &str, daytime: Option<u32>) -> &'static str {
    match daytime {
        Some(t) if t != 0xffff_ffff => {
            let f = t as f64 / 65536.0;
            if f < 0.25 {
                "Night"
            } else if f < 0.5 {
                "Sunrise"
            } else if f < 0.75 {
                "Day"
            } else {
                "Sunset"
            }
        }
        _ => normalise_mood(decoration_mood),
    }
}

/// The pack's `Mood.MoodSetting.xml` constants for a collection × mood (read 2026-09-22 from the
/// five paks; the `MoodXml` fields are the XML attributes).
#[derive(Clone, Copy, Debug)]
pub struct MoodXml {
    pub collection: &'static str,
    pub mood: &'static str,
    pub latitude: f32,
    pub daytime01: f32,
    pub l_ambient: [f32; 3],
    pub l_dir_sun: [f32; 3],
    pub l_dir_moon: [f32; 3],
    pub max_hdr: f32,
    pub bounce_factor: f32,
    pub sky_factor: f32,
}

const fn x(collection: &'static str, mood: &'static str, latitude: f32, daytime01: f32, l_ambient: [f32; 3], l_dir_sun: [f32; 3], l_dir_moon: [f32; 3], max_hdr: f32, bounce_factor: f32, sky_factor: f32) -> MoodXml {
    MoodXml { collection, mood, latitude, daytime01, l_ambient, l_dir_sun, l_dir_moon, max_hdr, bounce_factor, sky_factor }
}

pub const MOOD_XML: &[MoodXml] = &[
    x("BlueBay", "Day", 20.0, 0.644, [0.492269, 0.620016, 0.815248], [2.86093, 2.83547, 2.51654], [0.0; 3], 3.0, 2.0, 1.0),
    x("BlueBay", "Night", 20.0, 0.15, [0.00973407, 0.00973407, 0.0123014], [0.0; 3], [0.0566298, 0.0742476, 0.2], 1.7, 2.0, 3.0),
    x("BlueBay", "Sunrise", 20.0, 0.515, [0.407062, 0.458064, 0.546095], [1.9, 1.387, 0.399], [0.0; 3], 1.0, 1.6, 1.0),
    x("BlueBay", "Sunset", 20.0, 0.75, [0.395648, 0.361113, 0.515065], [2.84039, 0.994136, 0.482866], [0.0; 3], 3.0, 2.0, 1.0),
    x("GreenCoast", "Day", 48.0, 0.644, [0.492269, 0.620016, 0.815248], [2.86093, 2.83547, 2.51654], [0.0; 3], 3.0, 2.0, 1.0),
    x("GreenCoast", "Night", 48.0, 0.15, [0.00973407, 0.00973407, 0.0123014], [0.0; 3], [0.0566298, 0.0742476, 0.2], 1.7, 2.0, 5.0),
    x("GreenCoast", "Sunrise", 48.0, 0.515, [0.407062, 0.458064, 0.546095], [1.683, 1.207, 0.255], [0.0; 3], 1.8, 1.6, 0.6),
    x("GreenCoast", "Sunset", 48.0, 0.75, [0.395648, 0.361113, 0.515065], [2.84039, 0.994136, 0.482866], [0.0; 3], 3.0, 2.0, 0.8),
    x("RedIsland", "Day", 36.0, 0.644, [0.492269, 0.620016, 0.815248], [2.86093, 2.83547, 2.51654], [0.0; 3], 3.0, 2.0, 0.65),
    x("RedIsland", "Night", 36.0, 0.15, [0.00973407, 0.00973407, 0.0123014], [0.0; 3], [0.05663, 0.0742475, 0.2], 1.7, 3.0, 3.0),
    x("RedIsland", "Sunrise", 36.0, 0.515, [0.407062, 0.458064, 0.546095], [1.8, 1.29091, 0.272727], [0.0; 3], 1.0, 2.0, 1.0),
    x("RedIsland", "Sunset", 36.0, 0.75, [0.395648, 0.361113, 0.515065], [2.84039, 0.994136, 0.482866], [0.0; 3], 3.0, 2.0, 1.0),
    x("Stadium", "Day", 45.0, 0.6, [0.245533, 0.332724, 0.500045], [3.00027, 2.42108, 1.69429], [0.0; 3], 3.0, 2.0, 1.0),
    x("Stadium", "Night", 45.0, 0.15, [0.00406201, 0.00426893, 0.00539485], [0.0; 3], [0.0314011, 0.0456443, 0.100007], 3.5, 2.0, 1.0),
    x("Stadium", "Sunrise", 45.0, 0.52, [0.17888, 0.218505, 0.260498], [1.80022, 1.24912, 0.557112], [0.0; 3], 2.2, 2.0, 1.0),
    x("Stadium", "Sunset", 45.0, 0.73, [0.518425, 0.418344, 0.482193], [2.00018, 0.549404, 0.0818378], [0.0; 3], 2.7, 1.8, 1.0),
    x("WhiteShore", "Day", 60.0, 0.644, [0.492269, 0.620016, 0.815248], [2.86093, 2.83547, 2.51654], [0.0; 3], 3.0, 2.0, 0.5),
    x("WhiteShore", "Night", 60.0, 0.15, [0.00973407, 0.00973407, 0.0123014], [0.0; 3], [0.0251189, 0.0329335, 0.0887126], 1.7, 2.0, 3.0),
    x("WhiteShore", "Sunrise", 60.0, 0.515, [0.407062, 0.458064, 0.546095], [1.7, 1.309, 0.510925], [0.0; 3], 1.0, 1.6, 0.5),
    x("WhiteShore", "Sunset", 60.0, 0.75, [0.395648, 0.361113, 0.515065], [2.84039, 1.0403, 0.533484], [0.0; 3], 3.0, 2.0, 1.0),
];

pub fn mood_xml(collection: &str, mood: &str) -> Option<&'static MoodXml> {
    MOOD_XML.iter().find(|m| m.collection.eq_ignore_ascii_case(collection) && m.mood.eq_ignore_ascii_case(mood))
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────────
// THE MOOD BLENDER, transcribed (RE child 10, 2026-09-26 02:30Z NOTES block; decompiles decomp49-re10.tgz)
//
// 1. The map's DayTime word (chunk 0x03043056) is the TIME OF DAY: t = word · 2^-16 (RenderLighting_Frames
//    0x14021e340 l.312 `word · 1.5258789e-05` → zone+0x754). The moods' default words ARE key_to_time(DayTime01)
//    (FUN_14020d170: each mood listed as lroundf(key_to_time(mood.DayTime01)·65536) — BlueBay Sunrise 0.515 →
//    07:20:24 = 0x4e4b, Day 0.644 → 14:34 = 0x9b59, Sunset 0.75 → 20:30 = 0xdaab, Night 0.15 → 02:24 = 0x199a;
//    Stadium 0.52/0.6/0.73 → 0x5148/0x8111/0xceb8). The editor's day-time slider is a key whose quarters are
//    the four moods; it is STORED as key_to_time(slider) — the quarter rule of `effective_mood`.
// 2. time → blend key: FUN_140494690 over the collection's CPlugMoodBlender curve (`BlenderCurve::time_to_key`).
// 3. key → the two moods and the weight: CPlugMoodCurve (CPlugMoodBlender+0x48; SMOOTHSTEP on, ctor 0x1404baad0
//    +0x38 = 1): the XML `<MoodWeights>` keys (0.08/0.2/0.55/0.69, Weight 0 on all five decoration blenders)
//    MERGED with the moods sorted by DayTime01 (FUN_1404bab70; a mood's Weight = parity of its sorted index);
//    FUN_1404bae10: bracket the key (FUN_14018c4b0/14018c240/14018c3a0), w = smoothstep-lerp of the two bracket
//    weights (FUN_1404bace0), A = the first MOOD walking backward from the bracket's low key, B = the first mood
//    walking forward from its high key, t toward B = (B odd) ? w : 1 − w. Between two moods the curve runs
//    W_A → 0 at the XML key → W_B, so the blend toward B is PINNED at the XML key: BlueBay [0.55, 0.69] = pure
//    Day, [0.15, 0.2] ∪ [1.08, 1.15] = pure Night; EVERY MOOD DEFAULT WORD BAKES PURE (t ≤ 1.2e-7 — the record's
//    MaxHDR_Mood 2.9999998 / SkyFactor 1.0000002 on the 0xdaab saves are this 1e-7, not a 26 % blend); custom words
//    blend (np-tk3 0x5000 = 07:30 → Sunrise + 1.9 % Day: the record's 1.0378075 / 1.6075615 to the last bit).
// 4. The blended CPlugMoodSetting (CHmsMoodBlender::Update 0x14028d0b0): every field a + fl(fl(b − a)·t) — Latitude
//    (the MOODS' XML latitude; the blender XML's 47.5 only with CPlugMoodBlender+0x30 ≠ 0, which nothing sets),
//    LocalLightX, HelperHdrX, LAmbient (rgb + scale), LDirSun, LDirMoon, T3SpecularLocal, T3LightMap MaxHDR /
//    BounceFactor / SkyFactor (SkyUseClouds = the NEARER mood's), HdrScales.Player, the whole <Atmo> block (HdrSun
//    Power, Atmo1/Atmo2 Power/Scale/Color — colours LINEAR, so the lerp is in linear space), the FogMatter entries;
//    the <Fog> block by FUN_141418090 as fl(1 − t)·a + t·b (DepthMin/Max, Exponant, Intens, Height, Clouds,
//    SkyClouds.GlobalIntens = Sky_p's FogIntens, Color, WaterFog, Noise). DayTime01 = the key; EnableStars = near's.
//    The dome: GradientV = A's SkyColor, GradientV1 = B's, ScaleGrad0 = 1 − t, ScaleGrad1 = t (pwc-day: t = 1 →
//    (0, 1)); the lobes/fog/sun colour/ambient from the blended fields; the sun direction FUN_140494810(blended
//    Latitude, b) with b = the sun-arc parameter of `time_to_key`; the frame record's MaxHDR_Mood / BounceFactor /
//    SkyFactor / SkyUseClouds = the blended T3LightMap (FUN_14020d370 → FUN_14020d170 → compute params +0x1c..).
// ─────────────────────────────────────────────────────────────────────────────────────────────────────

/// The game's field lerp (0x14028d0b0): `a + fl(fl(b − a)·t)`.
#[inline]
pub fn lerp_field(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The <Fog> block's lerp (FUN_141418090): `fl(fl(1 − t)·a) + fl(t·b)`.
#[inline]
pub fn lerp_fog_field(a: f32, b: f32, t: f32) -> f32 {
    (1.0 - t) * a + t * b
}

/// FUN_1404942f0: a time of day → the DayTime word (lroundf(t·65536), 1..65535 else 0).
pub fn word_of_time(t: f32) -> u32 {
    let w = (t * 65536.0).round();
    if w >= 1.0 && w <= 65535.0 { w as u32 } else { 0 }
}

/// The time of day of a DayTime word (RenderLighting_Frames: `word · 2^-16`, exact in f32).
#[inline]
pub fn time_of_word(word: u32) -> f32 {
    word as f32 * f32::from_bits(0x3780_0000)
}

/// One key of the CPlugMoodCurve (stride 0xc): X, Weight, the mood's sorted index (−1 = an XML key).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoodCurveKey {
    pub x: f32,
    pub weight: f32,
    pub mood: i32,
}

/// The collection's moods sorted by `DayTime01` (CHmsMoodBlender build 0x14028aea0: CRT qsort, cmp 0x14028aaf0)
/// — the mood index k of the curve is the position in this list.
pub fn moods_sorted(collection: &str) -> Vec<&'static MoodXml> {
    let mut ms: Vec<&'static MoodXml> = MOOD_XML.iter().filter(|m| m.collection.eq_ignore_ascii_case(collection)).collect();
    ms.sort_by(|a, b| a.daytime01.partial_cmp(&b.daytime01).unwrap());
    ms
}

/// FUN_1404bab70: the XML weight keys (sorted, moodIndex −1) merged with the sorted moods' DayTime01 keys, a mood
/// entry = {DayTime01, Weight = (k & 1) as f32, k}; on a tie the mood goes BEFORE the XML key (`fVar2 <= *pfVar1`
/// inserts at the position).
pub fn merge_mood_curve(xml_keys: &[f32], mood_keys: &[f32]) -> Vec<MoodCurveKey> {
    let mut list: Vec<MoodCurveKey> = xml_keys.iter().map(|&x| MoodCurveKey { x, weight: 0.0, mood: -1 }).collect();
    let n = list.len();
    let (mut j, mut k) = (0usize, 0usize);
    while k < mood_keys.len() {
        if n + k <= j || mood_keys[k] <= list[j].x {
            list.insert(j, MoodCurveKey { x: mood_keys[k], weight: (k & 1) as f32, mood: k as i32 });
            k += 1;
        }
        j += 1;
    }
    list
}

/// FUN_14018c240 (flag 1, hint 0) + FUN_14018c3a0 + FUN_14018c4b0 (period 1): the bracket (i0, i1) of `key` in the
/// sorted `xs` and the fraction inside it. `key` is first wrapped into [0, 1) (fmod; +1 when negative); below the
/// first key it is raised by the period and, like a key past the last one, lands in the WRAP interval
/// (n−1 → 0) with frac = (key − last)/((first + 1) − last). Inside: the first consecutive pair with
/// xs[i] − 1e-5 ≤ key ≤ xs[i+1] + 1e-5 scanning from 0 (so an exact hit on a key is the END of the previous
/// interval, frac 1); key < xs[0] + 1e-5 → (0, 0, 0); key > xs[n−1] − 1e-5 → (n−1, n−1, 0).
pub fn curve_lookup(xs: &[f32], key: f32) -> (usize, usize, f32) {
    let n = xs.len();
    assert!(n >= 1, "empty curve");
    let period = 1.0f32;
    let mut key = key;
    if key < 0.0 || key >= period {
        key %= period;
        if key < 0.0 {
            key += period;
        }
    }
    let (first, last) = (xs[0], xs[n - 1]);
    let inside = if first <= key { key <= last } else { key += period; false };
    if !inside {
        return (n - 1, 0, (key - last) / ((first + period) - last));
    }
    if n == 1 {
        return (0, 0, 0.0);
    }
    let eps = 1e-5f32;
    if xs[0] + eps > key {
        return (0, 0, 0.0);
    }
    if key > xs[n - 1] - eps {
        return (n - 1, n - 1, 0.0);
    }
    let (mut i0, mut i1) = (0usize, 1usize);
    let mut cur = 0usize;
    for _ in 0..n - 1 {
        let (lo, hi) = if cur + 1 < n { (cur, cur + 1) } else { (0, 1) };
        i0 = lo;
        i1 = hi;
        if !(key < xs[lo] - eps) && !(xs[hi] + eps < key) {
            break;
        }
        cur = hi;
    }
    if i0 == i1 {
        return (i0, i1, 0.0);
    }
    let (k0, k1) = (xs[i0], xs[i1]);
    let d = k1 - k0;
    if d < 1e-5 && k0 - k1 < 1e-5 {
        return (i0, i1, 0.0);
    }
    (i0, i1, (key - k0) / d)
}

/// FUN_1404bace0: the curve's weight at `key` — the bracket weights blended by the SMOOTHSTEP of the fraction
/// (`s = fl(fl(f·3)·f) − fl(fl(fl(f·2)·f)·f)`, 0 / 1 outside (0, 1)): `fl(fl(1 − s)·W0) + fl(s·W1)`. Returns
/// (w, i0, i1).
pub fn curve_weight(curve: &[MoodCurveKey], key: f32) -> (f32, usize, usize) {
    let xs: Vec<f32> = curve.iter().map(|k| k.x).collect();
    let (i0, i1, f) = curve_lookup(&xs, key);
    let s = if f <= 0.0 { 0.0 } else if f < 1.0 { f * 3.0 * f - f * 2.0 * f * f } else { 1.0 };
    ((1.0 - s) * curve[i0].weight + s * curve[i1].weight, i0, i1)
}

/// FUN_1404bae10: the two moods (sorted indices) around `key` and the weight toward the second.
/// A = the first mood entry walking BACKWARD (cyclic) from i0; B = the first mood entry walking forward from
/// i1' (= (i0 + 1) % n when i0 == i1); t = (B.mood & 1 ≠ 0) ? w : 1 − w.
pub fn curve_moods(curve: &[MoodCurveKey], key: f32) -> (usize, usize, f32) {
    let n = curve.len();
    let (w, i0, i1) = curve_weight(curve, key);
    let i1p = if i0 == i1 { (i0 + 1) % n } else { i1 };
    let mut a = i0;
    while curve[a].mood == -1 {
        a = if a == 0 { n - 1 } else { a - 1 };
        if a == i0 {
            break;
        }
    }
    let mut b = i1p;
    if curve[b].mood == -1 {
        loop {
            b = if b + 1 >= n { 0 } else { b + 1 };
            if b == i1 || curve[b].mood != -1 {
                break;
            }
        }
    }
    let (am, bm) = (curve[a].mood.max(0) as usize, curve[b].mood.max(0) as usize);
    (am, bm, if bm & 1 != 0 { w } else { 1.0 - w })
}

/// The mood blend of a map: the two moods A/B (of `moods_sorted`) and the game's weight t toward B, with the
/// time, the blend key and the sun-arc parameter it came from.
#[derive(Clone, Copy, Debug)]
pub struct MoodBlend {
    pub a: &'static MoodXml,
    pub b: &'static MoodXml,
    pub t: f32,
    /// The time of day (the DayTime word / 65536).
    pub time: f32,
    /// The blend key (FUN_140494690 of the time) = the blended mood's DayTime01.
    pub key: f32,
    /// The sun-arc parameter b of FUN_140494810 (0 at SunRise, 1 at SunFall; 0/1 at night).
    pub sun_arc: f32,
}

impl MoodBlend {
    /// The nearer mood (t < 0.5 → A): its EnableStars / SkyUseClouds / name go into the blended setting.
    pub fn near(&self) -> &'static MoodXml {
        if self.t < 0.5 { self.a } else { self.b }
    }
    /// Sky_p's (ScaleGrad0, ScaleGrad1) of the dome: GradientV = A's SkyColor × (1 − t), GradientV1 = B's × t.
    pub fn scale_grad(&self) -> (f32, f32) {
        (1.0 - self.t, self.t)
    }
    /// Pure to the game's own precision: the default words come out at t ≤ 1.2e-7.
    pub fn is_pure(&self) -> bool {
        self.t <= 1e-6 || self.t >= 1.0 - 1e-6
    }
    /// The one mood of an EXACTLY pure blend (t == 0 → A, t == 1 → B); None when it blends. A key within 1e-5 above a
    /// mood key evaluates in the interval BEFORE that mood with frac ≈ 1 (the lookup's tolerance), so a pure mood can
    /// come out as (previous mood → it, t = 1) — same weights, same blended fields.
    pub fn pure(&self) -> Option<&'static MoodXml> {
        if self.t == 0.0 { Some(self.a) } else if self.t == 1.0 { Some(self.b) } else { None }
    }
}

/// `moods::blend_weights(word, collection)`: the blend the game applies to a map whose DayTime word is `word`
/// (0xffffffff = the decoration's default → the mood's default word).
pub fn blend_weights(word: u32, collection: &str) -> Option<MoodBlend> {
    let curve_x = BlenderCurve::for_collection(collection);
    let ms = moods_sorted(collection);
    if ms.is_empty() {
        return None;
    }
    let time = time_of_word(word);
    let (key, sun_arc) = curve_x.time_to_key(time);
    let mood_keys: Vec<f32> = ms.iter().map(|m| m.daytime01).collect();
    let curve = merge_mood_curve(curve_x.weight_keys, &mood_keys);
    let (ai, bi, t) = curve_moods(&curve, key);
    Some(MoodBlend { a: ms[ai], b: ms[bi], t, time, key, sun_arc })
}

/// The blend at a TIME of day (the DayTime word / 65536 — what `lmtool bake` passes): (A, B, t toward B).
pub fn blend(collection: &str, time01: f32) -> Option<(&'static MoodXml, &'static MoodXml, f32)> {
    blend_weights(word_of_time(time01), collection).map(|b| (b.a, b.b, b.t))
}

/// The blended mood constants at a time of day (the DayTime word / 65536): every field `a + fl(fl(b − a)·t)`
/// (Latitude too — the moods' own), `daytime01` = the blend key, the names = the nearer mood's. The frame record's
/// MaxHDR_Mood / BounceFactor / SkyFactor are `max_hdr` / `bounce_factor` / `sky_factor` here.
pub fn blended_xml(collection: &str, time01: f32) -> Option<MoodXml> {
    let bl = blend_weights(word_of_time(time01), collection)?;
    Some(blended_xml_of(&bl))
}

/// `blended_xml` from a `MoodBlend`.
pub fn blended_xml_of(bl: &MoodBlend) -> MoodXml {
    let (a, b, t) = (bl.a, bl.b, bl.t);
    let l3 = |p: [f32; 3], q: [f32; 3]| [lerp_field(p[0], q[0], t), lerp_field(p[1], q[1], t), lerp_field(p[2], q[2], t)];
    let near = bl.near();
    MoodXml {
        collection: near.collection,
        mood: near.mood,
        latitude: lerp_field(a.latitude, b.latitude, t),
        daytime01: bl.key,
        l_ambient: l3(a.l_ambient, b.l_ambient),
        l_dir_sun: l3(a.l_dir_sun, b.l_dir_sun),
        l_dir_moon: l3(a.l_dir_moon, b.l_dir_moon),
        max_hdr: lerp_field(a.max_hdr, b.max_hdr, t),
        bounce_factor: lerp_field(a.bounce_factor, b.bounce_factor, t),
        sky_factor: lerp_field(a.sky_factor, b.sky_factor, t),
    }
}

/// The mood NAME the game gives a map at a time: the sorted mood whose DayTime01 is nearest the blend key
/// (FUN_14028ccb0, first minimum wins).
pub fn nearest_mood_name(collection: &str, word: u32) -> Option<&'static str> {
    let bl = blend_weights(word, collection)?;
    let ms = moods_sorted(collection);
    let mut best: Option<(&MoodXml, f32)> = None;
    for m in ms {
        let d = (m.daytime01 - bl.key).abs();
        if best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((m, d));
        }
    }
    best.map(|(m, _)| m.mood)
}

/// The DayTime word a mood's default maps to: what Nadeo's editor baked the default-word sources
/// with (their frame records; 2026-09-23): Day 0x9b59 (Stadium 0x8111), Sunrise 0x4e4b (Stadium
/// 0x5148), Sunset 0xdaab (Stadium 0xceb8), Night 0x199a.
pub fn default_daytime(collection: &str, mood: &str) -> u32 {
    let stadium = collection.eq_ignore_ascii_case("stadium");
    match normalise_mood(mood) {
        "Night" => 0x199a,
        "Sunrise" => if stadium { 0x5148 } else { 0x4e4b },
        "Sunset" => if stadium { 0xceb8 } else { 0xdaab },
        _ => if stadium { 0x8111 } else { 0x9b59 },
    }
}

/// The rendered-sky fit per (collection, mood): (gradient scale = GlobalScale·ScaleGrad0 stand-in,
/// default bounce albedo for materials without a measured value, v flipped = the texture's top row is
/// the horizon (GradientV_InvertY) rather than the zenith). DIFFERENTIAL — fitted 2026-09-23 on the
/// q4 editor references so the all-texel mean matches (sun path, bounce read-back /BounceFactor and the
/// measured per-material albedo in force): BlueBay Day (tiny 11) and Sunset (tiny 16) 2.75, Sunrise /
/// Night from the 31-item test map; WhiteShore Day (tiny 03 reduced) 1.6/flipped; GreenCoast Day (tiny
/// 04 AC items) 2.5/flipped; Stadium Sunrise (giant 20 ×2 reduced) 1.0, Day (giant 10 ×2) 0.74, Sunset
/// (giant 05 ×2) 0.30. An unmeasured mood takes its collection's nearest.
pub fn sky_fit(_collection: &str, _mood: &str) -> (f32, f32, bool) {
    // ONE number for every collection and mood (2026-09-23 21:50Z): with the game's own sky constants
    // (GlobalScale 1, ScaleGrad0 1, FogIntens = SkyClouds GlobalIntens, the mood XML's Atmo lobes, the
    // texture's v = sin(elevation)) the sky term needs ×2 against my accumulation E = Σ 4/N·max(0,n·D)·L
    // — the BlueBay pad-only test map at Day comes out at −2.6 % (floors 0.97, walls 0.97) and at Sunset
    // at +9.6 % (floors 1.10, walls 0.91). DIFFERENTIAL: the factor is a convention I have not located
    // (the direction weights or the dome target's scale); no v-flip anywhere any more.
    (2.0, 0.3, false)
}

/// The sky gradient's global scale per mood — see `sky_fit`.
pub fn sky_grad_scale(collection: &str, mood: &str) -> f32 {
    sky_fit(collection, mood).0
}

/// Sky_p's FogIntens for the dome per mood, DIFFERENTIAL: the mood XML's depth formula does not give it
/// (Day 0.976 → 0.32 at 5300 m fits, but Sunset's parameters give 0.77 of a dark fog where 0.32 fits
/// again — the pad-only test maps at 0x9b59 and 0xdaab, 2026-09-23). None = the XML formula at the dome
/// distance.
pub fn fog_intens(_collection: &str, _mood: &str) -> Option<f32> {
    // pinned by RE child 4 (21:45Z): Sky_p's FogIntens = the mood XML's <SkyClouds GlobalIntens>, read by
    // skygrad::fog_from_xml — no per-mood fit any more
    None
}

/// THE BLENDER CURVE (CPlugMoodBlender+0x18: {Latitude, w = 0x3caaaaab (1/48, not in the XML), SunRise, LocalLight_SwitchOff,
/// LocalLight_SwitchOn, SunFall}; parser 0x1404bb2d0, "HH:MM:SS" → fl(seconds / 86400) by FUN_1404944a0; defaults FUN_140494530 =
/// 48, 1/48, 06:00, 06:30, 17:00, 18:00) and the `<MoodWeights>` keys. The five decoration blenders (RE 4, lightmap-re/moods/):
/// Latitude 47.5 (WhiteShore 55), SunRise 06:00, SwitchOff 06:30, SwitchOn 18:30 (GreenCoast 19:10), SunFall 21:00, keys
/// 0.08 / 0.2 / 0.55 / 0.69 with Weight 0.
///
/// The map's DayTime word is the TIME OF DAY (word / 65536) — RE 7's reading of it as a blend key (and `key_to_time` on it) was a
/// double conversion: 0x5148 = 07:37, 0x9b59 = 14:34, 0xdaab = 20:30, 0x4e4b = 07:20. `time_to_key` (FUN_140494690) turns the
/// time into the blend key; `key_to_time` (FUN_140494560) is its inverse and gives the moods' default words from their DayTime01.
pub struct BlenderCurve {
    /// The blender XML's Latitude — used by the game only when CPlugMoodBlender+0x30 ≠ 0 (nothing sets it): the sun runs on the
    /// MOODS' lerped Latitude.
    pub latitude: f32,
    pub w: f32,
    pub sun_rise: f32,
    pub switch_off: f32,
    pub switch_on: f32,
    pub sun_fall: f32,
    /// The `<MoodWeights><Key X Weight>` X values (all Weight 0 on the decoration blenders).
    pub weight_keys: &'static [f32],
}

/// fl(h:m:s / 86400) as the game parses a blender time.
pub const fn hms(h: u32, m: u32, s: u32) -> f32 {
    ((h * 3600 + m * 60 + s) as f32) / 86400.0
}

impl BlenderCurve {
    /// The decoration blenders' values (all five collections; GreenCoast's SwitchOn 19:10, WhiteShore's Latitude 55).
    pub fn for_collection(collection: &str) -> BlenderCurve {
        let green = collection.eq_ignore_ascii_case("GreenCoast");
        let white = collection.eq_ignore_ascii_case("WhiteShore");
        BlenderCurve {
            latitude: if white { 55.0 } else { 47.5 },
            w: f32::from_bits(0x3caa_aaab),
            sun_rise: hms(6, 0, 0),
            switch_off: hms(6, 30, 0),
            switch_on: if green { hms(19, 10, 0) } else { hms(18, 30, 0) },
            sun_fall: hms(21, 0, 0),
            weight_keys: &[0.08, 0.2, 0.55, 0.69],
        }
    }

    /// FUN_140494690(&key, curve, time, &b): the time of day → (blend key, sun-arc parameter b), in the game's f32 op order.
    /// Day (t_r ≤ t ≤ t_s): b = clamp((t − t_r)/(t_s − t_r)); t < t_r + w → key = 0.25 + ((t − t_r)·0.25)/w; t < t_s − w →
    /// key = 0.5 + (((t − t_r) − w)·0.25)/((t_s − t_r) − 2w); else key = 0.75 + (((t − t_s) + w)·0.25)/w. Night: mid =
    /// (t_s + t_r)·0.5, span = (t_r + 1) − t_s; t > mid → b = 1, key = ((t − t_s)·0.25)/span; else b = 0, key =
    /// (((1 − t_s) + t)·0.25)/span.
    pub fn time_to_key(&self, t: f32) -> (f32, f32) {
        let (t_r, t_s, w) = (self.sun_rise, self.sun_fall, self.w);
        if t >= t_r && t <= t_s {
            let mut b = (t - t_r) / (t_s - t_r);
            b = if b <= 0.0 { 0.0 } else if b < 1.0 { b } else { 1.0 };
            let key = if t_r + w > t {
                ((t - t_r) * 0.25) / w + 0.25
            } else if t_s - w > t {
                (((t - t_r) - w) * 0.25) / ((t_s - t_r) - (w + w)) + 0.5
            } else {
                (((t - t_s) + w) * 0.25) / w + 0.75
            };
            (key, b)
        } else {
            let mid = (t_s + t_r) * 0.5;
            let span = (t_r + 1.0) - t_s;
            if t > mid { (((t - t_s) * 0.25) / span, 1.0) } else { ((((1.0 - t_s) + t) * 0.25) / span, 0.0) }
        }
    }

    /// FUN_140494560: the blend key back to the time of day (wraps the key into [−0.001, 1.001]; key ≤ 0.25 → the night
    /// formula t = span·(4·key) + t_s, −1 when > 1; ≤ 0.5 → (key − 0.25)·4·w + t_r; ≤ 0.75 → (key − 0.5)·4·((t_s − t_r) − 2w)
    /// + (t_r + w); else (key − 0.75)·4·w + (t_s − w)).
    pub fn key_to_time(&self, u: f32) -> f32 {
        let (t_r, t_s, w) = (self.sun_rise, self.sun_fall, self.w);
        let mut u = u;
        while u < -0.001 {
            u += 1.0;
        }
        while u > 1.001 {
            u -= 1.0;
        }
        if u <= 0.25 {
            let mut t = ((t_r + 1.0) - t_s) * (u * 4.0) + t_s;
            if t > 1.0 {
                t -= 1.0;
            }
            t
        } else if u <= 0.5 {
            (u - 0.25) * 4.0 * w + t_r
        } else if u <= 0.75 {
            (u - 0.5) * 4.0 * ((t_s - t_r) - (w + w)) + (t_r + w)
        } else {
            (u - 0.75) * 4.0 * w + (t_s - w)
        }
    }

    /// The mood blender's local-light flag (FUN_1402697e0 → vc+8, FUN_14020d170's per-mood table, 0x14028aea0 l.685): ON iff
    /// time < LocalLight_SwitchOff || time > LocalLight_SwitchOn, with time = the DayTime word / 65536. 0x4e4b 07:20 → off,
    /// 0x5148 07:37 → off, 0x9b59 14:34 → off, 0xdaab 20:30 → on, 0x199a 02:24 → on. The lightmapper bakes the local-light
    /// frame (frame 1) whenever the map has lamps regardless of this flag (stpad at 07:37 is lit): the switch is the RUNTIME
    /// toggle of the colourless per-texel light list (RE 7); the frame record's LocalLight_Switch stays 2 (Unknown).
    pub fn local_lights_on(&self, daytime_word: u32) -> bool {
        let t = time_of_word(daytime_word);
        t < self.switch_off || t > self.switch_on
    }

    /// The default DayTime word of a mood: lroundf(key_to_time(DayTime01)·65536) (FUN_14020d170's table entry).
    pub fn default_word(&self, daytime01: f32) -> u32 {
        word_of_time(self.key_to_time(daytime01))
    }
}

#[cfg(test)]
mod mood_blender_tests {
    use super::*;

    fn bits(x: f32) -> u32 {
        x.to_bits()
    }

    #[test]
    fn the_moods_default_words_are_key_to_time_of_their_daytime01() {
        // the words the editor writes for the decorations' default moods (frame records of the Summer sources, 2026-09-23)
        for (coll, mood, word) in [("BlueBay", "Sunrise", 0x4e4b), ("BlueBay", "Day", 0x9b59), ("BlueBay", "Sunset", 0xdaab), ("BlueBay", "Night", 0x199a), ("Stadium", "Sunrise", 0x5148), ("Stadium", "Day", 0x8111), ("Stadium", "Sunset", 0xceb8), ("Stadium", "Night", 0x199a)] {
            let c = BlenderCurve::for_collection(coll);
            let x = mood_xml(coll, mood).unwrap();
            assert_eq!(c.default_word(x.daytime01), word, "{coll} {mood}");
            assert_eq!(default_daytime(coll, mood), word, "{coll} {mood} table");
            // and the round trip time → key lands on the mood's key to the word's 1/65536
            let (key, _) = c.time_to_key(time_of_word(word));
            assert!((key - x.daytime01).abs() < 1.5e-4, "{coll} {mood}: key {key} vs {}", x.daytime01);
        }
    }

    #[test]
    fn the_bluebay_curve_is_the_xml_keys_merged_with_the_parity_weighted_moods() {
        let c = BlenderCurve::for_collection("BlueBay");
        let ms = moods_sorted("BlueBay");
        assert_eq!(ms.iter().map(|m| m.mood).collect::<Vec<_>>(), ["Night", "Sunrise", "Day", "Sunset"]);
        let curve = merge_mood_curve(c.weight_keys, &ms.iter().map(|m| m.daytime01).collect::<Vec<_>>());
        let want = [(0.08, 0.0, -1), (0.15, 0.0, 0), (0.2, 0.0, -1), (0.515, 1.0, 1), (0.55, 0.0, -1), (0.644, 0.0, 2), (0.69, 0.0, -1), (0.75, 1.0, 3)];
        assert_eq!(curve.len(), want.len());
        for (k, (x, w, m)) in curve.iter().zip(want) {
            assert_eq!((k.x, k.weight, k.mood), (x, w, m));
        }
    }

    #[test]
    fn pwc_day_0x9b59_is_a_pure_day_dome() {
        // the captured DOME PS 16774 cbuffer: ScaleGrad0 = 0, ScaleGrad1 = 1 → t = 1 toward Day
        let b = blend_weights(0x9b59, "BlueBay").unwrap();
        assert_eq!((b.a.mood, b.b.mood), ("Sunrise", "Day"));
        assert_eq!(b.t, 1.0);
        assert_eq!(b.pure().map(|m| m.mood), Some("Day"));
        assert_eq!(b.scale_grad(), (0.0, 1.0));
        assert!((b.time * 24.0 - 14.566).abs() < 0.01, "{}", b.time * 24.0);
        let x = blended_xml_of(&b);
        assert_eq!((x.max_hdr, x.bounce_factor, x.sky_factor), (3.0, 2.0, 1.0));
        assert_eq!(x.mood, "Day");
        assert_eq!(nearest_mood_name("BlueBay", 0x9b59), Some("Day"));
    }

    #[test]
    fn hill4_0xdaab_is_a_pure_sunset_to_the_records_last_bit() {
        // hill4 / np-tk3 0xdaab editor records: MaxHDR_Mood 2.9999998, SkyFactor 1.0000002, BounceFactor 2 — the 1e-7
        // toward Night of the word's 1/65536 rounding, not a 26 % blend
        let b = blend_weights(0xdaab, "BlueBay").unwrap();
        assert_eq!((b.a.mood, b.b.mood), ("Sunset", "Night"));
        assert!(b.t > 0.0 && b.t < 2e-7, "{}", b.t);
        assert!(b.is_pure());
        let x = blended_xml_of(&b);
        assert_eq!(bits(x.max_hdr), bits(2.9999998_f32), "{}", x.max_hdr);
        assert_eq!(bits(x.sky_factor), bits(1.0000002_f32), "{}", x.sky_factor);
        assert_eq!(x.bounce_factor, 2.0);
        assert_eq!(x.mood, "Sunset");
    }

    #[test]
    fn np_tk3_0x5000_is_sunrise_plus_1_9_percent_day() {
        // np-tk3-BlueBay-0x5000-q3-editor record: MaxHDR_Mood 1.0378075, BounceFactor 1.6075615, SkyFactor 1 (Sunrise 1/1.6/1,
        // Day 3/2/1): a LINEAR weight would give t = 0.0816 (1.163 / 1.633)
        let b = blend_weights(0x5000, "BlueBay").unwrap();
        assert_eq!((b.a.mood, b.b.mood), ("Sunrise", "Day"));
        assert!((b.time * 24.0 - 7.5).abs() < 1e-4);
        assert!((b.key - 0.5178571).abs() < 1e-6, "{}", b.key);
        assert!((b.t - 0.0189037).abs() < 1e-6, "{}", b.t);
        let x = blended_xml_of(&b);
        assert!((x.max_hdr - 1.0378075).abs() <= 2.4e-7, "{}", x.max_hdr);
        assert!((x.bounce_factor - 1.6075615).abs() <= 2.4e-7, "{}", x.bounce_factor);
        assert_eq!(x.sky_factor, 1.0);
        assert_eq!(x.mood, "Sunrise");
    }

    #[test]
    fn np_tk3_0xc000_is_day_toward_sunset() {
        // 18:00 → key 0.7053571 in [0.69 XML, Sunset 0.75]: smoothstep(0.256) = 0.163 toward Sunset (both moods (3, 2, 1))
        let b = blend_weights(0xc000, "BlueBay").unwrap();
        assert_eq!((b.a.mood, b.b.mood), ("Day", "Sunset"));
        assert!((b.key - 0.7053571).abs() < 1e-6, "{}", b.key);
        assert!((b.t - 0.162999).abs() < 1e-5, "{}", b.t);
        let x = blended_xml_of(&b);
        assert_eq!((x.max_hdr, x.bounce_factor, x.sky_factor), (3.0, 2.0, 1.0));
    }

    #[test]
    fn stpad_0x5148_is_a_pure_stadium_sunrise() {
        // stpad-Stadium-Sunrise-q3-editor record: MaxHDR_Mood 2.2 exactly
        // the key 0.5200021 sits 2e-6 above Sunrise's 0.52: the lookup's 1e-5 tolerance puts it at the END of the
        // [0.2, 0.52] interval (frac 1.0000066 → s = 1 → w = Sunrise's weight 1 → t = 1 toward Sunrise)
        let b = blend_weights(0x5148, "Stadium").unwrap();
        assert_eq!((b.a.mood, b.b.mood, b.t), ("Night", "Sunrise", 1.0));
        assert_eq!(b.pure().map(|m| m.mood), Some("Sunrise"));
        assert!((b.key - 0.5200021).abs() < 1e-6, "{}", b.key);
        let x = blended_xml_of(&b);
        assert_eq!(x.max_hdr, 2.2);
        assert_eq!(x.mood, "Sunrise");
    }

    #[test]
    fn the_blend_is_pinned_between_the_xml_keys() {
        // BlueBay: keys in [0.55, 0.69] are pure Day; [0.15, 0.2] pure Night
        let c = BlenderCurve::for_collection("BlueBay");
        let ms = moods_sorted("BlueBay");
        let curve = merge_mood_curve(c.weight_keys, &ms.iter().map(|m| m.daytime01).collect::<Vec<_>>());
        for key in [0.56, 0.6, 0.644, 0.66, 0.689] {
            let (a, b, t) = curve_moods(&curve, key);
            let pure_day = (ms[a].mood == "Day" && t == 0.0) || (ms[b].mood == "Day" && t == 1.0);
            assert!(pure_day, "key {key}: {} → {} t {t}", ms[a].mood, ms[b].mood);
        }
        for key in [0.16, 0.19] {
            let (a, b, t) = curve_moods(&curve, key);
            let pure_night = (ms[a].mood == "Night" && t == 0.0) || (ms[b].mood == "Night" && t == 1.0);
            assert!(pure_night, "key {key}: {} → {} t {t}", ms[a].mood, ms[b].mood);
        }
        // and a mid Sunrise→Day key blends by the smoothstep of the fraction to the XML key 0.55
        let (a, b, t) = curve_moods(&curve, 0.5325);
        assert_eq!((ms[a].mood, ms[b].mood), ("Sunrise", "Day"));
        assert!((t - 0.5).abs() < 1e-5, "{t}");
    }

    #[test]
    fn the_local_light_flag_of_the_saves() {
        let c = BlenderCurve::for_collection("Stadium");
        assert!(!c.local_lights_on(0x4e4b), "07:20 → off");
        assert!(!c.local_lights_on(0x5148), "07:37 → off (stpad's frame 1 is lit regardless: the flag is the runtime toggle)");
        assert!(!c.local_lights_on(0x9b59), "14:34 → off");
        assert!(c.local_lights_on(0xdaab), "20:30 → on");
        assert!(c.local_lights_on(0x199a), "02:24 → on");
        let g = BlenderCurve::for_collection("GreenCoast");
        assert!(!g.local_lights_on(word_of_time(hms(19, 0, 0))) && g.local_lights_on(word_of_time(hms(19, 20, 0))));
    }

    #[test]
    fn the_wrap_interval_and_the_exact_hits_of_the_curve_lookup() {
        let xs = [0.08, 0.15, 0.2, 0.515, 0.55, 0.644, 0.69, 0.75];
        // past the last key → the wrap interval toward the first key + 1
        let (i0, i1, f) = curve_lookup(&xs, 0.9);
        assert_eq!((i0, i1), (7, 0));
        assert!((f - (0.9 - 0.75) / 0.33).abs() < 1e-6);
        // below the first key: raised by the period, same interval
        let (i0, i1, f) = curve_lookup(&xs, 0.05);
        assert_eq!((i0, i1), (7, 0));
        assert!((f - (1.05 - 0.75) / 0.33).abs() < 1e-6);
        // an exact hit is the end of the previous interval
        let (i0, i1, f) = curve_lookup(&xs, 0.55);
        assert_eq!((i0, i1), (3, 4));
        assert!((f - 1.0).abs() < 1e-6);
        // within 1e-5 of the first key → (0, 0)
        assert_eq!(curve_lookup(&xs, 0.080005), (0, 0, 0.0));
    }
}
