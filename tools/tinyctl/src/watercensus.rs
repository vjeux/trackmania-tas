//! `tinyctl water-census` — the water census of a whole giant set: every source
//! map against its shipped giant (`mapgeom watercells`), the anchors read from
//! the pipeline logs, one TSV per map and a WATER-CENSUS.md table (2026-10-01,
//! vjeux: "The water on giant 15 is not working correctly … for giant we have
//! no excuse, we need to get water working perfectly").
//!
//! ```text
//! tinyctl water-census --src-dir DIR --giant-dir DIR --log pipeline-x2.log[,more.log]
//!                      [--maps 01,15,20] [--scale 2] [--out DIR] [--bin-dir DIR]
//!                      [--anchor NN=sx,sy,sz:tx,ty,tz]…
//! ```
//!
//! The anchor of map NN is the LAST `anchor: source […] -> target […]` line
//! under the `===== NN:` header across the logs in order (a redo log after the
//! first run wins); `--anchor NN=…` overrides, and a map with no anchor at all
//! falls back to `from-tiles` (the pool tiles' min corner).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::build::paks_for;
use crate::views::{collection_name, collection_of};
use tmmaps::map::MapFile;

fn flag(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}

/// Per-map anchors from pipeline logs: `===== NN: …` headers, the last anchor line under each.
pub fn anchors_from_logs(logs: &[PathBuf]) -> Result<BTreeMap<String, String>, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for log in logs {
        let text = std::fs::read_to_string(log).map_err(|e| format!("{}: {e}", log.display()))?;
        let mut current: Option<String> = None;
        for line in text.lines() {
            let l = line.trim();
            if let Some(rest) = l.strip_prefix("===== ") {
                current = rest.split(':').next().map(|s| s.trim().to_string());
                continue;
            }
            if let Some(rest) = l.strip_prefix("anchor: source [") {
                if let (Some(nn), Some((sv, rest))) = (&current, rest.split_once("] -> target [")) {
                    if let Some((tv, _)) = rest.split_once(']') {
                        let clean = |s: &str| s.split(',').map(|x| x.trim().to_string()).collect::<Vec<_>>().join(",");
                        out.insert(nn.clone(), format!("{}:{}", clean(sv), clean(tv)));
                    }
                }
            }
        }
    }
    Ok(out)
}

