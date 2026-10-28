//! `mapgeom item-icon ITEM.Item.Gbx --bgra CROP.bgra --size 256 [--out OUT] [--margin 0.06]
//! [--no-flip] [--opaque]` — the item's inventory icon (header chunk 0x2E001004, 64×64 raw
//! BGRA) from a top-down box capture: `CROP.bgra` is a `size`×`size` 32-bit BGRA dump of the
//! capture's centre (tinyctl icon-crops), the item sits on grass, so the picture's corners
//! give the background colour; the foreground's bounding box, squared with a margin, is
//! box-filtered down to 64×64 and the background pixels go transparent (`--opaque` keeps
//! them). `--dir ITEMS --crops CROPS` does every `<stem>.Item.Gbx` with a `<stem>.bgra`.
//!
//! Written for Everios96's water-shape blockset (2026-10-28): 61 shapes × 7 surfaces are
//! unusable in the item browser without icons.
use std::path::{Path, PathBuf};

type R<T> = Result<T, String>;

fn flag<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

/// 64×64 BGRA rows (top-down) from a size×size BGRA crop.
pub fn icon_pixels(crop: &[u8], size: usize, margin: f32, opaque: bool) -> R<Vec<u8>> {
    if crop.len() != size * size * 4 {
        return Err(format!("crop is {} bytes, expected {}×{}×4 = {}", crop.len(), size, size, size * size * 4));
    }
    let px = |x: usize, y: usize| -> [i32; 3] {
        let o = (y * size + x) * 4;
        [crop[o] as i32, crop[o + 1] as i32, crop[o + 2] as i32]
    };
    // the background = the stadium grass under the item: four corner patches (12×12), kept
    // SEPARATELY (a big curve's crop straddles the stadium's shadow edge, so one mean sat
    // between sunlit and shaded grass and matched neither — 2026-10-28, Curve4/5 icons
    // came out with green patches). A pixel is background when it is GREEN-DOMINANT and
    // near the closest corner patch; dirt, wood, ice, plastic, tarmac, metal rims and the
    // walls' shadows are not green-dominant.
    let mut corners: Vec<[i32; 3]> = Vec::new();
    for &(cx, cy) in &[(0usize, 0usize), (size - 12, 0), (0, size - 12), (size - 12, size - 12)] {
        let mut acc = [0i64; 3];
        let mut n = 0i64;
        for y in cy..cy + 12 {
            for x in cx..cx + 12 {
                let p = px(x, y);
                acc[0] += p[0] as i64;
                acc[1] += p[1] as i64;
                acc[2] += p[2] as i64;
                n += 1;
            }
        }
        corners.push([(acc[0] / n) as i32, (acc[1] / n) as i32, (acc[2] / n) as i32]);
    }
    let is_fg = |p: [i32; 3]| -> bool {
        // BGRA: p[0] blue, p[1] green, p[2] red
        let green_dom = p[1] - p[0].max(p[2]);
        let near = corners.iter().map(|c| (p[0] - c[0]).abs() + (p[1] - c[1]).abs() + (p[2] - c[2]).abs()).min().unwrap_or(999);
        !(green_dom > 8 && near < 110)
    };
    // the box is searched in the crop's central 84 % — a neighbouring item's sliver at
    // the very corner must not stretch it
    let lo = size * 8 / 100;
    let hi = size - lo;
    let (mut x0, mut y0, mut x1, mut y1) = (size, size, 0usize, 0usize);
    let mut fg = vec![false; size * size];
    for y in 0..size {
        for x in 0..size {
            if is_fg(px(x, y)) {
                fg[y * size + x] = true;
                if x >= lo && x < hi && y >= lo && y < hi {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
    }
    if x1 < x0 {
        return Err("no foreground found (the crop is all background)".into());
    }
    // speckle guard: shrink the box to rows/columns with at least 2 % foreground
    let min_count = (size / 50).max(2);
    let row_ok = |y: usize| (0..size).filter(|&x| fg[y * size + x]).count() >= min_count;
    let col_ok = |x: usize| (0..size).filter(|&y| fg[y * size + x]).count() >= min_count;
    while y0 < y1 && !row_ok(y0) { y0 += 1; }
    while y1 > y0 && !row_ok(y1) { y1 -= 1; }
    while x0 < x1 && !col_ok(x0) { x0 += 1; }
    while x1 > x0 && !col_ok(x1) { x1 -= 1; }
    // square the box around its centre, add the margin, clamp
    let w = (x1 - x0 + 1) as f32;
    let h = (y1 - y0 + 1) as f32;
    let side = (w.max(h) * (1.0 + 2.0 * margin)).min(size as f32);
    let cx = (x0 + x1) as f32 / 2.0;
    let cy = (y0 + y1) as f32 / 2.0;
    let sx0 = (cx - side / 2.0).max(0.0).min(size as f32 - side);
    let sy0 = (cy - side / 2.0).max(0.0).min(size as f32 - side);
    // box filter to 64×64 (BGRA top-down); background pixels transparent
    let mut out = vec![0u8; 64 * 64 * 4];
    let cell = side / 64.0;
    for oy in 0..64 {
        for ox in 0..64 {
            let (fx0, fy0) = (sx0 + ox as f32 * cell, sy0 + oy as f32 * cell);
            let (fx1, fy1) = (fx0 + cell, fy0 + cell);
            let (ix0, iy0) = (fx0.floor() as usize, fy0.floor() as usize);
            let (ix1, iy1) = ((fx1.ceil() as usize).min(size), (fy1.ceil() as usize).min(size));
            let mut acc = [0f32; 4];
            let mut cnt = 0f32;
            for y in iy0..iy1.max(iy0 + 1) {
                for x in ix0..ix1.max(ix0 + 1) {
                    let (x, y) = (x.min(size - 1), y.min(size - 1));
                    let o = (y * size + x) * 4;
                    let a = if opaque || fg[y * size + x] { 1.0 } else { 0.0 };
                    acc[0] += crop[o] as f32 * a;
                    acc[1] += crop[o + 1] as f32 * a;
                    acc[2] += crop[o + 2] as f32 * a;
                    acc[3] += a;
                    cnt += 1.0;
                }
            }
            let oo = (oy * 64 + ox) * 4;
            if acc[3] > 0.0 {
                out[oo] = (acc[0] / acc[3]).round() as u8;
                out[oo + 1] = (acc[1] / acc[3]).round() as u8;
                out[oo + 2] = (acc[2] / acc[3]).round() as u8;
                out[oo + 3] = (255.0 * acc[3] / cnt).round() as u8;
            }
        }
    }
    Ok(out)
}

fn one(item: &Path, crop_path: &Path, out: &Path, size: usize, margin: f32, flip: bool, opaque: bool) -> R<()> {
    let bytes = std::fs::read(item).map_err(|e| format!("{}: {e}", item.display()))?;
    let crop = std::fs::read(crop_path).map_err(|e| format!("{}: {e}", crop_path.display()))?;
    let px = icon_pixels(&crop, size, margin, opaque)?;
    let out_bytes = tmmaps::header::set_icon_chunk(&bytes, 64, 64, &px, flip);
    std::fs::write(out, &out_bytes).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("{}: icon 64×64 from {} ({} bytes → {} bytes)", out.display(), crop_path.display(), bytes.len(), out_bytes.len());
    Ok(())
}

pub fn run(args: &[String]) -> R<()> {
    let size: usize = flag(args, "--size").and_then(|v| v.parse().ok()).unwrap_or(256);
    let margin: f32 = flag(args, "--margin").and_then(|v| v.parse().ok()).unwrap_or(0.06);
    let flip = !args.iter().any(|a| a == "--no-flip");
    let opaque = args.iter().any(|a| a == "--opaque");
    if let (Some(dir), Some(crops)) = (flag(args, "--dir"), flag(args, "--crops")) {
        let mut n = 0usize;
        let mut missing = Vec::new();
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).map_err(|e| format!("{dir}: {e}"))?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for p in entries {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let Some(stem) = name.strip_suffix(".Item.Gbx") else { continue };
            let crop = Path::new(crops).join(format!("{stem}.bgra"));
            if !crop.exists() {
                missing.push(stem.to_string());
                continue;
            }
            one(&p, &crop, &p, size, margin, flip, opaque)?;
            n += 1;
        }
        println!("{n} items iconed; {} without a crop{}", missing.len(), if missing.is_empty() { String::new() } else { format!(": {}", missing.join(" ")) });
        return Ok(());
    }
    let item = args.get(1).ok_or("item-icon ITEM.Item.Gbx --bgra CROP.bgra [--out OUT] | --dir ITEMS --crops CROPS")?;
    let crop = flag(args, "--bgra").ok_or("--bgra CROP.bgra")?;
    let out = flag(args, "--out").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(item));
    one(Path::new(item), Path::new(crop), &out, size, margin, flip, opaque)
}
