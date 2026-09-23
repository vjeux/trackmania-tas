//! The ROM side: MIO0 decompression, the asset index (`assets.json` + the
//! per-course jsons + the symbol table in `tools/linkonly_generator.py`), and
//! the N64 texel formats → RGBA8.

use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Image {
        Image { w, h, rgba: rgba.iter().copied().cycle().take((w * h * 4) as usize).collect() }
    }
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y % self.h) * self.w + (x % self.w)) as usize * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
    /// Nearest-neighbour upscale by an integer factor.
    pub fn upscale(&self, f: u32) -> Image {
        let (w, h) = (self.w * f, self.h * f);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&self.pixel(x / f, y / f));
            }
        }
        Image { w, h, rgba }
    }
    /// The image mirrored into a 2× tile along the axes asked for (the N64
    /// `G_TX_MIRROR` wrap: texel u in [w, 2w) reads w-1-(u-w)).
    pub fn mirrored(&self, s: bool, t: bool) -> Image {
        let (w, h) = (if s { self.w * 2 } else { self.w }, if t { self.h * 2 } else { self.h });
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let sy = if y >= self.h { 2 * self.h - 1 - y } else { y };
            for x in 0..w {
                let sx = if x >= self.w { 2 * self.w - 1 - x } else { x };
                rgba.extend_from_slice(&self.pixel(sx, sy));
            }
        }
        Image { w, h, rgba }
    }
    /// Multiplied by a colour (the N64 vertex-colour modulation, baked).
    pub fn tinted(&self, tint: [u8; 3]) -> Image {
        if tint == [255, 255, 255] {
            return self.clone();
        }
        let mut rgba = self.rgba.clone();
        for p in rgba.chunks_mut(4) {
            for k in 0..3 {
                p[k] = ((p[k] as u32 * tint[k] as u32 + 127) / 255) as u8;
            }
        }
        Image { w: self.w, h: self.h, rgba }
    }
    pub fn has_alpha(&self) -> bool {
        self.rgba.chunks(4).any(|p| p[3] < 250)
    }
}

/// Standard N64 MIO0: `MIO0`, BE lengths, a layout bitstream, raw and
/// compressed byte streams.
pub fn mio0_decode(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 16 || &data[0..4] != b"MIO0" {
        return Err("not a MIO0 block".into());
    }
    let be = |o: usize| u32::from_be_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as usize;
    let out_len = be(4);
    let mut comp = be(8);
    let mut raw = be(12);
    let mut out = Vec::with_capacity(out_len);
    let mut bits_pos = 16usize;
    let mut bit = 0u32;
    let mut bits_left = 0u32;
    while out.len() < out_len {
        if bits_left == 0 {
            if bits_pos + 4 > data.len() {
                return Err("MIO0 layout stream truncated".into());
            }
            bit = u32::from_be_bytes([data[bits_pos], data[bits_pos + 1], data[bits_pos + 2], data[bits_pos + 3]]);
            bits_pos += 4;
            bits_left = 32;
        }
        let take_raw = bit & 0x8000_0000 != 0;
        bit <<= 1;
        bits_left -= 1;
        if take_raw {
            if raw >= data.len() {
                return Err("MIO0 raw stream truncated".into());
            }
            out.push(data[raw]);
            raw += 1;
        } else {
            if comp + 1 >= data.len() {
                return Err("MIO0 compressed stream truncated".into());
            }
            let word = u16::from_be_bytes([data[comp], data[comp + 1]]) as usize;
            comp += 2;
            let len = (word >> 12) + 3;
            let dist = (word & 0xFFF) + 1;
            if dist > out.len() {
                return Err("MIO0 back-reference before start".into());
            }
            let start = out.len() - dist;
            for k in 0..len {
                let b = out[start + k];
                out.push(b);
            }
        }
    }
    out.truncate(out_len);
    Ok(out)
}

/// Where a texture lives in the ROM.
#[derive(Clone, Debug)]
pub struct AssetLoc {
    pub rom_offset: u64,
    pub block_offset: u64,
    pub w: u32,
    pub h: u32,
    pub fmt: String,
    /// The palette symbol of a colour-indexed (`ci8`/`ci4`) texture.
    pub tlut: Option<String>,
}

/// Symbol → asset location, over the three tables of the decomp.
#[derive(Default, Debug)]
pub struct AssetIndex {
    /// `assets.json`: "textures/standalone/road_1.rgba16.png" → loc
    pub by_path: HashMap<String, AssetLoc>,
    /// linkonly table: gTextureRoad1 → ("road_1", "rgba16")
    pub symbols: HashMap<String, (String, String)>,
    /// per-course jsons: gTextureLuigiRacewaySignLeft → loc
    pub course_syms: HashMap<String, AssetLoc>,
}

