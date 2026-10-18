//! `tinyctl box-gate [--idle-secs N] [--allow NAME,..] [--twice]` — the box etiquette as ONE
//! fail-closed check before any launch (2026-10-17, the pause-lift rules): (1) the FOREIGN-GAME
//! census: any process whose image name starts with a known game (Hades2, Minecraft/javaw, …)
//! refuses; (2) the WINDOWED-APP census (tmdrive's rule, 2026-09-26: the lock does not see
//! humans): any process with a main window outside the allowlist (Trackmania, Openplanet,
//! Discord, RenderDoc, the shell hosts) refuses — a person is at the screen; (3) BOX-IDLE: the
//! Windows last-input age must be ≥ --idle-secs (default 300) — a fresh input clock means a
//! hand on the box; (4) `--lock-free TMDRIVE`: the tmdrive lock FREE or dead-reclaimable;
//! `--twice` repeats the whole check after 5 minutes (FREE-twice). Exit 3 on
//! refusal with the reason; exit 0 prints the census.
use std::process::Command;

const FOREIGN_GAME_PREFIXES: &[&str] = &["Hades2", "Hades", "Minecraft", "javaw", "Diablo", "Wow", "Overwatch", "Baldur", "Cyberpunk", "Fortnite", "VALORANT", "cs2", "csgo", "dota2", "RocketLeague", "eldenring", "Palworld", "Satisfactory", "Factorio", "FortniteClient", "LeagueClient", "League of Legends", "r5apex", "GTA5", "RDR2", "witcher3", "Starfield", "Skyrim", "FactoryGame", "Stardew", "Terraria", "Celeste", "Hollow", "Balatro", "Helldivers", "Deadlock", "Marvel", "Civilization", "stellaris", "eu4", "hoi4", "ck3", "Battle.net", "EpicGamesLauncher", "GalaxyClient"];
const WINDOW_ALLOW: &[&str] = &["Trackmania", "Openplanet", "Discord", "RenderDoc", "TextInputHost", "csrss", "explorer", "ApplicationFrameHost", "SystemSettings", "ShellExperienceHost", "SearchHost", "StartMenuExperienceHost", "Widgets", "msedgewebview2", "SecurityHealthSystray", "OneDrive", "steam", "steamwebhelper", "NVIDIA Overlay", "NVIDIA Share", "nvcontainer", "RtkAudUService64", "WindowsTerminal", "conhost", "powershell", "cmd", "wsl", "wslhost", "sihost", "taskhostw", "dwm", "LockApp", "Notepad"];

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

pub struct Census {
    pub foreign: Vec<String>,
    pub windows: Vec<(String, String)>,
    pub idle_secs: Option<f64>,
}

pub fn census(allow: &[String]) -> Result<Census, String> {
    let o = Command::new("/mnt/c/Windows/System32/tasklist.exe").output().map_err(|e| format!("tasklist.exe: {e}"))?;
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    if !text.contains("Image Name") {
        return Err("tasklist probe INVALID (no header)".into());
    }
    let mut foreign = Vec::new();
    for line in text.lines().skip(3) {
        let name = line.split_whitespace().next().unwrap_or("");
        let stem = name.trim_end_matches(".exe");
        if FOREIGN_GAME_PREFIXES.iter().any(|p| stem.to_ascii_lowercase().starts_with(&p.to_ascii_lowercase())) && !allow.iter().any(|a| a.eq_ignore_ascii_case(stem)) {
            foreign.push(name.to_string());
        }
    }
    foreign.sort();
    foreign.dedup();
    // windowed apps
    let ps = "Get-Process | Where-Object { $_.MainWindowTitle -ne '' } | ForEach-Object { $_.ProcessName + \"`t\" + $_.MainWindowTitle }";
    let o = Command::new("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe").args(["-NoProfile", "-Command", ps]).output().map_err(|e| format!("powershell: {e}"))?;
    let mut windows = Vec::new();
    for line in String::from_utf8_lossy(&o.stdout).lines() {
        let line = line.trim_end_matches('\r');
        let Some((proc_name, title)) = line.split_once('\t') else { continue };
        let ok = WINDOW_ALLOW.iter().any(|a| proc_name.eq_ignore_ascii_case(a)) || allow.iter().any(|a| a.eq_ignore_ascii_case(proc_name));
        if !ok {
            windows.push((proc_name.to_string(), title.to_string()));
        }
    }
    // last input age (GetLastInputInfo), seconds
    let ps2 = "Add-Type @'\nusing System; using System.Runtime.InteropServices;\npublic class LI { [StructLayout(LayoutKind.Sequential)] public struct LASTINPUTINFO { public uint cbSize; public uint dwTime; }\n [DllImport(\"user32.dll\")] public static extern bool GetLastInputInfo(ref LASTINPUTINFO plii);\n public static uint Age() { LASTINPUTINFO li = new LASTINPUTINFO(); li.cbSize = (uint)Marshal.SizeOf(li); GetLastInputInfo(ref li); return ((uint)Environment.TickCount - li.dwTime); } }\n'@\n[LI]::Age()";
    let o = Command::new("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe").args(["-NoProfile", "-Command", ps2]).output().map_err(|e| format!("powershell: {e}"))?;
    let idle_secs = String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok().map(|ms| ms / 1000.0);
    Ok(Census { foreign, windows, idle_secs })
}

