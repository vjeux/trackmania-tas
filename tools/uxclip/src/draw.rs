//! A canvas and a 5x7 bitmap font, so a contact sheet can carry its own
//! timestamps and labels without `drawtext` -- the static ffmpeg on the render
//! box was built without libfreetype, and a sheet whose frames do not say
//! WHEN they are is a sheet nobody can cite.

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>, // rgb24
}

impl Canvas {
    pub fn new(w: usize, h: usize, bg: [u8; 3]) -> Canvas {
        let mut px = vec![0u8; w * h * 3];
        for p in px.chunks_exact_mut(3) {
            p.copy_from_slice(&bg);
        }
        Canvas { w, h, px }
    }

    pub fn fill_rect(&mut self, x: i64, y: i64, w: usize, h: usize, c: [u8; 3]) {
        for yy in y.max(0)..(y + h as i64).min(self.h as i64) {
            for xx in x.max(0)..(x + w as i64).min(self.w as i64) {
                let i = (yy as usize * self.w + xx as usize) * 3;
                self.px[i..i + 3].copy_from_slice(&c);
            }
        }
    }

    /// Copy an rgb24 frame of `fw`x`fh` at (x, y), clipped.
    pub fn blit(&mut self, frame: &[u8], fw: usize, fh: usize, x: i64, y: i64) {
        for row in 0..fh {
            let yy = y + row as i64;
            if yy < 0 || yy >= self.h as i64 {
                continue;
            }
            let src = &frame[row * fw * 3..(row + 1) * fw * 3];
            let x0 = x.max(0);
            let x1 = (x + fw as i64).min(self.w as i64);
            if x1 <= x0 {
                continue;
            }
            let sx0 = (x0 - x) as usize;
            let n = (x1 - x0) as usize;
            let di = (yy as usize * self.w + x0 as usize) * 3;
            self.px[di..di + n * 3].copy_from_slice(&src[sx0 * 3..(sx0 + n) * 3]);
        }
    }

    /// Text in the 5x7 font at integer `scale`, on a background box.
    pub fn text(&mut self, x: i64, y: i64, s: &str, scale: usize, fg: [u8; 3], bg: Option<[u8; 3]>) {
        let s = s.to_uppercase();
        let adv = 6 * scale;
        if let Some(bg) = bg {
            self.fill_rect(x - scale as i64, y - scale as i64, s.chars().count() * adv + scale, 7 * scale + 2 * scale, bg);
        }
        let mut cx = x;
        for ch in s.chars() {
            let g = glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0b10000 >> col) != 0 {
                        self.fill_rect(cx + (col * scale) as i64, y + (row * scale) as i64, scale, scale, fg);
                    }
                }
            }
            cx += adv as i64;
        }
    }
}

/// 5 columns x 7 rows, MSB = leftmost column.
fn glyph(c: char) -> [u8; 7] {
    match c {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        ':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        ',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '_' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        '=' => [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '%' => [0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011],
        '@' => [0b01110, 0b10001, 0b00001, 0b01101, 0b10101, 0b10101, 0b01110],
        '#' => [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        '\'' => [0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000],
        ' ' => [0; 7],
        _ => [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111],
    }
}

/// `m:ss.d` for a time in seconds, the way a stamp should read.
pub fn hms(t: f64) -> String {
    let t = t.max(0.0);
    let m = (t / 60.0).floor() as u64;
    let s = t - m as f64 * 60.0;
    format!("{m}:{s:04.1}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps() {
        assert_eq!(hms(0.0), "0:00.0");
        assert_eq!(hms(61.25), "1:01.2");
        assert_eq!(hms(125.0), "2:05.0");
    }

    #[test]
    fn draws_without_panicking_at_edges() {
        let mut c = Canvas::new(20, 10, [0, 0, 0]);
        c.text(-3, -2, "AB:9", 1, [255, 255, 255], Some([0, 0, 0]));
        c.text(15, 8, "ZZ", 2, [255, 255, 255], None);
        let f = vec![7u8; 4 * 3 * 3];
        c.blit(&f, 4, 3, 18, 9);
        c.blit(&f, 4, 3, -2, -1);
        assert_eq!(c.px.len(), 20 * 10 * 3);
    }
}
