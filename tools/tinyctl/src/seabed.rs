//! `tinyctl seabed-profile --pak F:KEY.. --shelf OuterTerrain.tsv --out seabed-profile.png` — Hugo's side-view diagram
//! of the vista SEABED (2026-10-26): two rows, BlueBay and the three −6 vistas (RedIsland / WhiteShore / GreenCoast
//! share one bed; their sea levels differ by under a metre and are drawn as three lines). Per row a section from 8
//! cells inside the buildable grid to 20 cells outside it: the bed's [min, max] band per 4 m of distance over all
//! four sides and the corners (`mapgeom::outer_ring::bed_profile`, map frame, yoff 0), its crest (the max) and mean,
//! the sea surface, a vertical marker where the grid ends, the inside deep-water bed, the ring's v4.22 shelf ramps
//! (the `ramp` rows of OuterTerrain.tsv) and the slab band they carry. Horizontal 2 px / m, vertical exaggerated
//! `--exag` × (default 10; the picture says so). X axis in cells and metres from the grid edge, Y in metres.

use crate::png::{self, Image};

const BG: [u8; 3] = [250, 250, 250];
const GRID: [u8; 3] = [222, 222, 222];
const AXIS: [u8; 3] = [60, 60, 60];
const BAND: [u8; 3] = [205, 190, 160];
const CREST: [u8; 3] = [120, 80, 30];
const MEAN: [u8; 3] = [170, 130, 70];
const SEA: [u8; 3] = [60, 120, 220];
const INSIDE: [u8; 3] = [40, 150, 60];
const RING: [u8; 3] = [220, 40, 40];
const RINGBAND: [u8; 3] = [250, 190, 190];
const MARK: [u8; 3] = [0, 0, 0];
// label text colours (the font draws on a black box): light versions
const T_WHITE: [u8; 3] = [255, 255, 255];
const T_SEA: [u8; 3] = [140, 190, 255];
const T_INSIDE: [u8; 3] = [120, 240, 140];
const T_RING: [u8; 3] = [255, 130, 130];
const T_YELLOW: [u8; 3] = [255, 230, 90];

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

struct Row {
    title: String,
    /// (collection id, sea level) — the collections sharing this bed, each with its own sea line
    seas: Vec<(u32, f32)>,
    bands: Vec<(f32, f32, f32, f32)>, // d0, min, max, mean
    inside_bed: f32,
    /// the shelf ramps: (d0, d1, y at d0, y at d1)
    ramps: Vec<(f32, f32, f32, f32)>,
    lake_top: Vec<(u32, f32)>,
}

