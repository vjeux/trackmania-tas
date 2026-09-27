//! `lmtool texeldelta OURS.Map.Gbx --against EDITOR.Map.Gbx [--records TSV] [--frame 0] [--png PREFIX] [--worst N] [--min-texels 64]`
//! — the SUB-1 % TEXEL RESIDUE characterised (V2, 2026-09-27; the coordinator's ask: the game agrees with itself to 99.4 % of
//! bytes on a lamp-less map, we sit at 40–80 %). Over every chart both files place at the same rect (the layout gate), the
//! stored byte difference ours − editor per texel of the chart's OWN pixels, for frame `frame`'s two image planes (image 0 =
//! the colour atlas "A", image 1 = the second atlas "B"), split by (a) the texel's RING inside the chart (0 = the edge row/column
//! of the own rect, 1, 2, 3+ = interior), (b) the chart's CLASS (the records table by (obj, sub)), (c) SIGN (systematic +1 vs
//! symmetric noise), (d) the EDITOR'S BYTE magnitude (bins of 32: encode rounding sits where the sqrt curve is steep, an
//! accumulation difference scales with the value), (e) the PLANE and channel. `--png PREFIX` writes a contact sheet of the
//! worst chart per plane (ours | editor | Δ at 128 + 16·Δ, ×4 nearest) and `--worst N` lists the N charts with the largest
//! mean |Δ| (plane A) with their class and rect.

use crate::classcmp::{chart_own_px, read_records_tsv, RecRow};

#[derive(Clone, Debug, Default)]
pub struct Acc {
    pub n: usize,
    pub exact: usize,
    pub within1: usize,
    pub within2: usize,
    pub sum: i64,
    pub sum_abs: u64,
    pub max_abs: u8,
    pub pos: usize,
    pub neg: usize,
    /// Δ in −4..=+4 (index Δ + 4), the two ends hold everything beyond
    pub hist: [usize; 9],
}

impl Acc {
    pub fn add(&mut self, d: i16) {
        self.n += 1;
        let a = d.unsigned_abs() as u8;
        if a == 0 { self.exact += 1; }
        if a <= 1 { self.within1 += 1; }
        if a <= 2 { self.within2 += 1; }
        self.sum += d as i64;
        self.sum_abs += a as u64;
        self.max_abs = self.max_abs.max(a);
        if d > 0 { self.pos += 1; } else if d < 0 { self.neg += 1; }
        self.hist[(d.clamp(-4, 4) + 4) as usize] += 1;
    }
    pub fn merge(&mut self, o: &Acc) {
        self.n += o.n; self.exact += o.exact; self.within1 += o.within1; self.within2 += o.within2; self.sum += o.sum; self.sum_abs += o.sum_abs;
        self.max_abs = self.max_abs.max(o.max_abs); self.pos += o.pos; self.neg += o.neg;
        for i in 0..9 { self.hist[i] += o.hist[i]; }
    }
    pub fn line(&self) -> String {
        if self.n == 0 { return "—".into(); }
        let n = self.n as f64;
        format!("n {:>9}  identical {:5.1} %  ±1 {:5.1} %  ±2 {:5.1} %  mean Δ {:+.3}  mean |Δ| {:.3}  max |Δ| {:3}  +:− {:.2}  hist[−4…+4] {}",
            self.n, 100.0 * self.exact as f64 / n, 100.0 * self.within1 as f64 / n, 100.0 * self.within2 as f64 / n,
            self.sum as f64 / n, self.sum_abs as f64 / n, self.max_abs, self.pos as f64 / self.neg.max(1) as f64,
            self.hist.iter().map(|v| format!("{:.1}", 100.0 * *v as f64 / n)).collect::<Vec<_>>().join(" "))
    }
}

pub struct Options { pub frame: usize, pub png: Option<String>, pub worst: usize, pub min_texels: usize }

