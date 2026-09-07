//! The gameplay gates' logo panels, drawn statically.
//!
//! A `GateSpecial*` prefab carries LED sign panels — a row of squares along
//! the arch (material `SpecialSign<Kind>`, or the kind folder's `Sign`) and one
//! on the beam (`SpecialSignOff` / `SignOff`) — all on the Techno3
//! `Tech3_Block_TDSN_CubeOut_DispIn` shader: dark LED cells whose lit image is
//! a runtime DISPLAY INPUT fed by the gate's gameplay entity. On a live gate
//! every panel shows the kind's logo (yellow chevrons for Turbo, red for
//! Turbo2, …); with nothing feeding the shader the row is dark and the beam
//! panel falls back to its material's own `_I` texture, the grey ⊗ "off" sign
//! — what the tiny gates showed (vjeux, Summer 19: "the booster gates logos
//! don't seem to match"). A static item cannot carry the entity, so the panels
//! become plain self-lit custom-texture materials showing the logo as the
//! live gate does.
//!
//! The picture: the pack's `SpecialSign<Kind>_I.dds` (256² DXT1) is the logo
//! shape in one colour on a flat background (black chevrons on yellow for
//! Turbo, white on red for Turbo2, black on lime for Boost). The LED display
//! lights the SHAPE in the background's colour on dark cells, so the panel
//! image is: background colour where the texture differs from its background,
//! black where it is the background. Written as an uncompressed 32-bit DDS
//! (no mips) — the item-editor form the custom material loader reads.

use super::R;

/// `Stadium\Media\Material\SpecialSign<Kind>` / `SpecialSignOff` /
/// `Stadium\Media\Modifier\<Kind>\Sign` / `SignOff` → the kind whose logo the
/// panel shows on a live gate (`SpecialSignOff` and the base `SpecialSign*`
/// belong to the Turbo dress of the shared prefab; a gate of another kind
/// carries its kind's modifier folder, which names the kind directly).
pub fn sign_kind(link: &str, item_kind: Option<&str>) -> Option<String> {
    let low = link.to_ascii_lowercase();
    if let Some(rest) = low.strip_prefix("stadium\\media\\modifier\\") {
        let (kind, file) = rest.split_once('\\')?;
        if file == "sign" || file == "signoff" {
            // the folder is lower-cased here; take the case from the link
            let k = &link[link.len() - rest.len()..][..kind.len()];
            return Some(k.to_string());
        }
        return None;
    }
    let stem = low.strip_prefix("stadium\\media\\material\\")?;
    if stem == "specialsignoff" {
        return Some(item_kind.unwrap_or("Turbo").to_string());
    }
    if let Some(k) = stem.strip_prefix("specialsign") {
        if k.is_empty() {
            return None;
        }
        // case from the link
        let k = &link[link.len() - k.len()..];
        return Some(item_kind.map(|s| s.to_string()).unwrap_or_else(|| k.to_string()));
    }
    None
}

/// The archive file name of a kind's panel picture (`TINY_SIGN_SUFFIX` tags
/// the name so a lineup can carry several pictures of one kind).
pub fn logo_file(kind: &str) -> String {
    let suffix = std::env::var("TINY_SIGN_SUFFIX").unwrap_or_default();
    format!("SignLogo{kind}{suffix}.dds")
}

