//! `lmtool passdiff GAME_DIR OURS_DIR [--pass P] [--tol T] [--floor F] [--heat DIR] [--all-heat]
//! [--game-map MAP] [--stride S] [--report FILE.md]` — the per-pass differential between the game's
//! captured render targets and the port's `--dump-passes` intermediates, in pipeline order. Both
//! sides are the MANIFEST.json layout of `passdump.rs`.
//!
//! Per pass: texels compared, max |Δ|, mean |Δ|, RMSE, mean signed Δ (a bias), the percentage of texels
//! within the tolerance (`|a − b| ≤ tol·max(|a|, |b|) + floor`), and the first pass in pipeline order
//! that falls under the pass threshold. The game's TEXEL CONVENTIONS are explicit, NAMED transforms
//! applied before the comparison, each reported on its own line when it is what makes the two sides
//! agree — so a mismatch of convention (mirrored frame, layer order, a missing dome layer, the
//! one-texel inset, reversed depth, the R11G11B10 rounding rule, the ss resolve, a chart size) is
//! reported apart from a mismatch of LIGHT:
//!
//! * `frustum_remap`: a peel-space buffer rendered in another frustum is resampled through the world
//!   (game pixel → world point → our pixel); depths are compared in METRES along the game's forward
//!   axis (`depth_to_metres`), never in z01 units, so the frustum's depth range drops out.
//! * `mirror_x` / `mirror_y` / `transpose`: the orientation of ours that fits the game's peel target.
//! * `layer_order`: the game's k = 0 nearest → reversed; `strip_dome`: the game has no dome layer.
//! * `quantise_r11g11b10_rtne` / `_rtz` / `quantise_f16`: the storage rounding that explains a residual.
//! * `ss_resolve` / `box_down N`: a supersampled target box-averaged to the other side's resolution.
//! * `chart_cut`: the game's atlas cut into charts by its layout (or the baked map's mapping).
//! * `direction_permute`: the game's direction list matched to ours by nearest vector.

use crate::gpufmt::{Quant, Rounding};
use crate::passdump::{ChartRect, Entry, Frustum, Manifest};
use std::collections::HashMap;

/// Read a MANIFEST.json (ours or the game's — tolerant of missing fields).
pub fn read_manifest(txt: &str) -> Result<Manifest, String> {
    serde_json::from_str::<Manifest>(txt).map_err(|e| format!("MANIFEST.json: {e}"))
}

/// The per-direction peel frustums of a sweep, indexed by direction (from the `peel_depth` entries,
/// else `peel_color`); directions without an entry are filled from the nearest lower direction so
/// the list is dense up to the last captured direction.
pub fn peel_frustums(m: &Manifest, sweep: u32) -> Vec<Frustum> {
    let mut by_dir: HashMap<u32, Frustum> = HashMap::new();
    for e in &m.passes {
        if (e.pass == "peel_depth" || e.pass == "peel_color") && e.sweep.unwrap_or(0) == sweep {
            if let (Some(d), Some(f)) = (e.direction, &e.frustum) {
                by_dir.entry(d).or_insert_with(|| f.clone());
            }
        }
    }
    let Some(&max) = by_dir.keys().max() else { return Vec::new() };
    let mut out = Vec::with_capacity(max as usize + 1);
    for d in 0..=max {
        // the captured direction nearest in index (ties → the lower)
        let nearest = by_dir.keys().min_by_key(|k| ((**k as i64 - d as i64).abs(), **k)).copied().unwrap();
        out.push(by_dir[&nearest].clone());
    }
    out
}

/// A decoded buffer: `channels` f32 per pixel, row-major, top row first.
#[derive(Clone, Debug)]
pub struct Buf {
    pub w: u32,
    pub h: u32,
    pub channels: u32,
    pub data: Vec<f32>,
}

impl Buf {
    pub fn new(w: u32, h: u32, channels: u32) -> Buf {
        Buf { w, h, channels, data: vec![0.0; (w * h * channels) as usize] }
    }
    #[inline]
    pub fn get(&self, x: u32, y: u32, c: u32) -> f32 {
        self.data[((y * self.w + x) * self.channels + c) as usize]
    }
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, c: u32, v: f32) {
        self.data[((y * self.w + x) * self.channels + c) as usize] = v;
    }
    /// Crop a pixel rectangle (clamped to the buffer).
    pub fn crop(&self, x0: i64, y0: i64, w: u32, h: u32) -> Buf {
        let mut out = Buf::new(w, h, self.channels);
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = (x0 + x as i64, y0 + y as i64);
                if sx >= 0 && sy >= 0 && sx < self.w as i64 && sy < self.h as i64 {
                    for c in 0..self.channels {
                        out.set(x, y, c, self.get(sx as u32, sy as u32, c));
                    }
                }
            }
        }
        out
    }
    /// Box-average by an integer factor per axis (the ss resolve: the mean of the non-zero sub-samples
    /// when `weighted`, else the plain mean).
    pub fn box_down(&self, fx: u32, fy: u32, weighted: bool) -> Buf {
        let (w, h) = (self.w / fx.max(1), self.h / fy.max(1));
        let mut out = Buf::new(w, h, self.channels);
        for y in 0..h {
            for x in 0..w {
                for c in 0..self.channels {
                    let (mut s, mut n) = (0.0f64, 0.0f64);
                    for yy in 0..fy {
                        for xx in 0..fx {
                            let v = self.get(x * fx + xx, y * fy + yy, c);
                            let covered = !weighted || (0..self.channels).any(|k| self.get(x * fx + xx, y * fy + yy, k) != 0.0);
                            if covered {
                                s += v as f64;
                                n += 1.0;
                            }
                        }
                    }
                    out.set(x, y, c, if n > 0.0 { (s / n) as f32 } else { 0.0 });
                }
            }
        }
        out
    }
    /// Nearest-neighbour resample to another size.
    pub fn resample(&self, w: u32, h: u32) -> Buf {
        let mut out = Buf::new(w, h, self.channels);
        for y in 0..h {
            let sy = (((y as f32 + 0.5) * self.h as f32 / h as f32) as u32).min(self.h.saturating_sub(1));
            for x in 0..w {
                let sx = (((x as f32 + 0.5) * self.w as f32 / w as f32) as u32).min(self.w.saturating_sub(1));
                for c in 0..self.channels {
                    out.set(x, y, c, self.get(sx, sy, c));
                }
            }
        }
        out
    }
    pub fn mirror_x(&self) -> Buf {
        let mut out = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                for c in 0..self.channels {
                    out.set(x, y, c, self.get(self.w - 1 - x, y, c));
                }
            }
        }
        out
    }
    pub fn mirror_y(&self) -> Buf {
        let mut out = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                for c in 0..self.channels {
                    out.set(x, y, c, self.get(x, self.h - 1 - y, c));
                }
            }
        }
        out
    }
    pub fn transpose(&self) -> Buf {
        let mut out = Buf::new(self.h, self.w, self.channels);
        for y in 0..self.h {
            for x in 0..self.w {
                for c in 0..self.channels {
                    out.set(y, x, c, self.get(x, y, c));
                }
            }
        }
        out
    }
    /// Apply a per-pixel RGB quantiser (channels ≥ 3; other channels untouched).
    pub fn quantised(&self, q: Quant, r: Rounding) -> Buf {
        let mut out = self.clone();
        if self.channels >= 3 {
            for i in 0..(self.w * self.h) as usize {
                let b = i * self.channels as usize;
                let v = q.apply([self.data[b], self.data[b + 1], self.data[b + 2]], r);
                out.data[b..b + 3].copy_from_slice(&v);
            }
        } else {
            for v in out.data.iter_mut() {
                *v = q.apply([*v, 0.0, 0.0], r)[0];
            }
        }
        out
    }
}

