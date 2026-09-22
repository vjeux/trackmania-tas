//! `tinyctl dist` — the downloadable club set: one zip per part with the maps
//! under the club's file-name convention and a `MAPS.tsv`, the item-check gate
//! on every map first (2026-09-13, the giant U10S release; the tiny set was
//! made the same way by hand).
//!
//! ```text
//! tinyctl dist --out-root O --parts 01-39 --dist DIR --variant Giant
//!              [--tag giant] [--out-prefix U10S] [--paks "--pak F:KEY …"] [--jobs 8] [--no-gate]
//! ```
//!
//! For part `PP` the built maps are `O/pPP/tinyNN/<tag>/<prefix>-NN-<Variant>.Map.Gbx`
//! with their trackers `O/pPP/tracker.tsv` (the pipeline's rows: source name,
//! uid, times, size, fit note). Each map goes into
//! `DIR/<Variant>U10S-partPP.zip` as `<source name> By Everios96 [<Variant>].Map.Gbx`
//! (the in-file name the build already carries; `--name-format` on the build)
//! in campaign order, beside `MAPS.tsv`:
//! `nn file source_name source_uid authortime_ms gold silver bronze uid bytes fit_note`.
//! A map that fails the gate, or a part with a missing map, is listed in
//! `DIR/DIST.tsv` and left out of its zip (the zip still ships the rest).
//! The zips are STORED (the map files are already deflated archives).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let out_root = PathBuf::from(f("--out-root").ok_or("dist needs --out-root O (O/pNN/tinyNN/<tag>/…)")?);
    let dist = PathBuf::from(f("--dist").ok_or("dist needs --dist DIR")?);
    let variant = f("--variant").unwrap_or_else(|| "Giant".into());
    let tag = f("--tag").unwrap_or_else(|| variant.to_ascii_lowercase());
    let prefix = f("--out-prefix").unwrap_or_else(|| "U10S".into());
    let paks = f("--paks");
    let gate_on = !tmmaps::cli::has(args, "--no-gate");
    let jobs: usize = f("--jobs").and_then(|j| j.parse().ok()).unwrap_or(8).max(1);
    let parts_arg = f("--parts").ok_or("dist needs --parts 01-39 or 01,02,…")?;
    let parts: Vec<String> = if let Some((a, b)) = parts_arg.split_once('-') {
        let (a, b): (usize, usize) = (a.trim().parse().map_err(|_| "--parts A-B")?, b.trim().parse().map_err(|_| "--parts A-B")?);
        (a..=b).map(|n| format!("{n:02}")).collect()
    } else {
        parts_arg.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    };
    if gate_on && paks.is_none() {
        return Err("dist needs --paks \"--pak FILE:KEY …\" for the item-check gate (or --no-gate)".into());
    }
    std::fs::create_dir_all(&dist).map_err(|e| format!("{}: {e}", dist.display()))?;
    let queue = std::sync::Mutex::new(std::collections::VecDeque::from(parts.clone()));
    let results = std::sync::Mutex::new(Vec::<String>::new());
    let t0 = std::time::Instant::now();
    std::thread::scope(|s| {
        for _ in 0..jobs.min(parts.len()) {
            s.spawn(|| loop {
                let part = match queue.lock().unwrap().pop_front() {
                    Some(p) => p,
                    None => break,
                };
                let line = match one_part(&out_root, &dist, &variant, &tag, &prefix, paks.as_deref(), gate_on, &part) {
                    Ok(l) => l,
                    Err(e) => format!("{part}\tFAILED\t-\t-\t-\t{e}"),
                };
                eprintln!("[{:>5.0}s] {line}", t0.elapsed().as_secs_f64());
                results.lock().unwrap().push(line);
            });
        }
    });
    let mut lines = results.into_inner().unwrap();
    lines.sort();
    let summary = dist.join("DIST.tsv");
    std::fs::write(&summary, format!("part\tstatus\tmaps_in\tzip_bytes\tover_7mb\tnote\n{}\n", lines.join("\n"))).map_err(|e| format!("{}: {e}", summary.display()))?;
    println!("{}", lines.join("\n"));
    let bad = lines.iter().filter(|l| !l.contains("\tOK\t")).count();
    if bad > 0 {
        return Err(format!("{bad} of {} parts not complete — see {}", parts.len(), summary.display()));
    }
    Ok(())
}

/// The tracker's last row per map number: (nn -> row).
fn tracker_rows(path: &Path) -> BTreeMap<String, Vec<String>> {
    let mut by_nn: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string(path) {
        for l in text.lines().skip(1).filter(|l| !l.trim().is_empty()) {
            let r: Vec<String> = l.split('\t').map(|c| c.to_string()).collect();
            if let Some(nn) = r.first() {
                by_nn.insert(nn.clone(), r);
            }
        }
    }
    by_nn
}

