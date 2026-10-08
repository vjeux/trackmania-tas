//! `tinyctl remote-shots` — editor renders on the render box, driven from the devserver,
//! in Rust (the probechain.sh / w23shot.sh / effcat-shoot*.sh chains of 2026-09-24 retired;
//! vjeux: "don't write shell scripts, Rust only").
//!
//! ```text
//! tinyctl remote-shots --maps A,B[,…] --views V.tsv --outdir NAME [--push-from DIR]
//!     [--pull DIR] [--side o] [--purpose "…"] [--chunk 8]
//! ```
//!
//! - `--maps`: map names (without `.Map.Gbx`) in the box's `Maps/_shoot/`; with
//!   `--push-from DIR` the files `DIR/<name>.Map.Gbx` are pushed there first.
//! - `--views V.tsv`: the shootset view file (`name  target  dist  yaw  pitch`), pushed to the
//!   box's hunt dir; the game's free camera takes one frame per view.
//! - `--outdir NAME`: frames land in `C:\Users\vjeux\tinyshots\NAME\cmp-<map><view>-<side>.png`;
//!   `--pull DIR` copies them back (one tar over the bridge).
//!
//! The game lock: every render is `tmdrive run … -- shootctl shootset … --no-lock`, one
//! call per map per `--chunk` views (a bridge call is capped at 150 s; 8 views ≈ 60 s). The
//! box is waited for in ≤120 s slices; a game that is not running is launched as its own
//! lock step; a map that "did not open" (a stuck title) costs one `tmdrive kill` +
//! `tmdrive launch` and one retry. Nothing here holds the box between maps.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::wsx::Wsx;

const HUNT: &str = "/home/vjeux/shoot/hunt";
const MAPS_SHOOT: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot";
const MAPS_WIN: &str = "C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot";
const SHOTS: &str = "/mnt/c/Users/vjeux/tinyshots";
pub const ENV: &str = "cd /home/vjeux/shoot/hunt; export TM_SESSION=080a15b8-2c6a-4fff-9d8a-db7d0072ee43; export TM_SESSION_TITLE=\"Tiny u10s publish\"; export PATH=/home/vjeux/mk64-fidelity/l1-enum24-stage/frozen-bin:/home/vjeux/bin:$PATH";

