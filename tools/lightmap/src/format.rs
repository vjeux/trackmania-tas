//! The typed layout of chunk `0x0304305B` and the `CHmsLightMapCache` node it
//! carries. Every field is either understood (named) or kept as raw bytes
//! (`raw_*`), and `write` reproduces the input byte for byte — the round-trip
//! test over the store is what keeps this honest while the unknowns shrink.
//!
//! Layout (all little-endian; confidence tags in docs/formats/map-lightmap.md):
//!
//! ```text
//! u32 version (0)   u32 has_lightmaps   u32 u01   u32 u02
//! if has_lightmaps:
//!   u32 lightmap_version (10)   u32 frame_count (3)
//!   frame_count × { 3 × blob }      blob = u32 len + bytes (WEBP, len 0 = none)
//!   u32 cache_uncompressed  u32 cache_compressed  zlib → CacheBlob
//! CacheBlob = CHmsLightMapCache node (PIKS chunks … FACADE01) + trailer
//! ```

use crate::{zlib_inflate, Cur};

pub const IMAGES_PER_FRAME: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct LightmapChunk {
    pub version: u32,
    pub u01: u32,
    pub u02: u32,
    /// `None` = `has_lightmaps == 0` (the 16-byte form).
    pub data: Option<LightmapData>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightmapData {
    pub lightmap_version: u32,
    pub frames: Vec<Frame>,
    /// The inflated cache blob, as parsed.
    pub cache: CacheBlob,
    /// The zlib stream as found in the file (kept so a round-trip is
    /// byte-identical: recompression need not match the game's deflate).
    pub cache_compressed: Vec<u8>,
    pub cache_uncompressed_len: u32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Frame {
    /// The WEBP images: [0] colour atlas, [1] second atlas, [2] the small
    /// third atlas; empty = absent.
    pub images: Vec<Vec<u8>>,
}

/// The inflated blob: the node's chunks, then a trailer after `FACADE01`.
#[derive(Clone, Debug, PartialEq)]
pub struct CacheBlob {
    pub chunks: Vec<CacheChunk>,
    pub trailer: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CacheChunk {
    pub id: u32,
    pub body: ChunkBody,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChunkBody {
    Mapping(Mapping),
    Raw(Vec<u8>),
}

/// Chunk `0x0602201A` — `SHmsLightMapCacheMapping` and its per-chart tables.
#[derive(Clone, Debug, PartialEq)]
pub struct Mapping {
    pub version: u32,
    /// Bytes between the version word and the `u32 9` mapping-struct
    /// version: the frame descriptions (3 × 66 bytes) and a fixed head. Kept
    /// raw until decoded.
    pub head: Vec<u8>,
    pub map_version: u32,
    pub m_u01: u32,
    pub atlas_w: u32,
    pub atlas_h: u32,
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
    pub m_u02: u32,
    pub count: u32,
    /// z0: one f32 per chart (−1.0 everywhere so far).
    pub chart_f32: Vec<f32>,
    /// z1: per chart (sub-object index | flags, object index × 4).
    pub binds: Vec<ObjBind>,
    /// z2: per chart atlas position (x, y) in the 2048-wide layout.
    pub pos: Vec<(u16, u16)>,
    /// z3: per chart atlas size (w, h).
    pub size: Vec<(u16, u16)>,
    pub m_u03: u32,
    /// z4: per frame, one byte per chart.
    pub frame_bytes: Vec<Vec<u8>>,
    pub tail: Vec<u8>,
    /// The compressed form of each table as found (for byte-identical
    /// round-trips); `None` once a table was edited.
    pub raw_z: Vec<Option<(u32, Vec<u8>)>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjBind {
    pub obj_idx: u32,
    pub obj_group_idx: u32,
}

fn read_blob(c: &mut Cur) -> Result<Vec<u8>, String> {
    let n = c.u32()? as usize;
    Ok(c.take(n)?.to_vec())
}

impl LightmapChunk {
    pub fn parse(p: &[u8]) -> Result<Self, String> {
        let mut c = Cur::new(p);
        let version = c.u32()?;
        let has = c.u32()?;
        let u01 = c.u32()?;
        let u02 = c.u32()?;
        if has == 0 {
            if c.left() != 0 {
                return Err(format!("has_lightmaps = 0 but {} bytes follow", c.left()));
            }
            return Ok(LightmapChunk { version, u01, u02, data: None });
        }
        let lightmap_version = c.u32()?;
        let frame_count = c.u32()? as usize;
        if lightmap_version < 10 {
            return Err(format!("lightmap_version {lightmap_version}: only 10 is decoded"));
        }
        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let mut images = Vec::with_capacity(IMAGES_PER_FRAME);
            for _ in 0..IMAGES_PER_FRAME {
                images.push(read_blob(&mut c)?);
            }
            frames.push(Frame { images });
        }
        let cache_uncompressed_len = c.u32()?;
        let csize = c.u32()? as usize;
        let cache_compressed = c.take(csize)?.to_vec();
        if c.left() != 0 {
            return Err(format!("{} bytes after the cache", c.left()));
        }
        let raw = zlib_inflate(&cache_compressed, cache_uncompressed_len as usize)?;
        let cache = CacheBlob::parse(&raw)?;
        Ok(LightmapChunk {
            version,
            u01,
            u02,
            data: Some(LightmapData { lightmap_version, frames, cache, cache_compressed, cache_uncompressed_len }),
        })
    }

    /// Serialise. `recompress`: rebuild the zlib stream from the parsed cache
    /// (needed after any edit); otherwise the stored stream is written back.
    pub fn write(&self, recompress: bool) -> Vec<u8> {
        let mut o = Vec::new();
        o.extend_from_slice(&self.version.to_le_bytes());
        o.extend_from_slice(&(self.data.is_some() as u32).to_le_bytes());
        o.extend_from_slice(&self.u01.to_le_bytes());
        o.extend_from_slice(&self.u02.to_le_bytes());
        let Some(d) = &self.data else { return o };
        o.extend_from_slice(&d.lightmap_version.to_le_bytes());
        o.extend_from_slice(&(d.frames.len() as u32).to_le_bytes());
        for f in &d.frames {
            for im in &f.images {
                o.extend_from_slice(&(im.len() as u32).to_le_bytes());
                o.extend_from_slice(im);
            }
        }
        if recompress {
            let raw = d.cache.write();
            let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9);
            o.extend_from_slice(&(raw.len() as u32).to_le_bytes());
            o.extend_from_slice(&(z.len() as u32).to_le_bytes());
            o.extend_from_slice(&z);
        } else {
            o.extend_from_slice(&d.cache_uncompressed_len.to_le_bytes());
            o.extend_from_slice(&(d.cache_compressed.len() as u32).to_le_bytes());
            o.extend_from_slice(&d.cache_compressed);
        }
        o
    }
}

pub const PIKS: u32 = 0x534B_4950;
pub const FACADE: u32 = 0xFACA_DE01;
pub const MAPPING_CHUNK: u32 = 0x0602_201A;

impl CacheBlob {
    pub fn parse(raw: &[u8]) -> Result<Self, String> {
        let mut c = Cur::new(raw);
        let mut chunks = Vec::new();
        loop {
            let id = c.u32()?;
            if id == FACADE {
                break;
            }
            let magic = c.u32()?;
            if magic != PIKS {
                return Err(format!("cache chunk {id:#010x} at {:#x} is not skippable", c.o - 8));
            }
            let size = c.u32()? as usize;
            let payload = c.take(size)?;
            let body = if id == MAPPING_CHUNK {
                ChunkBody::Mapping(Mapping::parse(payload)?)
            } else {
                ChunkBody::Raw(payload.to_vec())
            };
            chunks.push(CacheChunk { id, body });
        }
        let trailer = c.take(c.left())?.to_vec();
        Ok(CacheBlob { chunks, trailer })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut o = Vec::new();
        for ch in &self.chunks {
            let payload = match &ch.body {
                ChunkBody::Mapping(m) => m.write(),
                ChunkBody::Raw(b) => b.clone(),
            };
            o.extend_from_slice(&ch.id.to_le_bytes());
            o.extend_from_slice(&PIKS.to_le_bytes());
            o.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            o.extend_from_slice(&payload);
        }
        o.extend_from_slice(&FACADE.to_le_bytes());
        o.extend_from_slice(&self.trailer);
        o
    }

    pub fn chunk(&self, id: u32) -> Option<&CacheChunk> {
        self.chunks.iter().find(|c| c.id == id)
    }
    pub fn mapping(&self) -> Option<&Mapping> {
        self.chunks.iter().find_map(|c| match &c.body {
            ChunkBody::Mapping(m) => Some(m),
            _ => None,
        })
    }
    pub fn mapping_mut(&mut self) -> Option<&mut Mapping> {
        self.chunks.iter_mut().find_map(|c| match &mut c.body {
            ChunkBody::Mapping(m) => Some(m),
            _ => None,
        })
    }
}

/// Size of the raw head between the chunk version and the `u32 9`.
pub const MAPPING_HEAD_LEN: usize = 0x112 - 4;

fn read_ztable(c: &mut Cur) -> Result<(u32, Vec<u8>, Vec<u8>), String> {
    let usize_ = c.u32()?;
    let csize = c.u32()? as usize;
    let z = c.take(csize)?.to_vec();
    let raw = zlib_inflate(&z, usize_ as usize)?;
    Ok((usize_, z, raw))
}

fn write_ztable(o: &mut Vec<u8>, raw: &[u8], stored: &Option<(u32, Vec<u8>)>) {
    match stored {
        Some((u, z)) => {
            o.extend_from_slice(&u.to_le_bytes());
            o.extend_from_slice(&(z.len() as u32).to_le_bytes());
            o.extend_from_slice(z);
        }
        None => {
            let z = miniz_oxide::deflate::compress_to_vec_zlib(raw, 9);
            o.extend_from_slice(&(raw.len() as u32).to_le_bytes());
            o.extend_from_slice(&(z.len() as u32).to_le_bytes());
            o.extend_from_slice(&z);
        }
    }
}

impl Mapping {
    pub fn parse(p: &[u8]) -> Result<Self, String> {
        let mut c = Cur::new(p);
        let version = c.u32()?;
        if version != 13 {
            return Err(format!("mapping chunk version {version}: only 13 is decoded"));
        }
        let head = c.take(MAPPING_HEAD_LEN)?.to_vec();
        let map_version = c.u32()?;
        if map_version != 9 {
            return Err(format!("mapping struct version {map_version} (expected 9) at {:#x}", c.o - 4));
        }
        let m_u01 = c.u32()?;
        let atlas_w = c.u32()?;
        let atlas_h = c.u32()?;
        let bbox_min = [c.f32()?, c.f32()?, c.f32()?];
        let bbox_max = [c.f32()?, c.f32()?, c.f32()?];
        let m_u02 = c.u32()?;
        let count = c.u32()?;
        let n = count as usize;
        let mut raw_z = Vec::new();
        // z0: f32 per chart
        let (u, z, raw) = read_ztable(&mut c)?;
        if raw.len() != n * 4 {
            return Err(format!("z0: {} bytes for {n} charts", raw.len()));
        }
        let chart_f32 = raw.chunks(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
        raw_z.push(Some((u, z)));
        // z1: (u32, u32) per chart
        let (u, z, raw) = read_ztable(&mut c)?;
        if raw.len() != n * 8 {
            return Err(format!("z1: {} bytes for {n} charts", raw.len()));
        }
        let binds = raw
            .chunks(8)
            .map(|b| ObjBind {
                obj_idx: u32::from_le_bytes(b[0..4].try_into().unwrap()),
                obj_group_idx: u32::from_le_bytes(b[4..8].try_into().unwrap()),
            })
            .collect();
        raw_z.push(Some((u, z)));
        // z2: (u16, u16) per chart
        let (u, z, raw) = read_ztable(&mut c)?;
        if raw.len() != n * 4 {
            return Err(format!("z2: {} bytes for {n} charts", raw.len()));
        }
        let pos = raw
            .chunks(4)
            .map(|b| (u16::from_le_bytes([b[0], b[1]]), u16::from_le_bytes([b[2], b[3]])))
            .collect();
        raw_z.push(Some((u, z)));
        // z3
        let (u, z, raw) = read_ztable(&mut c)?;
        if raw.len() != n * 4 {
            return Err(format!("z3: {} bytes for {n} charts", raw.len()));
        }
        let size = raw
            .chunks(4)
            .map(|b| (u16::from_le_bytes([b[0], b[1]]), u16::from_le_bytes([b[2], b[3]])))
            .collect();
        raw_z.push(Some((u, z)));
        let m_u03 = c.u32()?;
        // z4: u32 nframes, nframes × (u32 n, n bytes)
        let (u, z, raw) = read_ztable(&mut c)?;
        let mut fc = Cur::new(&raw);
        let nf = fc.u32()? as usize;
        let mut frame_bytes = Vec::with_capacity(nf);
        for _ in 0..nf {
            let m = fc.u32()? as usize;
            if m != n {
                return Err(format!("z4: frame table of {m} bytes for {n} charts"));
            }
            frame_bytes.push(fc.take(m)?.to_vec());
        }
        if fc.left() != 0 {
            return Err(format!("z4: {} bytes left", fc.left()));
        }
        raw_z.push(Some((u, z)));
        let tail = c.take(c.left())?.to_vec();
        Ok(Mapping {
            version,
            head,
            map_version,
            m_u01,
            atlas_w,
            atlas_h,
            bbox_min,
            bbox_max,
            m_u02,
            count,
            chart_f32,
            binds,
            pos,
            size,
            m_u03,
            frame_bytes,
            tail,
            raw_z,
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut o = Vec::new();
        o.extend_from_slice(&self.version.to_le_bytes());
        o.extend_from_slice(&self.head);
        o.extend_from_slice(&self.map_version.to_le_bytes());
        o.extend_from_slice(&self.m_u01.to_le_bytes());
        o.extend_from_slice(&self.atlas_w.to_le_bytes());
        o.extend_from_slice(&self.atlas_h.to_le_bytes());
        for v in self.bbox_min.iter().chain(self.bbox_max.iter()) {
            o.extend_from_slice(&v.to_le_bytes());
        }
        o.extend_from_slice(&self.m_u02.to_le_bytes());
        o.extend_from_slice(&self.count.to_le_bytes());
        let none = None;
        let rz = |i: usize| self.raw_z.get(i).unwrap_or(&none);
        let z0: Vec<u8> = self.chart_f32.iter().flat_map(|v| v.to_le_bytes()).collect();
        write_ztable(&mut o, &z0, rz(0));
        let z1: Vec<u8> = self
            .binds
            .iter()
            .flat_map(|b| [b.obj_idx.to_le_bytes(), b.obj_group_idx.to_le_bytes()].concat())
            .collect();
        write_ztable(&mut o, &z1, rz(1));
        let z2: Vec<u8> = self.pos.iter().flat_map(|(x, y)| [x.to_le_bytes(), y.to_le_bytes()].concat()).collect();
        write_ztable(&mut o, &z2, rz(2));
        let z3: Vec<u8> = self.size.iter().flat_map(|(x, y)| [x.to_le_bytes(), y.to_le_bytes()].concat()).collect();
        write_ztable(&mut o, &z3, rz(3));
        o.extend_from_slice(&self.m_u03.to_le_bytes());
        let mut z4 = Vec::new();
        z4.extend_from_slice(&(self.frame_bytes.len() as u32).to_le_bytes());
        for f in &self.frame_bytes {
            z4.extend_from_slice(&(f.len() as u32).to_le_bytes());
            z4.extend_from_slice(f);
        }
        write_ztable(&mut o, &z4, rz(4));
        o.extend_from_slice(&self.tail);
        o
    }

    /// Forget the stored compressed tables (call after editing any of them).
    pub fn mark_edited(&mut self) {
        self.raw_z.clear();
    }
}