/// The panel picture of a kind: a 32-bit DDS, lit logo on black.
pub fn logo_dds(store: &mut crate::store::DataStore, kind: &str) -> R<Vec<u8>> {
    let path = format!("Stadium\\Media\\Texture\\Image\\SpecialSign{kind}_I.dds");
    let bytes = store.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let (w, h, rgba) = decode_dxt1_top(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let bg = [rgba[0], rgba[1], rgba[2]];
    let bg_dark = bg.iter().all(|c| *c < 0x40);
    let mut out = Vec::with_capacity(rgba.len());
    // TINY_SIGN_ALPHA=mask|zero|full (default full): the alpha the panel
    // picture carries — a shading model may read the diffuse alpha as its
    // specular/gloss level (the black cells mirrored the sky in TDSNI)
    let alpha_mode = std::env::var("TINY_SIGN_ALPHA").unwrap_or_else(|_| "full".into());
    // Rows bottom-up: the live display shows the `_I` picture V-flipped
    // against a plain texture sampling of the panel's uv (Summer 19: the
    // chevrons pointed up on the tiny beam, down on the original).
    for row in (0..h as usize).rev() {
        for px in rgba[row * w as usize * 4..(row + 1) * w as usize * 4].chunks(4) {
            let d = (px[0] as i32 - bg[0] as i32).abs() + (px[1] as i32 - bg[1] as i32).abs() + (px[2] as i32 - bg[2] as i32).abs();
            let is_bg = d < 0x60;
            let c = if bg_dark {
                // the off sign: grey ⊗ on black is already the lit look
                [px[0], px[1], px[2]]
            } else if is_bg {
                [0, 0, 0]
            } else {
                bg
            };
            let lit = c.iter().any(|v| *v >= 0x40);
            let a = match alpha_mode.as_str() {
                "zero" => 0x00,
                "mask" => {
                    if lit {
                        0xFF
                    } else {
                        0x00
                    }
                }
                _ => 0xFF,
            };
            out.extend_from_slice(&[c[0], c[1], c[2], a]);
        }
    }
    Ok(write_dds_rgba(w, h, &out))
}

/// The top mip of a DXT1 (BC1) DDS as RGBA8.
fn decode_dxt1_top(dds: &[u8]) -> R<(u32, u32, Vec<u8>)> {
    if dds.len() < 128 || &dds[0..4] != b"DDS " {
        return Err("not a DDS file".into());
    }
    let u32_at = |o: usize| u32::from_le_bytes([dds[o], dds[o + 1], dds[o + 2], dds[o + 3]]);
    let h = u32_at(12);
    let w = u32_at(16);
    let fourcc = &dds[84..88];
    if fourcc != b"DXT1" {
        return Err(format!("{} is not DXT1", String::from_utf8_lossy(fourcc)));
    }
    let bw = (w as usize + 3) / 4;
    let bh = (h as usize + 3) / 4;
    let need = 128 + bw * bh * 8;
    if dds.len() < need {
        return Err(format!("{} bytes, {need} needed for the top mip", dds.len()));
    }
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let c565 = |v: u16| -> [u8; 3] {
        let r = ((v >> 11) & 31) as u32;
        let g = ((v >> 5) & 63) as u32;
        let b = (v & 31) as u32;
        [((r * 255 + 15) / 31) as u8, ((g * 255 + 31) / 63) as u8, ((b * 255 + 15) / 31) as u8]
    };
    for by in 0..bh {
        for bx in 0..bw {
            let o = 128 + (by * bw + bx) * 8;
            let c0 = u16::from_le_bytes([dds[o], dds[o + 1]]);
            let c1 = u16::from_le_bytes([dds[o + 2], dds[o + 3]]);
            let (p0, p1) = (c565(c0), c565(c1));
            let (p2, p3) = if c0 > c1 {
                ([((2 * p0[0] as u32 + p1[0] as u32) / 3) as u8, ((2 * p0[1] as u32 + p1[1] as u32) / 3) as u8, ((2 * p0[2] as u32 + p1[2] as u32) / 3) as u8], [((p0[0] as u32 + 2 * p1[0] as u32) / 3) as u8, ((p0[1] as u32 + 2 * p1[1] as u32) / 3) as u8, ((p0[2] as u32 + 2 * p1[2] as u32) / 3) as u8])
            } else {
                ([((p0[0] as u32 + p1[0] as u32) / 2) as u8, ((p0[1] as u32 + p1[1] as u32) / 2) as u8, ((p0[2] as u32 + p1[2] as u32) / 2) as u8], [0, 0, 0])
            };
            let pal = [p0, p1, p2, p3];
            let bits = u32::from_le_bytes([dds[o + 4], dds[o + 5], dds[o + 6], dds[o + 7]]);
            for py in 0..4 {
                for px in 0..4 {
                    let (x, y) = (bx * 4 + px, by * 4 + py);
                    if x >= w as usize || y >= h as usize {
                        continue;
                    }
                    let idx = ((bits >> (2 * (py * 4 + px))) & 3) as usize;
                    let c = pal[idx];
                    let at = (y * w as usize + x) * 4;
                    rgba[at..at + 3].copy_from_slice(&c);
                    rgba[at + 3] = 0xFF;
                }
            }
        }
    }
    Ok((w, h, rgba))
}

/// An uncompressed A8R8G8B8 DDS with one mip.
fn write_dds_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(128 + rgba.len());
    out.extend_from_slice(b"DDS ");
    let mut hdr = [0u32; 31];
    hdr[0] = 124; // size
    hdr[1] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x8; // caps, height, width, pixelformat, pitch
    hdr[2] = h;
    hdr[3] = w;
    hdr[4] = w * 4; // pitch
    hdr[6] = 1; // mip count
    hdr[18] = 32; // pixel format size
    hdr[19] = 0x40 | 0x1; // RGB + alpha
    hdr[21] = 32; // bit count
    hdr[22] = 0x00FF_0000; // R
    hdr[23] = 0x0000_FF00; // G
    hdr[24] = 0x0000_00FF; // B
    hdr[25] = 0xFF00_0000; // A
    hdr[26] = 0x1000; // caps: texture
    for v in hdr {
        out.extend_from_slice(&v.to_le_bytes());
    }
    // DDS pixel order for A8R8G8B8 masks above is BGRA in memory
    for px in rgba.chunks(4) {
        out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(sign_kind("Stadium\\Media\\Material\\SpecialSignTurbo", None).as_deref(), Some("Turbo"));
        assert_eq!(sign_kind("Stadium\\Media\\Material\\SpecialSignOff", None).as_deref(), Some("Turbo"));
        assert_eq!(sign_kind("Stadium\\Media\\Material\\SpecialSignOff", Some("Boost")).as_deref(), Some("Boost"));
        assert_eq!(sign_kind("Stadium\\Media\\Modifier\\Turbo2\\Sign", None).as_deref(), Some("Turbo2"));
        assert_eq!(sign_kind("Stadium\\Media\\Modifier\\Turbo2\\SignOff", Some("Turbo2")).as_deref(), Some("Turbo2"));
        assert_eq!(sign_kind("Stadium\\Media\\Modifier\\Turbo2\\SpecialFX", None), None);
        assert_eq!(sign_kind("Stadium\\Media\\Material\\TechnicsSpecials", None), None);
    }

    #[test]
    fn dds_roundtrip_header() {
        let d = write_dds_rgba(2, 2, &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 255]);
        assert_eq!(&d[0..4], b"DDS ");
        assert_eq!(d.len(), 128 + 16);
        // first pixel red, stored BGRA
        assert_eq!(&d[128..132], &[0, 0, 255, 255]);
    }
}

/// The one material slot both panels of a kind share (dedup key in
/// `Merged::material_slot`; `sign_logo_material` turns it into the picture
/// material). A pseudo link — no such file exists in the packs.
pub fn pseudo_link(kind: &str) -> String {
    format!("Stadium\\Media\\Material\\SignLogo{kind}")
}

/// The kind of a pseudo link, `None` for any real link.
pub fn kind_of_pseudo(link: &str) -> Option<&str> {
    link.strip_prefix("Stadium\\Media\\Material\\SignLogo").filter(|k| !k.is_empty())
}

/// `TINY_SIGN_LOGO=off` keeps the game materials (dark row, ⊗ on the beam).
pub fn enabled() -> bool {
    !std::env::var("TINY_SIGN_LOGO").map(|v| v == "off").unwrap_or(false)
}
