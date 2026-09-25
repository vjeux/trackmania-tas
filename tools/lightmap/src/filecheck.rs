//! ROW "file": the pwc6 run's END buffers (final_pwc6/frame7537: the encoded Y4 / Cb4 / Cr4 textures, the
//! MaxHdr buffer, the probe volumes) through the client's CPU steps to the lightmap chunk of the SAME run's
//! save (pwc-day-baked-q3-editor-capture6.Map.Gbx), byte for byte where the file is lossless and through
//! libwebp where it is VP8.
//!
//! The client (decomp2 14029c450 "frame images → blobs"): image 0 → `FUN_14029c830`: copy, `FUN_14029add0`
//! (the per-chart normalisation: fb0 = max over the chart of max(R, G, B), linked charts share the max,
//! then bytes × 255/fb0 truncated), `FUN_14029bc10` = the RGB import encode at quality 0x5b = 91; image 1 →
//! `FUN_14029bf40` (channels 0..2 of the source as three Y-only WEBPs, U = V = 128, quality `DAT_14205c7fc`)
//! or `FUN_14029bc10` at 0x50 = 80 in the other mode; image 2 → `FUN_14029cbf0` (the f16 vegetation atlas:
//! max → 1/max scale, a colour conversion, bytes truncated, format 5).

use crate::passdiff::Buf;

