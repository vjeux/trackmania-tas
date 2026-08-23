//! A picture of the model, so it can be looked at without a 3D viewer.
//!
//! An orthographic top-down raster: for every pixel, the highest triangle over
//! that spot, coloured by its physics material and shaded by height. Then the
//! run's trajectory over the top. This is deliberately the cheapest useful
//! view — the `.glb` is the real artefact — but "here is the map and here is
//! the line the car drove" answers most questions on its own, and it needs no
//! viewer and no client.
//!
//! PNG is written here rather than pulled in: the format is a CRC, a zlib
//! stream (`miniz_oxide`, already a dependency) and four chunks.

use crate::scene::Scene;

pub struct Image {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
}

/// Render a scene top-down. `px_per_m` sets the resolution; the frame is the
/// scene's own bounds.
/// Render a scene top-down, ignoring everything above `clip_y`.
///
/// The clip is not cosmetic. A stadium has a ROOF, and a top-down view of a
/// map with its decoration is a picture of the roof: the track is under
/// 100 000 square metres of canopy. Clipping just above the run is what makes
/// the picture a picture of the track.
pub fn top_down(scene: &Scene, px_per_m: f32, max_px: usize, clip_y: f32) -> Image {
    let (mut lo, mut hi) = scene.bounds().unwrap_or(([0.0; 3], [1.0; 3]));
    // Frame on the RUN when there is one. A stadium is 1.5 km across and a
    // track is often a few hundred metres of it; framing on the scene makes a
    // picture of the stands.
    if !scene.lines.is_empty() {
        let (mut a, mut b) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for l in &scene.lines {
            for p in &l.points {
                for k in 0..3 {
                    a[k] = a[k].min(p[k]);
                    b[k] = b[k].max(p[k]);
                }
            }
        }
        const MARGIN: f32 = 48.0;
        lo[0] = a[0] - MARGIN;
        hi[0] = b[0] + MARGIN;
        lo[2] = a[2] - MARGIN;
        hi[2] = b[2] + MARGIN;
    }
    let mut s = px_per_m;
    let mut w = (((hi[0] - lo[0]) * s).ceil() as usize).max(1);
    let mut h = (((hi[2] - lo[2]) * s).ceil() as usize).max(1);
    if w.max(h) > max_px {
        s *= max_px as f32 / w.max(h) as f32;
        w = (((hi[0] - lo[0]) * s).ceil() as usize).max(1);
        h = (((hi[2] - lo[2]) * s).ceil() as usize).max(1);
    }
    let mut depth = vec![f32::NEG_INFINITY; w * h];
    let mut rgb = vec![18u8; w * h * 3];

    let (ylo, yhi) = (lo[1], hi[1].max(lo[1] + 1.0));
    for (name, g) in &scene.groups {
        let c = crate::scene::colour_for(name);
        for t in &g.tris {
            let p = [g.verts[t[0] as usize], g.verts[t[1] as usize], g.verts[t[2] as usize]];
            if p.iter().all(|v| v[1] > clip_y) {
                continue;
            }
            // Pixel coordinates; +Z is north, so it runs UP the image.
            let px: Vec<(f32, f32, f32)> = p
                .iter()
                .map(|v| ((v[0] - lo[0]) * s, (hi[2] - v[2]) * s, v[1]))
                .collect();
            let minx = px.iter().map(|q| q.0).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize;
            let maxx =
                (px.iter().map(|q| q.0).fold(f32::NEG_INFINITY, f32::max).ceil() as usize).min(w - 1);
            let miny = px.iter().map(|q| q.1).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize;
            let maxy =
                (px.iter().map(|q| q.1).fold(f32::NEG_INFINITY, f32::max).ceil() as usize).min(h - 1);
            if minx > maxx || miny > maxy {
                continue;
            }
            let d = (px[1].1 - px[2].1) * (px[0].0 - px[2].0)
                + (px[2].0 - px[1].0) * (px[0].1 - px[2].1);
            if d.abs() < 1e-9 {
                continue;
            }
            for y in miny..=maxy {
                for x in minx..=maxx {
                    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((px[1].1 - px[2].1) * (fx - px[2].0)
                        + (px[2].0 - px[1].0) * (fy - px[2].1))
                        / d;
                    let w1 = ((px[2].1 - px[0].1) * (fx - px[2].0)
                        + (px[0].0 - px[2].0) * (fy - px[2].1))
                        / d;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * px[0].2 + w1 * px[1].2 + w2 * px[2].2;
                    if px[0].0 < 0.0 && px[1].0 < 0.0 && px[2].0 < 0.0 {
                        continue;
                    }
                    let i = y * w + x;
                    if z <= depth[i] {
                        continue;
                    }
                    depth[i] = z;
                    // Height shading: 55 % at the floor, full at the top.
                    let k = 0.55 + 0.45 * ((z - ylo) / (yhi - ylo)).clamp(0.0, 1.0);
                    for ch in 0..3 {
                        rgb[i * 3 + ch] = (c[ch] * k * 255.0).clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }

    for l in &scene.lines {
        let col = [
            (l.colour[0] * 255.0) as u8,
            (l.colour[1] * 255.0) as u8,
            (l.colour[2] * 255.0) as u8,
        ];
        let mut prev: Option<(i64, i64)> = None;
        for p in &l.points {
            let x = ((p[0] - lo[0]) * s).round() as i64;
            let y = ((hi[2] - p[2]) * s).round() as i64;
            if let Some((a, b)) = prev {
                line(&mut rgb, w, h, a, b, x, y, col);
            }
            prev = Some((x, y));
        }
    }
    Image { w, h, rgb }
}

fn line(rgb: &mut [u8], w: usize, h: usize, x0: i64, y0: i64, x1: i64, y1: i64, c: [u8; 3]) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        // Two pixels wide, so a trajectory stays visible over a busy map.
        for (ox, oy) in [(0, 0), (1, 0), (0, 1)] {
            let (px, py) = (x + ox, y + oy);
            if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                let i = (py as usize * w + px as usize) * 3;
                rgb[i..i + 3].copy_from_slice(&c);
            }
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

// --------------------------------------------------------------------- PNG

pub fn png(img: &Image) -> Vec<u8> {
    let mut raw = Vec::with_capacity(img.h * (1 + img.w * 3));
    for y in 0..img.h {
        raw.push(0); // filter: none
        raw.extend_from_slice(&img.rgb[y * img.w * 3..(y + 1) * img.w * 3]);
    }
    let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);

    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(img.w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(img.h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, e) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *e = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = table[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The PNG has to be a PNG. A viewer that refuses the file says nothing
    /// about the geometry, so the container is pinned on its own.
    #[test]
    fn png_container_is_well_formed() {
        let mut s = Scene::default();
        s.add_tris(
            "Concrete",
            &[[0.0, 0.0, 0.0], [10.0, 1.0, 0.0], [0.0, 0.0, 10.0]],
            [[0, 1, 2]].into_iter(),
        );
        let img = top_down(&s, 4.0, 512, f32::INFINITY);
        assert!(img.w > 1 && img.h > 1);
        let b = png(&img);
        assert_eq!(&b[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(&b[12..16], b"IHDR");
        assert_eq!(u32::from_be_bytes(b[16..20].try_into().unwrap()) as usize, img.w);
        assert_eq!(&b[b.len() - 8..b.len() - 4], b"IEND");
        // Something was actually drawn: not every pixel is the background.
        assert!(img.rgb.iter().any(|p| *p != 18));
    }

    /// CRC-32 against the value the PNG spec's own IEND chunk carries.
    #[test]
    fn crc32_matches_the_known_iend() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }
}