fn line(img: &mut Image, x0: i64, y0: i64, x1: i64, y1: i64, c: [u8; 3], t: i64) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        for ox in -(t / 2)..=(t / 2) {
            for oy in -(t / 2)..=(t / 2) {
                let (px, py) = (x + ox, y + oy);
                if px >= 0 && py >= 0 {
                    img.set(px as usize, py as usize, c);
                }
            }
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

fn dashed_h(img: &mut Image, x0: i64, x1: i64, y: i64, c: [u8; 3], t: i64) {
    let mut x = x0;
    while x < x1 {
        line(img, x, y, (x + 8).min(x1), y, c, t);
        x += 14;
    }
}

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let out = f(rest, "--out").unwrap_or_else(|| "seabed-profile.png".into());
    let shelf = f(rest, "--shelf").ok_or("seabed-profile needs --shelf OuterTerrain.tsv (the ramp rows)")?;
    let exag: f32 = f(rest, "--exag").and_then(|v| v.parse().ok()).unwrap_or(10.0);
    let inner: i32 = f(rest, "--inner").and_then(|v| v.parse().ok()).unwrap_or(8);
    let outer: i32 = f(rest, "--outer").and_then(|v| v.parse().ok()).unwrap_or(20);
    let mut store = mapgeom::store::DataStore::empty();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--pak" {
            if let Some((p, k)) = rest.get(i + 1).and_then(|s| s.rsplit_once(':')) {
                store.add_pak(p, k)?;
            }
            i += 1;
        }
        i += 1;
    }
    // the shelf ramps per collection (N side is enough: the four sides are symmetric), by distance from the edge
    let shelf_txt = std::fs::read_to_string(&shelf).map_err(|e| format!("{shelf}: {e}"))?;
    let ramps_of = |name: &str| -> Vec<(f32, f32, f32, f32)> {
        let mut v: Vec<(f32, f32, f32, f32)> = shelf_txt
            .lines()
            .skip(1)
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                if c.len() < 10 || c[0] != name || c[7] != "ramp" || c[9] != "N" {
                    return None;
                }
                // N side rows: z0..z1 negative, the edge at z = 0; distance = −z
                let (z0, z1): (f32, f32) = (c[4].parse().ok()?, c[5].parse().ok()?);
                let (floor, y_outer): (f32, f32) = (c[6].parse().ok()?, c[8].parse().ok()?);
                Some(((-z1).max(0.0), -z0, floor, y_outer))
            })
            .collect();
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        v
    };
    let mut rows: Vec<Row> = Vec::new();
    for (title, ids) in [("BLUEBAY", vec![0x1cu32]), ("REDISLAND / WHITESHORE / GREENCOAST (ONE BED)", vec![0x10u32, 0x1d, 0xf])] {
        let mut seas = Vec::new();
        let mut merged: std::collections::BTreeMap<i32, (f32, f32, f64, usize)> = Default::default();
        let mut lake_top = Vec::new();
        for &id in &ids {
            let p = mapgeom::outer_ring::bed_profile(&mut store, id, (64, 64), inner, outer)?;
            seas.push((id, p.sea_y));
            lake_top.push((id, mapgeom::floor_finish::water_surface(id)));
            for (d, mn, mx, mean, n, _) in p.bands {
                let e = merged.entry(d.round() as i32).or_insert((f32::INFINITY, f32::NEG_INFINITY, 0.0, 0));
                e.0 = e.0.min(mn);
                e.1 = e.1.max(mx);
                e.2 += mean as f64 * n as f64;
                e.3 += n;
            }
        }
        let bands = merged.into_iter().map(|(d, (mn, mx, s, n))| (d as f32, mn, mx, (s / n as f64) as f32)).collect();
        let inside_bed = mapgeom::outer_ring::open_water_bed(ids[0])?;
        let name = mapgeom::sttc::collection_name(ids[0]);
        rows.push(Row { title: title.into(), seas, bands, inside_bed, ramps: ramps_of(name), lake_top });
    }
    // layout
    let (px_m, left, right, top_pad, gap, bottom) = (2.0f32, 110usize, 40usize, 120usize, 120usize, 80usize);
    let d0 = -32.0 * inner as f32;
    let d1 = 32.0 * outer as f32;
    let plot_w = ((d1 - d0) * px_m) as usize;
    let mut heights = Vec::new();
    let mut ranges = Vec::new();
    for r in &rows {
        let mut lo = r.inside_bed.min(r.bands.iter().map(|b| b.1).fold(f32::INFINITY, f32::min));
        let mut hi = r.seas.iter().map(|s| s.1).fold(f32::NEG_INFINITY, f32::max).max(r.bands.iter().map(|b| b.2).fold(f32::NEG_INFINITY, f32::max));
        for rp in &r.ramps { lo = lo.min(rp.2.min(rp.3)); hi = hi.max(rp.2.max(rp.3)); }
        let (lo, hi) = ((lo - 1.5).floor(), (hi + 1.5).ceil());
        heights.push(((hi - lo) * exag) as usize);
        ranges.push((lo, hi));
    }
    let total_h = top_pad + heights.iter().sum::<usize>() + gap * (rows.len() - 1) + bottom;
    let mut img = Image::new(left + plot_w + right, total_h);
    img.fill(0, 0, img.w, img.h, BG);
    png::label(&mut img, 10, 8, &format!("VISTA SEABED SECTION - DISTANCE FROM THE BUILDABLE GRID EDGE - MAP FRAME, Y IN METRES - VERTICAL EXAGGERATION X{} (1 M = {} PX UP, {} PX ACROSS)", exag as i32, exag as i32, px_m as i32), 2, T_WHITE);
    png::label(&mut img, 10, 34, "TAN BAND = SEABED MIN..MAX OVER ALL 4 SIDES + CORNERS, PER 4 M OF DISTANCE    BROWN = CREST (THE HIGHEST BED AT THAT DISTANCE)    THIN BROWN = MEAN BED", 2, T_WHITE);
    png::label(&mut img, 10, 58, "BLUE DASHES = SEA SURFACE    GREEN = INSIDE DEEP-WATER BED (THE MAP OWN WATER BLOCKS)    RED = THE RING (V4.22 SHELF RAMPS) + ITS SLAB BAND -0.1..+0.3 M", 2, T_WHITE);
    let mut y_top = top_pad;
    for (ri, r) in rows.iter().enumerate() {
        let (lo, hi) = ranges[ri];
        let h = heights[ri];
        let xp = |d: f32| -> i64 { (left as f32 + (d - d0) * px_m) as i64 };
        let yp = |y: f32| -> i64 { (y_top as f32 + (hi - y) * exag) as i64 };
        // frame + horizontal grid every 2 m, vertical grid every cell
        let mut y = lo;
        while y <= hi + 0.01 {
            let yy = yp(y);
            line(&mut img, xp(d0), yy, xp(d1), yy, GRID, 1);
            if (y.round() as i32) % 2 == 0 {
                png::label(&mut img, 8, (yy - 9).max(0) as usize, &format!("{:>5}", format!("{y:+.0} M")), 2, T_WHITE);
            }
            y += 1.0;
        }
        let mut d = d0;
        while d <= d1 + 0.01 {
            let xx = xp(d);
            line(&mut img, xx, yp(hi), xx, yp(lo), if (d as i32) % 160 == 0 { [190, 190, 190] } else { GRID }, 1);
            d += 32.0;
        }
        line(&mut img, xp(d0), yp(lo), xp(d1), yp(lo), AXIS, 2);
        line(&mut img, xp(d0), yp(hi), xp(d0), yp(lo), AXIS, 2);
        // the bed band, crest and mean
        for b in &r.bands {
            let (x0, x1) = (xp(b.0), xp(b.0 + 4.0));
            let (ya, yb) = (yp(b.2), yp(b.1));
            if yb > ya {
                img.fill(x0 as usize, ya as usize, (x1 - x0) as usize, (yb - ya) as usize, BAND);
            }
        }
        for w in r.bands.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            line(&mut img, xp(a.0 + 2.0), yp(a.2), xp(b.0 + 2.0), yp(b.2), CREST, 3);
            line(&mut img, xp(a.0 + 2.0), yp(a.3), xp(b.0 + 2.0), yp(b.3), MEAN, 1);
        }
        // inside deep-water bed (the map's open-water blocks) from the inner edge to the grid edge
        line(&mut img, xp(d0), yp(r.inside_bed), xp(0.0), yp(r.inside_bed), INSIDE, 3);
        png::label(&mut img, (xp(d0) + 8) as usize, (yp(r.inside_bed) + 6) as usize, &format!("INSIDE BED {:+.1} (THE MAP OWN DEEP-WATER BLOCKS)", r.inside_bed), 2, T_INSIDE);
        // sea surface(s)
        for (_id, sea) in r.seas.iter() {
            dashed_h(&mut img, xp(d0), xp(d1), yp(*sea), SEA, 2);
        }
        let sea_txt: Vec<String> = r.seas.iter().map(|(id, sea)| format!("{} {:+.2}", mapgeom::sttc::collection_name(*id).to_ascii_uppercase(), sea)).collect();
        let sea_hi = r.seas.iter().map(|s| s.1).fold(f32::NEG_INFINITY, f32::max);
        png::label(&mut img, (xp(d1) - 12 * (14 + sea_txt.join(", ").len() as i64) - 20).max(xp(0.0) + 520) as usize, (yp(sea_hi) - 24).max(y_top as i64 + 22) as usize, &format!("SEA SURFACE: {}", sea_txt.join(", ")), 2, T_SEA);
        // the ring shelf: slab band −0.1 .. +0.3 around each ramp's surface, the surface line on top
        for rp in &r.ramps {
            let (xa, xb) = (xp(rp.0), xp(rp.1));
            let n = (xb - xa).max(1);
            for s in 0..n {
                let t = s as f32 / n as f32;
                let ys = rp.2 + (rp.3 - rp.2) * t;
                let (ya, yb) = (yp(ys + 0.3), yp(ys - 0.1));
                if yb > ya {
                    img.fill((xa + s) as usize, ya as usize, 1, (yb - ya) as usize, RINGBAND);
                }
            }
        }
        for rp in &r.ramps {
            line(&mut img, xp(rp.0), yp(rp.2), xp(rp.1), yp(rp.3), RING, 3);
        }
        if let (Some(first), Some(last)) = (r.ramps.first(), r.ramps.last()) {
            let yb = yp(lo) - 14;
            line(&mut img, xp(first.0), yb, xp(last.1), yb, RING, 2);
            line(&mut img, xp(first.0), yb - 8, xp(first.0), yb + 8, RING, 2);
            line(&mut img, xp(last.1), yb - 8, xp(last.1), yb + 8, RING, 2);
            png::label(&mut img, (xp(first.0) + 10) as usize, (yb - 30) as usize, &format!("RING = SHELF RAMPS 0..{:.0} M ({} CELLS), SURFACE {:+.2} AT THE EDGE TO {:+.2} AT {:.0} M", last.1, (last.1 / 32.0) as i32, first.2, last.3 + 0.0, last.1), 2, T_RING);
        }
        // the grid-end marker
        line(&mut img, xp(0.0), yp(hi) - 10, xp(0.0), yp(lo) + 10, MARK, 3);
        png::label(&mut img, (xp(0.0) + 8) as usize, (yp(hi) - 2) as usize, "GRID ENDS HERE: NOTHING CAN BE BUILT PAST THIS LINE", 2, T_YELLOW);
        png::label(&mut img, (xp(0.0) - 410) as usize, (yp(hi) - 2) as usize, "BUILDABLE GRID (64 CELLS WIDE)", 2, T_WHITE);
        png::label(&mut img, (xp(0.0) + 8) as usize, (yp(hi) + 20) as usize, "OPEN SEA (THE VISTA SCENE) FROM HERE ON", 2, T_WHITE);
        let crest = r.bands.iter().cloned().fold((0.0f32, f32::NEG_INFINITY), |a, b| if b.2 > a.1 { (b.0, b.2) } else { a });
        let deep = r.bands.iter().cloned().fold((0.0f32, f32::INFINITY), |a, b| if b.1 < a.1 { (b.0, b.1) } else { a });
        png::label(&mut img, (xp(d1) - 790) as usize, (y_top as i64 + 46) as usize, &format!("BED CREST {:+.2} M AT {:.0} M OUT, DEEPEST {:+.2} M AT {:.0} M OUT", crest.1, crest.0, deep.1, deep.0), 2, T_WHITE);
        // title + lake note
        let lakes: Vec<String> = r.lake_top.iter().map(|(id, t)| format!("{} {:+.2}", mapgeom::sttc::collection_name(*id).to_ascii_uppercase(), t)).collect();
        png::label(&mut img, left, y_top - 30, &format!("{}   (WATER BLOCKS INSIDE THE GRID: SURFACE {})", r.title, lakes.join(", ")), 2, T_YELLOW);
        // x axis: cells and metres
        let mut c = -inner;
        while c <= outer {
            let d = 32.0 * c as f32;
            let xx = xp(d);
            line(&mut img, xx, yp(lo), xx, yp(lo) + 6, AXIS, 2);
            if c % 2 == 0 {
                png::label(&mut img, (xx - 14).max(0) as usize, (yp(lo) + 10) as usize, &format!("{c:+}"), 2, T_WHITE);
            }
            if c % 4 == 0 {
                png::label(&mut img, (xx - 30).max(0) as usize, (yp(lo) + 32) as usize, &format!("{:+.0} M", d), 2, T_WHITE);
            }
            c += 1;
        }
        png::label(&mut img, (xp(0.0) + 8) as usize, (yp(lo) + 54) as usize, "X AXIS: CELLS (32 M EACH) AND METRES FROM THE GRID EDGE, NEGATIVE = INSIDE THE GRID", 2, T_WHITE);
        y_top += h + gap;
    }
    std::fs::write(&out, png::encode(&img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}: {}x{} px, {} rows, vertical x{exag}", img.w, img.h, rows.len());
    for r in &rows {
        let crest = r.bands.iter().map(|b| b.2).fold(f32::NEG_INFINITY, f32::max);
        let crest_d = r.bands.iter().max_by(|a, b| a.2.partial_cmp(&b.2).unwrap()).map(|b| b.0).unwrap_or(0.0);
        let deepest = r.bands.iter().map(|b| b.1).fold(f32::INFINITY, f32::min);
        println!("  {}: seas {:?}; bed crest {:+.2} at {:.0} m, deepest {:+.2}; inside bed {:+.1}; ramps {:?}", r.title, r.seas.iter().map(|s| format!("{:+.2}", s.1)).collect::<Vec<_>>(), crest, crest_d, deepest, r.inside_bed, r.ramps);
    }
    Ok(())
}
