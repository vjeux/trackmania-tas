//! `lmtool corpus-gate` — THE CORPUS GATE (baker-4, 2026-09-27): every landing bakes the whole test corpus
//! (feature × collection × mood cells, each a small map with an editor oracle) with the candidate binary and
//! prints the matrix — identity %, within ±1 / ±2, max |Δ|, record MaxHDR, the per-class ratios — per cell,
//! against the cell's oracle AND against the previous base's run of the same cell. A cell whose output moved
//! is named before the base is announced; a cell that meets the measured ceiling (V2-5: the editor against
//! its own nocache re-bake = 60.1 % identity / 92.4 % within ±2 / max |Δ| 43 / classes 1.000 ± 0.004) is CLOSED.
//!
//!     lmtool corpus-gate run    --corpus CORPUS.tsv --tip TIP --work W [--tmp DIR] [--refs DIR] [--paks DIR]
//!                               [--box K --boxes N] [--only cell,…] [--force] [--dry-run]
//!     lmtool corpus-gate report --corpus CORPUS.tsv --tip TIP --work W [--against PREV_TIP] [--md OUT.md]
//!                               [--target ID,WITHIN2,MAXD,RATIO]
//!     lmtool corpus-gate list   --corpus CORPUS.tsv [--refs DIR]
//!
//! CORPUS.tsv (tab-separated, `#` comments), one cell per line:
//!     cell  source  oracle  collection  quality  word  dirs  kept  extra
//!   source / oracle / kept: paths, relative to --refs unless absolute; `word` = `-` or the DayTime word the source copy
//!   gets (`0x199a`; `lmtool daytime-set`) so the bake runs at the oracle's time; `dirs` = `full` or N (`--max-dirs N`:
//!   frame 1 and the layout do not depend on the direction count — a d1 cell is a lamp / layout row, a full cell is a
//!   frame-0 row); `kept` = `-` or the reduced oracle's kept list; `extra` = `-` or extra bake flags (space-separated).
//!
//! The bake per cell is the PRODUCT recipe: `bake SRC --raster --lm-from-map --collection C --quality Q --pak <coll>
//! --pak Stadium --pak Maniaplanet [--kept K] [--max-dirs N] [extra] --records-tsv … --out …` — no LMTOOL_BAKE_TIME pin since
//! base 594ea30d: the FILETIME word is the map's TimeWriteMostRecentSolid (the writer's default, the game's load-time rule)
//! (the writer is deterministic: two runs of one binary give one md5, so "the cell moved" = "the md5 moved", and the
//! metrics say WHAT moved). Cells are dealt to boxes round-robin by their order in the file (`--box K --boxes N`);
//! every box writes W/TIP/<cell>/{ours.Map.Gbx, records.tsv, bake.log, metrics.json}, the report reads W.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Cell {
    pub name: String,
    pub source: PathBuf,
    /// None = a cell WITHOUT an oracle (RedIsland, q1/q2): listed in the matrix as "no oracle", never baked
    pub oracle: Option<PathBuf>,
    pub collection: String,
    pub quality: String,
    pub word: Option<String>,
    /// `--max-dirs N`; None = full; `compare` cells (no bake: `source` IS our side) carry `compare_only`
    pub dirs: Option<String>,
    pub compare_only: bool,
    pub kept: Option<PathBuf>,
    pub extra: Vec<String>,
    /// `env:KEY=VAL` tokens of the extra column → the bake's environment (LMTOOL_LAMP_FILTER=stock …)
    pub env: Vec<(String, String)>,
    /// V2's CEILING = the editor's own re-bake identity on that map class (99 lamp-less, 60 lamp maps); the verdict is
    /// measured against 90 % of it
    pub ceiling: f64,
}

fn pak_key(coll: &str) -> &'static str {
    match coll {
        "Stadium" => "B773D73047A4104857722366D78D28A6",
        "Maniaplanet" => "9A93723447347A8CE336CCFC49E65449",
        // BlueBay, GreenCoast, WhiteShore, RedIsland share the islands key
        _ => "660C4C156B80337E296A1034B0AA05B8",
    }
}

fn home() -> String { std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into()) }

fn default_refs() -> PathBuf { PathBuf::from(format!("{}/persistent/private-30d/tm-player/tiny/lightmap-re/refs", home())) }

fn default_paks() -> PathBuf {
    let local = PathBuf::from("/tmp/paks");
    if local.join("Maniaplanet.pak").exists() { local } else { PathBuf::from(format!("{}/persistent/private-30d/tm-paks", home())) }
}

fn resolve(refs: &Path, p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() { pb } else { refs.join(pb) }
}

pub fn read_corpus(path: &Path, refs: &Path) -> Result<Vec<Cell>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut cells = Vec::new();
    for (ln, line) in txt.lines().enumerate() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') { continue; }
        let cols: Vec<&str> = line.split('\t').map(|c| c.trim()).collect();
        if cols.len() < 5 { return Err(format!("{}:{}: {} columns, need cell/source/oracle/collection/quality[/word/dirs/kept/extra]", path.display(), ln + 1, cols.len())); }
        if cols[0] == "cell" && cols[1] == "source" { continue; } // a header line
        let opt = |i: usize| cols.get(i).filter(|c| !c.is_empty() && **c != "-").map(|c| c.to_string());
        let extra_all: Vec<String> = opt(8).map(|e| e.split_whitespace().map(|s| s.to_string()).collect()).unwrap_or_default();
        let env: Vec<(String, String)> = extra_all.iter().filter_map(|t| t.strip_prefix("env:")).filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
        let extra: Vec<String> = extra_all.into_iter().filter(|t| !t.starts_with("env:")).collect();
        let dirs = opt(6);
        let compare_only = dirs.as_deref() == Some("compare");
        let ceiling: f64 = match opt(9) { Some(c) => c.parse().map_err(|e| format!("{}:{}: ceiling {c:?}: {e}", path.display(), ln + 1))?, None => 99.0 };
        cells.push(Cell {
            name: cols[0].to_string(),
            source: resolve(refs, cols[1]),
            oracle: opt(2).map(|o| resolve(refs, &o)),
            collection: cols[3].to_string(),
            quality: cols[4].to_string(),
            word: opt(5),
            dirs: dirs.filter(|d| d != "full" && d != "compare"),
            compare_only,
            kept: opt(7).map(|k| resolve(refs, &k)),
            extra,
            env,
            ceiling,
        });
    }
    let mut seen = std::collections::HashSet::new();
    for c in &cells { if !seen.insert(c.name.clone()) { return Err(format!("duplicate cell name {}", c.name)); } }
    Ok(cells)
}

/// The bake command line of a cell (without the binary), paths as given.
pub fn bake_args(c: &Cell, source: &Path, paks: &Path, records: &Path, out: &Path) -> Vec<String> {
    let mut a: Vec<String> = vec!["bake".into(), source.to_string_lossy().into(), "--raster".into(), "--lm-from-map".into(), "--collection".into(), c.collection.clone(), "--quality".into(), c.quality.clone()];
    let mut pak_list: Vec<String> = vec![c.collection.clone()];
    if c.collection != "Stadium" { pak_list.push("Stadium".into()); }
    pak_list.push("Maniaplanet".into());
    for p in pak_list { a.push("--pak".into()); a.push(format!("{}:{}", paks.join(format!("{p}.pak")).to_string_lossy(), pak_key(&p))); }
    if let Some(k) = &c.kept { a.push("--kept".into()); a.push(k.to_string_lossy().into()); }
    if let Some(d) = &c.dirs { a.push("--max-dirs".into()); a.push(d.clone()); }
    a.extend(c.extra.iter().cloned());
    a.push("--records-tsv".into()); a.push(records.to_string_lossy().into());
    a.push("--out".into()); a.push(out.to_string_lossy().into());
    a
}

