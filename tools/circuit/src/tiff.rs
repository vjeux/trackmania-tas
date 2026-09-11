//! GeoTIFF rasters as the Environment Agency publishes them: one band,
//! float32 or integer samples, strips or tiles, uncompressed or LZW (with
//! the horizontal or floating-point predictor), georeferenced by pixel
//! scale + tie point or by the affine ModelTransformation. Anything else is
//! refused loudly rather than read wrong.

use std::path::Path;

pub struct Raster {
    pub width: usize,
    pub height: usize,
    /// Easting of the LEFT edge of the first column and northing of the TOP
    /// edge of the first row (GeoTIFF raster-is-area convention).
    pub x0: f64,
    pub y0: f64,
    /// Pixel size in metres (square pixels).
    pub px: f64,
    pub data: Vec<f32>,
    pub nodata: f32,
    pub min: f32,
    pub max: f32,
    pub nodata_count: usize,
}

fn rd16(b: &[u8], o: usize, be: bool) -> u16 {
    let v = [b[o], b[o + 1]];
    if be { u16::from_be_bytes(v) } else { u16::from_le_bytes(v) }
}
fn rd32(b: &[u8], o: usize, be: bool) -> u32 {
    let v: [u8; 4] = b[o..o + 4].try_into().unwrap();
    if be { u32::from_be_bytes(v) } else { u32::from_le_bytes(v) }
}
fn rdf64(b: &[u8], o: usize, be: bool) -> f64 {
    let v: [u8; 8] = b[o..o + 8].try_into().unwrap();
    if be { f64::from_be_bytes(v) } else { f64::from_le_bytes(v) }
}

struct Layout {
    width: usize,
    height: usize,
    bits: u16,
    compression: u16,
    sample_format: u16,
    predictor: u16,
    strip_offsets: Vec<usize>,
    strip_counts: Vec<usize>,
    rows_per_strip: usize,
    tile_w: usize,
    tile_h: usize,
    tile_offsets: Vec<usize>,
    tile_counts: Vec<usize>,
    scale: Option<(f64, f64)>,
    tie: Option<[f64; 4]>,
    transform: Option<[f64; 16]>,
    nodata: f32,
    tags_seen: Vec<(u16, u16, usize)>,
}

fn read_ifd(b: &[u8], be: bool) -> Layout {
    let ifd = rd32(b, 4, be) as usize;
    let n = rd16(b, ifd, be) as usize;
    let mut l = Layout {
        width: 0,
        height: 0,
        bits: 0,
        compression: 1,
        sample_format: 1,
        predictor: 1,
        strip_offsets: Vec::new(),
        strip_counts: Vec::new(),
        rows_per_strip: usize::MAX,
        tile_w: 0,
        tile_h: 0,
        tile_offsets: Vec::new(),
        tile_counts: Vec::new(),
        scale: None,
        tie: None,
        transform: None,
        nodata: f32::NAN,
        tags_seen: Vec::new(),
    };
    let type_size = |t: u16| match t {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        5 | 10 | 12 | 16 | 17 | 18 => 8,
        _ => panic!("tiff type {t}"),
    };
    for k in 0..n {
        let e = ifd + 2 + k * 12;
        let tag = rd16(b, e, be);
        let typ = rd16(b, e + 2, be);
        let count = rd32(b, e + 4, be) as usize;
        let size = type_size(typ) * count;
        let at = if size <= 4 { e + 8 } else { rd32(b, e + 8, be) as usize };
        l.tags_seen.push((tag, typ, count));
        let values = |i: usize| -> u64 {
            match typ {
                3 => rd16(b, at + 2 * i, be) as u64,
                4 => rd32(b, at + 4 * i, be) as u64,
                16 => (rd32(b, at + 8 * i, be) as u64) << 32 | rd32(b, at + 8 * i + 4, be) as u64,
                _ => panic!("tiff tag {tag} type {typ}"),
            }
        };
        match tag {
            256 => l.width = values(0) as usize,
            257 => l.height = values(0) as usize,
            258 => l.bits = values(0) as u16,
            259 => l.compression = values(0) as u16,
            273 => l.strip_offsets = (0..count).map(|i| values(i) as usize).collect(),
            278 => l.rows_per_strip = values(0) as usize,
            279 => l.strip_counts = (0..count).map(|i| values(i) as usize).collect(),
            317 => l.predictor = values(0) as u16,
            322 => l.tile_w = values(0) as usize,
            323 => l.tile_h = values(0) as usize,
            324 => l.tile_offsets = (0..count).map(|i| values(i) as usize).collect(),
            325 => l.tile_counts = (0..count).map(|i| values(i) as usize).collect(),
            339 => l.sample_format = values(0) as u16,
            33550 => l.scale = Some((rdf64(b, at, be), rdf64(b, at + 8, be))),
            33922 => {
                assert!(count >= 6, "tie point tag too short");
                l.tie = Some([rdf64(b, at, be), rdf64(b, at + 8, be), rdf64(b, at + 24, be), rdf64(b, at + 32, be)]);
            }
            34264 => {
                assert_eq!(count, 16, "ModelTransformation wants 16 doubles");
                let mut m = [0f64; 16];
                for (i, v) in m.iter_mut().enumerate() {
                    *v = rdf64(b, at + 8 * i, be);
                }
                l.transform = Some(m);
            }
            42113 => {
                let s = String::from_utf8_lossy(&b[at..at + count]).trim_end_matches('\0').trim().to_string();
                l.nodata = s.parse().unwrap_or(f32::NAN);
            }
            _ => {}
        }
    }
    l
}

