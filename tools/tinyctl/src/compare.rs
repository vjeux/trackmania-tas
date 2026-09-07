//! `tinyctl compare` — where does the tiny shot differ from the original?
//!
//! Both sides of a view are the same scene from the same relative camera, so
//! a perfect copy differs only in texture density and vegetation. The frame is
//! cut into cells and each cell's mean colour and edge energy compared; a cell
//! that moved further than the thresholds is flagged and cropped out of BOTH
//! frames side by side into one contact sheet, labelled with its view and
//! cell. One look at the sheet replaces one look at every view — which is
//! what the session image budget (32 views) demands at 12 views × 2 sides
//! per map.
//!
//! ```text
//! tinyctl compare --views VIEWS.tsv --dir DIR --tag sNN [--out-prefix DIR/cmpdiff-sNN]
//!                 [--color 50] [--edge 16] [--keep-hud] [--max-crops 16] [--hstack-ffmpeg BIN]
//! tinyctl compare --pair ORIG.png TINY.png [--out-prefix P]
//! ```
//!
//! Pairs are `DIR/cmp-<tag><NAME>-o.png` / `-t.png`, the names `shootset` and
//! `cmpviews.sh` write. Output: `<prefix>-overview.png` (every view, both
//! sides, flagged cells outlined), `<prefix>-crops.png` (the worst cells,
//! original | tiny, labelled), `<prefix>.tsv` (one row per flagged cell), and
//! the summary on stdout. `--hstack-ffmpeg` also writes the plain side-by-side
//! `cmp-<tag><NAME>.jpg` the earlier scripts produced, through that ffmpeg.

use std::path::{Path, PathBuf};

use crate::png::{self, Image};

const ANALYSIS_W: usize = 640;
const CELL: usize = 40;

struct CellStat {
    mean: [f32; 3],
    edge: f32,
}

fn luma(c: [u8; 3]) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

fn cell_stats(img: &Image, cols: usize, rows: usize) -> Vec<CellStat> {
    let mut out = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            let (x0, y0) = (c * CELL, r * CELL);
            let mut acc = [0f32; 3];
            let mut edge = 0f32;
            let mut n = 0f32;
            for y in y0..(y0 + CELL).min(img.h) {
                for x in x0..(x0 + CELL).min(img.w) {
                    let p = img.get(x, y);
                    acc[0] += p[0] as f32;
                    acc[1] += p[1] as f32;
                    acc[2] += p[2] as f32;
                    let l = luma(p);
                    if x + 1 < img.w {
                        edge += (luma(img.get(x + 1, y)) - l).abs();
                    }
                    if y + 1 < img.h {
                        edge += (luma(img.get(x, y + 1)) - l).abs();
                    }
                    n += 1.0;
                }
            }
            let n = n.max(1.0);
            out.push(CellStat { mean: [acc[0] / n, acc[1] / n, acc[2] / n], edge: edge / n });
        }
    }
    out
}

pub struct Flag {
    pub view: String,
    pub row: usize,
    pub col: usize,
    pub color: f32,
    pub edge_o: f32,
    pub edge_t: f32,
    pub severity: f32,
}

pub struct PairResult {
    pub view: String,
    pub cols: usize,
    pub rows: usize,
    pub flags: Vec<Flag>,
    pub frame_color: f32,
    /// analysis-size copies, for the sheets
    pub small_o: Image,
    pub small_t: Image,
    pub factor_o: usize,
    pub factor_t: usize,
}

