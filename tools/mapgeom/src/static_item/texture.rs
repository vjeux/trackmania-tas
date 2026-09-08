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

/// The 16 colours of one BC1 block (5:6:5 endpoints, 2-bit indices; the
/// `c0 <= c1` mode's fourth entry is transparent black).
fn bc1_block(block: &[u8], out: &mut [[u8; 4]; 16]) {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let c565 = |v: u16| -> [u8; 3] {
        let r = ((v >> 11) & 31) as u32;
        let g = ((v >> 5) & 63) as u32;
        let b = (v & 31) as u32;
        [((r * 255 + 15) / 31) as u8, ((g * 255 + 31) / 63) as u8, ((b * 255 + 15) / 31) as u8]
    };
    let (p0, p1) = (c565(c0), c565(c1));
    let mix = |a: [u8; 3], b: [u8; 3], wa: u32, wb: u32| -> [u8; 3] { [((a[0] as u32 * wa + b[0] as u32 * wb) / (wa + wb)) as u8, ((a[1] as u32 * wa + b[1] as u32 * wb) / (wa + wb)) as u8, ((a[2] as u32 * wa + b[2] as u32 * wb) / (wa + wb)) as u8] };
    let (p2, p3, a3) = if c0 > c1 { (mix(p0, p1, 2, 1), mix(p0, p1, 1, 2), 255u8) } else { (mix(p0, p1, 1, 1), [0, 0, 0], 0u8) };
    let pal = [[p0[0], p0[1], p0[2], 255], [p1[0], p1[1], p1[2], 255], [p2[0], p2[1], p2[2], 255], [p3[0], p3[1], p3[2], a3]];
    let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    for i in 0..16 {
        out[i] = pal[((bits >> (2 * i)) & 3) as usize];
    }
}

/// The 16 alphas of one BC3 alpha block (two 8-bit endpoints, 3-bit indices).
fn bc3_alpha_block(block: &[u8], out: &mut [u8; 16]) {
    let (a0, a1) = (block[0] as u32, block[1] as u32);
    let mut pal = [0u8; 8];
    pal[0] = a0 as u8;
    pal[1] = a1 as u8;
    if a0 > a1 {
        for i in 1..7u32 {
            pal[(i + 1) as usize] = (((7 - i) * a0 + i * a1) / 7) as u8;
        }
    } else {
        for i in 1..5u32 {
            pal[(i + 1) as usize] = (((5 - i) * a0 + i * a1) / 5) as u8;
        }
        pal[6] = 0;
        pal[7] = 255;
    }
    let mut bits: u64 = 0;
    for (i, b) in block[2..8].iter().enumerate() {
        bits |= (*b as u64) << (8 * i);
    }
    for i in 0..16 {
        out[i] = pal[((bits >> (3 * i)) & 7) as usize];
    }
}

/// The level of a BC1/BC3 (DXT1/DXT5) DDS that `dds_cap` would make the top,
/// decoded to RGBA8: (width, height, pixels).
pub fn decode_capped_rgba(dds: &[u8], max_side: u32) -> R<(u32, u32, Vec<u8>)> {
    let capped = dds_cap(dds, max_side)?;
    let (w, h, _) = dds_dims(&capped).ok_or("not a DDS")?;
    let fourcc = &capped[84..88];
    let (bpb, dxt5) = match fourcc {
        b"DXT1" => (8usize, false),
        b"DXT5" => (16usize, true),
        _ => return Err(format!("{}: only DXT1/DXT5 decode here", String::from_utf8_lossy(fourcc))),
    };
    let (bw, bh) = (w.max(1).div_ceil(4) as usize, h.max(1).div_ceil(4) as usize);
    if capped.len() < 128 + bw * bh * bpb {
        return Err("DDS shorter than its top level".into());
    }
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut colours = [[0u8; 4]; 16];
    let mut alphas = [255u8; 16];
    for by in 0..bh {
        for bx in 0..bw {
            let o = 128 + (by * bw + bx) * bpb;
            if dxt5 {
                bc3_alpha_block(&capped[o..o + 8], &mut alphas);
                bc1_block(&capped[o + 8..o + 16], &mut colours);
            } else {
                bc1_block(&capped[o..o + 8], &mut colours);
            }
            for py in 0..4 {
                for px in 0..4 {
                    let (x, y) = (bx * 4 + px, by * 4 + py);
                    if x >= w as usize || y >= h as usize {
                        continue;
                    }
                    let c = colours[py * 4 + px];
                    let a = if dxt5 { alphas[py * 4 + px] } else { c[3] };
                    let at = (y * w as usize + x) * 4;
                    rgba[at..at + 4].copy_from_slice(&[c[0], c[1], c[2], a]);
                }
            }
        }
    }
    Ok((w, h, rgba))
}

/// An uncompressed A8R8G8B8 DDS with one mip (the item editor's own texture
/// form), from RGBA8 pixels.
pub fn write_dds_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(128 + rgba.len());
    out.extend_from_slice(b"DDS ");
    let mut hdr = [0u32; 31];
    hdr[0] = 124;
    hdr[1] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x8;
    hdr[2] = h;
    hdr[3] = w;
    hdr[4] = w * 4;
    hdr[6] = 1;
    hdr[18] = 32;
    hdr[19] = 0x40 | 0x1;
    hdr[21] = 32;
    hdr[22] = 0x00FF_0000;
    hdr[23] = 0x0000_FF00;
    hdr[24] = 0x0000_00FF;
    hdr[25] = 0xFF00_0000;
    hdr[26] = 0x1000;
    for v in hdr {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for px in rgba.chunks(4) {
        out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    out
}

/// A BC1/BC3 DDS re-expressed as an uncompressed 32-bit DDS at the capped
/// level (no mips) — the form the item editor writes for its own custom
/// textures. Four times the bytes of the compressed level, but the alpha
/// channel survives whatever the loader does to a DXT5 (the leaf cards of
/// the first tree probes drew nothing under every opacity model with a DXT5
/// diffuse, 2026-09-08).
pub fn dds_uncompressed(dds: &[u8], max_side: u32) -> R<Vec<u8>> {
    let (w, h, rgba) = decode_capped_rgba(dds, max_side)?;
    Ok(write_dds_rgba(w, h, &rgba))
}
