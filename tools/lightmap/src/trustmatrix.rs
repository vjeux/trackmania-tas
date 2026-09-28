//! `lmtool trustmatrix MANIFEST.tsv [--out MATRIX.md] [--tol 0.03] [--min-texels 500]` — the TRUST MATRIX artifact (V2, 2026-09-27):
//! one line per corpus cell, its numbers read from the row's `classcmp --tsv` table, its state by rule. The manifest is a
//! tab-separated table with a header (columns by name, `#` lines ignored):
//!
//!   cell  collection  mood  word  quality  map  features  class_tsv  state  cause
//!
//! `class_tsv` = the classcmp table of the row (`-` when none exists yet); `state` overrides the rule (`closed`, `residue`,
//! `open`, `no-oracle`; empty = by rule); `cause` is free text carried into the table. The rule (from the editor's own
//! run-to-run ceiling, VALIDATION.md V2-5: two editor bakes agree to ~60 % of bytes, ~92 % within ±2, every class 1.000):
//! worst class = the largest |ratio − 1| over the classes with ≥ `--min-texels` LIT oracle texels (tiny or unlit classes are noise);
//! CLOSED-texel when the worst class is within `--tol` AND identity ≥ 90 % of the cell's `ceiling` (the editor's own re-bake identity on
//! that map class: 95–99 % lamp-less, ~60 % with lamps) AND within ±2 ≥ 90 %; CLOSED-class when only
//! the worst class is within `--tol`; RESIDUE otherwise (the worst class named); OPEN when the cell has no table;
//! NO ORACLE when marked. An `--own-rects` table (the `#record` line's last field) has no byte identity — the identity
//! columns print `—` and the texel verdict is not available for it.

#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub cell: String,
    pub collection: String,
    pub mood: String,
    pub word: String,
    pub quality: String,
    pub map: String,
    pub features: String,
    pub class_tsv: String,
    pub state: String,
    pub cause: String,
    /// the editor's own run-to-run byte identity on this cell's map class (%; the `ceiling` column; 99 when absent — lamp-less maps re-bake to 95–99 %, lamp maps to ~60 %)
    pub ceiling: f64,
    /// V4 (2026-09-28): the corpus-gate cell this matrix cell reads (`corpus_cell` column; empty = the table is hand-made or none) —
    /// `--refresh W/TIP` re-reads W/TIP/<corpus_cell>/ours.Map.Gbx against its oracle and rewrites `class_tsv`
    pub corpus_cell: String,
    /// the lightmap frame the cell reads (`frame` column; 0 = the colour frame, 1 = the lamp frame — ST-frame1)
    pub frame: usize,
    /// the probe frame "P" text (`probes` column): "**P-STATE** reason · summary", written by --refresh
    pub probes: String,
}

/// One class row of a classcmp table.
#[derive(Clone, Debug)]
pub struct ClassRow {
    pub class: String,
    pub texels: usize,
    /// the oracle's lit texels of the class (texels × lit_editor_pct / 100) — the mean runs over these
    pub lit_texels: usize,
    pub ratio: [f64; 3],
    pub identical: f64,
    pub within1: f64,
    pub within2: f64,
    pub max_delta: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    pub rows: Vec<ClassRow>,
    pub total: Option<ClassRow>,
    /// (frame, record ours, record editor, ratio, own_rects) from the `#record` line when present
    pub record: Option<(usize, f64, f64, f64, bool)>,
}

/// The manifest's `#!` lines: the version notes rendered under the matrix title (what CHANGED in the comparison from version to version).
pub fn read_notes(path: &str) -> Vec<String> {
    std::fs::read_to_string(path).map(|t| t.lines().filter(|l| l.starts_with("#!")).map(|l| l.trim_start_matches("#!").trim().to_string()).collect()).unwrap_or_default()
}

