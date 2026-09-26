//! The rendered sky as the lightmapper's dome radiance (RE child 2, Tech3/Sky_p): the mood's
//! `SkyColor.dds` (BC6H_UF16 2048×1024 gradient, u = azimuth relative to the sun, v = elevation)
//! × ScaleGrad0, plus the sun-side glow lobes `Scale·cos(θ)^Power·Rgb` (Atmo1/Atmo2 of the mood
//! XML, θ = the angle to the sun), then × GlobalScale. The fog blend is in (`fog`, `sky_ps`). There is
//! NO clouds layer in the lightmapper's dome: PS 16774's second texture `TMapGradientV1` is the OTHER
//! blended mood's SkyColor.dds (pwc-day: t0 = Sunrise, t1 = Day, byte-identical to the banked mood
//! files; ScaleGrad0/1 = (1 − t)·1, t·1 of the blender weight) — port engineer G, 2026-09-26;
//! docs/formats/lightmapper-client.md §6e for the cloud sprites (`clouds.rs`).

impl std::fmt::Debug for SkyGradient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkyGradient({}×{}, scale {})", self.w, self.h, self.scale)
    }
}

#[derive(Clone)]
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
    /// Tech3/Sky_p's fog blend: `lerp(sky, fog_rgb, fog_intens)` before the global scale (the mood XML
    /// <Fog Color IntensMax>; the dome sits at the far depth, so the intensity is IntensMax).
    pub fog: Option<([f32; 3], f32)>,
    /// GlobalScale — applied after the fog blend (the gradient's own ScaleGrad0 is `scale`).
    pub global_scale: f32,
    /// The second mood's gradient texture and the blend fraction toward it (the mood blender lerps the
    /// moods; the texture is lerped per texel here). Same size as `px`.
    pub px2: Option<Vec<[f32; 3]>>,
    pub blend_t: f32,
    /// v = sin(elevation) (a dome whose texture v follows the height) instead of elevation/90°.
    pub v_sin: bool,
    /// `dome_radiance`'s u addressing for the gradient texture: 0 wrap, 1 mirror, 2 clamp (the sampler
    /// state is not in the capture's action list; mirror is the reading that fits the u = az/π mesh).
    pub dome_u_mode: u8,
}

/// LMTOOL_BILINEAR_F32=1: f32 bilinear weights instead of the GPU's 8-bit fractions (a probe).
static BILINEAR_F32: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_BILINEAR_F32").map(|v| v == "1").unwrap_or(false));

impl SkyGradient {
    pub fn load(path: &str) -> Result<SkyGradient, String> {
        let d = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let dds = crate::bc6h::parse_dds(&d)?;
        let px = crate::bc6h::decode_image(dds.data, dds.w, dds.h, dds.format == 96);
        Ok(SkyGradient { w: dds.w, h: dds.h, px, sun_az: 0.0, u_sun: 0.0, u_sign: 1.0, v_top_is_zenith: true, v_full: false, scale: 1.0, lobes: Vec::new(), sun_dir: [0.0, 1.0, 0.0], fog: None, global_scale: 1.0, px2: None, blend_t: 0.0, v_sin: false, dome_u_mode: 1 })
    }

    /// The texel at (u, v) in 0..1 (u wraps, v clamps), nearest.
    pub fn texel(&self, u: f32, v: f32) -> [f32; 3] {
        let x = ((u.rem_euclid(1.0) * self.w as f32) as usize).min(self.w - 1);
        let y = ((v.clamp(0.0, 0.99999) * self.h as f32) as usize).min(self.h - 1);
        let p = self.px[y * self.w + x];
        match &self.px2 {
            Some(q) if self.blend_t > 0.0 => {
                let q = q[(y * self.w + x).min(q.len() - 1)];
                let t = self.blend_t;
                [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, p[2] + (q[2] - p[2]) * t]
            }
            _ => p,
        }
    }

    /// Blend toward a second mood's gradient (same size, else resampled by nearest).
    pub fn blend_with(&mut self, other: &SkyGradient, t: f32) {
        if other.w == self.w && other.h == self.h {
            self.px2 = Some(other.px.clone());
        } else {
            let mut q = Vec::with_capacity(self.w * self.h);
            for y in 0..self.h {
                for x in 0..self.w {
                    let (ox, oy) = ((x * other.w / self.w).min(other.w - 1), (y * other.h / self.h).min(other.h - 1));
                    q.push(other.px[oy * other.w + ox]);
                }
            }
            self.px2 = Some(q);
        }
        self.blend_t = t;
    }

