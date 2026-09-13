//! `tinyctl surface NN… --src-dir DIR --out-root DIR [--surface ice] [--campaign ID
//! --campaign-name NAME | --no-publish] [--no-shoot] …` — the SURFACE SWAP of a
//! club map end to end (2026-09-13, vjeux: "input 'ice' and it'll swap all the
//! road, dirt... blocks into their icy version; if one doesn't exist it'll
//! create a custom item version"):
//!
//! 1. `mapgeom surface-plan SRC --surface S --records R.tsv` — every authored
//!    block's fate: `rename` to its same-mesh twin of surface S (the platform,
//!    deco platform, deco hill, open road, deco wall and water families share
//!    their prefabs across surfaces and differ by a modifier folder), `bake`
//!    for a block with a shape of its own that S has no twin for (every road:
//!    RoadIce is a bobsleigh trough, not a re-dressed tech road; TrackWall;
//!    the tilt/wave/ramp pieces), `same`, `keep` (pillars, structures, gates);
//! 2. `mapgeom tiny-library SRC --scale 1 --only <bake names>` with the surface's
//!    dress (`TINY_FORCE_MODIFIER` = the Platform<S> terrain modifier folder,
//!    `TINY_REDRESS` = the road deck materials → the S road material, the water
//!    sheets off) — a full-size static-item copy of each such block, its deck
//!    LOOKING and DRIVING as S (collision physics from the material);
//! 3. `tmmaps surface SRC --plan R.tsv --library --mapping --out F --name "S <name>"`:
//!    renames, the baked blocks replaced by their items at the very same
//!    origin and quarter turn (waypoint tags carried), a new uid (`Ice2` + the
//!    source's tail, same length), the validation ghost off, medal times kept;
//! 4. `tinyctl views` (start, finish, top) + `tinyctl shoot` through the identity
//!    anchor at scale 1: the same editor cameras on the source and the swap;
//! 5. `tinyctl publish-map` (item-check gate over the baked items, Nadeo upload,
//!    campaign playlist position NN-1, stored-bytes md5 readback).
//!
//! One tracker row per map (TSV): counts of renamed / baked / kept records,
//! frames, the Nadeo verdict. `--reuse-build` skips 1–3 when the output exists.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// The surface's dress for the baked copies: (env, value) pairs.
pub fn surface_env(surface: &str) -> Result<Vec<(String, String)>, String> {
    let s = match surface.to_ascii_lowercase().as_str() {
        "ice" => "Ice",
        "dirt" => "Dirt",
        "grass" => "Grass",
        "plastic" => "Plastic",
        "tech" => "Tech",
        other => return Err(format!("surface `{other}`: ice, dirt, grass, plastic or tech")),
    };
    // the Water sheets STAY (the ice pool `WaterBaseIce` keeps its water, so a
    // RoadWater piece baked in it keeps its sheet too: without it the deck reads
    // as a black rectangle through the pool water — U10S_23, 2026-09-13)
    let mut env: Vec<(String, String)> = Vec::new();
    if s != "Tech" {
        // the platform family's modifier folder (Modifier\Platform<S>\{PlatformTech, TrackWall, DecoHill, …})
        env.push(("TINY_FORCE_MODIFIER".to_string(), format!("Stadium\\Media\\Modifier\\Platform{s}.TerrainModifier.Gbx")));
    }
    // the road decks: every road surface's deck material becomes the S road's
    let road = match s {
        "Ice" => Some("Stadium\\Media\\Material\\RoadIce"),
        "Dirt" => Some("Stadium\\Media\\Material\\RoadDirt"),
        "Tech" | "Grass" | "Plastic" => Some("Stadium\\Media\\Material\\RoadTech"),
        _ => None,
    };
    if let Some(link) = road {
        let rows: Vec<String> = ["RoadTech", "RoadDirt", "RoadBump", "RoadIce"].iter().filter(|stem| !link.ends_with(*stem)).map(|stem| format!("{stem}={link}")).collect();
        env.push(("TINY_REDRESS".to_string(), rows.join(";")));
    }
    Ok(env)
}

fn run(cmd: &mut Command, log: &Path) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("{:?}: {e}", cmd.get_program()))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    std::fs::write(log, &text).map_err(|e| format!("{}: {e}", log.display()))?;
    if !out.status.success() {
        let tail: Vec<&str> = text.lines().rev().take(3).collect();
        return Err(format!("{:?} failed ({}); log {}; {}", cmd.get_program().to_string_lossy().rsplit('/').next().unwrap_or(""), out.status, log.display(), tail.into_iter().rev().collect::<Vec<_>>().join(" | ")));
    }
    Ok(text)
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}

fn append(tracker: &Path, row: &[String]) -> Result<(), String> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(tracker).map_err(|e| format!("{}: {e}", tracker.display()))?;
    writeln!(f, "{}", row.join("\t")).map_err(|e| format!("{}: {e}", tracker.display()))
}