/// The item-check gate of `publish_one`, for one built map's library.
pub fn gate(items_dir: &Path, paks: &str) -> Result<usize, String> {
    let mut items: Vec<String> = std::fs::read_dir(items_dir).map_err(|e| format!("{}: {e}", items_dir.display()))?.filter_map(|e| e.ok()).map(|e| e.path().to_string_lossy().into_owned()).filter(|p| p.ends_with(".Item.Gbx")).collect();
    items.sort();
    if items.is_empty() {
        return Err(format!("{}: no .Item.Gbx files", items_dir.display()));
    }
    let n = items.len();
    let mut rest: Vec<String> = vec!["item-check".into(), "--quiet".into()];
    rest.extend(items);
    let mut open = || {
        let mut store = mapgeom::store::DataStore::empty();
        let toks: Vec<&str> = paks.split_whitespace().collect();
        let mut i = 0;
        while i < toks.len() {
            if toks[i] == "--pak" {
                if let Some((p, k)) = toks.get(i + 1).and_then(|s| s.rsplit_once(':')) {
                    if let Err(e) = store.add_pak(p, k) {
                        eprintln!("--paks: {p}: {e}");
                    }
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        store
    };
    mapgeom::static_item::check::run(&rest, &mut open).map_err(|e| format!("item-check refused: {e}"))?;
    Ok(n)
}

fn one_part(out_root: &Path, dist: &Path, variant: &str, tag: &str, prefix: &str, paks: Option<&str>, gate_on: bool, part: &str) -> Result<String, String> {
    let part_dir = out_root.join(format!("p{part}"));
    let rows = tracker_rows(&part_dir.join("tracker.tsv"));
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut tsv = String::from("nn\tfile\tsource_name\tsource_uid\tauthortime_ms\tgold\tsilver\tbronze\tuid\tbytes\tfit_note\n");
    let mut notes: Vec<String> = Vec::new();
    let mut n_in = 0usize;
    let mut over = 0usize;
    for n in 1..=25usize {
        let nn = format!("{n:02}");
        let build_dir = part_dir.join(format!("tiny{nn}")).join(tag);
        let map = build_dir.join(format!("{prefix}-{nn}-{variant}.Map.Gbx"));
        if !map.exists() {
            notes.push(format!("{nn}: not built"));
            continue;
        }
        let hdr = tmmaps::header::read(map.to_str().ok_or("map path")?)?;
        if gate_on {
            let items = build_dir.join("libx").join("Items");
            // a build whose libx was cleaned up: re-extract it from lib.zip
            if std::fs::read_dir(&items).map(|rd| rd.count()).unwrap_or(0) == 0 {
                let zip = build_dir.join("lib.zip");
                if zip.exists() {
                    let _ = std::fs::create_dir_all(&items);
                    let _ = std::process::Command::new("unzip").arg("-q").arg("-o").arg(&zip).arg("-d").arg(build_dir.join("libx")).output();
                }
            }
            if let Err(e) = gate(&items, paks.unwrap_or("")) {
                notes.push(format!("{nn}: {e}"));
                continue;
            }
        }
        let bytes = std::fs::read(&map).map_err(|e| format!("{}: {e}", map.display()))?;
        if bytes.len() >= 7_000_000 {
            over += 1;
        }
        // the file name is the in-file name (the club convention), sanitised for a zip entry
        let file = format!("{}.Map.Gbx", hdr.name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_"));
        let row = rows.get(&nn);
        let col = |i: usize| row.and_then(|r| r.get(i)).cloned().unwrap_or_else(|| "-".into());
        let fit = row.map(|r| r.get(16).cloned().unwrap_or_default()).unwrap_or_default();
        let fit = fit.split(';').find(|s| s.contains("fit under") || s.contains("DOES NOT FIT")).map(|s| s.trim().to_string()).unwrap_or_else(|| if bytes.len() >= 7_000_000 { "over 7 MB".into() } else { "fits as built".into() });
        tsv.push_str(&format!("{nn}\t{file}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{fit}\n", col(2), col(3), col(4), col(5), col(6), col(7), hdr.uid, bytes.len()));
        files.insert(file, bytes);
        n_in += 1;
    }
    files.insert("MAPS.tsv".to_string(), tsv.into_bytes());
    let zip = tmmaps::header::stored_zip(&files);
    let zip_path = dist.join(format!("{variant}U10S-part{part}.zip"));
    std::fs::write(&zip_path, &zip).map_err(|e| format!("{}: {e}", zip_path.display()))?;
    let status = if n_in == 25 { "OK" } else { "PARTIAL" };
    Ok(format!("{part}\t{status}\t{n_in}/25\t{}\t{over}\t{}", zip.len(), if notes.is_empty() { "-".to_string() } else { notes.join("; ") }))
}
