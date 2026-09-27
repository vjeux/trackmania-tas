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
//! --pak Stadium --pak Maniaplanet [--kept K] [--max-dirs N] [extra] --records-tsv … --out …` with LMTOOL_BAKE_TIME pinned
//! (the writer is deterministic: two runs of one binary give one md5, so "the cell moved" = "the md5 moved", and the
//! metrics say WHAT moved). Cells are dealt to boxes round-robin by their order in the file (`--box K --boxes N`);
//! every box writes W/TIP/<cell>/{ours.Map.Gbx, records.tsv, bake.log, metrics.json}, the report reads W.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Cell {
    pub name: String,
    pub source: PathBuf,
    pub oracle: PathBuf,
    pub collection: String,
    pub quality: String,
    pub word: Option<String>,
    pub dirs: Option<String>,
    pub kept: Option<PathBuf>,
    pub extra: Vec<String>,
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
        cells.push(Cell {
            name: cols[0].to_string(),
            source: resolve(refs, cols[1]),
            oracle: resolve(refs, cols[2]),
            collection: cols[3].to_string(),
            quality: cols[4].to_string(),
            word: opt(5),
            dirs: opt(6).filter(|d| d != "full"),
            kept: opt(7).map(|k| resolve(refs, &k)),
            extra: opt(8).map(|e| e.split_whitespace().map(|s| s.to_string()).collect()).unwrap_or_default(),
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
fn frame_metrics(ours: &crate::mapio::MapLightmap, theirs: &crate::mapio::MapLightmap, records: Option<&[crate::classcmp::RecRow]>, frame: usize) -> Result<serde_json::Value, String> {
    let o = crate::classcmp::Options { frame, lit: 8, by: crate::classcmp::GroupBy::Class, worst: 0, own_rects: false };
    let r = crate::classcmp::compare(ours, theirs, records, &o)?;
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
        "unmatched_rows": r.unmatched_rows, "rect_mismatch": r.rect_mismatch, "classes": classes,
    }))
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
    let f0 = frame_metrics(&ours, &theirs, records.as_deref(), 0)?;
    let f1 = frame_metrics(&ours, &theirs, records.as_deref(), 1)?;
    Ok(serde_json::json!({ "layout": layout_metrics(&ours, &theirs), "frames": [f0, f1] }))
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
                println!("  {:<28} {} q{} dirs {} word {}\n      source {}{}\n      oracle {}{}{}", c.name, c.collection, c.quality, c.dirs.as_deref().unwrap_or("full"), c.word.as_deref().unwrap_or("-"), c.source.display(), ok(&c.source), c.oracle.display(), ok(&c.oracle),
                    c.kept.as_ref().map(|k| format!("\n      kept {}{}", k.display(), ok(k))).unwrap_or_default());
            }
            Ok(())
        }
        "run" => run_cells(args, &cells),
        "report" => report(args, &cells),
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
    let host = std::env::var("HOSTNAME").unwrap_or_default();
    let mine: Vec<(usize, &Cell)> = cells.iter().enumerate().filter(|(i, c)| i % boxes == k && only.as_ref().map_or(true, |o| o.contains(&c.name))).collect();
    eprintln!("corpus-gate run: tip {tip}, box {k} of {boxes} ({host}): {} of {} cells; work {}, tmp {}, paks {}", mine.len(), cells.len(), work.display(), tmp.display(), paks.display());
    let t_all = std::time::Instant::now();
    let mut failed = 0usize;
    for (_, c) in mine {
        let wdir = work.join(&c.name);
        let _ = std::fs::create_dir_all(&wdir);
        if wdir.join("metrics.json").exists() && !force { eprintln!("corpus-gate: {}: done already, skipped (--force redoes it)", c.name); continue; }
        let cdir = tmp.join(&c.name);
        let _ = std::fs::create_dir_all(&cdir);
        // the source copy (local; the word set when the cell names one)
        let src_local = cdir.join("source.Map.Gbx");
        let out = cdir.join("ours.Map.Gbx");
        let records = cdir.join("records.tsv");
        let mut args_bake = bake_args(c, &src_local, &paks, &records, &out);
        if dry {
            println!("{}: {} {}{}", c.name, exe.display(), args_bake.join(" "), c.word.as_ref().map(|w| format!("   [source = daytime-set {} --set {w}]", c.source.display())).unwrap_or_default());
            continue;
        }
        if !c.source.exists() { eprintln!("corpus-gate: {}: source {} missing", c.name, c.source.display()); failed += 1; let _ = std::fs::write(wdir.join("failed"), "source missing"); continue; }
        if !c.oracle.exists() { eprintln!("corpus-gate: {}: oracle {} missing", c.name, c.oracle.display()); failed += 1; let _ = std::fs::write(wdir.join("failed"), "oracle missing"); continue; }
        let t = std::time::Instant::now();
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
        eprintln!("corpus-gate: {} ({} q{} dirs {}) …", c.name, c.collection, c.quality, c.dirs.as_deref().unwrap_or("full"));
        let r = std::process::Command::new(&exe).args(&args_bake).env("LMTOOL_BAKE_TIME", "1790000000").output();
        let bake_s = t.elapsed().as_secs_f64();
        let (bake_ok, log) = match &r {
            Ok(o) => (o.status.success() && out.exists(), String::from_utf8_lossy(&o.stderr).to_string()),
            Err(e) => (false, format!("spawn: {e}")),
        };
        let _ = std::fs::write(wdir.join("bake.log"), &log);
        let _ = std::fs::write(wdir.join("bake.cmd"), format!("{} {}\n", exe.display(), args_bake.join(" ")));
        if !bake_ok {
            let tail = log.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
            eprintln!("corpus-gate: {}: bake FAILED ({bake_s:.1} s) — {tail}", c.name);
            failed += 1;
            let _ = std::fs::write(wdir.join("failed"), format!("bake FAILED: {tail}\n"));
            let _ = std::fs::write(wdir.join("metrics.json"), serde_json::to_string_pretty(&serde_json::json!({ "cell": c.name, "tip": tip, "host": host, "ok": false, "bake_s": bake_s, "error": tail })).unwrap());
            continue;
        }
        let bytes = std::fs::read(&out).map_err(|e| e.to_string())?;
        let md5 = md5_hex(&bytes);
        let m = match measure(&out, &c.oracle, Some(&records)) {
            Ok(m) => m,
            Err(e) => { eprintln!("corpus-gate: {}: measure failed: {e}", c.name); failed += 1; let _ = std::fs::write(wdir.join("failed"), format!("measure: {e}")); continue; }
        };
        let rec = serde_json::json!({ "cell": c.name, "tip": tip, "host": host, "ok": true, "bake_s": bake_s, "md5": md5, "bytes": bytes.len(),
            "collection": c.collection, "quality": c.quality, "dirs": c.dirs.clone().unwrap_or_else(|| "full".into()), "word": c.word, "oracle": c.oracle.to_string_lossy(), "source": c.source.to_string_lossy(),
            "layout": m["layout"], "frames": m["frames"] });
        // bank: the map, the records, the metrics (the log went already)
        let _ = std::fs::copy(&out, wdir.join("ours.Map.Gbx"));
        let _ = std::fs::copy(&records, wdir.join("records.tsv"));
        std::fs::write(wdir.join("metrics.json"), serde_json::to_string_pretty(&rec).unwrap()).map_err(|e| format!("{}: {e}", wdir.display()))?;
        let _ = std::fs::remove_file(wdir.join("failed"));
        let f0 = &rec["frames"][0]; let f1 = &rec["frames"][1];
        eprintln!("corpus-gate: {}: ok {bake_s:.1} s md5 {} — f0 {:.2} % id / {:.2} % ±2 / max {} record {} vs {}; f1 {:.2} % id record {} vs {}; binds {}/{}", c.name, &md5[..8],
            f0["identity_pct"].as_f64().unwrap_or(0.0), f0["within2_pct"].as_f64().unwrap_or(0.0), f0["max_delta"], f0["record_ours"], f0["record_theirs"],
            f1["identity_pct"].as_f64().unwrap_or(0.0), f1["record_ours"], f1["record_theirs"], rec["layout"]["same_binds"], rec["layout"]["compared"]);
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
    let target = match flag(args, "--target") {
        Some(t) => { let v: Vec<f64> = t.split(',').map(|x| x.trim().parse::<f64>().map_err(|e| format!("--target: {e}"))).collect::<Result<_, _>>()?; if v.len() != 4 { return Err("--target ID,WITHIN2,MAXD,RATIO".into()); } Target { id: v[0], within2: v[1], maxd: v[2], ratio: v[3] } }
        None => Target { id: 60.0, within2: 92.0, maxd: 43.0, ratio: 0.004 },
    };
    let mut md = String::new();
    md += &format!("# corpus gate — tip {tip}{}\n\n", against.as_ref().map(|a| format!(" vs {a}")).unwrap_or_default());
    md += &format!("target (CLOSED): frame-0 identity ≥ {:.1} %, within ±2 ≥ {:.1} %, max |Δ| ≤ {}, every class ratio within 1 ± {}\n\n", target.id, target.within2, target.maxd, target.ratio);
    md += "| cell | coll q dirs | bake s | f0 identity % | f0 ±2 % | f0 max\\|Δ\\| | f0 record ours / editor | f0 TOTAL r/g/b | f1 identity % | f1 record ours / editor | binds | rects | head | verdict | vs previous |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n";
    let (mut n_ok, mut n_closed, mut n_moved, mut n_missing, mut n_failed) = (0, 0, 0, 0, 0);
    let mut moved_detail = String::new();
    for c in cells {
        let Some(m) = load_metrics(&work, &tip, &c.name) else { n_missing += 1; md += &format!("| {} | {} q{} {} | — | | | | | | | | | | | PENDING | |\n", c.name, c.collection, c.quality, c.dirs.as_deref().unwrap_or("full")); continue; };
        if !m["ok"].as_bool().unwrap_or(false) { n_failed += 1; md += &format!("| {} | {} q{} {} | {:.1} | | | | | | | | | | | **FAILED** {} | |\n", c.name, c.collection, c.quality, c.dirs.as_deref().unwrap_or("full"), f(&m["bake_s"]), m["error"].as_str().unwrap_or("").replace('|', "/")); continue; }
        n_ok += 1;
        let f0 = &m["frames"][0]; let f1 = &m["frames"][1]; let lay = &m["layout"];
        let ratios0 = class_ratios(f0);
        let worst_ratio = ratios0.iter().filter(|(_, r, ch)| *ch >= 4 && r.iter().all(|x| x.is_finite())).map(|(_, r, _)| r.iter().map(|x| (x - 1.0).abs()).fold(0.0, f64::max)).fold(0.0, f64::max);
        let closed = f(&f0["identity_pct"]) >= target.id && f(&f0["within2_pct"]) >= target.within2 && f(&f0["max_delta"]) <= target.maxd && worst_ratio <= target.ratio;
        if closed { n_closed += 1; }
        let head = format!("{} B / {} rec / {} fr", f0["head_ours"][0], f0["head_ours"][1], f0["head_ours"][2]);
        let head_ok = f0["head_ours"] == f0["head_theirs"];
        let verdict = if closed { "CLOSED ✓".to_string() } else { format!("open (worst class ±{:.3})", worst_ratio) };
        // vs the previous tip
        let prev_col = match &against {
            None => String::new(),
            Some(prev) => match load_metrics(&work, prev, &c.name) {
                None => "no previous run".to_string(),
                Some(p) if !p["ok"].as_bool().unwrap_or(false) => "previous FAILED".to_string(),
                Some(p) => {
                    if p["md5"] == m["md5"] { "same bytes".to_string() } else {
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
                        if moves.is_empty() { moves.push("bytes differ, every metric equal (a probe / trailer / f16-tail change?)".into()); }
                        moved_detail += &format!("- **{}**: {}\n", c.name, moves.join("; "));
                        format!("**MOVED** ({} lines)", moves.len())
                    }
                }
            },
        };
        md += &format!("| {} | {} q{} {} | {:.1} | {:.2} | {:.2} | {} | {} / {} | {} | {:.2} | {} / {} | {}/{} | {}/{} | {}{} | {} | {} |\n", c.name, c.collection, c.quality, m["dirs"].as_str().unwrap_or("full"), f(&m["bake_s"]),
            f(&f0["identity_pct"]), f(&f0["within2_pct"]), f0["max_delta"], f0["record_ours"], f0["record_theirs"], ratio_str(&f0["ratio"]),
            f(&f1["identity_pct"]), f1["record_ours"], f1["record_theirs"], lay["same_binds"], lay["compared"], lay["same_rects"], lay["compared"], head, if head_ok { "" } else { " ≠ editor" }, verdict, prev_col);
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