// ---------------------------------------------------------------- MD5 (RFC 1321), for the "did the bytes move" line
pub fn md5_hex(data: &[u8]) -> String {
    let s: [u32; 64] = [7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21];
    let mut k = [0u32; 64];
    for i in 0..64 { k[i] = ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32; }
    let (mut a0, mut b0, mut c0, mut d0) = (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 { msg.push(0); }
    msg.extend_from_slice(&bit_len.to_le_bytes());
    for chunk in msg.chunks(64) {
        let mut m = [0u32; 16];
        for i in 0..16 { m[i] = u32::from_le_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]); }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f2 = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d; d = c; c = b;
            b = b.wrapping_add(f2.rotate_left(s[i]));
        }
        a0 = a0.wrapping_add(a); b0 = b0.wrapping_add(b); c0 = c0.wrapping_add(c); d0 = d0.wrapping_add(d);
    }
    let mut out = String::new();
    for v in [a0, b0, c0, d0] { for byte in v.to_le_bytes() { out.push_str(&format!("{byte:02x}")); } }
    out
}

// ---------------------------------------------------------------- the metrics of one baked cell against its oracle
fn frame_metrics(ours: &crate::mapio::MapLightmap, theirs: &crate::mapio::MapLightmap, records: Option<&[crate::classcmp::RecRow]>, frame: usize, own_rects: bool) -> Result<serde_json::Value, String> {
    let mut o = crate::classcmp::Options { frame, lit: 8, by: crate::classcmp::GroupBy::Class, worst: 0, own_rects, lit_hdr: None, peaks: 0, near: None, density_bins: Vec::new(), y_bins: Vec::new() };
    // two layouts that differ (chart counts, a giant's numbering, OR the same count with other rects) are compared by (obj, sub)
    // with each side's own rects — decided from the layout BEFORE the compare (V3 2026-09-28 03:56Z: with equal counts and 215 of
    // 5 871 same rects the plain compare silently skipped the rect-mismatched charts and the headline numbers described 4 % of the
    // cell); the identity columns are then void and say so
    let r = match crate::classcmp::compare(ours, theirs, records, &o) {
        Ok(r) => r,
        Err(e) if e.contains("chart counts differ") || e.contains("--own-rects") => { o.own_rects = true; crate::classcmp::compare(ours, theirs, records, &o)? }
        Err(e) => return Err(e),
    };
    let pct = |n: usize, d: usize| if d == 0 { 0.0 } else { 100.0 * n as f64 / d as f64 };
    let fin = |v: f64| if v.is_finite() { serde_json::json!((v * 1e6).round() / 1e6) } else { serde_json::Value::Null };
    let classes: Vec<serde_json::Value> = r.classes.iter().map(|(name, c)| {
        let ratio = c.ratio();
        serde_json::json!({ "class": name, "charts": c.charts, "texels": c.texels, "used": c.used, "lit_ours_pct": pct(c.lit_ours, c.texels), "lit_theirs_pct": pct(c.lit_theirs, c.texels),
            "ratio": [fin(ratio[0]), fin(ratio[1]), fin(ratio[2])], "identity_pct": pct(c.exact, c.bytes), "within2_pct": pct(c.within2, c.bytes), "max_delta": c.max_delta })
    }).collect();
    let t = &r.total;
    Ok(serde_json::json!({
        "frame": frame, "record_ours": r.maxhdr_ours, "record_theirs": r.maxhdr_theirs,
        "identity_pct": pct(t.exact, t.bytes), "within1_pct": pct(t.within1, t.bytes), "within2_pct": pct(t.within2, t.bytes), "max_delta": t.max_delta,
        "lit_ours_pct": pct(t.lit_ours, t.texels), "lit_theirs_pct": pct(t.lit_theirs, t.texels), "texels": t.texels,
        "ratio": [fin(t.ratio()[0]), fin(t.ratio()[1]), fin(t.ratio()[2])],
        "peak_ours": r.peak_ours.0, "peak_theirs": r.peak_theirs.0,
        "head_ours": [r.head_ours.0, r.head_ours.1, r.head_ours.2], "head_theirs": [r.head_theirs.0, r.head_theirs.1, r.head_theirs.2],
        "unmatched_rows": r.unmatched_rows, "rect_mismatch": r.rect_mismatch, "own_rects": o.own_rects, "pair_refused": r.pair_refused, "classes": classes,
    }))
}

/// V2's PLANE ROW (2026-09-27): the stored-byte identity of frame 0's planes per chart at the same rect — A (image 0, the
/// colour atlas, RGB) and C1..C3 (image 1's three grey WebPs = the H-basis directional coefficients; `texeldelta::riff_parts`)
/// — split tiles / items, plus the charts whose frame byte differs between the files (ours fb 240 vs the editor's 241 on
/// 634 of np-tk3's 4 126). The editor reproduces the C planes 100 % between its own bakes; the initial bar is tiles ≥ 90 %,
/// items ≥ 85 %.
fn plane_metrics(ours: &crate::mapio::MapLightmap, theirs: &crate::mapio::MapLightmap) -> serde_json::Value {
    let (Some(d1), Some(d2)) = (ours.chunk.data.as_ref(), theirs.chunk.data.as_ref()) else { return serde_json::Value::Null };
    let (Some(m1), Some(m2)) = (d1.cache.mapping(), d2.cache.mapping()) else { return serde_json::Value::Null };
    let (Some(f1), Some(f2)) = (d1.frames.first(), d2.frames.first()) else { return serde_json::Value::Null };
    let mut planes: Vec<(String, crate::img::Rgb, crate::img::Rgb, usize)> = Vec::new();
    if let (Some(b1), Some(b2)) = (f1.images.first(), f2.images.first()) {
        if let (Ok(a), Ok(b)) = (crate::img::decode_webp(b1), crate::img::decode_webp(b2)) { if a.w == b.w && a.h == b.h { planes.push(("A".into(), a, b, 3)); } }
    }
    if let (Some(b1), Some(b2)) = (f1.images.get(1), f2.images.get(1)) {
        let (p1, p2) = (crate::texeldelta::riff_parts(b1), crate::texeldelta::riff_parts(b2));
        for k in 0..p1.len().min(p2.len()).min(3) {
            if let (Ok(a), Ok(b)) = (crate::img::decode_webp(p1[k]), crate::img::decode_webp(p2[k])) {
                if a.w == b.w && a.h == b.h && planes.first().map_or(true, |p| p.1.w == a.w && p.1.h == a.h) { planes.push((format!("C{}", k + 1), a, b, 1)); }
            }
        }
    }
    let n = m1.count.min(m2.count) as usize;
    // per plane: [tiles exact, tiles bytes, items exact, items bytes]
    let mut acc: Vec<[usize; 4]> = vec![[0; 4]; planes.len()];
    let (mut fb_same, mut fb_diff, mut skipped) = (0usize, 0usize, 0usize);
    let (fb1, fb2) = (m1.frame_bytes.first(), m2.frame_bytes.first());
    for i in 0..n {
        if m1.pos[i] != m2.pos[i] || m1.size[i] != m2.size[i] { skipped += 1; continue; }
        let (px, py, pw, ph) = crate::classcmp::chart_own_px(m1.pos[i], m1.size[i]);
        if pw == 0 || ph == 0 { continue; }
        let is_tile = m1.binds[i].obj_group_idx / 4 < 4096;
        let off = if is_tile { 0 } else { 2 };
        if let (Some(a), Some(b)) = (fb1.and_then(|v| v.get(i)), fb2.and_then(|v| v.get(i))) { if a == b { fb_same += 1; } else { fb_diff += 1; } }
        for (p, (_, a, b, nch)) in planes.iter().enumerate() {
            for y in 0..ph { for x in 0..pw {
                let (ca, cb) = (a.get(px + x, py + y), b.get(px + x, py + y));
                for c in 0..*nch { acc[p][off + 1] += 1; if ca[c] == cb[c] { acc[p][off] += 1; } }
            } }
        }
    }
    let pct = |e: usize, t: usize| if t == 0 { serde_json::Value::Null } else { serde_json::json!((10000.0 * e as f64 / t as f64).round() / 100.0) };
    let rows: Vec<serde_json::Value> = planes.iter().enumerate().map(|(p, (name, _, _, _))| serde_json::json!({ "plane": name, "tiles_identity_pct": pct(acc[p][0], acc[p][1]), "items_identity_pct": pct(acc[p][2], acc[p][3]), "tiles_bytes": acc[p][1], "items_bytes": acc[p][3] })).collect();
    serde_json::json!({ "planes": rows, "charts_same_rect": n - skipped, "rect_differs": skipped, "fb_same": fb_same, "fb_differs": fb_diff, "fb_differs_pct": pct(fb_diff, fb_same + fb_diff) })
}

