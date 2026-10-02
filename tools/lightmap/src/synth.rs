//! From-scratch synthesis of a lightmap chunk for a tiny map: our own chart
//! layout (one chart per ground slot and per item), our own atlas images, the
//! frame table / trailer / small chunks templated from a real bake of the same
//! mood. This is the authoring path; `paint` only edits a real bake in place.
//!
//! Encoding (measured in play mode, 2026-09-22): the colour atlas holds each
//! chart normalised to its own maximum, the per-chart frame-0 byte is that
//! maximum on a LINEAR scale (fb0 = 255 · max / K), and the second atlas
//! has no brightness effect (128 = neutral).

use crate::format::{CacheBlob, CacheChunk, ChunkBody, Frame, LightmapChunk, LightmapData, Mapping, ObjBind};
use crate::img::{encode_webp_lossless, Rgb};

/// What one chart should show (flat values; the baker fills real texels).
#[derive(Clone, Copy, Debug)]
pub struct ChartSpec {
    /// Frame-0 colour atlas value (already normalised to the chart scale).
    pub a: [u8; 3],
    /// Frame-0 second atlas value (128 = neutral).
    pub b: u8,
    /// Per-frame chart bytes.
    pub fb: [u8; 3],
}

impl Default for ChartSpec {
    fn default() -> Self {
        ChartSpec { a: [230, 220, 254], b: 119, fb: [148, 0, 0] }
    }
}

/// One chart ready for packing: 8-bit atlas content per pixel.
/// The stored texel for an HDR value: the game's `LmCompress_HBasis_YCbCr4` writes
/// p = sqrt(E / m) per channel and the runtime squares the decoded colour
/// (RE child, 2026-09-23, from the lightmapper shaders); `m` = the chart max the
/// frame byte encodes. A linear write shows every mid-tone at p².
pub fn encode_value(e: f32, enc_max: f32) -> u8 {
    ((e.max(0.0) / enc_max).min(1.0).sqrt() * 255.0).round().clamp(0.0, 255.0) as u8
}

/// The inverse: the HDR value a stored texel means, in units of the frame's MaxHDR.
/// The chart's frame byte is sqrt-encoded too — chartMax = (fb/255)²·MaxHDR: the same
/// open pad reads 0.60 (fb 191, MaxHDR 1.45) and 0.56 (fb 122, MaxHDR 2.98) on two
/// BlueBay test bakes this way, 0.81 vs 1.17 with a linear byte (2026-09-23).
pub fn decode_value(p: u8, fb: u8) -> f32 {
    let q = p as f32 / 255.0;
    let m = fb as f32 / 255.0;
    q * q * m * m
}

/// The frame byte for a chart whose brightest value is `max`, against the frame's MaxHDR `k`.
pub fn frame_byte(max: f32, k: f32) -> u8 {
    ((max.max(0.0) / k).min(1.0).sqrt() * 255.0).round().clamp(1.0, 255.0) as u8
}

/// The chart max a frame byte encodes (the value a 255 texel means), against `k`.
pub fn chart_max(fb: u8, k: f32) -> f32 {
    let m = fb as f32 / 255.0;
    m * m * k
}

#[derive(Clone, Debug)]
pub struct Chart {
    pub obj: u32,
    pub w: u32,
    pub h: u32,
    pub a: Vec<[u8; 3]>,
    /// Frame 1 (point lights), normalised like `a`; empty = black.
    pub a1: Vec<[u8; 3]>,
    pub b: u8,
    pub fb: [u8; 3],
    /// The bind's first word (sub-chart index | flags); 0 for a one-chart object.
    pub sub: u32,
}

impl Chart {
    pub fn flat(obj: u32, px: u32, spec: ChartSpec) -> Chart {
        Chart { obj, w: px, h: px, a: vec![spec.a; (px * px) as usize], a1: Vec::new(), b: spec.b, fb: spec.fb, sub: 0 }
    }

    /// `from_hdr` plus a frame-1 (point light) HDR chart normalised the same way (fb1 = 255·max/k).
    pub fn from_hdr2(obj: u32, w: u32, h: u32, rgb: &[[f32; 3]], rgb1: &[[f32; 3]], k: f32, b: u8) -> Chart {
        let mut c = Chart::from_hdr(obj, w, h, rgb, k, b);
        if !rgb1.is_empty() {
            let max1 = rgb1.iter().flat_map(|c| c.iter().copied()).fold(0.0f32, f32::max);
            if max1 > 1e-4 {
                let fb1 = frame_byte(max1, k);
                let enc_max = chart_max(fb1, k);
                c.a1 = rgb1.iter().map(|v| [encode_value(v[0], enc_max), encode_value(v[1], enc_max), encode_value(v[2], enc_max)]).collect();
                c.fb[1] = fb1;
            }
        }
        c
    }

    /// From HDR irradiance: normalise to the chart max, `k` = the scale the
    /// per-chart byte is relative to (fb0 = 255·max/k).
    pub fn from_hdr(obj: u32, w: u32, h: u32, rgb: &[[f32; 3]], k: f32, b: u8) -> Chart {
        let max = rgb.iter().flat_map(|c| c.iter().copied()).fold(0.0f32, f32::max).max(1e-4);
        let fb0 = frame_byte(max, k);
        // the stored max is quantised: normalise against what the byte encodes
        let enc_max = chart_max(fb0, k);
        let a: Vec<[u8; 3]> = rgb.iter().map(|c| [encode_value(c[0], enc_max), encode_value(c[1], enc_max), encode_value(c[2], enc_max)]).collect();
        Chart { obj, w, h, a, a1: Vec::new(), b, fb: [fb0, 0, 0], sub: 0 }
    }
}

