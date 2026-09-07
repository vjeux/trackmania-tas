//! Light colour skins — the placement's `Skins\Stadium\LightColors\<Name>.dds`
//! (Summer 17: 108 Orange lamps; 20: 115 Green tubes; 21: 69 Off) and the
//! older `Skins\Stadium\LightTube\<Name>.zip` form (an EMPTY zip; the name is
//! the colour) — baked into the item, since an embedded item is never
//! re-skinned by the game (proved by the screens thread, 2026-09-07).
//!
//! How the game applies one: the item header lists the skinnable textures
//! (Lamp: `LightSpot_I`, `ItemLamp_I`, `LightShape_I`; LightTubeBig4m:
//! `LightTube_I`, `LightTubeRefract_I`), and the skin `.dds` — a 16×16 DXT1
//! swatch of ONE colour — replaces each of them. The `_I` textures are the
//! self-illumination of the glass AND the light's projector bitmap, so one
//! swatch colours the glow and the beam alike (and a uniform swatch means the
//! skinned stock beam has no cookie either).
//!
//! Our bake: the GxLight colour is multiplied by the swatch (linear), an
//! `Off` (black) swatch drops the light, and every material of the model that
//! carries an `_I` texture becomes a self-lit custom-texture material whose
//! diffuse and illumination are the swatch file itself, shipped in the
//! library archive as `Items/LightColor_<Name>.dds`. The 20 swatches are the
//! game's own files (`Packs/Stadium_Skins.zip`), bundled here.

/// (name, DDS bytes) for every swatch the game ships.
pub const SWATCHES: &[(&str, &[u8])] = &[
    ("Aqua", include_bytes!("../assets/lightcolors/Aqua.dds")),
    ("Blue", include_bytes!("../assets/lightcolors/Blue.dds")),
    ("Coral", include_bytes!("../assets/lightcolors/Coral.dds")),
    ("Crimson", include_bytes!("../assets/lightcolors/Crimson.dds")),
    ("Cyan", include_bytes!("../assets/lightcolors/Cyan.dds")),
    ("Gold", include_bytes!("../assets/lightcolors/Gold.dds")),
    ("Green", include_bytes!("../assets/lightcolors/Green.dds")),
    ("Lime", include_bytes!("../assets/lightcolors/Lime.dds")),
    ("Magenta", include_bytes!("../assets/lightcolors/Magenta.dds")),
    ("Marine", include_bytes!("../assets/lightcolors/Marine.dds")),
    ("Off", include_bytes!("../assets/lightcolors/Off.dds")),
    ("Orange", include_bytes!("../assets/lightcolors/Orange.dds")),
    ("Orchid", include_bytes!("../assets/lightcolors/Orchid.dds")),
    ("Pink", include_bytes!("../assets/lightcolors/Pink.dds")),
    ("Purple", include_bytes!("../assets/lightcolors/Purple.dds")),
    ("Red", include_bytes!("../assets/lightcolors/Red.dds")),
    ("White", include_bytes!("../assets/lightcolors/White.dds")),
    ("WhiteCold", include_bytes!("../assets/lightcolors/WhiteCold.dds")),
    ("WhiteWarm", include_bytes!("../assets/lightcolors/WhiteWarm.dds")),
    ("Yellow", include_bytes!("../assets/lightcolors/Yellow.dds")),
];

/// A light colour skin, resolved.
#[derive(Clone, Debug, PartialEq)]
pub struct LightSkin {
    pub name: String,
    /// The swatch in sRGB 0..255.
    pub srgb: [u8; 3],
    /// The swatch as the multiplier for a light colour. NOT linearised: the
    /// stock Lamp under the Orange swatch (255, 193, 99) pools at G/R 0.79,
    /// B/R 0.38 on Summer 09's grass — the sRGB values as they are (0.76,
    /// 0.39); the linearised swatch (0.53, 0.13) came out a deep red-orange.
    pub linear: [f32; 3],
    pub dds: &'static [u8],
}

impl LightSkin {
    /// `Items/LightColor_<Name>.dds`'s file name, as the custom material names it.
    pub fn file(&self) -> String {
        format!("LightColor_{}.dds", self.name)
    }
    /// `Off` and any other swatch too dark to light anything.
    pub fn is_off(&self) -> bool {
        self.linear.iter().all(|c| *c < 0.02)
    }
}