/// The storage format of a raw dump, by its (DXGI / RenderDoc) name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fmt {
    F32(u32),
    R11G11B10,
    F16(u32),
    Unorm8(u32),
    Unorm8Srgb(u32),
    Unorm16(u32),
    D24S8,
    Unknown,
}

pub fn parse_format(name: &str) -> Fmt {
    let n = name.trim().to_ascii_uppercase();
    let n = n.strip_prefix("DXGI_FORMAT_").unwrap_or(&n).to_string();
    match n.as_str() {
        "R32_FLOAT" | "D32_FLOAT" | "R32_TYPELESS" | "D32_FLOAT_S8X24_UINT" | "R32_FLOAT_X8X24_TYPELESS" => Fmt::F32(1),
        "R32G32_FLOAT" => Fmt::F32(2),
        "R32G32B32_FLOAT" => Fmt::F32(3),
        "R32G32B32A32_FLOAT" => Fmt::F32(4),
        "R11G11B10_FLOAT" => Fmt::R11G11B10,
        "R16_FLOAT" => Fmt::F16(1),
        "R16G16_FLOAT" => Fmt::F16(2),
        "R16G16B16A16_FLOAT" => Fmt::F16(4),
        "R8_UNORM" => Fmt::Unorm8(1),
        "R8G8_UNORM" => Fmt::Unorm8(2),
        "R8G8B8_UNORM" => Fmt::Unorm8(3),
        "R8G8B8A8_UNORM" | "B8G8R8A8_UNORM" | "R8G8B8A8_TYPELESS" => Fmt::Unorm8(4),
        "R8G8B8A8_UNORM_SRGB" | "B8G8R8A8_UNORM_SRGB" => Fmt::Unorm8Srgb(4),
        "R16_UNORM" | "D16_UNORM" => Fmt::Unorm16(1),
        "R16G16B16A16_UNORM" => Fmt::Unorm16(4),
        "D24_UNORM_S8_UINT" | "R24_UNORM_X8_TYPELESS" | "R24G8_TYPELESS" => Fmt::D24S8,
        _ => Fmt::Unknown,
    }
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// Decode raw target bytes of `w`×`h` pixels with the given row pitch (0 = tight).
pub fn decode_raw(bytes: &[u8], fmt: Fmt, w: u32, h: u32, row_pitch: u32) -> Result<Buf, String> {
    let (bpp, channels) = match fmt {
        Fmt::F32(c) => (4 * c, c),
        Fmt::R11G11B10 => (4, 3),
        Fmt::F16(c) => (2 * c, c),
        Fmt::Unorm8(c) | Fmt::Unorm8Srgb(c) => (c, c),
        Fmt::Unorm16(c) => (2 * c, c),
        Fmt::D24S8 => (4, 1),
        Fmt::Unknown => return Err("unknown format".into()),
    };
    let pitch = if row_pitch == 0 { w * bpp } else { row_pitch } as usize;
    if bytes.len() < pitch * (h as usize - 1) + (w * bpp) as usize {
        return Err(format!("buffer too short: {} B for {w}×{h} × {bpp} B (pitch {pitch})", bytes.len()));
    }
    let mut out = Buf::new(w, h, channels);
    for y in 0..h as usize {
        let row = &bytes[y * pitch..];
        for x in 0..w as usize {
            let px = &row[x * bpp as usize..(x + 1) * bpp as usize];
            let o = (y * w as usize + x) * channels as usize;
            match fmt {
                Fmt::F32(c) => { for k in 0..c as usize { out.data[o + k] = f32::from_le_bytes(px[k * 4..k * 4 + 4].try_into().unwrap()); } }
                Fmt::R11G11B10 => { let v = crate::gpufmt::unpack_r11g11b10(u32::from_le_bytes(px[..4].try_into().unwrap())); out.data[o..o + 3].copy_from_slice(&v); }
                Fmt::F16(c) => { for k in 0..c as usize { out.data[o + k] = crate::gpufmt::decode_f16(u16::from_le_bytes(px[k * 2..k * 2 + 2].try_into().unwrap())); } }
                Fmt::Unorm8(c) => { for k in 0..c as usize { out.data[o + k] = px[k] as f32 / 255.0; } }
                Fmt::Unorm8Srgb(c) => { for k in 0..c as usize { out.data[o + k] = if k < 3 { srgb_to_linear(px[k] as f32 / 255.0) } else { px[k] as f32 / 255.0 }; } }
                Fmt::Unorm16(c) => { for k in 0..c as usize { out.data[o + k] = u16::from_le_bytes(px[k * 2..k * 2 + 2].try_into().unwrap()) as f32 / 65535.0; } }
                Fmt::D24S8 => { let v = u32::from_le_bytes(px[..4].try_into().unwrap()) & 0x00ff_ffff; out.data[o] = v as f32 / 16_777_215.0; }
                Fmt::Unknown => unreachable!(),
            }
        }
    }
    Ok(out)
}

/// A DDS file's first mip: (dxgi format id, width, height, row pitch or 0, payload offset).
fn parse_dds(b: &[u8]) -> Result<(u32, u32, u32, u32, usize), String> {
    if b.len() < 128 || &b[..4] != b"DDS " {
        return Err("not a DDS file".into());
    }
    let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let flags = u(8);
    let h = u(12);
    let w = u(16);
    let pitch = if flags & 0x8 != 0 { u(20) } else { 0 };
    let pf_flags = u(80);
    let fourcc = &b[84..88];
    let mut off = 128usize;
    let mut dxgi = 0u32;
    if pf_flags & 0x4 != 0 && fourcc == b"DX10" {
        if b.len() < 148 {
            return Err("truncated DX10 header".into());
        }
        dxgi = u(128);
        off = 148;
    } else if pf_flags & 0x4 != 0 {
        dxgi = match fourcc {
            b"\x74\x00\x00\x00" => 2,  // D3DFMT_A32B32G32R32F = 116
            b"\x72\x00\x00\x00" => 41, // D3DFMT_R32F = 114
            b"\x71\x00\x00\x00" => 10, // D3DFMT_A16B16G16R16F = 113
            b"\x6f\x00\x00\x00" => 54, // D3DFMT_R16F = 111
            _ => 0,
        };
    } else if pf_flags & 0x40 != 0 {
        // uncompressed RGB(A) 8-bit
        let bits = u(88);
        dxgi = if bits == 32 { 28 } else if bits == 24 { 0xffff } else if bits == 8 { 61 } else { 0 };
    }
    Ok((dxgi, w, h, pitch, off))
}

/// DXGI format id → Fmt.
pub fn dxgi_fmt(id: u32) -> Fmt {
    match id {
        2 => Fmt::F32(4),
        6 => Fmt::F32(3),
        10 => Fmt::F16(4),
        16 => Fmt::F32(2),
        26 => Fmt::R11G11B10,
        28 | 87 => Fmt::Unorm8(4),
        29 | 91 => Fmt::Unorm8Srgb(4),
        34 => Fmt::F16(2),
        39 | 40 | 41 => Fmt::F32(1),
        44 | 45 | 46 => Fmt::D24S8,
        54 => Fmt::F16(1),
        56 => Fmt::Unorm16(1),
        61 => Fmt::Unorm8(1),
        0xffff => Fmt::Unorm8(3),
        _ => Fmt::Unknown,
    }
}

/// Load one entry's buffer from its root: a raw dump by the manifest's format, or a DDS.
pub fn load_entry(root: &std::path::Path, e: &Entry) -> Result<Buf, String> {
    let p = root.join(&e.file);
    let bytes = std::fs::read(&p).map_err(|err| format!("{}: {err}", p.display()))?;
    if bytes.len() >= 4 && &bytes[..4] == b"DDS " {
        let (dxgi, w, h, pitch, off) = parse_dds(&bytes)?;
        let fmt = if !e.format.is_empty() && parse_format(&e.format) != Fmt::Unknown { parse_format(&e.format) } else { dxgi_fmt(dxgi) };
        if fmt == Fmt::Unknown {
            return Err(format!("{}: DDS format {dxgi} not supported", p.display()));
        }
        let (w, h) = if e.width > 0 && e.height > 0 { (e.width, e.height) } else { (w, h) };
        return decode_raw(&bytes[off..], fmt, w, h, pitch);
    }
    let fmt = parse_format(&e.format);
    if fmt == Fmt::Unknown {
        return Err(format!("{}: format {:?} not supported", p.display(), e.format));
    }
    if e.width == 0 || e.height == 0 {
        return Err(format!("{}: no width/height in the manifest", p.display()));
    }
    decode_raw(&bytes, fmt, e.width, e.height, e.row_pitch)
}

/// The comparison statistics of two equally shaped buffers over the compared channels.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub n: usize,
    pub max_abs: f32,
    pub mean_abs: f64,
    pub rmse: f64,
    pub mean_signed: f64,
    pub within: usize,
    /// The mean of |game| over the compared texels — the scale the errors are relative to.
    pub mean_ref: f64,
    /// Where the worst texel is (x, y, channel, game value, our value).
    pub worst: (u32, u32, u32, f32, f32),
}

