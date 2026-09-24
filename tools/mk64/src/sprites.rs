//! The character art of MK64: the kart sprites (`assets/karts/<char>_kart.json`,
//! 64×64 ci8 frames, each its own MIO0 block, palette = the character's 192
//! colours ++ a per-frame 64-colour wheel palette — the decomp's "stitched
//! palette"), the character-select faces (`assets/character_select/`, 64×64
//! rgba16 MIO0 blocks) and the 32×32 result-screen portraits from the common
//! block (`common_texture_portrait_<char>`). Used by the car skins.
use crate::texture::{decode, decode_ci, mio0_decode, AssetIndex, Image, Rom};
use std::path::Path;

/// The eight drivers: (decomp asset stem, select-face stem, display name).
pub const CHARACTERS: &[(&str, &str, &str)] = &[
    ("mario", "mario", "Mario"),
    ("luigi", "luigi", "Luigi"),
    ("peach", "peach", "Peach"),
    ("toad", "toad", "Toad"),
    ("yoshi", "yoshi", "Yoshi"),
    ("donkeykong", "donkeykong", "Donkey Kong"),
    ("wario", "wario", "Wario"),
    ("bowser", "bowser", "Bowser"),
];

/// The portrait symbol suffix of a character (the common block spells DK out).
pub fn portrait_symbol(stem: &str) -> String {
    let s = if stem == "donkeykong" { "donkey_kong" } else { stem };
    format!("common_texture_portrait_{s}")
}

#[derive(Clone, Debug)]
struct RawLoc {
    rom_offset: u64,
    w: u32,
    h: u32,
    fmt: String,
    tlut: Vec<String>,
}

/// One character's sprite tables, parsed from the decomp's JSON.
pub struct KartSprites {
    pub stem: String,
    frames: Vec<RawLoc>,
    palettes: std::collections::HashMap<String, RawLoc>,
    faces: Vec<RawLoc>,
}

fn parse_hex(s: &str) -> Option<u64> {
    u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()
}

/// A tiny JSON object reader for the decomp's flat tables: `{"name": {"k": v, …}, …}`.
fn parse_table(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    // The files are regular: one entry per `"name": {...}` — split on the top-level braces.
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    // skip to the first '{'
    while i < bytes.len() && bytes[i] != b'{' {
        i += 1;
    }
    i += 1;
    loop {
        // key
        let Some(q1) = text[i..].find('"') else { break };
        let ks = i + q1 + 1;
        let Some(q2) = text[ks..].find('"') else { break };
        let key = text[ks..ks + q2].to_string();
        i = ks + q2 + 1;
        let Some(ob) = text[i..].find('{') else { break };
        let os = i + ob;
        let Some(cb) = text[os..].find('}') else { break };
        let body = &text[os + 1..os + cb];
        i = os + cb + 1;
        // fields; a `"tlut": ["a", "b"]` list has commas inside — parts without a
        // colon continue the pending list value
        let mut merged: Vec<(String, String)> = Vec::new();
        let mut pending: Option<(String, String)> = None;
        for part in body.split(',') {
            if let Some((pk, pv)) = pending.take() {
                let joined = format!("{pv},{}", part.trim());
                if joined.contains(']') {
                    merged.push((pk, joined));
                } else {
                    pending = Some((pk, joined));
                }
                continue;
            }
            let Some((k, v)) = part.split_once(':') else { continue };
            let k = k.trim().trim_matches('"').to_string();
            let v = v.trim().to_string();
            if v.starts_with('[') && !v.contains(']') {
                pending = Some((k, v));
            } else {
                merged.push((k, v));
            }
        }
        if let Some(p) = pending.take() {
            merged.push(p);
        }
        out.push((key, merged));
        if text[i..].trim_start().starts_with('}') {
            break;
        }
    }
    out
}

fn raw_loc(fields: &[(String, String)]) -> Option<RawLoc> {
    let get = |k: &str| fields.iter().find(|(f, _)| f == k).map(|(_, v)| v.clone());
    let rom_offset = parse_hex(get("rom_offset")?.trim_matches('"'))?;
    let w: u32 = get("width")?.parse().ok()?;
    let h: u32 = get("height")?.parse().ok()?;
    let fmt = get("type")?.trim_matches('"').to_string();
    let tlut: Vec<String> = match get("tlut") {
        Some(t) => t
            .trim()
            .trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        None => Vec::new(),
    };
    Some(RawLoc { rom_offset, w, h, fmt, tlut })
}

