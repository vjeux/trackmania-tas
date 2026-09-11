//! A minimal PNG writer (RGB8, zlib via miniz_oxide) for previews.

pub struct Image {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 3]>,
}

impl Image {
    pub fn new(w: usize, h: usize, bg: [u8; 3]) -> Image {
        Image { w, h, px: vec![bg; w * h] }
    }
    pub fn put(&mut self, x: i64, y: i64, c: [u8; 3]) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }
    pub fn disc(&mut self, x: f64, y: f64, r: f64, c: [u8; 3]) {
        let (x0, y0) = (x.round() as i64, y.round() as i64);
        let ri = r.ceil() as i64;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f64 <= r * r {
                    self.put(x0 + dx, y0 + dy, c);
                }
            }
        }
    }
    pub fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, c: [u8; 3]) {
        let n = ((x1 - x0).abs().max((y1 - y0).abs()).ceil() as usize).max(1);
        for i in 0..=n {
            let t = i as f64 / n as f64;
            self.put((x0 + (x1 - x0) * t).round() as i64, (y0 + (y1 - y0) * t).round() as i64, c);
        }
    }
    pub fn save(&self, path: &std::path::Path) {
        let mut raw = Vec::with_capacity((self.w * 3 + 1) * self.h);
        for y in 0..self.h {
            raw.push(0u8);
            for x in 0..self.w {
                raw.extend_from_slice(&self.px[y * self.w + x]);
            }
        }
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        let mut out = Vec::new();
        out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(self.w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(self.h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        std::fs::write(path, out).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut crc_in = Vec::with_capacity(4 + data.len());
    crc_in.extend_from_slice(kind);
    crc_in.extend_from_slice(data);
    out.extend_from_slice(&crc_in);
    out.extend_from_slice(&crc32(&crc_in).to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 };
        }
        table[i as usize] = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// Heat colour for t in [0,1]: blue -> green -> yellow -> red.
pub fn heat(t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.33 {
        let u = t / 0.33;
        (0.0, u, 1.0 - u)
    } else if t < 0.66 {
        let u = (t - 0.33) / 0.33;
        (u, 1.0, 0.0)
    } else {
        let u = (t - 0.66) / 0.34;
        (1.0, 1.0 - u, 0.0)
    };
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}
