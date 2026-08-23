//! Where the glyphs of a fixed-position readout actually sit.
//!
//! Per-glyph bounding boxes measured frame by frame track the ink *threshold*,
//! not the glyph: an antialiased edge column crosses the cutoff and the same
//! digit is one pixel wider on the next frame. Cells have to be fixed once,
//! from the ink summed over every frame of a section, which is what this
//! prints.

use crate::frame::{Frame, Rect};
use std::io::Write;

pub struct Profile {
    pub rect: Rect,
    pub col: Vec<u64>,
    pub row: Vec<u64>,
    pub frames: u64,
}

impl Profile {
    pub fn new(rect: Rect) -> Self {
        Profile { rect, col: vec![0; rect.w], row: vec![0; rect.h], frames: 0 }
    }

    /// Accumulate one frame. Ink is `min(r,g,b) >= thresh`: these readouts are
    /// white, and white over a bright cyan pool is only separable in the
    /// minimum channel, never in luma.
    pub fn add(&mut self, f: &Frame, thresh: f32) {
        for dy in 0..self.rect.h {
            for dx in 0..self.rect.w {
                if f.minc(self.rect.x + dx, self.rect.y + dy) >= thresh {
                    self.col[dx] += 1;
                    self.row[dy] += 1;
                }
            }
        }
        self.frames += 1;
    }

    pub fn print(&self, o: &mut impl Write) {
        writeln!(o, "# {} frames, rect {:?}", self.frames, self.rect).unwrap();
        writeln!(o, "axis\tindex\tabs\tcount").unwrap();
        for (i, c) in self.col.iter().enumerate() {
            writeln!(o, "col\t{}\t{}\t{}", i, self.rect.x + i, c).unwrap();
        }
        for (i, c) in self.row.iter().enumerate() {
            writeln!(o, "row\t{}\t{}\t{}", i, self.rect.y + i, c).unwrap();
        }
    }
}
