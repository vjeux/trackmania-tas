//! `lmtool classcmp OURS.Map.Gbx --against EDITOR.Map.Gbx [--records TSV] [--by class|name|obj] [--frame 0] [--lit 8]
//! [--tsv OUT.tsv] [--worst N] [--own-rects]` — two WRITTEN maps side by side, per CLASS of chart: the decoded HDR texels
//! (HDR = (A/255)² · (fb/255)² · the frame record's MaxHDR, `synth::decode_value` × the record) averaged over the
//! oracle's LIT texels (image-A max channel ≥ `--lit`, the `charts` convention), ours / editor per channel, the lit
//! fractions, the byte identity of the stored image over the class's texels (identical / within 1 / within 2 / max |Δ|)
//! and the RMSE of the HDR difference relative to the oracle's mean — the per-class table the verification rows report.
//!
//! The class of a chart comes from the bake's records table (`lmtool bake … --records-tsv FILE`: chart index, class,
//! obj, sub, name, quality, centre y — the layout's `records::Rec` per chart), matched by (obj, sub) of the mapping's
//! bind word (the bind words are the editor's when the layout gate passes, so one table serves both files); without a
//! table every chart is one class, split by the object-id range (authored blocks number from 16384).

use crate::format::Mapping;

/// One row of a `--records-tsv` table.
#[derive(Clone, Debug)]
pub struct RecRow {
    pub chart: usize,
    pub class: String,
    pub obj: u32,
    pub sub: u32,
    pub name: String,
    pub quality: f32,
    pub centre_y: f32,
    /// centre x / z when the table carries them (the 9-column form)
    pub centre_x: Option<f32>,
    pub centre_z: Option<f32>,
}

/// Parse the table (header line optional; tab-separated: chart, class, obj, sub, name, quality, centre_y).
pub fn read_records_tsv(path: &str) -> Result<Vec<RecRow>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = Vec::new();
    for (ln, line) in txt.lines().enumerate() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 { continue; }
        let Ok(chart) = f[0].trim().parse::<usize>() else { if ln == 0 { continue } else { return Err(format!("{path}:{}: bad chart index {:?}", ln + 1, f[0])) } };
        out.push(RecRow {
            chart,
            class: f[1].trim().to_string(),
            obj: f[2].trim().parse().map_err(|e| format!("{path}:{}: obj: {e}", ln + 1))?,
            sub: f[3].trim().parse().map_err(|e| format!("{path}:{}: sub: {e}", ln + 1))?,
            name: f[4].trim().to_string(),
            quality: f.get(5).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0),
            centre_y: f.get(6).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0),
            centre_x: f.get(7).and_then(|v| v.trim().parse().ok()),
            centre_z: f.get(8).and_then(|v| v.trim().parse().ok()),
        });
    }
    Ok(out)
}

/// The frame record's MaxHDR (head: 3 × 66 bytes from offset 60; +20 = MaxHDR).
/// THE STORED TEXEL'S HDR VALUE (E, 2026-09-27 02:00Z): frame 0's colour image is the sqrt encode through the YCbCr writer — HDR =
/// (t/255)²·(fb/255)²·MaxHDR; FRAME 1's image is sRGB-ENCODED (the frame-1 writer's target): the editor's stpad night bytes are
/// 255·srgb_encode(v/M) to ±1 from byte 44 to 249 against our linear D_0 (a 20-bin curve over 220 000 texels, class-independent —
/// `lmtool f1curve`), where the square-root form is 8 bytes low mid-range. The chart byte rescales the ENCODED value (t = 255·c/c_max),
/// so the decode is srgb_decode((t/255)·(fb/255))·MaxHDR. LMTOOL_F1_DECODE=sqrt = the old square decode for frame 1 (study).
pub fn texel_hdr(frame: usize, t: u8, fb: u8, max_hdr: f32) -> f64 {
    static SQRT: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("LMTOOL_F1_DECODE").as_deref() == Ok("sqrt"));
    let c = (t as f64 / 255.0) * (fb as f64 / 255.0);
    if frame == 1 && !*SQRT { srgb_decode(c) * max_hdr as f64 } else { c * c * max_hdr as f64 }
}