impl Stats {
    pub fn pct_within(&self) -> f64 {
        if self.n == 0 { 0.0 } else { 100.0 * self.within as f64 / self.n as f64 }
    }
}

/// Compare `ours` against `game` (same w, h, channels) over the pixels where `mask` (game, ours) says
/// so; tolerance `|a − b| ≤ tol·max(|a|, |b|) + floor`.
pub fn compare(game: &Buf, ours: &Buf, channels: u32, tol: f32, floor: f32, stride: u32, mask: &dyn Fn(u32, u32) -> bool) -> Stats {
    let mut s = Stats::default();
    let (mut sum_abs, mut sum_sq, mut sum_signed, mut sum_ref) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let ch = channels.min(game.channels).min(ours.channels);
    let stride = stride.max(1);
    let mut y = 0;
    while y < game.h.min(ours.h) {
        let mut x = 0;
        while x < game.w.min(ours.w) {
            if mask(x, y) {
                for c in 0..ch {
                    let (a, b) = (game.get(x, y, c), ours.get(x, y, c));
                    if !a.is_finite() || !b.is_finite() {
                        continue;
                    }
                    let d = b - a;
                    s.n += 1;
                    sum_abs += d.abs() as f64;
                    sum_sq += (d as f64) * (d as f64);
                    sum_signed += d as f64;
                    sum_ref += a.abs() as f64;
                    if d.abs() <= tol * a.abs().max(b.abs()) + floor {
                        s.within += 1;
                    }
                    if d.abs() > s.max_abs {
                        s.max_abs = d.abs();
                        s.worst = (x, y, c, a, b);
                    }
                }
            }
            x += stride;
        }
        y += stride;
    }
    if s.n > 0 {
        s.mean_abs = sum_abs / s.n as f64;
        s.rmse = (sum_sq / s.n as f64).sqrt();
        s.mean_signed = sum_signed / s.n as f64;
        s.mean_ref = sum_ref / s.n as f64;
    }
    s
}

/// The pipeline order of the passes.
pub const PIPELINE: &[&str] = &["lm_pos", "lm_nrm", "mdiffuse", "sun_shadow", "ilightinput", "peel_depth", "peel_color", "ilightdir", "lightsum", "lightsum_resolved", "probe_skyvis", "probe_ilightdir", "probe_isvalid", "hbasis0", "hbasis1", "hbasis2", "hbasis3", "final_hdr", "final_atlas"];

pub fn pipeline_rank(pass: &str) -> usize {
    PIPELINE.iter().position(|p| *p == pass).unwrap_or(PIPELINE.len())
}

/// One compared buffer pair's report row.
#[derive(Clone, Debug)]
pub struct Row {
    pub pass: String,
    pub sweep: Option<u32>,
    pub direction: Option<u32>,
    pub layer: Option<u32>,
    pub chart: Option<u32>,
    pub stats: Stats,
    /// The named transforms applied to make the two sides comparable, and convention findings.
    pub transforms: Vec<String>,
    pub note: String,
    /// The two buffers as compared (for the heat dump), when kept.
    pub pair: Option<(Buf, Buf, u32)>,
}

impl Row {
    pub fn key(&self) -> String {
        let mut k = self.pass.clone();
        if let Some(s) = self.sweep { k += &format!(" s{s}"); }
        if let Some(d) = self.direction { k += &format!(" d{d:03}"); }
        if let Some(l) = self.layer { k += &format!(" l{l:02}"); }
        if let Some(c) = self.chart { k += &format!(" obj{c}"); }
        k
    }
}

/// The chart rectangles of a side: from its manifest's `layout`, else from a baked map's mapping.
pub fn chart_rects(m: &Manifest, map: Option<&str>) -> Vec<ChartRect> {
    if !m.layout.is_empty() {
        return m.layout.clone();
    }
    let Some(path) = map else { return Vec::new() };
    let Ok(lm) = crate::mapio::load(path) else { eprintln!("passdiff: {path}: no lightmap chunk"); return Vec::new() };
    let Some(d) = lm.chunk.data.as_ref() else { return Vec::new() };
    let Some(mp) = d.cache.mapping() else { return Vec::new() };
    (0..mp.count as usize)
        .map(|i| {
            let (x, y) = mp.pos[i];
            let (w, h) = mp.size[i];
            ChartRect { obj: mp.binds[i].obj_group_idx / 4, item: 0, sub: mp.binds[i].obj_idx, x: x as i32, y: y as i32, w: w as i32, h: h as i32, chart_w: (w as u32) / 2, chart_h: (h as u32) / 2 }
        })
        .collect()
}

/// Cut a chart out of an atlas-space buffer: the chart's stored footprint is texels `(x+1)/2 …` of
/// width `w/2`; a buffer of width `W` over the 1024² stored atlas is at scale `W/1024`.
pub fn cut_chart(atlas: &Buf, r: &ChartRect, stored_w: u32) -> Option<(Buf, u32)> {
    if stored_w == 0 || atlas.w % stored_w != 0 {
        return None;
    }
    let sc = atlas.w / stored_w;
    let (tx0, ty0) = (((r.x + 1) / 2) as i64, ((r.y + 1) / 2) as i64);
    let (tw, th) = ((r.w / 2).max(0) as u32, (r.h / 2).max(0) as u32);
    if tw == 0 || th == 0 {
        return None;
    }
    Some((atlas.crop(tx0 * sc as i64, ty0 * sc as i64, tw * sc, th * sc), sc))
}

/// Bring two chart buffers to the same size: box-average the finer one down by an integer factor
/// (the ss resolve, weighted by coverage), else nearest-resample ours to the game's grid.
fn align_chart(game: Buf, ours: Buf, transforms: &mut Vec<String>) -> (Buf, Buf) {
    if game.w == ours.w && game.h == ours.h {
        return (game, ours);
    }
    if game.w > ours.w && game.w % ours.w == 0 && game.h % ours.h == 0 {
        let (fx, fy) = (game.w / ours.w, game.h / ours.h);
        transforms.push(format!("ss_resolve(game ÷{fx}×{fy})"));
        return (game.box_down(fx, fy, true), ours);
    }
    if ours.w > game.w && ours.w % game.w == 0 && ours.h % game.h == 0 {
        let (fx, fy) = (ours.w / game.w, ours.h / game.h);
        transforms.push(format!("ss_resolve(ours ÷{fx}×{fy})"));
        return (game, ours.box_down(fx, fy, true));
    }
    transforms.push(format!("chart_size_mismatch(game {}×{}, ours {}×{} → nearest)", game.w, game.h, ours.w, ours.h));
    let (w, h) = (game.w, game.h);
    (game, ours.resample(w, h))
}

