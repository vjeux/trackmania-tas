//! Real-world check: the ESRI World Imagery aerial of a stretch of the lap,
//! in British National Grid so it lines up with the LIDAR, with our detected
//! edges drawn on it. For LOOKING at, next to what the intensity said —
//! not for shipping (the imagery is ESRI's).

use crate::edges::Edges;
use crate::png::Image;
use crate::track::Track;
use std::path::Path;

/// A 24/32-bit uncompressed BMP, as the ArcGIS export returns it.
fn parse_bmp(b: &[u8]) -> Option<(usize, usize, Vec<[u8; 3]>)> {
    if b.len() < 54 || &b[0..2] != b"BM" {
        return None;
    }
    let off = u32::from_le_bytes(b[10..14].try_into().ok()?) as usize;
    let w = i32::from_le_bytes(b[18..22].try_into().ok()?) as usize;
    let h_signed = i32::from_le_bytes(b[22..26].try_into().ok()?);
    let h = h_signed.unsigned_abs() as usize;
    let bpp = u16::from_le_bytes(b[28..30].try_into().ok()?) as usize;
    if bpp != 24 && bpp != 32 {
        return None;
    }
    let bytes = bpp / 8;
    let stride = (w * bytes + 3) / 4 * 4;
    let mut px = vec![[0u8; 3]; w * h];
    for row in 0..h {
        // bottom-up unless the height is negative
        let src_row = if h_signed > 0 { h - 1 - row } else { row };
        let base = off + src_row * stride;
        for x in 0..w {
            let i = base + x * bytes;
            if i + 2 >= b.len() {
                return None;
            }
            px[row * w + x] = [b[i + 2], b[i + 1], b[i]];
        }
    }
    Some((w, h, px))
}

/// BNG -> WGS84 by Newton iteration on the forward projection (the service
/// will not export small boxes in EPSG:27700 -- "Error: bytes" -- so the
/// aerial comes in Web Mercator and every point is mapped through here).
pub fn bng_to_wgs84(e: f64, n: f64) -> (f64, f64) {
    let (mut lat, mut lon) = (52.07f64, -1.01f64);
    for _ in 0..12 {
        let b = crate::geo::wgs84_to_bng(lat, lon);
        let (fe, fn_) = (b.e - e, b.n - n);
        if fe.abs() < 1e-4 && fn_.abs() < 1e-4 {
            break;
        }
        let h = 1e-6;
        let b1 = crate::geo::wgs84_to_bng(lat + h, lon);
        let b2 = crate::geo::wgs84_to_bng(lat, lon + h);
        let (dedlat, dndlat) = ((b1.e - b.e) / h, (b1.n - b.n) / h);
        let (dedlon, dndlon) = ((b2.e - b.e) / h, (b2.n - b.n) / h);
        let det = dedlat * dndlon - dedlon * dndlat;
        let dlat = (fe * dndlon - dedlon * fn_) / det;
        let dlon = (dedlat * fn_ - fe * dndlat) / det;
        lat -= dlat;
        lon -= dlon;
    }
    (lat, lon)
}

const R_MERC: f64 = 6378137.0;

