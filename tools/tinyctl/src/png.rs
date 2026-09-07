//! PNG in, PNG out — the two operations the comparison needs, on `miniz_oxide`
//! (already in the workspace) and nothing else.
//!
//! Decodes 8-bit greyscale / RGB / RGBA / grey+alpha, non-interlaced (what the
//! render box's `shotdpi.ps1` writes and what ffmpeg writes), into RGB8.
//! Encodes RGB8. Anything else is refused with a message rather than
//! mis-decoded: a comparison built on wrongly decoded pixels would flag the
//! whole frame and look like a broken map.

pub struct Image {
    pub w: usize,
    pub h: usize,
    /// RGB, row-major, `w*h*3` bytes.
    pub px: Vec<u8>,
}

impl Image {
    pub fn new(w: usize, h: usize) -> Image {
        Image { w, h, px: vec![0; w * h * 3] }
    }
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.w + x) * 3;
        [self.px[i], self.px[i + 1], self.px[i + 2]]
    }
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, c: [u8; 3]) {
        if x < self.w && y < self.h {
            let i = (y * self.w + x) * 3;
            self.px[i..i + 3].copy_from_slice(&c);
        }
    }
    /// Box-filter downscale by an integer factor (the fast, alias-free way to
    /// bring a 3840x2160 capture to analysis size).
    pub fn shrink(&self, f: usize) -> Image {
        if f <= 1 {
            return Image { w: self.w, h: self.h, px: self.px.clone() };
        }
        let (w, h) = (self.w / f, self.h / f);
        let mut out = Image::new(w, h);
        let n = (f * f) as u32;
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0u32; 3];
                for dy in 0..f {
                    let row = (y * f + dy) * self.w;
                    for dx in 0..f {
                        let i = (row + x * f + dx) * 3;
                        acc[0] += self.px[i] as u32;
                        acc[1] += self.px[i + 1] as u32;
                        acc[2] += self.px[i + 2] as u32;
                    }
                }
                out.set(x, y, [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8]);
            }
        }
        out
    }
    /// Crop (clamped to the image) and resample to `ow` pixels wide with
    /// nearest-neighbour — crops are looked at, not measured.
    pub fn crop_scaled(&self, x0: i64, y0: i64, x1: i64, y1: i64, ow: usize) -> Image {
        let x0 = x0.clamp(0, self.w as i64 - 1) as usize;
        let y0 = y0.clamp(0, self.h as i64 - 1) as usize;
        let x1 = x1.clamp(x0 as i64 + 1, self.w as i64) as usize;
        let y1 = y1.clamp(y0 as i64 + 1, self.h as i64) as usize;
        let (cw, ch) = (x1 - x0, y1 - y0);
        let oh = ((ch as f64) * (ow as f64) / (cw as f64)).round().max(1.0) as usize;
        let mut out = Image::new(ow, oh);
        for y in 0..oh {
            let sy = y0 + (y * ch) / oh;
            for x in 0..ow {
                let sx = x0 + (x * cw) / ow;
                out.set(x, y, self.get(sx, sy));
            }
        }
        out
    }
    pub fn blit(&mut self, src: &Image, x: usize, y: usize) {
        for sy in 0..src.h {
            for sx in 0..src.w {
                self.set(x + sx, y + sy, src.get(sx, sy));
            }
        }
    }
    pub fn fill(&mut self, x0: usize, y0: usize, w: usize, h: usize, c: [u8; 3]) {
        for y in y0..(y0 + h).min(self.h) {
            for x in x0..(x0 + w).min(self.w) {
                self.set(x, y, c);
            }
        }
    }
    pub fn rect(&mut self, x0: usize, y0: usize, w: usize, h: usize, t: usize, c: [u8; 3]) {
        self.fill(x0, y0, w, t, c);
        self.fill(x0, y0 + h.saturating_sub(t), w, t, c);
        self.fill(x0, y0, t, h, c);
        self.fill(x0 + w.saturating_sub(t), y0, t, h, c);
    }
}

fn be32(b: &[u8]) -> usize {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize
}

