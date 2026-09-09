//! `tinyctl cropstats IMG… --crop x,y,w,h [--cells N] [--sheet OUT.png]` —
//! numbers instead of a look, for a lineup row shot several times from one
//! camera (the flag experiments of 2026-09-08).
//!
//! The crop (pixels of the full capture) is split into N side-by-side cells,
//! one per item of the row. Per image and cell it prints, as TSV:
//!
//! * `fg`   — the share of pixels that are not background, where the
//!            background colour is the mean of the crop's top row (sky, when
//!            the camera is aimed so the row stands against it);
//! * `dark` — the share of pixels darker than 48 on every channel (a black
//!            sail, a garbage draw);
//! * `sat` / `white` — the share of pixels with a channel at or over 250, and
//!            with every channel there (a blown highlight; the lights lineup
//!            of 2026-09-09);
//! * `fgrgb` — the mean colour of the foreground pixels (the cloth: green /
//!            blue / white / black tells the hue mask apart from the band);
//! * `diff` — the mean absolute difference against the previous image's same
//!            cell, all pixels (a waving cloth moves; a still one gives ~0, a
//!            vanished or exploded one a jump).
//!
//! `--sheet OUT.png` stacks every image's crop (one row per image, cell
//! borders drawn) so ONE look covers the whole series.

use crate::png::{self, Image};
use std::path::{Path, PathBuf};

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn ints(s: &str, n: usize, what: &str) -> Result<Vec<i64>, String> {
    let v: Vec<i64> = s.split(',').map(|x| x.trim().parse::<i64>().map_err(|e| format!("{what}: {x:?}: {e}"))).collect::<Result<_, _>>()?;
    if v.len() != n {
        return Err(format!("{what}: wants {n} numbers, got {}", v.len()));
    }
    Ok(v)
}

struct CellStat {
    fg: f32,
    dark: f32,
    /// the share of pixels with a channel at or over 250 (a blown highlight)
    sat: f32,
    /// the share of pixels white on every channel (>= 250): the "completely white" of 2026-09-09
    white: f32,
    /// the mean colour of the brightest `--top` percent of the cell (by luma):
    /// a light's footprint against an ambient that dominates the mean
    top_rgb: [u8; 3],
    fg_rgb: [u8; 3],
    mean_rgb: [u8; 3],
}

fn cell_stat(img: &Image, x0: usize, y0: usize, w: usize, h: usize, bg: [f32; 3], top_pct: f32) -> CellStat {
    let mut lumas: Vec<(u32, [u8; 3])> = Vec::new();
    let mut n = 0usize;
    let mut nfg = 0usize;
    let mut ndark = 0usize;
    let mut nsat = 0usize;
    let mut nwhite = 0usize;
    let mut acc = [0u64; 3];
    let mut accfg = [0u64; 3];
    for y in y0..(y0 + h).min(img.h) {
        for x in x0..(x0 + w).min(img.w) {
            let c = img.get(x, y);
            n += 1;
            lumas.push((c[0] as u32 * 299 + c[1] as u32 * 587 + c[2] as u32 * 114, c));
            for k in 0..3 {
                acc[k] += c[k] as u64;
            }
            let d = (c[0] as f32 - bg[0]).abs() + (c[1] as f32 - bg[1]).abs() + (c[2] as f32 - bg[2]).abs();
            if d > 60.0 {
                nfg += 1;
                for k in 0..3 {
                    accfg[k] += c[k] as u64;
                }
            }
            if c[0] < 48 && c[1] < 48 && c[2] < 48 {
                ndark += 1;
            }
            if c[0] >= 250 || c[1] >= 250 || c[2] >= 250 {
                nsat += 1;
            }
            if c[0] >= 250 && c[1] >= 250 && c[2] >= 250 {
                nwhite += 1;
            }
        }
    }
    let n = n.max(1);
    lumas.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    let k = ((lumas.len() as f32 * top_pct / 100.0).round() as usize).clamp(1, lumas.len().max(1));
    let mut acct = [0u64; 3];
    for (_, c) in lumas.iter().take(k) {
        for j in 0..3 {
            acct[j] += c[j] as u64;
        }
    }
    let m = |a: [u64; 3], d: usize| -> [u8; 3] { let d = d.max(1) as u64; [(a[0] / d) as u8, (a[1] / d) as u8, (a[2] / d) as u8] };
    CellStat { fg: nfg as f32 / n as f32, dark: ndark as f32 / n as f32, sat: nsat as f32 / n as f32, white: nwhite as f32 / n as f32, top_rgb: m(acct, k), fg_rgb: m(accfg, nfg), mean_rgb: m(acc, n) }
}