/// The per-record plan's tally: (renamed, baked, same, kept, none) and the
/// names behind `bake`.
fn plan_tally(records: &Path) -> Result<(BTreeMap<String, usize>, Vec<String>), String> {
    let text = std::fs::read_to_string(records).map_err(|e| format!("{}: {e}", records.display()))?;
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    let mut bake_names: Vec<String> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        let (name, action, delete) = (f.get(1).copied().unwrap_or(""), f.get(2).copied().unwrap_or(""), f.get(14).copied().unwrap_or("0"));
        let key = if delete == "1" { "delete" } else { action };
        *tally.entry(key.to_string()).or_default() += 1;
        if action == "bake" && delete != "1" && !bake_names.contains(&name.to_string()) {
            bake_names.push(name.to_string());
        }
    }
    Ok((tally, bake_names))
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps: Vec<String> = args.iter().take_while(|a| !a.starts_with("--")).cloned().collect();
    if maps.is_empty() || !maps.iter().all(|m| m.len() == 2 && m.chars().all(|c| c.is_ascii_digit())) {
        return Err("surface needs two-digit map numbers first (tinyctl surface 01 02 … --src-dir DIR …)".into());
    }
    let surface = f("--surface").unwrap_or_else(|| "ice".into());
    let word = { let mut c = surface.chars(); c.next().map(|x| x.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default() };
    let src_dir = PathBuf::from(f("--src-dir").ok_or("surface needs --src-dir DIR (NN-<name>.Map.Gbx sources)")?);
    let out_root = PathBuf::from(f("--out-root").unwrap_or_else(|| format!("/tmp/surface-{surface}")));
    let frames_dir = PathBuf::from(f("--frames-dir").unwrap_or_else(|| out_root.join("frames").to_string_lossy().into_owned()));
    let tracker = PathBuf::from(f("--tracker").unwrap_or_else(|| out_root.join("tracker.tsv").to_string_lossy().into_owned()));
    let club = f("--club").unwrap_or_else(|| crate::publish::DEFAULT_CLUB.into());
    let no_shoot = tmmaps::cli::has(args, "--no-shoot");
    let no_publish = tmmaps::cli::has(args, "--no-publish");
    let uid_prefix = f("--uid-prefix").unwrap_or_else(|| format!("{}2", &word[..word.len().min(3)]));
    let (campaign, campaign_name) = if no_publish {
        (f("--campaign").unwrap_or_default(), f("--campaign-name").unwrap_or_default())
    } else {
        (f("--campaign").ok_or("surface needs --campaign ID (tinyctl nadeo-here campaign-create) or --no-publish")?, f("--campaign-name").ok_or("surface needs --campaign-name NAME")?)
    };
    let bin_dir = f("--bin-dir").map(PathBuf::from).or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))).ok_or("--bin-dir DIR")?;
    let mapgeom = bin_dir.join("mapgeom");
    let tmmaps = bin_dir.join("tmmaps");
    for p in [&mapgeom, &tmmaps] {
        if !p.exists() {
            return Err(format!("{}: no such binary", p.display()));
        }
    }
    let env = surface_env(&surface)?;
    std::fs::create_dir_all(&frames_dir).map_err(|e| format!("{}: {e}", frames_dir.display()))?;
    if let Some(p) = tracker.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    if !tracker.exists() {
        std::fs::write(&tracker, "nn\tsource_file\tsource_name\tsource_uid\tauthortime_ms\tgold\tsilver\tbronze\tout_name\tout_uid\tout_bytes\trenamed\tbaked\tsame\tkept\tnone\tbake_models\tbuild_s\tframes\tflagged_views\tnadeo\tnote\n").map_err(|e| format!("{}: {e}", tracker.display()))?;
    }
    let mut failed = 0usize;
    for nn in &maps {
        let t0 = Instant::now();
        let mut row: Vec<String> = vec![nn.clone()];
        let mut note = String::new();
        let src = std::fs::read_dir(&src_dir)
            .map_err(|e| format!("{}: {e}", src_dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.file_name().map(|n| n.to_string_lossy().starts_with(&format!("{nn}-")) && n.to_string_lossy().ends_with(".Map.Gbx")).unwrap_or(false));
        let Some(src) = src else {
            eprintln!("{nn}: no {nn}-*.Map.Gbx in {}", src_dir.display());
            failed += 1;
            continue;
        };
        let hdr = match tmmaps::header::read(&src.display().to_string()) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("{nn}: source header: {e}");
                failed += 1;
                continue;
            }
        };
        row.push(src.file_name().unwrap_or_default().to_string_lossy().into_owned());
        row.push(hdr.name.clone());
        row.push(hdr.uid.clone());
        row.push(hdr.authortime.clone());
        row.push(hdr.gold.clone());
        row.push(hdr.silver.clone());
        row.push(hdr.bronze.clone());
        println!("\n===== {nn}: {} ({}, AT {}) → {word} =====", hdr.name, hdr.uid, hdr.authortime);
        let m = tmmaps::map::MapFile::load(&src);
        let coll = crate::views::collection_of(&m);
        let paks = match crate::build::paks_for(coll) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{nn}: {e}");
                failed += 1;
                continue;
            }
        };
        let dir = out_root.join(format!("{surface}{nn}"));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        // --name-style everios: the club's convention (Everios96 to vjeux, 2026-09-13:
        // "U10S_26 By Everios96 [Tiny] — always like this and then the alteration name
        // in the last bracket"; the FILE name too): `U10S_N By Everios96 [Ice]`
        let everios = f("--name-style").as_deref() == Some("everios");
        let out_name = if everios { format!("{} By Everios96 [{word}]", hdr.name) } else { format!("{word} {}", hdr.name) };
        let out_map = if everios { dir.join(format!("{out_name}.Map.Gbx")) } else { dir.join(format!("U10S-{nn}-{word}.Map.Gbx")) };
        let records = dir.join("records.tsv");
        let models = dir.join("models.tsv");
        let build_t = Instant::now();
        let reuse = tmmaps::cli::has(args, "--reuse-build") && out_map.exists() && records.exists();
        let mut bake_models: Vec<String> = Vec::new();
        let build: Result<(), String> = (|| {
            if reuse {
                println!("{nn}: reusing {}", out_map.display());
                return Ok(());
            }
            // 1. the plan
            let mut plan = Command::new(&mapgeom);
            plan.args(&paks).arg("surface-plan").arg(&src).arg("--surface").arg(&surface).arg("--out").arg(&models).arg("--records").arg(&records);
            let text = run(&mut plan, &dir.join("plan.log"))?;
            for l in text.lines().filter(|l| l.starts_with("records:") || l.starts_with("  conflict")) {
                println!("  {}", l.trim());
            }
            let (_tally, names) = plan_tally(&records)?;
            bake_models = names.clone();
            // 2. the baked copies
            let lib = dir.join("lib.zip");
            let mapping = dir.join("placements.tsv");
            let mut with_items = false;
            if !names.is_empty() {
                let mut libcmd = Command::new(&mapgeom);
                libcmd.args(&paks);
                // the far detail levels off: the copies are few and beside the
                // track, and every byte counts against the 7 MB server cap
                let lod = f("--lod-pick").unwrap_or_else(|| "0".into());
                if lod != "-" {
                    libcmd.arg("--lod-pick").arg(&lod);
                }
                libcmd.arg("tiny-library").arg(&src).arg("--scale").arg("1").arg("--only").arg(names.join(",")).arg("--library-out").arg(&lib).arg("--mapping-out").arg(&mapping).arg("--report").arg(dir.join("report.tsv"));
                let map_no: usize = nn.parse().unwrap_or(0);
                libcmd.envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
                // item names unique per (part, map) across the whole club: the game caches
                // an embedded model by FILE NAME for the session (`--alias-part P`, as the
                // tiny pipeline: base (P*100+NN)*1000 + 500 leaves 500 names per map)
                match f("--alias-part").and_then(|p| p.parse::<usize>().ok()) {
                    Some(part) => {
                        libcmd.env("TINY_ALIAS_BASE", format!("{}", (part * 100 + map_no) * 1000 + 500));
                        libcmd.env("TINY_PICTURE_SUFFIX", format!("_{surface}p{part:02}{nn}"));
                    }
                    None => {
                        libcmd.env("TINY_ALIAS_BASE", format!("{}", map_no * 1_000_000 + 700_000));
                        libcmd.env("TINY_PICTURE_SUFFIX", format!("_{surface}{nn}"));
                    }
                }
                let text = run(&mut libcmd, &dir.join("library.log"))?;
                for l in text.lines().filter(|l| l.contains("library:") || l.contains("FAILED")) {
                    println!("  {}", l.trim());
                }
                with_items = lib.exists();
                // the items unpacked beside the map (publish-map's item-check gate)
                let libx = dir.join("libx");
                let _ = std::fs::remove_dir_all(&libx);
                std::fs::create_dir_all(&libx).map_err(|e| format!("{}: {e}", libx.display()))?;
                let unzip = Command::new("unzip").arg("-q").arg(&lib).arg("-d").arg(&libx).output().map_err(|e| format!("unzip: {e}"))?;
                if !unzip.status.success() {
                    return Err(format!("unzip {}: {}", lib.display(), String::from_utf8_lossy(&unzip.stderr)));
                }
            }
            // 3. the map
            let mut sw = Command::new(&tmmaps);
            sw.arg("surface").arg(&src).arg("--plan").arg(&records).arg("--out").arg(&out_map).arg("--name").arg(&out_name).arg("--uid-prefix").arg(&uid_prefix);
            if with_items {
                sw.arg("--library").arg(&lib).arg("--mapping").arg(&mapping);
            }
            if tmmaps::cli::has(args, "--strip-lightmap") {
                sw.arg("--strip-lightmap");
            }
            let text = run(&mut sw, &dir.join("surface.log"))?;
            for l in text.lines().filter(|l| l.starts_with("surface:") || l.contains("stage 3b") || l.contains("stay blocks") || l.starts_with("wrote") || l.trim_start().starts_with("trees:")) {
                println!("  {}", l.trim());
            }
            Ok(())
        })();
        if let Err(e) = build {
            eprintln!("{nn}: build FAILED: {e}");
            row.extend(vec!["-".into(); 9]);
            row.push(format!("{:.0}", build_t.elapsed().as_secs_f64()));
            row.extend(["-".into(), "-".into(), "-".into(), format!("FAILED build: {}", first_line(&e))]);
            append(&tracker, &row)?;
            failed += 1;
            continue;
        }
        let (tally, names) = plan_tally(&records)?;
        if bake_models.is_empty() {
            bake_models = names;
        }
        let ohdr = match tmmaps::header::read(&out_map.display().to_string()) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("{nn}: output header: {e}");
                row.extend(vec!["-".into(); 9]);
                row.push(format!("{:.0}", build_t.elapsed().as_secs_f64()));
                row.extend(["-".into(), "-".into(), "-".into(), format!("FAILED output header: {}", first_line(&e))]);
                append(&tracker, &row)?;
                failed += 1;
                continue;
            }
        };
        let bytes = std::fs::metadata(&out_map).map(|m| m.len()).unwrap_or(0);
        let get = |k: &str| tally.get(k).copied().unwrap_or(0).to_string();
        row.push(ohdr.name.clone());
        row.push(ohdr.uid.clone());
        row.push(bytes.to_string());
        row.push(get("rename"));
        row.push(get("bake"));
        row.push(get("same"));
        row.push(get("keep"));
        row.push(get("none"));
        row.push(bake_models.join(","));
        row.push(format!("{:.0}", build_t.elapsed().as_secs_f64()));
        if ohdr.authortime != hdr.authortime || ohdr.gold != hdr.gold || ohdr.silver != hdr.silver || ohdr.bronze != hdr.bronze {
            note.push_str(&format!("TIMES CHANGED ({} {} {} {} -> {} {} {} {}); ", hdr.authortime, hdr.gold, hdr.silver, hdr.bronze, ohdr.authortime, ohdr.gold, ohdr.silver, ohdr.bronze));
        }
        if bytes >= 7_000_000 {
            note.push_str(&format!("OVER THE 7 MB SERVER CAP ({bytes} B); "));
        }
        println!("  {} ({} B): {} renamed, {} baked, {} same, {} kept, {} none", ohdr.name, bytes, get("rename"), get("bake"), get("same"), get("keep"), get("none"));

        // 4. views + shoot (start, finish, top; the identity anchor at scale 1)
        let mut frames = "-".to_string();
        let mut flagged = "-".to_string();
        if !no_shoot {
            let views_all = dir.join("views-all.tsv");
            let views = dir.join("views.tsv");
            match crate::views::cmd(&[src.display().to_string(), "--out".into(), views_all.display().to_string()]) {
                Ok(()) => {
                    let keep: Vec<&str> = f("--views-only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect::<Vec<_>>()).unwrap_or_else(|| vec!["start".into(), "finish".into(), "top".into()]).leak().iter().map(|s| s.as_str()).collect();
                    let text = std::fs::read_to_string(&views_all).map_err(|e| format!("{}: {e}", views_all.display()))?;
                    let kept: Vec<&str> = text.lines().filter(|l| l.starts_with('#') || l.trim().is_empty() || keep.contains(&l.split('\t').next().unwrap_or("").trim())).collect();
                    std::fs::write(&views, kept.join("\n") + "\n").map_err(|e| format!("{}: {e}", views.display()))?;
                    // the anchor line is the tiny one (target y scaled); the swap is 1:1
                    let anchor = text.lines().find(|l| l.starts_with("# anchor ")).map(|l| l.trim_start_matches("# anchor ").trim().to_string()).and_then(|a| a.split(':').next().map(|s| s.to_string()));
                    match anchor {
                        Some(a) => {
                            let stag = format!("{surface}{nn}");
                            let mut sargs: Vec<String> = vec!["--orig".into(), src.display().to_string(), "--tiny".into(), out_map.display().to_string(), "--views".into(), views.display().to_string(), "--tag".into(), stag.clone(), "--anchor".into(), format!("{a}:{a}"), "--scale".into(), "1".into(), "--outdir".into(), frames_dir.display().to_string()];
                            if tmmaps::cli::has(args, "--fresh") {
                                sargs.push("--fresh".into());
                            }
                            for k in ["--wsx", "--box-shootctl", "--box-tinyctl"] {
                                if let Some(v) = f(k) {
                                    sargs.push(k.into());
                                    sargs.push(v);
                                }
                            }
                            match crate::shoot::cmd(&sargs) {
                                Ok(()) => {
                                    let n_jpg = std::fs::read_dir(&frames_dir).map(|rd| rd.filter(|e| e.as_ref().map(|e| e.file_name().to_string_lossy().starts_with(&format!("cmp-{stag}")) && e.file_name().to_string_lossy().ends_with(".jpg")).unwrap_or(false)).count()).unwrap_or(0);
                                    frames = format!("{n_jpg} pairs");
                                    let tsv = frames_dir.join(format!("cmpdiff-{stag}.tsv"));
                                    flagged = std::fs::read_to_string(&tsv).map(|t| t.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with("view")).count().to_string()).unwrap_or_else(|_| "?".into());
                                }
                                Err(e) => {
                                    frames = "FAILED".into();
                                    note.push_str(&format!("shoot: {}; ", first_line(&e)));
                                }
                            }
                            if !tmmaps::cli::has(args, "--keep-box-files") {
                                let wsx = crate::wsx::Wsx::new(args);
                                let _ = wsx.sh(&format!("rm -rf /mnt/c/Users/vjeux/tinyshots/{stag} /home/vjeux/shoot/_stage/{stag}Orig.Map.Gbx /home/vjeux/shoot/_stage/{stag}Tiny.Map.Gbx /home/vjeux/shoot/_stage/{stag}-views.tsv '/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/{stag}Orig.Map.Gbx' '/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/{stag}Tiny.Map.Gbx'"));
                            }
                        }
                        None => note.push_str("views: no anchor line; shoot skipped; "),
                    }
                }
                Err(e) => note.push_str(&format!("views: {}; ", first_line(&e))),
            }
        }
        row.push(frames);
        row.push(flagged);

        // 5. publish
        let mut nadeo = "-".to_string();
        if !no_publish {
            let n: usize = nn.parse().unwrap_or(0);
            let libx_items = dir.join("libx").join("Items");
            let mut pargs: Vec<String> = vec![nn.clone(), "--map".into(), out_map.display().to_string(), "--name".into(), ohdr.name.clone(), "--club".into(), club.clone(), "--campaign".into(), campaign.clone(), "--campaign-name".into(), campaign_name.clone(), "--position".into(), n.saturating_sub(1).to_string(), "--outdir".into(), dir.display().to_string(), "--any-uid".into()];
            if libx_items.exists() {
                pargs.push("--items-dir".into());
                pargs.push(libx_items.display().to_string());
                pargs.push("--paks".into());
                pargs.push(paks.join(" "));
            }
            for k in ["--wsx", "--box-tinyctl"] {
                if let Some(v) = f(k) {
                    pargs.push(k.into());
                    pargs.push(v);
                }
            }
            match crate::publish::publish_one(n, &pargs, None) {
                Ok(line) => {
                    let toks: Vec<&str> = line.split_whitespace().collect();
                    let map_id = toks.iter().position(|t| *t == "mapId").and_then(|i| toks.get(i + 1)).map(|s| s.to_string()).unwrap_or_else(|| "?".into());
                    let how = if line.contains("\tCREATE\t") { "created" } else if line.contains("\tUPDATE\t") { "updated" } else { "uploaded" };
                    nadeo = format!("OK {how} mapId {map_id} {}", if line.contains("stored IDENTICAL") { "stored IDENTICAL" } else { "stored ?" });
                }
                Err(e) => {
                    nadeo = format!("FAILED: {}", first_line(&e));
                    failed += 1;
                }
            }
            if !tmmaps::cli::has(args, "--keep-box-files") {
                let wsx = crate::wsx::Wsx::new(args);
                let _ = wsx.sh(&format!("rm -f /mnt/c/Users/vjeux/tinyshots/publish-{nn}/readback-*.Map.Gbx /home/vjeux/shoot/_stage/Tiny{nn}.Map.Gbx"));
            }
        }
        row.push(nadeo);
        row.push(note.trim_end_matches("; ").to_string());
        append(&tracker, &row)?;
        println!("{nn}: done in {:.0} s", t0.elapsed().as_secs_f64());
    }
    println!("\ntracker: {}", tracker.display());
    if failed > 0 {
        return Err(format!("{failed} map(s) failed"));
    }
    Ok(())
}

