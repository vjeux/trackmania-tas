//! `tinyctl tiny-regate` — the car-change gates of the LIVE Tiny U10S maps (club 145858)
//! re-dressed in place.
//!
//! The Tiny U10S set was built before 2026-09-21, when a stock `GateGameplaySnow16m` /
//! `GateGameplayRally…` / `GateGameplayDesert…` item or block baked in the STADIUM gate's
//! dress (the kind folder's `Screen`/`Decal`/`Collision` names did not match the prefab's
//! `GateGameplayScreen`/`DecalGateGameplay`/`CollisionGateGameplay` stems). The fix is in
//! the library since abb950e0; the live files never got it (shcr 2026-10-23: "it keeps
//! the texture of stadium gate", Tiny 206; Everios96: fix 201+).
//!
//! The geometry of such a gate copy does not change (same prefab, same scale, same LOD):
//! only its materials do, so the live map is patched IN PLACE — same uid, records kept:
//!
//! 1. the live tiny file (the club inventory's fileUrl) and the ORIGINAL map;
//! 2. the gates: an original stock ITEM `GateGameplay<Kind>…` is the k-th item of both maps
//!    (the tiny pass keeps the item order: U10S 182's snow gate is item 1 in both). The
//!    gate BLOCKS were baked right in the old build (their block info carries the kind
//!    modifier): nothing to do for them;
//! 3. every alias so found is rebuilt from the ORIGINAL with `mapgeom tiny-library --only
//!    <model> --scale 0.5` (every detail level, as the set was built; `--lod-pick N` otherwise), `TINY_ALIAS_BASE` set so the produced file carries
//!    the live alias (a second run re-bases when the kind has several variants);
//! 4. the new item must have the live item's visuals (vertex counts) and must carry a
//!    `Modifier\GateGameplay<Kind>\` material — else the map is left alone and reported;
//! 5. `tmmaps replace-embedded` writes the patched file; `--apply` uploads it
//!    (`publish-batch --files-only`, storage update: the campaign slot keeps the uid).
//!
//! ```text
//! tinyctl tiny-regate --inventory INV.tsv --originals ROOT --maps 206[,…] --out DIR
//!                     [--bin-dir DIR] [--token-dir DIR] [--apply]
//! ```
//! INV.tsv = `nadeo-here club-inventory --club 145858 --out INV.tsv` (name, uid, fileUrl).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

const PAK: &str = "/tmp/current-Stadium.pak:B773D73047A4104857722366D78D28A6";

/// `GateGameplay<Kind>…` / `GateExpandableGameplay<Kind>…` only: the environment-themed
/// gates (`RallyGateGameplay`, `SnowGateGameplay`: castle/temple dress of their own) never
/// had the stadium-skin defect
fn kind_of(name: &str) -> Option<&'static str> {
    let rest = name.strip_prefix("GateGameplay").or_else(|| name.strip_prefix("GateExpandableGameplay"))?;
    for k in ["Snow", "Rally", "Desert"] {
        if rest.starts_with(k) {
            return Some(match k {
                "Snow" => "Snow",
                "Rally" => "Rally",
                _ => "Desert",
            });
        }
    }
    None
}

fn item_materials(mapgeom: &Path, item: &Path) -> Vec<String> {
    let out = Command::new(mapgeom).args(["dump", &item.to_string_lossy(), "--no-pak"]).output().ok();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    text.lines().filter_map(|l| l.split("material ").nth(1).map(|m| m.split(' ').next().unwrap_or("").to_string())).collect()
}

fn item_visuals(mapgeom: &Path, item: &Path) -> Vec<String> {
    let out = Command::new(mapgeom).args(["dump", &item.to_string_lossy(), "--no-pak"]).output().ok();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    text.lines().filter(|l| l.contains("CPlugVisual,")).map(|l| l.split("CPlugVisual,").nth(1).unwrap_or("").trim().to_string()).collect()
}

/// `(a, s)` per axis: tiny = a + s * orig, fitted on the item pairs that exist in both maps
fn fit_transform(orig: &tmmaps::map::MapFile, tiny: &tmmaps::map::MapFile) -> Option<[(f32, f32); 3]> {
    let n = orig.items.len().min(tiny.items.len());
    if n < 2 {
        return None;
    }
    let mut out = [(0.0f32, 0.0f32); 3];
    for ax in 0..3 {
        let xs: Vec<f32> = orig.items[..n].iter().map(|i| i.pos[ax]).collect();
        let ys: Vec<f32> = tiny.items[..n].iter().map(|i| i.pos[ax]).collect();
        let mx = xs.iter().sum::<f32>() / n as f32;
        let my = ys.iter().sum::<f32>() / n as f32;
        let sxx: f32 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
        let sxy: f32 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum();
        let s = if sxx > 1e-3 { sxy / sxx } else { 0.5 };
        out[ax] = (my - s * mx, s);
    }
    Some(out)
}

