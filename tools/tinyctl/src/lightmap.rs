//! `tinyctl lightmap MAP… (--out OUT.Map.Gbx | --out-dir DIR) [--quality Q] [--name NAME] [--into SRC=DST]… [--resaved [--keep-uid|--uid U]] [--allow-unbusy]`
//! — the editor's lightmap for a tiny build, computed on the render box and
//! TRANSPLANTED into the input file (the devserver half of `shootctl lightmap`).
//!
//! Why a transplant and not the editor's own save (2026-09-12, the map 11
//! "shadow seam"): a tiny map ships with the SOURCE's stale lightmap, which the
//! game rejects (0 authored blocks), so play mode bakes a coarse per-item
//! lightmap at every load — big flat items come out as lighter/darker
//! rectangles with straight edges (the player report on 11's first-corner
//! ramp), undersides black. The editor can compute a real lightmap for the
//! tiny layout, but its `SaveMap` rewrites the file: it DROPS every embedded
//! .dds (the trees' colour/alpha atlases, LightColor — 46 files on 11), drops
//! the validation ghost, re-mints the uid and re-serialises the items. So the
//! default output is the INPUT file with only chunk 0x0304305B (the lightmap)
//! replaced by the editor's — verified on 11: the game accepts it in play (the
//! item order and count are unchanged, and the zone tiles the editor saw are
//! the ones the game regenerates at load), the ramp is uniform, the textures
//! and the ghost stay. `--resaved` keeps the old behaviour (the editor's file,
//! with `--keep-uid` / `--uid`).
//!
//! The bake must actually RUN: `shootctl lightmap` reports "(never saw the
//! editor busy)" when `/shadows?q=` did not start a compute (the editor skips
//! a quality it already has; q=2 and q=5 came back with the load-time lightmap
//! of the editor session, byte-identical) — that output is refused unless
//! `--allow-unbusy`. `--name` is the map name the saved file carries (SaveMap
//! renames the map to the file's stem); default: the input's own name. The
//! box-side file lands in `Maps/_lightmap/<name>.Map.Gbx`.
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::wsx::Wsx;

