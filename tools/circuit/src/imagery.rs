//! Aerial imagery as a British National Grid raster, for reading a layout
//! that post-dates the LIDAR (Kart Silverstone opened in 2025; the newest
//! intensity raster is 2019). Google's zoom-20 tiles are ~0.09 m/px on the
//! ground; they are resampled onto a 0.1 m BNG grid so the same
//! `(e, n)` lookups the LIDAR code uses work here.
//!
//! What the pixels are: tarmac is grey (low saturation, mid brightness),
//! grass is green, kerbs and painted lines are white, tyre barriers and
//! shadows are dark. Calibrated on Google's 2026 imagery of the kart track
//! (`circuit img-profile`).

use crate::aerial::{bng_to_wgs84, fetch_tiles, mercator, Source};
use crate::geo::Bng;
use crate::track::Track;

pub struct Imagery {
    /// Bottom-left corner (BNG) and pixel size (m).
    pub e0: f64,
    pub n0: f64,
    pub px: f64,
    pub w: usize,
    pub h: usize,
    /// Row 0 is the NORTH edge (like the TIFFs).
    pub rgb: Vec<[u8; 3]>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Class {
    Asphalt,
    Grass,
    White,
    Dark,
    /// Bare earth, gravel, sand: warm and desaturated-ish (the loop infield).
    Dirt,
    Other,
}

pub fn classify(c: [u8; 3]) -> Class {
    // Calibrated on Google 2026 imagery of Kart Silverstone: asphalt is a
    // warm grey, r >= g >= b with g-b under ~14 ((117,106,102), (163,155,153),
    // (94,85,78)); grass is olive, g >= r-3 with b well below ((98,101,84),
    // (76,80,47), (137,136,108)); kerbs (225,227,222); dirt (147,135,109).
    let (r, g, b) = (c[0] as i32, c[1] as i32, c[2] as i32);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let sat = max - min;
    if max < 65 {
        return Class::Dark;
    }
    if max > 185 && sat < 45 {
        return Class::White;
    }
    if g >= r - 3 && g - b >= 15 {
        return Class::Grass;
    }
    if r - g >= 5 && g - b >= 18 {
        return Class::Dirt;
    }
    if sat <= 45 && max <= 185 && g - b <= 16 {
        return Class::Asphalt;
    }
    Class::Other
}

impl Imagery {
    /// Fetch and resample the tiles over a BNG box (plus `margin` m).
    pub fn fetch(bbox: (f64, f64, f64, f64), margin: f64, px: f64, zoom: u32, src: Source) -> Result<Imagery, String> {
        let (e0, n0, e1, n1) = (bbox.0 - margin, bbox.1 - margin, bbox.2 + margin, bbox.3 + margin);
        let (tw, th, tiles, mb) = fetch_tiles((e0, n0, e1, n1), zoom, src)?;
        let w = ((e1 - e0) / px).ceil() as usize;
        let h = ((n1 - n0) / px).ceil() as usize;
        // BNG -> Mercator on a 10 m lattice, bilinear in between (the exact
        // map is smooth to well under a millimetre over 10 m)
        let step = 10.0;
        let gw = ((e1 - e0) / step).ceil() as usize + 1;
        let gh = ((n1 - n0) / step).ceil() as usize + 1;
        let mut lat_xy: Vec<(f64, f64)> = Vec::with_capacity(gw * gh);
        for j in 0..gh {
            for i in 0..gw {
                let (lat, lon) = bng_to_wgs84(e0 + i as f64 * step, n1 - j as f64 * step);
                lat_xy.push(mercator(lat, lon));
            }
        }
        let mut rgb = vec![[0u8; 3]; w * h];
        for y in 0..h {
            let n = n1 - (y as f64 + 0.5) * px;
            let gj = ((n1 - n) / step).clamp(0.0, (gh - 1) as f64 - 1e-9);
            let (j0, fj) = (gj.floor() as usize, gj.fract());
            for x in 0..w {
                let e = e0 + (x as f64 + 0.5) * px;
                let gi = ((e - e0) / step).clamp(0.0, (gw - 1) as f64 - 1e-9);
                let (i0, fi) = (gi.floor() as usize, gi.fract());
                let a = lat_xy[j0 * gw + i0];
                let b = lat_xy[j0 * gw + i0 + 1];
                let c = lat_xy[(j0 + 1) * gw + i0];
                let d = lat_xy[(j0 + 1) * gw + i0 + 1];
                let mx = (a.0 * (1.0 - fi) + b.0 * fi) * (1.0 - fj) + (c.0 * (1.0 - fi) + d.0 * fi) * fj;
                let my = (a.1 * (1.0 - fi) + b.1 * fi) * (1.0 - fj) + (c.1 * (1.0 - fi) + d.1 * fi) * fj;
                let tx = ((mx - mb.0) / (mb.2 - mb.0) * tw as f64) as i64;
                let ty = ((mb.3 - my) / (mb.3 - mb.1) * th as f64) as i64;
                if tx >= 0 && ty >= 0 && (tx as usize) < tw && (ty as usize) < th {
                    rgb[y * w + x] = tiles[ty as usize * tw + tx as usize];
                }
            }
        }
        Ok(Imagery { e0, n0, px, w, h, rgb })
    }

    pub fn rgb(&self, e: f64, n: f64) -> Option<[u8; 3]> {
        let x = ((e - self.e0) / self.px).floor();
        let y = ((self.n0 + self.h as f64 * self.px - n) / self.px).floor();
        if x < 0.0 || y < 0.0 || x >= self.w as f64 || y >= self.h as f64 {
            return None;
        }
        Some(self.rgb[y as usize * self.w + x as usize])
    }