/// sRGB → linear (the IEC 61966-2-1 transfer, the toe included).
pub fn srgb_decode(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// linear → sRGB.
pub fn srgb_encode(v: f64) -> f64 {
    if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

pub fn record_maxhdr(m: &Mapping, frame: usize) -> Option<f32> {
    let r = 60 + 66 * frame;
    if r + 24 > m.head.len() { return None; }
    Some(f32::from_le_bytes(m.head[r + 20..r + 24].try_into().unwrap()))
}

/// The frame record's MaxHdrMood (+16).
pub fn record_maxhdr_mood(m: &Mapping, frame: usize) -> Option<f32> {
    let r = 60 + 66 * frame;
    if r + 20 > m.head.len() { return None; }
    Some(f32::from_le_bytes(m.head[r + 16..r + 20].try_into().unwrap()))
}

/// The chart's own pixels in the stored (half-resolution) image: `((x + 1) / 2, (y + 1) / 2, w / 2, h / 2)`.
pub fn chart_own_px(pos: (u16, u16), size: (u16, u16)) -> (u32, u32, u32, u32) {
    ((pos.0 as u32 + 1) / 2, (pos.1 as u32 + 1) / 2, size.0 as u32 / 2, size.1 as u32 / 2)
}

#[derive(Clone, Debug, Default)]
pub struct ClassAcc {
    pub charts: usize,
    /// every texel of the class's rects (inside both images)
    pub texels: usize,
    pub lit_ours: usize,
    pub lit_theirs: usize,
    /// the texels our mean runs over (the oracle's lit texels; in --own-rects mode OUR lit texels over our rect)
    pub used: usize,
    /// the texels the oracle's mean runs over (= `used` unless --own-rects)
    pub used_t: usize,
    pub sum_ours: [f64; 3],
    pub sum_theirs: [f64; 3],
    pub sum_sq: [f64; 3],
    /// stored-byte identity over every channel of every texel of the class
    pub bytes: usize,
    pub exact: usize,
    pub within1: usize,
    pub within2: usize,
    pub max_delta: u8,
    /// per-chart: (chart, ratio of the per-chart HDR means (max channel deviation from 1), used texels)
    pub worst: Vec<(usize, f64, [f64; 3], [f64; 3], usize)>,
    /// The per-chart ratio spread (charts with ≥ 16 used texels): (n, median of the per-chart ratio of the channel-summed means,
    /// σ of ln(ratio)) — a global factor (a wrong SkyFactor, a MaxHDR split) leaves σ untouched, a wrong sun direction or a
    /// per-object defect widens it.
    pub spread: Option<(usize, f64, f64)>,
}

impl ClassAcc {
    pub fn mean_ours(&self) -> [f64; 3] { let n = self.used.max(1) as f64; [self.sum_ours[0] / n, self.sum_ours[1] / n, self.sum_ours[2] / n] }
    pub fn mean_theirs(&self) -> [f64; 3] { let n = self.used_t.max(1) as f64; [self.sum_theirs[0] / n, self.sum_theirs[1] / n, self.sum_theirs[2] / n] }
    pub fn ratio(&self) -> [f64; 3] { let (a, b) = (self.mean_ours(), self.mean_theirs()); let r = |x: f64, y: f64| if y > 1e-6 && self.used_t >= 16 { x / y } else { f64::NAN }; [r(a[0], b[0]), r(a[1], b[1]), r(a[2], b[2])] }
    /// RMSE of the HDR difference over the used texels, relative to the oracle's mean (per channel).
    pub fn rmse_rel(&self) -> [f64; 3] { let n = self.used.max(1) as f64; let b = self.mean_theirs(); [(self.sum_sq[0] / n).sqrt() / b[0].max(1e-12), (self.sum_sq[1] / n).sqrt() / b[1].max(1e-12), (self.sum_sq[2] / n).sqrt() / b[2].max(1e-12)] }
    pub fn merge(&mut self, o: &ClassAcc) {
        self.charts += o.charts; self.texels += o.texels; self.lit_ours += o.lit_ours; self.lit_theirs += o.lit_theirs; self.used += o.used; self.used_t += o.used_t;
        for c in 0..3 { self.sum_ours[c] += o.sum_ours[c]; self.sum_theirs[c] += o.sum_theirs[c]; self.sum_sq[c] += o.sum_sq[c]; }
        self.bytes += o.bytes; self.exact += o.exact; self.within1 += o.within1; self.within2 += o.within2; self.max_delta = self.max_delta.max(o.max_delta);
        self.worst.extend(o.worst.iter().cloned());
    }
}

/// How charts are grouped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GroupBy { Class, Name, Obj }

pub struct Options {
    pub frame: usize,
    pub lit: u8,
    /// `--lit-hdr F`: a texel is LIT when its decoded HDR max channel ≥ F (instead of the stored byte ≥ `lit`) — the byte test counts a
    /// dark chart's near-black texels (1e-4 HDR under a platform) as lit and a black texel of ours as unlit, which moved tiny03's tile lit
    /// fraction 76 % vs 91 % (G2, 2026-09-27); with an HDR floor both sides are judged on the same quantity
    pub lit_hdr: Option<f64>,
    pub by: GroupBy,
    pub worst: usize,
    /// Each file's class means over ITS OWN rects, charts matched by the (obj, sub) bind word — for two layouts that
    /// differ (a q2 / q5 bake against the q3 / q4 oracle); no byte identity, no RMSE.
    pub own_rects: bool,
}

/// The class key of chart `i`.
pub fn class_key(m: &Mapping, i: usize, rows: &std::collections::HashMap<(u32, u32), RecRow>, by_index: Option<&[RecRow]>, by: GroupBy) -> String {
    let obj = m.binds[i].obj_group_idx / 4;
    let sub = m.binds[i].obj_idx & 0x00ff_ffff;
    let row = rows.get(&(obj, sub)).or_else(|| by_index.and_then(|r| r.get(i)).filter(|r| r.chart == i));
    match (row, by) {
        (Some(r), GroupBy::Class) => { if r.class.starts_with("clip") { "clip".to_string() } else { r.class.clone() } }
        (Some(r), GroupBy::Name) => format!("{}:{}", if r.class.starts_with("clip") { "clip" } else { r.class.as_str() }, r.name),
        (Some(_), GroupBy::Obj) | (None, GroupBy::Obj) => format!("obj {obj}"),
        (None, _) => if obj >= 16384 { "obj≥16384".to_string() } else { "obj<16384".to_string() },
    }
}

pub struct Report {
    pub classes: Vec<(String, ClassAcc)>,
    pub total: ClassAcc,
    pub maxhdr_ours: f32,
    pub maxhdr_theirs: f32,
    /// Per file: (the decoded peak over every chart texel, texels whose image-A max channel is 255, charts whose frame byte is 255,
    /// the MaxHdrMood record word) — the encode-scale check: a file whose peak sits at the record with saturated texels was clipped
    pub peak_ours: (f64, usize, usize, Option<f32>),
    pub peak_theirs: (f64, usize, usize, Option<f32>),
    /// (mapping head bytes, frame records present, image frames) per file — the editor writes 204 B / 2 records / 2 frames
    pub head_ours: (usize, usize, usize),
    pub head_theirs: (usize, usize, usize),
    pub image_w: u32,
    pub image_h: u32,
    pub unmatched_rows: usize,
    pub rect_mismatch: usize,
    /// --own-rects pairs refused by the rect-area guard (a numbering mismatch between the two files)
    pub pair_refused: usize,
}

/// The whole comparison: both maps' frame `frame` image 0 decoded, per class.
pub fn compare(ours: &crate::mapio::MapLightmap, theirs: &crate::mapio::MapLightmap, records: Option<&[RecRow]>, o: &Options) -> Result<Report, String> {
    let (Some(d1), Some(d2)) = (ours.chunk.data.as_ref(), theirs.chunk.data.as_ref()) else { return Err("a map without a lightmap".into()) };
    let (Some(m1), Some(m2)) = (d1.cache.mapping(), d2.cache.mapping()) else { return Err("a map without a mapping chunk".into()) };
    if m1.count != m2.count && !o.own_rects { return Err(format!("chart counts differ: ours {} vs theirs {} — the layout gate first (or --own-rects)", m1.count, m2.count)); }
    let n = m1.count as usize;
    // --own-rects: the oracle's chart of the same (obj, sub) bind word
    let theirs_of: std::collections::HashMap<(u32, u32), usize> = (0..m2.count as usize).map(|j| ((m2.binds[j].obj_group_idx / 4, m2.binds[j].obj_idx & 0x00ff_ffff), j)).collect();
    let f = o.frame;
    let (Some(f1), Some(f2)) = (d1.frames.get(f), d2.frames.get(f)) else { return Err(format!("frame {f}: not in both files")) };
    let (Some(b1), Some(b2)) = (f1.images.first(), f2.images.first()) else { return Err(format!("frame {f}: no image 0")) };
    let i1 = crate::img::decode_webp(b1).map_err(|e| format!("ours frame {f} image 0: {e}"))?;
    let i2 = crate::img::decode_webp(b2).map_err(|e| format!("theirs frame {f} image 0: {e}"))?;
    if i1.w != i2.w || i1.h != i2.h { return Err(format!("image sizes differ: {}×{} vs {}×{}", i1.w, i1.h, i2.w, i2.h)); }
    let k1 = record_maxhdr(&m1, f).ok_or("ours: no frame record")?;
    let k2 = record_maxhdr(&m2, f).ok_or("theirs: no frame record")?;
    let fb1 = m1.frame_bytes.get(f).ok_or("ours: no frame bytes")?;
    let fb2 = m2.frame_bytes.get(f).ok_or("theirs: no frame bytes")?;
    // the records table by (obj, sub)
    let mut rows: std::collections::HashMap<(u32, u32), RecRow> = Default::default();
    let mut unmatched_rows = 0usize;
    if let Some(rs) = records {
        for r in rs { rows.insert((r.obj, r.sub), r.clone()); }
        // rows whose (obj, sub) no chart carries
        let keys: std::collections::HashSet<(u32, u32)> = (0..n).map(|i| (m1.binds[i].obj_group_idx / 4, m1.binds[i].obj_idx & 0x00ff_ffff)).collect();
        unmatched_rows = rs.iter().filter(|r| !keys.contains(&(r.obj, r.sub))).count();
    }
    let mut classes: Vec<(String, ClassAcc)> = Vec::new();
    let mut total = ClassAcc::default();
    let mut rect_mismatch = 0usize;
    let mut pair_refused = 0usize;
    for i in 0..n {
        if o.own_rects {
            // each side over its own rect; the oracle's chart by bind word
            let key_bind = (m1.binds[i].obj_group_idx / 4, m1.binds[i].obj_idx & 0x00ff_ffff);
            let Some(&j) = theirs_of.get(&key_bind) else { rect_mismatch += 1; continue };
            // the pairing guard: the same object at (almost) the same texel density has (almost) the same rect area — a pair whose
            // areas differ by more than a √2-ring step with slack (×2.9) is a NUMBERING mismatch (tiny03: our kind-0 trees at obj 4096… vs the editor's road
            // items), refused and counted rather than averaged
            { let (a1, a2) = (m1.size[i].0 as f64 * m1.size[i].1 as f64, m2.size[j].0 as f64 * m2.size[j].1 as f64); if a1 > 0.0 && a2 > 0.0 && (a1 / a2 > 2.9 || a2 / a1 > 2.9) { pair_refused += 1; continue; } }
            let key = class_key(&m1, i, &rows, records, o.by);
            let mut acc = ClassAcc { charts: 1, ..Default::default() };
            let (fbi, fbj) = (fb1.get(i).copied().unwrap_or(0), fb2.get(j).copied().unwrap_or(0));
            let (mut co, mut ct) = ([0f64; 3], [0f64; 3]);
            let (px, py, pw, ph) = chart_own_px(m1.pos[i], m1.size[i]);
            for y in py..(py + ph).min(i1.h) { for x in px..(px + pw).min(i1.w) {
                let a = i1.get(x, y);
                acc.texels += 1;
                if is_lit(o, a, fbi, k1) { acc.lit_ours += 1; acc.used += 1; for c in 0..3 { let ho = texel_hdr(o.frame, a[c], fbi, k1); acc.sum_ours[c] += ho; co[c] += ho; } }
            } }
            let (qx, qy, qw, qh) = chart_own_px(m2.pos[j], m2.size[j]);
            let mut tex_t = 0usize;
            for y in qy..(qy + qh).min(i2.h) { for x in qx..(qx + qw).min(i2.w) {
                let b = i2.get(x, y);
                tex_t += 1;
                if is_lit(o, b, fbj, k2) { acc.lit_theirs += 1; acc.used_t += 1; for c in 0..3 { let ht = texel_hdr(o.frame, b[c], fbj, k2); acc.sum_theirs[c] += ht; ct[c] += ht; } }
            } }
            // lit % of the oracle side is over ITS texel count: scale lit_theirs onto our texel count for the shared column
            if tex_t > 0 && acc.texels > 0 { acc.lit_theirs = (acc.lit_theirs as f64 * acc.texels as f64 / tex_t as f64).round() as usize; }
            if acc.used > 0 && acc.used_t > 0 {
                let (u, ut) = (acc.used as f64, acc.used_t as f64);
                let (mo, mt) = ([co[0] / u, co[1] / u, co[2] / u], [ct[0] / ut, ct[1] / ut, ct[2] / ut]);
                let dev = (0..3).map(|c| if mt[c] > 1e-9 { (mo[c] / mt[c] - 1.0).abs() } else { 0.0 }).fold(0.0, f64::max);
                acc.worst.push((i, dev, mo, mt, acc.used));
            }
            total.merge(&acc);
            match classes.iter_mut().find(|(k, _)| *k == key) { Some((_, c)) => c.merge(&acc), None => classes.push((key, acc)) }
            continue;
        }
        if m1.pos[i] != m2.pos[i] || m1.size[i] != m2.size[i] { rect_mismatch += 1; continue; }
        let key = class_key(&m1, i, &rows, records, o.by);
        let (px, py, pw, ph) = chart_own_px(m1.pos[i], m1.size[i]);
        let mut acc = ClassAcc { charts: 1, ..Default::default() };
        let (fbi, fbj) = (fb1.get(i).copied().unwrap_or(0), fb2.get(i).copied().unwrap_or(0));
        let (mut co, mut ct) = ([0f64; 3], [0f64; 3]);
        for y in py..(py + ph).min(i1.h) {
            for x in px..(px + pw).min(i1.w) {
                let a = i1.get(x, y);
                let b = i2.get(x, y);
                acc.texels += 1;
                let lo = is_lit(o, a, fbi, k1);
                let lt = is_lit(o, b, fbj, k2);
                if lo { acc.lit_ours += 1; }
                if lt { acc.lit_theirs += 1; }
                for c in 0..3 {
                    let d = a[c].abs_diff(b[c]);
                    acc.bytes += 1;
                    if d == 0 { acc.exact += 1; }
                    if d <= 1 { acc.within1 += 1; }
                    if d <= 2 { acc.within2 += 1; }
                    acc.max_delta = acc.max_delta.max(d);
                }
                if lt {
                    acc.used += 1; acc.used_t += 1;
                    for c in 0..3 {
                        let ho = texel_hdr(o.frame, a[c], fbi, k1);
                        let ht = texel_hdr(o.frame, b[c], fbj, k2);
                        acc.sum_ours[c] += ho; acc.sum_theirs[c] += ht; acc.sum_sq[c] += (ho - ht) * (ho - ht);
                        co[c] += ho; ct[c] += ht;
                    }
                }
            }
        }
        if acc.used > 0 {
            let u = acc.used as f64;
            let (mo, mt) = ([co[0] / u, co[1] / u, co[2] / u], [ct[0] / u, ct[1] / u, ct[2] / u]);
            let dev = (0..3).map(|c| if mt[c] > 1e-9 { (mo[c] / mt[c] - 1.0).abs() } else { 0.0 }).fold(0.0, f64::max);
            acc.worst.push((i, dev, mo, mt, acc.used));
        }
        total.merge(&acc);
        match classes.iter_mut().find(|(k, _)| *k == key) { Some((_, c)) => c.merge(&acc), None => classes.push((key, acc)) }
    }
    let spread_of = |c: &ClassAcc| -> Option<(usize, f64, f64)> {
        let mut ls: Vec<f64> = c.worst.iter().filter(|w| w.4 >= 16).map(|w| { let (a, b) = (w.2[0] + w.2[1] + w.2[2], w.3[0] + w.3[1] + w.3[2]); if a > 1e-9 && b > 1e-9 { (a / b).ln() } else { f64::NAN } }).filter(|v| v.is_finite()).collect();
        if ls.len() < 2 { return None; }
        ls.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = ls.len();
        let median = ls[n / 2].exp();
        let mean = ls.iter().sum::<f64>() / n as f64;
        let sigma = (ls.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1) as f64).sqrt();
        Some((n, median, sigma))
    };
    for (_, c) in classes.iter_mut() { c.spread = spread_of(c); }
    total.spread = spread_of(&total);
    let trim = |c: &mut ClassAcc| { c.worst.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)); c.worst.truncate(o.worst); };
    for (_, c) in classes.iter_mut() { trim(c); }
    trim(&mut total);
    let peak = |img: &crate::img::Rgb, m: &Mapping, fb: &[u8], k: f32| -> (f64, usize, usize, Option<f32>) {
        let (mut pk, mut sat, mut fb255) = (0f64, 0usize, 0usize);
        for i in 0..m.count as usize {
            let f = fb.get(i).copied().unwrap_or(0);
            if f == 255 { fb255 += 1; }
            let s = (f as f64 / 255.0).powi(2) * k as f64;
            let (px, py, pw, ph) = chart_own_px(m.pos[i], m.size[i]);
            for y in py..(py + ph).min(img.h) { for x in px..(px + pw).min(img.w) {
                let a = img.get(x, y);
                let mx = a[0].max(a[1]).max(a[2]);
                if mx == 255 { sat += 1; }
                pk = pk.max((mx as f64 / 255.0).powi(2) * s);
            } }
        }
        (pk, sat, fb255, record_maxhdr_mood(m, o.frame))
    };
    let peak_ours = peak(&i1, &m1, fb1, k1);
    let peak_theirs = peak(&i2, &m2, fb2, k2);
    let head_of = |m: &Mapping, d: &crate::format::LightmapData| (m.head.len(), (0..3).filter(|&k| 60 + 66 * k + 66 <= m.head.len()).count(), d.frames.len());
    let (head_ours, head_theirs) = (head_of(&m1, d1), head_of(&m2, d2));
    Ok(Report { classes, total, maxhdr_ours: k1, maxhdr_theirs: k2, image_w: i1.w, image_h: i1.h, unmatched_rows, rect_mismatch, pair_refused, peak_ours, peak_theirs, head_ours, head_theirs })
}

