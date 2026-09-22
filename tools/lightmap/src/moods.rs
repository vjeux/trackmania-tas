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
    /// Object index of item 0 (ground slots before it); Stadium tiny maps: 16384 + 2736.
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
    m("Stadium", "Day", "fitted", 197.5, 35.0, [0.247, 0.271, 0.278], [0.189, 0.151, 0.174], [0.031, 0.043, 0.036], [0.045, 0.043, 0.042], 16384 + 2736, "Stadium-Day"),
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
/// Measured on the 25 Summer sources and the tiny bakes (2026-09-22; to be
/// confirmed on the giant editor bakes): the objects before the items are
/// `decoration constant + blocks + empty ground columns`:
///
/// * every block (unbaked and baked, free ones included) is one object —
///   except embedded CUSTOM blocks (`…_CustomBlock`: the tiny 05's 46 free
///   water tiles add nothing);
/// * every ground column (grid cell in x, z) WITHOUT a block gets one object
///   (the decoration's own ground tile) — a Nadeo map has none of those (its
///   Grass/terrain blocks cover every column: base = block count exactly), a
///   tiny build with 2412 partial-fill blocks lands on 4096 = 64²;
/// * the Stadium decorations reserve 16384 objects in front (0..3 are the
///   decoration's own charts, 4..16383 unused) and an EMPTY 48×48 Stadium map
///   counts 2736 ground objects = 2304 + 432 (one data point, the tiny 05
///   bake; the Nadeo Stadium maps cover all 2304 columns and show no extra
///   432 — so the 432 are modelled as ground objects that exist only while
///   the map has no ground blocks: `stadium_extra`, to be settled by the 96³
///   giant bakes).
pub struct BaseRule {
    pub deco_const: u32,
    pub ground_cols: u32,
    pub blocks: u32,
    pub custom_blocks: u32,
    pub empty_cols: u32,
    pub stadium_extra: u32,
}

impl BaseRule {
    pub fn base(&self) -> u32 {
        self.deco_const + self.blocks + self.empty_cols + self.stadium_extra
    }
}

/// `blocks`: (grid cell x, grid cell z, name) of every unbaked and baked block.
pub fn base_rule<'a>(envir: &str, decoration: &str, size: [i32; 3], blocks: impl Iterator<Item = (i32, i32, &'a str)>) -> BaseRule {
    let stadium = envir.eq_ignore_ascii_case("stadium") || decoration.to_ascii_lowercase().contains("stadium") || decoration.to_ascii_lowercase().contains("48x48");
    let (sx, sz) = (size[0].max(1) as u32, size[2].max(1) as u32);
    let ground_cols = sx * sz;
    let mut covered = std::collections::HashSet::new();
    let (mut n_blocks, mut n_custom) = (0u32, 0u32);
    for (cx, cz, name) in blocks {
        if name.ends_with("_CustomBlock") {
            n_custom += 1;
            continue;
        }
        n_blocks += 1;
        if cx >= 0 && cz >= 0 && (cx as u32) < sx && (cz as u32) < sz {
            covered.insert((cx as u32, cz as u32));
        }
    }
    let empty_cols = ground_cols - (covered.len() as u32).min(ground_cols);
    let stadium_extra = if stadium && covered.is_empty() { 432 } else { 0 };
    BaseRule { deco_const: if stadium { 16384 } else { 0 }, ground_cols, blocks: n_blocks, custom_blocks: n_custom, empty_cols, stadium_extra }
}
