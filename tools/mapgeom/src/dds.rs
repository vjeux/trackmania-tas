//! DDS bitmaps (the game's diffuse textures): the 128-byte header and the block
//! formats the Stadium pack uses — DXT1 (BC1) and DXT5 (BC3) — decoded to RGB8.
//! Only the TOP mip is decoded; a caller wanting a smaller picture samples it.
//! No `DX10` header variant has been seen in the pack (2026-09-14).

pub struct Bitmap {
    pub w: usize,
    pub h: usize,
    /// RGB8, row-major, `w*h*3` bytes.
    pub rgb: Vec<u8>,
}

impl Bitmap {
    /// Nearest sample at texture coordinates (u, v) with wrap-around; v runs down
    /// the image like the game (DirectX convention: v = 0 is the top row).
    pub fn sample(&self, u: f32, v: f32) -> [u8; 3] {
        let fu = u - u.floor();
        let fv = v - v.floor();
        let x = ((fu * self.w as f32) as usize).min(self.w - 1);
        let y = ((fv * self.h as f32) as usize).min(self.h - 1);
        let i = (y * self.w + x) * 3;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }

    /// A box-filtered copy at most `max` pixels on the longer side (sheets need
    /// nothing like 4096²; sampling the full mip aliases badly).
    pub fn shrink_to(&self, max: usize) -> Bitmap {
        let f = (self.w.max(self.h) + max - 1) / max;
        if f <= 1 {
            return Bitmap { w: self.w, h: self.h, rgb: self.rgb.clone() };
        }
        let (w, h) = ((self.w / f).max(1), (self.h / f).max(1));
        let mut rgb = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0u32; 3];
                let mut n = 0u32;
                for dy in 0..f {
                    for dx in 0..f {
                        let (sx, sy) = (x * f + dx, y * f + dy);
                        if sx < self.w && sy < self.h {
                            let i = (sy * self.w + sx) * 3;
                            acc[0] += self.rgb[i] as u32;
                            acc[1] += self.rgb[i + 1] as u32;
                            acc[2] += self.rgb[i + 2] as u32;
                            n += 1;
                        }
                    }
                }
                let o = (y * w + x) * 3;
                for k in 0..3 {
                    rgb[o + k] = (acc[k] / n.max(1)) as u8;
                }
            }
        }
        Bitmap { w, h, rgb }
    }
}

fn rgb565(c: u16) -> [u8; 3] {
    let r = ((c >> 11) & 31) as u32;
    let g = ((c >> 5) & 63) as u32;
    let b = (c & 31) as u32;
    [((r * 255 + 15) / 31) as u8, ((g * 255 + 31) / 63) as u8, ((b * 255 + 15) / 31) as u8]
}