const STAGE: &str = "/home/vjeux/shoot/_stage";
const SHOTS: &str = "/mnt/c/Users/vjeux/tinyshots";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";
const LIGHTMAP_CHUNK: u32 = 0x0304_305B;

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(|s| s.to_string());
    // every positional that is not a flag value is a map
    let flag_with_value = ["--out", "--out-dir", "--quality", "--name", "--uid", "--box-shootctl", "--wsx", "--into", "--owner", "--stage-plugin"];
    let mut maps: Vec<PathBuf> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if flag_with_value.contains(&a.as_str()) {
            i += 2;
            continue;
        }
        if a.starts_with("--") || a == "-v" {
            i += 1;
            continue;
        }
        maps.push(PathBuf::from(a));
        i += 1;
    }
    if maps.is_empty() {
        return Err("lightmap needs MAP.Map.Gbx (one or more)".into());
    }
    let out_dir = f("--out-dir").map(PathBuf::from);
    let out_one = f("--out").map(PathBuf::from);
    if maps.len() > 1 && out_dir.is_none() {
        return Err("several maps need --out-dir DIR".into());
    }
    if out_one.is_none() && out_dir.is_none() {
        return Err("lightmap needs --out OUT.Map.Gbx or --out-dir DIR".into());
    }
    if let Some(d) = &out_dir {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let mut failures = Vec::new();
    for map in &maps {
        let out = match (&out_one, &out_dir) {
            (Some(o), None) => o.clone(),
            (_, Some(d)) => d.join(map.file_name().ok_or("map path has no file name")?),
            _ => unreachable!(),
        };
        let r = if tmmaps::cli::has(args, "--reduced") { reduced(args, map, &out) } else { one(args, map, &out) };
        match r {
            Ok(()) => {}
            Err(e) => {
                eprintln!("lightmap {}: {e}", map.display());
                failures.push(format!("{}: {e}", map.display()));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("{} of {} maps failed:\n  {}", failures.len(), maps.len(), failures.join("\n  ")))
    }
}

/// `--reduced`: the editor survives only a map WITHOUT Nadeo's stock vegetation clusters (Grove, Forest,
/// SpringPalmTree, Sparkler16m, ShowFogger8m — ComputeShadows dies at 7 s) and WITHOUT the items that
/// carry local lights (SaveMap dies) — bisected 2026-09-23 on WhiteShore/Stadium. So: build the reduced
/// map (`tmmaps keepitems` of everything else), bake THAT in the editor, and put its lightmap into the
/// full map with `lmtool transplant` (item charts renumbered by the kept list; the dropped items get no
/// chart — the game lights them from the probes; the local-light frames come from lmtool). The sibling
/// binaries (`lmtool`, `tmmaps`) are taken from this executable's directory.
/// The REDUCED copy of a bake copy for the editor's lightmapper (the 2026-09-23
/// bisection: the stock vegetation clusters kill ComputeShadows, the light-carrying
/// items kill SaveMap; `drop_av` also drops our own vegetation statics — GreenCoast):
/// `tmmaps keepitems` of everything else into `reduced_out`, the kept list written
/// to `<out>.kept`. The copy keeps its (stale) lightmap chunk — the editor wants one
/// to work from and does not care which. Returns (kept list path, kept, dropped).
pub fn reduced_copy(map: &Path, out: &Path, reduced_out: &Path, drop_av: bool) -> Result<(PathBuf, usize, usize), String> {
    reduced_copy_n(map, out, reduced_out, drop_av, 0)
}

/// `reduced_copy` with a ceiling on the kept items: beyond `max_items` (> 0) the kept
/// list is thinned evenly (every k-th placement kept) — tiny 22's 15.5k-item reduced copy
/// never opened in the editor (2026-10-01); a thinner copy bakes a lightmap for the kept
/// placements and the rest stay chartless (probe-lit), which beats the source's stale chunk.
pub fn reduced_copy_n(map: &Path, out: &Path, reduced_out: &Path, drop_av: bool, max_items: usize) -> Result<(PathBuf, usize, usize), String> {
    let dir = std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("exe dir")?.to_path_buf();
    let lmtool = dir.join("lmtool");
    let tmmaps_bin = dir.join("tmmaps");
    let run = |bin: &Path, a: &[&str]| -> Result<String, String> {
        let o = std::process::Command::new(bin).args(a).output().map_err(|e| format!("{}: {e}", bin.display()))?;
        if !o.status.success() {
            return Err(format!("{} {}: {}", bin.display(), a.join(" "), String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_string()));
        }
        Ok(String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr))
    };
    let lights_out = run(&lmtool, &["lights", map.to_str().unwrap()])?;
    let light_models: std::collections::HashSet<String> = lights_out
        .lines()
        .filter_map(|l| { let (name, rest) = l.split_once(" (")?; let n: usize = rest.split(' ').next()?.parse().ok()?; if n > 0 && !l.starts_with(' ') { Some(name.to_string()) } else { None } })
        .collect();
    const CLUSTERS: [&str; 6] = ["Grove", "Forest", "Ecotone", "SpringPalmTree", "Sparkler16m", "ShowFogger8m"];
    // every STOCK vegetation item (a Nadeo placement-group or species item: Bush, Flower, Cactus,
    // Tree…, Palm…, Plant, Sugar, Fir, Pine) — the client's lightmapper dies 5 s into
    // ComputeShadows on Fall 2026's GreenCoast maps with the clusters already gone, and the only
    // stock models they carry beyond the clusters are Bush*/Flower*/Ecotone (RedIsland 16: Bush,
    // CactusSmallC; 2026-10-01). A stock vegetation item gets no chart from the editor anyway
    // (lit at runtime through the vegetation path), so dropping it from the BAKE COPY changes
    // nothing in the result: the kept-list transplant leaves it chartless, as the editor would.
    const STOCK_VEGET_PREFIXES: [&str; 12] = ["Bush", "Flower", "Cactus", "Tree", "Palm", "Plant", "Sugar", "Fir", "Pine", "Grass", "Fern", "Spring"];
    let is_stock = |model: &str| !model.ends_with(".Item.Gbx");
    let m = tmmaps::map::MapFile::load(map);
    let kept: Vec<usize> = m
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| {
            let model = it.model.as_str();
            !CLUSTERS.contains(&model)
                && !light_models.contains(&it.model)
                && !(drop_av && model.starts_with("AV") && model.ends_with(".Item.Gbx"))
                && !(is_stock(model) && STOCK_VEGET_PREFIXES.iter().any(|p| model.starts_with(p)))
        })
        .map(|(i, _)| i)
        .collect();
    let kept: Vec<usize> = if max_items > 0 && kept.len() > max_items {
        // thin evenly, the Spawn kept whatever happens (the editor wants a start)
        let step = kept.len() as f64 / max_items as f64;
        let mut thin: Vec<usize> = (0..max_items).map(|j| kept[(j as f64 * step) as usize]).collect();
        if let Some(sp) = m.items.iter().position(|it| it.waypoint_tag.as_deref() == Some("Spawn")) {
            if !thin.contains(&sp) {
                thin.push(sp);
                thin.sort_unstable();
            }
        }
        eprintln!("reduced copy thinned to {} of {} kept items (--reduced-max-items {max_items})", thin.len(), kept.len());
        thin
    } else {
        kept
    };
    let dropped = m.items.len() - kept.len();
    let kept_list = kept.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let kept_path = out.with_extension("kept");
    std::fs::write(&kept_path, &kept_list).map_err(|e| format!("{}: {e}", kept_path.display()))?;
    run(&tmmaps_bin, &["keepitems", map.to_str().unwrap(), "--out", reduced_out.to_str().unwrap(), "--items", &kept_list])?;
    Ok((kept_path, kept.len(), dropped))
}

