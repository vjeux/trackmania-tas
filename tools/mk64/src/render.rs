//! A top-down textured render of a course mesh — the sanity check before any
//! Trackmania file exists (and the "what the N64 data says" half of every
//! side-by-side). Orthographic, y-buffered, nearest texel, vertex colour
//! modulated like the N64 does.

use crate::mesh::{Corner, Mesh};
use crate::texture::Image;
use std::collections::HashMap;

pub struct TopDown {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
    /// World x of column 0 and world z of row 0, metres per pixel.
    pub x0: f32,
    pub z0: f32,
    pub m_per_px: f32,
}

impl TopDown {
    /// Pixel of a world point (row 0 = north = max z).
    pub fn px(&self, x: f32, z: f32) -> (i32, i32) {
        (((x - self.x0) / self.m_per_px) as i32, ((self.z0 - z) / self.m_per_px) as i32)
    }
    pub fn dot(&mut self, x: f32, z: f32, r: i32, rgb: [u8; 3]) {
        let (cx, cy) = self.px(x, z);
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (px, py) = (cx + dx, cy + dy);
                if px >= 0 && py >= 0 && (px as usize) < self.w && (py as usize) < self.h {
                    let i = (py as usize * self.w + px as usize) * 3;
                    self.rgb[i..i + 3].copy_from_slice(&rgb);
                }
            }
        }
    }
    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), rgb: [u8; 3]) {
        let (x0, y0) = self.px(a.0, a.1);
        let (x1, y1) = self.px(b.0, b.1);
        let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for k in 0..=n {
            let t = k as f32 / n as f32;
            let x = x0 as f32 + (x1 - x0) as f32 * t;
            let y = y0 as f32 + (y1 - y0) as f32 * t;
            if x >= 0.0 && y >= 0.0 && (x as usize) < self.w && (y as usize) < self.h {
                let i = (y as usize * self.w + x as usize) * 3;
                self.rgb[i..i + 3].copy_from_slice(&rgb);
            }
        }
    }
    pub fn png(&self) -> Vec<u8> {
        mapgeom::render::png(&mapgeom::render::Image { w: self.w, h: self.h, rgb: self.rgb.clone() })
    }
}

/// Render `mesh` from above at `max_px` pixels on the longer side. `textures`
/// maps material index → image (already mirrored where the material says).
pub fn top_down(mesh: &Mesh, textures: &HashMap<usize, Image>, max_px: usize) -> TopDown {
    let (lo, hi) = crate::mesh::bbox(mesh.tris.iter().flat_map(|t| t.c.iter().map(|c| c.pos))).unwrap_or(([0.0; 3], [1.0; 3]));
    let span_x = (hi[0] - lo[0]).max(1.0);
    let span_z = (hi[2] - lo[2]).max(1.0);
    let m_per_px = span_x.max(span_z) / max_px as f32;
    let margin = 8.0 * m_per_px;
    let w = ((span_x + 2.0 * margin) / m_per_px).ceil() as usize;
    let h = ((span_z + 2.0 * margin) / m_per_px).ceil() as usize;
    let mut img = TopDown { w, h, rgb: vec![24; w * h * 3], x0: lo[0] - margin, z0: hi[2] + margin, m_per_px };
    let mut ybuf = vec![f32::NEG_INFINITY; w * h];
    // draw in order, higher y wins (a bridge over a road shows the bridge)
    for t in &mesh.tris {
        raster(&mut img, &mut ybuf, t, textures);
    }
    img
}

fn raster(img: &mut TopDown, ybuf: &mut [f32], t: &crate::mesh::Tri, textures: &HashMap<usize, Image>) {
    let tex = t.mat.and_then(|m| textures.get(&m));
    let to_px = |c: &Corner| -> (f32, f32) { ((c.pos[0] - img.x0) / img.m_per_px, (img.z0 - c.pos[2]) / img.m_per_px) };
    let p = [to_px(&t.c[0]), to_px(&t.c[1]), to_px(&t.c[2])];
    let min_x = p.iter().map(|q| q.0).fold(f32::INFINITY, f32::min).floor().max(0.0) as i64;
    let max_x = p.iter().map(|q| q.0).fold(f32::NEG_INFINITY, f32::max).ceil().min(img.w as f32 - 1.0) as i64;
    let min_y = p.iter().map(|q| q.1).fold(f32::INFINITY, f32::min).floor().max(0.0) as i64;
    let max_y = p.iter().map(|q| q.1).fold(f32::NEG_INFINITY, f32::max).ceil().min(img.h as f32 - 1.0) as i64;
    if min_x > max_x || min_y > max_y {
        return;
    }
    let area = edge(p[0], p[1], p[2]);
    if area.abs() < 1e-9 {
        return;
    }
    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let q = (px as f32 + 0.5, py as f32 + 0.5);
            let w0 = edge(p[1], p[2], q) / area;
            let w1 = edge(p[2], p[0], q) / area;
            let w2 = edge(p[0], p[1], q) / area;
            if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                continue;
            }
            let y = w0 * t.c[0].pos[1] + w1 * t.c[1].pos[1] + w2 * t.c[2].pos[1];
            let i = py as usize * img.w + px as usize;
            if y < ybuf[i] {
                continue;
            }
            let col = |k: usize| [t.c[k].rgba[0] as f32, t.c[k].rgba[1] as f32, t.c[k].rgba[2] as f32];
            let (c0, c1, c2) = (col(0), col(1), col(2));
            let mut rgb = [w0 * c0[0] + w1 * c1[0] + w2 * c2[0], w0 * c0[1] + w1 * c1[1] + w2 * c2[1], w0 * c0[2] + w1 * c1[2] + w2 * c2[2]];
            if t.lit {
                rgb = [200.0, 200.0, 200.0];
            }
            if let Some(tex) = tex {
                let u = w0 * t.c[0].uv[0] + w1 * t.c[1].uv[0] + w2 * t.c[2].uv[0];
                let v = w0 * t.c[0].uv[1] + w1 * t.c[1].uv[1] + w2 * t.c[2].uv[1];
                let tx = ((u.rem_euclid(1.0)) * tex.w as f32) as u32;
                let ty = (((1.0 - v).rem_euclid(1.0)) * tex.h as f32) as u32; // uv v runs bottom-up (the game's convention)
                let px4 = tex.pixel(tx.min(tex.w - 1), ty.min(tex.h - 1));
                if px4[3] < 128 {
                    continue; // alpha cutout
                }
                for k in 0..3 {
                    rgb[k] = rgb[k] * px4[k] as f32 / 255.0;
                }
            } else {
                // untextured: the vertex colour
            }
            ybuf[i] = y;
            let o = i * 3;
            img.rgb[o] = rgb[0].clamp(0.0, 255.0) as u8;
            img.rgb[o + 1] = rgb[1].clamp(0.0, 255.0) as u8;
            img.rgb[o + 2] = rgb[2].clamp(0.0, 255.0) as u8;
        }
    }
}

fn edge(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}