impl Raster {
    pub fn load(path: &Path) -> Raster {
        let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let be = match &b[0..2] {
            b"MM" => true,
            b"II" => false,
            _ => panic!("{}: not a TIFF", path.display()),
        };
        assert_eq!(rd16(&b, 2, be), 42, "{}: not a classic TIFF", path.display());
        let l = read_ifd(&b, be);
        let (width, height, bits, sf) = (l.width, l.height, l.bits, l.sample_format);
        assert!(l.compression == 1 || l.compression == 5, "{}: compression {} not supported (1 none, 5 LZW)", path.display(), l.compression);
        assert!(matches!((bits, sf), (32, 3) | (16, 1) | (16, 2) | (8, 1) | (32, 1) | (32, 2)), "{}: samples bits {bits} format {sf}", path.display());
        let bytes_per = (bits / 8) as usize;
        // Georeference: pixel scale + tie point, or the affine matrix
        // (x = m0*i + m1*j + m3; y = m4*i + m5*j + m7), north-up only.
        let (px, x0, y0) = match (l.scale, l.tie, l.transform) {
            (Some((sx, sy)), Some(t), _) => {
                assert!((sx - sy).abs() < 1e-9, "non-square pixels");
                (sx, t[2] - t[0] * sx, t[3] + t[1] * sy)
            }
            (_, _, Some(m)) => {
                assert!(m[1].abs() < 1e-9 && m[4].abs() < 1e-9, "rotated raster");
                assert!((m[0] + m[5]).abs() < 1e-9, "non-square pixels {} {}", m[0], m[5]);
                (m[0], m[3], m[7])
            }
            _ => panic!("{}: no georeference tags; tags present: {:?}", path.display(), l.tags_seen),
        };
        let nodata = l.nodata;
        let mut data = vec![f32::NAN; width * height];
        let (mut min, mut max, mut nodata_count) = (f32::MAX, f32::MIN, 0usize);
        let sample = |buf: &[u8], i: usize| -> f32 {
            let o = i * bytes_per;
            match (bits, sf) {
                (32, 3) => f32::from_bits(rd32(buf, o, be)),
                (32, 1) => rd32(buf, o, be) as f32,
                (32, 2) => rd32(buf, o, be) as i32 as f32,
                (16, 1) => rd16(buf, o, be) as f32,
                (16, 2) => rd16(buf, o, be) as i16 as f32,
                (8, 1) => buf[o] as f32,
                _ => unreachable!(),
            }
        };
        // One block (strip or tile): decompress, undo the predictor, decode.
        let mut place = |raw: &[u8], bw: usize, bh: usize, x_off: usize, y_off: usize| {
            let mut buf: Vec<u8> = if l.compression == 5 { lzw_decode(raw) } else { raw.to_vec() };
            let need = bw * bh * bytes_per;
            assert!(buf.len() >= need, "{}: block decodes to {} bytes, want {need}", path.display(), buf.len());
            buf.truncate(need);
            match l.predictor {
                1 => {}
                2 => {
                    for row in 0..bh {
                        let r = &mut buf[row * bw * bytes_per..(row + 1) * bw * bytes_per];
                        for x in 1..bw {
                            match bytes_per {
                                1 => r[x] = r[x].wrapping_add(r[x - 1]),
                                2 => {
                                    let a = rd16(r, (x - 1) * 2, be).wrapping_add(rd16(r, x * 2, be));
                                    let v = if be { a.to_be_bytes() } else { a.to_le_bytes() };
                                    r[x * 2..x * 2 + 2].copy_from_slice(&v);
                                }
                                4 => {
                                    let a = rd32(r, (x - 1) * 4, be).wrapping_add(rd32(r, x * 4, be));
                                    let v = if be { a.to_be_bytes() } else { a.to_le_bytes() };
                                    r[x * 4..x * 4 + 4].copy_from_slice(&v);
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                }
                3 => {
                    // floating-point predictor: per row, byte deltas over the
                    // byte planes (most significant plane first)
                    let mut out = vec![0u8; bw * bh * bytes_per];
                    for row in 0..bh {
                        let r = &mut buf[row * bw * bytes_per..(row + 1) * bw * bytes_per];
                        for k in 1..r.len() {
                            r[k] = r[k].wrapping_add(r[k - 1]);
                        }
                        for x in 0..bw {
                            for p in 0..bytes_per {
                                let byte = r[p * bw + x];
                                let dst = row * bw * bytes_per + x * bytes_per;
                                if be {
                                    out[dst + p] = byte
                                } else {
                                    out[dst + bytes_per - 1 - p] = byte
                                }
                            }
                        }
                    }
                    buf = out;
                }
                p => panic!("predictor {p}"),
            }
            for j in 0..bh {
                let y = y_off + j;
                if y >= height {
                    break;
                }
                for i in 0..bw {
                    let x = x_off + i;
                    if x >= width {
                        continue;
                    }
                    let v = sample(&buf, j * bw + i);
                    data[y * width + x] = if v == nodata || v.is_nan() || v < -1000.0 {
                        nodata_count += 1;
                        f32::NAN
                    } else {
                        min = min.min(v);
                        max = max.max(v);
                        v
                    };
                }
            }
        };
        if !l.tile_offsets.is_empty() {
            assert!(l.tile_w > 0 && l.tile_h > 0, "tile size");
            let across = (width + l.tile_w - 1) / l.tile_w;
            for (t, &o) in l.tile_offsets.iter().enumerate() {
                let n = l.tile_counts.get(t).copied().unwrap_or(l.tile_w * l.tile_h * bytes_per);
                place(&b[o..o + n], l.tile_w, l.tile_h, (t % across) * l.tile_w, (t / across) * l.tile_h);
            }
        } else {
            let strips = l.strip_offsets.len();
            assert!(strips > 0, "{}: neither strips nor tiles; tags {:?}", path.display(), l.tags_seen);
            for s in 0..strips {
                let rows = if s + 1 == strips { height - s * l.rows_per_strip } else { l.rows_per_strip.min(height) };
                let o = l.strip_offsets[s];
                let n = l.strip_counts.get(s).copied().unwrap_or(rows * width * bytes_per);
                place(&b[o..o + n], width, rows, 0, s * l.rows_per_strip);
            }
        }
        Raster { width, height, x0, y0, px, data, nodata, min, max, nodata_count }
    }

    pub fn contains(&self, e: f64, n: f64) -> bool {
        e >= self.x0 && e < self.x0 + self.width as f64 * self.px && n <= self.y0 && n > self.y0 - self.height as f64 * self.px
    }

    /// Nearest-pixel value (None off-raster or nodata).
    pub fn at(&self, e: f64, n: f64) -> Option<f32> {
        if !self.contains(e, n) {
            return None;
        }
        let i = ((e - self.x0) / self.px) as usize;
        let j = ((self.y0 - n) / self.px) as usize;
        let v = self.data[j.min(self.height - 1) * self.width + i.min(self.width - 1)];
        if v.is_nan() { None } else { Some(v) }
    }

    /// Bilinear sample at (e, n); None off the raster or on nodata.
    pub fn sample(&self, e: f64, n: f64) -> Option<f64> {
        if !self.contains(e, n) {
            return None;
        }
        // pixel centres sit at x0 + (i + 0.5) px
        let fx = (e - self.x0) / self.px - 0.5;
        let fy = (self.y0 - n) / self.px - 0.5;
        let i0 = fx.floor().max(0.0) as usize;
        let j0 = fy.floor().max(0.0) as usize;
        let i1 = (i0 + 1).min(self.width - 1);
        let j1 = (j0 + 1).min(self.height - 1);
        let tx = (fx - i0 as f64).clamp(0.0, 1.0);
        let ty = (fy - j0 as f64).clamp(0.0, 1.0);
        let g = |i: usize, j: usize| self.data[j * self.width + i] as f64;
        let (a, b, c, d) = (g(i0, j0), g(i1, j0), g(i0, j1), g(i1, j1));
        if a.is_nan() || b.is_nan() || c.is_nan() || d.is_nan() {
            let cands = [a, b, c, d];
            return cands.iter().copied().find(|v| !v.is_nan());
        }
        Some(a * (1.0 - tx) * (1.0 - ty) + b * tx * (1.0 - ty) + c * (1.0 - tx) * ty + d * tx * ty)
    }
}

/// Several tiles of one product: sampled as one surface.
pub struct Mosaic {
    pub tiles: Vec<Raster>,
}

impl Mosaic {
    pub fn load(paths: &[std::path::PathBuf]) -> Mosaic {
        Mosaic { tiles: paths.iter().map(|p| Raster::load(p)).collect() }
    }
    pub fn sample(&self, e: f64, n: f64) -> Option<f64> {
        self.tiles.iter().find(|t| t.contains(e, n)).and_then(|t| t.sample(e, n))
    }
    pub fn at(&self, e: f64, n: f64) -> Option<f32> {
        self.tiles.iter().find(|t| t.contains(e, n)).and_then(|t| t.at(e, n))
    }
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for t in &self.tiles {
            b.0 = b.0.min(t.x0);
            b.1 = b.1.min(t.y0 - t.height as f64 * t.px);
            b.2 = b.2.max(t.x0 + t.width as f64 * t.px);
            b.3 = b.3.max(t.y0);
        }
        b
    }
}

/// TIFF-flavour LZW (MSB-first codes, 9..12 bits, clear 256, end 257, early
/// code-width change).
pub fn lzw_decode(src: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(src.len() * 3);
    // table entries as (prefix code, last byte, length); expanded on output
    let mut prefix: Vec<u32> = Vec::with_capacity(4096);
    let mut last: Vec<u8> = Vec::with_capacity(4096);
    let mut first: Vec<u8> = Vec::with_capacity(4096);
    let mut len: Vec<u32> = Vec::with_capacity(4096);
    let reset = |prefix: &mut Vec<u32>, last: &mut Vec<u8>, first: &mut Vec<u8>, len: &mut Vec<u32>| {
        prefix.clear();
        last.clear();
        first.clear();
        len.clear();
        for i in 0..258u32 {
            prefix.push(u32::MAX);
            last.push(i as u8);
            first.push(i as u8);
            len.push(1);
        }
    };
    reset(&mut prefix, &mut last, &mut first, &mut len);
    let mut width = 9u32;
    let mut bitpos: usize = 0;
    let total_bits = src.len() * 8;
    let mut prev: Option<usize> = None;
    let mut scratch: Vec<u8> = Vec::new();
    let emit = |code: usize, out: &mut Vec<u8>, scratch: &mut Vec<u8>, prefix: &[u32], last: &[u8], len: &[u32]| {
        scratch.clear();
        scratch.resize(len[code] as usize, 0);
        let mut c = code;
        for k in (0..len[code] as usize).rev() {
            scratch[k] = last[c];
            c = prefix[c] as usize;
        }
        out.extend_from_slice(scratch);
    };
    while bitpos + width as usize <= total_bits {
        let mut code = 0u32;
        for _ in 0..width {
            let bit = (src[bitpos / 8] >> (7 - (bitpos % 8))) & 1;
            code = (code << 1) | bit as u32;
            bitpos += 1;
        }
        let code = code as usize;
        if code == 256 {
            reset(&mut prefix, &mut last, &mut first, &mut len);
            width = 9;
            prev = None;
            continue;
        }
        if code == 257 {
            break;
        }
        if code < prefix.len() {
            emit(code, &mut out, &mut scratch, &prefix, &last, &len);
            if let Some(p) = prev {
                prefix.push(p as u32);
                last.push(first[code]);
                first.push(first[p]);
                len.push(len[p] + 1);
            }
        } else if let Some(p) = prev {
            // KwKwK case: prev string + its own first byte
            prefix.push(p as u32);
            last.push(first[p]);
            first.push(first[p]);
            len.push(len[p] + 1);
            emit(code, &mut out, &mut scratch, &prefix, &last, &len);
        } else {
            panic!("lzw: code {code} before any entry");
        }
        prev = Some(code);
        if prefix.len() + 1 >= (1usize << width) && width < 12 {
            width += 1;
        }
    }
    out
}