/// The kept-list transplant: the REDUCED copy's editor lightmap into the full
/// `shipped` file (charts renumbered by the kept list; dropped items chartless).
pub fn transplant_kept(resaved: &Path, shipped: &Path, kept_path: &Path, out: &Path) -> Result<(), String> {
    let dir = std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("exe dir")?.to_path_buf();
    let lmtool = dir.join("lmtool");
    let kept_list = std::fs::read_to_string(kept_path).map_err(|e| format!("{}: {e}", kept_path.display()))?;
    let o = std::process::Command::new(&lmtool).args(["transplant", "--from", resaved.to_str().unwrap(), "--into", shipped.to_str().unwrap(), "--kept", kept_list.trim(), "--out", out.to_str().unwrap()]).output().map_err(|e| format!("lmtool: {e}"))?;
    if !o.status.success() {
        return Err(format!("lmtool transplant --kept: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_string()));
    }
    verify_filetime(out)?;
    report(out, "the kept-list transplant")
}

fn reduced(args: &[String], map: &Path, out: &Path) -> Result<(), String> {
    let dir = std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("exe dir")?.to_path_buf();
    let lmtool = dir.join("lmtool");
    let tmmaps_bin = dir.join("tmmaps");
    let run = |bin: &Path, a: &[&str]| -> Result<String, String> {
        let o = std::process::Command::new(bin).args(a).output().map_err(|e| format!("{}: {e}", bin.display()))?;
        if !o.status.success() {
            return Err(format!("{} {}: {}", bin.display(), a.join(" "), String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_string()));
        }
        Ok(String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr))
    };
    // the light-carrying models: `lmtool lights MAP` lists "<Model> (N placements): L lights"
    let lights_out = run(&lmtool, &["lights", map.to_str().unwrap()])?;
    let light_models: std::collections::HashSet<String> = lights_out
        .lines()
        .filter_map(|l| { let (name, rest) = l.split_once(" (")?; let n: usize = rest.split(' ').next()?.parse().ok()?; if n > 0 && !l.starts_with(' ') { Some(name.to_string()) } else { None } })
        .collect();
    const CLUSTERS: [&str; 5] = ["Grove", "Forest", "SpringPalmTree", "Sparkler16m", "ShowFogger8m"];
    let m = tmmaps::map::MapFile::load(map);
    // --reduced-veget: also drop our own vegetation statics (AV*.Item.Gbx) — GreenCoast tiny 04 with its 5352 of
    // them dies 7 min into the compute, without them it bakes
    let drop_av = tmmaps::cli::has(args, "--reduced-veget");
    let kept: Vec<usize> = m.items.iter().enumerate().filter(|(_, it)| !CLUSTERS.contains(&it.model.as_str()) && !light_models.contains(&it.model) && !(drop_av && it.model.starts_with("AV") && it.model.ends_with(".Item.Gbx"))).map(|(i, _)| i).collect();
    let dropped = m.items.len() - kept.len();
    let kept_list = kept.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    eprintln!("--reduced: {} of {} items kept ({dropped} dropped: {} light-carrying models + the vegetation clusters)", kept.len(), m.items.len(), light_models.len());
    // The kept list is written beside the outputs BEFORE anything runs, so an interrupted run (the box held, the
    // editor dying) can be finished by hand with `lmtool transplant --kept $(cat OUT.kept)` instead of being lost —
    // the g23 WhiteShore oracle of 2026-09-27 02:15Z was baked by the editor and could not be transplanted because
    // the list lived only in this process.
    let kept_path = out.with_extension("kept");
    std::fs::write(&kept_path, &kept_list).map_err(|e| format!("{}: {e}", kept_path.display()))?;
    eprintln!("--reduced: kept list written to {}", kept_path.display());
    if tmmaps::cli::has(args, "--kept-only") {
        return Ok(());
    }
    if dropped == 0 {
        return one(args, map, out);
    }
    let red0 = out.with_extension("reduced0.Map.Gbx");
    let red = out.with_extension("reduced.Map.Gbx");
    let red_ed = out.with_extension("reduced-editor.Map.Gbx");
    run(&tmmaps_bin, &["keepitems", map.to_str().unwrap(), "--out", red0.to_str().unwrap(), "--items", &kept_list])?;
    // a valid (lmtool) chunk in the copy: the editor's lightmapper wants one to work from —
    // `--seed-quality Q` (default: the bake's --quality) keeps that seed cheap on a small
    // devserver: the editor recomputes everything at its own quality anyway (2026-10-01)
    let q = tmmaps::cli::flag(args, "--seed-quality").or(tmmaps::cli::flag(args, "--quality")).unwrap_or("3").to_string();
    run(&lmtool, &["bake", red0.to_str().unwrap(), "--out", red.to_str().unwrap(), "--quality", &q])?;
    one(args, &red, &red_ed)?;
    run(&lmtool, &["transplant", "--from", red_ed.to_str().unwrap(), "--into", map.to_str().unwrap(), "--kept", &kept_list, "--out", out.to_str().unwrap()])?;
    eprintln!("--reduced: wrote {} (the reduced set's editor lightmap in the full map; {dropped} items chartless)", out.display());
    Ok(())
}

fn one(args: &[String], map: &Path, out: &Path) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(|s| s.to_string());
    let quality: u32 = f("--quality").and_then(|q| q.parse().ok()).unwrap_or(3);
    let resaved = tmmaps::cli::has(args, "--resaved");
    if !map.exists() {
        return Err(format!("{}: no such file", map.display()));
    }
    let name = match f("--name") {
        Some(n) => n,
        None => {
            let h = tmmaps::header::read(map.to_str().unwrap_or_default())?;
            if h.name.is_empty() { "Tiny".to_string() } else { h.name }
        }
    };
    // the stem is the map name; keep it file-system clean
    let stem: String = name.chars().map(|c| if c == '/' || c == '\\' || c == ':' { '-' } else { c }).collect();
    let tag = format!("lm{}-{}", std::process::id(), stem.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>());
    let shootctl = f("--box-shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    let wsx = Wsx::new(args);
    let r_map = format!("{STAGE}/{tag}.Map.Gbx");
    let r_dir = format!("{SHOTS}/{tag}");
    let rel = format!("_lightmap/{stem}.Map.Gbx");
    // The game CACHES a computed lightmap by map uid: a second bake of the same uid at
    // the same or a lower quality is skipped by the editor (the q=2/q=5 "never busy"
    // runs of 2026-09-12). The copy that goes to the box carries a fresh uid; the
    // transplant lands in the input, whose uid is untouched.
    let bake_copy = out.with_extension("bakecopy.Map.Gbx");
    {
        let mut m = tmmaps::map::MapFile::load(map);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let old_uid = tmmaps::header::read(map.to_str().unwrap_or_default()).map(|h| h.uid).unwrap_or_default();
        let fresh = fresh_uid_like(&old_uid, nanos % 100_000_000, (nanos / 7) % 100_000_000);
        m.set_map_uid(&fresh);
        // A Nadeo campaign map is editor-LOCKED (header NeedUnlock + chunk 0x03043029) and the
        // converter keeps the chunk: the editor parks the title on a password popup the plugin
        // cannot see (2026-09-29 21:50 PT) — the bake copy goes out unlocked, the shipped file
        // keeps its lock untouched.
        m.remove_password();
        m.write_to(&bake_copy).map_err(|e| format!("{}: {e}", bake_copy.display()))?;
        // The stale lightmap STAYS in the copy: without any lightmap the editor leaves to
        // the menu right after the compute (ctx 0, nothing saved — twice on 11, 2026-09-12);
        // the skip-if-cached problem is handled by the fresh uid plus the game-cache drop.
        eprintln!("bake copy {} with a fresh uid {fresh} (the game caches lightmaps by uid)", bake_copy.display());
    }
    // --fresh: a NEW game process for this bake (the client's lightmapper crashes
    // once a session has loaded ~10 maps; a 75-map batch restarts the game every
    // few bakes and after every failure). Detached on the box under the render
    // lock, like `tinyctl shoot --fresh`.
    if tmmaps::cli::has(args, "--fresh") {
        // `shootctl quit` / `launch --force` kill Trackmania BY IMAGE NAME — on the shared box that once closed vjeux's own game
        // (2026-09-27 14:19 PT). A bake never restarts a game it did not launch; `shootctl lightmap --stage-plugin` launches its
        // own game and kills it by PID at the end, which is the fresh game --fresh wanted.
        return Err("--fresh is refused on the shared box (by-name kills); use --stage-plugin DIR: the run launches its own game and closes it by PID".into());
    }
    eprintln!("pushing {} to the box …", bake_copy.display());
    wsx.push(&bake_copy, &r_map)?;
    // The game also caches computed lightmaps by CONTENT in
    // C:\ProgramData\Trackmania\Cache\<hash>_<hash>_<Collection>_<mood>.Bump.LightMap.zip and
    // serves a cached one instead of computing (a fresh uid does not help — 2026-09-12,
    // 11 with different quality bytes came back "never busy" at q=4). Drop them first.
    let dropped = wsx.sh("ls /mnt/c/ProgramData/Trackmania/Cache/ | grep -c LightMap.zip; rm -f /mnt/c/ProgramData/Trackmania/Cache/*.LightMap.zip").unwrap_or_default();
    eprintln!("game lightmap cache: {} entries dropped", dropped.trim());
    // --owner NAME (tinyctl) → the box lock owner; the default stays shootctl's `lightmap-<pid>`
    let owner_flag = match f("--owner") { Some(o) => format!(" --owner '{o}'"), None => String::new() };
    // --stage-plugin DIR (a box path): the quarantined GhostShooter copied into Plugins for this run only, removed on every exit
    // (vjeux uninstalled every agent plugin 2026-09-27 14:34 PT); the run then also closes its own game by PID at the end
    let owner_flag = match f("--stage-plugin") { Some(d) => format!("{owner_flag} --stage-plugin '{d}'"), None => owner_flag };
    let cmd = format!("{shootctl} lightmap --detach --map {r_map} --out '{rel}' --quality {quality} --outdir {r_dir}{owner_flag}");
    eprintln!("computing the lightmap (quality {quality}) …");
    let started = wsx.sh(&cmd)?;
    if wsx.verbose {
        eprintln!("{}", started.trim());
    }
    let done = wsx.wait_done(&format!("{r_dir}/done.txt"), &format!("{r_dir}/lightmap.log"), Duration::from_secs(3600), "lightmap")?;
    let line = done.lines().next().unwrap_or("").to_string();
    if !line.starts_with("OK") {
        return Err(format!("lightmap: {line}"));
    }
    // did the bake run? the box log says when the editor never went busy
    let log = wsx.cat(&format!("{r_dir}/lightmap.log")).unwrap_or_default();
    let shadows_line = log.lines().find(|l| l.contains("shadows done")).unwrap_or("").trim().to_string();
    if shadows_line.contains("never saw the editor busy") && !tmmaps::cli::has(args, "--allow-unbusy") {
        return Err(format!("the lightmapper never ran on the box ({shadows_line}); the editor skips a quality it already holds — try another --quality, or --allow-unbusy to take the file anyway"));
    }
    eprintln!("{shadows_line}");
    let saved = line.split('\t').nth(1).ok_or("lightmap: no path in the done file")?.to_string();
    let resaved_path: PathBuf = if resaved { out.to_path_buf() } else { out.with_extension("resaved.Map.Gbx") };
    let n = wsx.pull(&saved, &resaved_path)?;
    eprintln!("pulled {} ({n} bytes)", resaved_path.display());
    // THE EDITOR'S SAVE DROPS EVERY SUPPORT FILE of the embedded archive (it re-emits only the item files it uses): a re-save
    // kept as a bake SOURCE has no cut-out masks and bakes its vegetation cards opaque on our side (g23 -source-bake: 566 items /
    // 0 .dds; G2 2026-09-28). Put the input's support files back before anything reads this file.
    let restored_tmp = resaved_path.with_extension("restoring.Map.Gbx");
    match tmmaps::header::restore_support_files(&resaved_path, map, &restored_tmp, false) {
        Ok(added) if added.is_empty() => {}
        Ok(added) => {
            std::fs::rename(&restored_tmp, &resaved_path).map_err(|e| format!("{}: {e}", resaved_path.display()))?;
            eprintln!("the editor's save lost {} support files (textures) of the archive — restored from the input: {}", added.len(), added.join(", "));
        }
        Err(e) => eprintln!("WARNING: support files not restored into the re-save ({e}) — the file may bake with opaque cut-out cards"),
    }

    if resaved {
        // --keep-uid: the editor's re-save mints a new uid; the input's (or --uid U)
        // goes back in, so a publish is an UPDATE of the existing record
        let want_uid = match f("--uid") {
            Some(u) => Some(u),
            None if tmmaps::cli::has(args, "--keep-uid") => Some(tmmaps::header::read(map.to_str().unwrap_or_default())?.uid),
            None => None,
        };
        if let Some(u) = want_uid {
            let mut m = tmmaps::map::MapFile::load(out);
            m.set_map_uid(&u);
            m.write_to(out).map_err(|e| format!("{}: {e}", out.display()))?;
            eprintln!("uid set back to {u}");
        }
        report(out, "the editor's re-save")?;
        return Ok(());
    }

    // the transplant: the input file (and every --into SRC=DST target — a shipped file
    // with the SAME item list, e.g. the validated m5 of the same build) with only the
    // lightmap chunk replaced
    let re = tmmaps::map::MapFile::load(&resaved_path);
    transplant(map, &re, &resaved_path, out)?;
    report(out, "the transplant")?;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--into" {
            let spec = args.get(i + 1).ok_or("--into needs SRC=DST")?;
            let (src, dst) = spec.split_once('=').ok_or("--into wants SRC=DST")?;
            let (src, dst) = (Path::new(src), Path::new(dst));
            if let Some(p) = dst.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            transplant(src, &re, &resaved_path, dst)?;
            report(dst, "the transplant (--into)")?;
            i += 2;
        } else {
            i += 1;
        }
    }
    let _ = std::fs::remove_file(&resaved_path);
    let _ = std::fs::remove_file(&bake_copy);
    Ok(())
}

