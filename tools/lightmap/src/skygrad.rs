//! The rendered sky as the lightmapper's dome radiance (RE child 2, Tech3/Sky_p): the mood's
//! `SkyColor.dds` (BC6H_UF16 2048×1024 gradient, u = azimuth relative to the sun, v = elevation)
//! × ScaleGrad0, plus the sun-side glow lobes `Scale·cos(θ)^Power·Rgb` (Atmo1/Atmo2 of the mood
//! XML, θ = the angle to the sun), then × GlobalScale. The clouds layer and the fog blend are
//! left out until their constants are read.

impl std::fmt::Debug for SkyGradient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkyGradient({}×{}, scale {})", self.w, self.h, self.scale)
    }
}

pub struct SkyGradient {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 3]>,
    /// Azimuth of the sun (radians, our convention: atan2(x, z)), the u = 0 reference.
    pub sun_az: f32,
    /// u offset of the sun in the texture (0..1) and the u direction sign.
    pub u_sun: f32,
    pub u_sign: f32,
    /// v = 0 row is the zenith when true (else the horizon/bottom).
    pub v_top_is_zenith: bool,
    /// v covers the full −90..90° when true, else 0..90°.
    pub v_full: bool,
    pub scale: f32,
    /// (power, rgb, scale) glow lobes around the sun direction.
    pub lobes: Vec<(f32, [f32; 3], f32)>,
    pub sun_dir: [f32; 3],
}

impl SkyGradient {
    pub fn load(path: &str) -> Result<SkyGradient, String> {
        let d = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let dds = crate::bc6h::parse_dds(&d)?;
        let px = crate::bc6h::decode_image(dds.data, dds.w, dds.h, dds.format == 96);
        Ok(SkyGradient { w: dds.w, h: dds.h, px, sun_az: 0.0, u_sun: 0.0, u_sign: 1.0, v_top_is_zenith: true, v_full: false, scale: 1.0, lobes: Vec::new(), sun_dir: [0.0, 1.0, 0.0] })
    }

    /// The texel at (u, v) in 0..1 (u wraps, v clamps), nearest.
    pub fn texel(&self, u: f32, v: f32) -> [f32; 3] {
        let x = ((u.rem_euclid(1.0) * self.w as f32) as usize).min(self.w - 1);
        let y = ((v.clamp(0.0, 0.99999) * self.h as f32) as usize).min(self.h - 1);
        self.px[y * self.w + x]
    }

    /// The sky radiance in direction `d` (unit).
    pub fn radiance(&self, d: [f32; 3]) -> [f32; 3] {
        let az = d[0].atan2(d[2]);
        let el = d[1].clamp(-1.0, 1.0).asin();
        let u = self.u_sun + self.u_sign * (az - self.sun_az) / (2.0 * std::f32::consts::PI);
        let v_up = if self.v_full { (el / std::f32::consts::PI) + 0.5 } else { (el / std::f32::consts::FRAC_PI_2).max(0.0) };
        let v = if self.v_top_is_zenith { 1.0 - v_up } else { v_up };
        let t = self.texel(u, v);
        let mut out = [t[0] * self.scale, t[1] * self.scale, t[2] * self.scale];
        let cos_t = (d[0] * self.sun_dir[0] + d[1] * self.sun_dir[1] + d[2] * self.sun_dir[2]).max(0.0);
        for (power, rgb, scale) in &self.lobes {
            let f = scale * cos_t.powf(*power);
            for k in 0..3 {
                out[k] += f * rgb[k];
            }
        }
        out
    }

    /// Mean radiance per row band (10 bands) and the brightest column of the brightest band.
    pub fn profile(&self) -> String {
        let mut s = String::new();
        let bands = 10;
        let mut best = (0f32, 0usize, 0usize);
        for b in 0..bands {
            let (y0, y1) = (b * self.h / bands, (b + 1) * self.h / bands);
            let mut acc = [0f64; 3];
            let mut n = 0usize;
            for y in y0..y1 {
                for x in (0..self.w).step_by(8) {
                    let p = self.px[y * self.w + x];
                    for k in 0..3 { acc[k] += p[k] as f64; }
                    n += 1;
                    let lum = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
                    if lum > best.0 { best = (lum, x, y); }
                }
            }
            let nn = n.max(1) as f64;
            s += &format!("  rows {y0:>4}..{y1:<4} mean ({:.3}, {:.3}, {:.3})\n", acc[0] / nn, acc[1] / nn, acc[2] / nn);
        }
        s += &format!("  brightest texel lum {:.3} at x {} (u {:.3}) y {} (v {:.3})", best.0, best.1, best.1 as f32 / self.w as f32, best.2, best.2 as f32 / self.h as f32);
        s
    }
}

/// Parse the mood XML's glow lobes: `<Atmo1 Power="…" Color="rrggbb" Scale="…"/>` and `<Atmo2 …>`
/// (colours are sRGB hex → linear).
pub fn lobes_from_xml(xml: &str) -> Vec<(f32, [f32; 3], f32)> {
    let mut out = Vec::new();
    for tag in ["<Atmo1 ", "<Atmo2 "] {
        let Some(p) = xml.find(tag) else { continue };
        let seg = &xml[p..xml[p..].find("/>").map(|e| p + e).unwrap_or(xml.len())];
        let attr = |name: &str| -> Option<&str> { let k = format!("{name}=\""); let s = seg.find(&k)? + k.len(); let e = seg[s..].find('"')? + s; Some(&seg[s..e]) };
        let (Some(power), Some(color), Some(scale)) = (attr("Power"), attr("Color"), attr("Scale")) else { continue };
        let power: f32 = power.parse().unwrap_or(1.0);
        let scale: f32 = scale.parse().unwrap_or(0.0);
        let hex = u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0xffffff);
        let srgb = |c: u32| -> f32 { let v = c as f32 / 255.0; if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) } };
        let rgb = [srgb((hex >> 16) & 255), srgb((hex >> 8) & 255), srgb(hex & 255)];
        out.push((power, rgb, scale));
    }
    out
}

/// The banked mood files: `…/lightmap-re/client-re/moods/<Collection>-<Mood>-<file>`.
pub fn mood_file(collection: &str, mood: &str, file: &str) -> String {
    format!("{}/persistent/private-30d/tm-player/tiny/lightmap-re/client-re/moods/{collection}-{mood}-{file}", std::env::var("HOME").unwrap_or_default())
}