/// The ring of pixel (x, y) inside a w×h rect: 0 on the edge, 1 one in, …
fn ring(x: u32, y: u32, w: u32, h: u32) -> usize {
    let r = x.min(y).min(w - 1 - x).min(h - 1 - y);
    (r as usize).min(3)
}

pub fn run(ours: &crate::mapio::MapLightmap, theirs: &crate::mapio::MapLightmap, records: Option<&[RecRow]>, o: &Options) -> Result<(), String> {
    let (Some(d1), Some(d2)) = (ours.chunk.data.as_ref(), theirs.chunk.data.as_ref()) else { return Err("a map without a lightmap".into()) };
    let (Some(m1), Some(m2)) = (d1.cache.mapping(), d2.cache.mapping()) else { return Err("a map without a mapping chunk".into()) };
    if m1.count != m2.count { return Err(format!("chart counts differ: ours {} vs theirs {} — the layout gate first", m1.count, m2.count)); }
    let f = o.frame;
    let (Some(f1), Some(f2)) = (d1.frames.get(f), d2.frames.get(f)) else { return Err(format!("frame {f}: not in both files")) };
    // the planes: image 0 (the colour atlas, RGB) and — on frame 0 — image 1's THREE concatenated grey WebPs (the H-basis directional
    // coefficients C1..C3, sign-sqrt encoded with 128 = zero; `riff_parts`), each compared as a grey plane
    let mut imgs: Vec<(crate::img::Rgb, crate::img::Rgb)> = Vec::new();
    let mut plane_names: Vec<String> = Vec::new();
    {
        let (Some(b1), Some(b2)) = (f1.images.first(), f2.images.first()) else { return Err(format!("frame {f}: no image 0")) };
        let a = crate::img::decode_webp(b1).map_err(|e| format!("ours frame {f} image 0: {e}"))?;
        let b = crate::img::decode_webp(b2).map_err(|e| format!("theirs frame {f} image 0: {e}"))?;
        if a.w != b.w || a.h != b.h { return Err("image 0 sizes differ".into()); }
        imgs.push((a, b)); plane_names.push("A (image 0, the colour atlas)".into());
    }
    if let (Some(b1), Some(b2)) = (f1.images.get(1), f2.images.get(1)) {
        let (p1, p2) = (riff_parts(b1), riff_parts(b2));
        for k in 0..p1.len().min(p2.len()) {
            let a = crate::img::decode_webp(p1[k]).map_err(|e| format!("ours frame {f} image 1 part {k}: {e}"))?;
            let b = crate::img::decode_webp(p2[k]).map_err(|e| format!("theirs frame {f} image 1 part {k}: {e}"))?;
            if a.w != imgs[0].0.w || a.h != imgs[0].0.h || b.w != a.w || b.h != a.h { eprintln!("texeldelta: image 1 part {k} is {}×{} (image 0 {}×{}) — skipped", a.w, a.h, imgs[0].0.w, imgs[0].0.h); continue; }
            imgs.push((a, b)); plane_names.push(format!("C{} (image 1 part {k}: H-basis directional coefficient, grey, 128 = zero)", k + 1));
        }
    }
    let planes = imgs.len();
    // the directional planes decoded to VALUES: the encode (CS 23025, gpuenc.rs) stores per coefficient k = 1..3 the luma of
    // c = clamp(0.5 + 0.5·sign(v)·sqrt(|v| / m_k)), m_k = the GPU MaxHdr of that plane; the record word HBasis234[k] = m_k · 0.6909883
    // (filecheck::record_scales) → v ≈ sign(b / 255 − 0.5) · ((b / 255 − 0.5) / 0.5)² · HBasis234[k] / 0.6909883 (the luma of three encoded
    // channels stands in for one; a per-class MEAN and a SIGN agreement, not a texel truth)
    let hb = |m: &crate::format::Mapping| -> [f32; 3] { let r = 60 + 66 * f; if r + 66 <= m.head.len() { [54usize, 58, 62].map(|o| f32::from_le_bytes(m.head[r + o..r + o + 4].try_into().unwrap())) } else { [1.0; 3] } };
    let (hb1, hb2) = (hb(&m1), hb(&m2));
    let cval = |b: u8, k: usize, hbw: [f32; 3]| -> f64 { let t = b as f64 / 255.0 - 0.5; t.signum() * (t / 0.5) * (t / 0.5) * (hbw[k] as f64 / 0.6909883) };
    // per class per C plane: Σ ours, Σ editor, Σ|ours|, Σ|editor|, n, sign agreements among texels with |editor| ≥ 5 % of its plane word
    let mut cvals: std::collections::BTreeMap<String, Vec<(f64, f64, f64, f64, usize, usize, usize)>> = Default::default();
    let fb1 = m1.frame_bytes.get(f).ok_or("ours: no frame bytes")?;
    let fb2 = m2.frame_bytes.get(f).ok_or("theirs: no frame bytes")?;
    let rows: std::collections::HashMap<(u32, u32), RecRow> = records.map(|rs| rs.iter().map(|r| ((r.obj, r.sub), r.clone())).collect()).unwrap_or_default();
    let class_of = |i: usize| -> String {
        let key = (m1.binds[i].obj_group_idx / 4, m1.binds[i].obj_idx & 0x00ff_ffff);
        match rows.get(&key) { Some(r) => if r.class == "item" || r.class == "block" || r.class == "clip" { format!("{}:{}", r.class, r.name) } else { r.class.clone() }, None => if key.0 < 4096 { "tile".into() } else { "other".into() } }
    };
    let n = m1.count as usize;
    // accumulators
    let mut by_plane_ch: Vec<[Acc; 3]> = (0..planes).map(|_| Default::default()).collect();
    let mut by_ring: Vec<[Acc; 4]> = (0..planes).map(|_| Default::default()).collect();
    let mut by_bin: Vec<[Acc; 8]> = (0..planes).map(|_| Default::default()).collect();
    let mut by_class: std::collections::BTreeMap<String, Vec<Acc>> = Default::default();
    let mut fb_diff: Vec<Acc> = (0..planes).map(|_| Acc::default()).collect(); // charts whose frame byte differs
    let mut charts_same_fb = 0usize;
    let mut skipped = 0usize;
    // per chart: (chart, class, mean |Δ| plane A, texels)
    let mut per_chart: Vec<(usize, String, f64, usize)> = Vec::new();
    for i in 0..n {
        if m1.pos[i] != m2.pos[i] || m1.size[i] != m2.size[i] { skipped += 1; continue; }
        let (px, py, pw, ph) = chart_own_px(m1.pos[i], m1.size[i]);
        if pw == 0 || ph == 0 { continue; }
        let cls = class_of(i);
        let entry = by_class.entry(cls.clone()).or_insert_with(|| (0..planes).map(|_| Acc::default()).collect());
        let same_fb = fb1.get(i) == fb2.get(i);
        if same_fb { charts_same_fb += 1; }
        let mut chart_acc = Acc::default();
        for p in 0..planes {
            let (a, b) = &imgs[p];
            for y in 0..ph { for x in 0..pw {
                let (ca, cb) = (a.get(px + x, py + y), b.get(px + x, py + y));
                let r = ring(x, y, pw, ph);
                let nch = if p == 0 { 3 } else { 1 };
                for c in 0..nch {
                    let d = ca[c] as i16 - cb[c] as i16;
                    by_plane_ch[p][c].add(d);
                    by_ring[p][r].add(d);
                    by_bin[p][(cb[c] / 32) as usize].add(d);
                    entry[p].add(d);
                    if p > 0 {
                        let cv = cvals.entry(cls.clone()).or_insert_with(|| vec![(0.0, 0.0, 0.0, 0.0, 0, 0, 0); 3]);
                        let (vo, ve) = (cval(ca[0], p - 1, hb1), cval(cb[0], p - 1, hb2));
                        let e = &mut cv[(p - 1).min(2)];
                        e.0 += vo; e.1 += ve; e.2 += vo.abs(); e.3 += ve.abs(); e.4 += 1;
                        if ve.abs() >= 0.05 * (hb2[(p - 1).min(2)] as f64 / 0.6909883) { e.5 += 1; if vo.signum() == ve.signum() { e.6 += 1; } }
                    }
                    if !same_fb { fb_diff[p].add(d); }
                    if p == 0 { chart_acc.add(d); }
                }
            } }
        }
        if chart_acc.n > 0 { per_chart.push((i, cls, chart_acc.sum_abs as f64 / chart_acc.n as f64, (pw * ph) as usize)); }
    }
    println!("texeldelta frame {f}: {} charts compared at the same rect ({skipped} skipped: rect differs), {planes} plane(s) {}×{} (A = image 0 RGB; C1..C3 = image 1's grey parts, channel 0 only); Δ = ours − editor per stored byte; chart frame bytes identical on {charts_same_fb} of {n}", n - skipped, imgs[0].0.w, imgs[0].0.h);
    let plane_name = |p: usize| plane_names[p].as_str();
    for p in 0..planes {
        println!("\n== plane {} — per channel", plane_name(p));
        for c in 0..(if p == 0 { 3 } else { 1 }) { println!("  ch {c}: {}", by_plane_ch[p][c].line()); }
        println!("-- by RING inside the chart's own rect (0 = edge row/column, 3 = 3+ in)");
        for r in 0..4 { println!("  ring {r}: {}", by_ring[p][r].line()); }
        println!("-- by the EDITOR's byte (bins of 32)");
        for b in 0..8 { println!("  {:3}–{:3}: {}", b * 32, b * 32 + 31, by_bin[p][b].line()); }
        println!("-- charts whose frame byte DIFFERS between the files: {}", fb_diff[p].line());
    }
    println!("\n== the directional coefficients DECODED to values (v ≈ sign·((b/255 − 0.5)/0.5)²·HBasis234[k]/0.691; record words ours {:?} vs editor {:?}) — per class: mean signed ours / editor, mean |v| ours / editor, ratio of mean |v|, sign agreement where |editor| ≥ 5 % of the plane word", hb1, hb2);
    {
        let mut cs: Vec<(&String, &Vec<(f64, f64, f64, f64, usize, usize, usize)>)> = cvals.iter().collect();
        cs.sort_by(|a, b| b.1[0].4.cmp(&a.1[0].4));
        for (k, v) in cs.iter().take(24) {
            let cells: Vec<String> = v.iter().enumerate().filter(|(_, e)| e.4 > 0).map(|(i, e)| { let n = e.4 as f64; format!("C{}: {:+.4}/{:+.4} |{:.4}/{:.4}| ×{:.2} sign {:.0} % of {}", i + 1, e.0 / n, e.1 / n, e.2 / n, e.3 / n, (e.2 / n) / (e.3 / n).max(1e-9), 100.0 * e.6 as f64 / e.5.max(1) as f64, e.5) }).collect();
            println!("  {k:40} {}", cells.join("   "));
        }
    }
    println!("\n== by CLASS (plane A, then C1..C3)");
    let mut classes: Vec<(&String, &Vec<Acc>)> = by_class.iter().collect();
    classes.sort_by(|a, b| b.1[0].n.cmp(&a.1[0].n));
    for (k, accs) in classes.iter().take(40) {
        println!("  {k}");
        for (p, acc) in accs.iter().enumerate() { println!("    {}: {}", short_plane(p), acc.line()); }
    }
    // the worst charts
    per_chart.retain(|c| c.3 >= o.min_texels);
    per_chart.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    if o.worst > 0 {
        println!("\n== worst charts by mean |Δ| (plane A, charts with ≥ {} own texels)", o.min_texels);
        for (i, cls, m, t) in per_chart.iter().take(o.worst) {
            let (px, py, pw, ph) = chart_own_px(m1.pos[*i], m1.size[*i]);
            println!("  chart {i:6} {cls:40} mean |Δ| {m:.3} over {t} texels at ({px},{py}) {pw}×{ph} fb {} / {}", fb1.get(*i).copied().unwrap_or(0), fb2.get(*i).copied().unwrap_or(0));
        }
    }
    if let Some(prefix) = &o.png {
        if let Some((i, cls, m, _)) = per_chart.first() {
            let (px, py, pw, ph) = chart_own_px(m1.pos[*i], m1.size[*i]);
            let scale = (256 / pw.max(ph)).clamp(1, 8);
            for p in 0..planes {
                let (a, b) = &imgs[p];
                let (w, h) = (pw * scale, ph * scale);
                // three panels side by side with a 2-px gap: ours | editor | Δ
                let gap = 2u32;
                let tw = 3 * w + 2 * gap;
                let mut buf = vec![0u8; (tw * h * 3) as usize];
                for y in 0..h { for x in 0..w {
                    let (sx, sy) = (px + x / scale, py + y / scale);
                    let (ca, cb) = (a.get(sx, sy), b.get(sx, sy));
                    let put = |buf: &mut Vec<u8>, ox: u32, c: [u8; 3]| { let k = ((y * tw + ox + x) * 3) as usize; buf[k..k + 3].copy_from_slice(&c); };
                    put(&mut buf, 0, ca);
                    put(&mut buf, w + gap, cb);
                    let d: [u8; 3] = [0, 1, 2].map(|c| (128i32 + 16 * (ca[c] as i32 - cb[c] as i32)).clamp(0, 255) as u8);
                    put(&mut buf, 2 * (w + gap), d);
                } }
                let path = format!("{prefix}-chart{i}-plane{}.png", short_plane(p));
                crate::png::write_rgb(&path, tw, h, &buf).map_err(|e| format!("{path}: {e}"))?;
                println!("png: {path} — worst chart {i} ({cls}, mean |Δ| {m:.3}): ours | editor | Δ (128 + 16·Δ), ×{scale}");
            }
        }
    }
    Ok(())
}

