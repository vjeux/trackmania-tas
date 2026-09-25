//! `lmtool bake --raster --dump-passes DIR`: the port's intermediates, written at the same points the
//! game has its render targets, one raw buffer per file plus one MANIFEST.json at the root (the
//! layout agreed with the baker's capture, 2026-09-24: pass names, raw bytes exactly as a GPU target
//! holds them — R11G11B10 as one u32 per pixel, depth as f32 — origin top-left, and per entry the
//! direction vector and the orthographic frustum the buffer was rendered with). `passdiff` reads the
//! same manifest from both sides.
//!
//! Pass names, pipeline order: `sun_shadow` (D32, once), `lm_pos` / `lm_nrm` (the LM-space
//! position/normal raster, once), `mdiffuse` (per chart, once), `ilightinput` (per sweep, per chart),
//! `peel_depth` / `peel_color` (per sweep, direction, layer), `ilightdir` (per sweep, direction, chart),
//! `lightsum` (per sweep, per chart, the accumulation target), `lightsum_resolved` (per sweep, per
//! chart, after the ss resolve), `probe_*`, `final_hdr` (the atlas before encode, f32) and
//! `final_atlas` (the 8-bit colour image before WEBP).

use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::geometry::V3;

/// An orthographic frustum as the game's `CHmsVolumeShadow` holds it: a world centre, half extents
/// along (right, up, forward), reversed depth `z01 = 0.5 + (center·forward − p·forward)/(2·half.z)`
/// (1 = the near plane on the camera's side, 0 = far); pixel x grows along +right, pixel y along −up
/// (D3D viewport, origin top-left), `px = (p·right − center·right + half.x)/(2·half.x)·width`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Frustum {
    #[serde(default = "Frustum::default_ortho")]
    pub ortho: bool,
    pub center: [f32; 3],
    pub half: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    #[serde(default = "Frustum::default_depth")]
    pub depth: String,
}

impl Frustum {
    pub const REVERSED: &'static str = "reversed_z01";
    fn default_ortho() -> bool {
        true
    }
    fn default_depth() -> String {
        Frustum::REVERSED.into()
    }

    /// Pixel coordinates (continuous, pixel k spans [k, k+1)) and reversed z01 of a world point on a
    /// `w`×`h` target.
    pub fn project(&self, p: V3, w: u32, h: u32) -> (f32, f32, f32) {
        let d = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let px = (d(p, self.right) - d(self.center, self.right) + self.half[0]) / (2.0 * self.half[0]) * w as f32;
        let py = (d(self.center, self.up) + self.half[1] - d(p, self.up)) / (2.0 * self.half[1]) * h as f32;
        let z = 0.5 + (d(self.center, self.forward) - d(p, self.forward)) / (2.0 * self.half[2]);
        (px, py, z)
    }

    /// The world point at pixel (px, py) (continuous) and reversed depth `z01`.
    pub fn unproject(&self, px: f32, py: f32, z01: f32, w: u32, h: u32) -> V3 {
        let s = (px / w as f32) * 2.0 * self.half[0] - self.half[0];
        let t = self.half[1] - (py / h as f32) * 2.0 * self.half[1];
        let f = (0.5 - z01) * 2.0 * self.half[2];
        let mut p = self.center;
        for k in 0..3 {
            p[k] += s * self.right[k] + t * self.up[k] + f * self.forward[k];
        }
        p
    }

    /// Metres along `forward` from the far plane (z01 = 0) — a frustum-free depth for comparisons.
    pub fn depth_metres(&self, z01: f32) -> f32 {
        (1.0 - z01) * 2.0 * self.half[2]
    }

    /// The frustum behind the game's `WorldPw01Shadow` = Bias·Proj·View in the row-vector convention
    /// `(u, v, z01, 1) = (x, y, z, 1)·M`: u, v are the lookup's texture coordinates over the whole target
    /// (the Bias rows' one-texel inset is inside them, and the layer render's 1-px-inset viewport
    /// registers to the same pixels), z01 the reversed depth. Columns 0–2 of the upper 3×3 are the
    /// three axes scaled by the inverse extents, row 3 the offsets.
    pub fn from_pw01(m: &[[f32; 4]; 4]) -> Option<Frustum> {
        let a = [m[0][0], m[1][0], m[2][0]];
        let b = [m[0][1], m[1][1], m[2][1]];
        let c = [m[0][2], m[1][2], m[2][2]];
        let (a0, b0, c0) = (m[3][0], m[3][1], m[3][2]);
        let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let (la, lb, lc) = (len(a), len(b), len(c));
        if !(la > 0.0 && lb > 0.0 && lc > 0.0) || !la.is_finite() || !lb.is_finite() || !lc.is_finite() {
            return None;
        }
        let right = [a[0] / la, a[1] / la, a[2] / la];
        let up = [-b[0] / lb, -b[1] / lb, -b[2] / lb];
        let forward = [-c[0] / lc, -c[1] / lc, -c[2] / lc];
        let half = [0.5 / la, 0.5 / lb, 0.5 / lc];
        let (cr, cu, cf) = ((0.5 - a0) / la, (b0 - 0.5) / lb, (c0 - 0.5) / lc);
        let mut center = [0f32; 3];
        for k in 0..3 {
            center[k] = cr * right[k] + cu * up[k] + cf * forward[k];
        }
        Some(Frustum { ortho: true, center, half, right, up, forward, depth: Frustum::REVERSED.into() })
    }
}