/// `target` with its lightmap chunk replaced by the editor re-save's, written to `out`
/// LZO-compressed (the shipped form; an uncompressed body is ~1 MB bigger — the Nadeo cap).
/// The lightmap is applied by object index, so the two files must list the same items.
/// The devserver tail of a bake whose editor save was pulled to `resaved`
/// (`lightmap-run`): the support files the editor's save dropped restored from
/// the bake copy, then the lightmap chunk transplanted from the re-save into
/// the SHIPPED file → `out` (the shipped file and the copy carry the same item
/// list, checked by the transplant).
pub fn finish_from_resaved(bake_copy: &Path, resaved: &Path, _copy: &Path, shipped: &Path, out: &Path) -> Result<(), String> {
    let restored_tmp = resaved.with_extension("restoring.Map.Gbx");
    match tmmaps::header::restore_support_files(resaved, bake_copy, &restored_tmp, false) {
        Ok(added) if added.is_empty() => {}
        Ok(added) => {
            std::fs::rename(&restored_tmp, resaved).map_err(|e| format!("{}: {e}", resaved.display()))?;
            eprintln!("the editor's save lost {} support files (textures) of the archive — restored from the bake copy", added.len());
        }
        Err(e) => eprintln!("WARNING: support files not restored into the re-save ({e})"),
    }
    let re = tmmaps::map::MapFile::load(resaved);
    transplant(shipped, &re, resaved, out)?;
    report(out, "the transplant")
}

