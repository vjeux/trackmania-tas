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

/// Where the imagery comes from. ESRI's export is what the F1 edges were
/// checked against; Google's and Bing's tile pyramids are usually more
/// recent (Kart Silverstone opened in 2025 and ESRI still showed it under
/// construction), so a layout that post-dates the LIDAR is read off those.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Source {
    Esri,
    Google,
    Bing,
}

impl Source {
    pub fn parse(s: &str) -> Option<Source> {
        match s.to_ascii_lowercase().as_str() {
            "esri" => Some(Source::Esri),
            "google" => Some(Source::Google),
            "bing" => Some(Source::Bing),
            _ => None,
        }
    }
    fn tile_url(self, x: u32, y: u32, z: u32) -> String {
        match self {
            Source::Google => format!("https://mt{}.google.com/vt/lyrs=s&x={x}&y={y}&z={z}", (x + y) % 4),
            Source::Bing => {
                let mut q = String::new();
                for i in (1..=z).rev() {
                    let d = ((x >> (i - 1)) & 1) | (((y >> (i - 1)) & 1) << 1);
                    q.push(char::from(b'0' + d as u8));
                }
                format!("https://ecn.t{}.tiles.virtualearth.net/tiles/a{q}.jpeg?g=1", (x + y) % 4)
            }
            Source::Esri => format!("https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"),
        }
    }
}

/// One JPEG tile through the forward proxy, decoded to RGB.
fn fetch_tile(src: Source, x: u32, y: u32, z: u32) -> Result<Vec<[u8; 3]>, String> {
    let url = src.tile_url(x, y, z);
    let out = std::process::Command::new("curl")
        .args(["-s", "-m", "60", "-x", "http://fwdproxy:8080", "-A", "Mozilla/5.0", &url])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    let mut dec = jpeg_decoder::Decoder::new(std::io::Cursor::new(&out.stdout));
    let data = dec.decode().map_err(|e| format!("{url}: {e} ({} bytes)", out.stdout.len()))?;
    let info = dec.info().ok_or("no jpeg info")?;
    if info.width != 256 || info.height != 256 {
        return Err(format!("{url}: {}x{} tile", info.width, info.height));
    }
    let px = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => data.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
        jpeg_decoder::PixelFormat::L8 => data.iter().map(|&g| [g, g, g]).collect(),
        other => return Err(format!("{url}: pixel format {other:?}")),
    };
    Ok(px)
}

/// The Web Mercator tile pyramid over a BNG box at `zoom`, stitched: the
/// image and its Mercator box (whole tiles, so a little larger than asked).
/// Zoom 19 is ~0.18 m/px on the ground at Silverstone, 20 is ~0.09.
pub fn fetch_tiles(bbox: (f64, f64, f64, f64), zoom: u32, src: Source) -> Result<(usize, usize, Vec<[u8; 3]>, (f64, f64, f64, f64)), String> {
    let (e0, n0, e1, n1) = bbox;
    let mut mb = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (e, n) in [(e0, n0), (e1, n0), (e0, n1), (e1, n1)] {
        let (lat, lon) = bng_to_wgs84(e, n);
        let (x, y) = mercator(lat, lon);
        mb.0 = mb.0.min(x);
        mb.1 = mb.1.min(y);
        mb.2 = mb.2.max(x);
        mb.3 = mb.3.max(y);
    }
    let world = 2.0 * std::f64::consts::PI * R_MERC;
    let n = (1u64 << zoom) as f64;
    let tile = |m: f64| ((m + world / 2.0) / world * n).floor();
    let tile_y = |m: f64| ((world / 2.0 - m) / world * n).floor();
    let (tx0, tx1) = (tile(mb.0) as u32, tile(mb.2) as u32);
    let (ty0, ty1) = (tile_y(mb.3) as u32, tile_y(mb.1) as u32);
    let (nx, ny) = ((tx1 - tx0 + 1) as usize, (ty1 - ty0 + 1) as usize);
    let (w, h) = (nx * 256, ny * 256);
    let coords: Vec<(u32, u32)> = (ty0..=ty1).flat_map(|ty| (tx0..=tx1).map(move |tx| (tx, ty))).collect();
    // eight tiles at a time
    let results: Vec<Result<Vec<[u8; 3]>, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = coords.chunks((coords.len() + 7) / 8).map(|chunk| s.spawn(move || chunk.iter().map(|&(x, y)| fetch_tile(src, x, y, zoom)).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().expect("tile thread")).collect()
    });
    let mut pix = vec![[0u8; 3]; w * h];
    let mut failed = 0;
    for (k, r) in results.into_iter().enumerate() {
        let (tx, ty) = coords[k];
        let (ox, oy) = ((tx - tx0) as usize * 256, (ty - ty0) as usize * 256);
        match r {
            Ok(t) => {
                for y in 0..256 {
                    pix[(oy + y) * w + ox..(oy + y) * w + ox + 256].copy_from_slice(&t[y * 256..y * 256 + 256]);
                }
            }
            Err(e) => {
                failed += 1;
                eprintln!("tile {tx},{ty}: {e}");
            }
        }
    }
    if failed == coords.len() {
        return Err(format!("all {} tiles failed", coords.len()));
    }
    let tile_m = world / n;
    let full = (tx0 as f64 * tile_m - world / 2.0, world / 2.0 - (ty1 + 1) as f64 * tile_m, (tx1 + 1) as f64 * tile_m - world / 2.0, world / 2.0 - ty0 as f64 * tile_m);
    eprintln!("{src:?} zoom {zoom}: {nx}x{ny} tiles, {failed} failed");
    Ok((w, h, pix, full))
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