/// Which chart an atlas-space buffer belongs to (our buffers are written per chart; the game's atlas
/// is cut into charts by its mapping).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChartRef {
    /// The object id (item base + item index) — the mapping's `obj_group_idx / 4`.
    pub obj: u32,
    /// The item index in the map.
    pub item: u32,
    pub sub: u32,
}

/// One buffer of the dump.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub pass: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sweep: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<u32>,
    /// Which of the direction's peels (the game runs two per direction: 0 = the whole-scene frustum,
    /// 1 = the frustum fitted to the lightmapped items; the accumulate takes the later peel's layer
    /// where it has one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peel: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart: Option<ChartRef>,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub row_pitch: u32,
    #[serde(default)]
    pub origin: String,
    /// `peel` (the direction's ortho target), `chart_ss` (a chart's ss× raster), `chart` (a chart at
    /// stored resolution), `atlas` (the whole stored atlas), `probe`.
    #[serde(default)]
    pub space: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frustum: Option<Frustum>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleared_to: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Capture-side extras (RenderDoc): the peel camera's `WorldPw01Shadow` (Bias·Proj·View, row-vector
    /// convention), the viewport, the last event id of the buffer's snapshot, the raster / depth state.
    #[serde(default, rename = "view_proj_bias_GbxWorldPw01Shadow", skip_serializing_if = "Option::is_none")]
    pub pw01: Option<[[f32; 4]; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eid_last: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depthstate: Option<serde_json::Value>,
}