/// The orientation of `ours` that best fits `game` (identity, mirror x, mirror y, both, transpose):
/// tried on a coarse stride; returns the transformed buffer and the transform's name (None = identity).
fn best_orientation(game: &Buf, ours: &Buf, channels: u32, floor: f32) -> (Buf, Option<String>) {
    let cands: Vec<(&str, Buf)> = vec![("mirror_x", ours.mirror_x()), ("mirror_y", ours.mirror_y()), ("mirror_xy", ours.mirror_x().mirror_y())];
    let all = |_x: u32, _y: u32| true;
    let base = compare(game, ours, channels, 0.02, floor, 8, &all);
    let mut best: Option<(String, Buf, f64)> = None;
    for (name, b) in cands {
        let s = compare(game, &b, channels, 0.02, floor, 8, &all);
        if s.n > 0 && s.rmse < base.rmse * 0.5 && best.as_ref().map(|x| s.rmse < x.2).unwrap_or(true) {
            best = Some((name.to_string(), b, s.rmse));
        }
    }
    if game.w == game.h {
        let t = ours.transpose();
        let s = compare(game, &t, channels, 0.02, floor, 8, &all);
        if s.n > 0 && s.rmse < base.rmse * 0.5 && best.as_ref().map(|x| s.rmse < x.2).unwrap_or(true) {
            best = Some(("transpose".into(), t, s.rmse));
        }
    }
    match best {
        Some((n, b, _)) => (b, Some(n)),
        None => (ours.clone(), None),
    }
}

/// Resample a peel-space buffer of ours into the game's pixel grid through the world: for every
/// game pixel centre, the world point on the game's near plane → our pixel (nearest). Depth channels
/// (channels == 1 and `depth`) are converted to metres along the game's forward axis on both sides.
/// The third buffer marks (1.0) the game pixels to compare: inside our frame, and not a clear
/// (z01 = 0) on BOTH sides — a clear against a surface stays in as the divergence it is.
fn remap_peel(game: &Buf, gf: &Frustum, ours: &Buf, of: &Frustum, depth: bool) -> (Buf, Buf, Buf) {
    let mut o2 = Buf::new(game.w, game.h, ours.channels);
    let mut g2 = game.clone();
    let mut valid = Buf::new(game.w, game.h, 1);
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    for y in 0..game.h {
        for x in 0..game.w {
            let pw = gf.unproject(x as f32 + 0.5, y as f32 + 0.5, 0.5, game.w, game.h);
            let (ox, oy, _) = of.project(pw, ours.w, ours.h);
            let (oxi, oyi) = (ox.floor() as i64, oy.floor() as i64);
            if oxi < 0 || oyi < 0 || oxi >= ours.w as i64 || oyi >= ours.h as i64 {
                if depth { g2.set(x, y, 0, f32::NAN); }
                continue;
            }
            let both_clear = (0..ours.channels).all(|c| ours.get(oxi as u32, oyi as u32, c) == 0.0) && (0..game.channels).all(|c| game.get(x, y, c) == 0.0);
            valid.set(x, y, 0, if both_clear { 0.0 } else { 1.0 });
            for c in 0..ours.channels {
                let v = ours.get(oxi as u32, oyi as u32, c);
                if depth && c == 0 {
                    // both depths → the world point's coordinate along the game's forward axis (metres)
                    let po = of.unproject(oxi as f32 + 0.5, oyi as f32 + 0.5, v, ours.w, ours.h);
                    let pg = gf.unproject(x as f32 + 0.5, y as f32 + 0.5, game.get(x, y, 0), game.w, game.h);
                    o2.set(x, y, 0, dot(po, gf.forward));
                    g2.set(x, y, 0, dot(pg, gf.forward));
                } else {
                    o2.set(x, y, c, v);
                }
            }
        }
    }
    (g2, o2, valid)
}

fn same_frustum(a: &Frustum, b: &Frustum) -> bool {
    let close = |p: [f32; 3], q: [f32; 3], tol: f32| (0..3).all(|k| (p[k] - q[k]).abs() <= tol);
    close(a.center, b.center, 1e-3 * a.half[0].max(1.0)) && close(a.half, b.half, 1e-4 * a.half[0].max(1.0)) && close(a.right, b.right, 1e-5) && close(a.up, b.up, 1e-5) && close(a.forward, b.forward, 1e-5)
}

/// Depth buffers (z01) to metres along the frustum's forward axis, same grid.
fn depth_to_metres(b: &Buf, f: &Frustum) -> Buf {
    let mut o = b.clone();
    for v in o.data.iter_mut() {
        *v = f.depth_metres(*v);
    }
    o
}

/// Options of a run.
pub struct Opts {
    pub pass: Option<String>,
    pub tol: f32,
    pub floor: f32,
    pub stride: u32,
    pub pass_threshold: f64,
    pub game_map: Option<String>,
    pub keep_pairs: bool,
    pub quiet: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { pass: None, tol: 0.02, floor: 1e-3, stride: 1, pass_threshold: 99.0, game_map: None, keep_pairs: true, quiet: false }
    }
}

fn is_depth_pass(p: &str) -> bool {
    p == "peel_depth" || p == "sun_shadow"
}

/// Match the game's direction list to ours (nearest vector) for a sweep: game index → our index.
fn direction_map(game: &Manifest, ours: &Manifest, sweep: u32) -> (HashMap<u32, u32>, Option<String>) {
    let gs = game.sweeps.iter().find(|s| s.sweep == sweep);
    let os = ours.sweeps.iter().find(|s| s.sweep == sweep);
    let mut map = HashMap::new();
    let (Some(gs), Some(os)) = (gs, os) else { return (map, None) };
    if gs.dirs.is_empty() || os.dirs.is_empty() {
        return (map, None);
    }
    let mut permuted = 0usize;
    let mut worst_deg = 0.0f32;
    for (gi, gd) in gs.dirs.iter().enumerate() {
        let mut best = (0usize, -2.0f32);
        for (oi, od) in os.dirs.iter().enumerate() {
            let c = gd[0] * od[0] + gd[1] * od[1] + gd[2] * od[2];
            if c > best.1 {
                best = (oi, c);
            }
        }
        if best.0 != gi {
            permuted += 1;
        }
        worst_deg = worst_deg.max(best.1.clamp(-1.0, 1.0).acos().to_degrees());
        map.insert(gi as u32, best.0 as u32);
    }
    let note = if permuted > 0 { Some(format!("direction_permute(sweep {sweep}: {permuted} of {} directions re-matched by nearest vector, worst {worst_deg:.2}°)", gs.dirs.len())) } else if worst_deg > 0.05 { Some(format!("direction_set(sweep {sweep}: same order, worst angle {worst_deg:.2}°)")) } else { None };
    (map, note)
}

