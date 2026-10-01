//! `tinyctl bake-run` (ON THE BOX, under `tmdrive run`) and `tinyctl lightmap-run`
//! (the devserver half) — the editor's lightmap for SEVERAL maps in ONE game.
//!
//! Why: `shootctl lightmap --stage-plugin` launches and closes a game per map
//! (the quarantine policy of 2026-09-27: no agent plugin lives in Openplanet's
//! Plugins folder; a run stages its own and takes it out before the box is
//! released), and every launch after a by-PID kill goes through a COLD Ubisoft
//! launcher chain — ~150 s before the compute even starts, against a 50-s q4
//! compute (Fall 2026, 2026-10-01: 4 min per map, 50 maps). The coordinator's
//! word: one hold = stage once → launch once → bake up to N maps in that game
//! (the client leaks across loads; 4 is the Summer batches' safe count) → close
//! by PID → unstage; a crash or a refused load → relaunch. Per-map rules stay:
//! the game's *.LightMap.zip cache dropped before each bake, a fresh uid on the
//! copy (the devserver side does that), the fail-closed process probe before
//! each launch.
//!
//! ```text
//! box:  tinyctl bake-run --maps A.Map.Gbx,B.Map.Gbx,… --quality 4 --report R.tsv [--done D]
//!                        [--max-maps 4] [--stage-plugin DIR] [--shootctl PATH]
//!       (maps = WSL paths of the staged copies; the saved files land in
//!        Maps/_lightmap/<stem>.Map.Gbx; the report row: map, saved, verdict, secs)
//! dev:  tinyctl lightmap-run --manifest M.tsv --quality 4 [--group 4] [--stage-plugin DIR]
//!                            [--report R.tsv] [--box-tinyctl PATH]
//!       (the lightmap-batch manifest: copy, shipped, out, name; per group of
//!        `--group` rows: bake copies with a fresh uid and no password pushed
//!        to the box, one `tmdrive run -- bake-run` for the group, the saved
//!        files pulled, each transplanted into its shipped file → out)
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::nadeo::{game_pids, plugin_pong, StagedPlugin, QUARANTINE_PLUGIN, BOX_TOOLS, GAME_EXE};
use crate::wsx::Wsx;

const MAPS: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps";
const MAPS_SHOOT: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot";
const GS_STORE: &str = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
const LM_CACHE: &str = "/mnt/c/ProgramData/Trackmania/Cache";
const STAGE: &str = "/home/vjeux/shoot/_stage";