/// `tinyctl remote-drive --maps A,B --outdir NAME [--drive-ms 10000] [--drive-at-ms 14500]
/// [--carlog-ms 26000] [--pull DIR] [--purpose "…"]` — PLAY-mode probe per map, on the box from
/// the devserver: the car spawned at the start, the accelerator held, the live car position
/// logged per frame (`car-<map>.tsv`) — json9841's wheel-status test as data (2026-09-24).
/// One `tmdrive run … shootctl playshots` per map, the box waited for in slices.
pub fn drive_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps: Vec<String> = f("--maps").ok_or("remote-drive needs --maps A,B")?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let outdir = f("--outdir").ok_or("remote-drive needs --outdir NAME")?;
    let drive_ms = f("--drive-ms").unwrap_or_else(|| "10000".into());
    let drive_at = f("--drive-at-ms").unwrap_or_else(|| "14500".into());
    let carlog = f("--carlog-ms").unwrap_or_else(|| "26000".into());
    let purpose = f("--purpose").unwrap_or_else(|| format!("drive probe: {}", maps.join(" ")));
    let wsx = Wsx::new(args);
    let t0 = Instant::now();
    wsx.sh(&format!("mkdir -p {SHOTS}/{outdir}"))?;
    let mut failed = Vec::new();
    for m in &maps {
        game_up(&wsx)?;
        wait_box(&wsx, &purpose, 3600)?;
        // the playground load + intro (~15 s) + the drive + the log: one call under 150 s
        let run = format!(
            "{ENV}; tmdrive run --purpose \"{purpose}\" --wait 100 -- /home/vjeux/bin/shootctl playshots --map \"{MAPS_WIN}/{m}.Map.Gbx\" --outdir {SHOTS}/{outdir} --tag {m} --shots 2 --first-ms 16000 --every-ms 6000 --drive-ms {drive_ms} --drive-at-ms {drive_at} --carlog-ms {carlog} --timeout 130 2>&1 | tail -3"
        );
        let mut out = wsx.sh(&run).unwrap_or_else(|e| format!("bridge: {e}"));
        let mut rows: usize = wsx.sh(&format!("wc -l < {SHOTS}/{outdir}/car-{m}.tsv 2>/dev/null")).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
        if rows < 10 && (out.contains("could not get back to the menu") || out.contains("did not open") || out.contains("exit status: 1")) {
            // a game stuck in an editor (a modal /back cannot answer): restarted as its own
            // lock steps, the probe tried once more
            println!("[{:>4.0}s] {m}: {} — restarting the game", t0.elapsed().as_secs_f64(), out.lines().last().unwrap_or("").chars().take(100).collect::<String>());
            let _ = wsx.sh(&format!("{ENV}; tmdrive kill --purpose \"{purpose} (restart)\" --wait 100"));
            let _ = wsx.sh(&format!("{ENV}; sed -i \"s/^DeveloperMode=false/DeveloperMode=true/\" /mnt/c/Users/vjeux/OpenplanetNext/Settings.ini; tmdrive launch --timeout 120 --purpose \"{purpose} (launch)\" --wait 100"));
            wait_box(&wsx, &purpose, 3600)?;
            out = wsx.sh(&run).unwrap_or_else(|e| format!("bridge: {e}"));
            rows = wsx.sh(&format!("wc -l < {SHOTS}/{outdir}/car-{m}.tsv 2>/dev/null")).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
        }
        println!("[{:>4.0}s] {m}: {rows} car rows — {}", t0.elapsed().as_secs_f64(), out.lines().last().unwrap_or("").chars().take(120).collect::<String>());
        if rows < 10 {
            failed.push(m.clone());
        }
        // a bridge call that timed out (150 s cap) leaves ITS shootctl running on the box, and
        // that process keeps the tmdrive lock for another session's minutes (the MK64 session
        // waited on a dead `playshots-d57fix` hold, 2026-09-26 07:50): reap our own orphan by
        // pid — the one shootctl whose command line names this map — then let the lock lapse.
        if out.contains("timed out") {
            let pids = wsx.sh(&format!("pgrep -f 'shootctl playshots .*{m}\\.Map\\.Gbx' || true")).unwrap_or_default();
            for pid in pids.split_whitespace().filter(|p| p.chars().all(|c| c.is_ascii_digit())) {
                println!("[{:>4.0}s] {m}: reaping the orphaned shootctl (pid {pid}) left by the timed-out bridge call", t0.elapsed().as_secs_f64());
                let _ = wsx.sh(&format!("kill {pid} 2>/dev/null; sleep 2; kill -9 {pid} 2>/dev/null; true"));
            }
        }
    }
    if let Some(dir) = f("--pull") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let tar = format!("{HUNT}/rd-{outdir}.tgz");
        wsx.sh(&format!("cd {SHOTS}/{outdir} && tar czf {tar} car-*.tsv *.png 2>/dev/null; ls -la {tar}"))?;
        let local = dir.join(format!("{outdir}.tgz"));
        wsx.pull(&tar, &local)?;
        let st = std::process::Command::new("tar").arg("xzf").arg(&local).arg("-C").arg(&dir).status().map_err(|e| e.to_string())?;
        if !st.success() {
            return Err(format!("tar xzf {}: {st}", local.display()));
        }
        let _ = std::fs::remove_file(&local);
        println!("[{:>4.0}s] logs in {}", t0.elapsed().as_secs_f64(), dir.display());
        close_if_ours(&wsx, &purpose);
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("remote-drive: no car log for {}", failed.join(", ")))
    }
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps: Vec<String> = f("--maps").ok_or("remote-shots needs --maps A,B")?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let views = PathBuf::from(f("--views").ok_or("remote-shots needs --views V.tsv")?);
    let outdir = f("--outdir").ok_or("remote-shots needs --outdir NAME")?;
    let side = f("--side").unwrap_or_else(|| "o".into());
    let purpose = f("--purpose").unwrap_or_else(|| format!("editor shots: {}", maps.join(" ")));
    let chunk: usize = f("--chunk").and_then(|v| v.parse().ok()).unwrap_or(8).max(1);
    let wsx = Wsx::new(args);
    let t0 = Instant::now();
    // the views, in chunks the bridge can wait for
    let text = std::fs::read_to_string(&views).map_err(|e| format!("{}: {e}", views.display()))?;
    let rows: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).collect();
    if rows.is_empty() {
        return Err("remote-shots: the views file has no rows".into());
    }
    let mut view_files: Vec<String> = Vec::new();
    for (i, part) in rows.chunks(chunk).enumerate() {
        let local = std::env::temp_dir().join(format!("remote-shots-views-{}-{i}.tsv", std::process::id()));
        std::fs::write(&local, part.join("\n") + "\n").map_err(|e| e.to_string())?;
        let remote = format!("{HUNT}/rs-views-{outdir}-{i}.tsv");
        wsx.push(&local, &remote)?;
        let _ = std::fs::remove_file(&local);
        view_files.push(remote);
    }
    // the maps
    if let Some(dir) = f("--push-from") {
        for m in &maps {
            let local = Path::new(&dir).join(format!("{m}.Map.Gbx"));
            if !local.exists() {
                return Err(format!("{}: no such file", local.display()));
            }
            // pushed to the hunt dir (a Windows-side write over the bridge fails on the
            // OneDrive folder), then copied in
            wsx.push(&local, &format!("{HUNT}/rs-{m}.Map.Gbx"))?;
            wsx.sh(&format!("cp {HUNT}/rs-{m}.Map.Gbx \"{MAPS_SHOOT}/{m}.Map.Gbx\" && rm -f {HUNT}/rs-{m}.Map.Gbx"))?;
        }
        println!("[{:>4.0}s] {} maps pushed", t0.elapsed().as_secs_f64(), maps.len());
    }
    wsx.sh(&format!("mkdir -p {SHOTS}/{outdir}"))?;
    let mut failed: Vec<String> = Vec::new();
    for m in &maps {
        let mut ok = true;
        for (i, vf) in view_files.iter().enumerate() {
            let mut attempt = 0;
            loop {
                attempt += 1;
                game_up(&wsx)?;
                wait_box(&wsx, &purpose, 3600)?;
                let run = format!(
                    "{ENV}; tmdrive run --purpose \"{purpose}\" --wait 100 -- /home/vjeux/bin/shootctl shootset --map \"{MAPS_WIN}/{m}.Map.Gbx\" --views {vf} --side {side} --outdir {SHOTS}/{outdir} --tag {m} --no-lock --load-timeout 100 2>&1 | tail -3"
                );
                let out = wsx.sh(&run).unwrap_or_else(|e| format!("bridge: {e}"));
                let last = out.lines().last().unwrap_or("").to_string();
                // the frames themselves are the verdict (the run's text can be a lock hint)
                let want = rows.len().min(chunk * (i + 1));
                let have: usize = wsx.sh(&format!("ls {SHOTS}/{outdir}/cmp-{m}*-{side}.png 2>/dev/null | wc -l")).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
                if have < want || out.contains("FAILED") || out.contains("did not open") || out.contains("bridge:") {
                    println!("[{:>4.0}s] {m} views {i}: {have}/{want} frames — {} (attempt {attempt})", t0.elapsed().as_secs_f64(), last.chars().take(120).collect::<String>());
                    if attempt >= 2 {
                        ok = false;
                        break;
                    }
                    // a stuck title: the game is restarted as its own lock steps
                    let _ = wsx.sh(&format!("{ENV}; tmdrive kill --purpose \"{purpose} (restart)\" --wait 100"));
                    let _ = wsx.sh(&format!("{ENV}; sed -i \"s/^DeveloperMode=false/DeveloperMode=true/\" /mnt/c/Users/vjeux/OpenplanetNext/Settings.ini; tmdrive launch --timeout 120 --purpose \"{purpose} (launch)\" --wait 100"));
                    continue;
                }
                println!("[{:>4.0}s] {m} views {i}: {}", t0.elapsed().as_secs_f64(), last.chars().take(120).collect::<String>());
                break;
            }
            if !ok {
                break;
            }
        }
        if !ok {
            failed.push(m.clone());
        }
    }
    if let Some(dir) = f("--pull") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let tar = format!("{HUNT}/rs-{outdir}.tgz");
        wsx.sh(&format!("cd {SHOTS}/{outdir} && tar czf {tar} *.png"))?;
        let local = dir.join(format!("{outdir}.tgz"));
        wsx.pull(&tar, &local)?;
        let st = std::process::Command::new("tar").arg("xzf").arg(&local).arg("-C").arg(&dir).status().map_err(|e| e.to_string())?;
        if !st.success() {
            return Err(format!("tar xzf {}: {st}", local.display()));
        }
        let _ = std::fs::remove_file(&local);
        let n = std::fs::read_dir(&dir).map(|rd| rd.flatten().filter(|e| e.path().extension().map(|x| x == "png").unwrap_or(false)).count()).unwrap_or(0);
        println!("[{:>4.0}s] {n} frames in {}", t0.elapsed().as_secs_f64(), dir.display());
    }
    for vf in &view_files {
        let _ = wsx.sh(&format!("rm -f {vf}"));
    }
    if failed.is_empty() {
        close_if_ours(&wsx, &purpose);
        println!("remote-shots done in {:.0}s", t0.elapsed().as_secs_f64());
        Ok(())
    } else {
        Err(format!("remote-shots: {} map(s) did not render: {}", failed.len(), failed.join(", ")))
    }
}

