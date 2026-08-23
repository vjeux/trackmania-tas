//! The tyre-wetness readout: a percentage whose digits move.
//!
//! Speed sits in a fixed three-cell field. This one does not: it is drawn
//! left-aligned after a droplet icon as `43%`, `20%`, `100%`, so both the digit
//! count and every cell's position change with the value. Two anchors make it
//! readable anyway — the **droplet**, which says the readout is on screen at
//! all, and the **`%`**, which the digits are right-aligned against.
//!
//! Locating the `%` per frame and reading leftwards from it is what makes this
//! work, and it is also the guard against the trap this box sets: the same
//! line draws `! Slip` when the car is sliding. A reader that assumes digits
//! decodes `S`, `l`, `i`, `p` as numbers. Requiring the `%` before reading
//! anything refuses those frames instead.

use crate::digits::{Field, Patch, Templates};
use crate::frame::{Frame, Rect};

/// The droplet icon, on the 2560x1440 master.
pub const ICON: Rect = Rect::new(2119, 1229, 12, 15);
/// Where the `%` can be, and the glyph box every cell is cut with.
pub const PCT_X: (usize, usize) = (2140, 2180);
pub const CELL_Y: usize = 1230;
pub const CELL_W: usize = 12;
pub const CELL_H: usize = 15;
/// Distance between the left edges of neighbouring digit cells.
pub const PITCH: f32 = 9.6;

/// Is the readout on screen? Contrast, not level: the box sits over everything
/// from a dark tunnel to a white wall, so an absolute threshold measures the
/// scenery rather than the icon.
pub fn icon_present(f: &Frame, span_min: f32) -> bool {
    let mut v: Vec<f32> = Vec::with_capacity(ICON.w * ICON.h);
    for y in ICON.y..ICON.y + ICON.h {
        for x in ICON.x..ICON.x + ICON.w {
            v.push(f.minc(x, y));
        }
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[(v.len() - 1) * 95 / 100] - v[(v.len() - 1) * 5 / 100] >= span_min
}

fn cell(x: usize) -> Field {
    Field::parse(&format!("{x};{CELL_Y};{CELL_W};{CELL_H}"))
}

/// Where the `%` is, and how well it matched.
pub fn find_pct(f: &Frame, t: &Templates) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;
    for x in PCT_X.0..=PCT_X.1 {
        let p = Patch::cut(f, &cell(x), 0, 0, 0);
        let s = t.g.get(&(0, '%')).map(|q| p.dot(q)).unwrap_or(-2.0);
        if best.map_or(true, |(_, b)| s > b) {
            best = Some((x, s));
        }
    }
    best
}

pub struct Reading {
    pub text: String,
    pub value: Option<f64>,
    pub pct_x: usize,
    pub pct_score: f32,
    pub worst: f32,
}

/// Read the field on one frame. `None` when the droplet is not on screen.
pub fn read(f: &Frame, t: &Templates, span_min: f32, pct_min: f32, digit_min: f32) -> Option<Reading> {
    if !icon_present(f, span_min) {
        return None;
    }
    let (px, ps) = find_pct(f, t)?;
    if ps < pct_min {
        // No per-cent sign: this is the `! Slip` line, or the box is
        // illegible. Either way there is no number here to read.
        return Some(Reading { text: String::new(), value: None, pct_x: px, pct_score: ps, worst: 0.0 });
    }
    // Up to three digits, right-aligned against the `%`.
    let mut digits: Vec<(char, f32)> = Vec::new();
    for k in 1..=3 {
        let x = px as f32 - PITCH * k as f32;
        if x < 0.0 {
            break;
        }
        let fd = cell(x.round() as usize);
        let mut best = ('?', -2.0f32);
        for ((_, c), q) in t.g.iter() {
            if !c.is_ascii_digit() {
                continue;
            }
            let mut s = -2.0f32;
            for dx in -1..=1 {
                let d = Patch::cut(f, &fd, 0, dx, 0).dot(q);
                if d > s {
                    s = d;
                }
            }
            if s > best.1 {
                best = (*c, s);
            }
        }
        if best.1 < digit_min {
            break;
        }
        digits.push(best);
    }
    digits.reverse();
    let text: String = digits.iter().map(|d| d.0).collect();
    let worst = digits.iter().map(|d| d.1).fold(2.0f32, f32::min);
    // 0..100 only. A four-digit reading, or a value over 100, is a misread
    // rather than a surprising measurement.
    let value = text.parse::<f64>().ok().filter(|v| *v <= 100.0);
    Some(Reading { text, value, pct_x: px, pct_score: ps, worst })
}