pub fn compare_pair(view: &str, orig: &Image, tiny: &Image, color_thr: f32, edge_thr: f32, keep_hud: bool) -> PairResult {
    let fo = (orig.w + ANALYSIS_W - 1) / ANALYSIS_W;
    let ft = (tiny.w + ANALYSIS_W - 1) / ANALYSIS_W;
    let so = orig.shrink(fo.max(1));
    let st = tiny.shrink(ft.max(1));
    let cols = so.w.min(st.w) / CELL;
    let rows = so.h.min(st.h) / CELL;
    let a = cell_stats(&so, cols, rows);
    let b = cell_stats(&st, cols, rows);
    let mut flags = Vec::new();
    let mut frame = [0f32; 3];
    for (i, (ca, cb)) in a.iter().zip(b.iter()).enumerate() {
        let row = i / cols;
        // the editor UI: the title bar in the top row, the block/item palette
        // in the bottom row — identical in kind, different in content
        if !keep_hud && (row == 0 || row + 1 == rows) {
            continue;
        }
        let d = [cb.mean[0] - ca.mean[0], cb.mean[1] - ca.mean[1], cb.mean[2] - ca.mean[2]];
        frame[0] += d[0];
        frame[1] += d[1];
        frame[2] += d[2];
        let color = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let edge = (cb.edge - ca.edge).abs();
        let severity = color / color_thr + edge / edge_thr;
        if color > color_thr || edge > edge_thr {
            flags.push(Flag { view: view.to_string(), row: i / cols, col: i % cols, color, edge_o: ca.edge, edge_t: cb.edge, severity });
        }
    }
    let n = (cols * rows).max(1) as f32;
    let frame_color = ((frame[0] / n).powi(2) + (frame[1] / n).powi(2) + (frame[2] / n).powi(2)).sqrt();
    PairResult { view: view.to_string(), cols, rows, flags, frame_color, small_o: so, small_t: st, factor_o: fo.max(1), factor_t: ft.max(1) }
}

const RED: [u8; 3] = [255, 40, 40];
const AMBER: [u8; 3] = [255, 190, 0];
const WHITE: [u8; 3] = [255, 255, 255];

/// Every view, both sides at thumbnail size, flagged cells outlined.
fn overview_sheet(results: &[PairResult]) -> Image {
    let thumb_w = 320usize;
    let per_row = 3usize;
    let gap = 6usize;
    let rows_needed = (results.len() + per_row - 1) / per_row;
    let thumb_h = |r: &PairResult| (r.small_o.h * thumb_w / 2 / r.small_o.w.max(1)).max(1);
    let row_h = results.iter().map(thumb_h).max().unwrap_or(90) + 14 + gap;
    let mut sheet = Image::new(per_row * (thumb_w + gap), rows_needed * row_h + gap);
    sheet.fill(0, 0, sheet.w, sheet.h, [24, 24, 24]);
    for (i, r) in results.iter().enumerate() {
        let (gx, gy) = ((i % per_row) * (thumb_w + gap) + gap / 2, (i / per_row) * row_h + gap / 2);
        let half = thumb_w / 2;
        let mut o = r.small_o.crop_scaled(0, 0, r.small_o.w as i64, r.small_o.h as i64, half);
        let mut t = r.small_t.crop_scaled(0, 0, r.small_t.w as i64, r.small_t.h as i64, half);
        let sx = half as f32 / r.small_o.w as f32;
        for f in &r.flags {
            let (cx, cy) = ((f.col * CELL) as f32 * sx, (f.row * CELL) as f32 * sx);
            let cw = (CELL as f32 * sx).ceil() as usize;
            let colr = if f.severity >= 2.0 { RED } else { AMBER };
            o.rect(cx as usize, cy as usize, cw, cw, 1, colr);
            t.rect(cx as usize, cy as usize, cw, cw, 1, colr);
        }
        sheet.blit(&o, gx, gy + 12);
        sheet.blit(&t, gx + half, gy + 12);
        let text = format!("{} {} FLAGS", r.view, r.flags.len());
        png::label(&mut sheet, gx, gy, &text, 1, WHITE);
    }
    sheet
}

