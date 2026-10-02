//! `tinyctl genealogy-census --sources DIR [--tiny DIR] [--x2 DIR] [--out census.tsv]`
//! — the zone-table verdict of every built map of a campaign against its source:
//! one TSV row per source `NN-*.Map.Gbx` and per built dir (`<Prefix>-NN-<Label>.Map.Gbx`,
//! matched by NN): the source's record count, first zone, most common zone and water
//! zone (with its cell count); the built file's record count and zone set; the
//! `tmmaps ponds` uncovered count (kept Sea cells of the island whose full-size cell
//! beneath has neither a Sea record nor a water-zone genealogy — the bottomless
//! water of Summer 01's lagoon and Fall 09's); and the verdict:
//!   `ok`        — a water zone fills the whole table (or Stadium's Grass is kept), 0 uncovered;
//!   `CLEARED`   — 0 records: nothing regenerated under the island (the Fall 2026
//!                 BlueBay tinies 04 09 14 19 24 as shipped 2026-10-01);
//!   `UNCOVERED` — a table but still bottomless cells;
//!   `MIXED`     — more than one zone in the table (a kept source table on a
//!                 terrain collection: the full-size island regenerates under the tiny one).
//! Non-zero exit when any row is not `ok` (a gate for the publish batch).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const WATER: [&str; 3] = ["Sea", "Water", "Lake"];

fn nn_of_source(name: &str) -> Option<String> {
    // 09-Fall-2026---09.Map.Gbx → 09
    let nn = name.split('-').next()?;
    (nn.len() == 2 && nn.chars().all(|c| c.is_ascii_digit())).then(|| nn.to_string())
}

fn nn_of_built(name: &str) -> Option<String> {
    // Fall-09-Tiny.Map.Gbx → 09
    let mut it = name.split('-');
    it.next()?;
    let nn = it.next()?;
    (nn.len() == 2 && nn.chars().all(|c| c.is_ascii_digit())).then(|| nn.to_string())
}

fn maps_by_nn(dir: &Path, nn_of: fn(&str) -> Option<String>) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let p = e.map_err(|e| e.to_string())?.path();
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else { continue };
        if !name.ends_with(".Map.Gbx") {
            continue;
        }
        if let Some(nn) = nn_of(name) {
            out.insert(nn, p);
        }
    }
    Ok(out)
}

struct Zones {
    count: usize,
    first: String,
    top: String,
    water: Option<(String, usize)>,
    distinct: usize,
    set: String,
}

fn zones_of(m: &tmmaps::map::MapFile) -> Zones {
    let zones = m.genealogy_zones();
    let mut hist: BTreeMap<&str, usize> = BTreeMap::new();
    for z in &zones {
        *hist.entry(z.as_str()).or_default() += 1;
    }
    let top = hist.iter().max_by_key(|(_, c)| **c).map(|(z, _)| z.to_string()).unwrap_or_else(|| "-".into());
    let water = hist.iter().filter(|(z, _)| WATER.contains(z)).max_by_key(|(_, c)| **c).map(|(z, c)| (z.to_string(), *c));
    let set = if hist.len() <= 3 { hist.iter().map(|(z, c)| format!("{z}:{c}")).collect::<Vec<_>>().join(",") } else { format!("{} zones", hist.len()) };
    Zones { count: zones.len(), first: zones.first().cloned().unwrap_or_else(|| "-".into()), top, water, distinct: hist.len(), set }
}

/// The verdict of one built file: its zone table and the uncovered count.
fn verdict(m: &tmmaps::map::MapFile, z: &Zones, uncovered: usize, envir: &str) -> &'static str {
    if z.count == 0 {
        return "CLEARED";
    }
    if uncovered > 0 {
        return "UNCOVERED";
    }
    if z.distinct > 1 {
        return "MIXED";
    }
    let _ = m;
    // one zone everywhere: a water zone, or Stadium's grass floor
    if WATER.contains(&z.top.as_str()) || (envir == "Stadium" && z.top == "Grass") {
        "ok"
    } else {
        "LAND-FILL"
    }
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let src_dir = PathBuf::from(f("--sources").ok_or("genealogy-census needs --sources DIR")?);
    let built: Vec<(String, PathBuf)> = [("tiny", f("--tiny")), ("x2", f("--x2"))].into_iter().filter_map(|(l, d)| d.map(|d| (l.to_string(), PathBuf::from(d)))).collect();
    if built.is_empty() {
        return Err("genealogy-census wants --tiny DIR and/or --x2 DIR".into());
    }
    let scale_of = |label: &str| -> f32 { if label == "x2" { 2.0 } else { 0.5 } };
    let sources = maps_by_nn(&src_dir, nn_of_source)?;
    let mut rows: Vec<String> = Vec::new();
    rows.push("nn\tscale\tenvir\tsrc_records\tsrc_first\tsrc_top\tsrc_water\tbuilt_records\tbuilt_zones\tuncovered\tsea_cells\tbytes\tverdict\tfile".to_string());
    let mut bad = 0usize;
    for (label, dir) in &built {
        let files = maps_by_nn(dir, nn_of_built)?;
        for (nn, p) in &files {
            let m = tmmaps::map::MapFile::load(p);
            let h = tmmaps::header::read(p.to_str().unwrap_or_default()).map_err(|e| format!("{}: {e}", p.display()))?;
            let bz = zones_of(&m);
            let src = sources.get(nn);
            let sz = src.map(|s| zones_of(&tmmaps::map::MapFile::load(s)));
            let sea_cells = m.blocks.iter().chain(m.baked.iter()).filter(|b| b.name == "Sea").count();
            let uncovered = tmmaps::tiny::uncovered_sea_cells(&m, scale_of(label)).len();
            let v = verdict(&m, &bz, uncovered, &h.envir);
            if v != "ok" {
                bad += 1;
            }
            let bytes = std::fs::metadata(p).map(|md| md.len()).unwrap_or(0);
            rows.push(format!(
                "{nn}\t{label}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{uncovered}\t{sea_cells}\t{bytes}\t{v}\t{}",
                h.envir,
                sz.as_ref().map(|z| z.count.to_string()).unwrap_or_else(|| "-".into()),
                sz.as_ref().map(|z| z.first.clone()).unwrap_or_else(|| "-".into()),
                sz.as_ref().map(|z| z.top.clone()).unwrap_or_else(|| "-".into()),
                sz.as_ref().and_then(|z| z.water.as_ref().map(|(w, c)| format!("{w}:{c}"))).unwrap_or_else(|| "-".into()),
                bz.count,
                bz.set,
                p.display()
            ));
        }
    }
    let text = rows.join("\n") + "\n";
    print!("{text}");
    if let Some(out) = f("--out") {
        std::fs::write(&out, &text).map_err(|e| format!("{out}: {e}"))?;
        eprintln!("{} rows -> {out}", rows.len() - 1);
    }
    eprintln!("{} built files, {bad} not ok", rows.len() - 1);
    if bad > 0 {
        return Err(format!("{bad} built file(s) with a cleared, mixed or bottomless zone table"));
    }
    Ok(())
}