/// One channel of an RGBA8 texture as bytes.
pub fn channel_bytes(b: &Buf, c: u32) -> Vec<u8> {
    let mut out = vec![0u8; (b.w * b.h) as usize];
    for y in 0..b.h {
        for x in 0..b.w {
            out[(y * b.w + x) as usize] = (b.get(x, y, c) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// How a 2048² plane becomes a 1024² one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Down {
    /// the texel (2x, 2y)
    Sample00,
    /// the texel (2x+1, 2y+1)
    Sample11,
    /// the 2×2 mean, truncated
    MeanTrunc,
    /// the 2×2 mean, rounded (+2 >> 2)
    MeanRound,
    /// (s + 1) >> 2
    MeanPlus1,
    /// (s + 3) >> 2 (ceiling)
    MeanCeil,
    /// two passes of pairwise rounding means: ((a+b+1)>>1 + (c+d+1)>>1 + 1) >> 1
    PairwiseRound,
    /// two passes of pairwise truncating means
    PairwiseTrunc,
}

pub fn downsample(plane: &[u8], w: usize, h: usize, d: Down) -> Vec<u8> {
    let (ow, oh) = (w / 2, h / 2);
    let mut out = vec![0u8; ow * oh];
    for y in 0..oh {
        for x in 0..ow {
            let p = |dx: usize, dy: usize| plane[(2 * y + dy) * w + 2 * x + dx] as u32;
            out[y * ow + x] = match d {
                Down::Sample00 => p(0, 0) as u8,
                Down::Sample11 => p(1, 1) as u8,
                Down::MeanTrunc => ((p(0, 0) + p(1, 0) + p(0, 1) + p(1, 1)) / 4) as u8,
                Down::MeanRound => ((p(0, 0) + p(1, 0) + p(0, 1) + p(1, 1) + 2) / 4) as u8,
                Down::MeanPlus1 => ((p(0, 0) + p(1, 0) + p(0, 1) + p(1, 1) + 1) / 4) as u8,
                Down::MeanCeil => ((p(0, 0) + p(1, 0) + p(0, 1) + p(1, 1) + 3) / 4) as u8,
                Down::PairwiseRound => (((p(0, 0) + p(1, 0) + 1) / 2 + (p(0, 1) + p(1, 1) + 1) / 2 + 1) / 2) as u8,
                Down::PairwiseTrunc => (((p(0, 0) + p(1, 0)) / 2 + (p(0, 1) + p(1, 1)) / 2) / 2) as u8,
            };
        }
    }
    out
}

/// Split a concatenation of RIFF files.
pub fn split_riff(b: &[u8]) -> Vec<&[u8]> {
    let mut parts = Vec::new();
    let mut off = 0usize;
    while off + 12 <= b.len() && &b[off..off + 4] == b"RIFF" {
        let sz = u32::from_le_bytes([b[off + 4], b[off + 5], b[off + 6], b[off + 7]]) as usize + 8;
        parts.push(&b[off..(off + sz).min(b.len())]);
        off += sz;
    }
    if parts.is_empty() {
        parts.push(b);
    }
    parts
}

/// The first differing byte of two byte strings, and the VP8 frame header bytes of each.
pub fn cmp_bytes(a: &[u8], b: &[u8]) -> String {
    let common = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let hdr = |v: &[u8]| -> String { v.iter().position(|&x| x == b'V').map(|p| v[p + 8..(p + 24).min(v.len())].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")).unwrap_or_default() };
    if a.len() == b.len() && common == a.len() {
        format!("BYTE-IDENTICAL ({} bytes)", a.len())
    } else {
        format!("{} vs {} bytes, first difference at byte {common}; VP8 heads: theirs {} | ours {}", a.len(), b.len(), hdr(a), hdr(b))
    }
}

/// Decoded-image difference: (values, exact, within 1, within 2, max).
pub fn cmp_decoded(a: &[u8], b: &[u8]) -> Option<(usize, usize, usize, usize, u8)> {
    let ia = crate::img::decode_webp(a).ok()?;
    let ib = crate::img::decode_webp(b).ok()?;
    if ia.w != ib.w || ia.h != ib.h {
        return None;
    }
    let (mut ex, mut w1, mut w2, mut mx) = (0usize, 0usize, 0usize, 0u8);
    for (x, y) in ia.px.iter().zip(ib.px.iter()) {
        let d = x.abs_diff(*y);
        if d == 0 { ex += 1; }
        if d <= 1 { w1 += 1; }
        if d <= 2 { w2 += 1; }
        mx = mx.max(d);
    }
    Some((ia.px.len(), ex, w1, w2, mx))
}

/// `lmtool final-check ROOT MAP [--frame 7537]`: the greys (frame 0 image 1) from Y4's channels 1..3 through
/// the downsample candidates and libwebp, against the file's three WEBPs.
pub fn check_greys(root: &std::path::Path, map: &str, frame: u32, qualities: &[f32]) -> Result<(), String> {
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
    let m = crate::passdiff::read_manifest(&txt)?;
    let tex = |id: &str| -> Result<Buf, String> {
        let e = m.passes.iter().filter(|e| e.pass == "final_06_encoded_rgba8_cs23025" && e.frame == Some(frame) && e.file.contains(&format!("_{id}.dds"))).max_by_key(|e| e.eid.unwrap_or(0)).ok_or_else(|| format!("no encoded texture {id} in frame {frame}"))?;
        println!("{id}: {} (eid {:?})", e.file, e.eid);
        crate::passdiff::load_entry(root, e)
    };
    let y4 = tex("8797")?;
    println!("Y4 {}×{} ×{}; libwebp {} linked: {}", y4.w, y4.h, y4.channels, crate::webpenc::version(), crate::webpenc::available());
    let mm = crate::mapio::load(map)?;
    let d = mm.chunk.data.as_ref().ok_or("no lightmap data")?;
    let greys = split_riff(&d.frames[0].images[1]);
    println!("file: frame 0 image 1 = {} WEBPs of {:?} bytes", greys.len(), greys.iter().map(|g| g.len()).collect::<Vec<_>>());
    for (k, g) in greys.iter().enumerate() {
        let plane = channel_bytes(&y4, (k + 1) as u32);
        let theirs = crate::img::decode_webp(g)?;
        println!("grey {k}: decoded {}×{}", theirs.w, theirs.h);
        for down in [Down::Sample00, Down::Sample11, Down::MeanTrunc, Down::MeanRound] {
            let small = if theirs.w as usize * 2 == y4.w as usize { downsample(&plane, y4.w as usize, y4.h as usize, down) } else { plane.clone() };
            // the decoded grey = the Y plane expanded from studio range: compare our plane with the decoded value's Y
            let mut ex = 0usize;
            let mut n = 0usize;
            for (i, &p) in small.iter().enumerate() {
                let dec = theirs.px[i * 3] as i32;
                // Y = 16 + dec·219/255 (the decoder expands (Y−16)·255/219); compare after the same expansion
                let ours = (((p as i32 - 16) * 255 + 109) / 219).clamp(0, 255);
                n += 1;
                if (ours - dec).abs() <= 1 { ex += 1; }
            }
            println!("  {down:?}: plane vs decoded within ±1: {ex} of {n} ({:.2} %)", 100.0 * ex as f64 / n as f64);
            for &q in qualities {
                if let Some(ours) = crate::webpenc::encode_grey(&small, theirs.w, theirs.h, q) {
                    let dec = cmp_decoded(g, &ours);
                    println!("    q {q}: {}{}", cmp_bytes(g, &ours), dec.map(|(n, ex, w1, w2, mx)| format!("; decoded: {ex} of {n} exact, {w1} within 1, {w2} within 2, max {mx}")).unwrap_or_default());
                }
            }
        }
    }
    Ok(())
}

// ───────────────────────────── the CPU steps, transcribed ─────────────────────────────

/// `NHmsLightMap::YCbCr_to_RGB_Down2x2` (client 0x14022af60): the colour image of a frame from the encode's
/// three planes — for every output pixel (x, y) of the HALF-size image: the four Y bytes of the 2×2 block
/// `(2x.., 2y..)` of `y4`'s channel `c` are summed in f32, `l = (Σ · 0.25) · 1.1643835`; with `cb`, `cr` the
/// bytes of the chroma planes at (x, y) (channel `c`):
/// `R = (l − 222.92155) + cb·0 + cr·1.5960268`, `G = ((l + 135.5753) − cb·0.3917623) − cr·0.81296766`,
/// `B = (l − 276.83585) + cb·2.0172322 + cr·0`, each stored as `(int)v` clamped to 0..255 (truncation toward
/// zero; values below 1 give 0). Returns RGB triplets, `w/2 × h/2`.
pub fn ycbcr_to_rgb_down2x2(y4: &[u8], cb4: &[u8], cr4: &[u8], w: usize, h: usize) -> Vec<u8> {
    let (ow, oh) = (w / 2, h / 2);
    let mut out = vec![0u8; ow * oh * 3];
    let clamp = |v: f32| -> u8 { let i = v as i32; if i < 1 { 0 } else if i > 0xfe { 0xff } else { i as u8 } };
    for y in 0..oh {
        for x in 0..ow {
            let mut sum = 0.0f32;
            for dy in 0..2 {
                for dx in 0..2 {
                    sum += y4[(2 * y + dy) * w + 2 * x + dx] as f32;
                }
            }
            let l = sum * 0.25 * 1.1643835;
            let cb = cb4[y * ow + x] as f32;
            let cr = cr4[y * ow + x] as f32;
            let r = (l - 222.92155) + cb * 0.0 + cr * 1.5960268;
            let g = ((l + 135.5753) - cb * 0.3917623) - cr * 0.81296766;
            let b = (l - 276.83585) + cb * 2.0172322 + cr * 0.0;
            let o = (y * ow + x) * 3;
            out[o] = clamp(r);
            out[o + 1] = clamp(g);
            out[o + 2] = clamp(b);
        }
    }
    out
}

/// The grey planes' expansion (client 0x14022b370, l.~150): `byte = (int)(Y · 1.1643835 − 18.630136)` clamped to
/// 0..255 (truncation) — Y4's channels 1..3 to full range, still at the encode's resolution.
pub fn expand_grey(plane: &[u8]) -> Vec<u8> {
    plane.iter().map(|&y| { let v = (y as f32 * 1.1643835 - 18.630136) as i32; if v < 1 { 0 } else if v > 0xfe { 0xff } else { v as u8 } }).collect()
}

/// One chart's pixel rectangle as the client's per-chart pass spans it (0x14029ac70, read off the file: the
/// rule that reproduces all 4099 frame bytes): the layout rectangle `[x, x + w] × [y, y + h]` in half units
/// with floor division — `(x / 2, y / 2)` to `((x + w) / 2, (y + h) / 2)` inclusive, i.e. the chart's
/// `w/2 × h/2` pixels plus the one-pixel gutter column and row before them (odd positions, even sizes).
pub fn chart_px(x: u32, y: u32, w: u32, h: u32) -> (u32, u32, u32, u32) {
    (x / 2, y / 2, (x + w) / 2 - x / 2 + 1, (y + h) / 2 - y / 2 + 1)
}

/// The per-chart normalisation of the colour image (client 0x14029add0): per chart the largest R/G/B byte over
/// its pixels (`fb0`; charts stacked without a gap — a chart whose top-left is another's bottom-left — form a
/// chain sharing the chain's maximum; an empty or out-of-image chart gets 0xff); then every pixel of a chart
/// with 1 ≤ fb0 ≤ 254 is multiplied by 255/fb0 and truncated. Returns (fb0 per chart, the scaled image).
pub fn chart_normalise(rgb: &mut [u8], w: u32, h: u32, charts: &[(u32, u32, u32, u32)]) -> Vec<u8> {
    let n = charts.len();
    let px: Vec<(u32, u32, u32, u32)> = charts.iter().map(|&(x, y, cw, ch)| chart_px(x, y, cw, ch)).collect();
    // the chains: position (x, y + h) → the chart starting there
    let mut by_pos = std::collections::HashMap::<(u32, u32), usize>::new();
    for (i, &(x, y, cw, ch)) in charts.iter().enumerate() {
        if cw != 0 && ch != 0 && x + cw < 2 * w && y + ch < 2 * h {
            by_pos.insert((x, y), i);
        }
    }
    let mut next = vec![usize::MAX; n];
    let mut prev = vec![usize::MAX; n];
    for (i, &(x, y, _cw, ch)) in charts.iter().enumerate() {
        if y + ch < 2 * h {
            if let Some(&j) = by_pos.get(&(x, y + ch)) {
                if j != i {
                    next[i] = j;
                    prev[j] = i;
                }
            }
        }
    }
    let mut fb = vec![0u8; n];
    for i in 0..n {
        let (x0, y0, cw, ch) = px[i];
        if cw == 0 || ch == 0 || x0 + cw > w || y0 + ch > h {
            fb[i] = 0xff;
            continue;
        }
        let mut m = 0u8;
        for y in y0..y0 + ch {
            for x in x0..x0 + cw {
                let o = ((y * w + x) * 3) as usize;
                m = m.max(rgb[o]).max(rgb[o + 1]).max(rgb[o + 2]);
            }
        }
        fb[i] = m;
    }
    // chain heads propagate the chain max
    for i in 0..n {
        if next[i] != usize::MAX && prev[i] == usize::MAX {
            let mut m = fb[i];
            let mut j = next[i];
            while j != usize::MAX { m = m.max(fb[j]); j = next[j]; }
            let mut j = i;
            while j != usize::MAX { fb[j] = m; j = next[j]; }
        }
    }
    for i in 0..n {
        let m = fb[i];
        if m == 0 || m == 255 { continue; }
        let (x0, y0, cw, ch) = px[i];
        if cw == 0 || ch == 0 || x0 + cw > w || y0 + ch > h { continue; }
        let s = 255.0f32 / m as f32;
        // a chained chart's last row is left out when it has a successor (the decompile's `− 1`)
        let rows = if next[i] != usize::MAX { ch.saturating_sub(1) } else { ch };
        for y in y0..y0 + rows {
            for x in x0..x0 + cw {
                let o = ((y * w + x) * 3) as usize;
                for k in 0..3 { rgb[o + k] = (rgb[o + k] as f32 * s) as i32 as u8; }
            }
        }
    }
    fb
}

/// `lmtool final-check … --colour`: Y4/Cb4/Cr4 → the colour image (Down2x2) → per-chart fb0 (vs the file's
/// frame bytes) → libwebp q91 (vs blob 0).
pub fn check_colour(root: &std::path::Path, map: &str, frame: u32) -> Result<(), String> {
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
    let m = crate::passdiff::read_manifest(&txt)?;
    let tex = |id: &str| -> Result<Buf, String> {
        let e = m.passes.iter().filter(|e| e.pass == "final_06_encoded_rgba8_cs23025" && e.frame == Some(frame) && e.file.contains(&format!("_{id}.dds"))).max_by_key(|e| e.eid.unwrap_or(0)).ok_or_else(|| format!("no encoded texture {id} in frame {frame}"))?;
        crate::passdiff::load_entry(root, e)
    };
    let (y4, cb4, cr4) = (tex("8797")?, tex("8800")?, tex("8803")?);
    let (w, h) = (y4.w as usize, y4.h as usize);
    let mm = crate::mapio::load(map)?;
    let d = mm.chunk.data.as_ref().ok_or("no lightmap data")?;
    let mp = d.cache.mapping().ok_or("no mapping")?;
    let charts: Vec<(u32, u32, u32, u32)> = (0..mp.count as usize).map(|i| (mp.pos[i].0 as u32, mp.pos[i].1 as u32, mp.size[i].0 as u32, mp.size[i].1 as u32)).collect();
    let mut rgb = ycbcr_to_rgb_down2x2(&channel_bytes(&y4, 0), &channel_bytes(&cb4, 0), &channel_bytes(&cr4, 0), w, h);
    let (ow, oh) = ((w / 2) as u32, (h / 2) as u32);
    let fb = chart_normalise(&mut rgb, ow, oh, &charts);
    let file_fb = &mp.frame_bytes[0];
    let same = fb.iter().zip(file_fb.iter()).filter(|(a, b)| a == b).count();
    println!("colour: {}×{}; per-chart fb0: {} of {} charts identical to the file's frame bytes", ow, oh, same, charts.len());
    let mut shown = 0;
    let mut hist = std::collections::BTreeMap::<i32, usize>::new();
    for i in 0..charts.len() {
        *hist.entry(fb[i] as i32 - file_fb[i] as i32).or_default() += 1;
        if fb[i] != file_fb[i] && shown < 8 { println!("  chart {i} {:?}: ours {} file {}", charts[i], fb[i], file_fb[i]); shown += 1; }
    }
    println!("  fb0 difference histogram (ours − file): {:?}", hist);
    let blob0 = &d.frames[0].images[0];
    let theirs = crate::img::decode_webp(blob0)?;
    let (mut ex, mut w1, mut w2, mut mx) = (0usize, 0usize, 0usize, 0u8);
    for (a, b) in rgb.iter().zip(theirs.px.iter()) { let dd = a.abs_diff(*b); if dd == 0 { ex += 1; } if dd <= 1 { w1 += 1; } if dd <= 2 { w2 += 1; } mx = mx.max(dd); }
    println!("  our image vs the file's decoded blob 0: {ex} of {} bytes exact, {w1} within 1, {w2} within 2, max {mx}", rgb.len());
    for q in [91.0f32, 90.0, 92.0, 80.0, 75.0] {
        if let Some(ours) = crate::webpenc::encode_rgb(&rgb, ow, oh, q) {
            println!("  libwebp RGB import q {q}: {}{}", cmp_bytes(blob0, &ours), cmp_decoded(blob0, &ours).map(|(n, ex, w1, w2, mx)| format!("; decoded: {ex} of {n} exact, {w1} within 1, {w2} within 2, max {mx}")).unwrap_or_default());
        }
    }
    Ok(())
}

/// `lmtool final-check … --greys2`: the greys with the client's order (expand-truncate at the encode's
/// resolution, then the 2× resize) under the resize roundings, libwebp q30, vs the file's three WEBPs.
pub fn check_greys2(root: &std::path::Path, map: &str, frame: u32, q: f32) -> Result<(), String> {
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
    let m = crate::passdiff::read_manifest(&txt)?;
    let e = m.passes.iter().filter(|e| e.pass == "final_06_encoded_rgba8_cs23025" && e.frame == Some(frame) && e.file.contains("_8797.dds")).max_by_key(|e| e.eid.unwrap_or(0)).ok_or("no Y4")?;
    let y4 = crate::passdiff::load_entry(root, e)?;
    let (w, h) = (y4.w as usize, y4.h as usize);
    let mm = crate::mapio::load(map)?;
    let d = mm.chunk.data.as_ref().ok_or("no lightmap data")?;
    let greys = split_riff(&d.frames[0].images[1]);
    for (k, g) in greys.iter().enumerate() {
        let raw = channel_bytes(&y4, (k + 1) as u32);
        let full = expand_grey(&raw);
        let theirs = crate::img::decode_webp(g)?;
        for (order, down) in [("expand→resize", Down::MeanTrunc), ("expand→resize", Down::MeanRound), ("expand→resize", Down::MeanPlus1), ("expand→resize", Down::MeanCeil), ("expand→resize", Down::PairwiseRound), ("expand→resize", Down::PairwiseTrunc), ("expand→resize", Down::Sample00), ("resize→expand", Down::MeanTrunc), ("resize→expand", Down::MeanRound)] {
            let small = if order == "expand→resize" { downsample(&full, w, h, down) } else { expand_grey(&downsample(&raw, w, h, down)) };
            // the decoder expands the stored Y plane once more: compare expand(ours) with the decoded grey
            let dec_ours = expand_grey(&small);
            let mut ex = 0usize;
            for (i, &p) in dec_ours.iter().enumerate() { if (p as i32 - theirs.px[i * 3] as i32).abs() <= 1 { ex += 1; } }
            let down = format!("{order} {down:?}");
            let ours = crate::webpenc::encode_grey(&small, theirs.w, theirs.h, q);
            println!("grey {k} {down}: expand(plane) vs decoded within ±1: {ex} of {} ({:.2} %); q {q}: {}", small.len(), 100.0 * ex as f64 / small.len() as f64, ours.as_ref().map(|o| format!("{}{}", cmp_bytes(g, o), cmp_decoded(g, o).map(|(n, ex, w1, w2, mx)| format!("; decoded {ex} of {n} exact, {w1} within 1, {w2} within 2, max {mx}")).unwrap_or_default())).unwrap_or_else(|| "no libwebp".into()));
        }
    }
    Ok(())
}

/// `--rects`: which pixel rectangle the client's per-chart maximum spans — the fb0 match rate under a few
/// rules for turning the layout rectangle into pixels.
pub fn check_rects(root: &std::path::Path, map: &str, frame: u32) -> Result<(), String> {
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
    let m = crate::passdiff::read_manifest(&txt)?;
    let tex = |id: &str| -> Result<Buf, String> {
        let e = m.passes.iter().filter(|e| e.pass == "final_06_encoded_rgba8_cs23025" && e.frame == Some(frame) && e.file.contains(&format!("_{id}.dds"))).max_by_key(|e| e.eid.unwrap_or(0)).ok_or_else(|| format!("no encoded texture {id} in frame {frame}"))?;
        crate::passdiff::load_entry(root, e)
    };
    let (y4, cb4, cr4) = (tex("8797")?, tex("8800")?, tex("8803")?);
    let (w, h) = (y4.w as usize, y4.h as usize);
    let mm = crate::mapio::load(map)?;
    let d = mm.chunk.data.as_ref().ok_or("no lightmap data")?;
    let mp = d.cache.mapping().ok_or("no mapping")?;
    let rgb = ycbcr_to_rgb_down2x2(&channel_bytes(&y4, 0), &channel_bytes(&cb4, 0), &channel_bytes(&cr4, 0), w, h);
    let ow = (w / 2) as u32;
    let file_fb = &mp.frame_bytes[0];
    let rules: [(&str, &dyn Fn(u32, u32, u32, u32) -> (u32, u32, u32, u32)); 6] = [
        ("((x+1)/2, (y+1)/2, w/2, h/2)", &|x, y, cw, ch| ((x + 1) / 2, (y + 1) / 2, cw / 2, ch / 2)),
        ("(x/2, y/2, w/2+1, h/2+1)", &|x, y, cw, ch| (x / 2, y / 2, cw / 2 + 1, ch / 2 + 1)),
        ("(x/2, y/2, (w+1)/2, (h+1)/2)", &|x, y, cw, ch| (x / 2, y / 2, (cw + 1) / 2, (ch + 1) / 2)),
        ("((x+1)/2, (y+1)/2, w/2+1, h/2+1)", &|x, y, cw, ch| ((x + 1) / 2, (y + 1) / 2, cw / 2 + 1, ch / 2 + 1)),
        ("(x/2, y/2, (w+2)/2, (h+2)/2)", &|x, y, cw, ch| (x / 2, y / 2, (cw + 2) / 2, (ch + 2) / 2)),
        ("((x+1)/2-1, (y+1)/2-1, w/2+2, h/2+2)", &|x, y, cw, ch| (((x + 1) / 2).saturating_sub(1), ((y + 1) / 2).saturating_sub(1), cw / 2 + 2, ch / 2 + 2)),
    ];
    for (name, rule) in rules.iter() {
        let mut same = 0usize;
        let mut hist = std::collections::BTreeMap::<i32, usize>::new();
        for i in 0..mp.count as usize {
            let (x0, y0, cw, ch) = rule(mp.pos[i].0 as u32, mp.pos[i].1 as u32, mp.size[i].0 as u32, mp.size[i].1 as u32);
            let mut mx = 0u8;
            for y in y0..(y0 + ch).min(ow) { for x in x0..(x0 + cw).min(ow) { let o = ((y * ow + x) * 3) as usize; mx = mx.max(rgb[o]).max(rgb[o + 1]).max(rgb[o + 2]); } }
            if mx == file_fb[i] { same += 1; }
            *hist.entry(mx as i32 - file_fb[i] as i32).or_default() += 1;
        }
        println!("{name}: {same} of {} charts match; histogram {:?}", mp.count, hist);
    }
    Ok(())
}

/// `--probes`: the four probe images from the end-state volumes (`probepass::download_probes`) laid out as the
/// stored 21×21 atlas, through libwebp at several qualities, against the four WEBPs of frame 0 image 2.
pub fn check_probes(root: &std::path::Path, map: &str, frame: u32) -> Result<(), String> {
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
    let m = crate::passdiff::read_manifest(&txt)?;
    let last = |pass: &str| -> Result<crate::probepass::Volume3, String> {
        let e = m.passes.iter().filter(|e| e.pass == pass && e.frame == Some(frame)).max_by_key(|e| e.eid.unwrap_or(0)).ok_or_else(|| format!("no {pass} entry for frame {frame}"))?;
        crate::probecheck::load_volume(root, e)
    };
    let colour = last("probe3d_fold0")?;
    let updown = last("probe3d_fold1")?;
    let skyvis = last("probe3d_skyvis")?;
    let mm = crate::mapio::load(map)?;
    let d = mm.chunk.data.as_ref().ok_or("no lightmap data")?;
    let v = crate::volume::Volume::parse(&d.cache.trailer)?;
    let b = v.blocks.first().ok_or("no probe block")?;
    let parts = crate::volume::split_probe_blob(&d.frames[0].images[2], &v.frame_info);
    println!("probe blob: {} parts of {:?} bytes; trailer scales {:?}", parts.len(), parts.iter().map(|p| p.len()).collect::<Vec<_>>(), v.frame_info);
    let dl = crate::probepass::download_probes(&colour, &updown, Some(&skyvis), (b.min, b.max));
    println!("max0 {} (trailer {}), max2 {} (trailer {})", dl.max0, v.frame_info[0].0, dl.max2, v.frame_info[1].0);
    let dec: Vec<crate::img::Rgb> = parts.iter().map(|p| crate::img::decode_webp(p)).collect::<Result<_, _>>()?;
    let (aw, ah) = (dec[0].w, dec[0].h);
    let tile = |level: u32| -> Option<(u32, u32)> { b.slices.get((level - b.min[1]) as usize).copied().flatten() };
    // the four atlases: 0 colour rgb, 1 sky (grey: same byte in r, g, b), 2 signed sqrt + 128, 3 zeros (local lights)
    // the atlas pixels no tile covers hold 128 in the file (the unused 3×3 cell decodes to 128/130/127/128)
    let mut imgs = vec![vec![128u8; (aw * ah * 3) as usize]; 4];
    // sky-visibility curve candidates for image 1
    let curves: Vec<(&str, Box<dyn Fn(f32) -> u8>)> = vec![
        ("linear round", Box::new(|v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8)),
        ("linear trunc", Box::new(|v: f32| (v * 255.0).clamp(0.0, 255.0) as u8)),
        ("sRGB round", Box::new(|v: f32| (crate::gpufmt::linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0).round().clamp(0.0, 255.0) as u8)),
        ("sRGB trunc", Box::new(|v: f32| (crate::gpufmt::linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0).clamp(0.0, 255.0) as u8)),
    ];
    let mut sky_imgs: Vec<Vec<u8>> = vec![vec![128u8; (aw * ah * 3) as usize]; curves.len()];
    for ((x, y, z), rgb, ok, _sky, sq) in &dl.probes {
        let Some((tx, ty)) = tile(*y) else { continue };
        let (px, py) = (tx + (x - b.min[0]), ty + (z - b.min[2]));
        let o = ((py * aw + px) * 3) as usize;
        if !*ok { continue; }
        imgs[0][o..o + 3].copy_from_slice(rgb);
        for c in 0..3 { imgs[2][o + c] = (sq[c] as i32 + 128) as u8; }
        // image 3 = the local-light pass (none in this bake): 0 inside the tiles
        imgs[3][o..o + 3].copy_from_slice(&[0, 0, 0]);
        let sv = skyvis.get(*x, *y, *z, 0);
        for (ci, (_, f)) in curves.iter().enumerate() { let bb = f(sv); sky_imgs[ci][o] = bb; sky_imgs[ci][o + 1] = bb; sky_imgs[ci][o + 2] = bb; }
    }
    // the sky-visibility curve search: Y-only at q80 (the header of the stored WEBP matches that quality)
    {
        let mut hits = 0;
        let mut best: Option<(String, usize)> = None;
        for gi in 0..=60u32 {
            let gamma = 0.30 + gi as f32 * 0.03;
            for mode in 0..40u32 {
                let mut grey = vec![128u8; (aw * ah) as usize];
                for ((x, y, z), _rgb, ok, _sky, _sq) in &dl.probes {
                    let Some((tx, ty)) = tile(*y) else { continue };
                    if !*ok { continue; }
                    let (px, py) = (tx + (x - b.min[0]), ty + (z - b.min[2]));
                    let v = skyvis.get(*x, *y, *z, 0).clamp(0.0, 1.0);
                    // modes 4..39: a = (mode − 4) · 0.025 (the value mapped to 0), f = ((v − a) / (1 − a))^gamma, rounded (even) / truncated (odd)
                    let f = match mode { 0 => v.powf(gamma), 1 => crate::gpufmt::linear_to_srgb(v).powf(gamma), 2 => (v / 0.998).min(1.0).powf(gamma), 3 => v.powf(gamma), m => { let a = ((m - 4) / 2) as f32 * 0.025; ((v - a) / (1.0 - a)).clamp(0.0, 1.0).powf(gamma) } };
                    let bb = if mode == 3 || (mode >= 4 && mode % 2 == 1) { (f * 255.0) as i32 } else { (f * 255.0).round() as i32 };
                    grey[(py * aw + px) as usize] = bb.clamp(0, 255) as u8;
                }
                if let Some(ours) = crate::webpenc::encode_grey(&grey, aw, ah, 80.0) {
                    let common = ours.iter().zip(parts[1].iter()).take_while(|(a, bb)| a == bb).count();
                    if ours == parts[1] { println!("  SKY CURVE HIT: mode {mode} gamma {gamma:.2}: BYTE-IDENTICAL"); hits += 1; }
                    if best.as_ref().map_or(true, |(_, c)| common > *c) { best = Some((format!("mode {mode} gamma {gamma:.2} ({} bytes)", ours.len()), common)); }
                }
            }
        }
        println!("  sky curve search: {hits} exact hits; longest common prefix {:?}", best);
    }
    // image 0 / image 2 byte-rule variants (round vs truncate, the sRGB curve vs a 4096-entry table), Y q91
    {
        let mut variants0: Vec<(String, Vec<u8>)> = Vec::new();
        let mut variants2: Vec<(String, Vec<u8>)> = Vec::new();
        let inv0 = 1.0f32 / dl.max0;
        let inv2 = 1.0f32 / dl.max2;
        for rule in 0..24u32 {
            let mut im0 = vec![128u8; (aw * ah * 3) as usize];
            let mut im2 = vec![128u8; (aw * ah * 3) as usize];
            // rules 12..23 = rules 0..11 with the division done as a multiplication by the reciprocal (f32)
            let recip = rule >= 12;
            let rule = rule % 12;
            for ((x, y, z), _rgb, ok, _sky, _sq) in &dl.probes {
                let Some((tx, ty)) = tile(*y) else { continue };
                if !*ok { continue; }
                let (px, py) = (tx + (x - b.min[0]), ty + (z - b.min[2]));
                let o = ((py * aw + px) * 3) as usize;
                for c in 0..3 {
                    let raw = colour.get(*x, *y, *z, c as u32);
                    let v = (if recip { raw * inv0 } else { raw / dl.max0 }).clamp(0.0, 1.0);
                    let sv = crate::gpufmt::linear_to_srgb(v);
                    im0[o + c] = match rule { 0 => (sv * 255.0).round(), 1 => (sv * 255.0).floor(), 2 => (sv * 255.0 + 0.5).floor(), 3 => { let q = (v * 4095.0) as u32; (crate::gpufmt::linear_to_srgb(q as f32 / 4095.0) * 255.0).round() }, 4 => (sv * 255.0).ceil(), 5 => ((sv * 255.0) as i32) as f32,
                        6 => (v.powf(1.0 / 2.2) * 255.0).round(), 7 => (v.powf(1.0 / 2.2) * 255.0).floor(), 8 => (v.powf(1.0 / 2.4) * 255.0).round(), 9 => (v.sqrt() * 255.0).round(), 10 => (v.sqrt() * 255.0).floor(), _ => (v * 255.0).round() }.clamp(0.0, 255.0) as u8;
                    let uraw = updown.get(*x, *y, *z, c as u32);
                    let u = if recip { uraw * inv2 } else { uraw / dl.max2 };
                    let sgn = if u < 0.0 { -1.0 } else { 1.0 };
                    let r = u.abs().sqrt() * sgn;
                    im2[o + c] = match rule { 0 => (r * 127.0).round() + 128.0, 1 => (r * 127.0) as i32 as f32 + 128.0, 2 => (r * 127.5 + 127.5).round(), 3 => (r * 127.5 + 127.5).floor(), 4 => (r * 127.0 + 128.0).floor(), _ => (r * 128.0 + 128.0).round() }.clamp(0.0, 255.0) as u8;
                }
            }
            variants0.push((format!("rule {rule}{}", if recip { " ×1/max" } else { "" }), im0));
            variants2.push((format!("rule {rule}{}", if recip { " ×1/max" } else { "" }), im2));
        }
        // where does the best-size variant of image 0 (rule 1: floor) sit off the decoded by more than 3?
        if let Some((_, im)) = variants0.get(1) {
            let mut n = 0;
            for ((x, y, z), _rgb, ok, _sky, _sq) in &dl.probes {
                let Some((tx, ty)) = tile(*y) else { continue };
                if !*ok { continue; }
                let (px, py) = (tx + (x - b.min[0]), ty + (z - b.min[2]));
                let o = ((py * aw + px) * 3) as usize;
                let d: Vec<i32> = (0..3).map(|c| im[o + c] as i32 - dec[0].px[o + c] as i32).collect();
                if d.iter().any(|v| v.abs() > 3) && n < 12 {
                    println!("  image 0 probe ({x},{y},{z}) → ({px},{py}): ours {:?} decoded {:?} colour {:?} α {:.4}", &im[o..o + 3], &dec[0].px[o..o + 3], [colour.get(*x, *y, *z, 0), colour.get(*x, *y, *z, 1), colour.get(*x, *y, *z, 2)], colour.get(*x, *y, *z, 3));
                    n += 1;
                }
            }
        }
        for (k, vs) in [(0usize, &variants0), (2, &variants2)] {
            for (name, im) in vs {
                let (mut ex, mut w2) = (0usize, 0usize);
                for (a, bb) in im.iter().zip(dec[k].px.iter()) { let dd = a.abs_diff(*bb); if dd == 0 { ex += 1; } if dd <= 2 { w2 += 1; } }
                let mut line = format!("  image {k} {name}: {ex} exact / {w2} within 2 of {} vs decoded;", im.len());
                for q in [91.0f32, 90.0, 92.0, 100.0, 80.0] {
                    if let Some(ours) = crate::webpenc::encode_rgb(im, aw, ah, q) { if ours == parts[k] { line.push_str(&format!(" q {q}: BYTE-IDENTICAL")); } else if q == 91.0 { let common = ours.iter().zip(parts[k].iter()).take_while(|(a, bb)| a == bb).count(); line.push_str(&format!(" q91: {} bytes vs {}, common prefix {common}", ours.len(), parts[k].len())); } }
                }
                println!("{line}");
            }
        }
    }
    // stored vs ours per image, then the encodes
    for (k, name) in [(0usize, "image 0 colour"), (2, "image 2 signed sqrt +128"), (3, "image 3 local lights (zeros)")] {
        let (mut ex, mut w2) = (0usize, 0usize);
        for (a, bb) in imgs[k].iter().zip(dec[k].px.iter()) { let dd = a.abs_diff(*bb); if dd == 0 { ex += 1; } if dd <= 2 { w2 += 1; } }
        println!("{name}: bytes vs decoded: {ex} exact, {w2} within 2 of {}", imgs[k].len());
        for q in [91.0f32, 100.0, 90.0, 80.0, 75.0, 50.0, 30.0] {
            if let Some(ours) = crate::webpenc::encode_rgb(&imgs[k], aw, ah, q) {
                let r = cmp_bytes(&parts[k], &ours);
                if r.starts_with("BYTE") || q == 91.0 { println!("  RGB import q {q}: {r}"); }
            }
        }
    }
    for (ci, (cname, _)) in curves.iter().enumerate() {
        let (mut ex, mut w2) = (0usize, 0usize);
        for (a, bb) in sky_imgs[ci].iter().zip(dec[1].px.iter()) { let dd = a.abs_diff(*bb); if dd == 0 { ex += 1; } if dd <= 2 { w2 += 1; } }
        println!("image 1 sky visibility [{cname}]: bytes vs decoded: {ex} exact, {w2} within 2 of {}", sky_imgs[ci].len());
        let grey: Vec<u8> = sky_imgs[ci].chunks(3).map(|c| c[0]).collect();
        let hdr = |v: &[u8]| -> String { v.iter().position(|&x| x == b'V').map(|p| v[p + 18..(p + 24).min(v.len())].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")).unwrap_or_default() };
        let want = hdr(&parts[1]);
        for q10 in 500..=1000u32 {
            let q = q10 as f32 / 10.0;
            if let Some(ours) = crate::webpenc::encode_rgb(&sky_imgs[ci], aw, ah, q) { let r = cmp_bytes(&parts[1], &ours); if r.starts_with("BYTE") || hdr(&ours) == want { println!("  RGB import q {q}: {r}"); } }
            if let Some(ours) = crate::webpenc::encode_grey(&grey, aw, ah, q) { let r = cmp_bytes(&parts[1], &ours); if r.starts_with("BYTE") || hdr(&ours) == want { println!("  Y-only q {q}: {r}"); } }
        }
    }
    Ok(())
}

/// The frame record's scale fields from the captured MaxHdr buffer (client 0x14022be30 l.~430 / 0x14022b370
/// end): `MaxHDR = f32(max[0] · 0.39894226)` (κ = 1/√(2π)), and the three f32 at the record's end (the docs'
/// "LAmbient") = `f32(max[k] · 0.6909883)` = √3·κ·max[k] for k = 1..3 — after the CPU's clip
/// `s = 1 / max(1, max[0] / (Mood_MaxHdr · 2.5066283))` applied to all four when `s < 1`.
pub fn record_scales(maxhdr: [f32; 4], mood_max_hdr: f32) -> (f32, [f32; 3]) {
    let r = maxhdr[0] / (mood_max_hdr * 2.5066283f32);
    let s = 1.0f32 / r.max(1.0);
    let m = if s < 1.0 { [maxhdr[0] * s, maxhdr[1] * s, maxhdr[2] * s, maxhdr[3] * s] } else { maxhdr };
    (m[0] * 0.39894226f32, [m[1] * 0.6909883f32, m[2] * 0.6909883f32, m[3] * 0.6909883f32])
}

/// `--records`: the frame-0 record's f32 scale fields vs the file, from the captured MaxHdr buffer.
pub fn check_records(root: &std::path::Path, map: &str, frame: u32) -> Result<(), String> {
    let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
    let m = crate::passdiff::read_manifest(&txt)?;
    let e = m.passes.iter().find(|e| e.pass == "final_05_maxreduce_buffer" && e.frame == Some(frame)).ok_or("no MaxHdr buffer entry")?;
    let bytes = crate::passdiff::read_entry_bytes(root, &e.file)?;
    if bytes.len() < 16 { return Err("MaxHdr buffer shorter than 16 bytes".into()); }
    let mx: [f32; 4] = [0, 4, 8, 12].map(|o| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]));
    let mm = crate::mapio::load(map)?;
    let d = mm.chunk.data.as_ref().ok_or("no lightmap data")?;
    let mp = d.cache.mapping().ok_or("no mapping")?;
    // record 0 starts 12 bytes before the −FLT_MAX word
    let head = &mp.head;
    let pos = head.windows(4).position(|w| w == [0xff, 0xff, 0x7f, 0xff]).ok_or("no −FLT_MAX in the mapping head")?;
    let r = &head[pos - 12..pos - 12 + 66];
    let f = |o: usize| f32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]);
    let (mood, file_max, file_tri) = (f(16), f(20), [f(54), f(58), f(62)]);
    let (ours_max, ours_tri) = record_scales(mx, mood);
    println!("MaxHdr buffer (GPU): {mx:?}; record MaxHDR_Mood {mood}");
    println!("MaxHDR: file {file_max} ours {ours_max} — {}", if file_max.to_bits() == ours_max.to_bits() { "BIT-IDENTICAL" } else { "DIFFERENT" });
    for k in 0..3 { println!("√3·κ·max[{}]: file {} ours {} — {}", k + 1, file_tri[k], ours_tri[k], if file_tri[k].to_bits() == ours_tri[k].to_bits() { "BIT-IDENTICAL" } else { "DIFFERENT" }); }
    let h3: Vec<u16> = (0..3).map(|k| u16::from_le_bytes([r[36 + 2 * k], r[37 + 2 * k]])).collect();
    println!("MaxHDR_HBasisScaled234 (f16, origin open): {:?} = {:?}", h3, h3.iter().map(|&h| crate::gpufmt::decode_f16(h)).collect::<Vec<_>>());
    Ok(())
}