pub fn read_manifest(path: &str) -> Result<Vec<Cell>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut header: Option<Vec<String>> = None;
    let mut out = Vec::new();
    for (ln, line) in txt.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') { continue; }
        let f: Vec<&str> = line.split('\t').collect();
        let Some(h) = &header else { header = Some(f.iter().map(|s| s.trim().to_string()).collect()); continue };
        let get = |name: &str| h.iter().position(|c| c == name).and_then(|i| f.get(i)).map(|s| s.trim().to_string()).unwrap_or_default();
        if get("cell").is_empty() { return Err(format!("{path}:{}: a row without a cell id", ln + 1)); }
        out.push(Cell { cell: get("cell"), collection: get("collection"), mood: get("mood"), word: get("word"), quality: get("quality"), map: get("map"), features: get("features"), class_tsv: get("class_tsv"), state: get("state").to_ascii_lowercase(), cause: get("cause"), ceiling: get("ceiling").parse().unwrap_or(99.0),
            corpus_cell: { let c = get("corpus_cell"); if c == "-" { String::new() } else { c } }, frame: get("frame").parse().unwrap_or(0), probes: get("probes") });
    }
    if header.is_none() { return Err(format!("{path}: no header line")); }
    Ok(out)
}

/// One cell's move under `--refresh`: the old table's headline vs the new one (None = no old table).
pub struct Refreshed {
    pub cell: String,
    pub tsv: String,
    pub own_rects: bool,
    pub old: Option<(f64, f64, f64, [f64; 3])>,
    pub new: (f64, f64, f64, [f64; 3]),
}

fn headline(t: &Table) -> (f64, f64, f64, [f64; 3]) {
    let tot = t.total.as_ref();
    (tot.map(|r| r.identical).unwrap_or(f64::NAN), tot.map(|r| r.within2).unwrap_or(f64::NAN), t.record.map(|r| r.3).unwrap_or(f64::NAN), tot.map(|r| r.ratio).unwrap_or([f64::NAN; 3]))
}