/// Run the differential. Returns the rows in pipeline order and the convention findings.
pub fn run(game_root: &std::path::Path, ours_root: &std::path::Path, opts: &Opts) -> Result<(Vec<Row>, Vec<String>), String> {
    let game = read_manifest(&std::fs::read_to_string(game_root.join("MANIFEST.json")).map_err(|e| format!("{}: {e}", game_root.join("MANIFEST.json").display()))?)?;
    let ours = read_manifest(&std::fs::read_to_string(ours_root.join("MANIFEST.json")).map_err(|e| format!("{}: {e}", ours_root.join("MANIFEST.json").display()))?)?;
    let mut findings: Vec<String> = Vec::new();
    // the game's chart rects (its layout, or the baked map's mapping); ours from our layout
    let game_map_path: Option<String> = opts.game_map.clone().or_else(|| game.baked_map.clone()).or(if game.map.is_empty() { None } else { Some(game.map.clone()) });
    let game_rects = chart_rects(&game, game_map_path.as_deref());
    if !game.layout.is_empty() { findings.push(format!("layout: the game's chart rects from its manifest ({} charts)", game.layout.len())); } else if let Some(p) = &game_map_path { findings.push(format!("layout: the game's chart rects from the mapping of {p} ({} charts)", game_rects.len())); } else { findings.push("layout: NO chart rects for the game side (no `layout`, no `baked_map`, no --game-map) — atlas-space passes cannot be cut".into()); }
    let game_rect_of: HashMap<u32, ChartRect> = game_rects.iter().map(|r| (r.obj, r.clone())).collect();
    let our_rect_of: HashMap<u32, ChartRect> = ours.layout.iter().map(|r| (r.obj, r.clone())).collect();
    // layout comparison (a convention pass of its own)
    let mut layout_rows: Vec<String> = Vec::new();
    for (obj, orr) in &our_rect_of {
        match game_rect_of.get(obj) {
            Some(gr) => { if gr.w != orr.w || gr.h != orr.h { layout_rows.push(format!("obj {obj}: size game {}×{} vs ours {}×{} (layout units)", gr.w, gr.h, orr.w, orr.h)); } if gr.x != orr.x || gr.y != orr.y { layout_rows.push(format!("obj {obj}: position game ({}, {}) vs ours ({}, {})", gr.x, gr.y, orr.x, orr.y)); } }
            None if !game_rect_of.is_empty() => layout_rows.push(format!("obj {obj}: not in the game's layout")),
            None => {}
        }
    }
    if !layout_rows.is_empty() {
        findings.push(format!("layout: {} differences (the packer / chart sizes — compared per chart by object id, resampled where the sizes differ): {}", layout_rows.len(), layout_rows.iter().take(6).cloned().collect::<Vec<_>>().join("; ")));
    }
    // direction sets per sweep
    let sweeps: Vec<u32> = { let mut v: Vec<u32> = ours.sweeps.iter().map(|s| s.sweep).chain(game.sweeps.iter().map(|s| s.sweep)).collect(); v.sort(); v.dedup(); v };
    let mut dir_maps: HashMap<u32, HashMap<u32, u32>> = HashMap::new();
    for &sw in &sweeps {
        let (m, note) = direction_map(&game, &ours, sw);
        if let Some(n) = note { findings.push(n); }
        dir_maps.insert(sw, m);
    }
    // layer order / dome layer conventions from the game's manifest
    let game_near_first = game.conventions.get("layer_order").and_then(|v| v.as_str()).map(|s| s.to_ascii_lowercase().contains("near-to-far") || s.to_ascii_lowercase().starts_with("nearest")).unwrap_or(false);
    let game_has_dome = game.conventions.get("layer0").and_then(|v| v.as_str()).map(|s| s.to_ascii_lowercase().contains("dome") || s.to_ascii_lowercase().contains("sky")).unwrap_or(true);
    let ours_has_dome = ours.conventions.get("layer0").and_then(|v| v.as_str()).map(|s| s.to_ascii_lowercase().contains("dome")).unwrap_or(false);
    if game_near_first { findings.push("layer_order: the game's k = 0 is the NEAREST layer — its layers are re-indexed far-to-near before the comparison".into()); }
    // index the game's entries by (pass, sweep, our-direction, layer, chart)
    let mut game_idx: HashMap<(String, Option<u32>, Option<u32>, Option<u32>, Option<u32>), Vec<usize>> = HashMap::new();
    let game_layer_count: HashMap<(Option<u32>, Option<u32>), u32> = {
        let mut m: HashMap<(Option<u32>, Option<u32>), u32> = HashMap::new();
        for e in game.passes.iter().filter(|e| e.pass == "peel_depth") { let k = (e.sweep, e.direction); let c = m.entry(k).or_insert(0); *c = (*c).max(e.layer.unwrap_or(0) + 1); }
        m
    };
    for (i, e) in game.passes.iter().enumerate() {
        let sweep = e.sweep;
        let dir = e.direction.map(|d| dir_maps.get(&sweep.unwrap_or(0)).and_then(|m| m.get(&d).copied()).unwrap_or(d));
        let mut layer = e.layer;
        if let (Some(l), true) = (layer, game_near_first) {
            let n = game_layer_count.get(&(e.sweep, e.direction)).copied().unwrap_or(l + 1);
            layer = Some(n - 1 - l);
        }
        if let Some(l) = layer {
            // our layer 0 is the synthetic dome: the game's real layer k ↔ our k + 1 when the game has none
            if ours_has_dome && !game_has_dome { layer = Some(l + 1); }
        }
        game_idx.entry((e.pass.clone(), sweep, dir, layer, e.chart.as_ref().map(|c| c.obj))).or_default().push(i);
    }
    if ours_has_dome && !game_has_dome { findings.push("strip_dome: the game has no dome layer — our synthetic layer 0 is skipped, our layer k + 1 ↔ the game's layer k".into()); }
    // walk OUR entries in pipeline order
    let mut order: Vec<usize> = (0..ours.passes.len()).collect();
    order.sort_by_key(|&i| { let e = &ours.passes[i]; (pipeline_rank(&e.pass), e.sweep.unwrap_or(0), e.direction.unwrap_or(0), e.layer.unwrap_or(0), e.chart.as_ref().map(|c| c.obj).unwrap_or(0)) });
    let mut rows: Vec<Row> = Vec::new();
    let mut compared_passes: std::collections::BTreeSet<String> = Default::default();
    let mut missing: HashMap<String, usize> = HashMap::new();
    for i in order {
        let oe = &ours.passes[i];
        if let Some(p) = &opts.pass { if &oe.pass != p { continue; } }
        if pipeline_rank(&oe.pass) == PIPELINE.len() { continue; }
        let obj = oe.chart.as_ref().map(|c| c.obj);
        // the game's matching entry: same chart, or an atlas-space one to cut; our `final_hdr` (E) also
        // matches the game's `hbasis0` (C0 = √(2π)·E for a flat normal) through a named scale
        let key_chart = (oe.pass.clone(), oe.sweep, oe.direction, oe.layer, obj);
        let key_atlas = (oe.pass.clone(), oe.sweep, oe.direction, oe.layer, None);
        let mut pre_scale: Option<(f32, &str)> = None;
        let mut ge_i = game_idx.get(&key_chart).and_then(|v| v.first().copied()).or_else(|| if obj.is_some() { game_idx.get(&key_atlas).and_then(|v| v.first().copied()) } else { None });
        if ge_i.is_none() && oe.pass == "final_hdr" {
            ge_i = game_idx.get(&("hbasis0".to_string(), None, None, None, None)).and_then(|v| v.first().copied());
            if ge_i.is_some() { pre_scale = Some((0.398_942_28, "hbasis_c0_to_irradiance(game C0 × 1/√(2π))")); }
        }
        let Some(ge_i) = ge_i else { *missing.entry(oe.pass.clone()).or_insert(0) += 1; continue };
        let ge = &game.passes[ge_i];
        let mut transforms: Vec<String> = Vec::new();
        let ob = match load_entry(ours_root, oe) { Ok(b) => b, Err(e) => { eprintln!("passdiff: ours {}: {e}", oe.file); continue; } };
        let mut gb = match load_entry(game_root, ge) { Ok(b) => b, Err(e) => { eprintln!("passdiff: game {}: {e}", ge.file); continue; } };
        if let Some((s, name)) = pre_scale { for v in gb.data.iter_mut() { *v *= s; } transforms.push(name.into()); }
        let depth = is_depth_pass(&oe.pass);
        let channels = if depth { 1 } else { ob.channels.min(gb.channels).min(3) };
        let floor = if depth { opts.floor.max(1e-3) } else { opts.floor };
        let (mut g, mut o): (Buf, Buf);
        let mut remap_valid: Option<Buf> = None;
        if oe.space == "peel" || ge.space == "peel" {
            // peel space: the same frustum → pixel to pixel; else remap through the world
            match (&ge.frustum, &oe.frustum) {
                (Some(gf), Some(of)) if !same_frustum(gf, of) || gb.w != ob.w || gb.h != ob.h => {
                    transforms.push(format!("frustum_remap(game centre {:?} half {:?} {}×{} ← ours centre {:?} half {:?} {}×{})", gf.center, gf.half, gb.w, gb.h, of.center, of.half, ob.w, ob.h));
                    if depth { transforms.push("depth_to_metres(along the game's forward)".into()); }
                    let (g2, o2, v) = remap_peel(&gb, gf, &ob, of, depth);
                    g = g2; o = o2; remap_valid = Some(v);
                }
                (Some(gf), Some(_)) if depth => {
                    transforms.push("depth_to_metres".into());
                    g = depth_to_metres(&gb, gf); o = depth_to_metres(&ob, gf);
                }
                (None, _) | (_, None) if gb.w != ob.w || gb.h != ob.h => {
                    transforms.push(format!("resample(no frustum: ours {}×{} → game {}×{})", ob.w, ob.h, gb.w, gb.h));
                    g = gb.clone(); o = ob.resample(gb.w, gb.h);
                }
                _ => { g = gb.clone(); o = ob.clone(); }
            }
            // orientation
            let (o3, name) = best_orientation(&g, &o, channels, floor);
            if let Some(n) = name { transforms.push(format!("{n}(the game's target is ours mirrored/transposed)")); o = o3; }
        } else {
            // chart space: cut the game's atlas by the chart rect when the game entry is atlas-wide
            if ge.chart.is_none() {
                let Some(obj) = obj else { continue };
                let Some(r) = game_rect_of.get(&obj) else { *missing.entry(format!("{} (no game rect for obj {obj})", oe.pass)).or_insert(0) += 1; continue };
                let stored_w = if game.atlas.stored_w > 0 { game.atlas.stored_w } else { 1024 };
                match cut_chart(&gb, r, stored_w) {
                    Some((cut, sc)) => { transforms.push(format!("chart_cut(obj {obj}: rect ({}, {}) {}×{} at scale {sc})", r.x, r.y, r.w, r.h)); g = cut; }
                    None => { eprintln!("passdiff: {}: cannot cut obj {obj} from a {}×{} buffer", oe.pass, gb.w, gb.h); continue; }
                }
            } else { g = gb.clone(); }
            o = ob.clone();
            let (g2, o2) = align_chart(g, o, &mut transforms);
            g = g2; o = o2;
        }
        // masks: compare where either side is non-zero (uncovered texels / clears are skipped); a depth
        // buffer's clear is z01 = 0 BEFORE the metre conversion, so the mask is taken on the raw buffers
        let raw_mask: Option<(Buf, Buf)> = if depth && remap_valid.is_none() { Some((gb.clone(), ob.clone())) } else { None };
        let (gc, oc) = (g.clone(), o.clone());
        let mask = move |x: u32, y: u32| -> bool {
            if let Some(v) = &remap_valid {
                return v.get(x, y, 0) != 0.0;
            }
            match &raw_mask {
                Some((rg, ro)) if rg.w == gc.w && rg.h == gc.h && ro.w == oc.w && ro.h == oc.h => rg.get(x, y, 0) != 0.0 || ro.get(x, y, 0) != 0.0,
                _ => (0..channels).any(|c| gc.get(x, y, c) != 0.0 || oc.get(x, y, c) != 0.0),
            }
        };
        let mut stats = compare(&g, &o, channels, opts.tol, floor, opts.stride, &mask);
        // quantisation conventions for colour passes: does a storage rounding explain the residual?
        // (only when the residual is small — a storage rounding is a few percent at most)
        if !depth && channels >= 3 && stats.n > 0 && stats.pct_within() < 99.99 && stats.mean_abs <= 0.05 * stats.mean_ref.max(1e-6) {
            let cands = [("quantise_r11g11b10_rtne", Quant::R11G11B10, Rounding::NearestEven), ("quantise_r11g11b10_rtz", Quant::R11G11B10, Rounding::Truncate), ("quantise_f16_rtne", Quant::F16, Rounding::NearestEven)];
            let mut best: Option<(&str, Stats, Buf)> = None;
            for (name, q, r) in cands {
                let oq = o.quantised(q, r);
                let s = compare(&g, &oq, channels, opts.tol, floor, opts.stride.max(2), &mask);
                if s.n > 0 && s.rmse < stats.rmse * 0.7 && best.as_ref().map(|b| s.rmse < b.1.rmse).unwrap_or(true) {
                    best = Some((name, s, oq));
                }
            }
            if let Some((name, _s, oq)) = best {
                let s_full = compare(&g, &oq, channels, opts.tol, floor, opts.stride, &mask);
                transforms.push(format!("{name}(ours re-quantised: RMSE {:.4} → {:.4})", stats.rmse, s_full.rmse));
                stats = s_full;
                o = oq;
            }
        }
        // the bias line (a systematic offset is a convention smell: depth bias, a scale, the sky ×2)
        let note = if stats.n > 0 && stats.mean_ref > 0.0 && stats.mean_signed.abs() > 0.25 * stats.mean_abs && stats.mean_abs > opts.floor as f64 { format!("systematic: mean Δ {:+.4} ({:+.1} % of the game's mean {:.4})", stats.mean_signed, 100.0 * stats.mean_signed / stats.mean_ref, stats.mean_ref) } else { String::new() };
        compared_passes.insert(oe.pass.clone());
        rows.push(Row { pass: oe.pass.clone(), sweep: oe.sweep, direction: oe.direction, layer: oe.layer, chart: obj, stats, transforms, note, pair: if opts.keep_pairs { Some((g, o, channels)) } else { None } });
    }
    for (p, n) in &missing {
        findings.push(format!("not compared: {n} of our `{p}` buffers have no game entry"));
    }
    Ok((rows, findings))
}