/// A fresh bake-copy uid of the SAME LENGTH as `old` (the in-place uid patch keeps
/// the byte length: a source uid is 26 or 27 characters — "02b27YA4k3MWlS50A9TFGZy7Eu"
/// gave Fall 2026 - 12 a 26-byte `Tin2…` uid, 2026-10-01): `Tlm1` + hex digits.
pub fn fresh_uid_like(old: &str, a: u32, b: u32) -> String {
    let want = if old.len() >= 20 { old.len() } else { 27 };
    let mut s = format!("Tlm1{:08X}{:07}{:08X}", a, std::process::id() % 10_000_000, b);
    while s.len() < want {
        s.push('0');
    }
    s.truncate(want);
    s
}

/// THE BAKE-COPY RULE (2026-09-12 "shadow seam", restated 2026-10-01 after the Fall 19 audit): a lightmap chunk binds
/// its charts to objects BY INDEX, so the file a chunk is grafted onto must carry the SAME item list as the file it was
/// baked on — same count, same model names in the same order (and the same placements: a moved item reads its chart on
/// the wrong texels). Returns `None` when `a` and `b` agree, else one line naming the first differences.
pub fn item_list_mismatch(a: &tmmaps::map::MapFile, b: &tmmaps::map::MapFile) -> Option<String> {
    if a.items.len() != b.items.len() {
        return Some(format!("item count {} vs {}", a.items.len(), b.items.len()));
    }
    let mut names = Vec::new();
    let mut moved = Vec::new();
    for (i, (x, y)) in a.items.iter().zip(b.items.iter()).enumerate() {
        if x.model != y.model {
            if names.len() < 5 { names.push(format!("item {i}: {} vs {}", x.model, y.model)); }
            continue;
        }
        let d = (0..3).map(|k| (x.pos[k] - y.pos[k]).abs()).fold(0.0f32, f32::max);
        if d > 0.01 || (x.yaw - y.yaw).abs() > 1e-4 {
            if moved.len() < 5 { moved.push(format!("item {i} ({}): pos/yaw differ by {d:.3} m / {:.4} rad", x.model, (x.yaw - y.yaw).abs())); }
        }
    }
    let n_names = a.items.iter().zip(b.items.iter()).filter(|(x, y)| x.model != y.model).count();
    let n_moved = a.items.iter().zip(b.items.iter()).filter(|(x, y)| x.model == y.model && ((0..3).any(|k| (x.pos[k] - y.pos[k]).abs() > 0.01) || (x.yaw - y.yaw).abs() > 1e-4)).count();
    if n_names == 0 && n_moved == 0 {
        return None;
    }
    let mut s = String::new();
    if n_names > 0 { s.push_str(&format!("{n_names} of {} items name a different model ({})", a.items.len(), names.join("; "))); }
    if n_moved > 0 { if !s.is_empty() { s.push_str("; "); } s.push_str(&format!("{n_moved} items moved ({})", moved.join("; "))); }
    Some(s)
}