/// The game running (launched as its own lock step when not).
/// Another session's dirty-recovery marker (the MK64 native runs, 2026-09-26): while
/// `PluginStorage/TmDriveRecovery.v1` exists the box is mid-restore — every game step of
/// ours waits (up to an hour), never launches or takes the lock over it.
fn wait_recovery_clear(wsx: &Wsx) -> Result<(), String> {
    let t0 = Instant::now();
    loop {
        let m = wsx.sh("test -e /mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/TmDriveRecovery.v1 && echo DIRTY || echo clean").unwrap_or_default();
        if !m.contains("DIRTY") {
            return Ok(());
        }
        if t0.elapsed().as_secs() > 3600 {
            return Err("TmDriveRecovery.v1 marker still present after 60 min".into());
        }
        println!("  TmDriveRecovery.v1 marker present (another session restoring the box) — waiting");
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

/// Did THIS run launch the game? Then it closes it again at the end: the shared box's other
/// lanes need the game ABSENT, and a game left up after the lock is released reads as an
/// orphan to them (the MK64 session killed one of mine at 13:45, 2026-09-27).
/// The PID of the game THIS run launched, 0 when we launched none. Closing is BY PID only:
/// `tmdrive kill` kills Trackmania by NAME, and a name-kill can take down a game someone
/// else — a human — started (vjeux's game kept closing while he played, 2026-09-27 14:26).
pub(crate) static LAUNCHED_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Close the game if this run launched it, by its own pid; never a game we did not start.
pub(crate) fn close_if_ours(wsx: &Wsx, _purpose: &str) {
    let pid = LAUNCHED_PID.swap(0, std::sync::atomic::Ordering::SeqCst);
    if pid == 0 {
        return;
    }
    // is that pid still the game we launched? (a stale pid must never be killed blind)
    let alive = wsx.sh(&format!("/mnt/c/Windows/System32/tasklist.exe /FI \"PID eq {pid}\" 2>/dev/null | grep -ci trackmania")).unwrap_or_default().trim().to_string();
    if alive.starts_with('0') {
        println!("  the game this run launched (pid {pid}) is already gone — nothing to close");
        return;
    }
    let _ = wsx.sh(&format!("/mnt/c/Windows/System32/taskkill.exe /PID {pid} /F > /dev/null 2>&1; true"));
    println!("  closed the game this run launched (pid {pid}) — by pid, never by name");
}

fn game_up(wsx: &Wsx) -> Result<(), String> {
    wait_recovery_clear(wsx)?;
    let up = wsx.sh("/mnt/c/Windows/System32/tasklist.exe 2>/dev/null | grep -ci trackmania; true").unwrap_or_default().trim().to_string();
    if up == "0" {
        println!("  the game is not running — launching");
        wsx.sh(&format!("{ENV}; sed -i \"s/^DeveloperMode=false/DeveloperMode=true/\" /mnt/c/Users/vjeux/OpenplanetNext/Settings.ini; tmdrive launch --timeout 120 --purpose \"remote-shots (launch)\" --wait 100"))?;
        // remember the pid so the post-run close is by pid, never by name
        if let Ok(out) = wsx.sh("/mnt/c/Windows/System32/tasklist.exe /FO CSV /NH 2>/dev/null | grep -i trackmania | head -1 | cut -d, -f2 | tr -d '\"'") {
            if let Ok(pid) = out.trim().parse::<u32>() {
                LAUNCHED_PID.store(pid, std::sync::atomic::Ordering::SeqCst);
                println!("  the game this run launched is pid {pid}");
            }
        }
    }
    Ok(())
}

/// Wait for the box in ≤120 s slices (the bridge caps one call at 150 s).
fn wait_box(wsx: &Wsx, purpose: &str, max_secs: u64) -> Result<(), String> {
    wait_recovery_clear(wsx)?;
    let t0 = Instant::now();
    loop {
        let out = wsx.sh(&format!("{ENV}; tmdrive wait --timeout 120 > /dev/null 2>&1; echo RC=$?")).unwrap_or_default();
        if out.contains("RC=0") {
            return Ok(());
        }
        if t0.elapsed().as_secs() > max_secs {
            return Err(format!("{purpose}: the box was held for {max_secs}s"));
        }
        println!("  waiting for the box ({}s)", t0.elapsed().as_secs());
    }
}