pub struct Plan {
    /// Object index space size: ground slots + items.
    pub base: u32,
    pub items: u32,
    /// Chart size in PIXELS of the 1024 atlas for ground slots and items.
    pub ground_px: u32,
    pub item_px: u32,
    pub ground: ChartSpec,
    pub item_default: ChartSpec,
    pub item_spec: Vec<Option<ChartSpec>>,
    pub bbox: ([f32; 3], [f32; 3]),
}

/// A shelf packer over a W×W pixel atlas with 1-pixel gutters. Charts are
/// placed in the order given (sort by height first for a tight fit).
struct Shelf {
    w: u32,
    x: u32,
    y: u32,
    row_h: u32,
}

impl Shelf {
    fn new(w: u32) -> Self {
        Shelf { w, x: 1, y: 1, row_h: 0 }
    }
    fn place(&mut self, pw: u32, ph: u32) -> Result<(u32, u32), String> {
        if self.x + pw + 1 > self.w {
            self.x = 1;
            self.y += self.row_h + 1;
            self.row_h = 0;
        }
        if self.y + ph + 1 > self.w {
            return Err(format!("atlas full at row y={} (fill more than 1024²: lower the texel density)", self.y));
        }
        let at = (self.x, self.y);
        self.x += pw + 1;
        self.row_h = self.row_h.max(ph);
        Ok(at)
    }
}

/// Would these chart sizes shelf-pack into a `w`×`w` atlas (tallest first, 1-px gutters)?
pub fn shelf_fits(sizes: &[(u32, u32)], w: u32) -> bool {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(sizes[i].1), std::cmp::Reverse(sizes[i].0)));
    let mut shelf = Shelf::new(w);
    order.iter().all(|&i| shelf.place(sizes[i].0, sizes[i].1).is_ok())
}

pub struct Synth {
    pub chunk: LightmapChunk,
    pub charts: u32,
    /// Fraction of the 1024² atlas the charts occupy (gutters excluded).
    pub fill: f32,
    /// The 8-bit colour atlas before its WEBP encode (the differential harness's `final_atlas`).
    pub atlas8: Option<Rgb>,
    /// Where every chart landed: (obj, sub, x, y, w, h) in stored texels (1024² space).
    pub placed: Vec<(u32, u32, u32, u32, u32, u32)>,
}

/// Flat charts everywhere (the first acceptance test).
pub fn synth(plan: &Plan, template: &LightmapChunk) -> Result<Synth, String> {
    let n = (plan.base + plan.items) as usize;
    let mut charts = Vec::with_capacity(n);
    for obj in 0..n as u32 {
        let (px, spec) = if obj < plan.base {
            (plan.ground_px, plan.ground)
        } else {
            let i = (obj - plan.base) as usize;
            (plan.item_px, plan.item_spec.get(i).copied().flatten().unwrap_or(plan.item_default))
        };
        charts.push(Chart::flat(obj, px, spec));
    }
    build(charts, plan.bbox, template)
}

/// What replaces the template's probe volume: the small-atlas blob and the trailer.
#[derive(Clone)]
pub struct ProbeBlob {
    pub blob: Vec<u8>,
    pub trailer: Vec<u8>,
}

/// Pack `charts` (any order; one per object) into a 1024² atlas, encode, and
/// wrap them in a chunk templated on `template`. `probes`: our own probe
/// volume (else the template's is copied). `vp8_q`: Some(q) encodes the big
/// atlases as lossy VP8 (Nadeo's form), None as lossless VP8L.
pub fn build(charts: Vec<Chart>, bbox: ([f32; 3], [f32; 3]), template: &LightmapChunk) -> Result<Synth, String> {
    build_full(charts, bbox, template, None, None)
}

/// The frame records' mood constants (the template's are replaced when given).
#[derive(Clone, Debug)]
pub struct FrameParams {
    /// The map's DayTime word (0xffffffff = default).
    pub daytime: u32,
    pub max_hdr_mood: f32,
    /// Frame 0's MaxHDR: min(the brightest chart, the mood's MaxHDR) — the K the frame bytes are scaled by.
    pub max_hdr: f32,
    pub bounce: f32,
    pub sky: f32,
    /// The bake's per-file cache values (RE child 2 / lmtool diff on editor bakes): Σ chart area in m²
    /// (chunk 0x0602200B = (1, Σarea)), the quality index (0x0602200F = (q, 0); High = 2), the
    /// decoration name and the bake's FILETIME (0x06022015 / 0x06022013). None = keep the template's.
    pub sum_area: Option<f32>,
    pub quality: Option<u32>,
    pub decoration: Option<String>,
    /// The bake's FILETIME word (chunk 0x06022013, 100-ns ticks since 1601): None = KEEP THE TEMPLATE'S — the writer is
    /// then deterministic run to run (two bakes of the same inputs give byte-identical files; the wall clock was the
    /// only nondeterministic element, engineer 2 2026-09-25). `--bake-time now|TICKS` sets it.
    pub filetime: Option<u64>,
    /// THE EDITOR'S 0x06022017 / 0x06022018 WORDS (the Fall 2026 hang bisect, 2026-10-01): every editor bake of a block-less
    /// map writes cache chunk 0x06022018 = 0 and 0x06022017 = (0, small count); a template taken from a Nadeo source carries
    /// that source's words (0x18 = a FILETIME) and the port copied them into every lit file. `true` = write the editor's
    /// form (0x17 = (0, 0), 0x18 = 0); `false` = keep the template's.
    pub editor_cache_words: bool,
}

