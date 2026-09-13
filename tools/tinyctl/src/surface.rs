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
    let mut env = vec![("TINY_KEEP_WATER".to_string(), "0".to_string())];
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
        let out_map = dir.join(format!("U10S-{nn}-{word}.Map.Gbx"));
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
                libcmd.env("TINY_ALIAS_BASE", format!("{}", map_no * 1_000_000 + 700_000));
                libcmd.env("TINY_PICTURE_SUFFIX", format!("_{surface}{nn}"));
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
            sw.arg("surface").arg(&src).arg("--plan").arg(&records).arg("--out").arg(&out_map).arg("--name").arg(format!("{word} {}", hdr.name)).arg("--uid-prefix").arg(&uid_prefix);
            if with_items {
                sw.arg("--library").arg(&lib).arg("--mapping").arg(&mapping);
            }
            if tmmaps::cli::has(args, "--strip-lightmap") {
                sw.arg("--strip-lightmap");
            }
            let text = run(&mut sw, &dir.join("surface.log"))?;
            for l in text.lines().filter(|l| l.starts_with("surface:") || l.contains("stage 3b") || l.contains("stay blocks") || l.starts_with("wrote")) {
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
