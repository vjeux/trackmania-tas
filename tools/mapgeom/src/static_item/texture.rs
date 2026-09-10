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

/// One level of an RGBA8 mip chain.
pub struct Level {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// The share of pixels whose alpha is at or over `alpha_ref` (the pixels an
/// alpha test at that reference keeps).
pub fn alpha_coverage(rgba: &[u8], alpha_ref: u8) -> f64 {
    let n = rgba.len() / 4;
    if n == 0 {
        return 0.0;
    }
    rgba.chunks(4).filter(|p| p[3] >= alpha_ref).count() as f64 / n as f64
}

/// The next mip level: a 2×2 box filter, the colour averaged with alpha as
/// the weight (a transparent texel's colour must not bleed into the leaf it
/// borders), the alpha averaged plainly.
fn downsample(l: &Level) -> Level {
    let (w, h) = ((l.w / 2).max(1), (l.h / 2).max(1));
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            let mut asum = 0u32;
            let mut n = 0u32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let (sx, sy) = ((x * 2 + dx).min(l.w - 1), (y * 2 + dy).min(l.h - 1));
                    let p = &l.rgba[((sy * l.w + sx) * 4) as usize..][..4];
                    let a = p[3] as u32;
                    acc[0] += p[0] as u32 * a;
                    acc[1] += p[1] as u32 * a;
                    acc[2] += p[2] as u32 * a;
                    acc[3] += a;
                    asum += a;
                    n += 1;
                }
            }
            let o = ((y * w + x) * 4) as usize;
            if asum > 0 {
                out[o] = ((acc[0] + asum / 2) / asum) as u8;
                out[o + 1] = ((acc[1] + asum / 2) / asum) as u8;
                out[o + 2] = ((acc[2] + asum / 2) / asum) as u8;
            } else {
                // all four transparent: the plain mean keeps the under-mask colour
                let mut c = [0u32; 3];
                for dy in 0..2 {
                    for dx in 0..2 {
                        let (sx, sy) = ((x * 2 + dx).min(l.w - 1), (y * 2 + dy).min(l.h - 1));
                        let p = &l.rgba[((sy * l.w + sx) * 4) as usize..][..4];
                        c[0] += p[0] as u32;
                        c[1] += p[1] as u32;
                        c[2] += p[2] as u32;
                    }
                }
                out[o] = (c[0] / 4) as u8;
                out[o + 1] = (c[1] / 4) as u8;
                out[o + 2] = (c[2] / 4) as u8;
            }
            out[o + 3] = ((acc[3] + n / 2) / n) as u8;
        }
    }
    Level { w, h, rgba: out }
}