/// THE CACHE FILETIME RULE after a graft (the tiny 04 case, 2026-10-01): the game keeps a lightmap at load only when
/// cache chunk 0x06022013's word equals the newest CPlugSolid2Model.FileWriteTime over the map's OWN embedded items;
/// a card-less bake copy can carry a newer model than the shipped file, so the editor's word is the copy's and the
/// shipped file plays the coarse load-time bake. `lmtool filetime-check OUT` reads the verdict; on OFF the word is
/// rewritten to the file's own solids (`lmtool filetime-set`, content untouched) and checked again. Default on;
/// `TINY_LIGHTMAP_VERIFY=0` opts out (the check is reported but never acted on or failed).
fn verify_filetime(out: &Path) -> Result<(), String> {
    let dir = std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("exe dir")?.to_path_buf();
    let lmtool = dir.join("lmtool");
    let enforce = std::env::var("TINY_LIGHTMAP_VERIFY").map(|v| v != "0").unwrap_or(true);
    let check = |p: &Path| -> Result<(bool, String), String> {
        let o = std::process::Command::new(&lmtool).args(["filetime-check", p.to_str().unwrap()]).output().map_err(|e| format!("lmtool: {e}"))?;
        let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
        let verdict = text.lines().find(|l| l.trim_start().starts_with('→')).unwrap_or("").trim().to_string();
        Ok((verdict.starts_with("→ EQUAL"), verdict))
    };
    let (ok, verdict) = check(out)?;
    if ok {
        eprintln!("{}: cache FILETIME {verdict}", out.display());
        return Ok(());
    }
    if !enforce {
        eprintln!("WARNING {}: cache FILETIME {verdict} — TINY_LIGHTMAP_VERIFY=0, left as is (the game will play its load-time bake)", out.display());
        return Ok(());
    }
    eprintln!("{}: cache FILETIME {verdict} — rewriting the word to the file's own TimeWriteMostRecentSolid", out.display());
    let fixed = out.with_extension("filetime.Map.Gbx");
    let o = std::process::Command::new(&lmtool).args(["filetime-set", out.to_str().unwrap(), "--out", fixed.to_str().unwrap()]).output().map_err(|e| format!("lmtool: {e}"))?;
    if !o.status.success() {
        let _ = std::fs::remove_file(&fixed);
        return Err(format!("{}: the cache FILETIME word is off ({verdict}) and lmtool filetime-set failed: {}", out.display(), String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("")));
    }
    std::fs::rename(&fixed, out).map_err(|e| format!("{}: {e}", fixed.display()))?;
    let (ok2, verdict2) = check(out)?;
    if !ok2 {
        return Err(format!("{}: the cache FILETIME word is still off after the rewrite ({verdict2}) — not shipped", out.display()));
    }
    eprintln!("{}: cache FILETIME {verdict2} after the rewrite", out.display());
    Ok(())
}