/// THE FRAME RECORD COUNT (V's scan of all 30 editor refs, 2026-09-26 22:50Z; RE 13 0x140229620 l.524–560): every editor bake
/// carries exactly TWO frame records — rec 0 {StoreLAmbient 1, Storage 0}, rec 1 {StoreLAmbient 0, Storage 1} — and two image
/// frames (mapping head 60 + 66·2 + 12 = 204 bytes); a Storage-2 record exists only when the frame list has one (the between-pass
/// snapshot of the two-pass lamp render — no oracle carries it). Our templates carried three (the third, Storage 2, MaxHDR 1e-5 with
/// a black frame): the writer now emits the records the frame list calls for — two, unless LMTOOL_WRITE_RECORDS=3 asks for the old
/// three. Returns the number of records kept.
pub fn frame_records_wanted() -> usize {
    std::env::var("LMTOOL_WRITE_RECORDS").ok().and_then(|v| v.parse().ok()).unwrap_or(2)
}

/// Cut a mapping head (60 constants + 66·k records + the 12-byte tail) down to `want` records; a head with fewer stays.
/// THE RECORD-0 KIND WORD FOLLOWS THE FRAME COUNT (the Fall 2026 play-load-hang bisect, 2026-10-01; 77 files): every editor and
/// Nadeo file with THREE image frames carries record kinds [3, 3, 2], every one with TWO carries [2, 3] — and the five port-lit
/// files carried [3, 3] with two frames (the 3-record template cut to two, record 0 untouched): the client's PlayMap hung on its
/// loading frame on every one of them; the same chunk with the editor's two records (A4) or record 0's kind word alone (A4a)
/// loads. So a cut to two records writes record 0's kind = 2.
pub fn cut_head_records(head: &mut Vec<u8>, want: usize) -> usize {
    let have = head.len().saturating_sub(72) / 66;
    if have <= want || head.len() != 60 + 66 * have + 12 { return have; }
    let tail = head[60 + 66 * have..].to_vec();
    head.truncate(60 + 66 * want);
    head.extend_from_slice(&tail);
    if want == 2 && head.len() >= 64 {
        head[60..64].copy_from_slice(&2u32.to_le_bytes());
    }
    want
}

/// Record 0's kind word for a head that already has `n` records (the rule above): 2 with two frames, 3 with three.
pub fn fix_record0_kind(head: &mut [u8]) -> Option<(u32, u32)> {
    let have = head.len().saturating_sub(72) / 66;
    if have == 0 || head.len() < 64 { return None; }
    let want: u32 = if have >= 3 { 3 } else { 2 };
    let had = u32::from_le_bytes(head[60..64].try_into().unwrap());
    head[60..64].copy_from_slice(&want.to_le_bytes());
    Some((had, want))
}

/// Patch the three 66-byte frame records inside a mapping head (see `CacheBlob::frame_max_hdr`).
/// The small per-bake cache chunks: 0x0602200B (1, Σarea), 0x0602200F (quality, 0), 0x06022013
/// (1, 1, FILETIME), 0x06022015 (5, hash, 3, 0x1c, Id decoration, 1, 0, DayTime, zeros).
fn patch_raw_chunk(id: u32, b: &[u8], fp: Option<&FrameParams>) -> Vec<u8> {
    let Some(fp) = fp else { return b.to_vec() };
    let mut o = b.to_vec();
    match id {
        0x0602_200B if o.len() >= 8 => {
            if let Some(a) = fp.sum_area {
                o[4..8].copy_from_slice(&a.to_le_bytes());
            }
        }
        0x0602_200F if o.len() >= 8 => {
            if let Some(q) = fp.quality {
                o[0..4].copy_from_slice(&q.to_le_bytes());
            }
        }
        0x0602_2013 if o.len() >= 16 => {
            // FILETIME (100-ns ticks since 1601-01-01) — the bake's creation time, the ONE field of a written map that
            // differed between two identical bakes (and, deflated with the trailer, re-encodes the 30 KB after it).
            // Precedence: FrameParams::filetime (`--bake-time now|TICKS`), else LMTOOL_BAKE_TIME=<unix seconds> /
            // SOURCE_DATE_EPOCH (engineer 2's pin), else THE TEMPLATE'S WORD IS KEPT — the default output is deterministic
            let pinned: Option<u64> = std::env::var("LMTOOL_BAKE_TIME").ok().or_else(|| std::env::var("SOURCE_DATE_EPOCH").ok()).and_then(|v| v.trim().parse::<u64>().ok());
            let ft: Option<u64> = fp.filetime.or_else(|| pinned.map(|secs| secs * 10_000_000 + 116_444_736_000_000_000));
            if let Some(ft) = ft {
                o[8..16].copy_from_slice(&ft.to_le_bytes());
            }
        }
        0x0602_2017 if o.len() >= 8 && fp.editor_cache_words => {
            // the editor writes (0, n) with n = 0 on most block-less maps (615 / 47569 seen once each); a Nadeo source carries
            // (40653, 1595650) — the template's words were shipped by the port until 2026-10-01
            o[..8].copy_from_slice(&[0u8; 8]);
        }
        0x0602_2018 if o.len() >= 8 && fp.editor_cache_words => {
            // a FILETIME in Nadeo sources (2024-07 in Fall's), ZERO in every editor bake of our maps
            o[..8].copy_from_slice(&[0u8; 8]);
        }
        0x0602_2015 if o.len() >= 40 => {
            // (5, u64, 3, 0x1c, Id(0x40000000, len, name), 1, 0, DayTime, 0…): rewrite the name and the time
            let name_len = u32::from_le_bytes([o[24], o[25], o[26], o[27]]) as usize;
            if o.len() >= 28 + name_len + 12 && &o[20..24] == &[0, 0, 0, 0x40] {
                let tail = o[28 + name_len..].to_vec();
                let mut n = o[..24].to_vec();
                let name = fp.decoration.clone().unwrap_or_else(|| String::from_utf8_lossy(&o[28..28 + name_len]).to_string());
                n.extend_from_slice(&(name.len() as u32).to_le_bytes());
                n.extend_from_slice(name.as_bytes());
                let mut tail = tail;
                if tail.len() >= 12 && fp.daytime != 0xffff_ffff {
                    tail[8..12].copy_from_slice(&fp.daytime.to_le_bytes());
                }
                n.extend_from_slice(&tail);
                o = n;
            }
        }
        _ => {}
    }
    o
}

