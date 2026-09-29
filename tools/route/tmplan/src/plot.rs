//! `tmplan leg-plot`: a headless picture of one leg — top-down surfaces by physics, walls, the human line by speed,
//! the search's chains by death cause, the gates; and a side elevation along the human's arc length (surface under the
//! line, the human's height, the chains' heights and speeds). Hand-rolled raster + PNG (zlib via miniz_oxide) — no
//! plotting crate in the workspace. Coordinator's ask 2026-09-09 15:56Z.

use std::collections::BTreeMap;

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 3]>,
}

impl Canvas {
    pub fn new(w: usize, h: usize, bg: [u8; 3]) -> Self {
        Canvas { w, h, px: vec![bg; w * h] }
    }
    pub fn set(&mut self, x: i64, y: i64, c: [u8; 3]) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }
    pub fn blend(&mut self, x: i64, y: i64, c: [u8; 3], a: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let p = &mut self.px[y as usize * self.w + x as usize];
            for k in 0..3 { p[k] = (p[k] as f32 * (1.0 - a) + c[k] as f32 * a).round() as u8; }
        }
    }
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: [u8; 3], width: i64) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let n = dx.abs().max(dy.abs()).ceil().max(1.0) as usize;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let (x, y) = ((x0 + dx * t).round() as i64, (y0 + dy * t).round() as i64);
            for ox in -(width / 2)..=(width / 2) { for oy in -(width / 2)..=(width / 2) { self.set(x + ox, y + oy, c); } }
        }
    }
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: [u8; 3]) {
        let (x0, y0) = (cx.round() as i64, cy.round() as i64);
        let ri = r.ceil() as i64;
        for dx in -ri..=ri { for dy in -ri..=ri { if ((dx * dx + dy * dy) as f32) <= r * r { self.set(x0 + dx, y0 + dy, c); } } }
    }
    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, c: [u8; 3]) {
        let n = (r * 8.0).max(12.0) as usize;
        for i in 0..n {
            let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
            self.line(cx + r * a0.cos(), cy + r * a0.sin(), cx + r * a1.cos(), cy + r * a1.sin(), c, 2);
        }
    }
    pub fn rect(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, c: [u8; 3]) {
        for y in y0..=y1 { for x in x0..=x1 { self.set(x, y, c); } }
    }
    /// 5x7 bitmap text (digits, upper-case letters, a few symbols), scale 1 = 6 px advance
    pub fn text(&mut self, x: i64, y: i64, s: &str, c: [u8; 3], scale: i64) {
        let mut cx = x;
        for ch in s.chars() {
            if let Some(g) = glyph(ch.to_ascii_uppercase()) {
                for (row, bits) in g.iter().enumerate() {
                    for col in 0..5 {
                        if bits & (1 << (4 - col)) != 0 {
                            for sx in 0..scale { for sy in 0..scale { self.set(cx + col as i64 * scale + sx, y + row as i64 * scale + sy, c); } }
                        }
                    }
                }
            }
            cx += 6 * scale;
        }
    }
    pub fn write_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        // filter byte 0 per row, zlib (miniz), chunks with crc32
        let mut raw = Vec::with_capacity((self.w * 3 + 1) * self.h);
        for y in 0..self.h {
            raw.push(0u8);
            for x in 0..self.w { raw.extend_from_slice(&self.px[y * self.w + x]); }
        }
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        let mut out = Vec::new();
        out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(self.w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(self.h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        std::fs::write(path, out)
    }
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut crc_in = Vec::with_capacity(4 + data.len());
    crc_in.extend_from_slice(kind);
    crc_in.extend_from_slice(data);
    out.extend_from_slice(&crc_in);
    out.extend_from_slice(&crc32(&crc_in).to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 { c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 }; }
        table[i as usize] = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data { crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8); }
    crc ^ 0xFFFF_FFFF
}

fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E], '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F], '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02], '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E], '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E], '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11], 'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E], 'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F], 'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F], 'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E], 'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11], 'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11], 'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E], 'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D], 'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E], 'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E], 'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A], 'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04], 'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        ' ' => [0; 7], '.' => [0, 0, 0, 0, 0, 0x0C, 0x0C], ',' => [0, 0, 0, 0, 0x0C, 0x04, 0x08],
        ':' => [0, 0x0C, 0x0C, 0, 0x0C, 0x0C, 0], '-' => [0, 0, 0, 0x1F, 0, 0, 0], '+' => [0, 0x04, 0x04, 0x1F, 0x04, 0x04, 0],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10], '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08], '=' => [0, 0, 0x1F, 0, 0x1F, 0, 0], '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03], '_' => [0, 0, 0, 0, 0, 0, 0x1F], '#' => [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A],
        _ => return None,
    })
}