fn cell_diff(a: &Image, b: &Image, x0: usize, y0: usize, w: usize, h: usize) -> f32 {
    let mut acc = 0u64;
    let mut n = 0u64;
    for y in y0..(y0 + h).min(a.h).min(b.h) {
        for x in x0..(x0 + w).min(a.w).min(b.w) {
            let p = a.get(x, y);
            let q = b.get(x, y);
            acc += (p[0] as i32 - q[0] as i32).unsigned_abs() as u64 + (p[1] as i32 - q[1] as i32).unsigned_abs() as u64 + (p[2] as i32 - q[2] as i32).unsigned_abs() as u64;
            n += 3;
        }
    }
    if n == 0 { 0.0 } else { acc as f32 / n as f32 }
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let crop = ints(flag(args, "--crop").ok_or("cropstats needs --crop x,y,w,h")?, 4, "--crop")?;
    let cells: usize = flag(args, "--cells").map(|s| s.parse().map_err(|e| format!("--cells: {e}"))).transpose()?.unwrap_or(1).max(1);
    let sheet: Option<PathBuf> = flag(args, "--sheet").map(PathBuf::from);
    // --top P: the brightest P percent of each cell, as a mean colour (default 5)
    let top_pct: f32 = flag(args, "--top").map(|s| s.parse().map_err(|e| format!("--top: {e}"))).transpose()?.unwrap_or(5.0);
    let mut files: Vec<PathBuf> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--crop" | "--cells" | "--sheet" | "--top" => i += 2,
            a if a.starts_with("--") => return Err(format!("unknown flag {a}")),
            a => {
                files.push(PathBuf::from(a));
                i += 1;
            }
        }
    }
    if files.is_empty() {
        return Err("cropstats needs at least one image".into());
    }
    let (cx, cy, cw, ch) = (crop[0].max(0) as usize, crop[1].max(0) as usize, crop[2].max(1) as usize, crop[3].max(1) as usize);
    let cell_w = (cw / cells).max(1);
    println!("image\tcell\tfg%\tdark%\tsat%\twhite%\tfg_rgb\tmean_rgb\ttop_rgb\tdiff_prev");
    let mut prev: Option<Image> = None;
    let mut rows: Vec<Image> = Vec::new();
    for f in &files {
        let img = read(f)?;
        if cx + cw > img.w || cy + ch > img.h {
            return Err(format!("{}: crop {cx},{cy},{cw},{ch} leaves the {}x{} image", f.display(), img.w, img.h));
        }
        // the background: the crop's top row
        let mut bg = [0f32; 3];
        for x in cx..cx + cw {
            let c = img.get(x, cy);
            for k in 0..3 {
                bg[k] += c[k] as f32;
            }
        }
        for k in 0..3 {
            bg[k] /= cw as f32;
        }
        let name = f.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        for c in 0..cells {
            let x0 = cx + c * cell_w;
            let st = cell_stat(&img, x0, cy, cell_w, ch, bg, top_pct);
            let d = prev.as_ref().map(|p| cell_diff(p, &img, x0, cy, cell_w, ch));
            println!(
                "{name}\t{c}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:02x}{:02x}{:02x}\t{:02x}{:02x}{:02x}\t{:02x}{:02x}{:02x}\t{}",
                st.fg * 100.0,
                st.dark * 100.0,
                st.sat * 100.0,
                st.white * 100.0,
                st.fg_rgb[0], st.fg_rgb[1], st.fg_rgb[2],
                st.mean_rgb[0], st.mean_rgb[1], st.mean_rgb[2],
                st.top_rgb[0], st.top_rgb[1], st.top_rgb[2],
                d.map(|d| format!("{d:.2}")).unwrap_or_else(|| "-".into())
            );
        }
        if sheet.is_some() {
            let mut row = img.crop_scaled(cx as i64, cy as i64, (cx + cw) as i64, (cy + ch) as i64, cw);
            for c in 1..cells {
                row.fill(c * cell_w, 0, 1, ch, [255, 0, 255]);
            }
            png::label(&mut row, 2, 2, &name, 1, [255, 255, 0]);
            rows.push(row);
        }
        prev = Some(img);
    }
    if let Some(out) = sheet {
        let h: usize = rows.iter().map(|r| r.h + 2).sum();
        let mut s = Image::new(cw, h);
        let mut y = 0;
        for r in &rows {
            s.blit(r, 0, y);
            y += r.h + 2;
        }
        std::fs::write(&out, png::encode(&s)).map_err(|e| format!("{}: {e}", out.display()))?;
        println!("sheet {} ({}x{})", out.display(), s.w, s.h);
    }
    Ok(())
}

fn read(p: &Path) -> Result<Image, String> {
    let d = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    png::decode(&d).map_err(|e| format!("{}: {e}", p.display()))
}
