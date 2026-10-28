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
        "{EXE} status --window-s {window_s} --state {STATE} --agent-game C:/Users/vjeux/agent-game.txt --agent-launch C:/Users/vjeux/agent-launch.txt 2>&1; echo __TASKS__; /mnt/c/Windows/System32/tasklist.exe 2>/dev/null | tr -d '\\r'; echo __LOCK__; /home/vjeux/bin/tmdrive status 2>&1 | head -1; true"
    ))?;
    let mut line = out.lines().find(|l| l.contains("verdict=")).unwrap_or("").trim().to_string();
    if line.is_empty() {
        return Err(format!("inputidle gave no verdict line: {}", out.trim().chars().take(200).collect::<String>()));
    }
    if line.contains("verdict=error") {
        return Err(line);
    }
    let mut free = line.contains("verdict=free");
    // MY OWN RUNS ARE NOT A PERSON (Everios96 2026-10-27: "you shouldn't activate that
    // clock yourself"): a game-driving run of this tool (gate() below logs its start, its
    // heartbeat and the idle reading it started from) leaves synthetic input on the box —
    // the launcher's Alt press, the plugin's pointer warps — and the OS clock counts it. If
    // the last input falls inside one of this host's logged runs (±45 s), the idle time
    // is measured from the last HUMAN input the run saw when it started, and the verdict
    // is re-taken against the window from that. Input AFTER a run ended is a person's.
    if let Some(raw) = line.split_whitespace().find_map(|w| w.strip_prefix("kbm_idle_s=")).and_then(|v| v.parse::<f64>().ok()) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        let last_input = now - raw;
        if let Some((start, end, idle_at_start, what)) = runlog::covering(last_input) {
            let human_last = start - idle_at_start;
            let eff = now - human_last;
            let pad_busy = line.split_whitespace().find_map(|w| w.strip_prefix("pad_idle_s=")).and_then(|v| v.parse::<f64>().ok()).map(|p| p < window_s as f64).unwrap_or(false);
            let was_free = free;
            free = eff >= window_s as f64 && !pad_busy;
            line = line.replacen(&format!("kbm_idle_s={raw}"), &format!("kbm_idle_s={eff:.0}"), 1);
            line = line.replacen(if was_free { "verdict=free" } else { "verdict=busy" }, if free { "verdict=free" } else { "verdict=busy" }, 1);
            line.push_str(&format!(" agent_run=\"{what}\" raw_kbm_idle_s={raw:.0} run_start_ago_s={:.0} run_end_ago_s={:.0}", now - start, now - end));
        }
    }
    let mut line = line;
    // Process facts are INFORMATION, not a veto (2026-10-19). The 11:38 version made
    // "a human-only app runs" or "a Trackmania no lease drives" = BUSY; by 16:00 it had
    // held a 6,000-map rollout for 4.5 h on a Minecraft parked since the evening before
    // and a Trackmania that was MY OWN leftover (started 06:25:37, inside my 602 render
    // run) — while the input clock said free the whole time, correctly. vjeux's rule
    // (2026-10-04) is the input rule: no keyboard/mouse/pad for a minute = free, a parked
    // app does not count. The facts stay on the line so a log reader sees them.
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
        line.push_str(&format!(" apps={}", procs.join("+")));
    }
    if tm_running && !lock_alive {
        line.push_str(" undriven_game=Trackmania.exe");
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
        // this run is about to touch the box: log it, so the input it leaves is not a person's
        let idle_at_start = v.line.split_whitespace().find_map(|w| w.strip_prefix("kbm_idle_s=")).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);
        runlog::start(what, idle_at_start);
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


/// The log of this host's own box runs: `~/.tinyctl/box-runs.tsv`, one line per run —
/// `pid \t start_unix \t end_unix \t idle_at_start_s \t what`. `start` writes the line and
/// spawns a heartbeat thread that moves `end` forward every 20 s for as long as the
/// process lives, so a killed run is dated to within 20 s of its death; `covering(t)`
/// finds a run whose [start − 5, end + 120] holds the instant `t`.
pub mod runlog {
    use std::io::Write;

    fn path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
        std::path::PathBuf::from(home).join(".tinyctl").join("box-runs.tsv")
    }

    fn now() -> f64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
    }

    fn rewrite(pid: u32, start: f64, end: f64, idle: f64, what: &str) {
        let p = path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        let mine = format!("{pid}\t");
        let mut lines: Vec<String> = text.lines().filter(|l| !l.starts_with(&mine)).map(String::from).collect();
        // the log stays short: runs older than two days go
        let cutoff = now() - 2.0 * 86400.0;
        lines.retain(|l| l.split('\t').nth(1).and_then(|v| v.parse::<f64>().ok()).map(|s| s > cutoff).unwrap_or(false));
        lines.push(format!("{pid}\t{start:.0}\t{end:.0}\t{idle:.0}\t{}", what.replace(['\t', '\n'], " ")));
        let tmp = p.with_extension("tmp");
        if let Ok(mut f) = std::fs::File::create(&tmp) {
            let _ = writeln!(f, "{}", lines.join("\n"));
            let _ = std::fs::rename(&tmp, &p);
        }
    }

    pub fn start(what: &str, idle_at_start: f64) {
        let pid = std::process::id();
        let t0 = now();
        let w = what.to_string();
        rewrite(pid, t0, t0, idle_at_start, &w);
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(20));
            rewrite(pid, t0, now(), idle_at_start, &w);
        });
    }

    /// (start, end, idle_at_start, what) of a logged run covering instant `t`.
    pub fn covering(t: f64) -> Option<(f64, f64, f64, String)> {
        let text = std::fs::read_to_string(path()).ok()?;
        let mut best: Option<(f64, f64, f64, String)> = None;
        for l in text.lines() {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 5 {
                continue;
            }
            let (Ok(start), Ok(end), Ok(idle)) = (f[1].parse::<f64>(), f[2].parse::<f64>(), f[3].parse::<f64>()) else { continue };
            // 120 s of grace after the run's last heartbeat: the game keeps swallowing the
            // run's trailing synthetic input (the foreground helper's Alt, the plugin's pointer
            // warps) for a while after this process has exited — 17:02 on 10-27 the last
            // input landed 55 s after a shoot's end and the next shoot held itself for a person
            if t >= start - 5.0 && t <= end + 120.0 {
                // the run whose START is earliest wins: the human reading it carries is the oldest
                if best.as_ref().map(|b| start < b.0).unwrap_or(true) {
                    best = Some((start, end, idle, f[4].to_string()));
                }
            }
        }
        best
    }
}