    pub fn class(&self, e: f64, n: f64) -> Option<Class> {
        self.rgb(e, n).map(classify)
    }

    /// Is the ground asphalt here? A majority vote over a `step` m square
    /// (for the terrain classifier): None outside the imagery.
    pub fn asphalt_majority(&self, e: f64, n: f64, step: f64) -> Option<bool> {
        let mut seen = 0;
        let mut asphalt = 0;
        let mut green = 0;
        let k = 4;
        for j in 0..k {
            for i in 0..k {
                let ee = e - step / 2.0 + (i as f64 + 0.5) * step / k as f64;
                let nn = n - step / 2.0 + (j as f64 + 0.5) * step / k as f64;
                match self.class(ee, nn) {
                    None => {}
                    Some(Class::Asphalt) | Some(Class::White) => {
                        seen += 1;
                        asphalt += 1;
                    }
                    Some(Class::Grass) | Some(Class::Dirt) => {
                        seen += 1;
                        green += 1;
                    }
                    Some(_) => seen += 1,
                }
            }
        }
        if seen == 0 {
            return None;
        }
        // shadows/barriers/cars are neither: the vote is between what was read
        Some(asphalt > green)
    }

    /// Save as PNG with optional polylines drawn (for looking at).
    pub fn save_png(&self, out: &std::path::Path, lines: &[(Vec<Bng>, [u8; 3])]) {
        let mut img = crate::png::Image::new(self.w, self.h, [0, 0, 0]);
        for y in 0..self.h {
            for x in 0..self.w {
                img.put(x as i64, y as i64, self.rgb[y * self.w + x]);
            }
        }
        let n1 = self.n0 + self.h as f64 * self.px;
        for (pts, c) in lines {
            for q in pts.windows(2) {
                img.line((q[0].e - self.e0) / self.px, (n1 - q[0].n) / self.px, (q[1].e - self.e0) / self.px, (n1 - q[1].n) / self.px, *c);
            }
        }
        img.save(out);
    }
}

/// One edge reading: offset from the centreline (m, positive) and the
/// width of the white kerb band just outside it.
#[derive(Clone, Copy, Debug)]
pub struct EdgeRead {
    pub edge: f64,
    pub kerb: f64,
}

/// Walk outwards along one side of station `i` (`side` +1 left, -1 right)
/// and find where the tarmac ends: the start of the first run of at least
/// `run` metres that is not asphalt, provided it is not merely a shadow
/// (a dark run counts only when grass or white follows within 1.5 m).
pub fn read_edge(img: &Imagery, tr: &Track, i: usize, side: f64, min_half: f64, max_half: f64, run: f64) -> Option<EdgeRead> {
    let step = 0.1;
    let n = ((max_half + 3.0) / step) as usize;
    let classes: Vec<Option<Class>> = (0..=n)
        .map(|k| {
            let p = tr.offset(i, side * k as f64 * step);
            img.class(p[0], p[1])
        })
        .collect();
    let is_asphalt = |c: Option<Class>| matches!(c, Some(Class::Asphalt) | Some(Class::Other));
    let run_n = (run / step).round() as usize;
    let mut k = (min_half / step) as usize;
    while (k as f64) * step <= max_half {
        if !is_asphalt(classes[k]) {
            // a run of non-asphalt from k
            let mut j = k;
            while j <= n && !is_asphalt(classes[j]) {
                j += 1;
            }
            let len = j - k;
            if len >= run_n {
                // grass or white anywhere in the first 1.5 m past k?
                let real = (k..(k + 15).min(j)).any(|q| matches!(classes[q], Some(Class::Grass) | Some(Class::White) | Some(Class::Dirt)));
                if real || j > n {
                    // kerb: the white run starting at k (allow one dark/other sample first)
                    let mut kw = 0usize;
                    let mut q = k;
                    while q < j && matches!(classes[q], Some(Class::White) | Some(Class::Dark) | Some(Class::Other)) && kw < 30 {
                        if classes[q] == Some(Class::White) {
                            kw += 1;
                        } else if kw == 0 && q > k + 3 {
                            break;
                        }
                        q += 1;
                    }
                    let kerb = if kw >= 2 { (q - k) as f64 * step } else { 0.0 };
                    return Some(EdgeRead { edge: k as f64 * step, kerb: kerb.min(1.6) });
                }
            }
            k = j.max(k + 1);
            continue;
        }
        k += 1;
    }
    None
}

impl Imagery {
    /// A sub-image over a BNG box (clamped to what there is).
    pub fn crop(&self, bbox: (f64, f64, f64, f64)) -> Imagery {
        let n1 = self.n0 + self.h as f64 * self.px;
        let x0 = (((bbox.0 - self.e0) / self.px).floor().max(0.0)) as usize;
        let x1 = (((bbox.2 - self.e0) / self.px).ceil().max(0.0) as usize).min(self.w);
        let y0 = (((n1 - bbox.3) / self.px).floor().max(0.0)) as usize;
        let y1 = (((n1 - bbox.1) / self.px).ceil().max(0.0) as usize).min(self.h);
        let (w, h) = (x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1));
        let mut rgb = vec![[0u8; 3]; w * h];
        for y in 0..h {
            for x in 0..w {
                if y0 + y < self.h && x0 + x < self.w {
                    rgb[y * w + x] = self.rgb[(y0 + y) * self.w + x0 + x];
                }
            }
        }
        Imagery { e0: self.e0 + x0 as f64 * self.px, n0: n1 - y1 as f64 * self.px, px: self.px, w, h, rgb }
    }
}
