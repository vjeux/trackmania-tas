//! Every operation that CHANGES the game's state.
//!
//! All of them take `&GameLock`. That is the enforcement on the Rust side:
//! there is no way to spell one of these calls without first holding the box,
//! so "I forgot to lock" is a compile error rather than a corrupted run found
//! days later. The contract test `no_raw_game_access.rs` asserts this file
//! contains nothing but guarded operations.
//!
//! Read-only helpers live in the crate root, not here.

use crate::{drive_c, system32, Error, GameLock, Result};

/// The ONE Trackmania install, in the Windows spelling Explorer needs.
///
/// WHY THERE IS EXACTLY ONE, AND WHY IT IS THIS ONE.
///
/// This box used to carry two complete installs -- Steam (7.0 GB) and Ubisoft
/// (6.5 GB) -- same game executable, INDEPENDENT Openplanet DLLs beside them.
/// Whichever launcher a tool happened to call decided which Openplanet it got,
/// and the two drifted: 1.29.14 on one, 1.28.0 on the other. That cost an
/// evening on 2026-09-23 -- a 50 s `Autodetecting game version data` scan on
/// every start and every plugin broken by APIs 1.28.0 lacks -- and it
/// presented as a network problem, then a hard link, then a crash. It was none
/// of those.
///
/// Steam is the survivor because Steam MANAGES it (`StateFlags 4`,
/// `BytesToDownload 0`: installed, current, silent). The Ubisoft copy was
/// unmanaged and sat behind an "Update available" modal that blocked launches.
pub const GAME_EXE_WIN: &str =
    "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Trackmania\\Trackmania.exe";

/// Steam's own app id for Trackmania.
///
/// LAUNCH THROUGH STEAM, NOT THROUGH THE EXE. Running `Trackmania.exe`
/// directly -- even the right one -- leaves Ubisoft Connect to sort out
/// entitlement on its own, and it does so by showing a modal that silently
/// prevents the game from starting. `steam://rungameid/...` hands the whole
/// launcher dance to Steam, which owns the licence here: measured
/// 2026-09-23, game up in 18 s and Openplanet 12 s later, against a launch
/// that simply never produced a process.
pub const STEAM_APP_ID: &str = "2225070";

/// The game directory, as a WSL-visible path.
pub const GAME_DIR_UNIX: &str = "/mnt/c/Program Files (x86)/Steam/steamapps/common/Trackmania";

/// Install locations that must NOT exist: a second copy here is the exact
/// condition that caused the 2026-09-23 version split.
pub const FORBIDDEN_INSTALL_DIRS: &[&str] =
    &["/mnt/c/Program Files (x86)/Ubisoft/Ubisoft Game Launcher/games/Trackmania"];

/// Every process name the game runs under.
pub const GAME_IMAGES: &[&str] = &["Trackmania.exe", "TmForever.exe"];

/// Also killed when restarting: the launcher re-spawns the game, so leaving it
/// running defeats the restart.
pub const GAME_AND_LAUNCHER_IMAGES: &[&str] =
    &["Trackmania.exe", "UbisoftGameLauncher.exe", "upc.exe"];

/// Openplanet's log, which is the only honest account of whether a launch
/// actually finished starting.
pub const OP_LOG: &str = "/mnt/c/Users/vjeux/OpenplanetNext/Openplanet.log";

/// Did the newest Openplanet session finish starting?
///
/// `Loop entry initialization...` is the line that means the script engine is
/// coming up. A session that sits at `Initializing meta...` is stalled on the
/// Nadeo web-services login — Openplanet is in the process and healthy, the
/// game renders, and then the game exits on its own a few minutes later with
/// no crash and no Windows error record. shootctl learned this in August; it
/// cost this session another two hours to rediscover from the other end.
fn openplanet_ready(host: &crate::Host) -> Option<bool> {
    let out = host
        .read_cmd(&format!(
            "n=$(grep -n 'Openplanet for Trackmania' '{OP_LOG}' 2>/dev/null | tail -1 | cut -d: -f1); \
             [ -n \"$n\" ] || {{ echo NOSESSION; exit 0; }}; \
             tail -n +\"$n\" '{OP_LOG}' | grep -qF 'Loop entry initialization' && echo READY || echo STALLED"
        ))
        .ok()?;
    match out.trim() {
        "READY" => Some(true),
        "STALLED" => Some(false),
        _ => None,
    }
}