fn get(shootctl: &str, route: &str, timeout_s: u64) -> String {
    let o = Command::new("timeout").arg(format!("{}", timeout_s + 5)).arg(shootctl).arg("get").arg(route).output();
    match o {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

fn ctx(shootctl: &str) -> Option<i64> {
    let body = get(shootctl, "/ctx", 5);
    let i = body.find("\"ctx\":")? + 6;
    let rest = &body[i..];
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn kill_pid(pid: u32) {
    let _ = Command::new("/mnt/c/Windows/System32/taskkill.exe").args(["/PID", &pid.to_string(), "/F"]).output();
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(40) {
        if !game_pids().map(|p| p.contains(&pid)).unwrap_or(true) {
            break;
        }
        std::thread::sleep(Duration::from_millis(1000));
    }
}

/// Our own game: launched by explorer (the plain Steam path), the plugin's
/// /ping awaited; closed by PID on drop.
struct MyGame {
    pid: u32,
    shootctl: String,
}

impl MyGame {
    fn launch(shootctl: &str, timeout: Duration) -> Result<MyGame, String> {
        let before = game_pids()?;
        if !before.is_empty() {
            return Err(format!("a Trackmania is running (pid {before:?}) that this run did not launch — not ours; nothing touched"));
        }
        Command::new("/mnt/c/Windows/explorer.exe").arg(GAME_EXE).output().map_err(|e| format!("explorer.exe: {e}"))?;
        let t0 = Instant::now();
        loop {
            if plugin_pong(shootctl) {
                break;
            }
            if t0.elapsed() > timeout {
                for pid in game_pids().unwrap_or_default() {
                    kill_pid(pid);
                }
                return Err(format!("the plugin never answered /ping within {} s after the launch (our game closed again)", timeout.as_secs()));
            }
            std::thread::sleep(Duration::from_millis(1500));
        }
        let pids = game_pids()?;
        if pids.len() != 1 {
            return Err(format!("{} Trackmania pids after the launch ({pids:?}) — not touching any", pids.len()));
        }
        Ok(MyGame { pid: pids[0], shootctl: shootctl.to_string() })
    }
    fn alive(&self) -> bool {
        game_pids().map(|p| p.contains(&self.pid)).unwrap_or(false)
    }
    /// /back until the menu (ctx 0); the save prompt on the way out answered.
    fn to_menu(&self) -> Result<(), String> {
        for _ in 0..12 {
            if ctx(&self.shootctl) == Some(0) {
                return Ok(());
            }
            if !self.alive() {
                return Err("the game is gone on the way to the menu".into());
            }
            let _ = get(&self.shootctl, "/back", 20);
            let _ = get(&self.shootctl, "/dismiss", 10);
            std::thread::sleep(Duration::from_millis(1500));
        }
        Err(format!("the game never reached the menu (ctx {:?})", ctx(&self.shootctl)))
    }
    fn ready(&self) -> Result<(), String> {
        let body = get(&self.shootctl, "/await?c=ready&ms=60000", 75);
        if body.contains("\"ok\":true") {
            Ok(())
        } else {
            Err(format!("the title never got ready: {}", body.chars().take(120).collect::<String>()))
        }
    }
}

impl Drop for MyGame {
    fn drop(&mut self) {
        kill_pid(self.pid);
        eprintln!("  game pid {} closed", self.pid);
    }
}

/// One map in the open game: load in the editor, compute, save, (the start check:
/// the test drive and the car's position), back to the menu.
/// Returns (saved WSL path, compute seconds, "x y z" of the car or "-").
fn bake_one(g: &MyGame, map_wsl: &str, quality: u32, load_timeout: Duration, compute_timeout: Duration, startcheck: bool, check_only: bool) -> Result<(String, f64, String), String> {
    let shootctl = &g.shootctl;
    let name = Path::new(map_wsl).file_name().and_then(|n| n.to_str()).ok_or("map path has no file name")?.to_string();
    let stem = name.strip_suffix(".Map.Gbx").unwrap_or(&name).to_string();
    // the game's own lightmap cache (content-keyed) would serve a cached one instead of computing
    let mut dropped = 0;
    if let Ok(rd) = std::fs::read_dir(LM_CACHE) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().ends_with("LightMap.zip") && std::fs::remove_file(e.path()).is_ok() {
                dropped += 1;
            }
        }
    }
    // the map where the game loads from
    std::fs::create_dir_all(MAPS_SHOOT).map_err(|e| format!("{MAPS_SHOOT}: {e}"))?;
    let game_copy = format!("{MAPS_SHOOT}/{name}");
    std::fs::copy(map_wsl, &game_copy).map_err(|e| format!("{map_wsl} -> {game_copy}: {e}"))?;
    let game_map = format!("C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/{name}");
    std::fs::write(format!("{GS_STORE}/editmap.txt"), &game_map).map_err(|e| format!("editmap.txt: {e}"))?;
    let t0 = Instant::now();
    println!("  {name}: cache {dropped} dropped; /editmap: {}", get(shootctl, "/editmap", 30));
    loop {
        if t0.elapsed() > load_timeout {
            return Err(format!("the map did not open in {} s; last ctx {}", load_timeout.as_secs(), get(shootctl, "/ctx", 10)));
        }
        if !g.alive() {
            return Err("the game process is gone — the map crashed the client while loading".into());
        }
        let c = get(shootctl, "/ctx", 10);
        if c.contains("\"ctx\":1") {
            break;
        }
        let secs = t0.elapsed().as_secs();
        if secs >= 20 && secs % 20 == 0 {
            let dlg = get(shootctl, "/dlgtext", 10).to_ascii_lowercase();
            if dlg.contains("load map") || dlg.contains("couldn") {
                return Err(format!("the editor refused the map: {}", dlg.trim()));
            }
        }
        if c.contains("FrameAskYesNo") {
            let _ = get(shootctl, "/yes", 10);
        }
        std::thread::sleep(Duration::from_millis(1500));
    }
    println!("  {name}: editor open after {:.1} s", t0.elapsed().as_secs_f64());
    std::thread::sleep(Duration::from_millis(4000));
    if check_only {
        // `--check-only`: no compute, no save — the start check alone on the pushed file
        let car = start_check(g, &name)?;
        let _ = std::fs::remove_file(&game_copy);
        g.to_menu()?;
        return Ok(("-".into(), 0.0, car));
    }
    let ts = Instant::now();
    println!("  {name}: /shadows q={quality}: {}", get(shootctl, &format!("/shadows?q={quality}"), 10));
    let mut saw_busy = false;
    loop {
        std::thread::sleep(Duration::from_millis(1000));
        if !g.alive() {
            return Err(format!("the game process is gone — the lightmapper crashed the client after {:.0} s", ts.elapsed().as_secs_f64()));
        }
        if ts.elapsed() > compute_timeout {
            return Err(format!("the lightmapper did not finish in {} s", compute_timeout.as_secs()));
        }
        let c = get(shootctl, "/ctx", 10);
        if c.contains("FrameAskYesNo") {
            let _ = get(shootctl, "/yes", 10);
        }
        let s = get(shootctl, "/shadowsq", 10);
        if s.contains("\"ready\":false") {
            saw_busy = true;
        } else if s.contains("\"ready\":true") && saw_busy {
            break;
        } else if !saw_busy && s.contains("\"ready\"") && ts.elapsed().as_secs() > 20 {
            return Err(format!("the lightmapper never ran (never saw the editor busy: {s})"));
        }
    }
    let compute_s = ts.elapsed().as_secs_f64();
    println!("  {name}: shadows done in {compute_s:.0} s");
    // the save, relative to the Maps folder
    let rel = format!("_lightmap/{stem}.Map.Gbx");
    let target = format!("{MAPS}/{rel}");
    let _ = std::fs::remove_file(&target);
    let _ = std::fs::create_dir_all(format!("{MAPS}/_lightmap"));
    std::fs::write(format!("{GS_STORE}/arg.txt"), &rel).map_err(|e| format!("arg.txt: {e}"))?;
    println!("  {name}: /mapsave {rel}: {}", get(shootctl, "/mapsave", 25));
    let tsave = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(1000));
        let c = get(shootctl, "/ctx", 10);
        if c.contains("FrameAskYesNo") {
            let _ = get(shootctl, "/yes", 10);
        }
        if let Ok(md) = std::fs::metadata(&target) {
            if md.len() > 0 {
                std::thread::sleep(Duration::from_millis(2000));
                if std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0) == md.len() {
                    break;
                }
            }
        }
        if !g.alive() {
            return Err("the game process is gone during SaveMap".into());
        }
        if tsave.elapsed() > Duration::from_secs(120) {
            return Err(format!("no file at {target} 120 s after SaveMap; ctx {}", c.trim()));
        }
    }
    let _ = std::fs::remove_file(&game_copy);
    println!("  {name}: saved {} ({} bytes)", target, std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0));
    // the START CHECK in the same session (`--startcheck`): the editor's test drive
    // (/edtest → the playground, ctx 3) and the car's resting position from /wheels —
    // the client-side half of `tinyctl startcheck`, without another push or load
    let car = if startcheck { start_check(g, &name)? } else { "-".to_string() };
    g.to_menu()?;
    Ok((target, compute_s, car))
}