/// `tinyctl surface-report --tracker T.tsv --out-root DIR --frames-dir DIR [--uploads uploads.txt]
/// [--out REPORT.md] [--surface ice] [--campaign-url URL]` — the tracker as a
/// Markdown report: one summary table (the LAST row per map wins), then a
/// section per map with its three before/after frames embedded (uploaded
/// through `tinyctl upload` unless `--uploads` already lists `cmp-<surface>NN<view>`)
/// and the block accounting from the map's plan: which models were renamed to
/// which twin, which were baked as re-dressed copies, which stayed and why.
pub fn report_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let surface = f("--surface").unwrap_or_else(|| "ice".into());
    let word = { let mut c = surface.chars(); c.next().map(|x| x.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default() };
    let out_root = PathBuf::from(f("--out-root").unwrap_or_else(|| format!("/tmp/surface-{surface}")));
    let tracker = PathBuf::from(f("--tracker").unwrap_or_else(|| out_root.join("tracker.tsv").to_string_lossy().into_owned()));
    let frames_dir = PathBuf::from(f("--frames-dir").unwrap_or_else(|| out_root.join("frames").to_string_lossy().into_owned()));
    let uploads_path = PathBuf::from(f("--uploads").unwrap_or_else(|| out_root.join("uploads.txt").to_string_lossy().into_owned()));
    let out = PathBuf::from(f("--out").unwrap_or_else(|| out_root.join("REPORT.md").to_string_lossy().into_owned()));
    let text = std::fs::read_to_string(&tracker).map_err(|e| format!("{}: {e}", tracker.display()))?;
    let mut lines = text.lines();
    let head: Vec<String> = lines.next().unwrap_or("").split('\t').map(|s| s.to_string()).collect();
    let col_of = |n: &str| head.iter().position(|h| h == n).unwrap_or_else(|| panic!("tracker: no column `{n}`"));
    let mut by_nn: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in lines.filter(|l| !l.trim().is_empty()) {
        let r: Vec<String> = l.split('\t').map(|s| s.to_string()).collect();
        by_nn.insert(r[0].clone(), r);
    }
    let col = |r: &[String], n: &str| r.get(col_of(n)).cloned().unwrap_or_else(|| "-".into());
    // uploads: NAME ID rows; missing frames are uploaded now
    let mut uploads: BTreeMap<String, String> = std::fs::read_to_string(&uploads_path).ok().map(|t| t.lines().filter_map(|l| l.split_once(char::is_whitespace).map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))).collect()).unwrap_or_default();
    let views = ["start", "finish", "top"];
    let no_upload = tmmaps::cli::has(args, "--no-upload");
    for nn in by_nn.keys() {
        for v in views {
            let key = format!("cmp-{surface}{nn}{v}");
            if uploads.contains_key(&key) || no_upload {
                continue;
            }
            let p = frames_dir.join(format!("{key}.jpg"));
            if !p.exists() {
                continue;
            }
            match crate::upload::cmd(&[p.display().to_string(), "--record".into(), uploads_path.display().to_string()]) {
                Ok(()) => {
                    // the recorder appended `NAME ID`; read it back
                    if let Some(id) = std::fs::read_to_string(&uploads_path).ok().and_then(|t| t.lines().filter_map(|l| l.split_once(char::is_whitespace).map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))).filter(|(a, _)| *a == key).last().map(|(_, b)| b)) {
                        uploads.insert(key, id);
                    }
                }
                Err(e) => eprintln!("upload {key}: {e}"),
            }
        }
    }
    let secs = |ms: &str| ms.parse::<f64>().map(|v| format!("{:.3}", v / 1000.0)).unwrap_or_else(|_| ms.to_string());
    let mut md = String::new();
    md.push_str(&format!("# {word} U10S — the first 25 U10S maps re-surfaced as {surface}\n\n"));
    if let Some(u) = f("--campaign-url") {
        md.push_str(&format!("Campaign: {u}\n\n"));
    }
    let tot = |n: &str| by_nn.values().map(|r| col(r, n).parse::<usize>().unwrap_or(0)).sum::<usize>();
    md.push_str(&format!("{} maps · {} block records renamed to their {surface} twin · {} baked as re-dressed item copies · {} already {surface} · {} left as they are (pillars, structures, gates, decoration) · {} with no {surface} form at all.\n\n", by_nn.len(), tot("renamed"), tot("baked"), tot("same"), tot("kept"), tot("none")));
    md.push_str("| # | source | AT / gold / silver / bronze (s) | ice map | size | renamed | baked | same | kept | none | frames | Nadeo |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for (nn, r) in &by_nn {
        let times = format!("{} / {} / {} / {}", secs(&col(r, "authortime_ms")), secs(&col(r, "gold")), secs(&col(r, "silver")), secs(&col(r, "bronze")));
        let size = col(r, "out_bytes").parse::<f64>().map(|b| format!("{:.2} MB", b / 1e6)).unwrap_or_else(|_| col(r, "out_bytes"));
        let nadeo = col(r, "nadeo").replace("OK created mapId ", "created `").replace("OK updated mapId ", "updated `").replace(" stored IDENTICAL", "` · md5 identical");
        // frames: what is on disk (a publish-only re-run's row says "-")
        let n_frames = views.iter().filter(|v| frames_dir.join(format!("cmp-{surface}{nn}{v}.jpg")).exists()).count();
        let frames = if n_frames > 0 { format!("{n_frames} pairs") } else { col(r, "frames") };
        md.push_str(&format!("| {nn} | {} `{}` | {times} | {} `{}` | {size} | {} | {} | {} | {} | {} | {frames} | {nadeo} |\n", col(r, "source_name"), col(r, "source_uid"), col(r, "out_name"), col(r, "out_uid"), col(r, "renamed"), col(r, "baked"), col(r, "same"), col(r, "kept"), col(r, "none")));
    }
    md.push_str("\nEvery map keeps the source's medal times, items, MediaTracker and thumbnail; the uid is `Ice2` + the source uid's tail; the validation ghost (an author run on another surface) is removed.\n\n");
    for (nn, r) in &by_nn {
        md.push_str(&format!("## {nn} — {} → {}\n\n", col(r, "source_name"), col(r, "out_name")));
        let note = col(r, "note");
        if !note.is_empty() && note != "-" {
            md.push_str(&format!("Note: {note}\n\n"));
        }
        for v in views {
            let key = format!("cmp-{surface}{nn}{v}");
            match uploads.get(&key) {
                Some(id) => md.push_str(&format!("**{v}** (left: original · right: {surface})\n\n![{key}](/api/attachments/view?file_id={id})\n\n")),
                None => md.push_str(&format!("**{v}**: no frame\n\n")),
            }
        }
        // the trees (tmmaps surface's `trees:` line in surface.log)
        if let Some(l) = std::fs::read_to_string(out_root.join(format!("{surface}{nn}")).join("surface.log")).ok().and_then(|t| t.lines().find(|l| l.trim_start().starts_with("trees:")).map(|l| l.trim().to_string())) {
            let body = l.trim_start_matches("trees:").trim();
            md.push_str(&format!("**Trees:** {}\n\n", body.replace(" -> ", " → ")));
        }
        // the block accounting from models.tsv
        let models = out_root.join(format!("{surface}{nn}")).join("models.tsv");
        if let Ok(t) = std::fs::read_to_string(&models) {
            let mut groups: BTreeMap<&str, Vec<String>> = BTreeMap::new();
            for l in t.lines().skip(1).filter(|l| !l.trim().is_empty()) {
                let c: Vec<&str> = l.split('\t').collect();
                let (name, count, action, target, detail) = (c[0], c[1], c[2], c.get(3).copied().unwrap_or(""), c.get(5).copied().unwrap_or(""));
                let entry = match action {
                    "rename" => format!("{name} → {target} ×{count}"),
                    "bake" => format!("{name} ×{count} ({})", detail.split(" — ").next().unwrap_or(detail).replace("different mesh: ", "")),
                    "same" => format!("{name} ×{count}"),
                    "none" => format!("{name} ×{count} ({detail})"),
                    _ => format!("{name} ×{count}"),
                };
                groups.entry(action).or_default().push(entry);
            }
            for (action, label) in [("rename", "Renamed to the ice twin"), ("bake", "Baked as re-dressed item copies (no same-shape ice block)"), ("same", "Already ice"), ("none", "No ice form — left as is"), ("keep", "Not a surface (left as is)")] {
                if let Some(v) = groups.get(action) {
                    md.push_str(&format!("**{label}:** {}\n\n", v.join(" · ")));
                }
            }
        }
    }
    std::fs::write(&out, &md).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("wrote {} ({} maps, {} frames linked)", out.display(), by_nn.len(), uploads.len());
    Ok(())
}

