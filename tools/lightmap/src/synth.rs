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
                let fb1 = (255.0 * max1 / k).round().clamp(1.0, 255.0) as u8;
                let enc_max = fb1 as f32 / 255.0 * k;
                c.a1 = rgb1.iter().map(|v| { let f = |x: f32| (x / enc_max * 255.0).round().clamp(0.0, 255.0) as u8; [f(v[0]), f(v[1]), f(v[2])] }).collect();
                c.fb[1] = fb1;
            }
        }
        c
    }

    /// From HDR irradiance: normalise to the chart max, `k` = the scale the
    /// per-chart byte is relative to (fb0 = 255·max/k).
    pub fn from_hdr(obj: u32, w: u32, h: u32, rgb: &[[f32; 3]], k: f32, b: u8) -> Chart {
        let max = rgb.iter().flat_map(|c| c.iter().copied()).fold(0.0f32, f32::max).max(1e-4);
        let fb0 = (255.0 * max / k).round().clamp(1.0, 255.0) as u8;
        // the stored max is quantised: normalise against what the byte encodes
        let enc_max = fb0 as f32 / 255.0 * k;
        let a: Vec<[u8; 3]> = rgb
            .iter()
            .map(|c| {
                let f = |v: f32| (v / enc_max * 255.0).round().clamp(0.0, 255.0) as u8;
                [f(c[0]), f(c[1]), f(c[2])]
            })
            .collect();
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

pub struct Synth {
    pub chunk: LightmapChunk,
    pub charts: u32,
    /// Fraction of the 1024² atlas the charts occupy (gutters excluded).
    pub fill: f32,
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

pub fn build_full(mut charts: Vec<Chart>, bbox: ([f32; 3], [f32; 3]), template: &LightmapChunk, probes: Option<ProbeBlob>, vp8_q: Option<u8>) -> Result<Synth, String> {
    let td = template.data.as_ref().ok_or("template has no lightmap")?;
    let tm = td.cache.mapping().ok_or("template has no mapping chunk")?;
    // fit: shrink every chart uniformly until the shelf packer accepts the set
    let mut factor = 1.0f32;
    loop {
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
    let chunks: Vec<CacheChunk> = td
        .cache
        .chunks
        .iter()
        .map(|c| match &c.body {
            ChunkBody::Mapping(_) => CacheChunk { id: c.id, body: ChunkBody::Mapping(mapping.clone()) },
            ChunkBody::Raw(b) => CacheChunk { id: c.id, body: ChunkBody::Raw(b.clone()) },
        })
        .collect();
    let trailer = match &probes {
        Some(p) => p.trailer.clone(),
        None => td.cache.trailer.clone(),
    };
    let cache = CacheBlob { chunks, trailer };
    let enc = |im: &Rgb| -> Result<Vec<u8>, String> {
        match vp8_q {
            Some(q) => Ok(crate::vp8enc::encode(&im.px, im.w, im.h, q)),
            None => encode_webp_lossless(im),
        }
    };
    let black = enc(&Rgb::new(1024, 1024))?;
    let mut frames = Vec::new();
    for fi in 0..td.frames.len() {
        let mut images = Vec::new();
        for ii in 0..td.frames[fi].images.len() {
            let src = &td.frames[fi].images[ii];
            let im = match (fi, ii) {
                (0, 0) => enc(&ia)?,
                (0, 1) => enc(&ib)?,
                (0, 2) => match &probes {
                    Some(p) => p.blob.clone(),
                    None => src.clone(),
                },
                (1, 0) if any_lights => enc(&i1)?,
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
    Ok(Synth { chunk, charts: n as u32, fill: area as f32 / (1024.0 * 1024.0) })
}
