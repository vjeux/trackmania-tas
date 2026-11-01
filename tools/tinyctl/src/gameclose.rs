//! Close the game a run launched — the default, not a chore (vjeux 2026-10-31 20:39:
//! "make sure to close the game if you're not using it").
//!
//! `GameGuard::new(&wsx, args)`: taken at the start of a box run (`tinyctl shoot`,
//! `play`, `loadloop`, `remote-drive`). It remembers whether a Trackmania was up
//! BEFORE the run; when the run ends (any way out — the guard drops) and no game was
//! up before, the one that is up now was launched for this run, and it is closed BY
//! PID (`tmdrive kill` kills by name and took down a human's game on 2026-09-27).
//! `--keep-game` opts out: a sequence of runs passes it on all but the last, so the
//! game is launched once and closed once.
//!
//! `tinyctl box-close-game [--idle-s N] [--dry-run]`: the sweep — close an AGENT's
//! leftover game on an idle box: the render lock is dead/free/this session's (a lock
//! HELD by another live session means theirs), no `shootctl` run is driving it, and
//! the box has been idle ≥ N s (default 600). A human's game is never closed: a held
//! foreign lock or recent input refuses.

use crate::wsx::Wsx;

pub const SESSION: &str = "080a15b8-2c6a-4fff-9d8a-db7d0072ee43";

/// The pid of the Trackmania the box runs, if any (the first when several).
pub fn tm_pid(wsx: &Wsx) -> Option<u32> {
    let out = wsx.sh("/mnt/c/Windows/System32/tasklist.exe /FI \"IMAGENAME eq Trackmania.exe\" /NH /FO CSV 2>/dev/null | grep -i trackmania | head -1 | cut -d, -f2 | tr -d '\" \\r'; true").ok()?;
    out.trim().parse().ok()
}

/// Close a game by pid, never by name. Says what it did.
pub fn close_pid(wsx: &Wsx, pid: u32, why: &str) {
    let alive = wsx.sh(&format!("/mnt/c/Windows/System32/tasklist.exe /FI \"PID eq {pid}\" 2>/dev/null | grep -ci trackmania; true")).unwrap_or_default();
    if alive.trim().starts_with('0') {
        println!("  game pid {pid} already gone ({why})");
        return;
    }
    let _ = wsx.sh(&format!("/mnt/c/Windows/System32/taskkill.exe /PID {pid} /F > /dev/null 2>&1; true"));
    println!("  closed the game (pid {pid}, by pid) — {why}");
}

/// The run-scoped guard: see the module doc.
pub struct GameGuard<'a> {
    wsx: &'a Wsx,
    before: Option<u32>,
    keep: bool,
}

impl<'a> GameGuard<'a> {
    pub fn new(wsx: &'a Wsx, args: &[String]) -> GameGuard<'a> {
        let keep = args.iter().any(|a| a == "--keep-game") || std::env::var("TINY_KEEP_GAME").map(|v| v == "1").unwrap_or(false);
        let before = tm_pid(wsx);
        GameGuard { wsx, before, keep }
    }
}

impl Drop for GameGuard<'_> {
    fn drop(&mut self) {
        if self.keep || self.before.is_some() {
            return;
        }
        match tm_pid(self.wsx) {
            Some(pid) => close_pid(self.wsx, pid, "launched for this run; --keep-game keeps it"),
            None => {}
        }
    }
}

/// `tinyctl box-close-game [--idle-s N] [--dry-run]`.
pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let idle_s: u64 = f("--idle-s").map(|v| v.parse().map_err(|_| "--idle-s wants seconds")).transpose()?.unwrap_or(600);
    let dry = args.iter().any(|a| a == "--dry-run");
    let wsx = Wsx::new(args);
    let Some(pid) = tm_pid(&wsx) else {
        println!("no game running");
        return Ok(());
    };
    let lock = wsx.sh("/home/vjeux/bin/tmdrive status 2>/dev/null | head -1; true").unwrap_or_default().trim().to_string();
    let foreign_live = lock.starts_with("HELD") && !lock.contains(SESSION);
    if foreign_live {
        return Err(format!("game pid {pid} left alone: the render lock is HELD by another live session ({})", lock.chars().take(100).collect::<String>()));
    }
    let driving = wsx.sh("pgrep -f 'shootctl (loadloo[p]|shootse[t]|playshot[s]|prob[e]|ru[n])' >/dev/null && echo LIVE || echo none; true").unwrap_or_default().contains("LIVE");
    if driving {
        return Err(format!("game pid {pid} left alone: a shootctl run is driving it"));
    }
    let idle = crate::inputidle::read(&wsx, idle_s as u32).map_err(|e| format!("input idle: {e}"))?;
    if !idle.free {
        return Err(format!("game pid {pid} left alone: the box saw input within {idle_s} s (a person may be on it)"));
    }
    let agent = wsx.sh("head -1 /mnt/c/Users/vjeux/agent-game.txt 2>/dev/null | tr -d '\\r '; true").unwrap_or_default().trim().to_string();
    let why = format!("agent leftover (agent-game.txt {}, lock {}, idle ≥ {idle_s} s)", if agent == pid.to_string() { "matches" } else { "differs" }, lock.split_whitespace().next().unwrap_or("?"));
    if dry {
        println!("would close game pid {pid}: {why}");
        return Ok(());
    }
    close_pid(&wsx, pid, &why);
    Ok(())
}
