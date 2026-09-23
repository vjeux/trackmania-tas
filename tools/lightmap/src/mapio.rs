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

/// The map's DayTime word from chunk 0x03043056 (None when the chunk is absent).
/// The map's DayTime word. The editor reads chunk **0x0304306B** `{u32 0, u32 DayTime, u32 0, u32 0,
/// u32 300000}` (found 2026-09-23 12:55Z: every DayTime variant that patched only 0x03043056 and the
/// lightmap records still baked at this chunk's word); the legacy 0x03043056 `{version 3, u32, DayTime,
/// dynamic, duration}` carries the same value on Nadeo's files and is the fallback.
pub fn daytime(body: &[u8]) -> Option<u32> {
    let cs = tmmaps::gbx::all_skip_chunks(body);
    if let Some(c) = cs.iter().find(|c| c.0 == 0x0304306B && c.3 >= 8) {
        let p = c.2;
        return Some(u32::from_le_bytes([body[p + 4], body[p + 5], body[p + 6], body[p + 7]]));
    }
    let c = cs.iter().find(|c| c.0 == 0x03043056)?;
    let p = c.2;
    Some(u32::from_le_bytes([body[p + 8], body[p + 9], body[p + 10], body[p + 11]]))
}

/// The offsets (in `body`) of every DayTime word: 0x0304306B at +4 and 0x03043056 at +8.
pub fn daytime_word_offsets(body: &[u8]) -> Vec<usize> {
    let cs = tmmaps::gbx::all_skip_chunks(body);
    let mut out = Vec::new();
    for c in &cs {
        if c.0 == 0x0304306B && c.3 >= 8 { out.push(c.2 + 4); }
        if c.0 == 0x03043056 && c.3 >= 12 { out.push(c.2 + 8); }
    }
    out
}