/// The wall clock as a FILETIME word (for `--bake-time now`).
pub fn filetime_now() -> u64 {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() / 100).unwrap_or(0) as u64;
    unix + 116_444_736_000_000_000
}

/// THE GAME'S `TimeWriteMostRecentSolid` OF A MAP (the MK64 flat-lightmap lane, 2026-09-28; lmflat 0006, docs/formats/map-lightmap.md
/// §4.1): the cache chunk 0x06022013's FILETIME word is compared at load against the newest `CPlugSolid2Model.FileWriteTime`
/// (chunk 0x090BB000 v ≥ 4, the u64 after the PreLightGen) over the map's embedded item models — a chunk whose word differs is
/// DROPPED ("TimeWriteMostRecentSolid has changed") and the game falls back to its coarse load-time bake. Every 09-23 MK64 editor
/// bake's word EQUALS its 130 item solids' 0x01dd4b6a0b594800. Rule as read: the max FileWriteTime over the PLACED item models'
/// solids (V4 2026-09-28 20:15Z on all 31 corpus oracles: 31/31 equal to the max over the models the map's item list places; in
/// 16 the embedded zip carries MORE item files than the map places and the word follows the placed set) — a static object's own
/// solid, or a prefab's static-object entities'. (None, …) = no placed model carries one (the template's word is kept then).
/// Returns (time, solids counted, placed model files parsed).
pub fn most_recent_solid(mf: &tmmaps::map::MapFile) -> Result<(Option<u64>, usize, usize), String> {
    let files = mapgeom::embedded::files(mf)?;
    let placed: std::collections::HashSet<String> = mf.items.iter().map(|it| it.model.rsplit(['/', '\\']).next().unwrap_or(&it.model).to_ascii_lowercase()).collect();
    let (mut best, mut n_solids, mut n_files) = (None::<u64>, 0usize, 0usize);
    for (name, bytes) in &files {
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name).to_ascii_lowercase();
        if !base.ends_with(".item.gbx") || !placed.contains(&base) {
            continue;
        }
        let Ok(f) = mapgeom::static_item::file::parse_file(bytes) else { continue };
        n_files += 1;
        let mut times: Vec<u64> = Vec::new();
        if let Some(so) = f.item.static_object() {
            if let Some(s2) = so.solid2() {
                times.push(s2.file_write_time);
            }
        } else if let Some(pf) = f.item.prefab() {
            for e in &pf.ents {
                if let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() {
                    if let Some(s2) = so.solid2() {
                        times.push(s2.file_write_time);
                    }
                }
            }
        }
        for t in times {
            n_solids += 1;
            if t != 0 && best.map(|b| t > b).unwrap_or(true) {
                best = Some(t);
            }
        }
    }
    Ok((best, n_solids, n_files))
}

/// A FILETIME word as `0x… = unix N`.
pub fn filetime_text(t: u64) -> String {
    format!("{t:#x} = unix {}", (t / 10_000_000).saturating_sub(11_644_473_600))
}

pub fn patch_frame_records(head: &mut [u8], fp: &FrameParams) {
    for i in 0..3 {
        let r = 60 + 66 * i;
        if r + 32 > head.len() {
            break;
        }
        // 0xffffffff = keep the template's time word
        if fp.daytime != 0xffff_ffff {
            head[r + 8..r + 12].copy_from_slice(&fp.daytime.to_le_bytes());
        }
        head[r + 16..r + 20].copy_from_slice(&fp.max_hdr_mood.to_le_bytes());
        // frame 0's MaxHDR is the bake's K; the editor writes the other frames' (the black night frame) as 1e-5
        // (every editor save: "frame record 1: MaxHDR 0.00001" — validation 2026-09-25)
        let k = if i == 0 { fp.max_hdr } else { 1e-5f32 };
        head[r + 20..r + 24].copy_from_slice(&k.to_le_bytes());
        head[r + 24..r + 28].copy_from_slice(&fp.bounce.to_le_bytes());
        head[r + 28..r + 32].copy_from_slice(&fp.sky.to_le_bytes());
    }
}

pub fn build_full(charts: Vec<Chart>, bbox: ([f32; 3], [f32; 3]), template: &LightmapChunk, probes: Option<ProbeBlob>, vp8_q: Option<u8>) -> Result<Synth, String> {
    build_full2(charts, bbox, template, probes, vp8_q, None)
}