impl KartSprites {
    pub fn load(decomp: &Path, stem: &str, face_stem: &str) -> Result<KartSprites, String> {
        let kart = std::fs::read_to_string(decomp.join(format!("assets/karts/{stem}_kart.json")))
            .map_err(|e| format!("{stem}_kart.json: {e}"))?;
        let mut frames: Vec<(u32, RawLoc)> = Vec::new();
        let mut palettes = std::collections::HashMap::new();
        let frame_prefix = format!("{stem}_kart_frame");
        for (name, fields) in parse_table(&kart) {
            let Some(loc) = raw_loc(&fields) else { continue };
            if let Some(n) = name.strip_prefix(&frame_prefix) {
                if let Ok(idx) = n.parse::<u32>() {
                    frames.push((idx, loc));
                    continue;
                }
            }
            palettes.insert(name, loc);
        }
        frames.sort_by_key(|(i, _)| *i);
        let frames = frames.into_iter().map(|(_, l)| l).collect();
        let sel = std::fs::read_to_string(decomp.join(format!("assets/character_select/{face_stem}_select.json")))
            .map_err(|e| format!("{face_stem}_select.json: {e}"))?;
        let mut faces: Vec<(u32, RawLoc)> = Vec::new();
        let face_prefix = format!("{face_stem}_face_");
        for (name, fields) in parse_table(&sel) {
            let Some(loc) = raw_loc(&fields) else { continue };
            if let Some(n) = name.strip_prefix(&face_prefix) {
                if let Ok(idx) = n.parse::<u32>() {
                    faces.push((idx, loc));
                }
            }
        }
        faces.sort_by_key(|(i, _)| *i);
        Ok(KartSprites { stem: stem.to_string(), frames, palettes, faces: faces.into_iter().map(|(_, l)| l).collect() })
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    fn raw_bytes(rom: &Rom, loc: &RawLoc) -> Result<Vec<u8>, String> {
        let ro = loc.rom_offset as usize;
        let size = crate::texture::texel_bytes(&loc.fmt, loc.w, loc.h).ok_or("texel format")?;
        if ro + 4 > rom.bytes.len() {
            return Err("offset past the ROM".into());
        }
        if &rom.bytes[ro..ro + 4] == b"MIO0" {
            let dec = mio0_decode(&rom.bytes[ro..])?;
            if dec.len() < size {
                return Err(format!("MIO0 block {} < {size}", dec.len()));
            }
            Ok(dec[..size].to_vec())
        } else {
            if ro + size > rom.bytes.len() {
                return Err("raw asset past the ROM".into());
            }
            Ok(rom.bytes[ro..ro + size].to_vec())
        }
    }

    /// Kart sprite frame `i` as RGBA (transparent where the palette says so).
    pub fn frame(&self, rom: &Rom, i: usize) -> Result<Image, String> {
        let loc = self.frames.get(i).ok_or_else(|| format!("frame {i} of {}", self.frames.len()))?;
        let idx = Self::raw_bytes(rom, loc)?;
        // the stitched palette: every tlut in order (character 192 colours, wheel 64)
        let mut pal: Vec<u8> = Vec::new();
        for t in &loc.tlut {
            let p = self.palettes.get(t).ok_or_else(|| format!("palette {t}"))?;
            pal.extend(Self::raw_bytes(rom, p)?);
        }
        while pal.len() < 512 {
            pal.push(0);
        }
        decode_ci("ci8", loc.w, loc.h, &idx, &pal)
    }

    /// Character-select face `i` (64×64 RGBA).
    pub fn face(&self, rom: &Rom, i: usize) -> Result<Image, String> {
        let loc = self.faces.get(i).ok_or_else(|| format!("face {i} of {}", self.faces.len()))?;
        let raw = Self::raw_bytes(rom, loc)?;
        decode(&loc.fmt, loc.w, loc.h, &raw)
    }

    /// The 32×32 result-screen portrait (common block).
    pub fn portrait(&self, rom: &mut Rom, assets: &AssetIndex) -> Result<Image, String> {
        let sym = portrait_symbol(&self.stem);
        let loc = assets.locate(&sym).ok_or_else(|| format!("{sym} not in the asset index"))?;
        let tl = loc.tlut.as_ref().and_then(|t| assets.locate(t));
        rom.texture(&loc, tl.as_ref())
    }
}

/// A contact sheet: `cols` images per row, each scaled ×`scale` (nearest),
/// on a checker so transparent pixels show.
pub fn contact_sheet(images: &[Image], cols: u32, scale: u32) -> Image {
    if images.is_empty() {
        return Image::solid(1, 1, [0, 0, 0, 255]);
    }
    let cw = images.iter().map(|i| i.w).max().unwrap_or(1) * scale;
    let ch = images.iter().map(|i| i.h).max().unwrap_or(1) * scale;
    let rows = (images.len() as u32 + cols - 1) / cols;
    let mut sheet = Image::solid(cw * cols, ch * rows, [40, 40, 40, 255]);
    for (n, img) in images.iter().enumerate() {
        let ox = (n as u32 % cols) * cw;
        let oy = (n as u32 / cols) * ch;
        for y in 0..img.h * scale {
            for x in 0..img.w * scale {
                let sx = x / scale;
                let sy = y / scale;
                let si = ((sy * img.w + sx) * 4) as usize;
                let a = img.rgba[si + 3];
                let dx = ox + x;
                let dy = oy + y;
                let di = ((dy * sheet.w + dx) * 4) as usize;
                if a > 0 {
                    sheet.rgba[di..di + 4].copy_from_slice(&img.rgba[si..si + 4]);
                    sheet.rgba[di + 3] = 255;
                } else {
                    let c = if ((x / (4 * scale)) + (y / (4 * scale))) % 2 == 0 { 70 } else { 100 };
                    sheet.rgba[di..di + 4].copy_from_slice(&[c, c, c, 255]);
                }
            }
        }
    }
    sheet
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tlut_list_keeps_order() {
        let t = r#"{"mario_kart_frame000": {"output_dir": "mario/frames", "rom_offset": "0x1E01F0", "width": 64, "height": 64, "type": "ci8", "tlut": ["mario_kart_palette", "kart_000_wheel_0"], "meta": ["stitched_palette"]},
"kart_000_wheel_0": {"output_dir": "mario/palettes", "rom_offset": "0x24E6A0", "width": 16, "height": 4, "type": "rgba16"}}"#;
        let rows = parse_table(t);
        assert_eq!(rows.len(), 2);
        let loc = raw_loc(&rows[0].1).unwrap();
        assert_eq!(loc.tlut, vec!["mario_kart_palette".to_string(), "kart_000_wheel_0".to_string()]);
        assert_eq!(loc.rom_offset, 0x1E01F0);
        assert_eq!(raw_loc(&rows[1].1).unwrap().rom_offset, 0x24E6A0);
    }
}