fn find_map(dir: &Path, pred: impl Fn(&str) -> bool) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.file_name().map(|n| pred(&n.to_string_lossy())).unwrap_or(false))
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let src_dir = PathBuf::from(flag(args, "--src-dir").ok_or("--src-dir DIR (the source maps NN-*.Map.Gbx)")?);
    let giant_dir = PathBuf::from(flag(args, "--giant-dir").ok_or("--giant-dir DIR (the giant maps *-NN-Giant.Map.Gbx)")?);
    let logs: Vec<PathBuf> = flag(args, "--log").map(|l| l.split(',').map(PathBuf::from).collect()).unwrap_or_default();
    let scale = flag(args, "--scale").unwrap_or_else(|| "2".into());
    let out_dir = PathBuf::from(flag(args, "--out").unwrap_or_else(|| "water-census".into()));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let bin_dir = flag(args, "--bin-dir").map(PathBuf::from).or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))).ok_or("--bin-dir DIR")?;
    let mapgeom = bin_dir.join("mapgeom");
    if !mapgeom.exists() {
        return Err(format!("{}: no such binary", mapgeom.display()));
    }
    let mut anchors = anchors_from_logs(&logs)?;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--anchor" {
            if let Some((nn, a)) = args.get(i + 1).and_then(|v| v.split_once('=')) {
                anchors.insert(nn.to_string(), a.to_string());
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    let maps: Vec<String> = match flag(args, "--maps") {
        Some(m) => m.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        None => {
            let mut v: Vec<String> = std::fs::read_dir(&giant_dir)
                .map_err(|e| format!("{}: {e}", giant_dir.display()))?
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    let stem = n.strip_suffix("-Giant.Map.Gbx")?;
                    stem.rsplit('-').next().map(|s| s.to_string())
                })
                .filter(|s| s.len() == 2 && s.chars().all(|c| c.is_ascii_digit()))
                .collect();
            v.sort();
            v.dedup();
            v
        }
    };
    if maps.is_empty() {
        return Err("no maps (no *-NN-Giant.Map.Gbx in --giant-dir, no --maps)".into());
    }
    let mut md = String::from("# Water census — source water blocks vs the giant's engine water\n\n");
    md.push_str("| map | coll | source water blocks | native | partial | missing | missing m³ | inner rims | boundary rims (source has / source lacks) | missing kinds |\n|---|---|---|---|---|---|---|---|---|---|\n");
    let mut totals = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut details = String::new();
    for nn in &maps {
        let src = find_map(&src_dir, |n| n.starts_with(&format!("{nn}-")) && n.ends_with(".Map.Gbx")).ok_or(format!("{nn}: no {nn}-*.Map.Gbx in {}", src_dir.display()))?;
        let giant = find_map(&giant_dir, |n| n.ends_with(&format!("-{nn}-Giant.Map.Gbx"))).ok_or(format!("{nn}: no *-{nn}-Giant.Map.Gbx in {}", giant_dir.display()))?;
        let m = MapFile::load(&src);
        let coll = collection_of(&m);
        let paks = paks_for(coll)?;
        let anchor = anchors.get(nn).cloned().unwrap_or_else(|| "from-tiles".to_string());
        let census = out_dir.join(format!("census-{nn}.tsv"));
        let rims = out_dir.join(format!("rims-{nn}.tsv"));
        let mut c = Command::new(&mapgeom);
        c.args(&paks).arg("watercells").arg(&src).arg(&giant).arg("--anchor").arg(&anchor).arg("--scale").arg(&scale).arg("--collection").arg(collection_name(coll)).arg("--out").arg(&census).arg("--rims").arg(&rims);
        let o = c.output().map_err(|e| format!("{nn}: mapgeom watercells: {e}"))?;
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        if !o.status.success() {
            let err = String::from_utf8_lossy(&o.stderr);
            println!("{nn}: watercells FAILED: {}", err.lines().last().unwrap_or(""));
            md.push_str(&format!("| {nn} | {} | — | — | — | — | — | — | — | FAILED: {} |\n", collection_name(coll), err.lines().last().unwrap_or("").replace('|', "/")));
            continue;
        }
        // the summary lines
        let mut native = 0;
        let mut partial = 0;
        let mut missing = 0;
        let mut missing_m3 = 0usize;
        let mut src_blocks = 0usize;
        let mut inner = 0usize;
        let mut b_ok = 0usize;
        let mut b_extra = 0usize;
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for l in text.lines() {
            if let Some(rest) = l.strip_prefix("# TOTAL ") {
                let (v, rest) = rest.split_once(':').unwrap_or((rest, ""));
                let n: usize = rest.trim().split(' ').next().and_then(|x| x.parse().ok()).unwrap_or(0);
                let m3: usize = rest.split(',').nth(1).and_then(|x| x.trim().split(' ').next()).and_then(|x| x.parse().ok()).unwrap_or(0);
                match v {
                    "native" => native = n,
                    "partial" => partial = n,
                    "missing" => {
                        missing = n;
                        missing_m3 = m3;
                    }
                    _ => {}
                }
            } else if let Some(rest) = l.strip_prefix("# rims: ") {
                let nums: Vec<usize> = rest.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect();
                if nums.len() >= 3 {
                    inner = nums[0];
                    b_ok = nums[1];
                    b_extra = nums[2];
                }
            } else if l.starts_with("#   ") {
                let cols: Vec<&str> = l.trim_start_matches("#   ").split('\t').collect();
                if cols.len() >= 4 && cols[2] != "native" {
                    let n: usize = cols[3].split(' ').next().and_then(|x| x.parse().ok()).unwrap_or(0);
                    *kinds.entry(format!("{} [{}]", cols[0], cols[1])).or_insert(0) += n;
                }
            } else if l.starts_with("# watercells:") {
                if let Some(p) = l.find(" source water blocks") {
                    let head = &l[..p];
                    src_blocks = head.rsplit(' ').next().and_then(|x| x.parse().ok()).unwrap_or(0);
                }
            }
        }
        totals.0 += src_blocks;
        totals.1 += native;
        totals.2 += partial;
        totals.3 += missing;
        totals.4 += missing_m3;
        totals.5 += inner;
        let kinds_s = kinds.iter().map(|(k, n)| format!("{k} ×{n}")).collect::<Vec<_>>().join(", ");
        md.push_str(&format!("| {nn} | {} | {src_blocks} | {native} | {partial} | {missing} | {missing_m3} | {inner} | {b_ok} / {b_extra} | {} |\n", collection_name(coll), if kinds_s.is_empty() { "—".to_string() } else { kinds_s }));
        println!("{nn}: {} source water blocks: {native} native, {partial} partial, {missing} missing ({missing_m3} m³); rims inside the water {inner}, boundary {b_ok} ok / {b_extra} extra (anchor {anchor})", src_blocks);
        details.push_str(&format!("\n## {nn} ({}) — anchor {anchor}\n\n```\n{}```\n", collection_name(coll), text.lines().filter(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect::<String>()));
    }
    md.push_str(&format!("| **all** | | {} | {} | {} | {} | {} | {} | | |\n", totals.0, totals.1, totals.2, totals.3, totals.4, totals.5));
    md.push_str("\nPer-cell tables: census-NN.tsv (one row per source water block: kind, cell, scaled m³, covered m³, verdict, the giant blocks found, inner rims, the converter reason); rims-NN.tsv (every engine clip drawn inside the source's water, and boundary rims the source does not draw).\n");
    md.push_str(&details);
    let md_path = out_dir.join("WATER-CENSUS.md");
    std::fs::write(&md_path, &md).map_err(|e| format!("{}: {e}", md_path.display()))?;
    println!("wrote {}", md_path.display());
    Ok(())
}