fn planes_str(pm: &serde_json::Value) -> String {
    let Some(rows) = pm["planes"].as_array() else { return "—".into() };
    let cell = |v: &serde_json::Value| v.as_f64().map(|x| format!("{x:.1}")).unwrap_or_else(|| "—".into());
    let parts: Vec<String> = rows.iter().filter(|r| r["plane"] != "A").map(|r| format!("{} {}/{}", r["plane"].as_str().unwrap_or("?"), cell(&r["tiles_identity_pct"]), cell(&r["items_identity_pct"]))).collect();
    format!("{} · fb≠ {}", parts.join(" "), cell(&pm["fb_differs_pct"]))
}

/// The lossless-part line of `lmtool filecheck`: chart counts, bind words, rects, entry by entry.
fn layout_metrics(ours: &crate::mapio::MapLightmap, theirs: &crate::mapio::MapLightmap) -> serde_json::Value {
    let (Some(d1), Some(d2)) = (ours.chunk.data.as_ref(), theirs.chunk.data.as_ref()) else { return serde_json::json!({ "error": "a map without a lightmap" }) };
    let (Some(m1), Some(m2)) = (d1.cache.mapping(), d2.cache.mapping()) else { return serde_json::json!({ "error": "a map without a mapping chunk" }) };
    let n = m1.count.min(m2.count) as usize;
    let (mut same_bind, mut same_rect) = (0usize, 0usize);
    for i in 0..n {
        if m1.binds[i].obj_group_idx == m2.binds[i].obj_group_idx && m1.binds[i].obj_idx == m2.binds[i].obj_idx { same_bind += 1; }
        if m1.pos[i] == m2.pos[i] && m1.size[i] == m2.size[i] { same_rect += 1; }
    }
    serde_json::json!({ "charts_ours": m1.count, "charts_theirs": m2.count, "compared": n, "same_binds": same_bind, "same_rects": same_rect,
        "atlas_ours": [m1.atlas_w, m1.atlas_h], "atlas_theirs": [m2.atlas_w, m2.atlas_h], "bbox_equal": m1.bbox_min == m2.bbox_min && m1.bbox_max == m2.bbox_max,
        "head_bytes_ours": m1.head.len(), "head_bytes_theirs": m2.head.len(), "frames_ours": d1.frames.len(), "frames_theirs": d2.frames.len() })
}

pub fn measure(ours_path: &Path, oracle: &Path, records_tsv: Option<&Path>) -> Result<serde_json::Value, String> {
    let ours = crate::mapio::load(&ours_path.to_string_lossy()).map_err(|e| format!("ours: {e}"))?;
    let theirs = crate::mapio::load(&oracle.to_string_lossy()).map_err(|e| format!("oracle: {e}"))?;
    let records = match records_tsv { Some(p) if p.exists() => Some(crate::classcmp::read_records_tsv(&p.to_string_lossy())?), _ => None };
    let layout = layout_metrics(&ours, &theirs);
    let own_rects = layout["same_rects"] != layout["compared"] || layout["charts_ours"] != layout["charts_theirs"];
    let f0 = frame_metrics(&ours, &theirs, records.as_deref(), 0, own_rects)?;
    let f1 = frame_metrics(&ours, &theirs, records.as_deref(), 1, own_rects)?;
    Ok(serde_json::json!({ "layout": layout, "planes": plane_metrics(&ours, &theirs), "frames": [f0, f1] }))
}

/// V2's JOIN FILES: `classcmp --by name --tsv` of frame 0 and frame 1 (+ `--own-rects` when the two layouts' rects differ),
/// written by the classcmp subcommand itself so the trust matrix reads exactly the format it parses (the #record trailer).
fn write_classcmp_tsvs(exe: &Path, ours: &Path, oracle: &Path, records: Option<&Path>, own_rects: bool, wdir: &Path) -> Vec<String> {
    let mut notes = Vec::new();
    for frame in 0..2 {
        let out = wdir.join(format!("classcmp-f{frame}.tsv"));
        let mut a: Vec<String> = vec!["classcmp".into(), ours.to_string_lossy().into(), "--against".into(), oracle.to_string_lossy().into(), "--by".into(), "name".into(), "--frame".into(), frame.to_string(), "--tsv".into(), out.to_string_lossy().into()];
        if let Some(r) = records { if r.exists() { a.push("--records".into()); a.push(r.to_string_lossy().into()); } }
        if own_rects { a.push("--own-rects".into()); }
        match std::process::Command::new(exe).args(&a).output() {
            Ok(o) if o.status.success() => {}
            Ok(o) => notes.push(format!("classcmp f{frame}: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("failed").to_string())),
            Err(e) => notes.push(format!("classcmp f{frame}: spawn {e}")),
        }
    }
    notes
}

/// The reduced-oracle census the matrix shows per row: kept / total items and kept / total EMBEDDED-item lamps (the
/// `lights` subcommand's "MODEL (N placements): L lights" lines; stock items' lights are not in it). A cell without a kept
/// list bakes every item — the census then reads N/N.
pub fn census(exe: &Path, source: &Path, kept: Option<&Path>) -> serde_json::Value {
    let m = tmmaps::map::MapFile::load(source);
    let total_items = m.items.len();
    let kept_set: Option<std::collections::HashSet<usize>> = kept.and_then(|k| std::fs::read_to_string(k).ok()).map(|t| t.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|x| x.trim().parse().ok()).collect());
    let kept_items = kept_set.as_ref().map(|s| s.iter().filter(|&&i| i < total_items).count()).unwrap_or(total_items);
    let mut lights_per_model: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    if let Ok(o) = std::process::Command::new(exe).args(["lights", &source.to_string_lossy()]).output() {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if line.starts_with(' ') { continue; }
            // "AC16497078.Item.Gbx (1 placements): 4 lights, 0 user models, 0 insts"
            if let Some((name, rest)) = line.split_once(" (") {
                if let Some((_, after)) = rest.split_once("): ") {
                    if let Some(n) = after.split(' ').next().and_then(|x| x.parse::<usize>().ok()) { lights_per_model.insert(name.to_string(), n); }
                }
            }
        }
    }
    let (mut total_lamps, mut kept_lamps) = (0usize, 0usize);
    for (i, it) in m.items.iter().enumerate() {
        let l = *lights_per_model.get(&it.model).unwrap_or(&0);
        total_lamps += l;
        if kept_set.as_ref().map_or(true, |s| s.contains(&i)) { kept_lamps += l; }
    }
    // the embedded zip: item files vs textures — a source with items and NO texture bakes its alpha-tested cards OPAQUE
    // (the editor-saved reduced maps, G2 2026-09-28 01:00Z; corpus rule #3: a reduced source must carry its textures)
    let (zip_items, zip_tex) = tmmaps::header::embedded_zip(&m.gbx.body).map(|(_, names)| {
        let lc: Vec<String> = names.iter().map(|n| n.to_ascii_lowercase()).collect();
        (lc.iter().filter(|n| n.ends_with(".item.gbx")).count(), lc.iter().filter(|n| n.ends_with(".dds") || n.ends_with(".texture.gbx") || n.ends_with(".material.gbx")).count())
    }).unwrap_or((0, 0));
    serde_json::json!({ "kept_items": kept_items, "total_items": total_items, "kept_lamps": kept_lamps, "total_lamps": total_lamps, "reduced": kept_set.is_some(), "zip_items": zip_items, "zip_textures": zip_tex, "textures_missing": zip_items > 0 && zip_tex == 0 })
}

