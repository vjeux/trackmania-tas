//! The five-lamp key overlay the TAS tool draws in the bottom-left corner:
//! BRAKE, and the four arrows. Lit is a light-grey fill, unlit a dark-grey one,
//! and both carry the same white glyph, so the reading is a median.

use crate::frame::{Frame, Rect};

/// The lamps, in the order every table in this crate prints them.
pub const NAMES: [&str; 5] = ["brake", "up", "down", "left", "right"];

/// Lamp interiors, measured off the 2560x1440 master at 08:39.
/// The boxes are drawn at fixed screen positions; nothing here drifts.
pub const BOXES: [Rect; 5] = [
    Rect::new(8, 1320, 247, 105),  // BRAKE
    Rect::new(425, 1192, 110, 113), // up
    Rect::new(425, 1320, 110, 105), // down
    Rect::new(300, 1320, 105, 105), // left
    Rect::new(550, 1320, 105, 105), // right
];

pub struct Reading {
    /// Median luma inside each lamp.
    pub fill: [f32; 5],
    /// Median luma of the light-grey frame drawn around each lamp.
    pub border: [f32; 5],
    /// How far the brightest pixels inside a lamp rise above its fill. Every
    /// lamp carries a white glyph -- an arrow, or the word BRAKE -- so this is
    /// large lit or unlit, and it is ZERO on the flat bright patch of scenery
    /// that otherwise passes every ratio test at once.
    pub glyph: [f32; 5],
}

impl Reading {
    pub fn of(f: &Frame) -> Reading {
        let mut fill = [0f32; 5];
        let mut border = [0f32; 5];
        let mut glyph = [0f32; 5];
        for (i, b) in BOXES.iter().enumerate() {
            fill[i] = b.inset(12).median_luma(f);
            border[i] = b.median_border_luma(f, 3);
            glyph[i] = b.inset(12).pct_luma(f, 97) - fill[i];
        }
        Reading { fill, border, glyph }
    }

    /// The lamps are drawn inside a light frame that is the same colour lit or
    /// unlit, so the frame is the exposure reference: the editor grades these
    /// clips differently and the absolute grey levels move between them, but
    /// fill-over-border does not.
    pub fn ratios(&self) -> [f32; 5] {
        let mut r = [0f32; 5];
        for i in 0..5 {
            r[i] = self.fill[i] / self.border[i].max(1.0);
        }
        r
    }

    /// The overlay is on screen when all five lamps show a light frame and
    /// every fill sits clearly in one of the two states, with the gap between
    /// the states empty. A scene that fakes one lamp does not fake all five.
    pub fn present(&self, border_min: f32, lo: f32, hi: f32, glyph_min: f32) -> bool {
        let r = self.ratios();
        (0..5).all(|i| {
            self.border[i] >= border_min
                && (r[i] <= lo || r[i] >= hi)
                && self.glyph[i] >= glyph_min
        })
    }

    pub fn bits(&self, lo: f32, hi: f32) -> [bool; 5] {
        let r = self.ratios();
        let mid = 0.5 * (lo + hi);
        let mut b = [false; 5];
        for i in 0..5 {
            b[i] = r[i] >= mid;
        }
        b
    }
}
