//! `lightmap` — the map lightmap chunk `0x0304305B` (CGameCtnChallenge
//! "LightMap": the WEBP atlases + the `CHmsLightMapCache` node).
//!
//! Work in progress: the walker first, then the typed structure.

pub mod bake;
pub mod bvh;
pub mod format;
pub mod geometry;
pub mod img;
pub mod mapio;
pub mod probe;
pub mod synth;
pub mod volume;
pub mod vp8_tables;
pub mod vp8enc;
pub mod walk;

pub const LIGHTMAP_CHUNK: u32 = 0x0304_305B;

/// Little-endian cursor over a byte slice with bounds-checked reads that
/// report an error instead of panicking (the layout is being discovered, so
/// every misparse must be a message, not a crash).
pub struct Cur<'a> {
    pub b: &'a [u8],
    pub o: usize,
}

impl<'a> Cur<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Cur { b, o: 0 }
    }
    pub fn left(&self) -> usize {
        self.b.len().saturating_sub(self.o)
    }
    pub fn u8(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.o).ok_or_else(|| format!("eof at {:#x} (u8)", self.o))?;
        self.o += 1;
        Ok(v)
    }
    pub fn u16(&mut self) -> Result<u16, String> {
        let s = self.b.get(self.o..self.o + 2).ok_or_else(|| format!("eof at {:#x} (u16)", self.o))?;
        self.o += 2;
        Ok(u16::from_le_bytes(s.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, String> {
        let s = self.b.get(self.o..self.o + 4).ok_or_else(|| format!("eof at {:#x} (u32)", self.o))?;
        self.o += 4;
        Ok(u32::from_le_bytes(s.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }
    pub fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn u64(&mut self) -> Result<u64, String> {
        let s = self.b.get(self.o..self.o + 8).ok_or_else(|| format!("eof at {:#x} (u64)", self.o))?;
        self.o += 8;
        Ok(u64::from_le_bytes(s.try_into().unwrap()))
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self
            .b
            .get(self.o..self.o.saturating_add(n))
            .ok_or_else(|| format!("eof at {:#x} taking {n} bytes ({} left)", self.o, self.left()))?;
        self.o += n;
        Ok(s)
    }
    /// A GBX string: u32 length + bytes.
    pub fn string(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        if n > 0x10000 {
            return Err(format!("string length {n} at {:#x} is not a string", self.o - 4));
        }
        let s = self.take(n)?;
        Ok(String::from_utf8_lossy(s).into_owned())
    }
}

pub fn hexdump(b: &[u8], base: usize, max: usize) -> String {
    let mut s = String::new();
    for (n, ch) in b.chunks(32).enumerate().take(max.div_ceil(32)) {
        let hex: String = ch.iter().map(|x| format!("{x:02x}")).collect();
        let asc: String = ch.iter().map(|&x| if (32..127).contains(&x) { x as char } else { '.' }).collect();
        s.push_str(&format!("{:06x}  {hex:<64}  {asc}\n", base + n * 32));
    }
    s
}

/// Find the lightmap chunk in a map file body: (chunk offset, payload offset, size).
pub fn find_chunk(body: &[u8]) -> Option<(usize, usize, usize)> {
    gbx::all_skip_chunks(body)
        .into_iter()
        .find(|c| c.0 == LIGHTMAP_CHUNK)
        .map(|c| (c.1, c.2, c.3))
}

pub fn zlib_inflate(data: &[u8], expect: usize) -> Result<Vec<u8>, String> {
    let out = miniz_oxide::inflate::decompress_to_vec_zlib(data).map_err(|e| format!("zlib: {e:?}"))?;
    if expect != 0 && out.len() != expect {
        return Err(format!("zlib: inflated {} bytes, expected {expect}", out.len()));
    }
    Ok(out)
}