fn f3(v: [f64; 3], p: usize) -> String { let one = |x: f64| if x.is_finite() { format!("{:.*}", p, x) } else { "—".to_string() }; format!("{} / {} / {}", one(v[0]), one(v[1]), one(v[2])) }

/// The table on stdout (and, when asked, as TSV).
pub fn print(r: &Report, o: &Options, tsv: Option<&str>) -> Result<(), String> {
    println!("frame {}: record MaxHDR ours {} vs editor {} ({:+.2} %); image {}×{}; lit threshold {}; means over the EDITOR's lit texels; HDR = (A/255)²·(fb/255)²·MaxHDR", o.frame, r.maxhdr_ours, r.maxhdr_theirs, 100.0 * (r.maxhdr_ours as f64 / r.maxhdr_theirs.max(1e-12) as f64 - 1.0), r.image_w, r.image_h, match o.lit_hdr { Some(f) => format!("HDR ≥ {f}"), None => format!("byte ≥ {} (image-A max channel)", o.lit) });
    println!("  lossless head: ours {} bytes / {} frame records / {} image frames vs editor {} / {} / {}{}", r.head_ours.0, r.head_ours.1, r.head_ours.2, r.head_theirs.0, r.head_theirs.1, r.head_theirs.2, if r.head_ours != r.head_theirs { "  ← MISMATCH (the editor writes two records and two frames)" } else { "" });
    println!("  encode check — decoded PEAK vs the record (a clipped encode shows a peak at the record with many saturated texels): ours peak {:.5} = {:.4}× record, {} texels at A 255, {} charts at fb 255, MaxHdrMood {:?}; editor peak {:.5} = {:.4}× record, {} texels at A 255, {} charts at fb 255, MaxHdrMood {:?}; record ratio ours/editor {:.4}", r.peak_ours.0, r.peak_ours.0 / r.maxhdr_ours.max(1e-12) as f64, r.peak_ours.1, r.peak_ours.2, r.peak_ours.3, r.peak_theirs.0, r.peak_theirs.0 / r.maxhdr_theirs.max(1e-12) as f64, r.peak_theirs.1, r.peak_theirs.2, r.peak_theirs.3, r.maxhdr_ours as f64 / r.maxhdr_theirs.max(1e-12) as f64);
    if r.rect_mismatch > 0 { println!("  WARNING: {} charts {} — SKIPPED in the table below", r.rect_mismatch, if o.own_rects { "have no oracle chart of the same (obj, sub) bind word" } else { "have a different rect in the two files (the layout gate failed for them)" }); }
    if r.pair_refused > 0 { println!("  WARNING: {} pairs REFUSED by the rect-area guard (> 2.9× apart, beyond a quality ring step): the two files number their objects differently — the item rows below are NOT trustworthy until the numbering is settled", r.pair_refused); }
    if o.own_rects { println!("  --own-rects: each side's means over its own rects and its own lit texels (layouts differ); byte identity / RMSE columns are void"); }
    if r.unmatched_rows > 0 { println!("  note: {} records rows match no chart's (obj, sub)", r.unmatched_rows); }
    println!("class\tcharts\ttexels\tlit% ours\tlit% editor\tmean HDR ours (r/g/b)\tmean HDR editor (r/g/b)\tratio ours/editor (r/g/b)\tRMSE/mean (r/g/b)	bytes identical %	within 1 %	within 2 %	max|Δ|	per-chart ratio: n, median, σ(ln)");
    let mut out = String::new();
    let mut row = |name: &str, c: &ClassAcc| {
        let line = format!("{name}	{}	{}	{:.1}	{:.1}	{}	{}	{}	{}	{:.2}	{:.2}	{:.2}	{}	{}",
            c.charts, c.texels, 100.0 * c.lit_ours as f64 / c.texels.max(1) as f64, 100.0 * c.lit_theirs as f64 / c.texels.max(1) as f64,
            f3(c.mean_ours(), 4), f3(c.mean_theirs(), 4), f3(c.ratio(), 3), f3(c.rmse_rel(), 3),
            100.0 * c.exact as f64 / c.bytes.max(1) as f64, 100.0 * c.within1 as f64 / c.bytes.max(1) as f64, 100.0 * c.within2 as f64 / c.bytes.max(1) as f64, c.max_delta, match c.spread { Some((n, med, sg)) => format!("{n}, {med:.3}, {sg:.3}"), None => "—".to_string() });
        println!("{line}");
        out.push_str(&line); out.push('\n');
    };
    for (k, c) in &r.classes { row(k, c); }
    row("TOTAL", &r.total);
    if o.worst > 0 {
        println!("worst charts per class (by the max channel deviation of the per-chart HDR means): chart, ratio r/g/b, ours, editor, used texels");
        for (k, c) in &r.classes {
            for (i, _, mo, mt, used) in c.worst.iter().take(o.worst) {
                let rr = [mo[0] / mt[0].max(1e-12), mo[1] / mt[1].max(1e-12), mo[2] / mt[2].max(1e-12)];
                println!("  {k}\tchart {i}\tratio {}\tours {}\teditor {}\t{used} texels", f3(rr, 3), f3(*mo, 4), f3(*mt, 4));
            }
        }
    }
    if let Some(p) = tsv {
        let mut s = String::from("class\tcharts\ttexels\tlit_ours_pct\tlit_editor_pct\tmean_ours_rgb\tmean_editor_rgb\tratio_rgb\trmse_rel_rgb\tidentical_pct\twithin1_pct\twithin2_pct\tmax_delta\tchart_ratio_n_median_sigma\n");
        s.push_str(&out);
        // the row's record line for `lmtool trustmatrix` (V2, 2026-09-27): frame, the two record MaxHDRs and their ratio, the two heads
        // (bytes / records / frames), --own-rects (byte identity void when 1) — a `#` line after the rows so a reader that stops at TOTAL is unaffected
        s.push_str(&format!("#record\t{}\t{}\t{}\t{:.6}\t{}/{}/{}\t{}/{}/{}\t{}\n", o.frame, r.maxhdr_ours, r.maxhdr_theirs, r.maxhdr_ours as f64 / r.maxhdr_theirs.max(1e-12) as f64,
            r.head_ours.0, r.head_ours.1, r.head_ours.2, r.head_theirs.0, r.head_theirs.1, r.head_theirs.2, if o.own_rects { 1 } else { 0 }));
        std::fs::write(p, s).map_err(|e| format!("{p}: {e}"))?;
    }
    Ok(())
}

