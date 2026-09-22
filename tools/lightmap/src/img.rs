//! WEBP in/out for the atlases (image-webp: pure Rust; VP8 lossy decode,
//! VP8L lossless encode).

pub struct Rgb {
    pub w: u32,
    pub h: u32,
    /// RGB triplets, row-major.
    pub px: Vec<u8>,
}

impl Rgb {
    pub fn new(w: u32, h: u32) -> Self {
        Rgb { w, h, px: vec![0; (w * h * 3) as usize] }
    }
    pub fn get(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * self.w + x) * 3) as usize;
        [self.px[i], self.px[i + 1], self.px[i + 2]]
    }
    pub fn set(&mut self, x: u32, y: u32, c: [u8; 3]) {
        let i = ((y * self.w + x) * 3) as usize;
        self.px[i..i + 3].copy_from_slice(&c);
    }
    /// Mean colour over a rectangle (clamped to the image).
    pub fn mean(&self, x0: u32, y0: u32, w: u32, h: u32) -> [f32; 3] {
        let mut s = [0f32; 3];
        let mut n = 0f32;
        for y in y0..(y0 + h).min(self.h) {
            for x in x0..(x0 + w).min(self.w) {
                let c = self.get(x, y);
                for k in 0..3 {
                    s[k] += c[k] as f32;
                }
                n += 1.0;
            }
        }
        if n > 0.0 {
            for k in 0..3 {
                s[k] /= n;
            }
        }
        s
    }
}

pub fn decode_webp(b: &[u8]) -> Result<Rgb, String> {
    let mut dec = image_webp::WebPDecoder::new(std::io::Cursor::new(b)).map_err(|e| format!("webp: {e}"))?;
    let (w, h) = dec.dimensions();
    let bpp = if dec.has_alpha() { 4 } else { 3 };
    let mut buf = vec![0u8; (w * h) as usize * bpp];
    dec.read_image(&mut buf).map_err(|e| format!("webp: {e}"))?;
    let px = if bpp == 4 { buf.chunks(4).flat_map(|c| [c[0], c[1], c[2]]).collect() } else { buf };
    Ok(Rgb { w, h, px })
}

/// Lossless VP8L.
pub fn encode_webp_lossless(img: &Rgb) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let enc = image_webp::WebPEncoder::new(&mut out);
    enc.encode(&img.px, img.w, img.h, image_webp::ColorType::Rgb8).map_err(|e| format!("webp encode: {e}"))?;
    Ok(out)
}

/// Write a binary PPM (for eyeballing through ffmpeg/any viewer).
pub fn write_ppm(img: &Rgb, path: &str) -> std::io::Result<()> {
    let mut o = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    o.extend_from_slice(&img.px);
    std::fs::write(path, o)
}
