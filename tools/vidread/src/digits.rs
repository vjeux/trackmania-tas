//! A fixed-position numeric readout, read by template correlation.
//!
//! Five defects this is shaped around, all of them paid for once already in
//! `hudread`: cells are fixed from the summed ink profile, never per frame;
//! patches of different sizes are never compared; the score is
//! contrast-normalised correlation, not mean absolute difference; thin italic
//! glyphs are matched through a soft blurred ink ramp rather than a binary
//! mask; and the field's digits shift by a pixel or two as their neighbours
//! change width, so every cell is matched over a small shift window.

use crate::frame::Frame;
use std::collections::BTreeMap;
use std::io::Write;

/// Where the digits of one readout sit, in frame pixels.
#[derive(Clone, Debug)]
pub struct Field {
    /// Left edge of every digit cell. A clock is not on a uniform pitch — the
    /// colon and the point are narrower than a digit — so the cells are listed
    /// rather than derived from a single spacing.
    pub xs: Vec<f32>,
    pub y0: f32,
    pub pw: usize,
    pub ph: usize,
}

impl Field {
    /// `x0,y0,cw,pw,ph,cells` for a uniform pitch, or
    /// `x1:x2:...;y0;pw;ph` to list the cells.
    pub fn parse(s: &str) -> Field {
        if s.contains(';') {
            let p: Vec<&str> = s.split(';').collect();
            assert_eq!(p.len(), 4, "field spec is x1:x2:...;y0;pw;ph");
            return Field {
                xs: p[0].split(':').map(|v| v.trim().parse().expect("cell x")).collect(),
                y0: p[1].trim().parse().expect("y0"),
                pw: p[2].trim().parse().expect("pw"),
                ph: p[3].trim().parse().expect("ph"),
            };
        }
        let n: Vec<f32> = s.split(',').map(|v| v.trim().parse().expect("field spec")).collect();
        assert_eq!(n.len(), 6, "field spec is x0,y0,cw,pw,ph,cells");
        Field {
            xs: (0..n[5] as usize).map(|k| n[0] + n[2] * k as f32).collect(),
            y0: n[1],
            pw: n[3] as usize,
            ph: n[4] as usize,
        }
    }

    pub fn cells(&self) -> usize {
        self.xs.len()
    }
}

/// A contrast-normalised, softened patch. `v` has zero mean and unit norm, so
/// correlation with another patch is a plain dot product in [-1, 1].
#[derive(Clone)]
pub struct Patch {
    pub w: usize,
    pub h: usize,
    pub v: Vec<f32>,
}

impl Patch {
    /// Cut cell `k` out of `f`, offset by (`dx`, `dy`) pixels.
    pub fn cut(f: &Frame, fd: &Field, k: usize, dx: i32, dy: i32) -> Patch {
        let (w, h) = (fd.pw, fd.ph);
        let ox = fd.xs[k].round() as i32 + dx;
        let oy = fd.y0.round() as i32 + dy;
        let mut raw = vec![0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let sx = (ox + x as i32).clamp(0, f.w as i32 - 1) as usize;
                let sy = (oy + y as i32).clamp(0, f.h as i32 - 1) as usize;
                raw[y * w + x] = f.minc(sx, sy);
            }
        }
        Patch::finish(w, h, raw)
    }

    /// 3x3 box blur, then centre and normalise.
    fn finish(w: usize, h: usize, raw: Vec<f32>) -> Patch {
        let mut v = vec![0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let mut s = 0.0;
                let mut n = 0.0;
                for j in y.saturating_sub(1)..(y + 2).min(h) {
                    for i in x.saturating_sub(1)..(x + 2).min(w) {
                        s += raw[j * w + i];
                        n += 1.0;
                    }
                }
                v[y * w + x] = s / n;
            }
        }
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        for p in v.iter_mut() {
            *p -= mean;
        }
        let norm = v.iter().map(|p| p * p).sum::<f32>().sqrt().max(1e-6);
        for p in v.iter_mut() {
            *p /= norm;
        }
        Patch { w, h, v }
    }

    pub fn dot(&self, o: &Patch) -> f32 {
        assert_eq!(self.w, o.w, "patch widths differ");
        assert_eq!(self.h, o.h, "patch heights differ");
        self.v.iter().zip(o.v.iter()).map(|(a, b)| a * b).sum()
    }
}