impl AssetIndex {
    pub fn load(decomp: &Path) -> Result<AssetIndex, String> {
        let mut ix = AssetIndex::default();
        let assets = std::fs::read_to_string(decomp.join("assets.json")).map_err(|e| format!("assets.json: {e}"))?;
        for line in assets.lines() {
            let line = line.trim();
            if !line.starts_with('"') || !line.contains("\"offsets\"") {
                continue;
            }
            let key = match line[1..].find('"') {
                Some(e) => line[1..1 + e].to_string(),
                None => continue,
            };
            let dims = between(line, "\"dims\":[", "]").and_then(|d| {
                let mut it = d.split(',').map(|x| x.trim().parse::<u32>().ok());
                Some((it.next()??, it.next()??))
            });
            let us = between(line, "\"us\":[", "]").map(|s| s.split(',').map(|x| x.trim().trim_matches('"').to_string()).collect::<Vec<_>>());
            let (w, h) = match dims {
                Some(d) => d,
                None => continue,
            };
            let us = match us {
                Some(u) if u.len() >= 2 => u,
                _ => continue,
            };
            let fmt = key.rsplit('.').nth(1).unwrap_or("").to_string();
            ix.by_path.insert(key, AssetLoc { rom_offset: hex(&us[0]), block_offset: hex(&us[1]), w, h, fmt, tlut: None });
        }
        let gen = std::fs::read_to_string(decomp.join("tools/linkonly_generator.py")).map_err(|e| format!("linkonly_generator.py: {e}"))?;
        for line in gen.lines() {
            let l = line.trim();
            if !l.starts_with("\"gTexture") || !l.contains("):") && !l.contains("(\"") {
                continue;
            }
            // "gTextureRoad1":                  ("road_1",                      "rgba16"),
            let sym = between(l, "\"", "\"").unwrap_or("").to_string();
            let rest = &l[l.find('(').map(|p| p + 1).unwrap_or(l.len())..];
            let parts: Vec<String> = rest.split(',').map(|x| x.trim().trim_matches(|c| c == '"' || c == ')' || c == ',').to_string()).collect();
            if parts.len() >= 2 && !sym.is_empty() {
                ix.symbols.insert(sym, (parts[0].clone(), parts[1].clone()));
            }
        }
        let mut json_files: Vec<std::path::PathBuf> = Vec::new();
        // assets/*.json and every assets/<dir>/*.json (courses, lakitu, karts…)
        if let Ok(rd) = std::fs::read_dir(decomp.join("assets")) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    if let Ok(rd2) = std::fs::read_dir(&p) {
                        json_files.extend(rd2.flatten().map(|e| e.path()));
                    }
                } else {
                    json_files.push(p);
                }
            }
        }
        {
            for p in json_files {
                if p.extension().map(|x| x == "json").unwrap_or(false) {
                    if let Ok(txt) = std::fs::read_to_string(&p) {
                        for line in txt.lines() {
                            let l = line.trim();
                            if !(l.starts_with("\"gT") || l.starts_with("\"common_") || l.starts_with("\"texture_") || l.starts_with("\"minimap_")) {
                                continue;
                            }
                            let sym = between(l, "\"", "\"").unwrap_or("").to_string();
                            let ro = between(l, "\"rom_offset\": \"", "\"").map(hex);
                            let bo = between(l, "\"block_offset\": \"", "\"").map(hex).unwrap_or(0);
                            let w = between(l, "\"width\": ", ",").and_then(|x| x.trim().parse().ok());
                            let h = between(l, "\"height\": ", ",").and_then(|x| x.trim().parse().ok());
                            let fmt = between(l, "\"type\": \"", "\"").unwrap_or("").to_string();
                            let tlut = between(l, "\"tlut\": \"", "\"").map(|s| s.to_string());
                            if let (Some(ro), Some(w), Some(h)) = (ro, w, h) {
                                ix.course_syms.insert(sym, AssetLoc { rom_offset: ro, block_offset: bo, w, h, fmt, tlut });
                            }
                        }
                    }
                }
            }
        }
        Ok(ix)
    }

    /// A display list names textures through the course's own aliases
    /// (`gLRTextureRoad1` = Luigi Raceway's copy of `gTextureRoad1`, the
    /// linkonly generator's `g<ABBR>Texture<Name>`): resolve the alias first.
    pub fn locate(&self, sym: &str) -> Option<AssetLoc> {
        if let Some(l) = self.locate_exact(sym) {
            return Some(l);
        }
        let i = sym.find("Texture")?;
        if !sym.starts_with('g') || i <= 1 {
            return None;
        }
        self.locate_exact(&format!("gTexture{}", &sym[i + "Texture".len()..]))
    }

    fn locate_exact(&self, sym: &str) -> Option<AssetLoc> {
        if let Some(l) = self.course_syms.get(sym) {
            return Some(l.clone());
        }
        let (stem, fmt) = self.symbols.get(sym)?;
        let suffix = format!("/{stem}.{fmt}.png");
        self.by_path.iter().find(|(k, _)| k.ends_with(&suffix)).map(|(_, v)| v.clone())
    }
}

