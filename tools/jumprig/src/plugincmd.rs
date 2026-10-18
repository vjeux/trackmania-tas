//! `jumprig pcmd <PluginStorageFolder> <verb> [arg…]` — one command to any of
//! our probe plugins that speak the storage-folder protocol (cmd.txt `<seq>
//! <verb> [arg]`, cmd.seq watermark, state.json with `cmd_seq`, the full
//! report in out.txt): writes the command, waits for the plugin to act on it,
//! prints out.txt. Read-only on the game; the lock is taken by the caller.

use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::{num_after, str_after, POLL_TICK, PROGRESS_EVERY};

fn storage(folder: &str) -> PathBuf {
    PathBuf::from(format!("{}/Users/vjeux/OpenplanetNext/PluginStorage/{folder}", tmdrive::drive_c()))
}

fn read_seq(folder: &str) -> Option<(f64, i64, String)> {
    let raw = fs::read_to_string(storage(folder).join("state.json")).ok()?;
    if !raw.trim_end().ends_with('}') {
        return None;
    }
    Some((num_after(&raw, "heartbeat")?, num_after(&raw, "cmd_seq").unwrap_or(-1.0) as i64, str_after(&raw, "cmd_result").unwrap_or_default()))
}

/// Send `verb arg` and return the plugin's report (out.txt), or an error when
/// the plugin did not pick it up within `timeout`.
pub fn plugin_cmd(folder: &str, verb: &str, arg: &str, timeout: Duration) -> Result<String, String> {
    let dir = storage(folder);
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let (hb0, seq0, _) = read_seq(folder).ok_or_else(|| format!("no state.json in {} - is the plugin loaded?", dir.display()))?;
    let next = seq0 + 1;
    let line = if arg.is_empty() { format!("{next} {verb}") } else { format!("{next} {verb} {arg}") };
    let _ = fs::remove_file(dir.join("out.txt"));
    fs::write(dir.join("cmd.txt"), line).map_err(|e| format!("cannot write cmd.txt: {e}"))?;
    let start = Instant::now();
    let mut last_progress = Instant::now();
    loop {
        if let Some((hb, seq, result)) = read_seq(folder) {
            if seq >= next {
                // the report is written before state.json advances; read it now
                let out = fs::read_to_string(dir.join("out.txt")).unwrap_or_else(|_| result.clone());
                return Ok(out);
            }
            if start.elapsed() > Duration::from_secs(5) && hb == hb0 {
                return Err(format!("{folder}: heartbeat frozen at {hb} - the plugin is not running"));
            }
        }
        if start.elapsed() >= timeout {
            return Err(format!("TIMEOUT: {folder} did not act on command #{next} ({verb}) in {:.0}s", timeout.as_secs_f64()));
        }
        if last_progress.elapsed() >= PROGRESS_EVERY {
            last_progress = Instant::now();
            println!("  ... waiting for {folder} #{next} {verb} ({:.0}s)", start.elapsed().as_secs_f64());
        }
        sleep(POLL_TICK);
    }
}
