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

/// The lighting mood the game uses for a map: its DayTime word (chunk 0x03043056)
/// selects the QUARTER of the day — Night [0, ¼), Sunrise [¼, ½), Day [½, ¾),
/// Sunset [¾, 1) — else (0xffffffff = default) the decoration's own mood.
/// Measured on the 25 Summer sources: every frame record's MaxHDR/BounceFactor/
/// SkyFactor triple is the XML triple of THAT mood (0.10 → Night 1.7/2/5 on
/// GreenCoast 09; 0.306/0.3175 → Sunrise; 0.504/0.607 → Day; 0.8075/0.854 →
/// Sunset — Tiny 16's "Sunrise64" decoration bakes as a SUNSET at 0.854).
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

/// The mood BLEND at a blend key: the two moods whose `DayTime01` keys bracket it (cyclic — BlueBay:
/// Night 0.15 → Sunrise 0.515 → Day 0.644 → Sunset 0.75 → Night 1.15) and the fraction toward the
/// second. The game's CPlugMoodBlender lerps every mood field between them (RE child 3, 0x14028d0b0);
/// the moods' `DayTime01` are blend keys (the map's DayTime word/65536 is one too).
pub fn blend(collection: &str, key: f32) -> Option<(&'static MoodXml, &'static MoodXml, f32)> {
    let mut ms: Vec<&'static MoodXml> = MOOD_XML.iter().filter(|m| m.collection.eq_ignore_ascii_case(collection)).collect();
    if ms.is_empty() {
        return None;
    }
    ms.sort_by(|a, b| a.daytime01.partial_cmp(&b.daytime01).unwrap());
    let k = key.rem_euclid(1.0);
    let n = ms.len();
    for i in 0..n {
        let (a, b) = (ms[i], ms[(i + 1) % n]);
        let (ka, mut kb) = (a.daytime01, b.daytime01);
        if kb <= ka {
            kb += 1.0;
        }
        let kk = if k < ka { k + 1.0 } else { k };
        if kk >= ka && kk < kb {
            return Some((a, b, ((kk - ka) / (kb - ka).max(1e-6)).clamp(0.0, 1.0)));
        }
    }
    Some((ms[n - 1], ms[0], 0.0))
}

/// The blended mood constants at a key (the collection/mood names are the nearer mood's).
pub fn blended_xml(collection: &str, key: f32) -> Option<MoodXml> {
    let (a, b, t) = blend(collection, key)?;
    let l3 = |p: [f32; 3], q: [f32; 3]| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, p[2] + (q[2] - p[2]) * t];
    let l1 = |p: f32, q: f32| p + (q - p) * t;
    let near = if t < 0.5 { a } else { b };
    Some(MoodXml {
        collection: near.collection,
        mood: near.mood,
        latitude: l1(a.latitude, b.latitude),
        daytime01: key,
        l_ambient: l3(a.l_ambient, b.l_ambient),
        l_dir_sun: l3(a.l_dir_sun, b.l_dir_sun),
        l_dir_moon: l3(a.l_dir_moon, b.l_dir_moon),
        max_hdr: near.max_hdr,
        bounce_factor: l1(a.bounce_factor, b.bounce_factor),
        sky_factor: l1(a.sky_factor, b.sky_factor),
    })
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