    /// The texture sampled with bilinear filtering at (u, v) in texture units, u addressed by `u_mode`
    /// (0 wrap, 1 mirror, 2 clamp), v clamped — `sample_indexable` with SMapGradientV.
    pub fn sample_linear(&self, u: f32, v: f32, u_mode: u8) -> [f32; 3] {
        let wrap_u = |x: f32| -> f32 {
            match u_mode {
                1 => { let m = x.rem_euclid(2.0); if m > 1.0 { 2.0 - m } else { m } }
                2 => x.clamp(0.0, 1.0),
                _ => x.rem_euclid(1.0),
            }
        };
        let fx = wrap_u(u) * self.w as f32 - 0.5;
        let fy = (v.clamp(0.0, 1.0) * self.h as f32 - 0.5).clamp(0.0, (self.h - 1) as f32);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (mut tx, mut ty) = (fx - x0, fy - y0);
        // the GPU's bilinear weights carry 8 fractional bits (D3D11 3.2.3: at least 8 bits of sub-texel precision) — the
        // fraction ROUNDED to the nearest 1/256 (half to even), and the filtered value of this f16 texture is an f16: the
        // weighted sum rounded to nearest even (`lmtool dome-check` on the captured environment layer of pwc-day: floor
        // weights + an f32 result 98.59 / 99.15 / 99.49 % exact per channel; rounded weights + the f16 result 99.80 / 99.89 /
        // 99.96 %; 7- or 9-bit weights, truncated f16, per-product f16 all worse). LMTOOL_BILINEAR_F32=1 keeps the old form.
        let q8 = |t: f32| -> f32 { let s = t * 256.0; let f = s.floor(); let d = s - f; (if d > 0.5 { f + 1.0 } else if d < 0.5 { f } else if (f as i64) % 2 == 0 { f } else { f + 1.0 }) / 256.0 };
        if !*BILINEAR_F32 {
            tx = q8(tx);
            ty = q8(ty);
        }
        let xi = |x: f32| -> usize {
            match u_mode {
                0 => (x.rem_euclid(self.w as f32)) as usize % self.w,
                _ => (x.clamp(0.0, (self.w - 1) as f32)) as usize,
            }
        };
        let (xa, xb) = (xi(x0), xi(x0 + 1.0));
        let (ya, yb) = (y0 as usize, ((y0 + 1.0) as usize).min(self.h - 1));
        // THE MOOD BLEND (V's finding, 2026-09-26 16:35Z): a blended DayTime word loads mood A's gradient and `blend_with` mood B's
        // (px2, blend_t) — `texel` blended them, this sampler (the transcribed dome's) read `px` alone, so stpad at 0x4e4b
        // (Night 0.1 % + Sunrise 99.9 %) rendered the NIGHT sky at full weight (AddAmbient 0.554/0.665/0.813 → 0.088/0.071/0.051).
        // The texel = lerp(A, B, t) before the bilinear weights — the same value `texel` returns at the texel centre.
        let p = |x: usize, y: usize| -> [f32; 3] {
            let a = self.px[y * self.w + x];
            match &self.px2 {
                Some(q) if self.blend_t > 0.0 => { let b = q[(y * self.w + x).min(q.len() - 1)]; let t = self.blend_t; [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t] }
                _ => a,
            }
        };
        let (p00, p10, p01, p11) = (p(xa, ya), p(xb, ya), p(xa, yb), p(xb, yb));
        let mut out = [0f32; 3];
        let (w00, w10, w01, w11) = ((1.0 - tx) * (1.0 - ty), tx * (1.0 - ty), (1.0 - tx) * ty, tx * ty);
        for k in 0..3 {
            let v = p00[k] * w00 + p10[k] * w10 + p01[k] * w01 + p11[k] * w11;
            out[k] = if *BILINEAR_F32 { v } else { crate::gpufmt::quantise_f16(v, crate::gpufmt::Rounding::NearestEven) };
        }
        out
    }

