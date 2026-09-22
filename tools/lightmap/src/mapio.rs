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

/// The map with its lightmap chunk replaced by `payload`. The body is written
/// LZO-COMPRESSED (the shipped form — `tmmaps::gbx::Gbx::write_body_recompressed`,
/// the path `tinyctl lightmap` transplants through): a giant map's uncompressed
/// body runs over Nadeo's 25 MiB cap (Summer 01 ×2: 24.7 MB uncompressed, 15 MB
/// compressed). `LMTOOL_UNCOMPRESSED=1` keeps the old uncompressed write (byte
/// comparisons in tests).
pub fn save_with_chunk(m: &MapLightmap, payload: &[u8], out: &str) -> Result<(), String> {
    let (off, p, size) = m.at;
    let body = &m.gbx.body;
    let mut nb = Vec::with_capacity(body.len() + payload.len());
    nb.extend_from_slice(&body[..off + 8]);
    nb.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    nb.extend_from_slice(payload);
    nb.extend_from_slice(&body[p + size..]);
    if std::env::var("LMTOOL_UNCOMPRESSED").map(|v| v == "1").unwrap_or(false) {
        return gbx::container::write_gbx(&m.gbx, nb, out);
    }
    // the tmmaps container knows the LZO writer; rebuild its view of the same file
    let uncompressed = {
        let mut f = m.gbx.header_bytes_u();
        f.extend_from_slice(body);
        f
    };
    let t = tmmaps::gbx::Gbx::parse(&uncompressed);
    let file = t.write_body_recompressed(&nb);
    std::fs::write(out, file).map_err(|e| format!("{out}: {e}"))
}

/// A template: a `.Map.Gbx` (its lightmap chunk) or a raw `.lmchunk` file
/// (the chunk payload alone, as `lmtool dump` writes `chunk.bin`).
pub struct Template {
    pub chunk: LightmapChunk,
}

pub fn load_template(path: &str) -> Result<Template, String> {
    if path.ends_with(".lmchunk") || path.ends_with(".bin") {
        let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let chunk = LightmapChunk::parse(&data).map_err(|e| format!("{path}: {e}"))?;
        return Ok(Template { chunk });
    }
    let m = load(path)?;
    Ok(Template { chunk: m.chunk })
}