pub fn decode(d: &[u8]) -> Result<Image, String> {
    if d.len() < 33 || &d[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("not a PNG".into());
    }
    let mut o = 8;
    let (mut w, mut h, mut depth, mut ct, mut interlace) = (0usize, 0usize, 0u8, 0u8, 0u8);
    let mut idat: Vec<u8> = Vec::new();
    while o + 8 <= d.len() {
        let len = be32(&d[o..]);
        let ty = &d[o + 4..o + 8];
        if o + 8 + len > d.len() {
            return Err("truncated PNG chunk".into());
        }
        let body = &d[o + 8..o + 8 + len];
        match ty {
            b"IHDR" => {
                w = be32(&body[0..]);
                h = be32(&body[4..]);
                depth = body[8];
                ct = body[9];
                interlace = body[12];
            }
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        o += 12 + len;
    }
    if depth != 8 {
        return Err(format!("{depth}-bit PNG: only 8-bit is decoded here"));
    }
    if interlace != 0 {
        return Err("interlaced PNG: not decoded here".into());
    }
    let bpp = match ct {
        0 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        3 => return Err("palette PNG: not decoded here".into()),
        _ => return Err(format!("PNG colour type {ct}")),
    };
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&idat).map_err(|e| format!("PNG inflate: {e:?}"))?;
    let stride = w * bpp;
    if raw.len() < h * (stride + 1) {
        return Err(format!("PNG data short: {} < {}", raw.len(), h * (stride + 1)));
    }
    let mut out = Image::new(w, h);
    let mut prev = vec![0u8; stride];
    let mut cur = vec![0u8; stride];
    let mut p = 0;
    for y in 0..h {
        let f = raw[p];
        p += 1;
        cur.copy_from_slice(&raw[p..p + stride]);
        p += stride;
        for i in 0..stride {
            let a = if i >= bpp { cur[i - bpp] } else { 0 } as i32;
            let b = prev[i] as i32;
            let c = if i >= bpp { prev[i - bpp] } else { 0 } as i32;
            let x = cur[i] as i32;
            let v = match f {
                0 => x,
                1 => x + a,
                2 => x + b,
                3 => x + ((a + b) >> 1),
                4 => {
                    let pp = a + b - c;
                    let (pa, pb, pc) = ((pp - a).abs(), (pp - b).abs(), (pp - c).abs());
                    x + if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c }
                }
                _ => return Err(format!("PNG filter {f}")),
            };
            cur[i] = (v & 0xFF) as u8;
        }
        let row = y * w * 3;
        for x in 0..w {
            let s = x * bpp;
            let c = match bpp {
                1 | 2 => [cur[s], cur[s], cur[s]],
                _ => [cur[s], cur[s + 1], cur[s + 2]],
            };
            out.px[row + x * 3..row + x * 3 + 3].copy_from_slice(&c);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    Ok(out)
}

pub fn encode(img: &Image) -> Vec<u8> {
    let mut raw = Vec::with_capacity(img.h * (img.w * 3 + 1));
    for y in 0..img.h {
        raw.push(0);
        raw.extend_from_slice(&img.px[y * img.w * 3..(y + 1) * img.w * 3]);
    }
    let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
    let mut out = Vec::with_capacity(z.len() + 64);
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(img.w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(img.h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(ty);
    out.extend_from_slice(body);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

// ---------------------------------------------------------------- a 5x7 font
// Enough glyphs to label a crop: A-Z, 0-9 and a little punctuation. Each
// glyph is 7 rows of 5 bits, MSB left.
fn glyph(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        ':' => [0x00, 0x04, 0x04, 0x00, 0x04, 0x04, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        ' ' => [0; 7],
        _ => [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F],
    }
}

/// Draw `text` at (x, y) in the 5x7 font at integer `scale`, with a dark box
/// behind it so it reads on any background.
pub fn label(img: &mut Image, x: usize, y: usize, text: &str, scale: usize, color: [u8; 3]) {
    let n = text.chars().count();
    img.fill(x, y, (n * 6 + 1) * scale, 9 * scale, [0, 0, 0]);
    for (i, ch) in text.chars().enumerate() {
        let g = glyph(ch);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (0x10 >> col) != 0 {
                    img.fill(x + (1 + i * 6 + col) * scale, y + (1 + row) * scale, scale, scale, color);
                }
            }
        }
    }
}