/// Per-pass aggregate over rows.
#[derive(Clone, Debug, Default)]
pub struct PassSummary {
    pub pass: String,
    pub rows: usize,
    pub n: usize,
    pub max_abs: f32,
    pub mean_abs: f64,
    pub rmse: f64,
    pub within: usize,
    pub mean_signed: f64,
    pub mean_ref: f64,
    pub transforms: std::collections::BTreeSet<String>,
}

impl PassSummary {
    pub fn pct_within(&self) -> f64 {
        if self.n == 0 { 0.0 } else { 100.0 * self.within as f64 / self.n as f64 }
    }
}

pub fn summarise(rows: &[Row]) -> Vec<PassSummary> {
    let mut by: std::collections::BTreeMap<usize, PassSummary> = Default::default();
    for r in rows {
        let s = by.entry(pipeline_rank(&r.pass)).or_insert_with(|| PassSummary { pass: r.pass.clone(), ..Default::default() });
        s.rows += 1;
        let n = r.stats.n as f64;
        s.max_abs = s.max_abs.max(r.stats.max_abs);
        s.mean_abs = (s.mean_abs * s.n as f64 + r.stats.mean_abs * n) / (s.n as f64 + n).max(1.0);
        s.rmse = ((s.rmse * s.rmse * s.n as f64 + r.stats.rmse * r.stats.rmse * n) / (s.n as f64 + n).max(1.0)).sqrt();
        s.mean_signed = (s.mean_signed * s.n as f64 + r.stats.mean_signed * n) / (s.n as f64 + n).max(1.0);
        s.mean_ref = (s.mean_ref * s.n as f64 + r.stats.mean_ref * n) / (s.n as f64 + n).max(1.0);
        s.n += r.stats.n;
        s.within += r.stats.within;
        for t in &r.transforms {
            // keep the transform's name (before the parenthesis) so the set stays small
            s.transforms.insert(t.split('(').next().unwrap_or(t).to_string());
        }
    }
    by.into_values().collect()
}

