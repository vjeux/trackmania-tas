//! `jumprig prefabtest` — AR's 2026-10-11 question, measured: can an item that
//! lives in a map's archive (or in the user's Items folder) reach a PACK
//! prefab through its GBX reference table, so a map could carry a 2 KB
//! wrapper instead of the prefab's geometry?
//!
//! The instrument: `mapgeom reref --matrix` writes copies of the game's own
//! `ObstaclePusher16m.Item.Gbx` (whose entity model IS an external reference
//! to a pack prefab) with the reference table spelled from different roots
//! (native `Media\…`, `Stadium\…`, `GameData\Stadium\…`, `Game\Stadium\…`) at
//! ancestor levels 0..6 and useFile 0/1. One set (idents `RL…`) goes LOOSE
//! into Documents\Trackmania\Items, one set (`RE…`) is EMBEDDED in a lineup
//! map next to the stock pusher. The Fid Probe plugin then reports, inside
//! the running game: the drive tree (where user items, pack files and the
//! map's archive are mounted), each loose item preloaded (did its EntityModel
//! resolve?), and every anchored object of the loaded map with its model's
//! EntityModel state, plus the archive's files as the game mounted them.
//! The game's own UGCErrorsLog.txt is copied beside the reports.
//!
//!   jumprig prefabtest --map MAP --loose DIR [--install] [--out DIR]
//!
//! Under one lock from the first install to the last report; the loose items
//! are removed from the Items folder before the lock is released (a stale
//! loose item shadows an embedded one by name — the 2026-09-04 lesson).

use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};
use tmdrive::{ops, GameLock};

use crate::{enter_map, launch_and_hook, plugincmd, read_state, wait_for};

const PROBE: &str = "FidProbe";
const POWERSHELL: &str = "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe";
const SHOTDPI: &str = "C:\\Users\\vjeux\\shotdpi.ps1";

fn items_dir() -> PathBuf {
    PathBuf::from(format!("{}/Users/vjeux/OneDrive/Documents/Trackmania/Items", tmdrive::drive_c()))
}

fn ugc_log() -> PathBuf {
    PathBuf::from(format!("{}/Users/vjeux/OneDrive/Documents/Trackmania/UGCErrorsLog.txt", tmdrive::drive_c()))
}

/// GhostShooter (the map-loading HTTP plugin) is QUARANTINED on this box since
/// 2026-09-27 (Kart-mode crashes); a run that needs `/playmap` borrows the
/// known-good copy from ~/gs-good for its window and removes it again.
fn ghostshooter_dir() -> PathBuf {
    PathBuf::from(format!("{}/Users/vjeux/OpenplanetNext/Plugins/GhostShooter", tmdrive::drive_c()))
}

/// Returns true when the folder was already there (another lane's window): then
/// it is left in place at the end, only a missing one is ours to remove.
fn borrow_ghostshooter() -> Result<bool, String> {
    // the repo's own source set (Main.as + its modules, info.toml); ~/gs-good is a stale
    // 09-25 snapshot whose Main.as names modules it does not carry (Kine.as, PgSync)
    let good = Path::new("/home/vjeux/trackmania-tas/tools/openplanet-plugin");
    let dst = ghostshooter_dir();
    let pre_existing = dst.join("Main.as").exists();
    fs::create_dir_all(&dst).map_err(|e| format!("cannot create {}: {e}", dst.display()))?;
    let mut n = 0;
    for entry in fs::read_dir(good).map_err(|e| format!("{}: {e}", good.display()))? {
        let p = entry.map_err(|e| e.to_string())?.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if p.is_file() && (name.ends_with(".as") || name == "info.toml") {
            fs::copy(&p, dst.join(&name)).map_err(|e| format!("cannot borrow {}: {e}", p.display()))?;
            n += 1;
        }
    }
    println!("  GhostShooter borrowed from {} ({n} files) for this window{}", good.display(), if pre_existing { " (it was already installed; it stays)" } else { "" });
    Ok(pre_existing)
}