/// physics-name → colour (drivable surfaces in warm/neutral tones, terrain green, water blue, non-drivable grey)
pub fn material_colour(name: &str) -> [u8; 3] {
    match name {
        "Asphalt" | "RoadSynthetic" | "Concrete" | "TechMagnetic" => [200, 200, 205],
        "Dirt" | "RoadDirt" | "Sand" => [196, 160, 100],
        "Grass" | "Green" => [120, 170, 90],
        "RoadIce" | "Ice" => [180, 225, 255],
        "Metal" | "ResonantMetal" | "MetalTrans" => [150, 150, 170],
        "Wood" => [170, 120, 70],
        "Rubber" => [90, 90, 90],
        "Plastic" => [255, 150, 200],
        "Rock" => [130, 120, 110],
        "Water" | "Sea" | "Lake" | "WaterSurface" => [80, 140, 220],
        "NotCollidable" => [235, 235, 235],
        _ => [175, 175, 175],
    }
}

/// speed 0..80 m/s → blue → green → yellow → red
pub fn speed_colour(v: f32) -> [u8; 3] {
    let t = (v / 80.0).clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.5 { let u = t * 2.0; (0.0, u, 1.0 - u) } else { let u = (t - 0.5) * 2.0; (u, 1.0 - u, 0.0) };
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}

pub fn cause_colour(cause: &str) -> [u8; 3] {
    match cause {
        "fell" => [220, 40, 40],
        "offroute" => [240, 140, 20],
        "stopped" | "crawl" => [150, 40, 160],
        "finish" => [20, 160, 60],
        "alive" | "" => [40, 90, 220],
        _ => [0, 0, 0],
    }
}

pub struct ChainRow {
    pub t: f32,
    pub p: [f32; 3],
    pub v: f32,
}

pub struct Chain {
    pub id: String,
    pub rows: Vec<ChainRow>,
    pub cause: String,
}

/// `chain  t  x  y  z  v  cause` (tab or whitespace separated, header optional; cause on every row or on the last)
pub fn read_chains(path: &str) -> Result<Vec<Chain>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut map: BTreeMap<String, Chain> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    // header-aware: columns named chain, t, x, y, z, v|speed, cause|end|status (any order, extra columns ignored); no header = positional
    let mut col: Vec<usize> = vec![0, 1, 2, 3, 4, 5, 6];
    for line in txt.lines() {
        let f: Vec<&str> = line.split(|c| c == '\t' || c == ' ' || c == ',').filter(|s| !s.is_empty()).collect();
        if f.len() >= 6 && f[1].parse::<f32>().is_err() {
            let find = |names: &[&str]| names.iter().find_map(|n| f.iter().position(|h| h.eq_ignore_ascii_case(n)));
            if let (Some(c), Some(t), Some(x), Some(y), Some(z), Some(v)) = (find(&["chain", "id"]), find(&["t", "time", "t_s"]), find(&["x"]), find(&["y"]), find(&["z"]), find(&["v", "speed", "v_mps"])) { col = vec![c, t, x, y, z, v, find(&["cause", "end", "status", "end_cause"]).unwrap_or(usize::MAX)]; }
            continue;
        }
        if f.len() < 6 || f.get(col[1]).and_then(|s| s.parse::<f32>().ok()).is_none() { continue; }
        let g = |i: usize| f.get(col[i]).copied().unwrap_or("");
        let id = g(0).to_string();
        let row = ChainRow { t: g(1).parse().unwrap_or(0.0), p: [g(2).parse().unwrap_or(0.0), g(3).parse().unwrap_or(0.0), g(4).parse().unwrap_or(0.0)], v: g(5).parse().unwrap_or(0.0) };
        let cause = if col[6] == usize::MAX { String::new() } else { g(6).trim().to_lowercase() };
        let e = map.entry(id.clone()).or_insert_with(|| { order.push(id.clone()); Chain { id: id.clone(), rows: Vec::new(), cause: String::new() } });
        e.rows.push(row);
        if !cause.is_empty() { e.cause = cause; }
    }
    Ok(order.into_iter().filter_map(|k| map.remove(&k)).collect())
}