    /// THE GAME'S SKY DOME, transcribed from the capture (pwc-day frame 127448, draw eid 1051: VS 16773
    /// `GbxSkyV0`, PS 16774; mesh e001051: an ellipsoid of 2143 vertices in 33 rings, radii 22265.2265625
    /// (x, z) and 9751.1611328125 (y), centred at the WORLD ORIGIN with VisualToWorld = identity; 65 vertices
    /// per ring, u = atan2(x, z)/π; v per ring from the table below (InvertY: the texture v = 1 − v)).
    /// The peel's pixel ray `q + t·d` (orthographic) meets the dome's inner face at P (the far root); the
    /// mesh interpolates u linearly around the ring and v linearly in the height between the two rings of
    /// P's band; the vertex shader shifts u by `LightDirAngle_m11Zx` = the sun's azimuth/π and inverts v;
    /// the pixel shader samples TMapGradientV1 (ScaleGrad1 = 1; TMapGradientV × ScaleGrad0 = 0 adds
    /// nothing), adds the two Atmo lobes `Scale·max(0, view·L)^Power·Rgb` with view = normalize(P − eye),
    /// blends toward the fog colour by FogIntens, multiplies by GlobalScale and clamps to 16375.
    pub fn dome_radiance(&self, q: [f32; 3], d: [f32; 3], eye: [f32; 3]) -> [f32; 3] {
        self.dome_radiance_shift(q, d, eye, None)
    }

    /// `dome_radiance` with the vertex shader's u shift overridden (a probe: which shift the capture fits).
    pub fn dome_radiance_shift(&self, q: [f32; 3], d: [f32; 3], eye: [f32; 3], shift: Option<f32>) -> [f32; 3] {
        const A: f64 = 22265.2265625;
        const B: f64 = 9751.1611328125;
        // the 17 rings of the upper half: (y of the ring, v of the ring); the lower half mirrors them
        const RINGS: [(f32, f32); 17] = [
            (0.0, 0.0), (955.8, 0.0248), (1902.4, 0.0574), (2830.6, 0.0976), (3731.6, 0.1449), (4596.7, 0.1989), (5417.5, 0.2591), (6186.1, 0.3249),
            (6895.1, 0.3956), (7537.7, 0.4706), (8107.8, 0.5492), (8599.8, 0.6305), (9008.9, 0.7139), (9331.3, 0.7985), (9563.8, 0.8835), (9704.2, 0.9680), (9751.1611328125, 1.0),
        ];
        // the far intersection of q + t·d with (x² + z²)/A² + y²/B² = 1
        let (qx, qy, qz) = (q[0] as f64, q[1] as f64, q[2] as f64);
        let (dx, dy, dz) = (d[0] as f64, d[1] as f64, d[2] as f64);
        let aa = (dx * dx + dz * dz) / (A * A) + dy * dy / (B * B);
        let bb = 2.0 * ((qx * dx + qz * dz) / (A * A) + qy * dy / (B * B));
        let cc = (qx * qx + qz * qz) / (A * A) + qy * qy / (B * B) - 1.0;
        let disc = (bb * bb - 4.0 * aa * cc).max(0.0);
        let t = (-bb + disc.sqrt()) / (2.0 * aa);
        let p = [qx + t * dx, qy + t * dy, qz + t * dz];
        // the mesh's texture coordinates at P: u around the ring, v by the band's height
        let u_mesh = (p[0].atan2(p[2]) / std::f64::consts::PI) as f32;
        let ay = p[1].abs() as f32;
        let mut v_mesh = 1.0f32;
        for k in 0..16 {
            let (y0, v0) = RINGS[k];
            let (y1, v1) = RINGS[k + 1];
            if ay <= y1 {
                v_mesh = v0 + (v1 - v0) * ((ay - y0) / (y1 - y0)).clamp(0.0, 1.0);
                break;
            }
        }
        // VS: o1.x = u − LightDirAngle_m11Zx (GradientV_ForceX < 0), o1.y = 1 − v (GradientV_InvertY)
        let sun_u = shift.unwrap_or_else(|| (self.sun_dir[0].atan2(self.sun_dir[2]) / std::f32::consts::PI) as f32);
        let u = u_mesh - sun_u;
        let v = 1.0 - v_mesh;
        static DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *DEBUG.get_or_init(|| std::env::var_os("LMTOOL_DOME_DEBUG").is_some()) {
            let g = self.sample_linear(u, v, self.dome_u_mode);
            eprintln!("dome: q ({:.1},{:.1},{:.1}) d ({:.3},{:.3},{:.3}) t {t:.0} P ({:.0},{:.0},{:.0}) u_mesh {u_mesh:.4} v_mesh {v_mesh:.4} sun_u {sun_u:.4} u {u:.4} v {v:.4} tex ({:.4},{:.4},{:.4}) sun_dir ({:.3},{:.3},{:.3}) scale {} fog {:?} global {}", q[0], q[1], q[2], d[0], d[1], d[2], p[0], p[1], p[2], g[0], g[1], g[2], self.sun_dir[0], self.sun_dir[1], self.sun_dir[2], self.scale, self.fog, self.global_scale);
        }
        self.sky_ps([u, v], [p[0] as f32 - eye[0], p[1] as f32 - eye[1], p[2] as f32 - eye[2]])
    }