/// V4's REFRESH (2026-09-28): the matrix re-read from the corpus bank in one command. For every manifest row with a `corpus_cell`,
/// `work_tip/<corpus_cell>/ours.Map.Gbx` (baker's banked bake of the base) is compared against the oracle its metrics.json names
/// — `classcmp --by name --frame F --records records.tsv [--own-rects] [--lit-hdr F]` in-process — and the table is written beside the
/// manifest as `<cell><suffix>.tsv`; the manifest's `class_tsv` column is rewritten in place (comment lines, `#!` notes and the other
/// columns untouched). The gate's own tables are BYTE-lit (`lit 8`): on the giants and the islands the HDR floor changes the lit
/// fraction and the class means (g23 tiles 28 vs 63 % at 1e-3 against 77 vs 96 % by byte, V3-5), so the matrix reads its own
/// tables with `--lit-hdr 1e-3` rather than the gate's. `--own-rects` is decided from the two layouts BEFORE the compare (V3
/// 03:56Z: equal chart counts with 215 of 5 871 same rects must not pass as a same-rect compare). Cells without a banked bake are
/// reported and left as they were.
/// `causes`: (cell, text) pairs that REPLACE the row's cause column (a text starting with `+` is PREPENDED to the old cause with " · " —
/// the per-base note in front, the history behind it), read from `--causes FILE.tsv` (cell TAB text, `#` comments).
pub fn refresh(manifest: &str, work_tip: Option<&std::path::Path>, suffix: &str, lit_hdr: Option<f64>, only: Option<&[String]>, bind: &[(String, String, usize)], causes: &[(String, String)]) -> Result<(Vec<Refreshed>, Vec<String>), String> {
    let txt = std::fs::read_to_string(manifest).map_err(|e| format!("{manifest}: {e}"))?;
    let base_dir = std::path::Path::new(manifest).parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut header: Option<Vec<String>> = None;
    let mut out_lines: Vec<String> = Vec::new();
    let mut done: Vec<Refreshed> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut probes_pending: Vec<(String, String)> = Vec::new();
    for line in txt.lines() {
        if line.trim().is_empty() || line.starts_with('#') { out_lines.push(line.to_string()); continue; }
        let mut f: Vec<String> = line.split('\t').map(|s| s.to_string()).collect();
        let Some(h) = &header else {
            // `--bind` adds the two V4 columns to a manifest that lacks them
            let mut h: Vec<String> = f.iter().map(|s| s.trim().to_string()).collect();
            if !bind.is_empty() { for c in ["corpus_cell", "frame"] { if !h.iter().any(|x| x == c) { h.push(c.to_string()); } } }
            out_lines.push(h.join("\t"));
            header = Some(h);
            continue
        };
        let col = |name: &str| h.iter().position(|c| c == name);
        while f.len() < h.len() { f.push(String::new()); }
        let get = |f: &Vec<String>, name: &str| col(name).and_then(|i| f.get(i)).map(|s| s.trim().to_string()).unwrap_or_default();
        let cell = get(&f, "cell");
        if let Some((_, cc, fr)) = bind.iter().find(|(c, _, _)| c == &cell) {
            if let Some(i) = col("corpus_cell") { f[i] = cc.clone(); }
            if let Some(i) = col("frame") { f[i] = fr.to_string(); }
        }
        if let Some((_, text)) = causes.iter().find(|(c, _)| c == &cell) {
            if let Some(i) = col("cause") {
                let old = f[i].trim().to_string();
                f[i] = match text.strip_prefix('+') { Some(t) if !old.is_empty() => format!("{} · {old}", t.trim()), Some(t) => t.trim().to_string(), None => text.trim().to_string() };
            }
        }
        let cc = get(&f, "corpus_cell");
        let wanted = only.map_or(true, |o| o.iter().any(|x| x == &cell));
        let Some(work_tip) = work_tip else { out_lines.push(f.join("\t")); continue };
        if cc.is_empty() || cc == "-" || !wanted { out_lines.push(f.join("\t")); continue; }
        let Some(tsv_col) = col("class_tsv") else { return Err(format!("{manifest}: no class_tsv column")); };
        let wdir = work_tip.join(&cc);
        let ours_p = wdir.join("ours.Map.Gbx");
        if !ours_p.exists() { skipped.push(format!("{cell}: no banked bake at {}", ours_p.display())); out_lines.push(f.join("\t")); continue; }
        let metrics: serde_json::Value = std::fs::read_to_string(wdir.join("metrics.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null);
        let Some(oracle) = metrics["oracle"].as_str().map(|s| s.to_string()) else { skipped.push(format!("{cell}: {}/metrics.json names no oracle", wdir.display())); out_lines.push(f.join("\t")); continue; };
        let frame: usize = get(&f, "frame").parse().unwrap_or(0);
        let ours = crate::mapio::load(&ours_p.to_string_lossy()).map_err(|e| format!("{cell}: ours: {e}"))?;
        let theirs = crate::mapio::load(&oracle).map_err(|e| format!("{cell}: oracle {oracle}: {e}"))?;
        let records_p = wdir.join("records.tsv");
        let records = if records_p.exists() { Some(crate::classcmp::read_records_tsv(&records_p.to_string_lossy())?) } else { None };
        // own rects when the two layouts differ (chart counts, or the same count with other rects) — decided from the layouts
        let own_rects = {
            let (Some(d1), Some(d2)) = (ours.chunk.data.as_ref(), theirs.chunk.data.as_ref()) else { return Err(format!("{cell}: a map without a lightmap")) };
            let (Some(m1), Some(m2)) = (d1.cache.mapping(), d2.cache.mapping()) else { return Err(format!("{cell}: a map without a mapping chunk")) };
            let n = m1.count.min(m2.count) as usize;
            let same_rect = (0..n).filter(|&i| m1.pos[i] == m2.pos[i] && m1.size[i] == m2.size[i]).count();
            m1.count != m2.count || same_rect != n
        };
        let o = crate::classcmp::Options { frame, lit: 8, lit_hdr, by: crate::classcmp::GroupBy::Name, worst: 0, own_rects, peaks: 0 };
        let r = crate::classcmp::compare(&ours, &theirs, records.as_deref(), &o).map_err(|e| format!("{cell}: classcmp: {e}"))?;
        let tsv_name = format!("{cell}{suffix}.tsv");
        let tsv_path = base_dir.join(&tsv_name);
        crate::classcmp::print(&r, &o, Some(&tsv_path.to_string_lossy())).map_err(|e| format!("{cell}: {e}"))?;
        let old_tsv = f.get(tsv_col).map(|s| s.trim().to_string()).unwrap_or_default();
        let old = if old_tsv.is_empty() || old_tsv == "-" { None } else { read_table(&old_tsv, &base_dir).ok().map(|t| headline(&t)) };
        let new = read_table(&tsv_name, &base_dir).map(|t| headline(&t))?;
        while f.len() <= tsv_col { f.push(String::new()); }
        f[tsv_col] = tsv_name.clone();
        // THE PROBE FRAME "P" (the coordinator, 2026-09-28 08:48 PT): the cell's probe volume vs the oracle's, its state by the same
        // vocabulary; written into the manifest's `probes` column (added when absent) as "**P-STATE** reason · summary"
        let probes_txt = match crate::probecmp::summary(&ours, &theirs) {
            Ok(s) => {
                let lamps = s.images[3].mean_theirs().iter().sum::<f64>() > 1.5 || s.scales[2].1.map(|v| v > 1e-4).unwrap_or(false);
                let ceiling: f64 = get(&f, "ceiling").parse().unwrap_or(99.0);
                let p_ceiling = if ceiling >= 90.0 { 62.5 } else { 72.2 };
                let (state, reason) = crate::probecmp::p_state(&s, lamps, p_ceiling);
                format!("**{state}** {reason} · {}", crate::probecmp::summary_line(&s))
            }
            Err(e) => format!("**P-OPEN** {e}"),
        };
        match col("probes") { Some(i) => { while f.len() <= i { f.push(String::new()); } f[i] = probes_txt; } None => probes_pending.push((cell.clone(), probes_txt)) }
        out_lines.push(f.join("\t"));
        done.push(Refreshed { cell, tsv: tsv_name, own_rects, old, new });
    }
    if header.is_none() { return Err(format!("{manifest}: no header line")); }
    // a manifest without the `probes` column gets it appended (header + every data row) and the pending texts filled in
    if !probes_pending.is_empty() {
        let mut seen_header = false;
        for line in out_lines.iter_mut() {
            if line.trim().is_empty() || line.starts_with('#') { continue; }
            if !seen_header { seen_header = true; line.push_str("\tprobes"); continue; }
            let cell = line.split('\t').next().unwrap_or("").trim().to_string();
            let txt = probes_pending.iter().find(|(c, _)| c == &cell).map(|(_, t)| t.clone()).unwrap_or_default();
            line.push('\t'); line.push_str(&txt);
        }
    }
    std::fs::write(manifest, out_lines.join("\n") + "\n").map_err(|e| format!("{manifest}: {e}"))?;
    Ok((done, skipped))
}

fn parse_rgb(s: &str) -> Option<[f64; 3]> {
    let v: Vec<f64> = s.split('/').map(|x| x.trim().parse::<f64>().unwrap_or(f64::NAN)).collect();
    if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None }
}

/// Read a `classcmp --tsv` table (relative paths resolve against `base_dir`).
pub fn read_table(path: &str, base_dir: &std::path::Path) -> Result<Table, String> {
    let p = if std::path::Path::new(path).is_absolute() { std::path::PathBuf::from(path) } else { base_dir.join(path) };
    let txt = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut t = Table::default();
    for line in txt.lines() {
        if line.starts_with("#record") {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() >= 5 {
                let frame = f[1].trim().parse().unwrap_or(0);
                let (a, b) = (f[2].trim().parse().unwrap_or(f64::NAN), f[3].trim().parse().unwrap_or(f64::NAN));
                let ratio = f[4].trim().parse().unwrap_or(f64::NAN);
                let own = f.get(7).map(|v| v.trim() == "1").unwrap_or(false);
                t.record = Some((frame, a, b, ratio, own));
            }
            continue;
        }
        if line.starts_with('#') || line.starts_with("class\t") || line.trim().is_empty() { continue; }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 13 { continue; }
        let row = ClassRow {
            class: f[0].trim().to_string(),
            texels: f[2].trim().parse().unwrap_or(0),
            lit_texels: { let n: f64 = f[2].trim().parse().unwrap_or(0.0); let p: f64 = f[4].trim().parse().unwrap_or(100.0); (n * p / 100.0).round() as usize },
            ratio: parse_rgb(f[7]).unwrap_or([f64::NAN; 3]),
            identical: f[9].trim().parse().unwrap_or(f64::NAN),
            within1: f[10].trim().parse().unwrap_or(f64::NAN),
            within2: f[11].trim().parse().unwrap_or(f64::NAN),
            max_delta: f[12].trim().parse().unwrap_or(0),
        };
        if row.class == "TOTAL" { t.total = Some(row); } else { t.rows.push(row); }
    }
    if t.total.is_none() { return Err(format!("{}: no TOTAL row", p.display())); }
    Ok(t)
}

/// The worst class of a table: (class, ratio, deviation) over the classes with ≥ min_texels texels and a finite ratio.
pub fn worst_class(t: &Table, min_texels: usize) -> Option<(String, [f64; 3], f64)> {
    let mut best: Option<(String, [f64; 3], f64)> = None;
    for r in &t.rows {
        if r.lit_texels < min_texels || r.ratio.iter().any(|v| !v.is_finite()) { continue; }
        let dev = r.ratio.iter().map(|v| (v - 1.0).abs()).fold(0.0, f64::max);
        if best.as_ref().map(|b| dev > b.2).unwrap_or(true) { best = Some((r.class.clone(), r.ratio, dev)); }
    }
    best
}

#[derive(Clone, Debug, PartialEq)]
pub enum State { ClosedTexel, ClosedClass, Residue, Open, NoOracle }

impl State {
    pub fn label(&self) -> &'static str {
        match self { State::ClosedTexel => "CLOSED (texel)", State::ClosedClass => "CLOSED (class)", State::Residue => "RESIDUE", State::Open => "OPEN", State::NoOracle => "no oracle" }
    }
}