/// `tinyctl surface-all --src-root S --out-root O --parts 01-39 [--jobs 12] [--surface ice]
/// [--dist DIR]` — every part directory `S/pNN/` (maps `NN-<name>.Map.Gbx`)
/// converted by `surface` (build only: no shoot, no publish; the club's naming
/// `U10S_N By Everios96 [Ice]`; item names unique per part), N parts at a time,
/// one tracker per part (`O/pNN/tracker.tsv`), then one zip per part in `--dist`
/// (`<Word>U10S-partNN.zip`: the 25 maps in campaign order + `MAPS.tsv`) and a
/// summary `O/CONVERT.tsv` — the GitHub-release layout of the tiny set
/// (2026-09-13, vjeux: "the 1k giant and icy uploaded to github like you did for tiny").
pub fn all_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let src_root = PathBuf::from(f("--src-root").ok_or("surface-all needs --src-root S")?);
    let out_root = PathBuf::from(f("--out-root").ok_or("surface-all needs --out-root O")?);
    let surface = f("--surface").unwrap_or_else(|| "ice".into());
    let word = { let mut c = surface.chars(); c.next().map(|x| x.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default() };
    let dist = PathBuf::from(f("--dist").unwrap_or_else(|| out_root.join("dist").to_string_lossy().into_owned()));
    let parts_arg = f("--parts").ok_or("surface-all needs --parts 01,02,… or 01-39")?;
    let parts: Vec<String> = if let Some((a, b)) = parts_arg.split_once('-') {
        let (a, b): (usize, usize) = (a.trim().parse().map_err(|_| "--parts A-B")?, b.trim().parse().map_err(|_| "--parts A-B")?);
        (a..=b).map(|n| format!("{n:02}")).collect()
    } else {
        parts_arg.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    };
    let jobs: usize = f("--jobs").and_then(|j| j.parse().ok()).unwrap_or(8).max(1);
    std::fs::create_dir_all(&out_root).map_err(|e| format!("{}: {e}", out_root.display()))?;
    std::fs::create_dir_all(&dist).map_err(|e| format!("{}: {e}", dist.display()))?;
    let queue = std::sync::Mutex::new(std::collections::VecDeque::from(parts.clone()));
    let results = std::sync::Mutex::new(Vec::<String>::new());
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..jobs.min(parts.len()) {
            s.spawn(|| loop {
                let part = match queue.lock().unwrap().pop_front() {
                    Some(p) => p,
                    None => break,
                };
                let src_dir = src_root.join(format!("p{part}"));
                let part_out = out_root.join(format!("p{part}"));
                let _ = std::fs::create_dir_all(&part_out);
                let mut nums: Vec<String> = std::fs::read_dir(&src_dir)
                    .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_string_lossy().split('-').next().filter(|s| s.len() == 2 && s.chars().all(|c| c.is_ascii_digit())).map(|s| s.to_string())).collect())
                    .unwrap_or_default();
                nums.sort();
                nums.dedup();
                let t1 = Instant::now();
                let line = if nums.is_empty() {
                    format!("{part}\tFAILED\tno NN-*.Map.Gbx in {}", src_dir.display())
                } else {
                    let mut pargs: Vec<String> = nums.clone();
                    pargs.extend(["--src-dir".to_string(), src_dir.display().to_string(), "--out-root".into(), part_out.display().to_string(), "--surface".into(), surface.clone(), "--tracker".into(), part_out.join("tracker.tsv").display().to_string(), "--alias-part".into(), part.trim_start_matches('0').to_string(), "--name-style".into(), "everios".into(), "--no-shoot".into(), "--no-publish".into()]);
                    if tmmaps::cli::has(args, "--reuse-build") {
                        pargs.push("--reuse-build".into());
                    }
                    for k in ["--lod-pick", "--bin-dir"] {
                        if let Some(v) = f(k) {
                            pargs.push(k.into());
                            pargs.push(v);
                        }
                    }
                    let r = cmd(&pargs);
                    // the part's zip: the maps in campaign order + MAPS.tsv
                    let z = part_zip(&part_out, &part, &word, &dist);
                    match (r, z) {
                        (Ok(()), Ok((n, bytes, over))) => format!("{part}\tOK\t{n}/{} maps\t{bytes} B zip\t{over} over 7 MB\t{:.0}s", nums.len(), t1.elapsed().as_secs_f64()),
                        (Err(e), Ok((n, bytes, over))) => format!("{part}\tPARTIAL\t{n}/{} maps\t{bytes} B zip\t{over} over 7 MB\t{:.0}s\t{}", nums.len(), t1.elapsed().as_secs_f64(), e.lines().next().unwrap_or("")),
                        (_, Err(e)) => format!("{part}\tFAILED\tzip: {e}"),
                    }
                };
                eprintln!("[{:>5.0}s] {line}", t0.elapsed().as_secs_f64());
                results.lock().unwrap().push(line);
            });
        }
    });
    let mut lines = results.into_inner().unwrap();
    lines.sort();
    let summary = out_root.join("CONVERT.tsv");
    std::fs::write(&summary, lines.join("\n") + "\n").map_err(|e| format!("{}: {e}", summary.display()))?;
    println!("{}", lines.join("\n"));
    let bad = lines.iter().filter(|l| !l.contains("\tOK\t")).count();
    if bad > 0 {
        return Err(format!("{bad} of {} parts not fully built — see {}", parts.len(), summary.display()));
    }
    Ok(())
}