/// A chart's rectangle in the layout (2048-unit space, the mapping's convention) and its stored size.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ChartRect {
    pub obj: u32,
    #[serde(default)]
    pub item: u32,
    #[serde(default)]
    pub sub: u32,
    /// Layout units (x, y odd; w, h even) — the atlas texel footprint is x/2 … (x+w)/2 − 1 at 1024².
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Stored texels.
    #[serde(default)]
    pub chart_w: u32,
    #[serde(default)]
    pub chart_h: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sweep {
    #[serde(default)]
    pub sweep: u32,
    #[serde(default)]
    pub n_dirs: u32,
    /// The accumulation constant `4/N`.
    #[serde(default)]
    pub scale: f32,
    #[serde(default)]
    pub dirs: Vec<[f32; 3]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Atlas {
    /// The layout size (2048) and the ss factor.
    #[serde(default = "Atlas::d2048")]
    pub w: u32,
    #[serde(default = "Atlas::d2048")]
    pub h: u32,
    #[serde(default = "Atlas::d3")]
    pub ss: u32,
    /// The stored image size (1024).
    #[serde(default = "Atlas::d1024")]
    pub stored_w: u32,
    #[serde(default = "Atlas::d1024")]
    pub stored_h: u32,
}

impl Atlas {
    fn d2048() -> u32 {
        2048
    }
    fn d3() -> u32 {
        3
    }
    fn d1024() -> u32 {
        1024
    }
    pub fn default_atlas() -> Atlas {
        Atlas { w: 2048, h: 2048, ss: 3, stored_w: 1024, stored_h: 1024 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub producer: String,
    #[serde(default)]
    pub map: String,
    /// The baked output map (the editor's save after ComputeShadows): its mapping gives the chart rects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baked_map: Option<String>,
    #[serde(default)]
    pub quality: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daytime_word: Option<u32>,
    #[serde(default)]
    pub mood: String,
    #[serde(default = "Atlas::default_atlas")]
    pub atlas: Atlas,
    /// Unit vector TOWARDS the sun (the light travels along −sun_dir).
    #[serde(default)]
    pub sun_dir: [f32; 3],
    #[serde(default)]
    pub sun_rgb: [f32; 3],
    #[serde(default)]
    pub sweeps: Vec<Sweep>,
    #[serde(default)]
    pub layout: Vec<ChartRect>,
    /// Free-form conventions of this dump (depth bias, quantisers, layer 0 semantics…) for the reader.
    #[serde(default)]
    pub conventions: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub passes: Vec<Entry>,
}

/// The dump writer.
pub struct PassDump {
    pub root: std::path::PathBuf,
    pub manifest: Manifest,
    /// Which directions of each sweep get their per-direction buffers written (None = all).
    pub dirs: Option<Vec<u32>>,
    pub bytes_written: u64,
}

impl std::fmt::Debug for PassDump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PassDump({}, {} entries)", self.root.display(), self.manifest.passes.len())
    }
}

impl PassDump {
    pub fn new(root: &str, map: &str, quality: u32, mood: &str, ss: u32) -> std::io::Result<PassDump> {
        std::fs::create_dir_all(root)?;
        Ok(PassDump {
            root: std::path::PathBuf::from(root),
            manifest: Manifest {
                producer: "lmtool bake --raster --dump-passes".into(),
                map: map.into(),
                baked_map: None,
                quality,
                daytime_word: None,
                mood: mood.into(),
                atlas: Atlas { w: 2048, h: 2048, ss, stored_w: 1024, stored_h: 1024 },
                sun_dir: [0.0; 3],
                sun_rgb: [0.0; 3],
                sweeps: Vec::new(),
                layout: Vec::new(),
                conventions: serde_json::Map::new(),
                passes: Vec::new(),
            },
            dirs: None,
            bytes_written: 0,
        })
    }

    /// Is direction `i` one of those dumped per direction?
    pub fn wants_dir(&self, i: u32) -> bool {
        self.dirs.as_ref().map(|d| d.contains(&i)).unwrap_or(true)
    }

    pub fn convention(&mut self, key: &str, v: serde_json::Value) {
        self.manifest.conventions.insert(key.into(), v);
    }

    fn write_file(&mut self, rel: &str, bytes: &[u8]) -> std::io::Result<()> {
        let p = self.root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::io::BufWriter::new(std::fs::File::create(&p)?);
        f.write_all(bytes)?;
        self.bytes_written += bytes.len() as u64;
        Ok(())
    }

    /// Write an f32 buffer of `channels` per pixel.
    pub fn write_f32(&mut self, mut e: Entry, w: u32, h: u32, channels: u32, data: &[f32]) -> std::io::Result<()> {
        assert_eq!(data.len(), (w * h * channels) as usize, "{}: buffer size", e.pass);
        let mut bytes = Vec::with_capacity(data.len() * 4);
        for v in data {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        e.width = w;
        e.height = h;
        e.row_pitch = w * channels * 4;
        e.origin = "top-left".into();
        if e.format.is_empty() {
            e.format = match channels { 1 => "R32_FLOAT", 2 => "R32G32_FLOAT", 3 => "R32G32B32_FLOAT", _ => "R32G32B32A32_FLOAT" }.into();
        }
        self.write_file(&e.file.clone(), &bytes)?;
        self.manifest.passes.push(e);
        Ok(())
    }

    /// Write a u32-per-pixel buffer (R11G11B10_FLOAT packed).
    pub fn write_u32(&mut self, mut e: Entry, w: u32, h: u32, data: &[u32]) -> std::io::Result<()> {
        assert_eq!(data.len(), (w * h) as usize, "{}: buffer size", e.pass);
        let mut bytes = Vec::with_capacity(data.len() * 4);
        for v in data {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        e.width = w;
        e.height = h;
        e.row_pitch = w * 4;
        e.origin = "top-left".into();
        self.write_file(&e.file.clone(), &bytes)?;
        self.manifest.passes.push(e);
        Ok(())
    }

    /// Write an 8-bit buffer (`channels` bytes per pixel).
    pub fn write_u8(&mut self, mut e: Entry, w: u32, h: u32, channels: u32, data: &[u8]) -> std::io::Result<()> {
        assert_eq!(data.len(), (w * h * channels) as usize, "{}: buffer size", e.pass);
        e.width = w;
        e.height = h;
        e.row_pitch = w * channels;
        e.origin = "top-left".into();
        self.write_file(&e.file.clone(), data)?;
        self.manifest.passes.push(e);
        Ok(())
    }

    /// Write an RGB buffer in the given storage format: f32 triples, R11G11B10 words or f16 quads
    /// (alpha 1) — the bytes a target of that format holds.
    pub fn write_rgb(&mut self, e: Entry, w: u32, h: u32, rgb: &[[f32; 3]], q: crate::gpufmt::Quant, r: crate::gpufmt::Rounding) -> std::io::Result<()> {
        let mut e = e;
        e.format = q.dxgi_rgb().into();
        match q {
            crate::gpufmt::Quant::None => {
                let flat: Vec<f32> = rgb.iter().flat_map(|c| c.iter().copied()).collect();
                self.write_f32(e, w, h, 3, &flat)
            }
            crate::gpufmt::Quant::R11G11B10 => {
                let packed: Vec<u32> = rgb.iter().map(|c| crate::gpufmt::pack_r11g11b10(*c, r)).collect();
                self.write_u32(e, w, h, &packed)
            }
            crate::gpufmt::Quant::F16 => {
                let mut bytes = Vec::with_capacity(rgb.len() * 8);
                for c in rgb {
                    for k in 0..3 {
                        bytes.extend_from_slice(&crate::gpufmt::encode_f16(c[k], r).to_le_bytes());
                    }
                    bytes.extend_from_slice(&0x3c00u16.to_le_bytes());
                }
                e.width = w;
                e.height = h;
                e.row_pitch = w * 8;
                e.origin = "top-left".into();
                self.write_file(&e.file.clone(), &bytes)?;
                self.manifest.passes.push(e);
                Ok(())
            }
        }
    }

    /// Write MANIFEST.json (call once at the end; safe to call again after more entries).
    pub fn finish(&self) -> std::io::Result<()> {
        let s = serde_json::to_string_pretty(&self.manifest).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(self.root.join("MANIFEST.json"), s)
    }
}

/// An entry with the common fields filled; the writer fills the size fields.
pub fn entry(pass: &str, file: String, space: &str) -> Entry {
    Entry {
        pass: pass.into(),
        sweep: None,
        direction: None,
        layer: None,
        peel: None,
        chart: None,
        file,
        format: String::new(),
        width: 0,
        height: 0,
        row_pitch: 0,
        origin: "top-left".into(),
        space: space.into(),
        dir: None,
        frustum: None,
        cleared_to: None,
        notes: None,
        pw01: None,
        viewport: None,
        eid_last: None,
        raster: None,
        depthstate: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fr() -> Frustum {
        Frustum { ortho: true, center: [10.0, 5.0, -3.0], half: [8.0, 4.0, 20.0], right: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0], forward: [0.0, 0.0, 1.0], depth: Frustum::REVERSED.into() }
    }

    #[test]
    fn frustum_projection_conventions() {
        let f = fr();
        // the centre lands mid-target at z01 0.5
        let (px, py, z) = f.project(f.center, 64, 32);
        assert!((px - 32.0).abs() < 1e-4 && (py - 16.0).abs() < 1e-4 && (z - 0.5).abs() < 1e-6);
        // +right → larger px; +up → SMALLER py (D3D, origin top-left); +forward → smaller z01 (reversed)
        let (px, py, z) = f.project([18.0, 9.0, 17.0], 64, 32);
        assert!((px - 64.0).abs() < 1e-4, "right edge → px = w");
        assert!(py.abs() < 1e-4, "top edge → py = 0");
        assert!(z.abs() < 1e-6, "far plane → z01 = 0");
        let (_, _, z) = f.project([10.0, 5.0, -23.0], 64, 32);
        assert!((z - 1.0).abs() < 1e-6, "near plane → z01 = 1");
        assert!((f.depth_metres(0.0) - 40.0).abs() < 1e-5 && f.depth_metres(1.0).abs() < 1e-5);
    }

    #[test]
    fn unproject_inverts_project() {
        let f = fr();
        let p = [12.5, 3.25, 6.0];
        let (px, py, z) = f.project(p, 128, 128);
        let q = f.unproject(px, py, z, 128, 128);
        for k in 0..3 {
            assert!((q[k] - p[k]).abs() < 1e-4, "{:?} vs {:?}", q, p);
        }
    }

    #[test]
    fn manifest_round_trips_through_json() {
        let mut d = PassDump { root: "/tmp".into(), manifest: Manifest { producer: "t".into(), map: "m".into(), baked_map: None, quality: 3, daytime_word: Some(7), mood: "BlueBay/Day".into(), atlas: Atlas { w: 2048, h: 2048, ss: 3, stored_w: 1024, stored_h: 1024 }, sun_dir: [0.0, 1.0, 0.0], sun_rgb: [1.0; 3], sweeps: vec![], layout: vec![], conventions: serde_json::Map::new(), passes: vec![] }, dirs: Some(vec![0, 3]), bytes_written: 0 };
        let mut e = entry("peel_depth", "peel_depth/s0/d000/l00.bin".into(), "peel");
        e.sweep = Some(0);
        e.direction = Some(0);
        e.layer = Some(0);
        e.frustum = Some(fr());
        e.format = "R32_FLOAT".into();
        d.manifest.passes.push(e);
        assert!(d.wants_dir(3) && !d.wants_dir(1));
        let s = serde_json::to_string(&d.manifest).unwrap();
        let back: Manifest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.passes.len(), 1);
        assert_eq!(back.passes[0].frustum.as_ref().unwrap().half, [8.0, 4.0, 20.0]);
        assert_eq!(back.passes[0].layer, Some(0));
    }
}