pub struct Verdict {
    pub state: State,
    pub identity: Option<(f64, f64, f64, u32)>,
    pub record_ratio: Option<f64>,
    pub worst: Option<(String, [f64; 3], f64)>,
    pub note: String,
}

pub fn judge(c: &Cell, t: Option<&Table>, tol: f64, min_texels: usize) -> Verdict {
    let mut v = Verdict { state: State::Open, identity: None, record_ratio: None, worst: None, note: String::new() };
    if let Some(t) = t {
        let own = t.record.map(|r| r.4).unwrap_or(false);
        if let Some(tot) = &t.total { if !own { v.identity = Some((tot.identical, tot.within1, tot.within2, tot.max_delta)); } }
        v.record_ratio = t.record.map(|r| r.3).filter(|r| r.is_finite());
        v.worst = worst_class(t, min_texels);
        let class_ok = v.worst.as_ref().map(|w| w.2 <= tol).unwrap_or(false);
        let texel_ok = v.identity.map(|(i, _, w2, _)| i >= 0.9 * c.ceiling && w2 >= 90.0).unwrap_or(false);
        v.state = if class_ok && texel_ok { State::ClosedTexel } else if class_ok { State::ClosedClass } else { State::Residue };
        if own { v.note.push_str("own rects (no byte identity); "); }
    }
    match c.state.as_str() {
        "no-oracle" | "no oracle" | "nooracle" => v.state = State::NoOracle,
        "open" => v.state = State::Open,
        "closed" => v.state = if v.identity.is_some() { State::ClosedTexel } else { State::ClosedClass },
        "closed-class" => v.state = State::ClosedClass,
        "residue" => v.state = State::Residue,
        _ => {}
    }
    v
}

