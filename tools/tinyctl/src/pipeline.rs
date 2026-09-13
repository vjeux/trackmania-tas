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
        for k in ["--lod-pick", "--lod-pick-min-verts", "--scale"] {
            if let Some(v) = f(k) {
                bargs.push(k.into());
                bargs.push(v);
            }
        }
        // --alias-part P: item file names unique per (part, map) — the game caches an
        // embedded model by FILE NAME for the whole session, and a player of the
        // whole-club campaign plays many maps in one session (2026-09-13). Base
        // (P*100+NN)*1000 leaves 1000 names per map; pictures get a `_pPPNN` suffix.
        if let Some(part) = f("--alias-part").and_then(|p| p.parse::<usize>().ok()) {
            let map_no: usize = nn.parse().unwrap_or(0);
            bargs.push("--env".into());
            bargs.push(format!("TINY_ALIAS_BASE={}", (part * 100 + map_no) * 1000));
            bargs.push("--env".into());
            bargs.push(format!("TINY_PICTURE_SUFFIX=_p{part:02}{map_no:02}"));
        }
        let build_dir = PathBuf::from(&out_root).join(format!("tiny{nn}")).join(&tag);
        let scale = crate::build::scale_of(args);
        let label = crate::build::variant_label(scale);
        let tiny = build_dir.join(crate::build::built_map_name(args, nn));
        let build_t = Instant::now();
        // --reuse-build: a map already built into the build dir (e.g. the one that
        // was published) is shot/published as it is, not rebuilt
        let reuse = tmmaps::cli::has(args, "--reuse-build") && tiny.exists();
        let max_bytes: Option<u64> = f("--max-bytes").and_then(|v| v.parse().ok());
        let build_res = if reuse {
            println!("{nn}: reusing {}", tiny.display());
            Ok(())
        } else {
            crate::build::cmd(&bargs)
        };
        if let Err(e) = build_res {
            eprintln!("{nn}: build FAILED: {e}");
            row.extend(["-".into(), "-".into(), "-".into(), "-".into(), format!("{:.0}", build_t.elapsed().as_secs_f64()), "-".into(), "-".into(), "-".into(), format!("FAILED build: {}", first_line(&e))]);
            append(&tracker, &row)?;
            failed += 1;
            continue;
        }
        // --max-bytes N: a server refuses a map over N bytes (7 MB, vjeux 2026-09-13).
        // A map over the cap is rebuilt down the detail ladder until it fits:
        // (a) the far LOD levels dropped (`--lod-pick 0`: every part at its nearest
        // level, no visual change up close); (b) the HEAVY parts one level coarser,
        // the vertex threshold above which a part goes coarser found by bisection
        // (the largest threshold that fits keeps the most parts sharp); (c) the
        // same one level further. The note records the setting that fit.
        if let Some(cap) = max_bytes.filter(|_| !reuse) {
            let size = |p: &std::path::Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX);
            let mut fit_note = String::new();
            if size(&tiny) >= cap {
                let rebuild = |extra: &[String]| -> Result<u64, String> {
                    let mut a = bargs.clone();
                    a.extend(extra.iter().cloned());
                    crate::build::cmd(&a)?;
                    Ok(size(&tiny))
                };
                let start = size(&tiny);
                let mut chosen: Option<(String, u64)> = None;
                // (a) far levels off
                let s0 = rebuild(&["--lod-pick".into(), "0".into()])?;
                println!("{nn}: {start} B over the {cap} B cap; nearest level only: {s0} B");
                if s0 < cap {
                    chosen = Some(("lod-pick 0 (far levels dropped)".into(), s0));
                }
                // (b), (c): level 1, then 2, then 3 — heavy parts first (bisection on the threshold)
                let mut level = 1u32;
                while chosen.is_none() && level <= 3 {
                    let all = rebuild(&["--lod-pick".into(), level.to_string()])?;
                    println!("{nn}: every part at level {level}: {all} B");
                    if all >= cap {
                        level += 1;
                        continue;
                    }
                    // the largest threshold V (parts under V vertices stay at their nearest level) that fits
                    let (mut lo, mut hi) = (0u32, 40000u32); // lo fits (= all); hi: checked first (only the monster parts coarser)
                    let s_hi = rebuild(&["--lod-pick".into(), level.to_string(), "--lod-pick-min-verts".into(), hi.to_string()])?;
                    println!("{nn}: level {level}, parts under {hi} vertices sharp: {s_hi} B");
                    if s_hi < cap {
                        chosen = Some((format!("lod-pick {level}, parts under {hi} vertices sharp"), s_hi));
                        break;
                    }
                    let mut best = (0u32, all);
                    while hi - lo > 500 {
                        let mid = (lo + hi) / 2;
                        let s = rebuild(&["--lod-pick".into(), level.to_string(), "--lod-pick-min-verts".into(), mid.to_string()])?;
                        println!("{nn}: level {level}, parts under {mid} vertices sharp: {s} B");
                        if s < cap {
                            lo = mid;
                            best = (mid, s);
                        } else {
                            hi = mid;
                        }
                    }
                    if best.0 != lo || size(&tiny) >= cap {
                        // leave the build in the fitting state
                        let s = rebuild(&["--lod-pick".into(), level.to_string(), "--lod-pick-min-verts".into(), best.0.to_string()])?;
                        best.1 = s;
                    }
                    chosen = Some((format!("lod-pick {level}, parts under {} vertices sharp", best.0), best.1));
                }
                match chosen {
                    Some((how, s)) => fit_note = format!("fit under {cap} B: {start} -> {s} B ({how}); "),
                    None => fit_note = format!("DOES NOT FIT under {cap} B even at level 3; "),
                }
            }
            note.push_str(&fit_note);
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
                    // the anchor the SHOOT maps the tiny side's camera through: the
                    // build's own (tmmaps tiny's "anchor:" line in tiny.log — the fit
                    // anchor of a giant build is not the views file's default),
                    // the views file's `# anchor` line as the fallback
                    let built_anchor = std::fs::read_to_string(build_dir.join("tiny2.log")).ok().or_else(|| std::fs::read_to_string(build_dir.join("tiny.log")).ok()).and_then(|t| t.lines().find(|l| l.trim_start().starts_with("anchor: source")).and_then(|l| crate::build::anchor_arg(l.trim())).map(|(a, _)| a));
                    let anchor = built_anchor.or_else(|| std::fs::read_to_string(&views).ok().and_then(|t| t.lines().find(|l| l.starts_with("# anchor ")).map(|l| l.trim_start_matches("# anchor ").trim().to_string())));
                    match anchor {
                        Some(anchor) => {
                            // the box-side tag: `uNN` for the tiny builds (the u10s
                            // runs), `giNN` for the giant ones — the two runs stage
                            // files side by side on the shared box
                            let stag = if scale > 1.0 { format!("gi{nn}") } else { format!("u{nn}") };
                            let mut sargs: Vec<String> = vec!["--orig".into(), src.display().to_string(), "--tiny".into(), tiny.display().to_string(), "--views".into(), views.display().to_string(), "--tag".into(), stag.clone(), "--anchor".into(), anchor, "--scale".into(), format!("{scale}"), "--outdir".into(), frames_dir.display().to_string()];
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
                            // the box's frames (4K PNGs, ~1 MB each here) and staged
                            // maps of this shoot go once the sheets are pulled: C: on
                            // the render PC is at 99 % (2026-09-12)
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
                    // publish_one returns the verdict line (name, uid, mapId, stored verdict)
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
                    // the box's staging of this map (C: is at 99 %, 2026-09-12): the
                    // readback copy and the pushed map go; the done file stays
                    if !tmmaps::cli::has(args, "--keep-box-files") {
                        let wsx = crate::wsx::Wsx::new(args);
                        let _ = wsx.sh(&format!("rm -f /mnt/c/Users/vjeux/tinyshots/{}/readback-*.Map.Gbx /home/vjeux/shoot/_stage/{}", crate::publish::box_publish_dir(label, n), crate::publish::box_stage_name(label, n)));
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

/// `tinyctl tracker-md --publish tracker.tsv [--frames frames.tsv] [--uploads uploads.txt] [--out REPORT.md]`
/// — the tracker rows as one Markdown table (one row per map: the publish run's
/// row joined with the frame run's `frames`/`flagged_views` columns by map
/// number; `--uploads` NAME<TAB>ID rows turn `cmp-uNN<view>` frames into links).
pub fn tracker_md_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let publish = f("--publish").ok_or("tracker-md needs --publish tracker.tsv")?;
    let read_rows = |p: &str| -> Result<Vec<Vec<String>>, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
        Ok(text.lines().skip(1).filter(|l| !l.trim().is_empty()).map(|l| l.split('\t').map(|c| c.to_string()).collect()).collect())
    };
    // the LAST row per map number wins (a re-run supersedes)
    let mut by_nn: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for r in read_rows(&publish)? {
        by_nn.insert(r[0].clone(), r);
    }
    let rows: Vec<Vec<String>> = by_nn.into_values().collect();
    let frames: Vec<Vec<String>> = f("--frames").map(|p| read_rows(&p)).transpose()?.unwrap_or_default();
    let uploads: Vec<(String, String)> = f("--uploads").and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.lines().filter_map(|l| l.split_once(char::is_whitespace).map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))).collect()).unwrap_or_default();
    let col = |r: &[String], i: usize| r.get(i).cloned().unwrap_or_else(|| "-".into());
    let mut out = String::new();
    out.push_str("| # | source | source uid | AT / gold / silver / bronze (s) | tiny uid | tiny size | items (gate) | frames (flagged cells) | Nadeo |\n|---|---|---|---|---|---|---|---|---|\n");
    for r in &rows {
        let nn = col(r, 0);
        let fr = frames.iter().filter(|x| x[0] == nn).last();
        let (frames_s, flagged) = match fr {
            Some(x) => (col(x, 13), col(x, 14)),
            None => (col(r, 13), col(r, 14)),
        };
        let secs = |ms: &str| ms.parse::<f64>().map(|v| format!("{:.3}", v / 1000.0)).unwrap_or_else(|_| ms.to_string());
        let times = format!("{} / {} / {} / {}", secs(&col(r, 4)), secs(&col(r, 5)), secs(&col(r, 6)), secs(&col(r, 7)));
        let size = col(r, 10).parse::<f64>().map(|b| format!("{:.1} MB", b / 1e6)).unwrap_or_else(|_| col(r, 10));
        let mut links: Vec<String> = Vec::new();
        for view in ["start", "finish", "top"] {
            let key = format!("cmp-u{nn}{view}");
            if let Some((_, id)) = uploads.iter().rev().find(|(n, _)| *n == key) {
                links.push(format!("[{view}](/api/attachments/view?file_id={id})"));
            }
        }
        let frames_cell = if links.is_empty() { format!("{frames_s} ({flagged})") } else { format!("{} ({flagged})", links.join(" ")) };
        let nadeo = col(r, 15).replace("OK created mapId ", "created `").replace(" stored IDENTICAL", "` · stored md5 identical");
        out.push_str(&format!("| {nn} | {} | `{}` | {times} | `{}` | {size} | {} ok | {frames_cell} | {nadeo} |\n", col(r, 2), col(r, 3), col(r, 9), col(r, 11)));
    }
    match f("--out") {
        Some(p) => std::fs::write(&p, &out).map_err(|e| format!("{p}: {e}")),
        None => {
            print!("{out}");
            Ok(())
        }
    }
}