fn transplant(target: &Path, re: &tmmaps::map::MapFile, resaved_path: &Path, out: &Path) -> Result<(), String> {
    let orig = tmmaps::map::MapFile::load(target);
    if let Some(why) = item_list_mismatch(&orig, re) {
        return Err(format!("{}: the editor's save does not carry this file's item list — {why}; the lightmap would bind its charts to the wrong items — not transplanted (the re-save is at {})", target.display(), resaved_path.display()));
    }
    let find = |body: &[u8]| tmmaps::gbx::all_skip_chunks(body).into_iter().find(|c| c.0 == LIGHTMAP_CHUNK);
    let ca = find(&orig.gbx.body).ok_or_else(|| format!("{}: no lightmap chunk 0x{LIGHTMAP_CHUNK:08X} to replace", target.display()))?;
    let cb = find(&re.gbx.body).ok_or_else(|| format!("{}: the editor's save has no lightmap chunk", resaved_path.display()))?;
    // an empty lightmap (24 bytes: version, HasLightmaps=0 …) means the compute produced nothing; a 95-item 2-frame bake is ~96 KB (2026-09-23), so the bar is 20 KB
    // (seen 2026-09-12 right after a game relaunch: "shadows done in 11s, quality 1") — never ship that
    if cb.3 < 20_000 {
        return Err(format!("the editor's save carries only a {}-byte lightmap chunk — the bake produced nothing; not transplanted (re-save kept at {})", cb.3, resaved_path.display()));
    }
    let mut body = Vec::with_capacity(orig.gbx.body.len() + cb.3);
    body.extend_from_slice(&orig.gbx.body[..ca.1]);
    body.extend_from_slice(&re.gbx.body[cb.1..cb.2 + cb.3]);
    body.extend_from_slice(&orig.gbx.body[ca.2 + ca.3..]);
    std::fs::write(out, orig.gbx.write_body_recompressed(&body)).map_err(|e| format!("{}: {e}", out.display()))?;
    eprintln!("{}: lightmap chunk {} -> {} bytes, transplanted (textures, ghost, header, uid untouched) -> {}", target.display(), ca.3, cb.3, out.display());
    verify_filetime(out)?;
    Ok(())
}

fn report(out: &Path, what: &str) -> Result<(), String> {
    let m = tmmaps::map::MapFile::load(out);
    let lm = tmmaps::map::skip_chunks(&m.gbx.body).into_iter().find(|(id, ..)| *id == LIGHTMAP_CHUNK).map(|(_, _, _, size)| size).unwrap_or(0);
    let h = tmmaps::header::read(out.to_str().unwrap_or_default())?;
    let size = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    println!(
        "wrote {} ({what}) — {size} bytes, map name {:?}, uid {}, lightmap chunk {lm} bytes, {} items, {} blocks, {} embedded files",
        out.display(),
        h.name,
        h.uid,
        m.items.len(),
        m.blocks.len(),
        tmmaps::header::embedded_zip(&m.gbx.body).map(|(_, names)| names.len()).unwrap_or(0)
    );
    Ok(())
}