fn between<'a>(s: &'a str, a: &str, b: &str) -> Option<&'a str> {
    let i = s.find(a)? + a.len();
    let j = s[i..].find(b)? + i;
    Some(&s[i..j])
}

fn hex(s: &str) -> u64 {
    let t = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    u64::from_str_radix(t, 16).unwrap_or(0)
}

/// The ROM with a cache of decompressed MIO0 blocks.
pub struct Rom {
    pub bytes: Vec<u8>,
    blocks: HashMap<u64, Vec<u8>>,
}

impl Rom {
    pub fn load(path: &Path) -> Result<Rom, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if bytes.len() < 0x40 || bytes[0..4] != [0x80, 0x37, 0x12, 0x40] {
            return Err("not a big-endian .z64 ROM".into());
        }
        Ok(Rom { bytes, blocks: HashMap::new() })
    }

    /// The raw texel bytes of an asset.
    pub fn asset_bytes(&mut self, loc: &AssetLoc) -> Result<Vec<u8>, String> {
        let size = texel_bytes(&loc.fmt, loc.w, loc.h).ok_or_else(|| format!("unknown texel format {}", loc.fmt))?;
        let ro = loc.rom_offset as usize;
        if ro + 4 > self.bytes.len() {
            return Err("asset offset past the ROM".into());
        }
        if &self.bytes[ro..ro + 4] == b"MIO0" {
            if !self.blocks.contains_key(&loc.rom_offset) {
                let dec = mio0_decode(&self.bytes[ro..])?;
                self.blocks.insert(loc.rom_offset, dec);
            }
            let block = &self.blocks[&loc.rom_offset];
            let bo = loc.block_offset as usize;
            if bo + size > block.len() {
                return Err(format!("asset past its MIO0 block ({bo}+{size} > {})", block.len()));
            }
            Ok(block[bo..bo + size].to_vec())
        } else if &self.bytes[ro..ro + 4] == b"TKMK" {
            Err("TKMK00-compressed asset (not a course texture)".into())
        } else {
            let start = ro + loc.block_offset as usize;
            if start + size > self.bytes.len() {
                return Err("raw asset past the ROM".into());
            }
            Ok(self.bytes[start..start + size].to_vec())
        }
    }

    /// A texture as RGBA8; a colour-indexed one needs its palette (`tlut`,
    /// an rgba16 asset of 16×16 or 8×29 entries).
    pub fn texture(&mut self, loc: &AssetLoc, tlut: Option<&AssetLoc>) -> Result<Image, String> {
        let raw = self.asset_bytes(loc)?;
        if loc.fmt == "ci8" || loc.fmt == "ci4" {
            let t = tlut.ok_or_else(|| format!("{} texture without a palette", loc.fmt))?;
            let pal = self.asset_bytes(t)?;
            return decode_ci(&loc.fmt, loc.w, loc.h, &raw, &pal);
        }
        decode(&loc.fmt, loc.w, loc.h, &raw)
    }
}

pub fn texel_bytes(fmt: &str, w: u32, h: u32) -> Option<usize> {
    let n = (w * h) as usize;
    Some(match fmt {
        "rgba16" | "ia16" => n * 2,
        "rgba32" => n * 4,
        "ci8" | "ia8" | "i8" => n,
        "ci4" | "ia4" | "i4" => (n + 1) / 2,
        "ia1" => (n + 7) / 8,
        _ => return None,
    })
}