    /// PS 16774 (the sky dome's pixel shader) on the interpolated attributes: `uv` = o1 (the VS's shifted,
    /// inverted texture coordinate), `view` = o2 = world − eye (normalised here, as the shader does):
    /// `r1 = TMapGradientV1(uv)·ScaleGrad1` (`scale`; TMapGradientV·ScaleGrad0 = 0 in the capture),
    /// the Atmo lobes `exp2(Power·log2(max(0, view·−LightDir)))·Scale·Rgb` (`lobes`; the sun disc is off:
    /// SunIsVisible 0), the fog `lerp(Fog, ·, sat(1 − FogIntens))`, × GlobalScale, min 16375.
    pub fn sky_ps(&self, uv: [f32; 2], view: [f32; 3]) -> [f32; 3] {
        let g = self.sample_linear(uv[0], uv[1], self.dome_u_mode);
        let mut out = [g[0] * self.scale, g[1] * self.scale, g[2] * self.scale];
        let view = { let l = (view[0] * view[0] + view[1] * view[1] + view[2] * view[2]).sqrt().max(1e-9); [view[0] / l, view[1] / l, view[2] / l] };
        let cos_t = (view[0] * self.sun_dir[0] + view[1] * self.sun_dir[1] + view[2] * self.sun_dir[2]).max(0.0);
        for (power, rgb, scale) in &self.lobes {
            // log/exp as the shader: exp(power·log(cos)) → 0 at cos = 0
            let f = if cos_t > 0.0 { scale * (power * cos_t.ln()).exp() } else { 0.0 };
            for k in 0..3 {
                out[k] += f * rgb[k];
            }
        }
        if let Some((fog, fi)) = self.fog {
            let w = (1.0 - fi).clamp(0.0, 1.0);
            for k in 0..3 {
                out[k] = (out[k] - fog[k]) * w + fog[k];
            }
        }
        for k in 0..3 {
            out[k] = (out[k] * self.global_scale).min(16375.0);
        }
        out
    }

    /// The VS 16773 constant GbxSkyV0.LightDirAngle_m11Zx as the port derives it: the sun's azimuth/π
    /// (atan2(x, z)/π of the direction TOWARDS the sun) — 0.8138 in the capture for the sun at
    /// (0.22097, 0.91646, −0.33357).
    pub fn light_dir_angle(&self) -> f32 {
        (self.sun_dir[0].atan2(self.sun_dir[2]) / std::f32::consts::PI) as f32
    }