pub fn build_full2(charts: Vec<Chart>, bbox: ([f32; 3], [f32; 3]), template: &LightmapChunk, probes: Option<ProbeBlob>, vp8_q: Option<u8>, frame: Option<FrameParams>) -> Result<Synth, String> {
    build_full2_placed(charts, bbox, template, probes, vp8_q, frame, None)
}

/// `build_full2` with the charts' STORED-TEXEL positions given (obj, sub) → (px, py) — the game's own layout
/// (`layout::for_map`: px = (X + 1)/2 of the 2048-unit layout x) instead of the shelf packer; no shrinking.
pub fn build_full2_placed(mut charts: Vec<Chart>, bbox: ([f32; 3], [f32; 3]), template: &LightmapChunk, probes: Option<ProbeBlob>, vp8_q: Option<u8>, frame: Option<FrameParams>, fixed: Option<&std::collections::HashMap<(u32, u32), (u32, u32)>>) -> Result<Synth, String> {
    let td = template.data.as_ref().ok_or("template has no lightmap")?;
    let tm = td.cache.mapping().ok_or("template has no mapping chunk")?;
    // fit: shrink every chart uniformly until the shelf packer accepts the set
    let mut factor = 1.0f32;
    if std::env::var("LMTOOL_DEBUG_PACK").is_ok() {
        let sizes: Vec<(u32, u32)> = charts.iter().map(|c| (c.w, c.h)).collect();
        let area: u64 = sizes.iter().map(|(w, h)| (*w as u64 + 1) * (*h as u64 + 1)).sum();
        let big = sizes.iter().filter(|(w, h)| *w > 64 || *h > 64).count();
        let mut hist = std::collections::BTreeMap::new();
        for (_, h) in &sizes { *hist.entry(*h).or_insert(0usize) += 1; }
        eprintln!("  pack debug: {} charts, gutter area {area} px² ({:.1} % of 1024²), {big} charts over 64 px, fits {}; height histogram {:?}", sizes.len(), 100.0 * area as f64 / 1048576.0, shelf_fits(&sizes, 1024), hist.iter().take(12).collect::<Vec<_>>());
    }
    loop {
        if fixed.is_some() {
            break;
        }
        let mut shelf = Shelf::new(1024);
        let mut order: Vec<usize> = (0..charts.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(charts[i].h), std::cmp::Reverse(charts[i].w)));
        let ok = order.iter().all(|&i| shelf.place(charts[i].w, charts[i].h).is_ok());
        if ok {
            break;
        }
        factor *= 0.92;
        if factor < 0.2 {
            return Err("atlas full even at 20 % chart size".into());
        }
        for c in charts.iter_mut() {
            let (nw, nh) = (((c.w as f32) * 0.92).round().max(2.0) as u32, ((c.h as f32) * 0.92).round().max(2.0) as u32);
            if nw == c.w && nh == c.h {
                continue;
            }
            let mut a = Vec::with_capacity((nw * nh) as usize);
            let mut a1 = Vec::with_capacity(if c.a1.is_empty() { 0 } else { (nw * nh) as usize });
            for y in 0..nh {
                for x in 0..nw {
                    let sx = ((x as f32 + 0.5) * c.w as f32 / nw as f32) as u32;
                    let sy = ((y as f32 + 0.5) * c.h as f32 / nh as f32) as u32;
                    let i = (sy.min(c.h - 1) * c.w + sx.min(c.w - 1)) as usize;
                    a.push(c.a[i]);
                    if !c.a1.is_empty() {
                        a1.push(c.a1[i]);
                    }
                }
            }
            c.w = nw;
            c.h = nh;
            c.a = a;
            c.a1 = a1;
        }
    }
    if factor < 1.0 {
        eprintln!("  atlas: charts shrunk to {:.0} % to fit", factor * 100.0);
    }
    // pack tallest first, then emit the tables in object order
    let area: u64 = charts.iter().map(|c| (c.w * c.h) as u64).sum();
    let mut ia = Rgb::new(1024, 1024);
    let mut ib = Rgb::new(1024, 1024);
    let mut i1 = Rgb::new(1024, 1024);
    let mut any_lights = false;
    for p in ib.px.iter_mut() {
        *p = 128;
    }
    charts.sort_by_key(|c| (c.obj, c.sub));
    let mut placed_by_obj: std::collections::HashMap<(u32, u32), (u32, u32)> = std::collections::HashMap::new();
    {
        let mut shelf = Shelf::new(1024);
        let mut order: Vec<usize> = (0..charts.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(charts[i].h), std::cmp::Reverse(charts[i].w)));
        for &i in &order {
            let c = &charts[i];
            if let Some(fx) = fixed {
                let p = *fx.get(&(c.obj, c.sub)).ok_or_else(|| format!("no fixed position for chart obj {} sub {}", c.obj, c.sub))?;
                placed_by_obj.insert((c.obj, c.sub), p);
                continue;
            }
            placed_by_obj.insert((c.obj, c.sub), shelf.place(c.w, c.h)?);
        }
    }
    let n = charts.len();
    let mut pos = Vec::with_capacity(n);
    let mut size = Vec::with_capacity(n);
    let mut binds = Vec::with_capacity(n);
    let mut fb: Vec<Vec<u8>> = vec![Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n)];
    for c in &charts {
        let (px, py) = placed_by_obj[&(c.obj, c.sub)];
        // the chart, plus a 1-pixel border of its own edge pixels (bilinear safety)
        for y in 0..c.h + 2 {
            for x in 0..c.w + 2 {
                let sx = (x as i64 - 1).clamp(0, c.w as i64 - 1) as u32;
                let sy = (y as i64 - 1).clamp(0, c.h as i64 - 1) as u32;
                let col = c.a[(sy * c.w + sx) as usize];
                let (ax, ay) = (px + x - 1, py + y - 1);
                if ax < 1024 && ay < 1024 {
                    ia.set(ax, ay, col);
                    ib.set(ax, ay, [c.b, c.b, c.b]);
                    if !c.a1.is_empty() {
                        i1.set(ax, ay, c.a1[(sy * c.w + sx) as usize]);
                        any_lights = true;
                    }
                }
            }
        }
        pos.push(((2 * px - 1) as u16, (2 * py - 1) as u16));
        size.push(((2 * c.w) as u16, (2 * c.h) as u16));
        binds.push(ObjBind { obj_idx: c.sub, obj_group_idx: c.obj * 4 });
        for k in 0..3 {
            fb[k].push(c.fb[k]);
        }
    }
    let mapping = Mapping {
        version: tm.version,
        head: tm.head.clone(),
        map_version: tm.map_version,
        m_u01: tm.m_u01,
        atlas_w: tm.atlas_w,
        atlas_h: tm.atlas_h,
        bbox_min: bbox.0,
        bbox_max: bbox.1,
        m_u02: tm.m_u02,
        count: n as u32,
        chart_f32: vec![-1.0; n],
        binds,
        pos,
        size,
        m_u03: tm.m_u03,
        frame_bytes: fb,
        tail: tm.tail.clone(),
        raw_z: Vec::new(),
    };
    let mut mapping = mapping;
    if let Some(fp) = &frame {
        patch_frame_records(&mut mapping.head, fp);
    }
    // the head's records follow the image frames (two): a 3-record source template is cut like the transcribed writer's
    let _ = cut_head_records(&mut mapping.head, frame_records_wanted());
    let _ = fix_record0_kind(&mut mapping.head);
    let chunks: Vec<CacheChunk> = td
        .cache
        .chunks
        .iter()
        .map(|c| match &c.body {
            ChunkBody::Mapping(_) => CacheChunk { id: c.id, body: ChunkBody::Mapping(mapping.clone()) },
            ChunkBody::Raw(b) => CacheChunk { id: c.id, body: ChunkBody::Raw(patch_raw_chunk(c.id, b, frame.as_ref())) },
        })
        .collect();
    let trailer = match &probes {
        Some(p) => p.trailer.clone(),
        None => td.cache.trailer.clone(),
    };
    let cache = CacheBlob { chunks, trailer };
    // the game's encoder settings (RE child, disassembly): libwebp preset DEFAULT, quality 91 for
    // the colour atlas / frame 1 / probes, 30 for the three grey directional images (the editor's VP8 segment header matches q 30, not 24–26), planes fed
    // as BT.601 studio YUV420. With libwebp linked that is what we write; `--vp8 Q` forces our own
    // encoder; without libwebp and without --vp8 the images are lossless
    let enc_q = |im: &Rgb, quality: f32| -> Result<Vec<u8>, String> {
        match vp8_q {
            Some(q) => Ok(crate::vp8enc::encode(&im.px, im.w, im.h, q)),
            None => match crate::webpenc::encode_rgb(&im.px, im.w, im.h, quality) {
                Some(b) => Ok(b),
                None => encode_webp_lossless(im),
            },
        }
    };
    let enc = |im: &Rgb| enc_q(im, 91.0);
    // THE ENCODES CONCURRENTLY (perf 8): the black frame-1 image, the colour atlas, the grey directional image and the
    // lights' atlas are independent encodes — one thread each (the same bytes: every encode is single-threaded libwebp
    // on its own input)
    let need_lights = any_lights && td.frames.len() > 1;
    let (black, im00, one, im10) = std::thread::scope(|sc| {
        let h_black = sc.spawn(|| enc(&Rgb::new(1024, 1024)));
        let h_atlas = sc.spawn(|| enc(&ia));
        let h_grey = sc.spawn(|| -> Result<Vec<u8>, String> {
            // the greys go in as a Y plane with flat chroma (the game's FUN_14029bf40)
            let grey: Vec<u8> = ib.px.chunks(3).map(|c| c[1]).collect();
            match vp8_q {
                None => crate::webpenc::encode_grey(&grey, ib.w, ib.h, 30.0).map(Ok).unwrap_or_else(|| enc_q(&ib, 30.0)),
                Some(_) => enc_q(&ib, 30.0),
            }
        });
        let h_lights = if need_lights { Some(sc.spawn(|| enc(&i1))) } else { None };
        (h_black.join().expect("encode"), h_atlas.join().expect("encode"), h_grey.join().expect("encode"), h_lights.map(|h| h.join().expect("encode")))
    });
    let (black, im00, one) = (black?, im00?, one?);
    let im10 = match im10 { Some(r) => Some(r?), None => None };
    let mut frames = Vec::new();
    // TWO IMAGE FRAMES ALWAYS (V2-15, 2026-09-27: a 3-frame SOURCE file made this writer emit 3 frames — the editor writes two, whatever
    // the source carried; the transcribed writer below already caps at the record count)
    for fi in 0..td.frames.len().min(frame_records_wanted()) {
        let mut images = Vec::new();
        for ii in 0..td.frames[fi].images.len() {
            let src = &td.frames[fi].images[ii];
            let im = match (fi, ii) {
                (0, 0) => im00.clone(),
                // frame 0 image 1 = THREE concatenated WebPs (the H-basis directional coefficients C1..C3,
                // sign-sqrt encoded with 128 = zero); a flat-normal bake writes three neutral images
                (0, 1) => {
                    let mut three = one.clone();
                    three.extend_from_slice(&one);
                    three.extend_from_slice(&one);
                    three
                }
                (0, 2) => match &probes {
                    Some(p) => p.blob.clone(),
                    None => src.clone(),
                },
                (1, 0) if any_lights => im10.clone().unwrap_or_default(),
                (_, 0) if !src.is_empty() => black.clone(),
                _ => Vec::new(),
            };
            images.push(im);
        }
        frames.push(Frame { images });
    }
    let raw = cache.write();
    let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9);
    let chunk = LightmapChunk {
        version: template.version,
        u01: template.u01,
        u02: template.u02,
        data: Some(LightmapData {
            lightmap_version: td.lightmap_version,
            frames,
            cache,
            cache_compressed: z,
            cache_uncompressed_len: raw.len() as u32,
        }),
    };
    let placed: Vec<(u32, u32, u32, u32, u32, u32)> = charts.iter().map(|c| { let (px, py) = placed_by_obj[&(c.obj, c.sub)]; (c.obj, c.sub, px, py, c.w, c.h) }).collect();
    Ok(Synth { chunk, charts: n as u32, fill: area as f32 / (1024.0 * 1024.0), atlas8: Some(ia), placed })
}

