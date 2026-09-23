//! The diffuse albedo the bounce uses per game material (the lightmapper's `MDiffuse`: the
//! material's diffuse texture, sampled at a coarse mip — RE child 2 (e)). The receiving texel does
//! not care about its own material (the lightmap stores irradiance); the albedo scales what a HIT
//! surface bounces back, so it matters for walls next to bright floors and for cavities.
//!
//! Values, in order: (1) the MEASURED table `data/albedo.tsv` (`mapgeom material-albedo LINK…`: the
//! mean linear RGB of the material's diffuse DDS from the client pack, alpha-weighted; BlueBay and
//! Stadium measured 2026-09-23, 152 links) plus any `*.tsv` in `$LMTOOL_ALBEDO_DIR`; a
//! `<Coll>\Media\Modifier\<X>\<Name>` link without its own image takes `<Coll>\Media\Material\<Name>`
//! or `Stadium\Media\Material\<Name>`; (2) the keyword table below; (3) `DEFAULT`.

pub const DEFAULT: f32 = 0.3;

const MEASURED: &str = include_str!("../data/albedo.tsv");

fn tables() -> &'static std::collections::HashMap<String, [f32; 3]> {
    static T: std::sync::OnceLock<std::collections::HashMap<String, [f32; 3]>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        let mut feed = |text: &str| {
            for l in text.lines() {
                let f: Vec<&str> = l.split('\t').collect();
                if f.len() >= 6 {
                    if let (Ok(r), Ok(g), Ok(b)) = (f[3].parse::<f32>(), f[4].parse::<f32>(), f[5].parse::<f32>()) {
                        m.insert(f[0].to_ascii_lowercase(), [r, g, b]);
                    }
                }
            }
        };
        feed(MEASURED);
        if let Ok(dir) = std::env::var("LMTOOL_ALBEDO_DIR") {
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    if e.path().extension().map(|x| x == "tsv").unwrap_or(false) {
                        if let Ok(t) = std::fs::read_to_string(e.path()) { feed(&t); }
                    }
                }
            }
        }
        m
    })
}

/// LMTOOL_ALBEDO_SRGB=1: hand out the measured albedo in sRGB ENCODING (the raw texture bytes/255 —
/// what a UNORM sample of a BC1_UNORM diffuse map gives, as opposed to the hardware-linearised
/// BC1_UNORM_SRGB read). The lightmapper's MDiffuse raster reads the material's diffuse map; which of the
/// two the format gives is not read from the exe yet — this switch lets the references decide.
fn encode(v: [f32; 3]) -> [f32; 3] {
    static SRGB: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    // default ON (DIFFERENTIAL, 2026-09-23): the sRGB-encoded values bring the test map's wall (lit by the
    // pads' sun bounce) from 0.43 to 0.62 of the editor's; LMTOOL_ALBEDO_SRGB=0 for linear
    if *SRGB.get_or_init(|| std::env::var("LMTOOL_ALBEDO_SRGB").map(|x| x != "0").unwrap_or(true)) {
        let f = |x: f32| if x <= 0.0031308 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
        [f(v[0]), f(v[1]), f(v[2])]
    } else {
        v
    }
}

fn measured(link: &str) -> Option<[f32; 3]> {
    let t = tables();
    let l = link.to_ascii_lowercase();
    if let Some(v) = t.get(&l) {
        return Some(encode(*v));
    }
    // a modifier material: the base material of the same name
    let parts: Vec<&str> = l.split('\\').collect();
    if parts.len() >= 3 && parts[2] == "modifier" {
        let name = parts.last().copied().unwrap_or("");
        for coll in [parts[0], "stadium"] {
            if let Some(v) = t.get(&format!("{coll}\\media\\material\\{name}")) {
                return Some(encode(*v));
            }
        }
    }
    None
}

/// `(keyword, albedo rgb)` — the FIRST keyword found in the link's last path component wins;
/// matching is case-insensitive.
const TABLE: &[(&str, [f32; 3])] = &[
    ("water", [0.06, 0.08, 0.10]),
    ("sand", [0.55, 0.50, 0.40]),
    ("beach", [0.55, 0.50, 0.40]),
    ("snow", [0.75, 0.75, 0.78]),
    ("ice", [0.55, 0.58, 0.62]),
    ("grass", [0.22, 0.28, 0.14]),
    ("dirt", [0.32, 0.26, 0.18]),
    ("rock", [0.30, 0.28, 0.26]),
    ("cliff", [0.30, 0.28, 0.26]),
    ("concrete", [0.42, 0.42, 0.40]),
    ("platform", [0.40, 0.40, 0.40]),
    ("road", [0.20, 0.20, 0.21]),
    ("asphalt", [0.16, 0.16, 0.17]),
    ("tech", [0.25, 0.25, 0.26]),
    ("wood", [0.32, 0.24, 0.16]),
    ("metal", [0.35, 0.35, 0.36]),
    ("glass", [0.15, 0.16, 0.18]),
    ("dark", [0.10, 0.10, 0.10]),
    ("white", [0.65, 0.65, 0.65]),
];

/// The albedo of a material link: measured, else by keyword, else None (the caller's default).
pub fn for_link(link: &str) -> Option<[f32; 3]> {
    if let Some(v) = measured(link) {
        return Some(v);
    }
    let last = link.rsplit(['\\', '/']).next().unwrap_or(link).to_ascii_lowercase();
    for (k, v) in TABLE {
        if last.contains(k) {
            return Some(*v);
        }
    }
    None
}

/// Is a link's albedo MEASURED (from its texture) rather than guessed?
pub fn is_measured(link: &str) -> bool {
    measured(link).is_some()
}

/// Luminance of an albedo triple (for reports).
pub fn lum(a: [f32; 3]) -> f32 {
    0.2126 * a[0] + 0.7152 * a[1] + 0.0722 * a[2]
}