fn census_str(c: &serde_json::Value) -> String {
    if c.is_null() { return "—".into(); }
    format!("{}/{} items, {}/{} lamps{}{}", c["kept_items"], c["total_items"], c["kept_lamps"], c["total_lamps"], if c["reduced"].as_bool().unwrap_or(false) { " (reduced)" } else { "" }, if c["textures_missing"].as_bool().unwrap_or(false) { format!(" WARNING {} item files, 0 textures embedded", c["zip_items"]) } else { String::new() })
}

// ---------------------------------------------------------------- run
fn flag(args: &[String], k: &str) -> Option<String> { args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned() }
fn has(args: &[String], k: &str) -> bool { args.iter().any(|x| x == k) }

pub fn run(args: &[String]) -> Result<(), String> {
    let sub = args.get(1).map(|s| s.as_str()).unwrap_or("");
    let refs = flag(args, "--refs").map(PathBuf::from).unwrap_or_else(default_refs);
    let corpus_path = PathBuf::from(flag(args, "--corpus").ok_or("--corpus CORPUS.tsv")?);
    let cells = read_corpus(&corpus_path, &refs)?;
    match sub {
        "list" => {
            println!("{} cells in {}:", cells.len(), corpus_path.display());
            for c in &cells {
                let ok = |p: &Path| if p.exists() { "" } else { "  ← MISSING" };
                let kind = if c.oracle.is_none() { "NO ORACLE" } else if c.compare_only { "compare-only" } else { "bake" };
                println!("  {:<34} {} q{} dirs {} word {} ceiling {} [{kind}]\n      source {}{}{}{}{}", c.name, c.collection, c.quality, c.dirs.as_deref().unwrap_or("full"), c.word.as_deref().unwrap_or("-"), c.ceiling, c.source.display(), ok(&c.source),
                    c.oracle.as_ref().map(|o| format!("\n      oracle {}{}", o.display(), ok(o))).unwrap_or_default(),
                    c.kept.as_ref().map(|k| format!("\n      kept {}{}", k.display(), ok(k))).unwrap_or_default(),
                    if c.env.is_empty() { String::new() } else { format!("\n      env {}", c.env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")) });
            }
            Ok(())
        }
        "run" => run_cells(args, &cells),
        "report" => report(args, &cells),
        "measure" => {
            // lmtool corpus-gate measure --corpus C --tip T --work W [--only cell,…]: (re)measure BANKED bakes (W/T/<cell>/ours.Map.Gbx)
            // with THIS binary's compare — for a cell whose metrics.json is missing or failed (a compare rule that changed, a giant
            // whose layout the old compare refused); the bake bytes are untouched
            let tip = flag(args, "--tip").ok_or("--tip TIP")?;
            let work = PathBuf::from(flag(args, "--work").ok_or("--work W")?).join(&tip);
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let only: Option<Vec<String>> = flag(args, "--only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
            let host = std::env::var("HOSTNAME").unwrap_or_default();
            for c in cells.iter().filter(|c| c.oracle.is_some() && !c.compare_only && only.as_ref().map_or(true, |o| o.contains(&c.name))) {
                let wdir = work.join(&c.name);
                let ours = wdir.join("ours.Map.Gbx");
                let oracle = c.oracle.as_ref().unwrap();
                if !ours.exists() { println!("{}: no banked bake", c.name); continue; }
                let prev: serde_json::Value = std::fs::read_to_string(wdir.join("metrics.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null);
                let bytes = std::fs::read(&ours).map_err(|e| e.to_string())?;
                let md5 = md5_hex(&bytes);
                let records = wdir.join("records.tsv");
                let m = match measure(&ours, oracle, Some(&records)) { Ok(m) => m, Err(e) => { println!("{}: measure failed: {e}", c.name); continue; } };
                let lm_md5 = lightmap_chunk_md5(&ours);
                let own_rects = m["layout"]["same_rects"] != m["layout"]["compared"] || m["layout"]["charts_ours"] != m["layout"]["charts_theirs"];
                let notes = write_classcmp_tsvs(&exe, &ours, oracle, Some(&records), own_rects, &wdir);
                let warn_lines: Vec<String> = std::fs::read_to_string(wdir.join("bake.log")).unwrap_or_default().lines().filter(|l| l.contains("not in any pack")).map(|l| l.trim().chars().take(300).collect()).collect();
                let cen = census(&exe, &c.source, c.kept.as_deref());
                let rec = serde_json::json!({ "cell": c.name, "tip": tip, "host": host, "ok": true, "compare_only": false, "bake_s": prev["bake_s"], "md5": md5, "lm_md5": lm_md5, "bytes": bytes.len(),
                    "collection": c.collection, "quality": c.quality, "dirs": c.dirs.clone().unwrap_or_else(|| "full".into()), "word": c.word, "oracle": oracle.to_string_lossy(), "source": c.source.to_string_lossy(),
                    "env": c.env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>(), "ceiling": c.ceiling, "census": cen, "own_rects_tsv": own_rects, "pak_warnings": warn_lines, "notes": notes, "remeasured": true,
                    "classcmp_tsv": [wdir.join("classcmp-f0.tsv").to_string_lossy(), wdir.join("classcmp-f1.tsv").to_string_lossy()],
                    "layout": m["layout"], "planes": m["planes"], "frames": m["frames"] });
                std::fs::write(wdir.join("metrics.json"), serde_json::to_string_pretty(&rec).unwrap()).map_err(|e| format!("{}: {e}", wdir.display()))?;
                let _ = std::fs::remove_file(wdir.join("failed"));
                let f0 = &rec["frames"][0];
                println!("{}: re-measured — f0 {:.2} % id / {:.2} % ±2 / max {} record {} vs {}; binds {}/{} rects {}/{}{}", c.name, f0["identity_pct"].as_f64().unwrap_or(0.0), f0["within2_pct"].as_f64().unwrap_or(0.0), f0["max_delta"], f0["record_ours"], f0["record_theirs"], rec["layout"]["same_binds"], rec["layout"]["compared"], rec["layout"]["same_rects"], rec["layout"]["compared"], if own_rects { " (own rects)" } else { "" });
            }
            Ok(())
        }
        "probes" => {
            // lmtool corpus-gate probes --corpus C --tip T --work W [--against OLD_TIP] [--only cell,…] [--md OUT.md]: THE PROBE-BLOB
            // COLUMN (V4, 2026-09-28): every banked bake's probe volume vs its oracle's (probecmp::summary — layout, common probes, the
            // four images' identity / ±2 / max |Δ| / ratio, the scale words), and vs the previous tip's bake of the same cell when
            // --against names one (the probe regression detector: "bytes differ, every metric equal" cells now say WHAT moved)
            let tip = flag(args, "--tip").ok_or("--tip TIP")?;
            let work = PathBuf::from(flag(args, "--work").ok_or("--work W")?);
            let against = flag(args, "--against");
            let only: Option<Vec<String>> = flag(args, "--only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
            let mut md = format!("# corpus probe column — tip {tip}{}\n\n| cell | probe volume vs the EDITOR's | vs the previous tip's bake |\n|---|---|---|\n", against.as_ref().map(|a| format!(" (vs {a})")).unwrap_or_default());
            for c in cells.iter().filter(|c| c.oracle.is_some() && !c.compare_only && only.as_ref().map_or(true, |o| o.contains(&c.name))) {
                let ours_p = work.join(&tip).join(&c.name).join("ours.Map.Gbx");
                if !ours_p.exists() { continue; }
                let oracle = c.oracle.as_ref().unwrap();
                let ours = match crate::mapio::load(&ours_p.to_string_lossy()) { Ok(m) => m, Err(e) => { md.push_str(&format!("| {} | load: {e} | |\n", c.name)); continue } };
                let theirs = match crate::mapio::load(&oracle.to_string_lossy()) { Ok(m) => m, Err(e) => { md.push_str(&format!("| {} | oracle: {e} | |\n", c.name)); continue } };
                let vs_editor = match crate::probecmp::summary(&ours, &theirs) { Ok(s) => crate::probecmp::summary_line(&s), Err(e) => format!("probecmp: {e}") };
                let vs_prev = match &against {
                    Some(a) => {
                        let prev_p = work.join(a).join(&c.name).join("ours.Map.Gbx");
                        if !prev_p.exists() { "no bake of this cell at the previous tip".to_string() }
                        else { match crate::mapio::load(&prev_p.to_string_lossy()).and_then(|prev| crate::probecmp::summary(&ours, &prev)) { Ok(s) => { let c0 = &s.images[0]; if s.layout_diffs == 0 && (0..4).all(|k| s.images[k].exact == s.images[k].n) && (0..4).all(|k| s.scales[k].0.map(f32::to_bits) == s.scales[k].1.map(f32::to_bits)) { "IDENTICAL probes".to_string() } else { format!("MOVED: {}; colour bias {:+.2}/{:+.2}/{:+.2} bytes", crate::probecmp::summary_line(&s).replace("editor", "previous"), c0.bias()[0], c0.bias()[1], c0.bias()[2]) } } Err(e) => format!("probecmp vs previous: {e}") } }
                    }
                    None => "—".to_string(),
                };
                println!("{:<40} {vs_editor} | {vs_prev}", c.name);
                md.push_str(&format!("| {} | {} | {} |\n", c.name, vs_editor, vs_prev));
            }
            if let Some(p) = flag(args, "--md") { std::fs::write(&p, &md).map_err(|e| format!("{p}: {e}"))?; println!("→ {p}"); }
            Ok(())
        }
        "census" => {
            // lmtool corpus-gate census --corpus C [--only cell,…]: the reduced-oracle census of every bake cell (kept / total
            // items, kept / total embedded-item lamps) without baking — RE 14's cross-check before a lamp row is read
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let only: Option<Vec<String>> = flag(args, "--only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
            for c in cells.iter().filter(|c| c.oracle.is_some() && !c.compare_only && only.as_ref().map_or(true, |o| o.contains(&c.name))) {
                let cen = census(&exe, &c.source, c.kept.as_deref());
                println!("{:<40} {}", c.name, census_str(&cen));
            }
            Ok(())
        }
        _ => Err("usage: lmtool corpus-gate run|report|list --corpus CORPUS.tsv …".into()),
    }
}

fn run_cells(args: &[String], cells: &[Cell]) -> Result<(), String> {
    let tip = flag(args, "--tip").ok_or("--tip TIP (the base the binary was built from)")?;
    let work = PathBuf::from(flag(args, "--work").ok_or("--work W (the shared results directory)")?).join(&tip);
    let tmp = PathBuf::from(flag(args, "--tmp").unwrap_or_else(|| format!("/tmp/corpus-gate/{tip}")));
    let paks = flag(args, "--paks").map(PathBuf::from).unwrap_or_else(default_paks);
    let boxes: usize = flag(args, "--boxes").map(|v| v.parse().map_err(|e| format!("--boxes: {e}"))).transpose()?.unwrap_or(1);
    let k: usize = flag(args, "--box").map(|v| v.parse().map_err(|e| format!("--box: {e}"))).transpose()?.unwrap_or(0);
    if k >= boxes { return Err("--box K below --boxes N".into()); }
    let only: Option<Vec<String>> = flag(args, "--only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    let force = has(args, "--force");
    let dry = has(args, "--dry-run");
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // --bake-with BIN: the binary under test bakes; this driver (any newer build) measures — the bytes are the bake binary's
    let bake_exe: PathBuf = flag(args, "--bake-with").map(PathBuf::from).unwrap_or_else(|| exe.clone());
    if !bake_exe.exists() { return Err(format!("--bake-with {}: no such file", bake_exe.display())); }
    let host = std::env::var("HOSTNAME").unwrap_or_default();
    let mine: Vec<(usize, &Cell)> = cells.iter().enumerate().filter(|(i, c)| i % boxes == k && only.as_ref().map_or(true, |o| o.contains(&c.name))).collect();
    eprintln!("corpus-gate run: tip {tip}, box {k} of {boxes} ({host}): {} of {} cells; work {}, tmp {}, paks {}", mine.len(), cells.len(), work.display(), tmp.display(), paks.display());
    let t_all = std::time::Instant::now();
    let mut failed = 0usize;
    for (_, c) in mine {
        let wdir = work.join(&c.name);
        let _ = std::fs::create_dir_all(&wdir);
        if wdir.join("metrics.json").exists() && !force { eprintln!("corpus-gate: {}: done already, skipped (--force redoes it)", c.name); continue; }
        // a cell without an oracle is a LINE of the matrix, not a bake
        let Some(oracle) = c.oracle.as_ref() else {
            let rec = serde_json::json!({ "cell": c.name, "tip": tip, "host": host, "ok": false, "no_oracle": true, "collection": c.collection, "quality": c.quality, "word": c.word, "source": c.source.to_string_lossy(), "ceiling": c.ceiling, "error": "no oracle" });
            if !dry { let _ = std::fs::write(wdir.join("metrics.json"), serde_json::to_string_pretty(&rec).unwrap()); }
            eprintln!("corpus-gate: {}: no oracle (a fixed line of the matrix)", c.name);
            continue;
        };
        let cdir = tmp.join(&c.name);
        let _ = std::fs::create_dir_all(&cdir);
        let src_local = cdir.join("source.Map.Gbx");
        let out = if c.compare_only { c.source.clone() } else { cdir.join("ours.Map.Gbx") };
        let records = cdir.join("records.tsv");
        let mut args_bake = bake_args(c, &src_local, &paks, &records, &out);
        if dry {
            if c.compare_only { println!("{}: compare-only — {} vs {}", c.name, c.source.display(), oracle.display()); }
            else { println!("{}: {}{} {}{}", c.name, c.env.iter().map(|(k, v)| format!("{k}={v} ")).collect::<String>(), exe.display(), args_bake.join(" "), c.word.as_ref().map(|w| format!("   [source = daytime-set {} --set {w}]", c.source.display())).unwrap_or_default()); }
            continue;
        }
        if !c.source.exists() { eprintln!("corpus-gate: {}: source {} missing", c.name, c.source.display()); failed += 1; let _ = std::fs::write(wdir.join("failed"), "source missing"); continue; }
        if !oracle.exists() { eprintln!("corpus-gate: {}: oracle {} missing", c.name, oracle.display()); failed += 1; let _ = std::fs::write(wdir.join("failed"), "oracle missing"); continue; }
        let t = std::time::Instant::now();
        let mut bake_s = 0.0f64;
        if !c.compare_only {
            let prep = match &c.word {
                Some(w) => {
                    let o = std::process::Command::new(&exe).args(["daytime-set", &c.source.to_string_lossy(), "--out", &src_local.to_string_lossy(), "--set", w]).output().map_err(|e| e.to_string())?;
                    if o.status.success() { Ok(()) } else { Err(format!("daytime-set: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_string())) }
                }
                None => std::fs::copy(&c.source, &src_local).map(|_| ()).map_err(|e| format!("copy source: {e}")),
            };
            if let Err(e) = prep { eprintln!("corpus-gate: {}: {e}", c.name); failed += 1; let _ = std::fs::write(wdir.join("failed"), e); continue; }
            // the kept list local too (the bake reads it once; the store is slow)
            if let Some(kp) = &c.kept {
                let kl = cdir.join("kept.txt");
                std::fs::copy(kp, &kl).map_err(|e| format!("kept: {e}"))?;
                if let Some(i) = args_bake.iter().position(|x| x == "--kept") { args_bake[i + 1] = kl.to_string_lossy().into(); }
            }
            let _ = std::fs::remove_file(&out);
            eprintln!("corpus-gate: {} ({} q{} dirs {}{}) …", c.name, c.collection, c.quality, c.dirs.as_deref().unwrap_or("full"), if c.env.is_empty() { String::new() } else { format!(", env {}", c.env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")) });
            let mut cmd = std::process::Command::new(&bake_exe);
            cmd.args(&args_bake).env_remove("LMTOOL_BAKE_TIME").env_remove("SOURCE_DATE_EPOCH");
            for (k, v) in &c.env { cmd.env(k, v); }
            let r = cmd.output();
            bake_s = t.elapsed().as_secs_f64();
            let (bake_ok, log) = match &r {
                Ok(o) => (o.status.success() && out.exists(), String::from_utf8_lossy(&o.stderr).to_string()),
                Err(e) => (false, format!("spawn: {e}")),
            };
            let _ = std::fs::write(wdir.join("bake.log"), &log);
            let _ = std::fs::write(wdir.join("bake.cmd"), format!("{}{} {}\n", c.env.iter().map(|(k, v)| format!("{k}={v} ")).collect::<String>(), bake_exe.display(), args_bake.join(" ")));
            if !bake_ok {
                let tail = log.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
                eprintln!("corpus-gate: {}: bake FAILED ({bake_s:.1} s) — {tail}", c.name);
                failed += 1;
                let _ = std::fs::write(wdir.join("failed"), format!("bake FAILED: {tail}\n"));
                let _ = std::fs::write(wdir.join("metrics.json"), serde_json::to_string_pretty(&serde_json::json!({ "cell": c.name, "tip": tip, "host": host, "ok": false, "bake_s": bake_s, "error": tail, "ceiling": c.ceiling })).unwrap());
                continue;
            }
        } else {
            eprintln!("corpus-gate: {} (compare-only: {} vs the oracle) …", c.name, c.source.display());
        }
        // E's WARNING lines (a material in no pack takes the PAD constant): counted, and the cell is flagged
        let warn_lines: Vec<String> = std::fs::read_to_string(wdir.join("bake.log")).unwrap_or_default().lines().filter(|l| l.contains("not in any pack")).map(|l| l.trim().chars().take(300).collect()).collect();
        let bytes = std::fs::read(&out).map_err(|e| e.to_string())?;
        let md5 = md5_hex(&bytes);
        let lm_md5 = lightmap_chunk_md5(&out);
        // bank the bake BEFORE measuring: a measure that fails (a layout the compare refuses) must not lose 25 min of giant
        if !c.compare_only {
            let _ = std::fs::copy(&out, wdir.join("ours.Map.Gbx"));
            let _ = std::fs::copy(&records, wdir.join("records.tsv"));
            let _ = std::fs::write(wdir.join("md5.txt"), format!("{md5}  ours.Map.Gbx\n"));
        }
        let records_opt = if c.compare_only { None } else { Some(records.as_path()) };
        let m = match measure(&out, oracle, records_opt) {
            Ok(m) => m,
            Err(e) => { eprintln!("corpus-gate: {}: measure failed: {e}", c.name); failed += 1; let _ = std::fs::write(wdir.join("failed"), format!("measure: {e}")); continue; }
        };
        let own_rects = m["layout"]["same_rects"] != m["layout"]["compared"] || m["layout"]["charts_ours"] != m["layout"]["charts_theirs"];
        let notes = write_classcmp_tsvs(&exe, &out, oracle, records_opt, own_rects, &wdir);
        let cen = if c.compare_only { serde_json::Value::Null } else { census(&exe, &c.source, c.kept.as_deref()) };
        let rec = serde_json::json!({ "cell": c.name, "tip": tip, "host": host, "ok": true, "compare_only": c.compare_only, "bake_s": bake_s, "md5": md5, "lm_md5": lm_md5, "bytes": bytes.len(),
            "collection": c.collection, "quality": c.quality, "dirs": c.dirs.clone().unwrap_or_else(|| "full".into()), "word": c.word, "oracle": oracle.to_string_lossy(), "source": c.source.to_string_lossy(),
            "env": c.env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>(), "ceiling": c.ceiling, "census": cen, "own_rects_tsv": own_rects, "pak_warnings": warn_lines, "notes": notes,
            "classcmp_tsv": [wdir.join("classcmp-f0.tsv").to_string_lossy(), wdir.join("classcmp-f1.tsv").to_string_lossy()],
            "layout": m["layout"], "planes": m["planes"], "frames": m["frames"] });
        std::fs::write(wdir.join("metrics.json"), serde_json::to_string_pretty(&rec).unwrap()).map_err(|e| format!("{}: {e}", wdir.display()))?;
        let _ = std::fs::remove_file(wdir.join("failed"));
        let f0 = &rec["frames"][0]; let f1 = &rec["frames"][1];
        eprintln!("corpus-gate: {}: ok {bake_s:.1} s md5 {} — f0 {:.2} % id / {:.2} % ±2 / max {} record {} vs {}; f1 {:.2} % id record {} vs {}; binds {}/{}; {}{}", c.name, &md5[..8],
            f0["identity_pct"].as_f64().unwrap_or(0.0), f0["within2_pct"].as_f64().unwrap_or(0.0), f0["max_delta"], f0["record_ours"], f0["record_theirs"],
            f1["identity_pct"].as_f64().unwrap_or(0.0), f1["record_ours"], f1["record_theirs"], rec["layout"]["same_binds"], rec["layout"]["compared"], census_str(&rec["census"]),
            if rec["pak_warnings"].as_array().map_or(false, |w| !w.is_empty()) { format!("; {} 'not in any pack' WARNING line(s)", rec["pak_warnings"].as_array().unwrap().len()) } else { String::new() });
    }
    eprintln!("corpus-gate run: box {k}: done in {:.1} s, {failed} failed", t_all.elapsed().as_secs_f64());
    if failed > 0 { std::process::exit(1); }
    Ok(())
}

// ---------------------------------------------------------------- report
fn load_metrics(work: &Path, tip: &str, cell: &str) -> Option<serde_json::Value> {
    let p = work.join(tip).join(cell).join("metrics.json");
    std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok())
}

fn f(v: &serde_json::Value) -> f64 { v.as_f64().unwrap_or(f64::NAN) }
fn rec(v: &serde_json::Value) -> String { match v.as_f64() { Some(x) => { let s = format!("{:.7}", x); let t = s.trim_end_matches('0').trim_end_matches('.'); if t.is_empty() { "0".into() } else { t.to_string() } } None => "—".into() } }

fn ratio_str(v: &serde_json::Value) -> String {
    match v.as_array() { Some(a) if a.len() == 3 => a.iter().map(|x| x.as_f64().map(|x| format!("{x:.3}")).unwrap_or_else(|| "—".into())).collect::<Vec<_>>().join("/"), _ => "—".into() }
}

/// The class-ratio cells of a frame as (class, [r,g,b]) with NaN for absent.
fn class_ratios(fr: &serde_json::Value) -> Vec<(String, [f64; 3], u64)> {
    fr["classes"].as_array().map(|cs| cs.iter().map(|c| {
        let r = c["ratio"].as_array().map(|a| [f(&a[0]), f(&a[1]), f(&a[2])]).unwrap_or([f64::NAN; 3]);
        (c["class"].as_str().unwrap_or("?").to_string(), r, c["charts"].as_u64().unwrap_or(0))
    }).collect()).unwrap_or_default()
}

struct Target { id: f64, within2: f64, maxd: f64, ratio: f64 }

fn report(args: &[String], cells: &[Cell]) -> Result<(), String> {
    let tip = flag(args, "--tip").ok_or("--tip TIP")?;
    let work = PathBuf::from(flag(args, "--work").ok_or("--work W")?);
    let against = flag(args, "--against");
    // V2's verdict rule: CLOSED (texel) at ≥ 90 % of the cell's CEILING (the editor's own re-bake identity on that map
    // class) with every lit class within --tol; CLOSED (class) when the classes hold but the identity does not; RESIDUE
    // (the worst class named) otherwise. --target ID,WITHIN2,MAXD,RATIO keeps the older absolute rule beside it.
    let tol: f64 = flag(args, "--tol").map(|v| v.parse().map_err(|e| format!("--tol: {e}"))).transpose()?.unwrap_or(0.03);
    let target = match flag(args, "--target") {
        Some(t) => { let v: Vec<f64> = t.split(',').map(|x| x.trim().parse::<f64>().map_err(|e| format!("--target: {e}"))).collect::<Result<_, _>>()?; if v.len() != 4 { return Err("--target ID,WITHIN2,MAXD,RATIO".into()); } Some(Target { id: v[0], within2: v[1], maxd: v[2], ratio: v[3] }) }
        None => None,
    };
    let mut md = String::new();
    md += &format!("# corpus gate — tip {tip}{}\n\n", against.as_ref().map(|a| format!(" vs {a}")).unwrap_or_default());
    md += &format!("verdict: CLOSED (texel) = frame-0 identity ≥ 90 % of the cell's ceiling (the editor's own re-bake identity: 99 lamp-less / 60 lamp maps) AND every lit class (≥ 4 charts) within 1 ± {tol}; CLOSED (class) = the classes hold, the identity does not; RESIDUE = the worst class named.{}\n\n", target.as_ref().map(|t| format!(" absolute target beside it: identity ≥ {:.1} %, ±2 ≥ {:.1} %, max |Δ| ≤ {}, classes ± {}", t.id, t.within2, t.maxd, t.ratio)).unwrap_or_default());
    md += "| cell | coll q dirs | census | bake s | f0 identity % (ceiling) | f0 ±2 % | f0 max\\|Δ\\| | f0 record ours / editor | f0 TOTAL r/g/b | C1–C3 identity % tiles/items · fb≠ charts % | f1 identity % | f1 record ours / editor | binds | rects | head | verdict | vs previous |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n";
    let (mut n_ok, mut n_closed, mut n_moved, mut n_missing, mut n_failed) = (0, 0, 0, 0, 0);
    let mut moved_detail = String::new();
    for c in cells {
        let Some(m) = load_metrics(&work, &tip, &c.name) else { n_missing += 1; md += &format!("| {} | {} q{} {} | | — | | | | | | | | | | | | PENDING | |\n", c.name, c.collection, c.quality, c.dirs.as_deref().unwrap_or("full")); continue; };
        if m["no_oracle"].as_bool().unwrap_or(false) { md += &format!("| {} | {} q{} {} | | — | | | | | | | | | | | | **no oracle** | |\n", c.name, c.collection, c.quality, c.dirs.as_deref().unwrap_or("full")); continue; }
        if !m["ok"].as_bool().unwrap_or(false) { n_failed += 1; md += &format!("| {} | {} q{} {} | | {:.1} | | | | | | | | | | | | **FAILED** {} | |\n", c.name, c.collection, c.quality, c.dirs.as_deref().unwrap_or("full"), f(&m["bake_s"]), m["error"].as_str().unwrap_or("").replace('|', "/")); continue; }
        n_ok += 1;
        let f0 = &m["frames"][0]; let f1 = &m["frames"][1]; let lay = &m["layout"];
        // a one-direction cell is a FRAME-1 / layout row: its verdict is frame 1's (frame 0 with one direction means nothing)
        let d1_cell = m["dirs"].as_str() == Some("1");
        let vf = if d1_cell { f1 } else { f0 };
        let ceiling = m["ceiling"].as_f64().unwrap_or(c.ceiling);
        let ratios0 = class_ratios(vf);
        // the lit classes (≥ 4 charts, a finite ratio): the worst deviation from 1 and its name
        let (worst_ratio, worst_name) = ratios0.iter().filter(|(_, r, ch)| *ch >= 4 && r.iter().all(|x| x.is_finite())).map(|(n, r, _)| (r.iter().map(|x| (x - 1.0).abs()).fold(0.0, f64::max), n.clone())).fold((0.0, String::new()), |a, b| if b.0 > a.0 { b } else { a });
        let id0 = f(&vf["identity_pct"]);
        let classes_ok = worst_ratio <= tol;
        let texel_ok = id0 >= 0.9 * ceiling;
        let abs_ok = target.as_ref().map(|t| id0 >= t.id && f(&f0["within2_pct"]) >= t.within2 && f(&f0["max_delta"]) <= t.maxd && worst_ratio <= t.ratio);
        let verdict = if classes_ok && texel_ok { n_closed += 1; "CLOSED (texel) ✓".to_string() } else if classes_ok { "CLOSED (class)".to_string() } else { format!("RESIDUE {} ±{:.3}", worst_name, worst_ratio) };
        let verdict = if d1_cell { format!("{verdict} (f1)") } else { verdict };
        let verdict = match abs_ok { Some(true) => format!("{verdict} · target ✓"), Some(false) => format!("{verdict} · target ✗"), None => verdict };
        let warn = m["pak_warnings"].as_array().map_or(0, |w| w.len());
        let verdict = if warn > 0 { format!("{verdict} · ⚠ {warn} 'not in any pack'") } else { verdict };
        let head = format!("{} B / {} rec / {} fr", f0["head_ours"][0], f0["head_ours"][1], f0["head_ours"][2]);
        let head_ok = f0["head_ours"] == f0["head_theirs"];
        // vs the previous tip(s): --against A,B,C — the first listed tip that holds a run of THIS cell is its previous
        let prev_hit: Option<(String, serde_json::Value)> = against.as_ref().and_then(|list| list.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()).find_map(|t| load_metrics(&work, t, &c.name).map(|p| (t.to_string(), p))));
        let prev_col = match &against {
            None => String::new(),
            Some(_) => match prev_hit {
                None => "no previous run".to_string(),
                Some((_, p)) if !p["ok"].as_bool().unwrap_or(false) => "previous FAILED".to_string(),
                Some((ptip, p)) => {
                    // the LIGHTMAP CHUNK's md5 decides "same": the whole-file md5 also moves when only the source's embedded zip
                    // rides in the output (the -tex sources, 2026-09-28) — that is not a bake move
                    let same_lm = p["lm_md5"].is_string() && p["lm_md5"] == m["lm_md5"];
                    if p["md5"] == m["md5"] { format!("same bytes (vs {ptip})") } else if same_lm { format!("same LIGHTMAP bytes (vs {ptip}; the file differs outside the lightmap chunk)") } else {
                        n_moved += 1;
                        let mut moves: Vec<String> = Vec::new();
                        for (fi, fr) in [(0usize, f0), (1usize, f1)] {
                            let pf = &p["frames"][fi];
                            let d_id = f(&fr["identity_pct"]) - f(&pf["identity_pct"]);
                            if d_id.abs() >= 0.005 { moves.push(format!("f{fi} identity {:.2} → {:.2} %", f(&pf["identity_pct"]), f(&fr["identity_pct"]))); }
                            let d_w2 = f(&fr["within2_pct"]) - f(&pf["within2_pct"]);
                            if d_w2.abs() >= 0.005 { moves.push(format!("f{fi} ±2 {:.2} → {:.2} %", f(&pf["within2_pct"]), f(&fr["within2_pct"]))); }
                            if fr["max_delta"] != pf["max_delta"] { moves.push(format!("f{fi} max|Δ| {} → {}", pf["max_delta"], fr["max_delta"])); }
                            let (ra, rb) = (f(&pf["record_ours"]), f(&fr["record_ours"]));
                            if (ra - rb).abs() > 1e-7 * ra.abs().max(1e-6) { moves.push(format!("f{fi} record {ra} → {rb}")); }
                            let prev_ratios: std::collections::HashMap<String, [f64; 3]> = class_ratios(pf).into_iter().map(|(n, r, _)| (n, r)).collect();
                            for (name, r, _) in class_ratios(fr) {
                                if let Some(pr) = prev_ratios.get(&name) {
                                    let d = (0..3).map(|i| (r[i] - pr[i]).abs()).filter(|x| x.is_finite()).fold(0.0, f64::max);
                                    if d > 0.0005 { moves.push(format!("f{fi} {name} {} → {}", pr.iter().map(|x| format!("{x:.3}")).collect::<Vec<_>>().join("/"), r.iter().map(|x| format!("{x:.3}")).collect::<Vec<_>>().join("/"))); }
                                }
                            }
                        }
                        if lay["same_binds"] != p["layout"]["same_binds"] || lay["same_rects"] != p["layout"]["same_rects"] || lay["charts_ours"] != p["layout"]["charts_ours"] { moves.push(format!("layout binds {}→{} rects {}→{} charts {}→{}", p["layout"]["same_binds"], lay["same_binds"], p["layout"]["same_rects"], lay["same_rects"], p["layout"]["charts_ours"], lay["charts_ours"])); }
                        if f0["head_ours"] != p["frames"][0]["head_ours"] { moves.push("lossless head".into()); }
                        if m["planes"]["fb_differs"] != p["planes"]["fb_differs"] { moves.push(format!("fb-differs charts {} → {}", p["planes"]["fb_differs"], m["planes"]["fb_differs"])); }
                        if let (Some(a), Some(b)) = (m["planes"]["planes"].as_array(), p["planes"]["planes"].as_array()) { for (x, y) in a.iter().zip(b.iter()) { if x["tiles_identity_pct"] != y["tiles_identity_pct"] || x["items_identity_pct"] != y["items_identity_pct"] { moves.push(format!("plane {} tiles/items identity {}/{} → {}/{}", x["plane"].as_str().unwrap_or("?"), y["tiles_identity_pct"], y["items_identity_pct"], x["tiles_identity_pct"], x["items_identity_pct"])); } } }
                        if moves.is_empty() { moves.push("bytes differ, every metric equal (a probe / trailer / f16-tail change?)".into()); }
                        moved_detail += &format!("- **{}** (vs {ptip}): {}\n", c.name, moves.join("; "));
                        format!("**MOVED** vs {ptip} ({} lines)", moves.len())
                    }
                }
            },
        };
        md += &format!("| {} | {} q{} {}{} | {} | {} | {:.2} ({:.0}) | {:.2} | {} | {} / {} | {} | {} | {:.2} | {} / {} | {}/{} | {}/{} | {}{} | {} | {} |\n", c.name, c.collection, c.quality, m["dirs"].as_str().unwrap_or("full"), if m["compare_only"].as_bool().unwrap_or(false) { " compare" } else { "" }, census_str(&m["census"]), if m["compare_only"].as_bool().unwrap_or(false) { "—".to_string() } else { format!("{:.1}", f(&m["bake_s"])) },
            id0, ceiling, f(&f0["within2_pct"]), f0["max_delta"], rec(&f0["record_ours"]), rec(&f0["record_theirs"]), ratio_str(&f0["ratio"]), planes_str(&m["planes"]),
            f(&f1["identity_pct"]), rec(&f1["record_ours"]), rec(&f1["record_theirs"]), lay["same_binds"], lay["compared"], lay["same_rects"], lay["compared"], head, if head_ok { "" } else { " ≠ editor" }, verdict, prev_col);
    }
    md += &format!("\n{n_ok} baked ({n_closed} CLOSED), {n_failed} failed, {n_missing} pending of {} cells{}\n", cells.len(), against.as_ref().map(|a| format!("; vs {a}: {n_moved} cell(s) MOVED")).unwrap_or_default());
    if !moved_detail.is_empty() { md += &format!("\n## What moved\n{moved_detail}"); }
    // the per-class tables of every baked cell (frame 0, then frame 1 where anything is lit)
    md += "\n## Per-class ratios (frame 0; r/g/b ours / editor over the editor's lit texels; identity % of the class's bytes)\n";
    for c in cells {
        let Some(m) = load_metrics(&work, &tip, &c.name) else { continue };
        if !m["ok"].as_bool().unwrap_or(false) { continue; }
        for fi in 0..2 {
            let fr = &m["frames"][fi];
            if fi == 1 && f(&fr["lit_theirs_pct"]) < 0.01 && f(&fr["lit_ours_pct"]) < 0.01 { continue; }
            md += &format!("\n**{}** frame {fi}: ", c.name);
            let mut parts: Vec<String> = Vec::new();
            for cl in fr["classes"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                parts.push(format!("{} ({}) {} id {:.1} %", cl["class"].as_str().unwrap_or("?"), cl["charts"], ratio_str(&cl["ratio"]), f(&cl["identity_pct"])));
            }
            md += &parts.join(" · ");
            md += "\n";
        }
    }
    println!("{md}");
    if let Some(p) = flag(args, "--md") { std::fs::write(&p, &md).map_err(|e| format!("{p}: {e}"))?; }
    let json = work.join(&tip).join("corpus-gate-report.json");
    let rows: Vec<serde_json::Value> = cells.iter().filter_map(|c| load_metrics(&work, &tip, &c.name)).collect();
    let _ = std::fs::write(&json, serde_json::to_string_pretty(&serde_json::json!({ "tip": tip, "against": against, "cells": rows })).unwrap());
    if n_moved > 0 || n_failed > 0 { std::process::exit(1); }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn md5_known_answers() {
        assert_eq!(super::md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(super::md5_hex(b"The quick brown fox jumps over the lazy dog"), "9e107d9d372bb6826bd81d3542a419d6");
        assert_eq!(super::md5_hex(&[b'a'; 1000]), "cabe45dcc9ae5b66ba86600cca6b8ba8");
    }

    #[test]
    fn corpus_line_parses_and_bakes_the_product_line() {
        let dir = std::env::temp_dir().join(format!("corpusgate-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("c.tsv");
        std::fs::write(&p, "cell\tsource\toracle\tcollection\tquality\tword\tdirs\tkept\textra\n# a comment\nstpad-night\tstpad-night-0x199a-source.Map.Gbx\tstpad-Stadium-Night-0x199a-q3-editor-nocache.Map.Gbx\tStadium\t3\t-\t1\t-\t-\ntiny16\tt16.Map.Gbx\tt16-ed.Map.Gbx\tBlueBay\t4\t0xdaab\tfull\tkept.txt\t--zone Sea\n").unwrap();
        let cells = super::read_corpus(&p, std::path::Path::new("/refs")).unwrap();
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].dirs.as_deref(), Some("1"));
        assert!(cells[0].word.is_none() && cells[0].kept.is_none());
        assert_eq!(cells[1].word.as_deref(), Some("0xdaab"));
        assert!(cells[1].dirs.is_none());
        assert_eq!(cells[1].kept.as_deref(), Some(std::path::Path::new("/refs/kept.txt")));
        let a = super::bake_args(&cells[1], std::path::Path::new("/t/src.Map.Gbx"), std::path::Path::new("/paks"), std::path::Path::new("/t/r.tsv"), std::path::Path::new("/t/o.Map.Gbx"));
        let s = a.join(" ");
        assert!(s.contains("--pak /paks/BlueBay.pak:660C4C156B80337E296A1034B0AA05B8 --pak /paks/Stadium.pak:B773D73047A4104857722366D78D28A6 --pak /paks/Maniaplanet.pak:9A93723447347A8CE336CCFC49E65449"), "{s}");
        assert!(s.contains("--kept /refs/kept.txt --zone Sea --records-tsv /t/r.tsv --out /t/o.Map.Gbx"), "{s}");
        assert!(!s.contains("--max-dirs"));
        let a0 = super::bake_args(&cells[0], std::path::Path::new("/t/s"), std::path::Path::new("/paks"), std::path::Path::new("/t/r"), std::path::Path::new("/t/o"));
        let s0 = a0.join(" ");
        assert!(s0.contains("--pak /paks/Stadium.pak:") && !s0.contains("BlueBay") && s0.contains("--max-dirs 1"), "{s0}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The md5 of a written map's LIGHTMAP CHUNK payload alone (chunk 0x0304305B as stored) — the identity the corpus compares:
/// the whole-file md5 also moves when the source's embedded zip (textures) rides unchanged into the output.
pub fn lightmap_chunk_md5(path: &Path) -> serde_json::Value {
    match crate::mapio::load(&path.to_string_lossy()) {
        Ok(m) => { let (_, p, n) = m.at; serde_json::Value::String(md5_hex(&m.gbx.body[p..p + n])) }
        Err(_) => serde_json::Value::Null,
    }
}