// ───────────────────────────── THE TRANSCRIBED WRITER (the game's own encoding, no encoder of ours) ─────────────────────────────

/// What the transcribed CPU chain hands the writer (see `filecheck::file_images` / `frame0_blobs` / `record_scales`):
/// frame 0's blobs already encoded the game's way, the per-chart frame bytes in the mapping's order, and the
/// record's scale fields. `lambient_f16` = None keeps the template's LAmbient triple.
pub struct TranscribedImages {
    pub blob0: Vec<u8>,
    pub blob1: Vec<u8>,
    pub fb0: Vec<u8>,
    pub max_hdr: f32,
    pub hbasis234: [f32; 3],
    pub lambient_f16: Option<[u16; 3]>,
    /// THE LOCAL-LIGHT FRAME (frame 1, engineer F): its WebP, per-chart frame bytes and MaxHDR when the mood has the lights on
    /// and a lamp lit something; None = the black frame at MaxHDR 1e-5 (Day, or no lamp).
    pub frame1: Option<crate::localdrive::Frame1Image>,
}

/// Patch the frame-0 record's MaxHDR_HBasisScaled234 triple (record +54/+58/+62) and, when given, the LAmbient f16
/// triple (record +36..+42) — the record layout per engineer C's `filecheck::check_records` (record 0 starts 12 bytes
/// before the −FLT_MAX word; `patch_frame_records` writes the same record's daytime/mood/MaxHDR/bounce/sky).
pub fn patch_record_scales(head: &mut [u8], hbasis234: [f32; 3], lambient_f16: Option<[u16; 3]>) -> bool {
    let Some(pos) = head.windows(4).position(|w| w == [0xff, 0xff, 0x7f, 0xff]) else { return false };
    if pos < 12 || pos - 12 + 66 > head.len() {
        return false;
    }
    let r = pos - 12;
    for k in 0..3 {
        head[r + 54 + 4 * k..r + 58 + 4 * k].copy_from_slice(&hbasis234[k].to_le_bytes());
    }
    if let Some(l) = lambient_f16 {
        for k in 0..3 {
            head[r + 36 + 2 * k..r + 38 + 2 * k].copy_from_slice(&l[k].to_le_bytes());
        }
    }
    true
}