/// Launch the game and wait until it is actually usable.
///
/// Three things have to be true, and each has burned an evening when assumed:
/// the process exists, it is the one that survives the startup handoff, and
/// Openplanet finished starting rather than stalling on the Nadeo login.
/// A stalled login is RETRIED, because the game it produces dies minutes later
/// for no visible reason.
pub fn launch(lock: &GameLock, timeout_s: u64) -> Result<u32> {
    crate::assert_single_install(&lock.host)?;

    if let Some(pid) = crate::game_pid(&lock.host) {
        if openplanet_ready(&lock.host) != Some(false) {
            lock.bind_game(pid)?;
            return Ok(pid);
        }
        eprintln!("tmdrive: the running game is stalled in Openplanet startup — restarting it");
        kill_with_launcher(lock)?;
        wait_gone(lock, 30);
    }

    let mut last = String::new();
    for attempt in 1..=3u32 {
        lock.host
            .read_cmd(&format!(
                "{}/Windows/explorer.exe 'steam://rungameid/{STEAM_APP_ID}' >/dev/null 2>&1 || true",
                drive_c()
            ))
            .map_err(Error::Op)?;

        match launch_settle(lock, timeout_s.min(240), attempt) {
            Ok(pid) => return Ok(pid),
            Err(e) => {
                last = e;
                eprintln!("tmdrive: launch attempt {attempt} failed: {last}");
                let _ = kill_with_launcher(lock);
                wait_gone(lock, 30);
            }
        }
    }
    Err(Error::Op(format!("the game would not start after 3 attempts: {last}")))
}