/// The editor's test drive (/edtest → the playground, ctx 3) and the car's resting
/// position from /wheels, ~3.5 s in — the client-side half of `tinyctl startcheck`.
fn start_check(g: &MyGame, name: &str) -> Result<String, String> {
    let shootctl = &g.shootctl;
    let mut car = String::from("-");
    println!("  {name}: /edtest: {}", get(shootctl, "/edtest", 30));
    let p0 = Instant::now();
    loop {
        if p0.elapsed() > Duration::from_secs(150) {
            car = "NO VEHICLE (the playground never came up)".into();
            break;
        }
        if !g.alive() {
            return Err("the game process is gone — the test drive crashed the client".into());
        }
        let c = get(shootctl, "/ctx", 10);
        if c.contains("FrameAskYesNo") {
            let _ = get(shootctl, "/yes", 10);
        }
        if ctx(shootctl) == Some(3) {
            std::thread::sleep(Duration::from_millis(1000));
            if ctx(shootctl) == Some(3) {
                let probe = get(shootctl, "/wheels?ms=100", 15);
                if let Some(l) = probe.lines().find(|l| !l.starts_with('#') && !l.starts_with("wall_ms") && l.split('\t').count() > 4) {
                    let c: Vec<String> = l.split('\t').map(String::from).collect();
                    // the first car row ~1 s into the playground; read again once it has settled
                    std::thread::sleep(Duration::from_millis(2500));
                    let again = get(shootctl, "/wheels?ms=100", 15);
                    let c2: Vec<String> = again.lines().find(|l| !l.starts_with('#') && !l.starts_with("wall_ms") && l.split('\t').count() > 4).map(|l| l.split('\t').map(String::from).collect()).unwrap_or(c);
                    car = format!("{} {} {}", c2[2], c2[3], c2[4]);
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    println!("  {name}: car at [{car}] (playground after {:.1} s)", p0.elapsed().as_secs_f64());
    Ok(car)
}

pub fn bake_run(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps: Vec<String> = f("--maps").ok_or("bake-run needs --maps A,B,…")?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let quality: u32 = f("--quality").and_then(|q| q.parse().ok()).unwrap_or(4);
    let max_maps: usize = f("--max-maps").and_then(|q| q.parse().ok()).unwrap_or(4).max(1);
    let report = PathBuf::from(f("--report").ok_or("bake-run needs --report R.tsv")?);
    let done = f("--done").map(PathBuf::from);
    let shootctl = f("--shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    let plugin_src = PathBuf::from(f("--stage-plugin").unwrap_or_else(|| QUARANTINE_PLUGIN.into()));
    let load_timeout = Duration::from_secs(f("--load-timeout").and_then(|s| s.parse().ok()).unwrap_or(420));
    let compute_timeout = Duration::from_secs(f("--compute-timeout").and_then(|s| s.parse().ok()).unwrap_or(1800));
    let startcheck = tmmaps::cli::has(args, "--startcheck");
    let check_only = tmmaps::cli::has(args, "--check-only");
    if std::env::var("TM_LOCK_TOKEN").map(|t| t.is_empty()).unwrap_or(true) {
        return Err("no TM_LOCK_TOKEN in the environment — run this under `tmdrive run --purpose … -- tinyctl bake-run …`".into());
    }
    let result = (|| -> Result<String, String> {
        let _plugin = StagedPlugin::place(&plugin_src)?;
        let mut rows = String::from("map\tsaved\tverdict\tcompute_s\tcar\n");
        let mut game: Option<MyGame> = None;
        let mut in_game = 0usize;
        let mut ok = 0usize;
        for m in &maps {
            if game.as_ref().map(|g| !g.alive()).unwrap_or(false) {
                eprintln!("  the game died — relaunching");
                game = None;
            }
            if in_game >= max_maps {
                eprintln!("  {in_game} maps in this game — a fresh one (the client leaks across loads)");
                game = None;
            }
            if game.is_none() {
                let t0 = Instant::now();
                let g = MyGame::launch(&shootctl, Duration::from_secs(240))?;
                g.to_menu()?;
                g.ready()?;
                eprintln!("  game pid {} up and at the menu in {:.0} s", g.pid, t0.elapsed().as_secs_f64());
                game = Some(g);
                in_game = 0;
            }
            let g = game.as_ref().unwrap();
            let row = match bake_one(g, m, quality, load_timeout, compute_timeout, startcheck || check_only, check_only) {
                Ok((saved, secs, car)) => {
                    ok += 1;
                    format!("{m}\t{saved}\tok\t{secs:.0}\t{car}\n")
                }
                Err(e) => {
                    eprintln!("  {m}: FAILED: {e}");
                    // a failed map leaves the game in an unknown state: the next map gets a fresh one
                    game = None;
                    format!("{m}\t-\tFAILED {}\t-\t-\n", e.replace('\t', " ").lines().next().unwrap_or(""))
                }
            };
            in_game += 1;
            rows.push_str(&row);
            std::fs::write(&report, &rows).map_err(|e| format!("{}: {e}", report.display()))?;
        }
        drop(game);
        Ok(format!("{ok} of {} maps baked", maps.len()))
    })();
    let text = match &result {
        Ok(s) => format!("OK\t{s}\n"),
        Err(e) => format!("FAIL\t{e}\n"),
    };
    print!("{text}");
    if let Some(d) = done {
        let tmp = d.with_extension("tmp");
        std::fs::write(&tmp, &text).and_then(|_| std::fs::rename(&tmp, &d)).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    result.map(|_| ())
}

/// The devserver half: groups of the manifest through one `bake-run` each.
pub fn lightmap_run(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let manifest = PathBuf::from(f("--manifest").ok_or("lightmap-run needs --manifest M.tsv (copy, shipped, out, name)")?);
    let quality = f("--quality").unwrap_or_else(|| "4".into());
    let group: usize = f("--group").and_then(|v| v.parse().ok()).unwrap_or(4).max(1);
    let report = PathBuf::from(f("--report").unwrap_or_else(|| manifest.with_extension("run-report.tsv").display().to_string()));
    let box_tinyctl = f("--box-tinyctl").unwrap_or_else(|| "/home/vjeux/shoot/fall2026/tinyctl-fall".into());
    let box_tmdrive = f("--box-tmdrive").unwrap_or_else(|| format!("{BOX_TOOLS}/tmdrive"));
    let stage_plugin = f("--stage-plugin").unwrap_or_else(|| QUARANTINE_PLUGIN.into());
    let purpose = f("--purpose").unwrap_or_else(|| "editor lightmap bakes (tinyctl lightmap-run)".into());
    // --startcheck [--tolerance M]: the client's car position read in the same editor session
    // (the test drive) and compared with the shipped map's Spawn placement — `tinyctl startcheck`'s
    // rule (12 m default; a scale-k start block spawns at (16, 2, 16)·k from its item: 52/76/100 for ×2/×3/×4)
    let startcheck = tmmaps::cli::has(args, "--startcheck") || tmmaps::cli::has(args, "--check-only");
    let check_only = tmmaps::cli::has(args, "--check-only");
    let tolerance: f32 = f("--tolerance").and_then(|v| v.parse().ok()).unwrap_or(12.0);
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let rows: Vec<Vec<String>> = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with("copy\t")).map(|l| l.split('\t').map(String::from).collect()).collect();
    if !report.exists() {
        std::fs::write(&report, "copy\tout\tverdict\tcompute_s\twall_s\tout_bytes\tstart\n").map_err(|e| format!("{}: {e}", report.display()))?;
    }
    let wsx = Wsx::new(args);
    // --check-only: every row whose OUT (the lit file) exists is checked (the file pushed is the lit
    // file itself, nothing is baked or transplanted); otherwise the rows still without an OUT are baked
    let todo: Vec<&Vec<String>> = rows.iter().filter(|r| r.len() >= 4 && (Path::new(&r[2]).exists() == check_only)).collect();
    println!("{} of {} maps to bake, groups of {group}", todo.len(), rows.len());
    let mut failed = 0usize;
    for (gi, chunk) in todo.chunks(group).enumerate() {
        let t0 = Instant::now();
        let tag = format!("lr{}-{gi}", std::process::id());
        // the bake copies: a fresh uid (the game caches lightmaps by uid), no editor password
        let mut remote_maps: Vec<String> = Vec::new();
        let mut copies: Vec<(PathBuf, &Vec<String>)> = Vec::new();
        for (k, r) in chunk.iter().enumerate() {
            let (copy, _shipped, out, _name) = (&r[0], &r[1], &r[2], &r[3]);
            if let Some(p) = Path::new(out).parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
            }
            let bake_copy = Path::new(out).with_extension(if check_only { "checkcopy.Map.Gbx" } else { "bakecopy.Map.Gbx" });
            let mut m = tmmaps::map::MapFile::load(Path::new(if check_only { out } else { copy }));
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
            let old_uid = tmmaps::header::read(copy).map(|h| h.uid).unwrap_or_default();
            let fresh = crate::lightmap::fresh_uid_like(&old_uid, (nanos + k as u32 * 7919) % 100_000_000, (nanos / 7 + gi as u32) % 100_000_000);
            m.set_map_uid(&fresh);
            m.remove_password();
            m.write_to(&bake_copy).map_err(|e| format!("{}: {e}", bake_copy.display()))?;
            let stem: String = Path::new(out).file_name().unwrap_or_default().to_string_lossy().trim_end_matches(".Map.Gbx").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
            let remote = format!("{STAGE}/{tag}-{stem}.Map.Gbx");
            println!("[group {gi}] pushing {} → {remote}", bake_copy.display());
            wsx.push(&bake_copy, &remote)?;
            remote_maps.push(remote);
            copies.push((bake_copy, r));
        }
        let r_report = format!("{STAGE}/{tag}.report.tsv");
        let r_done = format!("{STAGE}/{tag}.done");
        let r_log = format!("{STAGE}/{tag}.log");
        let cmd = format!(
            "rm -f '{r_done}'; nohup setsid {box_tmdrive} run --purpose '{purpose}' -- {box_tinyctl} bake-run --maps '{}' --quality {quality} --max-maps {group} --stage-plugin '{stage_plugin}' --report '{r_report}' --done '{r_done}'{}{} > '{r_log}' 2>&1 < /dev/null &",
            remote_maps.join(","),
            if startcheck { " --startcheck" } else { "" },
            if check_only { " --check-only" } else { "" }
        );
        println!("[group {gi}] one hold for {} maps …", remote_maps.len());
        // the pushed binary's exec bit does not survive `wsx push` (2026-10-01: "Permission denied" from tmdrive run)
        wsx.sh(&format!("chmod +x '{box_tinyctl}'"))?;
        wsx.sh(&cmd)?;
        let done = wsx.wait_done(&r_done, &r_log, Duration::from_secs(3600 * 2), &format!("bake-run group {gi}"))?;
        println!("[group {gi}] {}", done.trim());
        let rep = wsx.cat(&r_report).unwrap_or_default();
        for (bake_copy, r) in &copies {
            let (copy, shipped, out, _name) = (&r[0], &r[1], &r[2], &r[3]);
            let stem: String = Path::new(out).file_name().unwrap_or_default().to_string_lossy().trim_end_matches(".Map.Gbx").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
            let remote = format!("{STAGE}/{tag}-{stem}.Map.Gbx");
            let row = rep.lines().find(|l| l.starts_with(&format!("{remote}\t")));
            let (saved, verdict, secs, car) = match row {
                Some(l) => {
                    let c: Vec<&str> = l.split('\t').collect();
                    (c.get(1).unwrap_or(&"-").to_string(), c.get(2).unwrap_or(&"-").to_string(), c.get(3).unwrap_or(&"-").to_string(), c.get(4).unwrap_or(&"-").to_string())
                }
                None => ("-".into(), "FAILED no report row".into(), "-".into(), "-".into()),
            };
            // the start check against the SHIPPED map's Spawn placement
            let start = if !startcheck {
                "-".to_string()
            } else {
                let nums: Vec<f32> = car.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                let sm = tmmaps::map::MapFile::load(Path::new(shipped));
                match (nums.len() == 3, sm.items.iter().find(|it| it.waypoint_tag.as_deref() == Some("Spawn"))) {
                    (true, Some(sp)) => {
                        let d = ((nums[0] - sp.pos[0]).powi(2) + (nums[1] - sp.pos[1]).powi(2) + (nums[2] - sp.pos[2]).powi(2)).sqrt();
                        if d <= tolerance { format!("PASS {d:.1} m") } else { failed += 1; format!("FAIL {d:.1} m from the Spawn (tolerance {tolerance})") }
                    }
                    (true, None) => { failed += 1; "FAIL no Spawn placement".to_string() }
                    (false, _) => { failed += 1; format!("FAIL no car ({car})") }
                }
            };
            let mut out_bytes = 0u64;
            let verdict = if check_only {
                if verdict == "ok" { out_bytes = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0); "checked".to_string() } else { failed += 1; verdict }
            } else if verdict == "ok" && saved != "-" {
                let resaved = Path::new(out).with_extension("resaved.Map.Gbx");
                match wsx.pull(&saved, &resaved).and_then(|_| crate::lightmap::finish_from_resaved(bake_copy, &resaved, Path::new(copy), Path::new(shipped), Path::new(out))) {
                    Ok(()) => {
                        out_bytes = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
                        let _ = wsx.sh(&format!("rm -f '{saved}'"));
                        "ok".to_string()
                    }
                    Err(e) => {
                        failed += 1;
                        format!("FAILED transplant: {}", e.lines().next().unwrap_or(""))
                    }
                }
            } else {
                failed += 1;
                verdict
            };
            let _ = wsx.sh(&format!("rm -f '{remote}'"));
            println!("[group {gi}] {copy}: {verdict} ({secs} s compute; start {start})");
            let line = format!("{copy}\t{out}\t{verdict}\t{secs}\t{:.0}\t{out_bytes}\t{start}\n", t0.elapsed().as_secs_f64());
            let mut fh = std::fs::OpenOptions::new().append(true).open(&report).map_err(|e| format!("{}: {e}", report.display()))?;
            std::io::Write::write_all(&mut fh, line.as_bytes()).map_err(|e| e.to_string())?;
        }
    }
    if failed > 0 {
        return Err(format!("{failed} map(s) failed — see {}", report.display()));
    }
    Ok(())
}