/// Every level's alpha scaled so that its coverage at `alpha_ref` equals the
/// top level's (the alpha-tested foliage rule: a plain box filter halves the
/// share of texels over the reference with every level, so a crown thins to
/// twigs at 40 m while the near mip is dense — the scale is found by
/// bisection, NVIDIA Texture Tools' "alpha coverage" method). `gain` scales
/// the TOP level's alpha first (1 = the image's own; over 1 fattens every
/// leaf, the knob for a hard alpha test that eats the soft edges the
/// vegetation shader keeps).
pub fn mip_chain(top: Level, alpha_ref: u8, preserve_coverage: bool, gain: f32) -> Vec<Level> {
    let mut top = top;
    if (gain - 1.0).abs() > 1e-4 {
        for p in top.rgba.chunks_mut(4) {
            p[3] = (p[3] as f32 * gain).round().clamp(0.0, 255.0) as u8;
        }
    }
    let want = alpha_coverage(&top.rgba, alpha_ref);
    let mut levels = vec![top];
    while levels.last().map(|l| l.w > 1 || l.h > 1).unwrap_or(false) {
        let mut next = downsample(levels.last().unwrap());
        if preserve_coverage && want > 0.0 && alpha_ref > 0 {
            // the scale that brings this level's coverage back to the top's
            let cov = |s: f32| -> f64 { next.rgba.chunks(4).filter(|p| (p[3] as f32 * s).min(255.0) >= alpha_ref as f32).count() as f64 / (next.rgba.len() / 4) as f64 };
            let (mut lo, mut hi) = (1.0f32, 1.0f32);
            while cov(hi) < want && hi < 64.0 {
                hi *= 2.0;
            }
            if hi > 1.0 {
                for _ in 0..12 {
                    let mid = (lo + hi) / 2.0;
                    if cov(mid) < want {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let s = hi;
                for p in next.rgba.chunks_mut(4) {
                    p[3] = (p[3] as f32 * s).round().min(255.0) as u8;
                }
            }
        }
        levels.push(next);
    }
    levels
}

/// The DDS header for a mip chain of `w`×`h`, `mips` levels, either DXT5
/// (linear size of the top level) or A8R8G8B8 (row pitch).
fn dds_header(w: u32, h: u32, mips: u32, dxt5: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(b"DDS ");
    let mut hdr = [0u32; 31];
    hdr[0] = 124;
    // CAPS | HEIGHT | WIDTH | PIXELFORMAT | MIPMAPCOUNT (0x20000) + LINEARSIZE or PITCH
    hdr[1] = 0x1 | 0x2 | 0x4 | 0x1000 | if mips > 1 { 0x20000 } else { 0 } | if dxt5 { 0x80000 } else { 0x8 };
    hdr[2] = h;
    hdr[3] = w;
    hdr[4] = if dxt5 { w.max(1).div_ceil(4) * h.max(1).div_ceil(4) * 16 } else { w * 4 };
    hdr[6] = mips;
    hdr[18] = 32;
    if dxt5 {
        hdr[19] = 0x4;
        hdr[20] = u32::from_le_bytes(*b"DXT5");
    } else {
        hdr[19] = 0x40 | 0x1;
        hdr[21] = 32;
        hdr[22] = 0x00FF_0000;
        hdr[23] = 0x0000_FF00;
        hdr[24] = 0x0000_00FF;
        hdr[25] = 0xFF00_0000;
    }
    // caps: TEXTURE (+ COMPLEX | MIPMAP when there is a chain)
    hdr[26] = 0x1000 | if mips > 1 { 0x8 | 0x400000 } else { 0 };
    for v in hdr {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// A DXT5 DDS with every level's colour scaled by `factor` (alpha untouched),
/// re-encoded — the darker atlas copy of an inner depth band.
pub fn darken_dds(dds: &[u8], factor: f32) -> R<Vec<u8>> {
    let (w0, h0, mips) = dds_dims(dds).ok_or("not a DDS")?;
    let mut levels: Vec<Level> = Vec::new();
    let mut side = w0.max(h0);
    for _ in 0..mips {
        let (w, h, mut rgba) = decode_capped_rgba(dds, side)?;
        if let Some(l) = levels.last() {
            if l.w == w && l.h == h {
                break;
            }
        }
        for px in rgba.chunks_mut(4) {
            for c in 0..3 {
                px[c] = (px[c] as f32 * factor).round().clamp(0.0, 255.0) as u8;
            }
        }
        levels.push(Level { w, h, rgba });
        if side <= 1 {
            break;
        }
        side /= 2;
    }
    if levels.is_empty() {
        return Err("no level decoded".into());
    }
    Ok(write_dds_dxt5_mips(&levels))
}

/// An uncompressed A8R8G8B8 DDS carrying the whole chain.
pub fn write_dds_rgba_mips(levels: &[Level]) -> Vec<u8> {
    let top = &levels[0];
    let mut out = dds_header(top.w, top.h, levels.len() as u32, false);
    for l in levels {
        for px in l.rgba.chunks(4) {
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    out
}

/// A DXT5 (BC3) DDS carrying the whole chain, encoded here (`encode_bc3`).
pub fn write_dds_dxt5_mips(levels: &[Level]) -> Vec<u8> {
    let top = &levels[0];
    let mut out = dds_header(top.w, top.h, levels.len() as u32, true);
    for l in levels {
        out.extend(encode_bc3(l.w, l.h, &l.rgba));
    }
    out
}

fn to565(c: [f32; 3]) -> u16 {
    let r = (c[0] / 255.0 * 31.0).round().clamp(0.0, 31.0) as u16;
    let g = (c[1] / 255.0 * 63.0).round().clamp(0.0, 63.0) as u16;
    let b = (c[2] / 255.0 * 31.0).round().clamp(0.0, 31.0) as u16;
    (r << 11) | (g << 5) | b
}

fn from565(v: u16) -> [f32; 3] {
    let r = ((v >> 11) & 31) as f32;
    let g = ((v >> 5) & 63) as f32;
    let b = (v & 31) as f32;
    [(r * 255.0 + 15.0) / 31.0, (g * 255.0 + 31.0) / 63.0, (b * 255.0 + 15.0) / 31.0]
}

/// One BC1 colour block (4-colour mode) for 16 RGBA pixels: the endpoints
/// start as the two pixels farthest apart (the transparent texels weigh
/// little — their colour is the under-mask filler), then one least-squares
/// refit over the assignment, as the classic fast encoders do.
fn encode_bc1_block(px: &[[u8; 4]; 16], out: &mut [u8]) {
    let col = |p: &[u8; 4]| [p[0] as f32, p[1] as f32, p[2] as f32];
    let weight = |p: &[u8; 4]| if p[3] >= 16 { 1.0f32 } else { 0.05 };
    // the farthest pair, among the weighty pixels when there are any
    let mut best = (0usize, 0usize, -1.0f32);
    for i in 0..16 {
        for j in i + 1..16 {
            let (a, b) = (col(&px[i]), col(&px[j]));
            let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)) * weight(&px[i]) * weight(&px[j]);
            if d > best.2 {
                best = (i, j, d);
            }
        }
    }
    let (mut e0, mut e1) = (col(&px[best.0]), col(&px[best.1]));
    let mut indices = [0u8; 16];
    for _pass in 0..2 {
        // palette from the quantised endpoints
        let (q0, q1) = (to565(e0), to565(e1));
        let (p0, p1) = (from565(q0), from565(q1));
        let pal = [p0, p1, [(2.0 * p0[0] + p1[0]) / 3.0, (2.0 * p0[1] + p1[1]) / 3.0, (2.0 * p0[2] + p1[2]) / 3.0], [(p0[0] + 2.0 * p1[0]) / 3.0, (p0[1] + 2.0 * p1[1]) / 3.0, (p0[2] + 2.0 * p1[2]) / 3.0]];
        for (k, p) in px.iter().enumerate() {
            let c = col(p);
            let mut bi = 0usize;
            let mut bd = f32::MAX;
            for (i, q) in pal.iter().enumerate() {
                let d = (c[0] - q[0]).powi(2) + (c[1] - q[1]).powi(2) + (c[2] - q[2]).powi(2);
                if d < bd {
                    bd = d;
                    bi = i;
                }
            }
            indices[k] = bi as u8;
        }
        // least-squares refit of the endpoints over the assignment
        // (colour = e0 * (1 - t) + e1 * t with t in {0, 1, 1/3, 2/3})
        let mut a00 = 0.0f32;
        let mut a01 = 0.0f32;
        let mut a11 = 0.0f32;
        let mut b0 = [0.0f32; 3];
        let mut b1 = [0.0f32; 3];
        for (k, p) in px.iter().enumerate() {
            let t = match indices[k] {
                0 => 0.0f32,
                1 => 1.0,
                2 => 1.0 / 3.0,
                _ => 2.0 / 3.0,
            };
            let wgt = weight(p);
            let c = col(p);
            a00 += wgt * (1.0 - t) * (1.0 - t);
            a01 += wgt * (1.0 - t) * t;
            a11 += wgt * t * t;
            for i in 0..3 {
                b0[i] += wgt * (1.0 - t) * c[i];
                b1[i] += wgt * t * c[i];
            }
        }
        let det = a00 * a11 - a01 * a01;
        if det.abs() > 1e-6 {
            for i in 0..3 {
                e0[i] = ((a11 * b0[i] - a01 * b1[i]) / det).clamp(0.0, 255.0);
                e1[i] = ((a00 * b1[i] - a01 * b0[i]) / det).clamp(0.0, 255.0);
            }
        } else {
            break;
        }
    }
    let (mut q0, mut q1) = (to565(e0), to565(e1));
    // the 4-colour mode wants c0 > c1; swapping the endpoints flips index 0<->1 and 2<->3
    let mut swap = false;
    if q0 < q1 {
        std::mem::swap(&mut q0, &mut q1);
        swap = true;
    } else if q0 == q1 {
        // one colour: every index 0 (the 3-colour mode's pal[0] is that colour)
        indices = [0; 16];
    }
    // final assignment against the palette actually written
    if q0 != q1 {
        let (p0, p1) = (from565(q0), from565(q1));
        let pal = [p0, p1, [(2.0 * p0[0] + p1[0]) / 3.0, (2.0 * p0[1] + p1[1]) / 3.0, (2.0 * p0[2] + p1[2]) / 3.0], [(p0[0] + 2.0 * p1[0]) / 3.0, (p0[1] + 2.0 * p1[1]) / 3.0, (p0[2] + 2.0 * p1[2]) / 3.0]];
        for (k, p) in px.iter().enumerate() {
            let c = col(p);
            let mut bi = 0usize;
            let mut bd = f32::MAX;
            for (i, q) in pal.iter().enumerate() {
                let d = (c[0] - q[0]).powi(2) + (c[1] - q[1]).powi(2) + (c[2] - q[2]).powi(2);
                if d < bd {
                    bd = d;
                    bi = i;
                }
            }
            indices[k] = bi as u8;
        }
        let _ = swap;
    }
    out[0..2].copy_from_slice(&q0.to_le_bytes());
    out[2..4].copy_from_slice(&q1.to_le_bytes());
    let mut bits = 0u32;
    for (k, i) in indices.iter().enumerate() {
        bits |= (*i as u32 & 3) << (2 * k);
    }
    out[4..8].copy_from_slice(&bits.to_le_bytes());
}

/// One BC3 alpha block (the 8-value mode, a0 > a1) for 16 alphas.
fn encode_bc3_alpha_block(alphas: &[u8; 16], out: &mut [u8]) {
    let (mut a0, mut a1) = (*alphas.iter().max().unwrap(), *alphas.iter().min().unwrap());
    if a0 == a1 {
        // one value: pal[0] = pal[1] = a0 in either mode, every index 0
        out[0] = a0;
        out[1] = a1;
        for b in &mut out[2..8] {
            *b = 0;
        }
        return;
    }
    if a0 < a1 {
        std::mem::swap(&mut a0, &mut a1);
    }
    let mut pal = [0u8; 8];
    pal[0] = a0;
    pal[1] = a1;
    for i in 1..7u32 {
        pal[(i + 1) as usize] = (((7 - i) * a0 as u32 + i * a1 as u32) / 7) as u8;
    }
    let mut bits: u64 = 0;
    for (k, a) in alphas.iter().enumerate() {
        let mut bi = 0usize;
        let mut bd = i32::MAX;
        for (i, p) in pal.iter().enumerate() {
            let d = (*a as i32 - *p as i32).abs();
            if d < bd {
                bd = d;
                bi = i;
            }
        }
        bits |= (bi as u64) << (3 * k);
    }
    out[0] = a0;
    out[1] = a1;
    for i in 0..6 {
        out[2 + i] = ((bits >> (8 * i)) & 0xff) as u8;
    }
}

/// RGBA8 pixels as BC3 (DXT5) block data, 16 bytes per 4×4 block, rows of
/// blocks top to bottom; edge blocks are padded by clamping.
pub fn encode_bc3(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let (bw, bh) = (w.max(1).div_ceil(4) as usize, h.max(1).div_ceil(4) as usize);
    let mut out = vec![0u8; bw * bh * 16];
    let mut px = [[0u8; 4]; 16];
    let mut alphas = [0u8; 16];
    for by in 0..bh {
        for bx in 0..bw {
            for py in 0..4 {
                for pxi in 0..4 {
                    let x = (bx * 4 + pxi).min(w as usize - 1);
                    let y = (by * 4 + py).min(h as usize - 1);
                    let at = (y * w as usize + x) * 4;
                    px[py * 4 + pxi] = [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]];
                    alphas[py * 4 + pxi] = rgba[at + 3];
                }
            }
            let o = (by * bw + bx) * 16;
            encode_bc3_alpha_block(&alphas, &mut out[o..o + 8]);
            encode_bc1_block(&px, &mut out[o + 8..o + 16]);
        }
    }
    out
}

#[cfg(test)]
mod chain_tests {
    use super::*;

    fn checker(w: u32, h: u32) -> Level {
        // a leaf-like image: opaque green squares on a transparent dark background
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let o = ((y * w + x) * 4) as usize;
                let on = (x / 4 + y / 4) % 4 == 0;
                rgba[o..o + 4].copy_from_slice(&if on { [60, 140, 40, 255] } else { [30, 40, 20, 0] });
            }
        }
        Level { w, h, rgba }
    }

    #[test]
    fn coverage_is_preserved_down_the_chain() {
        let top = checker(64, 32);
        let want = alpha_coverage(&top.rgba, 128);
        assert!((want - 0.25).abs() < 0.01);
        let plain = mip_chain(checker(64, 32), 128, false, 1.0);
        let kept = mip_chain(checker(64, 32), 128, true, 1.0);
        assert_eq!(plain.len(), 7);
        assert_eq!(kept.len(), 7);
        // a plain box filter keeps the 4x4 squares as whole texels down to level 2 and
        // loses them at level 3 (one texel of 25 % alpha)
        assert!((alpha_coverage(&plain[2].rgba, 128) - want).abs() < 0.01);
        assert!(alpha_coverage(&plain[3].rgba, 128) < 0.05, "plain level 3 coverage {}", alpha_coverage(&plain[3].rgba, 128));
        for l in kept.iter().take(3) {
            let c = alpha_coverage(&l.rgba, 128);
            assert!((c - want).abs() < 0.01, "{}x{}: coverage {c} vs {want}", l.w, l.h);
        }
        // the scaled chain never falls under the target (the uniform level 3 of the
        // checker can only be all or nothing: all)
        assert!(alpha_coverage(&kept[3].rgba, 128) >= want);
        // a leaf-like alpha with a spread of values lands within a few percent
        let (w, h) = (128u32, 64u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let mut seed = 12345u32;
        for p in rgba.chunks_mut(4) {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let a = (seed >> 24) as u8;
            p.copy_from_slice(&[70, 120, 50, if a > 200 { 255 } else if a > 150 { a } else { 0 }]);
        }
        let want2 = alpha_coverage(&rgba, 128);
        let kept2 = mip_chain(Level { w, h, rgba }, 128, true, 1.0);
        for l in kept2.iter().take(4) {
            let c = alpha_coverage(&l.rgba, 128);
            assert!((c - want2).abs() < 0.05, "{}x{}: coverage {c} vs {want2}", l.w, l.h);
        }
    }

    #[test]
    fn bc3_round_trips_flat_blocks_and_the_header_reads_back() {
        let top = checker(16, 8);
        let levels = mip_chain(top, 128, true, 1.0);
        let dds = write_dds_dxt5_mips(&levels);
        assert_eq!(dds_dims(&dds), Some((16, 8, 5)));
        assert_eq!(&dds[84..88], b"DXT5");
        let (w, h, rgba) = decode_capped_rgba(&dds, 16).unwrap();
        assert_eq!((w, h), (16, 8));
        // the checker's two colours come back within 565 quantisation, alpha exact
        for (p, q) in rgba.chunks(4).zip(levels[0].rgba.chunks(4)) {
            assert_eq!(p[3], q[3]);
            for i in 0..3 {
                assert!((p[i] as i32 - q[i] as i32).abs() <= 8, "{p:?} vs {q:?}");
            }
        }
        let raw = write_dds_rgba_mips(&levels);
        assert_eq!(dds_dims(&raw), Some((16, 8, 5)));
        assert_eq!(raw.len(), 128 + levels.iter().map(|l| l.rgba.len()).sum::<usize>());
    }
}