#[cfg(test)]
mod rom_tests {
    use super::*;
    #[test]
    fn mario_frame0_is_red() {
        let Ok(decomp) = std::env::var("MK64_DECOMP") else { return };
        let Ok(romp) = std::env::var("MK64_ROM") else { return };
        let rom = Rom::load(Path::new(&romp)).unwrap();
        let ks = KartSprites::load(Path::new(&decomp), "mario", "mario").unwrap();
        let loc = &ks.frames[0];
        eprintln!("frame0 {:x?} tlut {:?}", loc.rom_offset, loc.tlut);
        for t in &loc.tlut {
            let p = ks.palettes.get(t).unwrap();
            let b = KartSprites::raw_bytes(&rom, p).unwrap();
            eprintln!("{t}: off {:x} {}x{} {} -> {} bytes, first {:02x?}", p.rom_offset, p.w, p.h, p.fmt, b.len(), &b[..8]);
        }
        let img = ks.frame(&rom, 0).unwrap();
        let mut hist = std::collections::HashMap::new();
        for p in img.rgba.chunks(4) {
            if p[3] > 0 {
                *hist.entry([p[0], p[1], p[2]]).or_insert(0u32) += 1;
            }
        }
        let mut v: Vec<_> = hist.into_iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        eprintln!("top colours {:?}", &v[..v.len().min(6)]);
        assert!(v.iter().take(6).any(|(c, _)| c[0] > 200 && c[1] < 60), "no red among the top colours");
    }
}