pub fn read_records(path: &str) -> Result<Vec<RecRow>, String> { read_records_tsv(path) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acc_counts_sign_and_histogram() {
        let mut a = Acc::default();
        for d in [0, 0, 1, -1, 2, 7, -9] { a.add(d); }
        assert_eq!((a.n, a.exact, a.within1, a.within2, a.max_abs, a.pos, a.neg), (7, 2, 4, 5, 9, 3, 2));
        assert_eq!(a.hist[4], 2); // Δ = 0
        assert_eq!(a.hist[8], 1); // +7 folded into the +4 end
        assert_eq!(a.hist[0], 1); // −9 folded into the −4 end
        assert_eq!(a.sum, 0 + 0 + 1 - 1 + 2 + 7 - 9);
    }
    #[test]
    fn ring_is_the_distance_to_the_edge_capped_at_3() {
        assert_eq!(ring(0, 5, 10, 10), 0);
        assert_eq!(ring(1, 5, 10, 10), 1);
        assert_eq!(ring(4, 4, 10, 10), 3);
        assert_eq!(ring(9, 0, 10, 10), 0);
        assert_eq!(ring(2, 7, 10, 10), 2);
    }
}

/// The RIFF chunks of a possibly concatenated WebP blob (frame 0 image 1 = three grey WebPs back to back).
pub fn riff_parts(blob: &[u8]) -> Vec<&[u8]> {
    let mut parts = Vec::new();
    let mut at = 0usize;
    while at + 12 <= blob.len() && &blob[at..at + 4] == b"RIFF" {
        let sz = u32::from_le_bytes([blob[at + 4], blob[at + 5], blob[at + 6], blob[at + 7]]) as usize + 8;
        let end = (at + sz).min(blob.len());
        parts.push(&blob[at..end]);
        at = end;
    }
    if parts.is_empty() && !blob.is_empty() { parts.push(blob); }
    parts
}

fn short_plane(p: usize) -> String { if p == 0 { "A".to_string() } else { format!("C{p}") } }