/// One template per glyph, keyed by the character it draws. When `per_cell`
/// is set the key is `cell*16 + digit_index` instead: a fractional cell pitch
/// gives every column its own sub-pixel phase, and a font that is shared
/// across columns still needs one template per column.
pub struct Templates {
    pub w: usize,
    pub h: usize,
    pub per_cell: bool,
    pub g: BTreeMap<(usize, char), Patch>,
}

impl Templates {
    /// Average the accumulated samples per glyph and re-normalise.
    pub fn from_samples(
        w: usize,
        h: usize,
        per_cell: bool,
        samples: &BTreeMap<(usize, char), Vec<Patch>>,
    ) -> Templates {
        let mut g = BTreeMap::new();
        for (k, list) in samples {
            let mut acc = vec![0f32; w * h];
            for p in list {
                for i in 0..acc.len() {
                    acc[i] += p.v[i];
                }
            }
            let mean = acc.iter().sum::<f32>() / acc.len() as f32;
            for p in acc.iter_mut() {
                *p -= mean;
            }
            let norm = acc.iter().map(|p| p * p).sum::<f32>().sqrt().max(1e-6);
            for p in acc.iter_mut() {
                *p /= norm;
            }
            g.insert(*k, Patch { w, h, v: acc });
        }
        Templates { w, h, per_cell, g }
    }

    pub fn write(&self, o: &mut impl Write) {
        writeln!(o, "{} {} {} {}", self.w, self.h, self.g.len(), self.per_cell as u8).unwrap();
        for ((cell, c), p) in &self.g {
            write!(o, "{} {}", cell, c).unwrap();
            for v in &p.v {
                write!(o, " {:.5}", v).unwrap();
            }
            writeln!(o).unwrap();
        }
    }

    pub fn read(s: &str) -> Templates {
        let mut lines = s.lines();
        let hdr: Vec<usize> =
            lines.next().unwrap().split_whitespace().map(|v| v.parse().unwrap()).collect();
        let (w, h, per_cell) = (hdr[0], hdr[1], hdr[3] == 1);
        let mut g = BTreeMap::new();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let mut it = line.split_whitespace();
            let cell: usize = it.next().unwrap().parse().unwrap();
            let c = it.next().unwrap().chars().next().unwrap();
            let v: Vec<f32> = it.map(|x| x.parse().unwrap()).collect();
            assert_eq!(v.len(), w * h, "template {cell}:{c} is the wrong size");
            g.insert((cell, c), Patch { w, h, v });
        }
        Templates { w, h, per_cell, g }
    }

    /// Read the whole field at one shared shift, and return the total score.
    /// The digits of a readout move together — the field is one sprite — so a
    /// shift is a property of the field, not of a cell.
    fn read_at(&self, f: &Frame, fd: &Field, dx: i32, dy: i32) -> (Vec<(char, f32, f32)>, f32) {
        let mut out = Vec::with_capacity(fd.cells());
        let mut total = 0.0;
        for k in 0..fd.cells() {
            let bank = if self.per_cell { k } else { 0 };
            let p = Patch::cut(f, fd, k, dx, dy);
            let mut best = ('?', -2.0f32);
            let mut second = -2.0f32;
            for ((b, c), t) in self.g.iter() {
                if *b != bank {
                    continue;
                }
                let d = p.dot(t);
                if d > best.1 {
                    second = best.1;
                    best = (*c, d);
                } else if d > second {
                    second = d;
                }
            }
            total += best.1;
            out.push((best.0, best.1, second));
        }
        (out, total)
    }

    /// Read the field, searching a shared shift window for the best placement.
    /// Returns the per-cell readings and the shift that produced them.
    pub fn read_field(
        &self,
        f: &Frame,
        fd: &Field,
        sx: i32,
        sy: i32,
    ) -> (Vec<(char, f32, f32)>, i32, i32) {
        let mut best = (Vec::new(), f32::MIN, 0, 0);
        for dy in -sy..=sy {
            for dx in -sx..=sx {
                let (cells, total) = self.read_at(f, fd, dx, dy);
                if total > best.1 {
                    best = (cells, total, dx, dy);
                }
            }
        }
        (best.0, best.2, best.3)
    }
}
