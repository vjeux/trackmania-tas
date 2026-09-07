//! `tinyctl replay-pull` — produce a REAL CLIENT RECORDING of a tiny map and
//! hand it to the player project.
//!
//! # Why
//!
//! The dedicated server and the client disagree about where a tiny map starts
//! (the server picks a fixed item slot, the client starts on the Spawn item —
//! measured 2026-09-07), and the player project's own measurement may itself be
//! a defect of its synthetic template container rather than of our maps. The
//! settling evidence is a recording made by the ORDINARY GAME on the ORDINARY
//! map, run through the server's `/validatepath`: two checkpoints means players
//! are fine and only the oracle needs the swap; a DNF at zero checkpoints means
//! the published files really are broken and the swapped freeze proceeds.
//!
//! # How
//!
//! The game already writes that artefact by itself. A finished solo run in play
//! mode is autosaved to
//! `Documents/Trackmania/Replays/Autosaves/vjeux_<MAP NAME>_PersonalBest_TimeAttack.Replay.Gbx`
//! with no dialog and no plugin involvement. Two traps this command handles so
//! nobody has to remember them:
//!
//!   * the file is keyed by the map's declared NAME, not by its uid or file
//!     name — a published tiny map still calls itself "Summer 2026 - 02", which
//!     is indistinguishable from the original's autosave, so the folder is
//!     cleared (moved aside, never deleted) before the run;
//!   * it is a PERSONAL BEST — a slower run does not overwrite a faster one, so
//!     without the clear you can pull a file from last week and never know.
//!
//! Sequence: move the Autosaves aside → open the map in play mode (holding the
//! render lock, through `shootctl playshots`, optionally holding the
//! accelerator with `--drive-ms`) → wait for a new autosave to appear → pull it
//! back with its md5. Driving a map that needs STEERING is not something this
//! can do: the run must be driven by a human at the box or by a fed input tape,
//! and `--wait-only` covers exactly that case (skip the play step, just watch
//! the folder while somebody drives).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::wsx::Wsx;

const AUTOSAVES: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Replays/Autosaves";
const PARKED: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Replays/_autosaves_parked";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";

fn quoted_win(p: &str) -> String {
    crate::wsx::to_win(p)
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let wait_only = args.iter().any(|a| a == "--wait-only");
    let map = f("--map");
    if map.is_none() && !wait_only {
        return Err("replay-pull needs --map MAP.Map.Gbx (or --wait-only to just watch for a run somebody else drives)".into());
    }
    let tag = f("--tag").unwrap_or_else(|| "rp".into());
    let out = PathBuf::from(
        f("--out").unwrap_or_else(|| "/home/vjeux/persistent/private-30d/tm-player/tiny/incoming/replays".into()),
    );
    let wait_s: u64 = f("--wait").and_then(|v| v.parse().ok()).unwrap_or(900);
    let drive_ms: u64 = f("--drive-ms").and_then(|v| v.parse().ok()).unwrap_or(0);
    let wsx = Wsx::new(args);

    // 1. Park the existing autosaves: the PersonalBest rule and the
    //    name collision both stop mattering once the folder is empty.
    let parked = format!("{PARKED}/{}", chrono_stamp());
    let clear = format!(
        "mkdir -p '{parked}' && (ls -1 '{AUTOSAVES}' 2>/dev/null | wc -l) && (mv '{AUTOSAVES}'/*.Replay.Gbx '{parked}'/ 2>/dev/null; true) && ls -1 '{AUTOSAVES}' 2>/dev/null | wc -l"
    );
    let before = wsx.sh(&clear)?;
    let mut it = before.split_whitespace().filter_map(|w| w.parse::<usize>().ok());
    let (had, now) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
    eprintln!("autosaves: {had} parked into {parked}, {now} left in the folder");
    if now != 0 {
        return Err(format!("{AUTOSAVES} still holds {now} replay(s) after the clear — refusing to guess which one the run wrote"));
    }

    // 2. Drive, or let a human drive.
    if let Some(m) = &map {
        if wait_only {
            eprintln!("--wait-only: NOT opening the map; drive it yourself on the box now");
        } else {
            let mut play: Vec<String> = vec![
                "play".into(),
                "--map".into(),
                m.clone(),
                "--tag".into(),
                tag.clone(),
                "--shots".into(),
                "1".into(),
                "--first-ms".into(),
                "800".into(),
            ];
            if drive_ms > 0 {
                play.push("--drive-ms".into());
                play.push(drive_ms.to_string());
            }
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            eprintln!("opening {m} in play mode …");
            let st = std::process::Command::new(exe).args(&play).status().map_err(|e| e.to_string())?;
            if !st.success() {
                eprintln!("  (tinyctl play returned {st} — the run may still be drivable by hand)");
            }
        }
    }

    // 3. Watch for the autosave. A finished run writes exactly one.
    eprintln!("waiting up to {wait_s}s for a finished run to autosave …");
    let t0 = Instant::now();
    let name = loop {
        let ls = wsx.sh(&format!("ls -1t '{AUTOSAVES}' 2>/dev/null | head -3"))?;
        let first = ls.lines().map(str::trim).find(|l| l.ends_with(".Replay.Gbx"));
        if let Some(n) = first {
            break n.to_string();
        }
        if t0.elapsed() > Duration::from_secs(wait_s) {
            return Err(format!(
                "no autosave appeared in {wait_s}s. A run only autosaves when it FINISHES — an unfinished or restarted run writes nothing. The parked replays are in {parked}"
            ));
        }
        std::thread::sleep(Duration::from_secs(5));
    };
    eprintln!("autosave: {name}");

    // 4. Pull it, with its md5 from the box (so a truncated transfer is loud).
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let remote = format!("{AUTOSAVES}/{name}");
    let md5_box = wsx
        .sh(&format!("md5sum '{remote}' | cut -d' ' -f1"))?
        .trim()
        .to_string();
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
    let local = out.join(&safe);
    let n = wsx.pull(&remote, &local)?;
    let md5_here = md5_of(&local)?;
    if md5_here != md5_box {
        return Err(format!("md5 mismatch: box {md5_box}, here {md5_here} — the transfer is not trustworthy"));
    }
    println!("{} ({n} B, md5 {md5_here})", local.display());
    println!("box copy: {remote}   parked autosaves: {parked}");
    println!("hand this to the player project for /validatepath: cps 2 on Summer-02-Tiny means the CLIENT path is sound and only the oracle needs the swap");
    let _ = quoted_win(AUTOSAVES);
    Ok(())
}

fn chrono_stamp() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{s}")
}

fn md5_of(p: &std::path::Path) -> Result<String, String> {
    let out = std::process::Command::new("md5sum").arg(p).output().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap_or("").to_string())
}