fn short_class(s: &str) -> String {
    let s = s.split(':').last().unwrap_or(s);
    s.trim_end_matches(".Item.Gbx").trim_end_matches(".Prefab.Gbx#0").to_string()
}

/// Build the Markdown table; returns (markdown, counts per state label).
pub fn render(cells: &[Cell], base_dir: &std::path::Path, tol: f64, min_texels: usize, base_label: &str, notes: &[String]) -> (String, Vec<(String, usize)>) {
    let mut md = String::new();
    md.push_str(&format!("# Trust matrix — {base_label}\n\n"));
    if !notes.is_empty() {
        md.push_str("What changed in the COMPARISON, version to version (the code improves while \"closed\" counts can fall — the yardstick moved):\n");
        for n in notes { md.push_str(&format!("- {n}\n")); }
        md.push('\n');
    }
    md.push_str(&format!("Rule: worst class over the classes with ≥ {min_texels} texels; CLOSED (texel) = worst within {:.0} % and identity ≥ 90 % of the cell's CEILING (the editor's own re-bake identity on that map class: 95–99 % lamp-less (pwc-day ×4 saves), ~60 % with lamps (stpad Night + nocache) — VALIDATION.md V2-5) and within ±2 ≥ 90 %; CLOSED (class) = worst within {:.0} % only; RESIDUE = a class beyond it (named); OPEN = no table yet; no oracle = the game cannot produce one.\n\n", 100.0 * tol, 100.0 * tol));
    md.push_str("| cell | collection | mood (word) | q | map | features | identity % (±1 / ±2) vs the editor's own | max \\|Δ\\| | record ours/editor | worst class (r/g/b) | state | probes (frame P) | cause / note |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    let mut pcounts: std::collections::BTreeMap<String, usize> = Default::default();
    for c in cells {
        let table = if c.class_tsv.is_empty() || c.class_tsv == "-" { None } else { match read_table(&c.class_tsv, base_dir) { Ok(t) => Some(t), Err(e) => { eprintln!("trustmatrix: {}: {e}", c.cell); None } } };
        let v = judge(c, table.as_ref(), tol, min_texels);
        *counts.entry(v.state.label().to_string()).or_default() += 1;
        let ident = match v.identity { Some((i, w1, w2, _)) => format!("{i:.1} ({w1:.1} / {w2:.1}) vs {:.0}", c.ceiling), None => "—".to_string() };
        let maxd = match v.identity { Some((_, _, _, m)) => m.to_string(), None => "—".to_string() };
        let rec = match v.record_ratio { Some(r) => format!("{r:.4}"), None => "—".to_string() };
        let worst = match &v.worst { Some((k, r, _)) => format!("{} {:.3}/{:.3}/{:.3}", short_class(k), r[0], r[1], r[2]), None => "—".to_string() };
        let mood = if c.word.is_empty() { c.mood.clone() } else { format!("{} ({})", c.mood, c.word) };
        let mut note = v.note.clone();
        if !c.cause.is_empty() { note.push_str(&c.cause); }
        let probes = if c.probes.trim().is_empty() { "—".to_string() } else { c.probes.replace('|', "/") };
        if let Some(p) = c.probes.split("**").nth(1) { *pcounts.entry(p.to_string()).or_default() += 1; }
        md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | **{}** | {} | {} |\n", c.cell, c.collection, mood, c.quality, c.map, c.features, ident, maxd, rec, worst, v.state.label(), probes, note.replace('|', "/")));
    }
    md.push_str("\nStates: ");
    md.push_str(&counts.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(" · "));
    if !pcounts.is_empty() {
        md.push_str(&format!("\n\nProbe frame P (the probe volume vs the editor's; the probe ceiling = the editor against itself: 62.5 % colour-identical lamp-less (pwc-day ×2 saves), 72.2 % with lamps (stpad Night + nocache); P-CLOSED (texel) = layout identical, scales within 3 %, colour value within 3 %, lamp images present, identity ≥ 90 % of the ceiling; P-CLOSED (class) = the same without the identity; P-RESIDUE = the worst term named): {}\n", pcounts.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(" · ")));
    }
    md.push('\n');
    (md, counts.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: &[(&str, usize, [f64; 3])], total_ident: (f64, f64, f64, u32), own: bool) -> Table {
        Table {
            rows: rows.iter().map(|(k, n, r)| ClassRow { class: k.to_string(), texels: *n, lit_texels: *n, ratio: *r, identical: 0.0, within1: 0.0, within2: 0.0, max_delta: 0 }).collect(),
            total: Some(ClassRow { class: "TOTAL".into(), texels: 1, lit_texels: 1, ratio: [1.0; 3], identical: total_ident.0, within1: total_ident.1, within2: total_ident.2, max_delta: total_ident.3 }),
            record: Some((0, 1.0, 1.0, 1.0, own)),
        }
    }

    #[test]
    fn closed_at_the_ceiling_is_texel_closed() {
        let t = table(&[("tile", 100000, [1.001, 1.000, 0.999]), ("item:a", 5000, [1.02, 1.01, 1.0])], (61.3, 86.5, 95.6, 9), false);
        let v = judge(&Cell { ceiling: 60.0, ..Default::default() }, Some(&t), 0.03, 500);
        assert_eq!(v.state, State::ClosedTexel);
        assert_eq!(v.worst.unwrap().0, "item:a");
    }

    #[test]
    fn a_class_beyond_tol_is_residue_and_named() {
        let t = table(&[("tile", 100000, [1.0; 3]), ("item:pillar", 10000, [0.744, 0.764, 0.774]), ("item:tiny", 20, [3.0; 3])], (33.0, 69.6, 88.0, 14), false);
        let v = judge(&Cell::default(), Some(&t), 0.03, 500);
        assert_eq!(v.state, State::Residue);
        assert_eq!(v.worst.unwrap().0, "item:pillar"); // the 20-texel class is ignored
    }

    #[test]
    fn own_rects_has_no_identity_and_stops_at_class() {
        let t = table(&[("tile", 100000, [1.01; 3])], (99.0, 99.0, 99.0, 0), true);
        let v = judge(&Cell::default(), Some(&t), 0.03, 500);
        assert_eq!(v.state, State::ClosedClass);
        assert!(v.identity.is_none());
    }

    #[test]
    fn below_a_lampless_ceiling_stays_at_class() {
        // np-tk3 Day: every class 1.000, identity 61 % — the editor re-bakes a lamp-less map to 99 %
        let t = table(&[("tile", 100000, [1.000, 1.000, 1.000])], (61.3, 86.5, 95.6, 9), false);
        assert_eq!(judge(&Cell { ceiling: 99.0, ..Default::default() }, Some(&t), 0.03, 500).state, State::ClosedClass);
        assert_eq!(judge(&Cell { ceiling: 60.0, ..Default::default() }, Some(&t), 0.03, 500).state, State::ClosedTexel);
    }

    #[test]
    fn overrides_win() {
        let c = Cell { state: "no-oracle".into(), ..Default::default() };
        assert_eq!(judge(&c, None, 0.03, 500).state, State::NoOracle);
        assert_eq!(judge(&Cell::default(), None, 0.03, 500).state, State::Open);
    }

    #[test]
    fn manifest_and_table_round_trip() {
        let dir = std::env::temp_dir().join(format!("trustmatrix-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("c.tsv"), "class\tcharts\ttexels\tlit_ours_pct\tlit_editor_pct\tmean_ours_rgb\tmean_editor_rgb\tratio_rgb\trmse_rel_rgb\tidentical_pct\twithin1_pct\twithin2_pct\tmax_delta\tchart_ratio_n_median_sigma\ntile:tile\t4096\t164581\t100.0\t100.0\t0.6804 / 1.0303 / 1.7304\t0.6803 / 1.0301 / 1.7303\t1.000 / 1.000 / 1.000\t0.011 / 0.008 / 0.010\t63.38\t83.33\t96.22\t9\t4096, 1.001, 0.005\nTOTAL\t4126\t963184\t100.0\t100.0\t0.6546 / 0.9814 / 1.6449\t0.6544 / 0.9826 / 1.6458\t1.000 / 0.999 / 0.999\t0.010 / 0.009 / 0.010\t61.32\t86.54\t95.59\t9\t4126, 1.001, 0.005\n#record\t0\t1.968219\t1.9651023\t1.001586\t204/2/2\t204/2/2\t0\n").unwrap();
        std::fs::write(dir.join("m.tsv"), "# comment\ncell\tcollection\tmood\tword\tquality\tmap\tfeatures\tclass_tsv\tstate\tcause\tceiling\nBB-Day\tBlueBay\tDay\t0x9b59\t3\tnp-tk3\tF2 F7\tc.tsv\t\t\t99\nRI\tRedIsland\tDay\t\t\t\tF1\t-\tno-oracle\tTitlePack\t\n").unwrap();
        let cells = read_manifest(dir.join("m.tsv").to_str().unwrap()).unwrap();
        assert_eq!(cells.len(), 2);
        let t = read_table("c.tsv", &dir).unwrap();
        assert_eq!(t.rows.len(), 1);
        let rec = t.record.unwrap();
        assert!((rec.3 - 1.001586).abs() < 1e-6 && !rec.4);
        let (md, counts) = render(&cells, &dir, 0.03, 500, "test", &[]);
        assert!(md.contains("CLOSED (class)") && md.contains("vs 99") && md.contains("no oracle"));
        assert!((cells[0].ceiling - 99.0).abs() < 1e-9);
        assert_eq!(counts.iter().map(|(_, n)| n).sum::<usize>(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
