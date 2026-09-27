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
        out.push(Cell { cell: get("cell"), collection: get("collection"), mood: get("mood"), word: get("word"), quality: get("quality"), map: get("map"), features: get("features"), class_tsv: get("class_tsv"), state: get("state").to_ascii_lowercase(), cause: get("cause"), ceiling: get("ceiling").parse().unwrap_or(99.0) });
    }
    if header.is_none() { return Err(format!("{path}: no header line")); }
    Ok(out)
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
    md.push_str("| cell | collection | mood (word) | q | map | features | identity % (±1 / ±2) vs the editor's own | max \\|Δ\\| | record ours/editor | worst class (r/g/b) | state | cause / note |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
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
        md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | **{}** | {} |\n", c.cell, c.collection, mood, c.quality, c.map, c.features, ident, maxd, rec, worst, v.state.label(), note.replace('|', "/")));
    }
    md.push_str("\nStates: ");
    md.push_str(&counts.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(" · "));
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
