//! DDS textures as the tree bake ships them next to the items: the pack's
//! image with its top mip levels cut off, so a 2048×1024 leaf atlas (2.8 MB
//! as DXT5) rides as its 512×256 level (175 KB with the rest of the chain) —
//! a half-size tree seen from a car never resolves more, and the map upload
//! cap (~25 MB) leaves no room for the originals.
//!
//! Header: `DDS ` magic, u32 124, flags, height (12), width (16),
//! pitch-or-linear-size (20), depth, mipmap count (28), reserved, then the
//! pixel format (size 32 at 76: flags at 80, fourCC at 84, rgb bit count at
//! 88, masks), caps at 108. A `DX10` fourCC adds a 20-byte extension
//! (DXGI format at 128).

use super::R;

/// Bytes of one mip level of `w`×`h` in the file's format.
fn level_bytes(dds: &[u8], w: u32, h: u32) -> R<usize> {
    let fourcc = &dds[84..88];
    let pf_flags = u32::from_le_bytes([dds[80], dds[81], dds[82], dds[83]]);
    let blocks = || (w.max(1).div_ceil(4) as usize) * (h.max(1).div_ceil(4) as usize);
    if pf_flags & 0x4 != 0 {
        // FOURCC
        return match fourcc {
            b"DXT1" | b"ATI1" | b"BC4U" | b"BC4S" => Ok(blocks() * 8),
            b"DXT2" | b"DXT3" | b"DXT4" | b"DXT5" | b"ATI2" | b"BC5U" | b"BC5S" => Ok(blocks() * 16),
            b"DX10" => {
                let dxgi = u32::from_le_bytes([dds[128], dds[129], dds[130], dds[131]]);
                match dxgi {
                    // BC1/BC4 (8 bytes a block), BC2/3/5/6/7 (16)
                    70..=72 | 79..=81 => Ok(blocks() * 8),
                    73..=78 | 82..=84 | 94..=99 => Ok(blocks() * 16),
                    28 | 87 | 88 => Ok((w.max(1) * h.max(1) * 4) as usize),
                    _ => Err(format!("DX10 format {dxgi} has no size rule")),
                }
            }
            _ => Err(format!("fourCC {} has no size rule", String::from_utf8_lossy(fourcc))),
        };
    }
    // uncompressed: the rgb bit count
    let bpp = u32::from_le_bytes([dds[88], dds[89], dds[90], dds[91]]);
    if bpp == 0 {
        return Err("uncompressed DDS without a bit count".into());
    }
    Ok(((w.max(1) * bpp).div_ceil(8) * h.max(1)) as usize)
}

/// Whether the file is a DDS with a 128-byte (or DX10 148-byte) header.
pub fn is_dds(dds: &[u8]) -> bool {
    dds.len() >= 128 && &dds[0..4] == b"DDS " && u32::from_le_bytes([dds[4], dds[5], dds[6], dds[7]]) == 124
}

/// (width, height, mip count) of a DDS.
pub fn dds_dims(dds: &[u8]) -> Option<(u32, u32, u32)> {
    if !is_dds(dds) {
        return None;
    }
    let u = |o: usize| u32::from_le_bytes([dds[o], dds[o + 1], dds[o + 2], dds[o + 3]]);
    Some((u(16), u(12), u(28).max(1)))
}

/// The DDS with its levels above `max_side` pixels (on the larger side)
/// removed: the first level kept becomes the top, the mip count and the
/// linear size follow. A texture already within the cap, or without enough
/// levels to cut, comes back as it is. The DDS header's own words say what
/// each level weighs, so the cut is exact for every block-compressed and
/// uncompressed format the rule table knows.
pub fn dds_cap(dds: &[u8], max_side: u32) -> R<Vec<u8>> {
    if !is_dds(dds) {
        return Err("not a DDS file".into());
    }
    let u = |o: usize| u32::from_le_bytes([dds[o], dds[o + 1], dds[o + 2], dds[o + 3]]);
    let (mut h, mut w, mips) = (u(12), u(16), u(28).max(1));
    let header_len = if &dds[84..88] == b"DX10" { 148 } else { 128 };
    if dds.len() < header_len {
        return Err("DDS header cut short".into());
    }
    let mut skip = 0usize;
    let mut cut = 0u32;
    while w.max(h) > max_side && cut + 1 < mips {
        skip += level_bytes(dds, w, h)?;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        cut += 1;
    }
    if cut == 0 {
        return Ok(dds.to_vec());
    }
    if header_len + skip > dds.len() {
        return Err(format!("the levels to cut weigh {skip} bytes, the file has {}", dds.len() - header_len));
    }
    let mut out = Vec::with_capacity(dds.len() - skip);
    out.extend_from_slice(&dds[..header_len]);
    out.extend_from_slice(&dds[header_len + skip..]);
    let put = |out: &mut Vec<u8>, o: usize, v: u32| out[o..o + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut out, 12, h);
    put(&mut out, 16, w);
    put(&mut out, 28, mips - cut);
    // pitch or linear size: the top level's byte size for a compressed
    // format (flags bit 0x80000 LINEARSIZE), else the row pitch
    let flags = u(8);
    let top = level_bytes(dds, w, h)?;
    if flags & 0x0008_0000 != 0 {
        put(&mut out, 20, top as u32);
    } else if flags & 0x8 != 0 {
        put(&mut out, 20, (top / h.max(1) as usize) as u32);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dxt1(w: u32, h: u32, mips: u32) -> Vec<u8> {
        let mut d = vec![0u8; 128];
        d[0..4].copy_from_slice(b"DDS ");
        d[4..8].copy_from_slice(&124u32.to_le_bytes());
        d[8..12].copy_from_slice(&(0x1 | 0x2 | 0x4 | 0x1000 | 0x20000 | 0x80000u32).to_le_bytes());
        d[12..16].copy_from_slice(&h.to_le_bytes());
        d[16..20].copy_from_slice(&w.to_le_bytes());
        d[28..32].copy_from_slice(&mips.to_le_bytes());
        d[76..80].copy_from_slice(&32u32.to_le_bytes());
        d[80..84].copy_from_slice(&4u32.to_le_bytes());
        d[84..88].copy_from_slice(b"DXT1");
        let (mut lw, mut lh) = (w, h);
        for level in 0..mips {
            let n = (lw.max(1).div_ceil(4) * lh.max(1).div_ceil(4) * 8) as usize;
            d.extend(std::iter::repeat(level as u8).take(n));
            lw = (lw / 2).max(1);
            lh = (lh / 2).max(1);
        }
        d
    }

    #[test]
    fn cutting_keeps_the_lower_levels_intact() {
        let d = dxt1(2048, 128, 12);
        let c = dds_cap(&d, 512).unwrap();
        assert_eq!(dds_dims(&c), Some((512, 32, 10)));
        // the new top level is the old level 2 (bytes 2)
        assert!(c[128..128 + 512 / 4 * 32 / 4 * 8].iter().all(|b| *b == 2));
        assert_eq!(c.len(), 128 + d[128..].len() - (2048 / 4 * 128 / 4 * 8 + 1024 / 4 * 64 / 4 * 8));
        assert_eq!(u32::from_le_bytes([c[20], c[21], c[22], c[23]]), 512 / 4 * 32 / 4 * 8);
    }

    #[test]
    fn a_small_texture_is_untouched() {
        let d = dxt1(256, 256, 9);
        assert_eq!(dds_cap(&d, 512).unwrap(), d);
    }
}