/// The worst cells, original | tiny, two pairs per row, labelled.
fn crops_sheet(results: &[PairResult], originals: &[(Image, Image)], max_crops: usize) -> Image {
    let mut all: Vec<(usize, &Flag)> = Vec::new();
    for (i, r) in results.iter().enumerate() {
        for f in &r.flags {
            all.push((i, f));
        }
    }
    all.sort_by(|a, b| b.1.severity.partial_cmp(&a.1.severity).unwrap());
    all.truncate(max_crops);
    let crop_w = 236usize;
    let pair_w = crop_w * 2 + 8;
    let per_row = 2usize;
    let rows = (all.len() + per_row - 1) / per_row;
    let row_h = crop_w * 3 / 4 + 22;
    let mut sheet = Image::new(per_row * pair_w + 8, (rows * row_h + 8).max(40));
    sheet.fill(0, 0, sheet.w, sheet.h, [24, 24, 24]);
    if all.is_empty() {
        png::label(&mut sheet, 8, 8, "NO CELL OVER THRESHOLD", 2, WHITE);
        return sheet;
    }
    for (k, (ri, f)) in all.iter().enumerate() {
        let r = &results[*ri];
        let (orig, tiny) = &originals[*ri];
        let (gx, gy) = ((k % per_row) * pair_w + 4, (k / per_row) * row_h + 4);
        // the cell plus one cell of margin, in FULL-resolution coordinates
        let crop = |img: &Image, factor: usize| {
            let cs = (CELL * factor) as i64;
            let x0 = f.col as i64 * cs - cs;
            let y0 = f.row as i64 * cs - cs;
            img.crop_scaled(x0, y0, x0 + 3 * cs, y0 + 3 * cs, crop_w)
        };
        let mut o = crop(orig, r.factor_o);
        let mut t = crop(tiny, r.factor_t);
        let third = crop_w / 3;
        let colr = if f.severity >= 2.0 { RED } else { AMBER };
        o.rect(third, third, third, third, 1, colr);
        t.rect(third, third, third, third, 1, colr);
        sheet.blit(&o, gx, gy + 18);
        sheet.blit(&t, gx + crop_w + 8, gy + 18);
        let text = format!("{} R{}C{} COL{:.0} EDGE{:.0}/{:.0}", r.view, f.row, f.col, f.color, f.edge_o, f.edge_t);
        png::label(&mut sheet, gx, gy, &text, 2, WHITE);
    }
    sheet
}

fn read_png(p: &Path) -> Result<Image, String> {
    let d = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    png::decode(&d).map_err(|e| format!("{}: {e}", p.display()))
}