/// N64 texel formats → RGBA8 (CI formats need a palette and come back as an error).
pub fn decode(fmt: &str, w: u32, h: u32, raw: &[u8]) -> Result<Image, String> {
    let n = (w * h) as usize;
    let mut rgba = Vec::with_capacity(n * 4);
    match fmt {
        "rgba16" => {
            for i in 0..n {
                let v = u16::from_be_bytes([raw[2 * i], raw[2 * i + 1]]);
                let r = ((v >> 11) & 0x1F) as u8;
                let g = ((v >> 6) & 0x1F) as u8;
                let b = ((v >> 1) & 0x1F) as u8;
                let a = (v & 1) as u8;
                rgba.extend_from_slice(&[(r << 3) | (r >> 2), (g << 3) | (g >> 2), (b << 3) | (b >> 2), if a == 1 { 255 } else { 0 }]);
            }
        }
        "rgba32" => rgba.extend_from_slice(&raw[..n * 4]),
        "ia16" => {
            for i in 0..n {
                let (l, a) = (raw[2 * i], raw[2 * i + 1]);
                rgba.extend_from_slice(&[l, l, l, a]);
            }
        }
        "ia8" => {
            for i in 0..n {
                let l = (raw[i] >> 4) * 17;
                let a = (raw[i] & 0xF) * 17;
                rgba.extend_from_slice(&[l, l, l, a]);
            }
        }
        "ia4" => {
            for i in 0..n {
                let b = raw[i / 2];
                let nib = if i % 2 == 0 { b >> 4 } else { b & 0xF };
                let l = (nib >> 1) * 36;
                let a = if nib & 1 == 1 { 255 } else { 0 };
                rgba.extend_from_slice(&[l, l, l, a]);
            }
        }
        "i8" => {
            for i in 0..n {
                let l = raw[i];
                rgba.extend_from_slice(&[l, l, l, l]);
            }
        }
        "i4" => {
            for i in 0..n {
                let b = raw[i / 2];
                let nib = if i % 2 == 0 { b >> 4 } else { b & 0xF };
                let l = nib * 17;
                rgba.extend_from_slice(&[l, l, l, l]);
            }
        }
        other => return Err(format!("texel format {other} not decoded (palette formats need their TLUT)")),
    }
    Ok(Image { w, h, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mio0_roundtrip_of_a_known_stream() {
        // "abcabcabcabc": 3 raw bytes then a back-reference of length 9, dist 3
        let mut blob = Vec::new();
        blob.extend_from_slice(b"MIO0");
        blob.extend_from_slice(&12u32.to_be_bytes());
        blob.extend_from_slice(&20u32.to_be_bytes()); // comp offset
        blob.extend_from_slice(&22u32.to_be_bytes()); // raw offset
        blob.extend_from_slice(&0b1110_0000_0000_0000_0000_0000_0000_0000u32.to_be_bytes()); // layout: 3 raw, then a copy
        let word: u16 = ((9 - 3) << 12) as u16 | (3 - 1);
        blob.extend_from_slice(&word.to_be_bytes());
        blob.extend_from_slice(b"abc");
        assert_eq!(mio0_decode(&blob).unwrap(), b"abcabcabcabc");
    }

    #[test]
    fn rgba16_decodes_opaque_white_and_transparent_black() {
        let img = decode("rgba16", 2, 1, &[0xFF, 0xFF, 0x00, 0x00]).unwrap();
        assert_eq!(img.pixel(0, 0), [255, 255, 255, 255]);
        assert_eq!(img.pixel(1, 0), [0, 0, 0, 0]);
    }
}

/// Colour-indexed texels through an rgba16 palette.
pub fn decode_ci(fmt: &str, w: u32, h: u32, raw: &[u8], pal: &[u8]) -> Result<Image, String> {
    let n = (w * h) as usize;
    let entries = pal.len() / 2;
    let mut rgba = Vec::with_capacity(n * 4);
    for i in 0..n {
        let idx = match fmt {
            "ci8" => raw[i] as usize,
            "ci4" => {
                let b = raw[i / 2];
                (if i % 2 == 0 { b >> 4 } else { b & 0xF }) as usize
            }
            other => return Err(format!("{other} is not a colour-indexed format")),
        };
        if idx >= entries {
            rgba.extend_from_slice(&[255, 0, 255, 255]);
            continue;
        }
        let v = u16::from_be_bytes([pal[2 * idx], pal[2 * idx + 1]]);
        let r = ((v >> 11) & 0x1F) as u8;
        let g = ((v >> 6) & 0x1F) as u8;
        let b = ((v >> 1) & 0x1F) as u8;
        let a = (v & 1) as u8;
        rgba.extend_from_slice(&[(r << 3) | (r >> 2), (g << 3) | (g >> 2), (b << 3) | (b >> 2), if a == 1 { 255 } else { 0 }]);
    }
    Ok(Image { w, h, rgba })
}