/// Decode the top mip of a DDS file. Errors name the unsupported format.
pub fn decode(bytes: &[u8]) -> Result<Bitmap, String> {
    if bytes.len() < 128 || &bytes[0..4] != b"DDS " {
        return Err("not a DDS file".into());
    }
    let u32_at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let h = u32_at(12) as usize;
    let w = u32_at(16) as usize;
    let fourcc = &bytes[84..88];
    let pf_flags = u32_at(80);
    let (block_bytes, bc3, data_off) = match fourcc {
        b"DXT1" => (8usize, false, 128usize),
        b"DXT5" => (16, true, 128),
        b"DXT3" => (16, true, 128), // alpha ignored: colour block reads like DXT5's
        b"DX10" => {
            let dxgi = u32_at(128);
            match dxgi {
                71 | 72 => (8, false, 148),  // BC1_UNORM(_SRGB)
                77 | 78 => (16, true, 148),  // BC3_UNORM(_SRGB)
                other => return Err(format!("DX10 DXGI format {other} not decoded")),
            }
        }
        _ if pf_flags & 0x40 != 0 => {
            // uncompressed RGB(A): 24 or 32 bpp
            let bpp = u32_at(88) as usize;
            let bytes_pp = bpp / 8;
            if bytes_pp != 3 && bytes_pp != 4 {
                return Err(format!("uncompressed {bpp} bpp not decoded"));
            }
            let (rm, gm, bm) = (u32_at(92), u32_at(96), u32_at(100));
            let shift = |m: u32| m.trailing_zeros();
            let mut rgb = vec![0u8; w * h * 3];
            for i in 0..w * h {
                let o = 128 + i * bytes_pp;
                if o + bytes_pp > bytes.len() {
                    break;
                }
                let mut px = 0u32;
                for k in 0..bytes_pp {
                    px |= (bytes[o + k] as u32) << (8 * k);
                }
                rgb[i * 3] = ((px & rm) >> shift(rm)) as u8;
                rgb[i * 3 + 1] = ((px & gm) >> shift(gm)) as u8;
                rgb[i * 3 + 2] = ((px & bm) >> shift(bm)) as u8;
            }
            return Ok(Bitmap { w, h, rgb });
        }
        other => return Err(format!("fourcc {:?} not decoded", String::from_utf8_lossy(other))),
    };
    let bw = (w + 3) / 4;
    let bh = (h + 3) / 4;
    let mut rgb = vec![0u8; w * h * 3];
    for by in 0..bh {
        for bx in 0..bw {
            let o = data_off + (by * bw + bx) * block_bytes + if bc3 { 8 } else { 0 };
            if o + 8 > bytes.len() {
                break;
            }
            let c0 = u16::from_le_bytes([bytes[o], bytes[o + 1]]);
            let c1 = u16::from_le_bytes([bytes[o + 2], bytes[o + 3]]);
            let bits = u32::from_le_bytes(bytes[o + 4..o + 8].try_into().unwrap());
            let (p0, p1) = (rgb565(c0), rgb565(c1));
            let mix = |a: [u8; 3], b: [u8; 3], wa: u32, wb: u32| -> [u8; 3] {
                let mut out = [0u8; 3];
                for k in 0..3 {
                    out[k] = ((a[k] as u32 * wa + b[k] as u32 * wb) / (wa + wb)) as u8;
                }
                out
            };
            let palette = if bc3 || c0 > c1 {
                [p0, p1, mix(p0, p1, 2, 1), mix(p0, p1, 1, 2)]
            } else {
                [p0, p1, mix(p0, p1, 1, 1), [0, 0, 0]]
            };
            for py in 0..4 {
                for px in 0..4 {
                    let (x, y) = (bx * 4 + px, by * 4 + py);
                    if x >= w || y >= h {
                        continue;
                    }
                    let code = (bits >> (2 * (py * 4 + px))) & 3;
                    let c = palette[code as usize];
                    let i = (y * w + x) * 3;
                    rgb[i..i + 3].copy_from_slice(&c);
                }
            }
        }
    }
    Ok(Bitmap { w, h, rgb })
}

/// The diffuse bitmap of a game material: `<Material>.Material.Gbx` names
/// `<X>_D.Texture.gbx`, which names `Image\<X>_D.dds` (hashed in the pack).
/// Resolved through the store like every other path; `None` when any link is
/// missing or the format is not decoded.
pub fn material_diffuse(store: &mut crate::store::DataStore, material_link: &str) -> Option<Bitmap> {
    let mat_path = if material_link.to_ascii_lowercase().ends_with(".material.gbx") { material_link.to_string() } else { format!("{material_link}.Material.Gbx") };
    let model = store.load_model(&mat_path).ok()?;
    // the texture files the material references. The diffuse is the `_D` one —
    // but a TRIPLANAR material (TrackWall, the castle walls) keeps its look in
    // `<X>Pxz_D` (the side projection) and `<X>Py*` (the top); its plain `<X>_D`
    // is a near-black mask. So: Pxz_D first, then any `_D` that is not a HueMask,
    // then the first texture at all.
    let texs: Vec<String> = model.externals.iter().map(|(_, p)| p.clone()).filter(|p| p.to_ascii_lowercase().contains(".texture.gbx")).collect();
    let lc = |p: &String| p.to_ascii_lowercase();
    let tex = texs.iter().find(|p| lc(p).ends_with("pxz_d.texture.gbx"))
        .or_else(|| texs.iter().find(|p| lc(p).ends_with("_d.texture.gbx") && !lc(p).contains("huemask") && !lc(p).contains("techno3")))
        .or_else(|| texs.iter().find(|p| !lc(p).contains("techno3")))
        .or_else(|| texs.first())?
        .clone();
    let tmodel = store.load_model(&tex).ok()?;
    let dds = tmodel.externals.iter().map(|(_, p)| p.clone()).find(|p| p.to_ascii_lowercase().ends_with(".dds") || p.to_ascii_lowercase().ends_with(".tga"))?;
    let bytes = store.read(&dds).ok()?;
    decode(&bytes).ok()
}
