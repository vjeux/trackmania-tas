//! Reading the lightmap chunk out of a `.Map.Gbx` and writing one back.

use crate::format::LightmapChunk;
use crate::find_chunk;

pub struct MapLightmap {
    pub gbx: gbx::Gbx,
    /// (chunk offset, payload offset, payload size) in `gbx.body`.
    pub at: (usize, usize, usize),
    pub chunk: LightmapChunk,
}

pub fn load(path: &str) -> Result<MapLightmap, String> {
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let gbx = gbx::Gbx::parse(&data);
    let at = find_chunk(&gbx.body).ok_or_else(|| format!("{path}: no lightmap chunk 0x0304305B"))?;
    let chunk = LightmapChunk::parse(&gbx.body[at.1..at.1 + at.2]).map_err(|e| format!("{path}: {e}"))?;
    Ok(MapLightmap { gbx, at, chunk })
}

/// The map with its lightmap chunk replaced by `payload` (the body is written
/// uncompressed, like every other write path in this repo).
pub fn save_with_chunk(m: &MapLightmap, payload: &[u8], out: &str) -> Result<(), String> {
    let (off, p, size) = m.at;
    let body = &m.gbx.body;
    let mut nb = Vec::with_capacity(body.len() + payload.len());
    nb.extend_from_slice(&body[..off + 8]);
    nb.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    nb.extend_from_slice(payload);
    nb.extend_from_slice(&body[p + size..]);
    gbx::container::write_gbx(&m.gbx, nb, out)
}