pub fn cmd(args: &[String]) -> i32 {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let Some(inv) = f("--inventory") else {
        eprintln!("tiny-regate needs --inventory INV.tsv");
        return 2;
    };
    let Some(originals) = f("--originals") else {
        eprintln!("tiny-regate needs --originals ROOT");
        return 2;
    };
    let Some(maps) = f("--maps") else {
        eprintln!("tiny-regate needs --maps N[,N…]");
        return 2;
    };
    let out = PathBuf::from(f("--out").unwrap_or_else(|| "/tmp/ww/regate".into()));
    let bin_dir = f("--bin-dir").map(PathBuf::from).or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))).unwrap();
    let mapgeom = bin_dir.join("mapgeom");
    let tmmaps_bin = bin_dir.join("tmmaps");
    let me = std::env::current_exe().unwrap();
    let token_dir = f("--token-dir").unwrap_or_else(|| "/tmp/tinyctl-tokens".into());
    let apply = tmmaps::cli::has(args, "--apply");
    // `--lod-pick N`: the Tiny U10S set was built with EVERY detail level (16 visual parts
    // on a 16 m gate: two levels of 8), so the default is no pick; the Summer tinies used 0
    let lod_args: Vec<String> = f("--lod-pick").map(|v| vec!["--lod-pick".to_string(), v]).unwrap_or_default();
    std::fs::create_dir_all(out.join("live")).ok();
    std::fs::create_dir_all(out.join("fixed")).ok();
    // the inventory: name → (uid, fileUrl)
    let inv_text = std::fs::read_to_string(&inv).unwrap_or_default();
    let mut live: BTreeMap<u32, (String, String)> = BTreeMap::new();
    for l in inv_text.lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 13 {
            continue;
        }
        // `U10S_206 By Everios96 [Tiny]`
        if let Some(n) = c[4].strip_prefix("U10S_").and_then(|r| r.split(' ').next()).and_then(|x| x.parse::<u32>().ok()) {
            live.insert(n, (c[3].to_string(), c[12].to_string()));
        }
    }
    let core = std::fs::read_to_string(Path::new(&token_dir).join("token-NadeoServices.txt")).unwrap_or_default().lines().next().unwrap_or("").trim().to_string();
    if core.is_empty() {
        eprintln!("tiny-regate: no core token in {token_dir}");
        return 2;
    }
    let mut results = String::from("num\tuid\tgates\taliases\tverdict\n");
    let mut manifest = String::new();
    let mut failed = 0usize;
    for num in maps.split(',').filter_map(|x| x.trim().parse::<u32>().ok()) {
        let Some((uid, url)) = live.get(&num) else {
            println!("{num}: not in the inventory");
            results.push_str(&format!("{num}\t-\t-\t-\tNOT IN INVENTORY\n"));
            failed += 1;
            continue;
        };
        let part = (num - 1) / 25 + 1;
        let orig_path = std::fs::read_dir(Path::new(&originals).join(format!("p{part}"))).ok().and_then(|rd| {
            rd.flatten().map(|e| e.path()).find(|p| {
                let n = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                n.ends_with(&format!("-U10S_{num:02}.Map.Gbx")) || n.ends_with(&format!("-U10S_{num}.Map.Gbx"))
            })
        });
        let Some(orig_path) = orig_path else {
            println!("{num}: no original under {originals}/p{part}");
            results.push_str(&format!("{num}\t{uid}\t-\t-\tNO ORIGINAL\n"));
            failed += 1;
            continue;
        };
        let live_path = out.join("live").join(format!("{num}.Map.Gbx"));
        if !live_path.is_file() {
            // the inventory's file column is "-" without --fetch-root: ask the core service
            let url: String = if url.starts_with("http") {
                url.clone()
            } else {
                let o = Command::new("curl").args(["-s", "--max-time", "60", "-H", &format!("Authorization: {core}"), &format!("https://prod.trackmania.core.nadeo.online/maps/?mapUidList={uid}")]).output().ok();
                let v: serde_json::Value = o.and_then(|o| serde_json::from_slice(&o.stdout).ok()).unwrap_or(serde_json::Value::Null);
                v.as_array().and_then(|a| a.first()).and_then(|m| m.get("fileUrl")).and_then(|u| u.as_str()).unwrap_or("").to_string()
            };
            if url.is_empty() {
                println!("{num}: no fileUrl for {uid}");
                results.push_str(&format!("{num}\t{uid}\t-\t-\tNO FILEURL\n"));
                failed += 1;
                continue;
            }
            let st = Command::new("curl").args(["-s", "-L", "--max-time", "120", "-H", &format!("Authorization: {core}"), "-o", &live_path.to_string_lossy(), &url]).status();
            if !st.map(|s| s.success()).unwrap_or(false) || std::fs::metadata(&live_path).map(|m| m.len() < 1000).unwrap_or(true) {
                println!("{num}: download failed");
                results.push_str(&format!("{num}\t{uid}\t-\t-\tDOWNLOAD FAILED\n"));
                failed += 1;
                continue;
            }
        }
        // a truncated download (282: 254 KB of a 6 MB file) panics the parser: load under
        // catch_unwind, delete the file and let the next run fetch it again
        let loaded = std::panic::catch_unwind(|| (tmmaps::map::MapFile::load(&orig_path), tmmaps::map::MapFile::load(&live_path)));
        let (orig, tiny) = match loaded {
            Ok(x) => x,
            Err(_) => {
                println!("{num}: the live file does not parse (truncated download?) — deleted, rerun fetches it again");
                std::fs::remove_file(&live_path).ok();
                results.push_str(&format!("{num}\t{uid}\t-\t-\tBAD DOWNLOAD\n"));
                failed += 1;
                continue;
            }
        };
        let tiny_uid = tmmaps::header::read(&live_path.to_string_lossy()).map(|h| h.uid).unwrap_or_default();
        if tiny_uid != *uid {
            println!("{num}: live file uid {tiny_uid} ≠ inventory {uid}");
            results.push_str(&format!("{num}\t{uid}\t-\t-\tUID MISMATCH\n"));
            failed += 1;
            continue;
        }
        // the gates: items by order, blocks by position
        let mut targets: BTreeMap<String, (String, &'static str, bool)> = BTreeMap::new(); // alias → (model/block name, kind, is_block)
        let mut gates = 0usize;
        for (k, it) in orig.items.iter().enumerate() {
            let Some(kind) = kind_of(&it.model) else { continue };
            gates += 1;
            let Some(t) = tiny.items.get(k) else {
                println!("{num}: original item {k} ({}) has no tiny counterpart", it.model);
                continue;
            };
            if !t.model.starts_with("AI") {
                println!("{num}: tiny item {k} is {} (not an AI alias) for {}", t.model, it.model);
                continue;
            }
            targets.insert(t.model.trim_end_matches(".Item.Gbx").to_string(), (it.model.clone(), kind, false));
        }
        // (the gate BLOCKS — `GateGameplay<Kind>` / `GateExpandableGameplay<Kind>` — baked
        // right in the old build too: U10S 277's AC01202040 carries the Rally Decal/Screen
        // with the whole 25-part prefab; only the stock ITEM form had the stadium skin)
        let _ = fit_transform(&orig, &tiny);
        if targets.is_empty() {
            println!("{num}: {gates} gate(s) in the original, none matched in the tiny");
            results.push_str(&format!("{num}\t{uid}\t{gates}\t0\tNO MATCH\n"));
            failed += 1;
            continue;
        }
        // the embedded entries of the live tiny
        let Some((_, zip)) = tiny.embedded_objects() else {
            println!("{num}: the tiny embeds nothing");
            results.push_str(&format!("{num}\t{uid}\t{gates}\t{}\tNO EMBEDDED\n", targets.len()));
            failed += 1;
            continue;
        };
        let entries: BTreeMap<String, Vec<u8>> = tmmaps::header::zip_entries(&zip).into_iter().map(|(n, b)| (n.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(&n).to_string(), b)).collect();
        let work = out.join(format!("w{num}"));
        std::fs::create_dir_all(&work).ok();
        let mut new_files: Vec<PathBuf> = Vec::new();
        let mut ok = true;
        let mut skipped_geometry = 0usize;
        for (alias, (model, kind, is_block)) in &targets {
            let file = format!("{alias}.Item.Gbx");
            let Some(live_bytes) = entries.get(&file) else {
                println!("{num}: alias {file} not among the embedded entries");
                ok = false;
                break;
            };
            let live_item = work.join(format!("live-{file}"));
            std::fs::write(&live_item, live_bytes).ok();
            let live_mats = item_materials(&mapgeom, &live_item);
            if live_mats.iter().any(|m| m.contains(&format!("Modifier\\GateGameplay{kind}"))) {
                println!("{num}: {file} ({model}) already carries the {kind} dress — nothing to do");
                continue;
            }
            let live_vis = item_visuals(&mapgeom, &live_item);
            let n: u32 = alias[2..].parse().unwrap_or(0);
            // first run at base 0: which produced alias is OUR variant (its placement at the
            // block's / item's position); then re-base so that one gets the live alias
            let only = model.clone();
            let run = |base: u32, dir: &Path| -> Result<BTreeMap<String, Vec<u8>>, String> {
                std::fs::create_dir_all(dir).ok();
                let st = Command::new(&mapgeom)
                    .env("TINY_ALIAS_BASE", base.to_string())
                    .args(["--pak", PAK])
                    .args(lod_args.iter())
                    .args(["tiny-library", &orig_path.to_string_lossy(), "--scale", "0.5", "--only", &only, "--library-out", &dir.join("lib.zip").to_string_lossy(), "--mapping-out", &dir.join("placements.tsv").to_string_lossy(), "--report", &dir.join("report.tsv").to_string_lossy()])
                    .output()
                    .map_err(|e| e.to_string())?;
                if !st.status.success() {
                    return Err(String::from_utf8_lossy(&st.stderr).lines().last().unwrap_or("library failed").to_string());
                }
                let z = std::fs::read(dir.join("lib.zip")).map_err(|e| e.to_string())?;
                Ok(tmmaps::header::zip_entries(&z).into_iter().map(|(n, b)| (n.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(&n).to_string(), b)).collect())
            };
            let first = match run(0, &work.join(format!("r0-{alias}"))) {
                Ok(e) => e,
                Err(e) => {
                    println!("{num}: {file}: library: {e}");
                    ok = false;
                    break;
                }
            };
            let produced: Vec<String> = first.keys().filter(|k| k.ends_with(".Item.Gbx")).cloned().collect();
            let prefix = if *is_block { "AC" } else { "AI" };
            // the produced copy whose visuals equal the live item's
            let mut pick: Option<usize> = None;
            for (j, p) in produced.iter().enumerate() {
                let tmp = work.join(format!("cand-{j}-{p}"));
                std::fs::write(&tmp, &first[p]).ok();
                if item_visuals(&mapgeom, &tmp) == live_vis && p.starts_with(prefix) {
                    pick = Some(j);
                    break;
                }
            }
            let Some(j) = pick else {
                // the old build's copy of a gate BLOCK is a 4-part stub (the trigger and sign
                // pieces); today's library bakes the whole gate prefab — a different item, not
                // a skin change: left alone and reported (the stock-ITEM gates match exactly)
                println!("{num}: {file} ({model}): none of {} rebuilt copies has the live item's visuals ({} parts) — skin left as it is", produced.len(), live_vis.len());
                skipped_geometry += 1;
                continue;
            };
            let pj: u32 = produced[j][2..].trim_end_matches(".Item.Gbx").parse().unwrap_or(0);
            let base = n.saturating_sub(pj);
            let second = match run(base, &work.join(format!("r1-{alias}"))) {
                Ok(e) => e,
                Err(e) => {
                    println!("{num}: {file}: library (re-based): {e}");
                    ok = false;
                    break;
                }
            };
            let Some(bytes) = second.get(&file) else {
                println!("{num}: {file} ({model}): the re-based run produced {:?}, not {file}", second.keys().collect::<Vec<_>>());
                ok = false;
                break;
            };
            let new_item = work.join(&file);
            std::fs::write(&new_item, bytes).ok();
            let new_vis = item_visuals(&mapgeom, &new_item);
            let new_mats = item_materials(&mapgeom, &new_item);
            if new_vis != live_vis {
                println!("{num}: {file}: visuals differ after re-basing ({} vs {})", new_vis.len(), live_vis.len());
                ok = false;
                break;
            }
            if !new_mats.iter().any(|m| m.contains(&format!("Modifier\\GateGameplay{kind}"))) {
                println!("{num}: {file} ({model}): the rebuilt item carries no Modifier\\GateGameplay{kind} material: {new_mats:?}");
                ok = false;
                break;
            }
            println!("{num}: {file} ← {model} [{kind}]: {} → {}", live_mats.iter().filter(|m| m.contains("GateGameplay")).cloned().collect::<Vec<_>>().join(","), new_mats.iter().filter(|m| m.contains("GateGameplay")).cloned().collect::<Vec<_>>().join(","));
            new_files.push(new_item);
        }
        if !ok {
            results.push_str(&format!("{num}\t{uid}\t{gates}\t{}\tFAILED\n", targets.len()));
            failed += 1;
            continue;
        }
        if new_files.is_empty() {
            results.push_str(&format!("{num}\t{uid}\t{gates}\t{}\t{}\n", targets.len(), if skipped_geometry > 0 { "SKIPPED (geometry)" } else { "ALREADY OK" }));
            continue;
        }
        let fixed = out.join("fixed").join(format!("U10S_{num} By Everios96 [Tiny].Map.Gbx"));
        let files: Vec<String> = new_files.iter().map(|p| p.to_string_lossy().to_string()).collect();
        let st = Command::new(&tmmaps_bin).args(["replace-embedded", &live_path.to_string_lossy(), "--file", &files.join(","), "--out", &fixed.to_string_lossy()]).output();
        let good = st.as_ref().map(|o| o.status.success()).unwrap_or(false);
        if !good {
            println!("{num}: replace-embedded failed: {}", st.map(|o| String::from_utf8_lossy(&o.stderr).to_string()).unwrap_or_default());
            results.push_str(&format!("{num}\t{uid}\t{gates}\t{}\tREPLACE FAILED\n", targets.len()));
            failed += 1;
            continue;
        }
        // the control: same uid, same blocks/items census, only the embedded bytes changed
        let patched = tmmaps::map::MapFile::load(&fixed);
        let same = patched.blocks.len() == tiny.blocks.len() && patched.items.len() == tiny.items.len() && tmmaps::header::read(&fixed.to_string_lossy()).map(|h| h.uid == *uid).unwrap_or(false);
        if !same {
            println!("{num}: the patched file differs beyond the embedded items — not shipped");
            results.push_str(&format!("{num}\t{uid}\t{gates}\t{}\tCONTROL FAILED\n", targets.len()));
            failed += 1;
            continue;
        }
        println!("{num}: {} gate item(s) re-dressed → {}{}", new_files.len(), fixed.display(), if skipped_geometry > 0 { format!(" ({skipped_geometry} block-gate stub(s) left)") } else { String::new() });
        results.push_str(&format!("{num}\t{uid}\t{gates}\t{}\tPATCHED{}\n", new_files.len(), if skipped_geometry > 0 { " (partial)" } else { "" }));
        manifest.push_str(&format!("{}\tU10S_{num} By Everios96 [Tiny]\n", fixed.display()));
    }
    std::fs::write(out.join("results.tsv"), &results).ok();
    std::fs::write(out.join("manifest.tsv"), &manifest).ok();
    println!("{}", results.trim_end());
    if apply && !manifest.is_empty() {
        let st = Command::new(&me)
            .args(["publish-batch", "--manifest", &out.join("manifest.tsv").to_string_lossy(), "--results", &out.join("upload-results.tsv").to_string_lossy(), "--no-campaign", "--files-only", "--token-dir", &token_dir, "--outdir", &out.join("stage").to_string_lossy()])
            .status();
        if !st.map(|s| s.success()).unwrap_or(false) {
            eprintln!("tiny-regate: the upload batch failed — see {}", out.join("upload-results.tsv").display());
            return 1;
        }
        let up = std::fs::read_to_string(out.join("upload-results.tsv")).unwrap_or_default();
        let bad: Vec<&str> = up.lines().skip(1).filter(|l| !l.ends_with("IDENTICAL")).collect();
        if !bad.is_empty() {
            eprintln!("tiny-regate: {} upload(s) not IDENTICAL:\n{}", bad.len(), bad.join("\n"));
            return 1;
        }
        println!("uploaded in place: {} map(s), all IDENTICAL", up.lines().count().saturating_sub(1));
    }
    let _ = BTreeSet::<String>::new();
    if failed > 0 {
        1
    } else {
        0
    }
}
