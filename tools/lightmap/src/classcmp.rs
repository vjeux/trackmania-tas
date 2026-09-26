//! `lmtool classcmp OURS.Map.Gbx --against EDITOR.Map.Gbx [--records TSV] [--by class|name|obj] [--frame 0] [--lit 8]
//! [--tsv OUT.tsv] [--worst N]` — two WRITTEN maps side by side, per CLASS of chart: the decoded HDR texels
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
        });
    }
    Ok(out)
}

/// The frame record's MaxHDR (head: 3 × 66 bytes from offset 60; +20 = MaxHDR).
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
    /// the texels the means run over (lit in the oracle)
    pub used: usize,
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
}

impl ClassAcc {
    pub fn mean_ours(&self) -> [f64; 3] { let n = self.used.max(1) as f64; [self.sum_ours[0] / n, self.sum_ours[1] / n, self.sum_ours[2] / n] }
    pub fn mean_theirs(&self) -> [f64; 3] { let n = self.used.max(1) as f64; [self.sum_theirs[0] / n, self.sum_theirs[1] / n, self.sum_theirs[2] / n] }
    pub fn ratio(&self) -> [f64; 3] { let (a, b) = (self.mean_ours(), self.mean_theirs()); [a[0] / b[0].max(1e-12), a[1] / b[1].max(1e-12), a[2] / b[2].max(1e-12)] }
    /// RMSE of the HDR difference over the used texels, relative to the oracle's mean (per channel).
    pub fn rmse_rel(&self) -> [f64; 3] { let n = self.used.max(1) as f64; let b = self.mean_theirs(); [(self.sum_sq[0] / n).sqrt() / b[0].max(1e-12), (self.sum_sq[1] / n).sqrt() / b[1].max(1e-12), (self.sum_sq[2] / n).sqrt() / b[2].max(1e-12)] }
    pub fn merge(&mut self, o: &ClassAcc) {
        self.charts += o.charts; self.texels += o.texels; self.lit_ours += o.lit_ours; self.lit_theirs += o.lit_theirs; self.used += o.used;
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
    pub by: GroupBy,
    pub worst: usize,
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
    pub image_w: u32,
    pub image_h: u32,
    pub unmatched_rows: usize,
    pub rect_mismatch: usize,
}

/// The whole comparison: both maps' frame `frame` image 0 decoded, per class.
pub fn compare(ours: &crate::mapio::MapLightmap, theirs: &crate::mapio::MapLightmap, records: Option<&[RecRow]>, o: &Options) -> Result<Report, String> {
    let (Some(d1), Some(d2)) = (ours.chunk.data.as_ref(), theirs.chunk.data.as_ref()) else { return Err("a map without a lightmap".into()) };
    let (Some(m1), Some(m2)) = (d1.cache.mapping(), d2.cache.mapping()) else { return Err("a map without a mapping chunk".into()) };
    if m1.count != m2.count { return Err(format!("chart counts differ: ours {} vs theirs {} — the layout gate first", m1.count, m2.count)); }
    let n = m1.count as usize;
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
    for i in 0..n {
        if m1.pos[i] != m2.pos[i] || m1.size[i] != m2.size[i] { rect_mismatch += 1; continue; }
        let key = class_key(&m1, i, &rows, records, o.by);
        let (px, py, pw, ph) = chart_own_px(m1.pos[i], m1.size[i]);
        let mut acc = ClassAcc { charts: 1, ..Default::default() };
        let (s1, s2) = ((fb1.get(i).copied().unwrap_or(0) as f64 / 255.0).powi(2) * k1 as f64, (fb2.get(i).copied().unwrap_or(0) as f64 / 255.0).powi(2) * k2 as f64);
        let (mut co, mut ct) = ([0f64; 3], [0f64; 3]);
        for y in py..(py + ph).min(i1.h) {
            for x in px..(px + pw).min(i1.w) {
                let a = i1.get(x, y);
                let b = i2.get(x, y);
                acc.texels += 1;
                let lo = a[0].max(a[1]).max(a[2]) >= o.lit;
                let lt = b[0].max(b[1]).max(b[2]) >= o.lit;
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
                    acc.used += 1;
                    for c in 0..3 {
                        let ho = (a[c] as f64 / 255.0).powi(2) * s1;
                        let ht = (b[c] as f64 / 255.0).powi(2) * s2;
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
    let trim = |c: &mut ClassAcc| { c.worst.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)); c.worst.truncate(o.worst); };
    for (_, c) in classes.iter_mut() { trim(c); }
    trim(&mut total);
    Ok(Report { classes, total, maxhdr_ours: k1, maxhdr_theirs: k2, image_w: i1.w, image_h: i1.h, unmatched_rows, rect_mismatch })
}

fn f3(v: [f64; 3], p: usize) -> String { format!("{:.*} / {:.*} / {:.*}", p, v[0], p, v[1], p, v[2]) }

/// The table on stdout (and, when asked, as TSV).
pub fn print(r: &Report, o: &Options, tsv: Option<&str>) -> Result<(), String> {
    println!("frame {}: record MaxHDR ours {} vs editor {} ({:+.2} %); image {}×{}; lit threshold {} (image-A max channel); means over the EDITOR's lit texels; HDR = (A/255)²·(fb/255)²·MaxHDR", o.frame, r.maxhdr_ours, r.maxhdr_theirs, 100.0 * (r.maxhdr_ours as f64 / r.maxhdr_theirs.max(1e-12) as f64 - 1.0), r.image_w, r.image_h, o.lit);
    if r.rect_mismatch > 0 { println!("  WARNING: {} charts have a different rect in the two files (the layout gate failed for them) — SKIPPED in the table below", r.rect_mismatch); }
    if r.unmatched_rows > 0 { println!("  note: {} records rows match no chart's (obj, sub)", r.unmatched_rows); }
    println!("class\tcharts\ttexels\tlit% ours\tlit% editor\tmean HDR ours (r/g/b)\tmean HDR editor (r/g/b)\tratio ours/editor (r/g/b)\tRMSE/mean (r/g/b)\tbytes identical %\twithin 1 %\twithin 2 %\tmax|Δ|");
    let mut out = String::new();
    let mut row = |name: &str, c: &ClassAcc| {
        let line = format!("{name}\t{}\t{}\t{:.1}\t{:.1}\t{}\t{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{}",
            c.charts, c.texels, 100.0 * c.lit_ours as f64 / c.texels.max(1) as f64, 100.0 * c.lit_theirs as f64 / c.texels.max(1) as f64,
            f3(c.mean_ours(), 4), f3(c.mean_theirs(), 4), f3(c.ratio(), 3), f3(c.rmse_rel(), 3),
            100.0 * c.exact as f64 / c.bytes.max(1) as f64, 100.0 * c.within1 as f64 / c.bytes.max(1) as f64, 100.0 * c.within2 as f64 / c.bytes.max(1) as f64, c.max_delta);
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
        let mut s = String::from("class\tcharts\ttexels\tlit_ours_pct\tlit_editor_pct\tmean_ours_rgb\tmean_editor_rgb\tratio_rgb\trmse_rel_rgb\tidentical_pct\twithin1_pct\twithin2_pct\tmax_delta\n");
        s.push_str(&out);
        std::fs::write(p, s).map_err(|e| format!("{p}: {e}"))?;
    }
    Ok(())
}

/// The bake's `--records-tsv FILE` writer: one line per chart from the layout's records.
pub fn write_records_tsv(path: &str, gl: &crate::layout::GameLayout) -> Result<(), String> {
    use std::io::Write;
    let mut fh = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    writeln!(fh, "chart\tclass\tobj\tsub\tname\tquality\tcentre_y").map_err(|e| e.to_string())?;
    for (k, r) in gl.records.iter().enumerate() {
        let name = if let Some((_, s)) = &r.item { s.split(' ').next().unwrap_or(s).rsplit('\\').next().unwrap_or(s).to_string() }
            else if let Some(mr) = &r.mesh { format!("{}#{}", mr.prefab.rsplit('\\').next().unwrap_or(&mr.prefab), mr.entity) }
            else { r.class.to_string() };
        writeln!(fh, "{k}\t{}\t{}\t{}\t{name}\t{}\t{:.3}", r.class, r.obj, r.sub, r.quality, r.centre[1]).map_err(|e| e.to_string())?;
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