/// GhostShooter present AND answering before a step that needs `/playmap` or
/// `/editmap`: another lane's cleanup removed the shared folder mid-run on
/// 2026-10-18 (its HTTP server went with it). Re-borrow the files and have
/// JumpButton load the folder again, then wait for `/ctx`.
fn ensure_ghostshooter(lock: &GameLock) -> Result<(), String> {
    let answering = || tmdrive::plugin::get("/ctx", 5).is_ok();
    if ghostshooter_dir().join("Main.as").exists() && answering() {
        return Ok(());
    }
    println!("  GhostShooter is gone or silent - borrowing it again and loading it in place");
    let _ = borrow_ghostshooter()?;
    let o = crate::send_command(lock.host(), "loadplugin", "GhostShooter", Duration::from_secs(20))?;
    println!("  -> {}", o.state.cmd_result.trim());
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(60) {
        if answering() {
            println!("  GhostShooter answers again ({:.1}s)", start.elapsed().as_secs_f64());
            return Ok(());
        }
        sleep(Duration::from_millis(500));
    }
    Err("GhostShooter did not come back within 60 s of loadplugin".into())
}

fn return_ghostshooter(pre_existing: bool) {
    if pre_existing {
        println!("  GhostShooter left installed (another lane's window)");
        return;
    }
    match fs::remove_dir_all(ghostshooter_dir()) {
        Ok(()) => println!("  GhostShooter removed again (quarantine kept)"),
        Err(e) => println!("  (could not remove GhostShooter: {e})"),
    }
}

fn install_plugins() -> Result<(), String> {
    let repo = std::env::current_exe()
        .ok()
        .and_then(|p| p.ancestors().nth(3).map(|a| a.to_path_buf()))
        .ok_or("cannot locate the repo from the jumprig binary")?;
    let plugins = PathBuf::from(format!("{}/Users/vjeux/OpenplanetNext/Plugins", tmdrive::drive_c()));
    for (dir, files) in [("FidProbe", &["Main.as", "info.toml"][..]), ("JumpButton", &["Main.as", "info.toml"][..])] {
        let src = repo.join("openplanet-plugin").join(dir);
        let dst = plugins.join(dir);
        fs::create_dir_all(&dst).map_err(|e| format!("cannot create {}: {e}", dst.display()))?;
        for f in files {
            fs::copy(src.join(f), dst.join(f)).map_err(|e| format!("cannot install {}/{f}: {e}", dir))?;
        }
        println!("  installed {dir} from {}", src.display());
    }
    Ok(())
}

/// The loose matrix into the game's Items folder; returns what was copied so
/// it can be removed again.
fn install_loose(loose: &Path) -> Result<Vec<PathBuf>, String> {
    let mut copied = Vec::new();
    let dst_dir = items_dir();
    for entry in fs::read_dir(loose).map_err(|e| format!("{}: {e}", loose.display()))? {
        let p = entry.map_err(|e| e.to_string())?.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if !name.to_ascii_lowercase().ends_with(".item.gbx") {
            continue;
        }
        let dst = dst_dir.join(&name);
        fs::copy(&p, &dst).map_err(|e| format!("cannot copy {name} into Items: {e}"))?;
        copied.push(dst);
    }
    println!("  {} loose items into {}", copied.len(), dst_dir.display());
    Ok(copied)
}

fn remove_loose(copied: &[PathBuf]) {
    let mut n = 0;
    for p in copied {
        if fs::remove_file(p).is_ok() {
            n += 1;
        }
    }
    println!("  removed {n} loose items from the Items folder");
}

fn wait_probe_alive(timeout: Duration) -> Result<(), String> {
    let start = Instant::now();
    let dir = PathBuf::from(format!("{}/Users/vjeux/OpenplanetNext/PluginStorage/{PROBE}", tmdrive::drive_c()));
    let mut first: Option<String> = None;
    loop {
        if let Ok(s) = fs::read_to_string(dir.join("state.json")) {
            match &first {
                None => first = Some(s),
                Some(f) if *f != s => return Ok(()),
                _ => {}
            }
        }
        if start.elapsed() > timeout {
            let log = fs::read_to_string(format!("{}/Users/vjeux/OpenplanetNext/Openplanet.log", tmdrive::drive_c())).unwrap_or_default();
            let errs: Vec<&str> = log.lines().filter(|l| l.contains(PROBE) && l.contains(" ERR ")).collect();
            if !errs.is_empty() {
                return Err(format!("{PROBE} did not compile:\n{}", errs.join("\n")));
            }
            return Err(format!("{PROBE} never wrote a moving state.json in {}", dir.display()));
        }
        sleep(Duration::from_millis(250));
    }
}