/// `--lock-free TMDRIVE`: the render lock must be FREE (or dead-reclaimable) too — another
/// session's live hold refuses.
fn lock_state(tmdrive: &str) -> Result<String, String> {
    let o = Command::new(tmdrive).arg("status").output().map_err(|e| format!("{tmdrive}: {e}"))?;
    Ok(String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").trim().to_string())
}

/// One census → the refusal reasons (empty = FREE), the line printed.
fn check(allow: &[String], idle_min: f64, lock: Option<&str>) -> Result<Vec<String>, String> {
    let c = census(allow)?;
    let idle = c.idle_secs.map(|s| format!("{s:.0} s")).unwrap_or_else(|| "unknown".into());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    println!("[{now}] box-gate: foreign games {:?}; windowed apps outside the allowlist {:?}; last input {idle} ago", c.foreign, c.windows);
    let mut reasons = Vec::new();
    if !c.foreign.is_empty() {
        reasons.push(format!("FOREIGN GAMES RUNNING: {}", c.foreign.join(", ")));
    }
    if !c.windows.is_empty() {
        reasons.push(format!("A PERSON'S APP HAS A WINDOW: {}", c.windows.iter().map(|(p, t)| format!("{p} ({t})")).collect::<Vec<_>>().join(", ")));
    }
    match c.idle_secs {
        Some(s) if s < idle_min => reasons.push(format!("INPUT {s:.0} s AGO (< {idle_min:.0} s): a hand on the box")),
        None => reasons.push("last-input age unreadable".into()),
        _ => {}
    }
    if let Some(t) = lock {
        let st = lock_state(t)?;
        println!("  lock: {st}");
        if !(st.starts_with("FREE") || st.starts_with("DEAD(")) {
            reasons.push(format!("THE RENDER LOCK IS HELD: {}", st.chars().take(120).collect::<String>()));
        }
    }
    Ok(reasons)
}

/// `--wait S --every E`: keep checking every E s (default 300) until FREE (then the FREE-twice
/// confirmation when asked) or the deadline — exit 3 at the deadline with the last reasons.
pub fn cmd(rest: &[String]) -> Result<(), String> {
    let idle_min: f64 = f(rest, "--idle-secs").and_then(|v| v.parse().ok()).unwrap_or(300.0);
    let allow: Vec<String> = f(rest, "--allow").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let twice = rest.iter().any(|a| a == "--twice");
    let wait: u64 = f(rest, "--wait").and_then(|v| v.parse().ok()).unwrap_or(0);
    let every: u64 = f(rest, "--every").and_then(|v| v.parse().ok()).unwrap_or(300);
    let lock = f(rest, "--lock-free");
    let t0 = std::time::Instant::now();
    loop {
        let mut reasons = check(&allow, idle_min, lock.as_deref())?;
        if reasons.is_empty() && twice {
            println!("second check in 300 s (FREE-twice)…");
            std::thread::sleep(std::time::Duration::from_secs(300));
            reasons = check(&allow, idle_min, lock.as_deref())?;
        }
        if reasons.is_empty() {
            println!("box-gate: FREE{}", if twice { " (twice, 5 min apart)" } else { "" });
            return Ok(());
        }
        if t0.elapsed().as_secs() + every > wait {
            eprintln!("box-gate REFUSED: {}", reasons.join("; "));
            std::process::exit(3);
        }
        println!("  not free ({}); next check in {every} s", reasons.join("; "));
        std::thread::sleep(std::time::Duration::from_secs(every));
    }
}
