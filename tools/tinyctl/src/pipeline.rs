//! `tinyctl pipeline NN… --src-dir DIR --campaign ID --campaign-name NAME …` —
//! one club map end to end from the devserver, the four steps in order and a
//! tracker row per map (2026-09-12, Everios96's u10s maps):
//!
//! 1. `tinyctl build NN` (the converter: library, tiny map, water pass),
//! 2. `tinyctl views SRC` (the comparison cameras) and `tinyctl shoot`
//!    (both sides in the editor on the render box, same cameras, diffed),
//! 3. `tinyctl publish-map NN` (item-check gate, Nadeo upload, campaign
//!    playlist at position NN-1, stored-bytes md5 readback).
//!
//! ```text
//! tinyctl pipeline 01 02 … --src-dir DIR --campaign ID --campaign-name "Tiny u10s (everios96)"
//!                  [--club 43788] [--out-root /tmp/u10s/out] [--tag u10s] [--out-prefix U10S]
//!                  [--recipe /tmp/u10s/recipe.env] [--env K=V …] [--frames-dir /tmp/u10s/frames]
//!                  [--tracker /tmp/u10s/tracker.tsv] [--no-shoot] [--no-publish] [--fresh]
//! ```
//!
//! Every step's failure ends that map's row (`FAILED <step>: <why>`) and the
//! next map starts; the tracker (TSV, one row per map, appended) is the source
//! of the report's table. Nothing here touches ghosts or the map's times.