/// The markdown report: the pass table, the convention findings, the first divergent pass.
pub fn report(rows: &[Row], findings: &[String], threshold: f64, tol: f32) -> String {
    let mut out = String::new();
    out += &format!("| pass | buffers | texels | max abs | mean abs | RMSE | mean Δ (ours − game) | within ±{:.0} % | conventions |\n|---|---|---|---|---|---|---|---|---|\n", tol * 100.0);
    let sums = summarise(rows);
    let mut first: Option<&PassSummary> = None;
    for s in &sums {
        out += &format!("| {} | {} | {} | {:.4} | {:.4} | {:.4} | {:+.4} ({:+.1} %) | {:.2} % | {} |\n", s.pass, s.rows, s.n, s.max_abs, s.mean_abs, s.rmse, s.mean_signed, if s.mean_ref > 0.0 { 100.0 * s.mean_signed / s.mean_ref } else { 0.0 }, s.pct_within(), s.transforms.iter().cloned().collect::<Vec<_>>().join(", "));
        if first.is_none() && s.n > 0 && s.pct_within() < threshold {
            first = Some(s);
        }
    }
    if !findings.is_empty() {
        out += "\nConventions:\n";
        for f in findings {
            out += &format!("* {f}\n");
        }
    }
    match first {
        Some(s) => {
            out += &format!("\n**FIRST DIVERGENT PASS: `{}`** — {:.2} % of {} texels within ±{:.0} % (threshold {threshold} %), RMSE {:.4}, mean Δ {:+.4} ({:+.1} % of the game's mean {:.4}), max |Δ| {:.4}.\n", s.pass, s.pct_within(), s.n, tol * 100.0, s.rmse, s.mean_signed, if s.mean_ref > 0.0 { 100.0 * s.mean_signed / s.mean_ref } else { 0.0 }, s.mean_ref, s.max_abs);
            // the worst buffers of that pass
            let mut worst: Vec<&Row> = rows.iter().filter(|r| r.pass == s.pass).collect();
            worst.sort_by(|a, b| b.stats.rmse.partial_cmp(&a.stats.rmse).unwrap_or(std::cmp::Ordering::Equal));
            for r in worst.iter().take(5) {
                out += &format!("  * {}: {:.2} % within, RMSE {:.4}, worst texel ({}, {}) ch {} game {:.4} ours {:.4}{}{}\n", r.key(), r.stats.pct_within(), r.stats.rmse, r.stats.worst.0, r.stats.worst.1, r.stats.worst.2, r.stats.worst.3, r.stats.worst.4, if r.note.is_empty() { String::new() } else { format!("; {}", r.note) }, if r.transforms.is_empty() { String::new() } else { format!("; transforms: {}", r.transforms.join(", ")) });
            }
        }
        None if !rows.is_empty() => out += &format!("\nNo divergent pass: every compared pass has ≥ {threshold} % of its texels within ±{:.0} %.\n", tol * 100.0),
        None => out += "\nNothing compared.\n",
    }
    out
}