fn screenshot(file: &Path) -> Result<(), String> {
    let win = tmdrive::game_path(file.to_str().ok_or("screenshot path is not utf-8")?)?.replace('/', "\\");
    let out = std::process::Command::new(POWERSHELL)
        .args(["-ExecutionPolicy", "Bypass", "-File", SHOTDPI, &win])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("powershell: {e}"))?;
    if !out.status.success() {
        return Err(format!("shotdpi.ps1 failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let t0 = Instant::now();
    while t0.elapsed().as_secs() < 10 {
        if fs::metadata(file).map(|m| m.len() > 0).unwrap_or(false) {
            return Ok(());
        }
        sleep(Duration::from_millis(200));
    }
    Err(format!("{}: not written", file.display()))
}

fn save(out: &Path, name: &str, text: &str) {
    if let Err(e) = fs::write(out.join(name), text) {
        println!("  (could not save {name}: {e})");
    }
}

/// A one-line verdict per probed item: resolved (EntityModel is a class) or not.
fn summarize_probe(report: &str) -> (usize, usize, Vec<String>) {
    let mut ok = 0;
    let mut bad = 0;
    let mut lines = Vec::new();
    let mut cur = String::new();
    for l in report.lines() {
        if let Some(name) = l.strip_prefix("- ") {
            cur = name.trim().to_string();
        } else if l.contains("Preload ->") {
            let em = l.split("EntityModel=").nth(1).map(|s| s.split(' ').next().unwrap_or("")).unwrap_or("?");
            let nod = l.split("Preload -> ").nth(1).map(|s| s.split(' ').next().unwrap_or("")).unwrap_or("?");
            let resolved = em != "null" && em != "?" && em != "-";
            if resolved { ok += 1 } else { bad += 1 }
            lines.push(format!("{cur}\tnod={nod}\tEntityModel={em}\t{}", if resolved { "RESOLVED" } else { "unresolved" }));
        }
    }
    (ok, bad, lines)
}

fn summarize_map(report: &str) -> (usize, usize, usize, Vec<String>) {
    let mut kept_ok = 0;
    let mut kept_bad = 0;
    let mut total = 0;
    let mut lines = Vec::new();
    for l in report.lines() {
        if l.trim_start().starts_with('[') && l.contains("EntityModel=") {
            total += 1;
            let model = l.split("] ").nth(1).map(|s| s.split(' ').next().unwrap_or("")).unwrap_or("");
            if !model.starts_with("RE") && !model.starts_with("Ctl") && !model.starts_with("Mir") && !model.starts_with("ObstaclePusher16m") {
                continue;
            }
            let em = l.split("EntityModel=").nth(1).map(|s| s.split(' ').next().unwrap_or("")).unwrap_or("?");
            let resolved = em != "null" && em != "-";
            if resolved { kept_ok += 1 } else { kept_bad += 1 }
            lines.push(format!("{model}\tEntityModel={em}\t{}", if resolved { "RESOLVED" } else { "unresolved" }));
        }
    }
    (kept_ok, kept_bad, total, lines)
}

/// The same map opened in the EDITOR: the game's own "Missing Items" dialog
/// (its control tree captured before it is answered), then the anchored
/// objects the editor kept. GhostShooter reads the map path from its
/// editmap.txt, not from the request.
fn editor_pass(lock: &GameLock, map: &str, out_dir: &Path) -> Result<(), String> {
    // /editmap is a menu-context route: leave the playground first and wait for ctx 0
    let r = ops::plugin(lock, "back", "").map_err(|e| e.to_string())?;
    println!("  /back -> {}", r.trim());
    let t0 = Instant::now();
    loop {
        let c = tmdrive::plugin::get("/ctx", 10).unwrap_or_default();
        if c.contains("\"ctx\":0") && c.contains("\"playground\":false") {
            break;
        }
        if t0.elapsed() > Duration::from_secs(120) {
            return Err(format!("still not at the menu 120 s after /back; ctx: {}", c.trim()));
        }
        sleep(Duration::from_millis(500));
    }
    sleep(Duration::from_secs(2));
    let editmap = PathBuf::from(format!("{}/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter/editmap.txt", tmdrive::drive_c()));
    let p = tmdrive::loadable_map_path(map)?;
    fs::write(&editmap, &p).map_err(|e| format!("cannot write {}: {e}", editmap.display()))?;
    let r = ops::plugin(lock, "editmap", "").map_err(|e| e.to_string())?;
    println!("  /editmap -> {}", r.trim());
    let start = Instant::now();
    let mut dialogs = String::new();
    let mut seen: Vec<String> = Vec::new();
    loop {
        if start.elapsed() > Duration::from_secs(240) {
            return Err("no editor within 240 s after /editmap".into());
        }
        if tmdrive::game_pid(lock.host()).is_none() {
            return Err("the game exited while opening the editor".into());
        }
        let c = tmdrive::plugin::get("/ctx", 10).unwrap_or_default();
        if let Some(i) = c.find("\"dialog\":\"") {
            let frame: String = c[i + 10..].chars().take_while(|ch| *ch != '"').collect();
            if !seen.contains(&frame) {
                seen.push(frame.clone());
                let tree = ops::plugin(lock, "dialogtree", "").map(|s| s).unwrap_or_else(|e| e.to_string());
                dialogs += &format!("=== dialog {frame} at {:.1}s ===\n{tree}\n", start.elapsed().as_secs_f64());
                let text: Vec<&str> = tree.lines().filter(|l| l.contains("Missing") || l.contains(".Item.Gbx") || l.contains("Label")).take(12).collect();
                println!("  dialog {frame}: {}", text.join(" | ").chars().take(400).collect::<String>());
            }
            let _ = ops::plugin(lock, "yes", "");
        }
        if c.contains("\"ctx\":1") && c.contains("\"map\":\"") {
            break;
        }
        sleep(Duration::from_millis(500));
    }
    save(out_dir, "editor-dialogs.txt", &dialogs);
    sleep(Duration::from_secs(3));
    let m = plugincmd::plugin_cmd(PROBE, "map", "", Duration::from_secs(120))?;
    save(out_dir, "map-report-editor.txt", &m);
    let (kept_ok, kept_bad, total, lines) = summarize_map(&m);
    if let Some(l) = m.lines().next() {
        println!("  {l}");
    }
    for l in &lines {
        println!("  {l}");
    }
    println!("  editor anchored objects {total}: matrix/stock resolved {kept_ok}, unresolved {kept_bad}");
    let shot = PathBuf::from(format!("{}/Users/vjeux/tm-video/prefabref-editor.png", tmdrive::drive_c()));
    match screenshot(&shot) {
        Ok(()) => println!("  screenshot {}", shot.display()),
        Err(e) => println!("  (no screenshot: {e})"),
    }
    Ok(())
}

pub fn prefab_test(lock: &GameLock, map: &str, loose: &Path, install: bool, out_dir: &Path, skip_loose: bool, editor: bool) -> Result<(), String> {
    let h = lock.host();
    fs::create_dir_all(out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    println!("=== 0. install ===");
    if install {
        install_plugins()?;
    }
    if tmdrive::game_pid(h).is_some() {
        println!("  a game is running from a previous hold - stopping it (the Items folder is indexed at launch)");
        ops::kill(lock).map_err(|e| e.to_string())?;
        wait_for(h, "exit", Duration::from_secs(60))?;
    }
    let ugc_before = fs::read_to_string(ugc_log()).map(|s| s.lines().count()).unwrap_or(0);
    let copied = if skip_loose { Vec::new() } else { install_loose(loose)? };
    let gs_pre_existing = borrow_ghostshooter()?;
    let result = (|| -> Result<(), String> {
        println!("=== 1. a fresh game with the probe ===");
        launch_and_hook(lock, Duration::from_secs(300))?;
        wait_probe_alive(Duration::from_secs(90))?;
        println!("  probe alive");

        println!("=== 2. the drive tree ===");
        let tree = plugincmd::plugin_cmd(PROBE, "tree", "", Duration::from_secs(60))?;
        save(out_dir, "tree-menu.txt", &tree);
        for l in tree.lines().filter(|l| l.contains("[0]") || l.starts_with("drive") || l.contains("chain:")) {
            println!("  {l}");
        }

        println!("=== 3a. control: the user folder's own items (mesh-modeler, vehicle) preloaded ===");
        let ctl = plugincmd::plugin_cmd(PROBE, "probe", "user Items .Item.Gbx", Duration::from_secs(300))?;
        save(out_dir, "probe-user-items.txt", &ctl);
        let (ok, bad, lines) = summarize_probe(&ctl);
        for l in lines.iter().filter(|l| !l.starts_with("RL")) {
            println!("  {l}");
        }
        println!("  user items: {ok} resolved, {bad} unresolved (RL* included)");
        if skip_loose {
            println!("=== 3. loose matrix skipped (--no-loose) ===");
        } else {
            println!("=== 3. loose items: preload each from the user drive ===");
            let probe = plugincmd::plugin_cmd(PROBE, "probe", "user Items RL", Duration::from_secs(300))?;
            save(out_dir, "probe-loose.txt", &probe);
            let (ok, bad, lines) = summarize_probe(&probe);
            for l in &lines {
                println!("  {l}");
            }
            println!("  loose: {ok} resolved, {bad} unresolved");
            save(out_dir, "loose-summary.tsv", &format!("{}\n", lines.join("\n")));
        }

        println!("=== 4. the lineup map with the embedded matrix ===");
        ensure_ghostshooter(lock)?;
        enter_map(lock, map)?;
        sleep(Duration::from_secs(3));
        let m = plugincmd::plugin_cmd(PROBE, "map", "", Duration::from_secs(120))?;
        save(out_dir, "map-report.txt", &m);
        let (kept_ok, kept_bad, total, lines) = summarize_map(&m);
        for l in m.lines().take(1) {
            println!("  {l}");
        }
        for l in &lines {
            println!("  {l}");
        }
        println!("  anchored objects {total}: matrix/stock resolved {kept_ok}, unresolved {kept_bad}");
        save(out_dir, "embedded-summary.tsv", &format!("{}\n", lines.join("\n")));
        let fake = plugincmd::plugin_cmd(PROBE, "ls", "fake  4", Duration::from_secs(60))?;
        save(out_dir, "fake-drive.txt", &fake);
        let tree2 = plugincmd::plugin_cmd(PROBE, "tree", "", Duration::from_secs(60))?;
        save(out_dir, "tree-inmap.txt", &tree2);
        // a frame of the row for the eyes
        let shot = PathBuf::from(format!("{}/Users/vjeux/tm-video/prefabref-row.png", tmdrive::drive_c()));
        match screenshot(&shot) {
            Ok(()) => println!("  screenshot {}", shot.display()),
            Err(e) => println!("  (no screenshot: {e})"),
        }
        if editor {
            println!("=== 5. the same map in the editor ===");
            ensure_ghostshooter(lock)?;
            editor_pass(lock, map, out_dir)?;
        }
        Ok(())
    })();
    // the game's own complaints, whatever happened
    let ugc = fs::read_to_string(ugc_log()).unwrap_or_default();
    let fresh: Vec<&str> = ugc.lines().skip(ugc_before).collect();
    save(out_dir, "ugc-errors-new.txt", &fresh.join("\n"));
    println!("  UGCErrorsLog.txt: {} new line(s)", fresh.len());
    for l in fresh.iter().take(40) {
        println!("    {l}");
    }
    remove_loose(&copied);
    return_ghostshooter(gs_pre_existing);
    let _ = read_state();
    println!("  evidence in {}", out_dir.display());
    result
}