/// `tinyctl convert-all --src-root S --out-root O --parts 01,02,…|01-39 [--jobs 8] [--tag u10s]
/// [--out-prefix U10S] [--recipe F] [--env K=V …]` — every part directory `S/pNN/`
/// (maps `NN-<name>.Map.Gbx`) converted into `O/pNN/tinyNN/<tag>/` by `pipeline`
/// (build only: no shoot, no publish), N parts at a time, one tracker per part
/// (`O/pNN/tracker.tsv`) and a summary `O/CONVERT.tsv`. Water blocks are off
/// (`TINY_WATER_BLOCKS=0`: the archetype's clip rims, 2026-09-13).
pub fn convert_all_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let src_root = PathBuf::from(f("--src-root").ok_or("convert-all needs --src-root S")?);
    let out_root = PathBuf::from(f("--out-root").ok_or("convert-all needs --out-root O")?);
    let parts_arg = f("--parts").ok_or("convert-all needs --parts 01,02,… or 01-39")?;
    let parts: Vec<String> = if let Some((a, b)) = parts_arg.split_once('-') {
        let (a, b): (usize, usize) = (a.trim().parse().map_err(|_| "--parts A-B")?, b.trim().parse().map_err(|_| "--parts A-B")?);
        (a..=b).map(|n| format!("{n:02}")).collect()
    } else {
        parts_arg.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    };
    let jobs: usize = f("--jobs").and_then(|j| j.parse().ok()).unwrap_or(8).max(1);
    let tag = f("--tag").unwrap_or_else(|| "u10s".into());
    let prefix = f("--out-prefix").unwrap_or_else(|| "U10S".into());
    let recipe = f("--recipe").unwrap_or_else(|| "/tmp/u10s/recipe.env".into());
    if !std::path::Path::new(&recipe).exists() {
        std::fs::write(&recipe, "").map_err(|e| format!("{recipe}: {e}"))?;
    }
    std::env::set_var("TINY_WATER_BLOCKS", "0");
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
    std::fs::create_dir_all(&out_root).map_err(|e| format!("{}: {e}", out_root.display()))?;
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
                    pargs.extend(["--src-dir".to_string(), src_dir.display().to_string(), "--out-root".into(), part_out.display().to_string(), "--tag".into(), tag.clone(), "--out-prefix".into(), prefix.clone(), "--recipe".into(), recipe.clone(), "--tracker".into(), part_out.join("tracker.tsv").display().to_string(), "--alias-part".into(), part.trim_start_matches('0').to_string(), "--no-shoot".into(), "--no-publish".into()]);
                    pargs.extend(envs.iter().cloned());
                    for k in ["--max-bytes", "--scale"] {
                        if let Some(v) = tmmaps::cli::flag(args, k) {
                            pargs.push(k.into());
                            pargs.push(v.to_string());
                        }
                    }
                    if let Some(only) = tmmaps::cli::flag(args, "--only") {
                        // --only pNN:a,b;pMM:c — restrict each part to the listed map numbers
                        for grp in only.split(';') {
                            if let Some((p, list)) = grp.split_once(':') {
                                if p.trim_start_matches('p').trim().parse::<usize>().ok() == part.parse::<usize>().ok() {
                                    let keep: Vec<String> = list.split(',').map(|s| format!("{:02}", s.trim().parse::<usize>().unwrap_or(0))).collect();
                                    pargs.retain(|a| !(a.len() == 2 && a.chars().all(|c| c.is_ascii_digit())) || keep.contains(a));
                                }
                            }
                        }
                    }
                    let r = cmd(&pargs);
                    let built = (1..=99usize).filter(|n| part_out.join(format!("tiny{n:02}")).join(&tag).join(crate::build::built_map_name(args, &format!("{n:02}"))).exists()).count();
                    match r {
                        Ok(()) => format!("{part}\tOK\t{built}/{} built\t{:.0}s", nums.len(), t1.elapsed().as_secs_f64()),
                        Err(e) => format!("{part}\tPARTIAL\t{built}/{} built\t{:.0}s\t{}", nums.len(), t1.elapsed().as_secs_f64(), e.lines().next().unwrap_or("")),
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