/// Heat maps of a compared pair: game | ours | |Δ| as one PNG (tonemapped by the pair's max).
pub fn heat_png(path: &str, g: &Buf, o: &Buf, channels: u32, tol: f32) -> std::io::Result<()> {
    let (w, h) = (g.w.min(o.w), g.h.min(o.h));
    let mut max = 1e-6f32;
    for y in 0..h { for x in 0..w { for c in 0..channels { let (a, b) = (g.get(x, y, c), o.get(x, y, c)); if a.is_finite() { max = max.max(a.abs()); } if b.is_finite() { max = max.max(b.abs()); } } } }
    let tone = |v: f32| -> u8 { ((v.max(0.0) / max).powf(1.0 / 2.2) * 255.0).round().clamp(0.0, 255.0) as u8 };
    let mut px = vec![0u8; (w * 3 * h * 3) as usize];
    let stride = (w * 3 * 3) as usize;
    for y in 0..h {
        for x in 0..w {
            let (mut gc, mut oc) = ([0u8; 3], [0u8; 3]);
            let mut dmax = 0.0f32;
            let mut rel = 0.0f32;
            for c in 0..3.min(channels) {
                let (a, b) = (g.get(x, y, c), o.get(x, y, c));
                gc[c as usize] = tone(a);
                oc[c as usize] = tone(b);
                if a.is_finite() && b.is_finite() {
                    dmax = dmax.max((a - b).abs());
                    rel = rel.max((a - b).abs() / (a.abs().max(b.abs()) + 1e-3));
                }
            }
            if channels == 1 { gc = [gc[0]; 3]; oc = [oc[0]; 3]; }
            // the heat: relative error, black = 0, blue ≤ tol, green 2·tol, yellow 4·tol, red ≥ 8·tol
            let t = (rel / (8.0 * tol)).min(1.0);
            let heat: [u8; 3] = if rel <= tol { [0, 0, (64.0 + 191.0 * (rel / tol)) as u8] } else if t < 0.5 { [(255.0 * (t - 0.125) / 0.375).clamp(0.0, 255.0) as u8, 255, 0] } else { [255, (255.0 * (1.0 - (t - 0.5) / 0.5)) as u8, 0] };
            let _ = dmax;
            let o0 = y as usize * stride + x as usize * 3;
            px[o0..o0 + 3].copy_from_slice(&gc);
            let o1 = y as usize * stride + (w + x) as usize * 3;
            px[o1..o1 + 3].copy_from_slice(&oc);
            let o2 = y as usize * stride + (2 * w + x) as usize * 3;
            px[o2..o2 + 3].copy_from_slice(&heat);
        }
    }
    crate::png::write_rgb(path, w * 3, h, &px)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fr(center: [f32; 3], half: [f32; 3]) -> Frustum {
        Frustum { ortho: true, center, half, right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], forward: [0.0, -1.0, 0.0], depth: Frustum::REVERSED.into() }
    }

    #[test]
    fn r11g11b10_raw_round_trip_through_the_loader() {
        let px = [[0.5f32, 1.25, 3.0], [0.0, 0.0, 0.0]];
        let mut bytes = Vec::new();
        for p in px { bytes.extend_from_slice(&crate::gpufmt::pack_r11g11b10(p, Rounding::NearestEven).to_le_bytes()); }
        let b = decode_raw(&bytes, parse_format("DXGI_FORMAT_R11G11B10_FLOAT"), 2, 1, 0).unwrap();
        assert_eq!(b.channels, 3);
        assert_eq!(&b.data[..3], &[0.5, 1.25, 3.0]);
        assert_eq!(parse_format("r32_float"), Fmt::F32(1));
        assert_eq!(parse_format("D32_FLOAT"), Fmt::F32(1));
        assert_eq!(parse_format("R16G16B16A16_FLOAT"), Fmt::F16(4));
    }

    #[test]
    fn f16_and_d24_decoding() {
        let mut bytes = Vec::new();
        for v in [1.0f32, 0.5, -2.0, 1.0] { bytes.extend_from_slice(&crate::gpufmt::encode_f16(v, Rounding::NearestEven).to_le_bytes()); }
        let b = decode_raw(&bytes, Fmt::F16(4), 1, 1, 0).unwrap();
        assert_eq!(b.data, vec![1.0, 0.5, -2.0, 1.0]);
        let d = decode_raw(&0x00ff_ffffu32.to_le_bytes(), Fmt::D24S8, 1, 1, 0).unwrap();
        assert_eq!(d.data[0], 1.0);
    }

    #[test]
    fn dds_dx10_header_is_read() {
        // a 2×1 R32_FLOAT DDS with a DX10 header
        let mut b = vec![0u8; 148 + 8];
        b[..4].copy_from_slice(b"DDS ");
        b[4..8].copy_from_slice(&124u32.to_le_bytes());
        b[8..12].copy_from_slice(&(0x1u32 | 0x2 | 0x4 | 0x1000).to_le_bytes());
        b[12..16].copy_from_slice(&1u32.to_le_bytes());
        b[16..20].copy_from_slice(&2u32.to_le_bytes());
        b[76..80].copy_from_slice(&32u32.to_le_bytes());
        b[80..84].copy_from_slice(&0x4u32.to_le_bytes());
        b[84..88].copy_from_slice(b"DX10");
        b[128..132].copy_from_slice(&41u32.to_le_bytes()); // DXGI_FORMAT_R32_FLOAT
        b[148..152].copy_from_slice(&0.25f32.to_le_bytes());
        b[152..156].copy_from_slice(&0.75f32.to_le_bytes());
        let (dxgi, w, h, pitch, off) = parse_dds(&b).unwrap();
        assert_eq!((dxgi, w, h, pitch, off), (41, 2, 1, 0, 148));
        let buf = decode_raw(&b[off..], dxgi_fmt(dxgi), w, h, pitch).unwrap();
        assert_eq!(buf.data, vec![0.25, 0.75]);
    }

    #[test]
    fn cut_chart_uses_the_stored_footprint_at_the_buffer_scale() {
        // a 2048-wide buffer over the 1024² atlas (scale 2): chart x = 1 (layout), w = 100 → texels 1..51 → pixels 2..102
        let mut atlas = Buf::new(2048, 2048, 1);
        for y in 0..2048 { for x in 0..2048 { atlas.set(x, y, 0, (x + 10_000 * y) as f32); } }
        let r = ChartRect { obj: 4096, item: 0, sub: 0, x: 1, y: 3, w: 100, h: 60, chart_w: 50, chart_h: 30 };
        let (cut, sc) = cut_chart(&atlas, &r, 1024).unwrap();
        assert_eq!(sc, 2);
        assert_eq!((cut.w, cut.h), (100, 60));
        assert_eq!(cut.get(0, 0, 0), (2 + 10_000 * 4) as f32);
    }

    #[test]
    fn box_down_is_the_coverage_weighted_mean() {
        let mut b = Buf::new(2, 2, 3);
        for c in 0..3 { b.set(0, 0, c, 1.0); b.set(1, 0, c, 3.0); }
        // (0,1) and (1,1) uncovered (all zero)
        let w = b.box_down(2, 2, true);
        assert_eq!(w.get(0, 0, 0), 2.0, "weighted: the mean of the two covered sub-samples");
        let u = b.box_down(2, 2, false);
        assert_eq!(u.get(0, 0, 0), 1.0, "plain: the mean over all four");
    }

    #[test]
    fn compare_counts_tolerance_and_bias() {
        let mut g = Buf::new(4, 1, 1);
        let mut o = Buf::new(4, 1, 1);
        for x in 0..4 { g.set(x, 0, 0, 1.0); o.set(x, 0, 0, if x == 3 { 1.5 } else { 1.01 }); }
        let s = compare(&g, &o, 1, 0.02, 0.0, 1, &|_, _| true);
        assert_eq!(s.n, 4);
        assert_eq!(s.within, 3);
        assert!((s.max_abs - 0.5).abs() < 1e-6);
        assert_eq!(s.worst.0, 3);
        assert!(s.mean_signed > 0.0);
        assert!((s.pct_within() - 75.0).abs() < 1e-9);
    }

    #[test]
    fn mirror_transform_is_detected_as_a_convention() {
        let mut g = Buf::new(16, 16, 1);
        for y in 0..16 { for x in 0..16 { g.set(x, y, 0, x as f32 + 0.1 * y as f32); } }
        let o = g.mirror_x();
        let (fixed, name) = best_orientation(&g, &o, 1, 1e-3);
        assert_eq!(name.as_deref(), Some("mirror_x"));
        assert_eq!(fixed.data, g.data);
        let (same, none) = best_orientation(&g, &g, 1, 1e-3);
        assert!(none.is_none());
        assert_eq!(same.data, g.data);
    }

    #[test]
    fn remap_peel_compares_depths_in_metres_through_the_world() {
        // the game looks straight down over a 20 m square, 8×8 px, depth range 40 m; ours over the same
        // square at 16×16 px with another depth range: a plane at y = 3 must agree to the metre
        let gf = fr([10.0, 5.0, 10.0], [10.0, 10.0, 20.0]);
        let of = fr([10.0, 0.0, 10.0], [10.0, 10.0, 8.0]);
        let mut g = Buf::new(8, 8, 1);
        let mut o = Buf::new(16, 16, 1);
        let plane = [0.0, 3.0, 0.0];
        for y in 0..8 { for x in 0..8 { let (_, _, z) = gf.project(plane, 8, 8); g.set(x, y, 0, z); } }
        for y in 0..16 { for x in 0..16 { let (_, _, z) = of.project(plane, 16, 16); o.set(x, y, 0, z); } }
        assert!((g.get(0, 0, 0) - 0.45).abs() < 1e-6, "game z01 of y = 3 (below the centre, farther from a camera looking down): 0.5 + (−5 + 3)/40");
        assert!((o.get(0, 0, 0) - (0.5 + 3.0 / 16.0)).abs() < 1e-6);
        let (g2, o2, _valid) = remap_peel(&g, &gf, &o, &of, true);
        let s = compare(&g2, &o2, 1, 0.0, 1e-4, 1, &|_, _| true);
        assert_eq!(s.n, 64);
        assert_eq!(s.within, 64, "both sides → −3 m along the game's forward (0, −1, 0): {:?}", s);
        assert!((g2.get(0, 0, 0) + 3.0).abs() < 1e-4);
    }

    #[test]
    fn pipeline_order_and_report_name_the_first_divergent_pass() {
        assert!(pipeline_rank("sun_shadow") < pipeline_rank("peel_depth"));
        assert!(pipeline_rank("peel_depth") < pipeline_rank("lightsum"));
        let ok = Row { pass: "sun_shadow".into(), sweep: None, direction: None, layer: None, chart: None, stats: Stats { n: 100, within: 100, ..Default::default() }, transforms: vec![], note: String::new(), pair: None };
        let bad = Row { pass: "peel_color".into(), sweep: Some(0), direction: Some(3), layer: Some(1), chart: None, stats: Stats { n: 100, within: 50, rmse: 0.2, max_abs: 0.9, mean_ref: 1.0, mean_signed: -0.1, ..Default::default() }, transforms: vec!["mirror_x(...)".into()], note: String::new(), pair: None };
        let r = report(&[ok, bad], &[], 99.0, 0.02);
        assert!(r.contains("FIRST DIVERGENT PASS: `peel_color`"), "{r}");
        assert!(r.contains("| sun_shadow | 1 | 100 |"), "{r}");
        assert!(r.contains("mirror_x"), "{r}");
    }

    #[test]
    fn manifest_reader_tolerates_a_sparse_capture_manifest() {
        let txt = r#"{"passes":[{"pass":"peel_depth","sweep":0,"direction":2,"layer":0,"file":"peel_depth/s0/d002/l00.dds","format":"D32_FLOAT","width":2048,"height":2048,"space":"peel","dir":[0,1,0],"frustum":{"center":[0,0,0],"half":[1,1,1],"right":[1,0,0],"up":[0,0,1],"forward":[0,-1,0]}}],"sweeps":[{"sweep":0,"dirs":[[0,1,0],[1,0,0],[0,0,1]]}]}"#;
        let m = read_manifest(txt).unwrap();
        assert_eq!(m.passes.len(), 1);
        assert_eq!(m.passes[0].frustum.as_ref().unwrap().depth, "reversed_z01");
        assert!(m.passes[0].frustum.as_ref().unwrap().ortho);
        let fs = peel_frustums(&m, 0);
        assert_eq!(fs.len(), 3, "dense up to direction 2, filled from the nearest lower captured direction");
    }
}
