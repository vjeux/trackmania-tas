//! Raw rgb24 frames on stdin, and the pixel statistics every reader in this
//! crate is built out of.

use std::io::{Read, Result};

/// One decoded frame: packed rgb24, `w * h * 3` bytes.
pub struct Frame {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

impl Frame {
    pub fn new(w: usize, h: usize) -> Self {
        Frame { w, h, px: vec![0u8; w * h * 3] }
    }

    /// Read the next frame in place. `Ok(false)` at a clean end of stream.
    pub fn read_from(&mut self, r: &mut impl Read) -> Result<bool> {
        let mut got = 0usize;
        while got < self.px.len() {
            match r.read(&mut self.px[got..])? {
                0 => {
                    if got == 0 {
                        return Ok(false);
                    }
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        format!("short frame: {} of {} bytes", got, self.px.len()),
                    ));
                }
                n => got += n,
            }
        }
        Ok(true)
    }

    #[inline]
    pub fn rgb(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let i = (y * self.w + x) * 3;
        (self.px[i], self.px[i + 1], self.px[i + 2])
    }

    /// Rec.601 luma, the channel every glyph here is drawn in.
    #[inline]
    pub fn luma(&self, x: usize, y: usize) -> f32 {
        let (r, g, b) = self.rgb(x, y);
        0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32
    }

    /// `min(r,g,b)`: white-on-bright-background text survives this where luma
    /// does not (a bright cyan pool is luma-bright but has a dark red channel).
    #[inline]
    pub fn minc(&self, x: usize, y: usize) -> f32 {
        let (r, g, b) = self.rgb(x, y);
        r.min(g).min(b) as f32
    }
}

/// An axis-aligned pixel rectangle in frame coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl Rect {
    pub const fn new(x: usize, y: usize, w: usize, h: usize) -> Self {
        Rect { x, y, w, h }
    }

    pub fn inset(&self, d: usize) -> Rect {
        Rect { x: self.x + d, y: self.y + d, w: self.w - 2 * d, h: self.h - 2 * d }
    }

    /// Median luma over the rectangle. Median, not mean, so the white arrow
    /// glyph painted inside a lamp cannot drag the reading.
    pub fn median_luma(&self, f: &Frame) -> f32 {
        let mut v: Vec<f32> = Vec::with_capacity(self.w * self.h);
        for y in self.y..self.y + self.h {
            for x in self.x..self.x + self.w {
                v.push(f.luma(x, y));
            }
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    }

    /// Median luma of the `d`-pixel ring just inside the rectangle's edge.
    pub fn median_border_luma(&self, f: &Frame, d: usize) -> f32 {
        let mut v: Vec<f32> = Vec::new();
        for y in self.y..self.y + self.h {
            let edge_row = y < self.y + d || y >= self.y + self.h - d;
            for x in self.x..self.x + self.w {
                if edge_row || x < self.x + d || x >= self.x + self.w - d {
                    v.push(f.luma(x, y));
                }
            }
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    }
}

impl Rect {
    /// `p`-th percentile of luma over the rectangle, 0..=100.
    pub fn pct_luma(&self, f: &Frame, p: usize) -> f32 {
        let mut v: Vec<f32> = Vec::with_capacity(self.w * self.h);
        for y in self.y..self.y + self.h {
            for x in self.x..self.x + self.w {
                v.push(f.luma(x, y));
            }
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[(v.len() - 1) * p / 100]
    }
}