pub fn mercator(lat: f64, lon: f64) -> (f64, f64) {
    let x = R_MERC * lon.to_radians();
    let y = R_MERC * (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln();
    (x, y)
}

/// The aerial for a BNG box, `px` (BNG) metres per pixel, as a Web Mercator
/// image plus its Mercator box (to map points into it).
pub fn fetch(bbox: (f64, f64, f64, f64), px: f64) -> Result<(usize, usize, Vec<[u8; 3]>, (f64, f64, f64, f64)), String> {
    let (e0, n0, e1, n1) = bbox;
    // the Mercator box that contains the BNG box's four corners
    let mut mb = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (e, n) in [(e0, n0), (e1, n0), (e0, n1), (e1, n1)] {
        let (lat, lon) = bng_to_wgs84(e, n);
        let (x, y) = mercator(lat, lon);
        mb.0 = mb.0.min(x);
        mb.1 = mb.1.min(y);
        mb.2 = mb.2.max(x);
        mb.3 = mb.3.max(y);
    }
    // Mercator metres are stretched by 1/cos(lat) relative to the ground
    let (lat_c, _) = bng_to_wgs84((e0 + e1) / 2.0, (n0 + n1) / 2.0);
    let stretch = 1.0 / lat_c.to_radians().cos();
    let w = ((mb.2 - mb.0) / (px * stretch)).round() as usize;
    let h = ((mb.3 - mb.1) / (px * stretch)).round() as usize;
    let url = format!(
        "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/export?bbox={},{},{},{}&bboxSR=3857&imageSR=3857&size={w},{h}&format=bmp&f=image",
        mb.0, mb.1, mb.2, mb.3
    );
    let out = std::process::Command::new("curl")
        .args(["-s", "-m", "120", "-x", "http://fwdproxy:8080", &url])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    let (w, h, pix) = parse_bmp(&out.stdout).ok_or_else(|| format!("not a BMP ({} bytes): {}", out.stdout.len(), String::from_utf8_lossy(&out.stdout[..out.stdout.len().min(200)])))?;
    Ok((w, h, pix, mb))
}

/// The aerial around station `at` (± `span` m) with the lap's centreline
/// (white dots), left edge (green), right edge (red) and kerb outer edges
/// (yellow) drawn on it; a 10 m scale bar bottom left.
pub fn overlay(tr: &Track, ed: &Edges, at: usize, span: f64, px: f64, out: &Path) -> Result<(), String> {
    let st = &tr.stations[at];
    let bbox = (st.e - span, st.n - span, st.e + span, st.n + span);
    let (w, h, pix, mb) = fetch(bbox, px)?;
    let mut img = Image::new(w, h, [0, 0, 0]);
    for y in 0..h {
        for x in 0..w {
            img.put(x as i64, y as i64, pix[y * w + x]);
        }
    }
    let to_px = |e: f64, n: f64| -> (f64, f64) {
        let (lat, lon) = bng_to_wgs84(e, n);
        let (x, y) = mercator(lat, lon);
        ((x - mb.0) / (mb.2 - mb.0) * w as f64, (mb.3 - y) / (mb.3 - mb.1) * h as f64)
    };
    let n = tr.len();
    let reach = (span * 1.5) as usize;
    let mut prev: Option<([f64; 2], [f64; 2], [f64; 2], [f64; 2])> = None;
    for d in 0..2 * reach {
        let i = (at + n - reach + d) % n;
        let s = &tr.stations[i];
        if (s.e - st.e).abs() > span + 5.0 || (s.n - st.n).abs() > span + 5.0 {
            prev = None;
            continue;
        }
        let l = tr.offset(i, ed.left[i]);
        let r = tr.offset(i, -ed.right[i]);
        let kl = tr.offset(i, ed.left[i] + crate::terrain::kerb_width(ed.kerb_left[i], ed.kerb_left[(i + 1) % n]));
        let kr = tr.offset(i, -(ed.right[i] + crate::terrain::kerb_width(ed.kerb_right[i], ed.kerb_right[(i + 1) % n])));
        let cur = ([l[0], l[1]], [r[0], r[1]], [kl[0], kl[1]], [kr[0], kr[1]]);
        if let Some(p) = prev {
            let seg = |img: &mut Image, a: [f64; 2], b: [f64; 2], c: [u8; 3]| {
                let (x0, y0) = to_px(a[0], a[1]);
                let (x1, y1) = to_px(b[0], b[1]);
                img.line(x0, y0, x1, y1, c);
            };
            seg(&mut img, p.0, cur.0, [0, 255, 0]);
            seg(&mut img, p.1, cur.1, [255, 40, 40]);
            seg(&mut img, p.2, cur.2, [255, 230, 0]);
            seg(&mut img, p.3, cur.3, [255, 230, 0]);
        }
        prev = Some(cur);
        if i % 10 == 0 {
            let (x, y) = to_px(s.e, s.n);
            img.disc(x, y, 1.5, [255, 255, 255]);
        }
    }
    // scale bar: 10 m (measured on the ground)
    let (ax, ay) = to_px(bbox.0 + 3.0, bbox.1 + 3.0);
    let (bx, _) = to_px(bbox.0 + 13.0, bbox.1 + 3.0);
    img.line(ax, ay, bx, ay, [255, 255, 255]);
    img.line(ax, ay + 1.0, bx, ay + 1.0, [255, 255, 255]);
    img.save(out);
    println!("{}: station {at} {} E{:.0} N{:.0}, {w}x{h} px at {px} m/px; left {:.1} m, right {:.1} m (width {:.1})", out.display(), st.label, st.e, st.n, ed.left[at], ed.right[at], ed.left[at] + ed.right[at]);
    Ok(())
}