fn wait_gone(lock: &GameLock, secs: u64) {
    let deadline = crate::now_s() + secs;
    while crate::game_pid(&lock.host).is_some() && crate::now_s() < deadline {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// Wait for a stable pid, then for Openplanet to finish starting.
fn launch_settle(lock: &GameLock, timeout_s: u64, attempt: u32) -> std::result::Result<u32, String> {
    let deadline = crate::now_s() + timeout_s;
    let mut stable_pid: Option<u32> = None;
    let mut stable_hits = 0;
    let pid = loop {
        // Not the first pid we see: Trackmania hands off during startup (the
        // process Steam starts registers with Ubisoft Connect and exits, and
        // the launcher spawns the real game). Binding the first pid bound the
        // bootstrapper, and seconds later the lock believed its game had died.
        match crate::game_pid(&lock.host) {
            Some(p) if Some(p) == stable_pid => {
                stable_hits += 1;
                if stable_hits >= 3 {
                    break p;
                }
            }
            Some(p) => {
                stable_pid = Some(p);
                stable_hits = 1;
            }
            None => {
                stable_pid = None;
                stable_hits = 0;
            }
        }
        if crate::now_s() >= deadline {
            return Err(format!("no game process settled within {timeout_s}s"));
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
        lock.renew();
    };

    // The process exists. Now: did Openplanet finish starting, or is it stuck
    // on the Nadeo login? A stalled one renders fine and then exits minutes
    // later, so waiting this out here is far cheaper than debugging it later.
    while crate::now_s() < deadline {
        match openplanet_ready(&lock.host) {
            Some(true) => {
                if attempt > 1 {
                    eprintln!("tmdrive: launch attempt {attempt} succeeded");
                }
                lock.bind_game(pid).map_err(|e| e.to_string())?;
                return Ok(pid);
            }
            _ => {
                if crate::game_pid(&lock.host).is_none() {
                    return Err("the game exited during startup".into());
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
        lock.renew();
    }
    Err("Openplanet stalled during startup (the Nadeo login never completed)".into())
}

/// Start the game through Steam, and return immediately.
///
/// The raw start, for callers with their own settle logic (`shootctl launch`
/// has a retry/diagnosis loop worth keeping). [`launch`] is the one that waits
/// for a stable pid and a finished Openplanet startup.
///
/// Steam, not the exe: running Trackmania.exe directly leaves Ubisoft Connect
/// to sort out entitlement, which it does by showing a modal that silently
/// prevents the game from starting.
pub fn launch_via_steam(lock: &GameLock) -> Result<()> {
    crate::assert_single_install(&lock.host)?;
    lock.host
        .read_cmd(&format!(
            "{}/Windows/explorer.exe 'steam://rungameid/{STEAM_APP_ID}' >/dev/null 2>&1 || true",
            drive_c()
        ))
        .map_err(Error::Op)?;
    lock.renew();
    Ok(())
}

/// Kill the game processes by image name. Low-level: callers wanting
/// diagnosis and retry (like `shootctl launch`) build on this.
pub fn kill_images(lock: &GameLock, images: &[&str]) -> Result<()> {
    for image in images {
        let _ = lock.host.read_cmd(&format!(
            "{} /F /IM {image} >/dev/null 2>&1; true",
            system32("taskkill.exe")
        ));
    }
    lock.renew();
    Ok(())
}

/// Stop the game. `taskkill` rather than a polite quit: the plugin's own quit
/// path needs a healthy script engine, which is exactly what is missing when a
/// driver needs to stop a wedged instance.
pub fn kill(lock: &GameLock) -> Result<()> {
    kill_images(lock, GAME_IMAGES)
}

/// Stop the game *and* its launcher, for a clean restart.
pub fn kill_with_launcher(lock: &GameLock) -> Result<()> {
    kill_images(lock, GAME_AND_LAUNCHER_IMAGES)
}

/// Start the game through Explorer at an explicit path.
///
/// Kept for callers that must bypass Steam (historical cells junctioned into
/// the install path). Prefer [`launch`].
pub fn launch_via_explorer(lock: &GameLock, exe_win_path: &str) -> Result<()> {
    lock.host
        .read_cmd(&format!(
            "{}/Windows/explorer.exe '{}' >/dev/null 2>&1; true",
            drive_c(),
            exe_win_path.replace('\'', "")
        ))
        .map_err(Error::Op)?;
    lock.renew();
    Ok(())
}

/// Send a command to the in-game plugin, carrying this lock's token. The
/// plugin refuses commands whose token does not match the live lock record.
pub fn plugin(lock: &GameLock, endpoint: &str, query: &str) -> Result<String> {
    let route = if query.is_empty() {
        format!("/{}", endpoint.trim_start_matches('/'))
    } else {
        format!("/{}?{}", endpoint.trim_start_matches('/'), query)
    };
    let out = crate::plugin::get(&route, 30).map_err(Error::Op);
    lock.renew();
    out
}

/// GhostShooter reads the map path from this file, NOT from the request.
const EDITMAP_TXT: &str =
    "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter/editmap.txt";

/// Load a map in the running game.
///
/// The `/playmap` route takes only `mode` from the query and reads the PATH
/// from `editmap.txt` — so passing the path as a query argument silently
/// loaded whatever the file happened to say last time. Reported from the MK64
/// session on 2026-09-23 after it loaded someone else's map twice.
///
/// Empty mode is deliberate: a mode name the title does not have loaded makes
/// PlayMap fail silently — it returns, the title reports ready, and no
/// playground ever appears.
pub fn play_map(lock: &GameLock, map_path: &str) -> Result<String> {
    let p = crate::game_path(map_path).map_err(Error::Op)?;
    lock.host
        .read_cmd(&format!("printf '%s' '{}' > '{EDITMAP_TXT}'", p.replace('\'', "")))
        .map_err(Error::Op)?;
    let out = plugin(lock, "playmap", "mode=")?;
    Ok(format!("{} [{}]", out.trim(), p))
}

/// Send OS-level key input to the game window.
pub fn input(lock: &GameLock, keys: &str, hold_ms: u64) -> Result<String> {
    let nav = format!("{}/Users/vjeux/hplnav.exe", drive_c());
    let out = lock
        .host
        .read_cmd(&format!("'{nav}' keys '{}' {hold_ms}", keys.replace('\'', "")))
        .map_err(Error::Op);
    lock.renew();
    out
}

/// Inject a DLL into the running game.
pub fn inject(lock: &GameLock, dll_win_path: &str) -> Result<String> {
    let ps = system32("WindowsPowerShell/v1.0/powershell.exe");
    let out = lock
        .host
        .read_cmd(&format!(
            "{ps} -ExecutionPolicy Bypass -File 'C:\\Users\\vjeux\\inject.ps1' -dll '{}'",
            dll_win_path.replace('\'', "")
        ))
        .map_err(Error::Op);
    lock.renew();
    out
}

/// Write a file into the game's own folders (plugins, maps, packs).
pub fn write_game_file(lock: &GameLock, local: &str, remote: &str) -> Result<()> {
    lock.host.push(local, remote).map_err(Error::Op)?;
    lock.renew();
    Ok(())
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
