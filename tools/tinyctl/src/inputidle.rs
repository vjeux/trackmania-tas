//! `tinyctl box-idle` — is a PERSON using the render box right now?
//!
//! ```text
//! tinyctl box-idle [--window-s 60] [--quiet] [--wait-s N [--poll-s 20]]
//!     exit 0 the box is free, 1 someone is using it, 2 the check itself failed
//! ```
//!
//! vjeux 2026-10-04: "implement a check using the machine: needs to have keyboard /
//! mouse / controller in the past minute. Nobody has been using the machine most of the
//! day." The process list was the old gate and it lied twice in one day (a "0 Trackmania"
//! box with Hades II, then Minecraft, in the foreground — my screenshots were his screen),
//! and waiting for his word left the machine idle for hours. This reads the OS's own
//! last-input time instead: `inputidle.exe` on the box (tools/inputidle, cross-linked for
//! Windows) reports keyboard/mouse idle seconds from `GetLastInputInfo` and controller
//! activity from its XInput sampler.
//!
//! `--wait-s N` polls until the box goes free (that is the loop a launcher wants).

use crate::wsx::Wsx;

pub const EXE: &str = "/mnt/c/Users/vjeux/inputidle.exe";
pub const STATE: &str = "C:/Users/vjeux/inputidle-pad.txt";

pub struct Verdict {
    pub free: bool,
    pub line: String,
}

/// One reading. `Err` means the check could not run (missing exe, bridge down) — the
/// caller must treat that as BUSY: an unknown box is never ours to take.
pub fn read(wsx: &Wsx, window_s: u32) -> Result<Verdict, String> {
    // one bridge call: the input verdict, the process list, the game lock
    let out = wsx.sh(&format!(
        "{EXE} status --window-s {window_s} --state {STATE} 2>&1; echo __TASKS__; /mnt/c/Windows/System32/tasklist.exe 2>/dev/null | tr -d '\\r'; echo __LOCK__; /home/vjeux/bin/tmdrive status 2>&1 | head -1; true"
    ))?;
    let line = out.lines().find(|l| l.contains("verdict=")).unwrap_or("").trim().to_string();
    if line.is_empty() {
        return Err(format!("inputidle gave no verdict line: {}", out.trim().chars().take(200).collect::<String>()));
    }
    if line.contains("verdict=error") {
        return Err(line);
    }
    let mut free = line.contains("verdict=free");
    let mut line = line;
    // The input clock is not the whole story (2026-10-19, 11:38: `verdict=free … clock=
    // app-pumped` while vjeux had Minecraft AND his own Trackmania open for hours — a
    // player holding a key, or a captured-cursor game, is indistinguishable from an app
    // pumping the clock; the gate waved a 1,574-map load loop onto his screen). So two
    // process facts are BUSY on their own, whatever the clock says:
    //   • a game only a person launches (`HUMAN_APPS`) is running;
    //   • a Trackmania.exe is running while the game lock (`tmdrive status`) names no
    //     live driver — nobody drives it, so a person does (the 2026-10-18 rule: a game
    //     no agent holds is vjeux's).
    // `--ignore-idle` / `TINY_IGNORE_IDLE=1` still overrides, deliberately, upstream.
    let tasks = out.split("__TASKS__").nth(1).and_then(|t| t.split("__LOCK__").next()).unwrap_or("");
    let lock = out.split("__LOCK__").nth(1).unwrap_or("").trim();
    let mut procs: Vec<String> = Vec::new();
    for l in tasks.lines() {
        let Some(exe) = l.split_whitespace().next() else { continue };
        if HUMAN_APPS.iter().any(|h| exe.eq_ignore_ascii_case(h)) && !procs.iter().any(|p| p == exe) {
            procs.push(exe.to_string());
        }
    }
    let tm_running = tasks.lines().any(|l| l.split_whitespace().next().map(|e| e.eq_ignore_ascii_case("Trackmania.exe")).unwrap_or(false));
    let lock_alive = lock.starts_with("ALIVE") || lock.starts_with("HELD") || (lock.contains("session=") && !lock.starts_with("DEAD") && !lock.starts_with("FREE"));
    if !procs.is_empty() {
        free = false;
        line.push_str(&format!(" human_app={}", procs.join("+")));
    }
    if tm_running && !lock_alive {
        free = false;
        line.push_str(" undriven_game=Trackmania.exe");
    }
    if !free && line.contains("verdict=free") {
        line = line.replacen("verdict=free", "verdict=busy(process)", 1);
    }
    Ok(Verdict { free, line })
}

/// Programs only a PERSON starts on the box: any of them running = busy.
pub const HUMAN_APPS: &[&str] = &["Minecraft.exe", "MinecraftLauncher.exe", "Hades2.exe", "HadesII.exe", "Hades.exe", "javaw.exe"];

/// Poll until the box is free or the deadline passes.
pub fn wait_free(wsx: &Wsx, window_s: u32, wait_s: u32, poll_s: u32, quiet: bool) -> Result<Verdict, String> {
    let t0 = std::time::Instant::now();
    loop {
        match read(wsx, window_s) {
            Ok(v) if v.free => return Ok(v),
            Ok(v) => {
                if t0.elapsed().as_secs() >= wait_s as u64 {
                    return Ok(v);
                }
                if !quiet {
                    eprintln!("[{:>4.0}s] box busy: {}", t0.elapsed().as_secs_f64(), v.line);
                }
            }
            Err(e) => return Err(e),
        }
        std::thread::sleep(std::time::Duration::from_secs(poll_s.max(1) as u64));
    }
}

/// The free-box gate (vjeux 2026-10-04): a run that launches or drives the game refuses
/// to start while a PERSON is using the box — keyboard, mouse or controller within the
/// last minute (`tinyctl box-idle`, the Windows `inputidle.exe`). `TINY_IGNORE_IDLE=1`
/// (or `--ignore-idle`) is the deliberate override; a check that cannot run is BUSY.
pub fn gate(wsx: &crate::wsx::Wsx, args: &[String], what: &str) -> Result<(), String> {
    if tmmaps::cli::has(args, "--ignore-idle") || std::env::var("TINY_IGNORE_IDLE").map(|v| v == "1").unwrap_or(false) {
        return Ok(());
    }
    let window: u32 = tmmaps::cli::flag(args, "--idle-window-s").and_then(|v| v.parse().ok()).unwrap_or(60);
    let wait: u32 = tmmaps::cli::flag(args, "--idle-wait-s").and_then(|v| v.parse().ok()).unwrap_or(0);
    let v = if wait > 0 { wait_free(wsx, window, wait, 20, false)? } else { read(wsx, window)? };
    if v.free {
        return Ok(());
    }
    Err(format!("{what}: someone is using the box — {} (wait, or --ignore-idle / TINY_IGNORE_IDLE=1)", v.line))
}

pub fn cmd(args: &[String]) -> i32 {
    let wsx = Wsx::new(args);
    let f = |k: &str| tmmaps::cli::flag(args, k).and_then(|v| v.parse::<u32>().ok());
    let window = f("--window-s").unwrap_or(60);
    let quiet = tmmaps::cli::has(args, "--quiet");
    let r = match f("--wait-s") {
        Some(w) => wait_free(&wsx, window, w, f("--poll-s").unwrap_or(20), quiet),
        None => read(&wsx, window),
    };
    match r {
        Ok(v) => {
            if !quiet {
                println!("{}", v.line);
            }
            i32::from(!v.free)
        }
        Err(e) => {
            println!("box-idle: {e}");
            2
        }
    }
}