/// One part's release zip from its tracker: `<Word>U10S-partNN.zip` with the
/// maps (the tracker's LAST row per map, in map order) and `MAPS.tsv`.
/// Returns (maps in the zip, zip bytes, maps over 7 000 000 B).
fn part_zip(part_out: &Path, part: &str, word: &str, dist: &Path) -> Result<(usize, u64, usize), String> {
    let tracker = part_out.join("tracker.tsv");
    let text = std::fs::read_to_string(&tracker).map_err(|e| format!("{}: {e}", tracker.display()))?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let col = |r: &[String], n: &str| head.iter().position(|h| *h == n).and_then(|i| r.get(i).cloned()).unwrap_or_else(|| "-".into());
    let mut by_nn: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in lines.filter(|l| !l.trim().is_empty()) {
        let r: Vec<String> = l.split('\t').map(|s| s.to_string()).collect();
        by_nn.insert(r[0].clone(), r);
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut tsv = String::from("nn\tfile\tsource_name\tsource_uid\tauthortime_ms\tgold\tsilver\tbronze\tuid\tbytes\trenamed\tbaked\tsame\tkept\tnone\tnote\n");
    let (mut n, mut over) = (0usize, 0usize);
    let surface = word.to_ascii_lowercase();
    for (nn, r) in &by_nn {
        let name = col(r, "out_name");
        if name == "-" {
            tsv.push_str(&format!("{nn}\t-\t{}\t{}\t{}\t{}\t{}\t{}\t-\t-\t-\t-\t-\t-\t-\t{}\n", col(r, "source_name"), col(r, "source_uid"), col(r, "authortime_ms"), col(r, "gold"), col(r, "silver"), col(r, "bronze"), col(r, "note")));
            continue;
        }
        let file = format!("{name}.Map.Gbx");
        let path = part_out.join(format!("{surface}{nn}")).join(&file);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if bytes.len() >= 7_000_000 {
            over += 1;
        }
        n += 1;
        // the zip entry: `NN-<name>.Map.Gbx` keeps campaign order in a file listing
        files.insert(format!("{nn}-{file}"), bytes.clone());
        tsv.push_str(&format!("{nn}\t{nn}-{file}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", col(r, "source_name"), col(r, "source_uid"), col(r, "authortime_ms"), col(r, "gold"), col(r, "silver"), col(r, "bronze"), col(r, "out_uid"), bytes.len(), col(r, "renamed"), col(r, "baked"), col(r, "same"), col(r, "kept"), col(r, "none"), col(r, "note")));
    }
    files.insert("MAPS.tsv".into(), tsv.into_bytes());
    // the maps are LZO-compressed GBX already: store them (deflate gains ~1 %
    // and costs a minute per part) — the tmmaps writer deflates; keep it simple
    let zip = tmmaps::header::deflated_zip(&files);
    let out = dist.join(format!("{word}U10S-part{part}.zip"));
    std::fs::write(&out, &zip).map_err(|e| format!("{}: {e}", out.display()))?;
    std::fs::write(part_out.join("MAPS.tsv"), files.get("MAPS.tsv").cloned().unwrap_or_default()).map_err(|e| format!("{}: {e}", part_out.display()))?;
    Ok((n, zip.len() as u64, over))
}