use std::path::PathBuf;
use std::time::Instant;

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps: Vec<String> = args.iter().take_while(|a| !a.starts_with("--")).cloned().collect();
    if maps.is_empty() || !maps.iter().all(|m| m.len() == 2 && m.chars().all(|c| c.is_ascii_digit())) {
        return Err("pipeline needs two-digit map numbers first (tinyctl pipeline 01 02 … --src-dir DIR …)".into());
    }
    let src_dir = PathBuf::from(f("--src-dir").ok_or("pipeline needs --src-dir DIR (NN-<name>.Map.Gbx sources)")?);
    let out_root = f("--out-root").unwrap_or_else(|| "/tmp/u10s/out".into());
    let tag = f("--tag").unwrap_or_else(|| "u10s".into());
    let prefix = f("--out-prefix").unwrap_or_else(|| "U10S".into());
    let recipe = f("--recipe").unwrap_or_else(|| "/tmp/u10s/recipe.env".into());
    let frames_dir = PathBuf::from(f("--frames-dir").unwrap_or_else(|| "/tmp/u10s/frames".into()));
    let tracker = PathBuf::from(f("--tracker").unwrap_or_else(|| "/tmp/u10s/tracker.tsv".into()));
    let club = f("--club").unwrap_or_else(|| crate::publish::DEFAULT_CLUB.into());
    let no_shoot = tmmaps::cli::has(args, "--no-shoot");
    let no_publish = tmmaps::cli::has(args, "--no-publish");
    let (campaign, campaign_name) = if no_publish {
        (f("--campaign").unwrap_or_default(), f("--campaign-name").unwrap_or_default())
    } else {
        (f("--campaign").ok_or("pipeline needs --campaign ID (tinyctl nadeo-here campaign-create) or --no-publish")?, f("--campaign-name").ok_or("pipeline needs --campaign-name NAME")?)
    };
    if !Path_exists(&recipe) {
        std::fs::write(&recipe, "").map_err(|e| format!("{recipe}: {e}"))?;
    }
    std::fs::create_dir_all(&frames_dir).map_err(|e| format!("{}: {e}", frames_dir.display()))?;
    if let Some(p) = tracker.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    if !tracker.exists() {
        std::fs::write(&tracker, "nn\tsource_file\tsource_name\tsource_uid\tauthortime_ms\tgold\tsilver\tbronze\ttiny_name\ttiny_uid\ttiny_bytes\titems\tbuild_s\tframes\tflagged_views\tnadeo\tnote\n").map_err(|e| format!("{}: {e}", tracker.display()))?;
    }
    // --env K=V pairs go through to build
    let mut envs: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--env" {
            if let Some(v) = args.get(i + 1) {
                envs.push("--env".into());
                envs.push(v.clone());
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    if !envs.iter().any(|e| e.starts_with("TINY_WATER_ROADS=")) {
        envs.push("--env".into());
        envs.push("TINY_WATER_ROADS=0".into());
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
        println!("\n===== {nn}: {} ({}, AT {}) =====", hdr.name, hdr.uid, hdr.authortime);

        // 1. build
        let mut bargs: Vec<String> = vec![nn.clone(), "--src-dir".into(), src_dir.display().to_string(), "--out-root".into(), out_root.clone(), "--tag".into(), tag.clone(), "--recipe".into(), recipe.clone(), "--out-prefix".into(), prefix.clone()];
        bargs.extend(envs.iter().cloned());
        for k in ["--lod-pick", "--lod-pick-min-verts"] {
            if let Some(v) = f(k) {
                bargs.push(k.into());
                bargs.push(v);
            }
        }
        let build_dir = PathBuf::from(&out_root).join(format!("tiny{nn}")).join(&tag);
        let tiny = build_dir.join(format!("{prefix}-{nn}-Tiny.Map.Gbx"));
        let build_t = Instant::now();
        if let Err(e) = crate::build::cmd(&bargs) {
            eprintln!("{nn}: build FAILED: {e}");
            row.extend(["-".into(), "-".into(), "-".into(), "-".into(), format!("{:.0}", build_t.elapsed().as_secs_f64()), "-".into(), "-".into(), "-".into(), format!("FAILED build: {}", first_line(&e))]);
            append(&tracker, &row)?;
            failed += 1;
            continue;
        }
        let thdr = match tmmaps::header::read(&tiny.display().to_string()) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("{nn}: tiny header: {e}");
                row.extend(["-".into(), "-".into(), "-".into(), "-".into(), format!("{:.0}", build_t.elapsed().as_secs_f64()), "-".into(), "-".into(), "-".into(), format!("FAILED tiny header: {}", first_line(&e))]);
                append(&tracker, &row)?;
                failed += 1;
                continue;
            }
        };
        let bytes = std::fs::metadata(&tiny).map(|m| m.len()).unwrap_or(0);
        let n_items = std::fs::read_dir(build_dir.join("libx").join("Items")).map(|rd| rd.filter(|e| e.as_ref().map(|e| e.path().extension().map(|x| x == "Gbx").unwrap_or(false)).unwrap_or(false)).count()).unwrap_or(0);
        row.push(thdr.name.clone());
        row.push(thdr.uid.clone());
        row.push(bytes.to_string());
        row.push(n_items.to_string());
        row.push(format!("{:.0}", build_t.elapsed().as_secs_f64()));
        // the source's times must survive the conversion untouched
        if thdr.authortime != hdr.authortime || thdr.gold != hdr.gold || thdr.silver != hdr.silver || thdr.bronze != hdr.bronze {
            note.push_str(&format!("TIMES CHANGED ({} {} {} {} -> {} {} {} {}); ", hdr.authortime, hdr.gold, hdr.silver, hdr.bronze, thdr.authortime, thdr.gold, thdr.silver, thdr.bronze));
        }

        // 2. views + shoot
        let views = build_dir.join("views.tsv");
        let mut frames = "-".to_string();
        let mut flagged = "-".to_string();
        if !no_shoot {
            match crate::views::cmd(&[src.display().to_string(), "--out".into(), views.display().to_string()]) {
                Ok(()) => {
                    // --views-only start,finish,top: keep those cameras only (each
                    // view costs ~45 s of the shared render box per side)
                    if let Some(keep) = f("--views-only") {
                        let keep: Vec<&str> = keep.split(',').map(|s| s.trim()).collect();
                        let text = std::fs::read_to_string(&views).map_err(|e| format!("{}: {e}", views.display()))?;
                        let kept: Vec<&str> = text.lines().filter(|l| l.starts_with('#') || l.trim().is_empty() || keep.contains(&l.split('\t').next().unwrap_or("").trim())).collect();
                        std::fs::write(&views, kept.join("\n") + "\n").map_err(|e| format!("{}: {e}", views.display()))?;
                    }
                    let anchor = std::fs::read_to_string(&views).ok().and_then(|t| t.lines().find(|l| l.starts_with("# anchor ")).map(|l| l.trim_start_matches("# anchor ").trim().to_string()));
                    match anchor {
                        Some(anchor) => {
                            let stag = format!("u{nn}");
                            let mut sargs: Vec<String> = vec!["--orig".into(), src.display().to_string(), "--tiny".into(), tiny.display().to_string(), "--views".into(), views.display().to_string(), "--tag".into(), stag.clone(), "--anchor".into(), anchor, "--outdir".into(), frames_dir.display().to_string()];
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
                        }
                        None => note.push_str("views: no anchor line; shoot skipped; "),
                    }
                }
                Err(e) => note.push_str(&format!("views: {}; ", first_line(&e))),
            }
        }
        row.push(frames);
        row.push(flagged);

        // 3. publish
        let mut nadeo = "-".to_string();
        if !no_publish {
            let n: usize = nn.parse().unwrap_or(0);
            let paks = crate::build::paks_for(crate::views::collection_of(&tmmaps::map::MapFile::load(&src))).map(|v| v.join(" "));
            match paks {
                Ok(paks) => {
                    let mut pargs: Vec<String> = vec![nn.clone(), "--map".into(), tiny.display().to_string(), "--items-dir".into(), build_dir.join("libx").join("Items").display().to_string(), "--paks".into(), paks, "--name".into(), thdr.name.clone(), "--club".into(), club.clone(), "--campaign".into(), campaign.clone(), "--campaign-name".into(), campaign_name.clone(), "--position".into(), n.saturating_sub(1).to_string(), "--outdir".into(), build_dir.display().to_string()];
                    for k in ["--wsx", "--box-tinyctl"] {
                        if let Some(v) = f(k) {
                            pargs.push(k.into());
                            pargs.push(v);
                        }
                    }
                    if tmmaps::cli::has(args, "--playcheck") {
                        pargs.push("--playcheck".into());
                    }
                    // publish-map prints its verdict line; we re-read the box's done file for the row
                    match crate::publish::publish_map_cmd(&pargs) {
                        Ok(()) => nadeo = "OK stored IDENTICAL".into(),
                        Err(e) => {
                            nadeo = format!("FAILED: {}", first_line(&e));
                            failed += 1;
                        }
                    }
                }
                Err(e) => {
                    nadeo = format!("FAILED paks: {e}");
                    failed += 1;
                }
            }
        }
        row.push(nadeo);
        note.push_str(&format!("{:.0} s total", t0.elapsed().as_secs_f64()));
        row.push(note.trim_end_matches(&[' ', ';'][..]).to_string());
        append(&tracker, &row)?;
        println!("{}", row.join("\t"));
    }
    if failed > 0 {
        return Err(format!("{failed} step(s) failed — see {}", tracker.display()));
    }
    Ok(())
}

#[allow(non_snake_case)]
fn Path_exists(p: &str) -> bool {
    std::path::Path::new(p).exists()
}

fn first_line(e: &str) -> String {
    e.lines().next().unwrap_or("").chars().take(160).collect()
}

fn append(tracker: &std::path::Path, row: &[String]) -> Result<(), String> {
    use std::io::Write;
    let mut fh = std::fs::OpenOptions::new().append(true).create(true).open(tracker).map_err(|e| format!("{}: {e}", tracker.display()))?;
    writeln!(fh, "{}", row.iter().map(|c| c.replace('\t', " ").replace('\n', " ")).collect::<Vec<_>>().join("\t")).map_err(|e| e.to_string())
}
