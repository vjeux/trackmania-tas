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

/// Launch the game and wait for the process to appear.
pub fn launch(lock: &GameLock, timeout_s: u64) -> Result<u32> {
    // A second install is the failure this box keeps regrowing; catch it
    // before it silently decides which Openplanet the session gets.
    crate::assert_single_install(&lock.host)?;

    if let Some(pid) = crate::game_pid(&lock.host) {
        lock.bind_game(pid)?;
        return Ok(pid);
    }

    lock.host
        .read_cmd(&format!(
            "{}/Windows/explorer.exe 'steam://rungameid/{STEAM_APP_ID}' >/dev/null 2>&1 || true",
            drive_c()
        ))
        .map_err(Error::Op)?;

    // Poll for the process: a fact, not a guessed duration.
    let deadline = crate::now_s() + timeout_s;
    loop {
        if let Some(pid) = crate::game_pid(&lock.host) {
            lock.bind_game(pid)?;
            return Ok(pid);
        }
        if crate::now_s() >= deadline {
            return Err(Error::Op(format!(
                "the game did not start within {timeout_s}s. Steam may be closed, or Ubisoft \
                 Connect may be holding a modal — check the screen."
            )));
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
        lock.renew();
    }
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

/// Load a map in the running game.
pub fn play_map(lock: &GameLock, map_path: &str) -> Result<String> {
    plugin(lock, "playmap", &format!("path={}", urlencode(map_path)))
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