/// The aerial of a BNG box with the OSM ways carrying `tag` (key=value)
/// drawn on it (one colour per way, node dots), for reading a layout off
/// the imagery.
pub fn box_overlay(bbox: (f64, f64, f64, f64), px: f64, osm: Option<&crate::osm::Ways>, tag: Option<&str>, out: &Path, src: Source, zoom: u32, extra: &[(Vec<crate::geo::Bng>, [u8; 3])]) -> Result<(), String> {
    let (w, h, pix, mb) = if src == Source::Esri { fetch(bbox, px)? } else { fetch_tiles(bbox, zoom, src)? };
    // crop the stitched tiles to the asked box (plus a 2 m margin)
    let to_px_full = |e: f64, n: f64| -> (f64, f64) {
        let (lat, lon) = bng_to_wgs84(e, n);
        let (x, y) = mercator(lat, lon);
        ((x - mb.0) / (mb.2 - mb.0) * w as f64, (mb.3 - y) / (mb.3 - mb.1) * h as f64)
    };
    let (mut cx0, mut cy0, mut cx1, mut cy1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (e, n) in [(bbox.0, bbox.1), (bbox.2, bbox.1), (bbox.0, bbox.3), (bbox.2, bbox.3)] {
        let (x, y) = to_px_full(e, n);
        cx0 = cx0.min(x);
        cy0 = cy0.min(y);
        cx1 = cx1.max(x);
        cy1 = cy1.max(y);
    }
    let (cx0, cy0) = ((cx0.floor() as i64).clamp(0, w as i64 - 1) as usize, (cy0.floor() as i64).clamp(0, h as i64 - 1) as usize);
    let (cx1, cy1) = ((cx1.ceil() as i64).clamp(cx0 as i64 + 1, w as i64) as usize, (cy1.ceil() as i64).clamp(cy0 as i64 + 1, h as i64) as usize);
    let (cw, ch) = (cx1 - cx0, cy1 - cy0);
    let mut img = Image::new(cw, ch, [0, 0, 0]);
    for y in 0..ch {
        for x in 0..cw {
            img.put(x as i64, y as i64, pix[(y + cy0) * w + x + cx0]);
        }
    }
    let to_px = |e: f64, n: f64| -> (f64, f64) {
        let (x, y) = to_px_full(e, n);
        (x - cx0 as f64, y - cy0 as f64)
    };
    let (w, h) = (cw, ch);
    // extra polylines (a candidate lap, a driven line), thick
    for (pts, c) in extra {
        for q in pts.windows(2) {
            let (x0, y0) = to_px(q[0].e, q[0].n);
            let (x1, y1) = to_px(q[1].e, q[1].n);
            for d in [-1.0, 0.0, 1.0] {
                img.line(x0 + d, y0, x1 + d, y1, *c);
                img.line(x0, y0 + d, x1, y1 + d, *c);
            }
        }
    }
    if let (Some(ways), Some(tag)) = (osm, tag) {
        let (k, v) = tag.split_once('=').ok_or("--tag wants key=value")?;
        let palette = [[255, 60, 60], [60, 255, 60], [60, 120, 255], [255, 230, 0], [255, 0, 255], [0, 255, 255], [255, 150, 0], [180, 255, 120], [255, 120, 180], [120, 200, 255], [200, 200, 200]];
        let mut i = 0;
        for way in &ways.ways {
            if way.tags.get(k).map(|x| x == v) != Some(true) {
                continue;
            }
            let c = palette[i % palette.len()];
            i += 1;
            let pts: Vec<_> = way.nodes.iter().filter_map(|id| ways.nodes.get(id)).collect();
            for q in pts.windows(2) {
                let (x0, y0) = to_px(q[0].e, q[0].n);
                let (x1, y1) = to_px(q[1].e, q[1].n);
                img.line(x0, y0, x1, y1, c);
                img.line(x0 + 1.0, y0, x1 + 1.0, y1, c);
            }
            for p in &pts {
                let (x, y) = to_px(p.e, p.n);
                img.disc(x, y, 2.0, [255, 255, 255]);
            }
            if let Some(p) = pts.first() {
                let (x, y) = to_px(p.e, p.n);
                img.disc(x, y, 4.0, c);
            }
            println!("way {} {:?} {} nodes colour {:?}", way.id, way.name, pts.len(), c);
        }
    }
    let (ax, ay) = to_px(bbox.0 + 3.0, bbox.1 + 3.0);
    let (bx, _) = to_px(bbox.0 + 53.0, bbox.1 + 3.0);
    img.line(ax, ay, bx, ay, [255, 255, 255]);
    img.line(ax, ay + 1.0, bx, ay + 1.0, [255, 255, 255]);
    img.save(out);
    println!("{}: {w}x{h} px, scale bar 50 m", out.display());
    Ok(())
}
