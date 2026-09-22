//! From-scratch synthesis of a lightmap chunk for a tiny map: our own chart
//! layout (one chart per ground slot and per item), our own atlas images, the
//! frame table / trailer / small chunks templated from a real bake of the same
//! mood. This is the authoring path; `paint` only edits a real bake in place.

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

/// A shelf packer over a 1024×1024 pixel atlas with 1-pixel gutters.
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
            return Err(format!("atlas full at row y={}", self.y));
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
}

/// Build the chunk. `template` supplies the frame table, the small chunks,
/// the trailer and the third (sprite) image.
pub fn synth(plan: &Plan, template: &LightmapChunk) -> Result<Synth, String> {
    let td = template.data.as_ref().ok_or("template has no lightmap")?;
    let tm = td.cache.mapping().ok_or("template has no mapping chunk")?;
    let n = (plan.base + plan.items) as usize;
    let mut pos = Vec::with_capacity(n);
    let mut size = Vec::with_capacity(n);
    let mut binds = Vec::with_capacity(n);
    let mut fb: Vec<Vec<u8>> = vec![Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n)];
    let mut ia = Rgb::new(1024, 1024);
    let mut ib = Rgb::new(1024, 1024);
    // gutters: neutral grey in B, black in A (the game does not sample them if the
    // uv transform is exact; a 1-pixel border of the chart's own colour would be
    // the safer fill — done below by painting the border too)
    for p in ib.px.iter_mut() {
        *p = 128;
    }
    let mut shelf = Shelf::new(1024);
    for obj in 0..n as u32 {
        let (px_size, spec) = if obj < plan.base {
            (plan.ground_px, plan.ground)
        } else {
            let i = (obj - plan.base) as usize;
            (plan.item_px, plan.item_spec.get(i).copied().flatten().unwrap_or(plan.item_default))
        };
        let (px, py) = shelf.place(px_size, px_size)?;
        // paint the chart and a 1-pixel border around it (bilinear safety)
        let x0 = px.saturating_sub(1);
        let y0 = py.saturating_sub(1);
        for y in y0..(py + px_size + 1).min(1024) {
            for x in x0..(px + px_size + 1).min(1024) {
                ia.set(x, y, spec.a);
                ib.set(x, y, [spec.b, spec.b, spec.b]);
            }
        }
        pos.push(((2 * px - 1) as u16, (2 * py - 1) as u16));
        size.push(((2 * px_size) as u16, (2 * px_size) as u16));
        binds.push(ObjBind { obj_idx: 0, obj_group_idx: obj * 4 });
        for k in 0..3 {
            fb[k].push(spec.fb[k]);
        }
    }
    let mapping = Mapping {
        version: tm.version,
        head: tm.head.clone(),
        map_version: tm.map_version,
        m_u01: tm.m_u01,
        atlas_w: tm.atlas_w,
        atlas_h: tm.atlas_h,
        bbox_min: plan.bbox.0,
        bbox_max: plan.bbox.1,
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
    let cache = CacheBlob { chunks, trailer: td.cache.trailer.clone() };
    let black = {
        let im = Rgb::new(1024, 1024);
        encode_webp_lossless(&im)?
    };
    let mut frames = Vec::new();
    for fi in 0..td.frames.len() {
        let mut images = Vec::new();
        for ii in 0..td.frames[fi].images.len() {
            let src = &td.frames[fi].images[ii];
            let im = match (fi, ii) {
                (0, 0) => encode_webp_lossless(&ia)?,
                (0, 1) => encode_webp_lossless(&ib)?,
                (0, 2) => src.clone(),
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
    Ok(Synth { chunk, charts: n as u32 })
}