fn view_names(views: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(views).map_err(|e| format!("{}: {e}", views.display()))?;
    Ok(text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).filter_map(|l| l.split('\t').next()).map(|s| s.trim().to_string()).collect())
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let color_thr: f32 = tmmaps::cli::flag(args, "--color").unwrap_or("50").parse().map_err(|_| "--color number")?;
    let edge_thr: f32 = tmmaps::cli::flag(args, "--edge").unwrap_or("16").parse().map_err(|_| "--edge number")?;
    let max_crops: usize = tmmaps::cli::flag(args, "--max-crops").unwrap_or("16").parse().map_err(|_| "--max-crops number")?;
    let mut pairs: Vec<(String, PathBuf, PathBuf)> = Vec::new();
    let prefix: PathBuf;
    if let Some(o) = tmmaps::cli::flag(args, "--pair") {
        let i = args.iter().position(|a| a == "--pair").unwrap();
        let t = args.get(i + 2).ok_or("--pair ORIG.png TINY.png")?;
        pairs.push(("pair".into(), PathBuf::from(o), PathBuf::from(t)));
        prefix = PathBuf::from(tmmaps::cli::flag(args, "--out-prefix").unwrap_or("cmpdiff"));
    } else {
        let views = PathBuf::from(tmmaps::cli::flag(args, "--views").ok_or("compare needs --views VIEWS.tsv (or --pair O T)")?);
        let dir = PathBuf::from(tmmaps::cli::flag(args, "--dir").unwrap_or("."));
        let tag = tmmaps::cli::flag(args, "--tag").unwrap_or("");
        for name in view_names(&views)? {
            let o = dir.join(format!("cmp-{tag}{name}-o.png"));
            let t = dir.join(format!("cmp-{tag}{name}-t.png"));
            if o.exists() && t.exists() {
                pairs.push((name, o, t));
            } else {
                eprintln!("skip {name}: missing {} or {}", o.display(), t.display());
            }
        }
        prefix = PathBuf::from(tmmaps::cli::flag(args, "--out-prefix").map(String::from).unwrap_or_else(|| dir.join(format!("cmpdiff-{tag}")).to_string_lossy().into_owned()));
    }
    if pairs.is_empty() {
        return Err("no pairs to compare".into());
    }
    let ffmpeg = tmmaps::cli::flag(args, "--hstack-ffmpeg");
    let mut results = Vec::new();
    let mut originals = Vec::new();
    let mut tsv = String::from("view\trow\tcol\tcolor\tedge_o\tedge_t\tseverity\n");
    for (name, o, t) in &pairs {
        let (io, it) = (read_png(o)?, read_png(t)?);
        let r = compare_pair(name, &io, &it, color_thr, edge_thr, tmmaps::cli::has(args, "--keep-hud"));
        let total = (r.cols * r.rows).max(1);
        println!("{:<12} {:>3}/{:<3} cells flagged ({:>4.1}%)  frame colour shift {:.1}{}", name, r.flags.len(), total, 100.0 * r.flags.len() as f32 / total as f32, r.frame_color, if r.frame_color > color_thr { "  ⚠ WHOLE FRAME DIFFERS (camera? map not loaded?)" } else { "" });
        for f in &r.flags {
            tsv.push_str(&format!("{}\t{}\t{}\t{:.1}\t{:.1}\t{:.1}\t{:.2}\n", f.view, f.row, f.col, f.color, f.edge_o, f.edge_t, f.severity));
        }
        if let Some(ff) = ffmpeg {
            // a Windows ffmpeg (…/ffmpeg.exe on the render box) cannot read a WSL
            // path: hand it C:/… spellings
            let win = ff.ends_with(".exe");
            let arg = |p: &Path| -> String {
                let s = p.to_string_lossy().into_owned();
                if win { crate::wsx::to_win(&s) } else { s }
            };
            let out = o.with_file_name(format!("cmp-{}.jpg", o.file_name().unwrap().to_string_lossy().trim_end_matches("-o.png").trim_start_matches("cmp-")));
            let st = std::process::Command::new(ff)
                .args(["-nostdin", "-y", "-loglevel", "error", "-i"])
                .arg(arg(o))
                .arg("-i")
                .arg(arg(t))
                .args(["-filter_complex", "[0:v]scale=960:-1[a];[1:v]scale=960:-1[b];[a][b]hstack", "-q:v", "4"])
                .arg(arg(&out))
                .status();
            match st {
                Ok(s) if s.success() => println!("             {}", out.display()),
                Ok(s) => eprintln!("ffmpeg exit {s} for {}", out.display()),
                Err(e) => eprintln!("ffmpeg: {e}"),
            }
        }
        results.push(r);
        originals.push((io, it));
    }
    let overview = overview_sheet(&results);
    let crops = crops_sheet(&results, &originals, max_crops);
    let p_over = PathBuf::from(format!("{}-overview.png", prefix.display()));
    let p_crops = PathBuf::from(format!("{}-crops.png", prefix.display()));
    let p_tsv = PathBuf::from(format!("{}.tsv", prefix.display()));
    std::fs::write(&p_over, png::encode(&overview)).map_err(|e| format!("{}: {e}", p_over.display()))?;
    std::fs::write(&p_crops, png::encode(&crops)).map_err(|e| format!("{}: {e}", p_crops.display()))?;
    std::fs::write(&p_tsv, tsv).map_err(|e| format!("{}: {e}", p_tsv.display()))?;
    let total: usize = results.iter().map(|r| r.flags.len()).sum();
    println!("{total} flagged cells over {} views -> {} ({}x{}), {} ({}x{}), {}", results.len(), p_over.display(), overview.w, overview.h, p_crops.display(), crops.w, crops.h, p_tsv.display());
    Ok(())
}