    /// The sky radiance in direction `d` (unit).
    pub fn radiance(&self, d: [f32; 3]) -> [f32; 3] {
        let az = d[0].atan2(d[2]);
        let el = d[1].clamp(-1.0, 1.0).asin();
        let u = self.u_sun + self.u_sign * (az - self.sun_az) / (2.0 * std::f32::consts::PI);
        let v_up = if self.v_full { (el / std::f32::consts::PI) + 0.5 } else if self.v_sin { el.sin().max(0.0) } else { (el / std::f32::consts::FRAC_PI_2).max(0.0) };
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
        // Sky_p: lerp toward the fog colour, then the global scale
        if let Some((fog, fi)) = self.fog {
            for k in 0..3 {
                out[k] = out[k] * (1.0 - fi) + fog[k] * fi;
            }
        }
        for k in 0..3 {
            out[k] *= self.global_scale;
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
                    let p = self.texel((x as f32 + 0.5) / self.w as f32, (y as f32 + 0.5) / self.h as f32);
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

/// The mood XML's fog for the sky dome at distance `dome_m`: (linear Color, intensity) with
/// `Intens = IntensMin + (IntensMax − IntensMin)·((d − DepthMin)/(DepthMax − DepthMin))^Exponant`
/// (`<Fog … DepthMin="256" DepthMax="25000" Exponant="0.7" IntensMin="0" IntensMax="0.976" Color="b8d7f5">`).
/// The dome distance is not read from the exe; 4000 m reproduces the BlueBay Day open-pad colour
/// (the fitted intensity 0.27) — DIFFERENTIAL.
pub fn fog_from_xml(xml: &str, dome_m: f32) -> Option<([f32; 3], f32)> {
    let p = xml.find("<Fog ")?;
    let seg = &xml[p..xml[p..].find('>').map(|e| p + e).unwrap_or(xml.len())];
    let attr = |name: &str| -> Option<&str> { let k = format!("{name}=\""); let s = seg.find(&k)? + k.len(); let e = seg[s..].find('"')? + s; Some(&seg[s..e]) };
    let num = |name: &str, dflt: f32| -> f32 { attr(name).and_then(|v| v.parse().ok()).unwrap_or(dflt) };
    let color = attr("Color")?;
    // Sky_p's FogIntens is the <Fog><SkyClouds GlobalIntens="…"/> value (RE child 4, the Vision sky
    // constant filler 0x1409f7a40: cb+0x3c = fog+0x3c when Fog.Enabled) — BlueBay Day 0.414, Sunset 0,
    // Sunrise 0.048, Night 0.068. The depth formula below is the fallback when the tag is missing
    // (dome_m ≤ 0 also forces it off).
    let sky_clouds: Option<f32> = xml.find("<SkyClouds ").and_then(|q| {
        let seg2 = &xml[q..xml[q..].find("/>").map(|e| q + e).unwrap_or(xml.len())];
        let k = "GlobalIntens=\"";
        let s = seg2.find(k)? + k.len();
        let e = seg2[s..].find('"')? + s;
        seg2[s..e].parse().ok()
    });
    let (imin, imax, dmin, dmax, ex) = (num("IntensMin", 0.0), num("IntensMax", 1.0), num("DepthMin", 0.0), num("DepthMax", 25000.0), num("Exponant", 1.0));
    let t = ((dome_m - dmin) / (dmax - dmin).max(1.0)).clamp(0.0, 1.0).powf(ex);
    let intens = match sky_clouds { Some(v) if dome_m > 0.0 => v, _ => imin + (imax - imin) * t };
    let hex = u32::from_str_radix(color.trim_start_matches('#'), 16).ok()?;
    let srgb = |c: u32| -> f32 { let v = c as f32 / 255.0; if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) } };
    Some(([srgb((hex >> 16) & 255), srgb((hex >> 8) & 255), srgb(hex & 255)], intens))
}

/// Lerp two lobe lists element-wise (Atmo1, Atmo2 of the two moods; the shorter list's missing lobes count
/// as zero-scale copies of the other's).
pub fn lerp_lobes(a: &[(f32, [f32; 3], f32)], b: &[(f32, [f32; 3], f32)], t: f32) -> Vec<(f32, [f32; 3], f32)> {
    let n = a.len().max(b.len());
    (0..n)
        .map(|i| {
            let la = a.get(i).copied().or_else(|| b.get(i).map(|l| (l.0, l.1, 0.0))).unwrap();
            let lb = b.get(i).copied().or_else(|| a.get(i).map(|l| (l.0, l.1, 0.0))).unwrap();
            let l1 = |p: f32, q: f32| p + (q - p) * t;
            (l1(la.0, lb.0), [l1(la.1[0], lb.1[0]), l1(la.1[1], lb.1[1]), l1(la.1[2], lb.1[2])], l1(la.2, lb.2))
        })
        .collect()
}

/// Lerp two fog settings (colour and intensity).
pub fn lerp_fog(a: Option<([f32; 3], f32)>, b: Option<([f32; 3], f32)>, t: f32) -> Option<([f32; 3], f32)> {
    match (a, b) {
        (Some((ca, ia)), Some((cb, ib))) => Some(([ca[0] + (cb[0] - ca[0]) * t, ca[1] + (cb[1] - ca[1]) * t, ca[2] + (cb[2] - ca[2]) * t], ia + (ib - ia) * t)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        _ => None,
    }
}

/// The banked mood files: `…/lightmap-re/client-re/moods/<Collection>-<Mood>-<file>`.
pub fn mood_file(collection: &str, mood: &str, file: &str) -> String {
    format!("{}/persistent/private-30d/tm-player/tiny/lightmap-re/client-re/moods/{collection}-{mood}-{file}", std::env::var("HOME").unwrap_or_default())
}