/// `tinyctl lightmap-batch --manifest M.tsv [--quality 4] [--fresh-every 4] [--report R.tsv] [--retries 1]`
/// — the editor bake of many maps, one after the other on the render box: the
/// manifest has one row per map, `copy<TAB>shipped<TAB>out<TAB>name` (the bake
/// copy — same items as the shipped file —, the shipped file the chunk is
/// transplanted into, the lit output, the map name the editor saves under). A
/// fresh game every `--fresh-every` bakes and after any failure (the client's
/// lightmapper dies after ~10 loads); a failed map is retried `--retries`
/// times with a fresh game; a map whose `out` exists is skipped (restartable).
/// One row per map goes to the report: map, verdict, bake seconds, wall
/// seconds, the lit file's bytes. The giant campaigns (2026-09-22): 75 rows,
/// run detached overnight.
pub fn batch(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(|s| s.to_string());
    let manifest = PathBuf::from(f("--manifest").ok_or("lightmap-batch needs --manifest M.tsv")?);
    let quality = f("--quality").unwrap_or_else(|| "4".into());
    let fresh_every: usize = f("--fresh-every").and_then(|v| v.parse().ok()).unwrap_or(4);
    let retries: usize = f("--retries").and_then(|v| v.parse().ok()).unwrap_or(1);
    let report = PathBuf::from(f("--report").unwrap_or_else(|| manifest.with_extension("report.tsv").display().to_string()));
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let rows: Vec<Vec<String>> = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with("copy\t")).map(|l| l.split('\t').map(String::from).collect()).collect();
    if !report.exists() {
        std::fs::write(&report, "copy\tout\tverdict\tbake_s\twall_s\tout_bytes\tattempts\n").map_err(|e| format!("{}: {e}", report.display()))?;
    }
    let mut since_fresh = 0usize;
    let mut failed = 0usize;
    for (i, r) in rows.iter().enumerate() {
        if r.len() < 4 {
            return Err(format!("manifest row {}: wants copy<TAB>shipped<TAB>out<TAB>name", i + 1));
        }
        let (copy, shipped, out, name) = (&r[0], &r[1], &r[2], &r[3]);
        if Path::new(out).exists() {
            println!("{copy}: {out} exists, skipped");
            continue;
        }
        if let Some(p) = Path::new(out).parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        let lit_copy = Path::new(out).with_extension("copy-lit.Map.Gbx");
        let t0 = std::time::Instant::now();
        let mut verdict = String::new();
        let mut bake_s = String::from("-");
        let mut attempts = 0usize;
        while attempts <= retries {
            attempts += 1;
            let fresh = since_fresh >= fresh_every || attempts > 1 || i == 0;
            let mut a: Vec<String> = vec![copy.clone(), "--out".into(), lit_copy.display().to_string(), "--quality".into(), quality.clone(), "--name".into(), name.clone(), "--into".into(), format!("{shipped}={out}")];
            // With --stage-plugin every bake launches its own game and closes it by PID
            // (the quarantine policy of 2026-09-27) — a fresh process per map, so the
            // leak-driven --fresh (refused on the shared box) has nothing left to do.
            if fresh && f("--stage-plugin").is_none() {
                a.push("--fresh".into());
                since_fresh = 0;
            }
            for k in ["--wsx", "--box-shootctl", "--stage-plugin", "--owner"] {
                if let Some(v) = f(k) {
                    a.push(k.into());
                    a.push(v);
                }
            }
            println!("\n===== [{}/{}] {} ({}){} =====", i + 1, rows.len(), name, copy, if fresh { ", fresh game" } else { "" });
            match cmd(&a) {
                Ok(()) => {
                    since_fresh += 1;
                    // the bake's own seconds from the box log line "shadows done … in N s" if printed
                    verdict = "ok".into();
                    break;
                }
                Err(e) => {
                    verdict = format!("FAILED: {}", e.lines().next().unwrap_or("").chars().take(160).collect::<String>());
                    eprintln!("{copy}: attempt {attempts}: {verdict}");
                    let _ = std::fs::remove_file(out);
                    let _ = std::fs::remove_file(&lit_copy);
                    // the lightmapper killing the client is a MAP property (stock vegetation clusters,
                    // light-carrying items — bisected 2026-09-23), not a flake: a retry only costs
                    // another launch; the map goes to the `--reduced` pass instead
                    if e.contains("lightmapper crashed") {
                        verdict = format!("CRASH: {}", e.lines().next().unwrap_or("").chars().take(160).collect::<String>());
                        break;
                    }
                }
            }
        }
        if verdict != "ok" {
            failed += 1;
        } else if let Ok(t) = std::fs::read_to_string(lit_copy.with_extension("log")) {
            bake_s = t.lines().find(|l| l.contains("shadows done")).map(|l| l.trim().to_string()).unwrap_or_else(|| "-".into());
        }
        let out_bytes = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
        let line = format!("{copy}\t{out}\t{verdict}\t{bake_s}\t{}\t{out_bytes}\t{attempts}\n", t0.elapsed().as_secs());
        let mut fh = std::fs::OpenOptions::new().append(true).open(&report).map_err(|e| format!("{}: {e}", report.display()))?;
        std::io::Write::write_all(&mut fh, line.as_bytes()).map_err(|e| e.to_string())?;
        println!("{copy}: {verdict} ({} s)", t0.elapsed().as_secs());
    }
    if failed > 0 {
        return Err(format!("{failed} of {} bakes failed", rows.len()));
    }
    Ok(())
}

/// `tinyctl lightmap-graft --from LIT.Map.Gbx --into SHIPPED=OUT [--into …]` — the
/// transplant alone, no box: chunk 0x0304305B of a lit file (an editor re-save
/// pulled earlier, or `lmtool bake --out` — which writes an UNCOMPRESSED body,
/// 0.5–1.5 MB heavier than the shipped file's LZO stream) goes into the shipped
/// file, whose body is recompressed by tmmaps; the item count must match. The
/// 25 MiB Nadeo cap is checked on every output (2026-09-22, the giant campaigns:
/// 33 of the 75 shipped files are within 2 MB of the cap).
pub fn graft(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(|s| s.to_string());
    let from = PathBuf::from(f("--from").ok_or("lightmap-graft needs --from LIT.Map.Gbx")?);
    let re = tmmaps::map::MapFile::load(&from);
    let mut n = 0usize;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--into" {
            let spec = args.get(i + 1).ok_or("--into needs SRC=DST")?;
            let (src, dst) = spec.split_once('=').ok_or("--into wants SRC=DST")?;
            let (src, dst) = (Path::new(src), Path::new(dst));
            if let Some(p) = dst.parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
            }
            transplant(src, &re, &from, dst)?;
            report(dst, "the graft")?;
            let size = std::fs::metadata(dst).map(|m| m.len()).unwrap_or(0);
            if size > 25 * 1024 * 1024 {
                return Err(format!("{}: {size} bytes is over Nadeo's 25 MiB cap after the lightmap — rebuild the shipped file one rung down", dst.display()));
            }
            n += 1;
            i += 2;
            continue;
        }
        i += 1;
    }
    if n == 0 {
        return Err("lightmap-graft: no --into SRC=DST given".into());
    }
    Ok(())
}