/// The colour name of a placement skin path when it is a light colour skin
/// (`Skins\Stadium\LightColors\Coral.dds` -> `Coral`,
/// `Skins\Stadium\LightTube\Red.zip` -> `Red`); `None` for every other skin.
pub fn skin_name(path: &str) -> Option<String> {
    let p = path.replace('/', "\\");
    let lower = p.to_ascii_lowercase();
    let dir_ok = lower.contains("\\lightcolors\\") || lower.contains("\\lighttube\\");
    if !dir_ok {
        return None;
    }
    let file = p.rsplit('\\').next()?;
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    if stem.is_empty() {
        return None;
    }
    Some(stem.to_string())
}

/// The skin by colour name (case-insensitive).
pub fn lookup(name: &str) -> Option<LightSkin> {
    let (n, dds) = SWATCHES.iter().find(|(n, _)| n.eq_ignore_ascii_case(name))?;
    let srgb = dxt1_uniform_rgb(dds)?;
    let lin = |c: u8| -> f32 { c as f32 / 255.0 };
    Some(LightSkin { name: n.to_string(), srgb, linear: [lin(srgb[0]), lin(srgb[1]), lin(srgb[2])], dds })
}

/// The colour of a DXT1 DDS whose first block is one colour (the swatches:
/// 16×16, `DXT1`, 128 data bytes). Block = c0, c1 (RGB565 LE) + 16 2-bit
/// indices; with every index the same the colour is c0 (0), c1 (1),
/// (2c0+c1)/3 (2) or (c0+2c1)/3 (3) — c0 > c1 in every swatch (no 1-bit
/// alpha mode).
pub fn dxt1_uniform_rgb(dds: &[u8]) -> Option<[u8; 3]> {
    if dds.len() < 128 + 8 || &dds[0..4] != b"DDS " {
        return None;
    }
    if &dds[84..88] != b"DXT1" {
        return None;
    }
    let b = &dds[128..136];
    let c0 = u16::from_le_bytes([b[0], b[1]]);
    let c1 = u16::from_le_bytes([b[2], b[3]]);
    let idx = b[4] & 3;
    let rgb = |c: u16| -> [u32; 3] {
        let r = ((c >> 11) & 0x1f) as u32;
        let g = ((c >> 5) & 0x3f) as u32;
        let bl = (c & 0x1f) as u32;
        [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (bl << 3) | (bl >> 2)]
    };
    let (a, c) = (rgb(c0), rgb(c1));
    let mix = |k: usize| -> u8 {
        match idx {
            0 => a[k] as u8,
            1 => c[k] as u8,
            2 => ((2 * a[k] + c[k]) / 3) as u8,
            _ => ((a[k] + 2 * c[k]) / 3) as u8,
        }
    };
    Some([mix(0), mix(1), mix(2)])
}

#[cfg(test)]
mod tests {
    #[test]
    fn swatches_decode() {
        for (name, dds) in super::SWATCHES {
            let s = super::lookup(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(s.dds.len(), dds.len());
            match *name {
                "Off" => assert!(s.is_off(), "{name} {:?}", s.srgb),
                "Red" => assert!(s.srgb[0] > 200 && s.srgb[1] < 100 && s.srgb[2] < 100, "{name} {:?}", s.srgb),
                "Green" => assert!(s.srgb[1] > 200 && s.srgb[0] < 160, "{name} {:?}", s.srgb),
                "White" => assert!(s.srgb.iter().all(|c| *c > 220), "{name} {:?}", s.srgb),
                _ => assert!(!s.is_off(), "{name} {:?}", s.srgb),
            }
        }
    }
    #[test]
    fn names() {
        assert_eq!(super::skin_name("Skins\\Stadium\\LightColors\\Coral.dds").as_deref(), Some("Coral"));
        assert_eq!(super::skin_name("Skins\\Stadium\\LightTube\\Red.zip").as_deref(), Some("Red"));
        assert_eq!(super::skin_name("Skins\\Any\\Advertisement2x1\\Red.zip"), None);
        assert_eq!(super::skin_name("Skins\\Stadium\\ItemFlag\\Summer.zip"), None);
    }
}