/// The bake's `--records-tsv FILE` writer: one line per chart from the layout's records.
pub fn write_records_tsv(path: &str, gl: &crate::layout::GameLayout) -> Result<(), String> {
    use std::io::Write;
    let mut fh = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    writeln!(fh, "chart\tclass\tobj\tsub\tname\tquality\tcentre_y\tcentre_x\tcentre_z\thalf_x\thalf_y\thalf_z").map_err(|e| e.to_string())?;
    for (k, r) in gl.records.iter().enumerate() {
        let name = if let Some((_, s)) = &r.item { s.split(' ').next().unwrap_or(s).rsplit('\\').next().unwrap_or(s).to_string() }
            else if let Some(mr) = &r.mesh { format!("{}#{}", mr.prefab.rsplit('\\').next().unwrap_or(&mr.prefab), mr.entity) }
            else { r.class.to_string() };
        writeln!(fh, "{k}\t{}\t{}\t{}\t{name}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}", r.class, r.obj, r.sub, r.quality, r.centre[1], r.centre[0], r.centre[2], r.half[0], r.half[1], r.half[2]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn own_px_matches_the_charts_convention() {
        assert_eq!(chart_own_px((1803, 679), (66, 66)), (902, 340, 33, 33));
        assert_eq!(chart_own_px((0, 0), (12, 14)), (0, 0, 6, 7));
    }
    #[test]
    fn records_tsv_roundtrip() {
        let p = std::env::temp_dir().join(format!("classcmp-{}.tsv", std::process::id()));
        std::fs::write(&p, "chart\tclass\tobj\tsub\tname\tquality\tcentre_y\n0\tblock\t16384\t0\tBase_Air.Prefab.Gbx#0\t1\t21.500\n1\tclipN\t25780\t3\tFCCenter_Air.Prefab.Gbx#0\t0.707\t22.000\n").unwrap();
        let rows = read_records_tsv(p.to_str().unwrap()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].class, "clipN");
        assert_eq!(rows[1].obj, 25780);
        assert_eq!(rows[1].sub, 3);
        assert!((rows[1].quality - 0.707).abs() < 1e-6);
        let _ = std::fs::remove_file(&p);
    }
}

/// `lmtool layoutcheck MAP…`: the packing invariants of a written map's mapping without an oracle — every rect inside
/// the atlas, no two rects overlapping (an occupancy grid in layout units), the fill, the frame bytes' range, the
/// frame records finite, every stored image decodable.
pub struct LayoutCheck {
    pub charts: usize,
    pub outside: usize,
    pub overlapping_cells: usize,
    pub overlapping_charts: usize,
    pub zero_area: usize,
    pub fill: f64,
    pub frame_bytes_zero: Vec<usize>,
    pub maxhdr: Vec<Option<f32>>,
    pub images: Vec<Result<(u32, u32), String>>,
}

pub fn layout_check(d: &crate::format::LightmapData) -> Result<LayoutCheck, String> {
    let m = d.cache.mapping().ok_or("a map without a mapping chunk")?;
    let (aw, ah) = (m.atlas_w as usize, m.atlas_h as usize);
    let mut grid = vec![0u8; aw * ah];
    let (mut outside, mut overlapping_cells, mut overlapping_charts, mut zero_area) = (0usize, 0usize, 0usize, 0usize);
    let mut area = 0u64;
    for i in 0..m.count as usize {
        let (x, y) = (m.pos[i].0 as usize, m.pos[i].1 as usize);
        let (w, h) = (m.size[i].0 as usize, m.size[i].1 as usize);
        if w == 0 || h == 0 { zero_area += 1; continue; }
        if x + w > aw || y + h > ah { outside += 1; continue; }
        area += (w * h) as u64;
        let mut hit = false;
        for yy in y..y + h { for xx in x..x + w { let c = &mut grid[yy * aw + xx]; if *c > 0 { overlapping_cells += 1; hit = true; } *c = c.saturating_add(1); } }
        if hit { overlapping_charts += 1; }
    }
    let frame_bytes_zero = m.frame_bytes.iter().map(|fb| fb.iter().filter(|&&b| b == 0).count()).collect();
    let maxhdr = (0..3).map(|f| record_maxhdr(&m, f)).collect();
    let images = d.frames.iter().flat_map(|f| f.images.iter()).filter(|b| !b.is_empty()).map(|b| crate::img::decode_webp(b).map(|i| (i.w, i.h))).collect();
    Ok(LayoutCheck { charts: m.count as usize, outside, overlapping_cells, overlapping_charts, zero_area, fill: area as f64 / (aw * ah) as f64, frame_bytes_zero, maxhdr, images })
}

/// The coverage (alpha) plane of a `--chain-final-dir` final (`chain-final-0.rgba16f`, 2048² RGBA f16) reduced to the
/// stored image's grid: per stored texel the MIN of the 2×2 atlas alphas (a texel is "partial" when any of its four
/// atlas texels was not fully covered — the resolve's own-normalisation path, PS 25113).
pub fn coverage_plane(path: &str, w: u32, h: u32) -> Result<Vec<f32>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let side = ((bytes.len() / 8) as f64).sqrt() as u32;
    if (side as usize) * (side as usize) * 8 != bytes.len() { return Err(format!("{path}: not a square RGBA16F plane ({} bytes)", bytes.len())); }
    if side % w != 0 || side % h != 0 { return Err(format!("{path}: {side}² does not reduce to {w}×{h}")); }
    let (sx, sy) = (side / w, side / h);
    let mut out = vec![1f32; (w * h) as usize];
    for y in 0..h { for x in 0..w {
        let mut m = f32::MAX;
        for dy in 0..sy { for dx in 0..sx {
            let i = (((y * sy + dy) * side + (x * sx + dx)) * 4 + 3) as usize * 2;
            let a = crate::gpufmt::decode_f16(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
            m = m.min(a);
        } }
        out[(y * w + x) as usize] = m;
    } }
    Ok(out)
}

/// The partial / full split of one class (ours / editor means over the editor's lit texels, as `compare`), from the
/// same rects; `partial` = coverage < `thr`.
#[derive(Clone, Debug, Default)]
pub struct CoverSplit { pub partial: ClassAcc, pub full: ClassAcc }

pub fn compare_coverage(ours: &crate::mapio::MapLightmap, theirs: &crate::mapio::MapLightmap, records: Option<&[RecRow]>, o: &Options, cov: &[f32], thr: f32) -> Result<Vec<(String, CoverSplit)>, String> {
    let (Some(d1), Some(d2)) = (ours.chunk.data.as_ref(), theirs.chunk.data.as_ref()) else { return Err("a map without a lightmap".into()) };
    let (Some(m1), Some(m2)) = (d1.cache.mapping(), d2.cache.mapping()) else { return Err("a map without a mapping chunk".into()) };
    if m1.count != m2.count { return Err("chart counts differ".into()); }
    let f = o.frame;
    let i1 = crate::img::decode_webp(&d1.frames[f].images[0])?;
    let i2 = crate::img::decode_webp(&d2.frames[f].images[0])?;
    if cov.len() != (i1.w * i1.h) as usize { return Err(format!("coverage plane {} texels vs image {}×{}", cov.len(), i1.w, i1.h)); }
    let (k1, k2) = (record_maxhdr(&m1, f).ok_or("ours: no frame record")?, record_maxhdr(&m2, f).ok_or("theirs: no frame record")?);
    let (fb1, fb2) = (&m1.frame_bytes[f], &m2.frame_bytes[f]);
    let mut rows: std::collections::HashMap<(u32, u32), RecRow> = Default::default();
    if let Some(rs) = records { for r in rs { rows.insert((r.obj, r.sub), r.clone()); } }
    let mut classes: Vec<(String, CoverSplit)> = Vec::new();
    for i in 0..m1.count as usize {
        if m1.pos[i] != m2.pos[i] || m1.size[i] != m2.size[i] { continue; }
        let key = class_key(&m1, i, &rows, records, o.by);
        let (px, py, pw, ph) = chart_own_px(m1.pos[i], m1.size[i]);
        let (fbi, fbj) = (fb1[i], fb2[i]);
        let mut sp = CoverSplit::default();
        sp.partial.charts = 1; sp.full.charts = 1;
        for y in py..(py + ph).min(i1.h) { for x in px..(px + pw).min(i1.w) {
            let (a, b) = (i1.get(x, y), i2.get(x, y));
            let acc = if cov[(y * i1.w + x) as usize] < thr { &mut sp.partial } else { &mut sp.full };
            acc.texels += 1;
            if is_lit(o, a, fbi, k1) { acc.lit_ours += 1; }
            if is_lit(o, b, fbj, k2) {
                acc.lit_theirs += 1; acc.used += 1; acc.used_t += 1;
                for c in 0..3 { let ho = texel_hdr(o.frame, a[c], fbi, k1); let ht = texel_hdr(o.frame, b[c], fbj, k2); acc.sum_ours[c] += ho; acc.sum_theirs[c] += ht; acc.sum_sq[c] += (ho - ht) * (ho - ht); }
            }
        } }
        match classes.iter_mut().find(|(k, _)| *k == key) { Some((_, c)) => { c.partial.merge(&sp.partial); c.full.merge(&sp.full); }, None => classes.push((key, sp)) }
    }
    Ok(classes)
}

pub fn print_coverage(classes: &[(String, CoverSplit)], thr: f32) {
    println!("coverage split (our finals' alpha, min over the 2×2 atlas texels; partial = < {thr}): class, texels partial / full, editor-lit partial %, ratio ours/editor on PARTIAL (r/g/b), on FULL (r/g/b)");
    for (k, c) in classes {
        let tot = (c.partial.texels + c.full.texels).max(1) as f64;
        println!("{k}\t{} / {}\t{:.1} %\tpartial {}\tfull {}", c.partial.texels, c.full.texels, 100.0 * c.partial.texels as f64 / tot, f3(c.partial.ratio(), 3), f3(c.full.ratio(), 3));
    }
}

/// The lit test of a texel: the stored byte ≥ `lit` (image-A max channel), or — with `--lit-hdr F` — the decoded HDR max channel ≥ F.
pub fn is_lit(o: &Options, px: [u8; 3], fb: u8, max_hdr: f32) -> bool {
    match o.lit_hdr {
        Some(f) => (0..3).map(|c| texel_hdr(o.frame, px[c], fb, max_hdr)).fold(0.0, f64::max) >= f,
        None => px[0].max(px[1]).max(px[2]) >= o.lit,
    }
}