/// The chunk from the transcribed chain: `placed` = the charts in the MAPPING's order as (obj, sub, px, py, w, h) in
/// stored texels (a `Synth::placed`, or `layout::for_map`'s table), `img` = the encoded frame-0 images and record
/// scales (`fb0` one byte per chart in the same order), `probes` = the probe blob + trailer (None = the template's).
/// Frame 1 image 0 stays the all-black 1024² at q 91 (the game's, byte-identical); frames 1–2's chart bytes are the
/// template's when the counts match, else 0. Nothing here is encoded by the port's own encoder.
pub fn build_transcribed(placed: &[(u32, u32, u32, u32, u32, u32)], bbox: ([f32; 3], [f32; 3]), template: &LightmapChunk, img: &TranscribedImages, probes: Option<ProbeBlob>, frame: Option<FrameParams>) -> Result<Synth, String> {
    let td = template.data.as_ref().ok_or("template has no lightmap")?;
    let tm = td.cache.mapping().ok_or("template has no mapping chunk")?;
    let n = placed.len();
    if img.fb0.len() != n {
        return Err(format!("transcribed writer: {} frame bytes for {n} charts", img.fb0.len()));
    }
    let mut pos = Vec::with_capacity(n);
    let mut size = Vec::with_capacity(n);
    let mut binds = Vec::with_capacity(n);
    let mut fb: Vec<Vec<u8>> = vec![Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n)];
    let same_count = tm.frame_bytes.iter().all(|v| v.len() == n);
    let mut area = 0u64;
    for (i, &(obj, sub, px, py, w, h)) in placed.iter().enumerate() {
        pos.push(((2 * px).saturating_sub(1) as u16, (2 * py).saturating_sub(1) as u16));
        size.push(((2 * w) as u16, (2 * h) as u16));
        binds.push(ObjBind { obj_idx: sub, obj_group_idx: obj * 4 });
        fb[0].push(img.fb0[i]);
        for k in 1..3 {
            // frame 1's byte from the local-light frame when it was baked, else the template's (when the counts match)
            let f1 = if k == 1 { img.frame1.as_ref().and_then(|f| f.fb1.get(i).copied()) } else { None };
            fb[k].push(f1.unwrap_or(if same_count { tm.frame_bytes.get(k).map(|v| v[i]).unwrap_or(0) } else { 0 }));
        }
        area += (w * h) as u64;
    }
    let mut mapping = Mapping {
        version: tm.version,
        head: tm.head.clone(),
        map_version: tm.map_version,
        m_u01: tm.m_u01,
        atlas_w: tm.atlas_w,
        atlas_h: tm.atlas_h,
        bbox_min: bbox.0,
        bbox_max: bbox.1,
        m_u02: tm.m_u02,
        count: n as u32,
        chart_f32: vec![-1.0; n],
        binds,
        pos,
        size,
        m_u03: tm.m_u03,
        frame_bytes: fb,
        tail: tm.tail.clone(),
        raw_z: Vec::new(),
    };
    if let Some(fp) = &frame {
        let mut fp2 = fp.clone();
        fp2.max_hdr = img.max_hdr;
        patch_frame_records(&mut mapping.head, &fp2);
    } else {
        // no mood parameters given: still the chain's MaxHDR in every record's slot
        for i in 0..3 {
            let r = 60 + 66 * i;
            if r + 24 <= mapping.head.len() {
                mapping.head[r + 20..r + 24].copy_from_slice(&img.max_hdr.to_le_bytes());
            }
        }
    }
    if !patch_record_scales(&mut mapping.head, img.hbasis234, img.lambient_f16) {
        return Err("transcribed writer: the mapping head has no frame record (−FLT_MAX word)".into());
    }
    // THE LOCAL-LIGHT FRAME'S RECORD (E, 2026-09-27 16:40Z, from the bytes of eleven oracles — my 09-26 read "no lamp → frame 0's MaxHDR,
    // pwc-day 2.1193807" was WRONG: pwc-day's editor rec 1 reads 0.00001 like every other lamp-less ref): MaxHDR₁ = the f16 MAX of the
    // resolved image (the RGBA16F resolve target's values — giant20x2 0.9995117 = 1 − 2⁻¹¹, the f16 just below 1; stpad exactly 1.0 = a
    // max in [0.99976, 1.00049); tiny16 1.2851563, tiny03 Sunset 1.5234375, tiny03 Day 0.0064468384 all f16-representable) — NO floor
    // at 1.0 (tiny03 Day's 0.0064 and giant20x2's 0.99951 refute it); WHEN NOTHING WAS LIT the record keeps the template's 0.00001
    // (np-tk3 Day + Night, hill4, tiny04ac, g23, pwc-day: all 1e-5). `f1.max_hdr` carries the f16 max (localdrive::frame1_*).
    {
        let r = 60 + 66;
        if mapping.head.len() >= r + 24 {
            let v = match &img.frame1 { Some(f1) if f1.lit_texels > 0 && f1.max_hdr > 1e-5 => f1.max_hdr, _ => 1e-5f32 };
            mapping.head[r + 20..r + 24].copy_from_slice(&v.to_le_bytes());
        }
    }
    // the records the frame list calls for (two on every editor bake — V's scan; `frame_records_wanted`)
    let n_records = cut_head_records(&mut mapping.head, frame_records_wanted());
    let _ = fix_record0_kind(&mut mapping.head);
    let chunks: Vec<CacheChunk> = td
        .cache
        .chunks
        .iter()
        .map(|c| match &c.body {
            ChunkBody::Mapping(_) => CacheChunk { id: c.id, body: ChunkBody::Mapping(mapping.clone()) },
            ChunkBody::Raw(b) => CacheChunk { id: c.id, body: ChunkBody::Raw(patch_raw_chunk(c.id, b, frame.as_ref())) },
        })
        .collect();
    let trailer = match &probes {
        Some(p) => p.trailer.clone(),
        None => td.cache.trailer.clone(),
    };
    let cache = CacheBlob { chunks, trailer };
    // the game's frame-1 image: an all-black 1024² RGB through libwebp at q 91 (byte-identical to the editor's, engineer C)
    let black = match crate::webpenc::encode_rgb(&Rgb::new(1024, 1024).px, 1024, 1024, 91.0) {
        Some(b) => b,
        None => return Err("transcribed writer needs libwebp (webpenc) for the frame-1 image".into()),
    };
    let mut frames = Vec::new();
    for fi in 0..td.frames.len().min(n_records) {
        let mut images = Vec::new();
        for ii in 0..td.frames[fi].images.len() {
            let src = &td.frames[fi].images[ii];
            let im = match (fi, ii) {
                (0, 0) => img.blob0.clone(),
                (0, 1) => img.blob1.clone(),
                (0, 2) => match &probes {
                    Some(p) => p.blob.clone(),
                    None => src.clone(),
                },
                (1, 0) if img.frame1.is_some() => img.frame1.as_ref().unwrap().webp.clone(),
                (_, 0) if !src.is_empty() => black.clone(),
                _ => Vec::new(),
            };
            images.push(im);
        }
        frames.push(Frame { images });
    }
    let raw = cache.write();
    let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9);
    let chunk = LightmapChunk {
        version: template.version,
        u01: template.u01,
        u02: template.u02,
        data: Some(LightmapData { lightmap_version: td.lightmap_version, frames, cache, cache_compressed: z, cache_uncompressed_len: raw.len() as u32 }),
    };
    Ok(Synth { chunk, charts: n as u32, fill: area as f32 / (1024.0 * 1024.0), atlas8: None, placed: placed.to_vec() })
}
