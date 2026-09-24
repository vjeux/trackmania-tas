//! shootctl's view of the game lock — a thin shim over [`tmdrive`].
//!
//! # Why this is a shim now
//!
//! The lock used to live here: a `mkdir` on the render box owned by a name of
//! the caller's choosing. It worked, and it was ignored, because nothing made
//! a driver take it — three drivers bypassed it on 2026-09-23 and one left it
//! held by a dead process for an hour while two others drove the game anyway.
//!
//! It also spent months fighting a question it could not answer: WHICH PID is
//! the holder? The CLI `acquire` exits immediately, so its own pid is dead a
//! millisecond later; recording the parent worked until the parent was the
//! bridge daemon, and a stale-lock sweeper killed it and took the box away
//! from every session (2026-09-08). Both patches are in this file's history.
//!
//! `tmdrive` dissolves that question rather than answering it: the owner is an
//! agentcloud SESSION, not a process. A session outlives any one command, can
//! be asked for the box, and its liveness is a fact the platform knows. No pid
//! heuristics, and nothing to kill.
//!
//! # Why the signatures did not change
//!
//! Every call site in shootctl keeps working unmodified — the `&Path` and
//! `owner` arguments are accepted and mapped onto the session lock. That was
//! deliberate: this crate had active uncommitted work in flight from another
//! session when the lock landed, and a signature change would have collided
//! with it for no benefit.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tmdrive::{GameLock, Host};

/// The guard this process holds, if any. Process-global so the old
/// `acquire(...); work; release(...)` call shape keeps working.
static HELD: Mutex<Option<GameLock>> = Mutex::new(None);

/// Kept for call compatibility; the real lock lives under the game's
/// PluginStorage so the in-game plugin can read the same record.
pub fn lock_dir() -> PathBuf {
    PathBuf::from(tmdrive::LOCK_DIR)
}

/// Take the box.
///
/// `_d` and `max_age_s` are ignored: the location is tmdrive's, and staleness
/// is no longer a caller's guess — a lock is reclaimable when its lease goes
/// unrenewed or its owning session stops running. `wait_s` still waits.
pub fn acquire(_d: &Path, owner: &str, wait_s: u64, _max_age_s: u64) -> Result<(), String> {
    let mut g = HELD.lock().map_err(|_| "lock poisoned".to_string())?;
    if g.is_some() {
        return Ok(()); // already ours; re-entrant by design
    }
    let host = Host::detect();
    if wait_s > 0 {
        if let Ok(me) = tmdrive::Identity::from_env() {
            tmdrive::wait_until_free(&host, &me.session_id, wait_s).map_err(|e| e.to_string())?;
        }
    }
    match tmdrive::acquire(host, owner) {
        Ok(l) => {
            println!("game lock: held by session {} ({owner})", l.session_id());
            *g = Some(l);
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Give it back. Dropping the process does this too.
pub fn release(_d: &Path, _owner: &str) -> Result<(), String> {
    let mut g = HELD.lock().map_err(|_| "lock poisoned".to_string())?;
    match g.take() {
        None => Ok(()),
        Some(l) => l.release().map_err(|e| e.to_string()),
    }
}

/// Who holds the box? Exit code 1 when a live driver holds it.
pub fn status(_d: &Path) -> i32 {
    match tmdrive::holder(&Host::detect()) {
        Ok(None) => {
            println!("game lock: free");
            0
        }
        Ok(Some(h)) => {
            println!("game lock: {}", h.summary());
            if h.reclaimable.is_none() {
                println!(
                    "  ask for it:  agentcloudctl send-message --to {} --body '...'",
                    h.session_id
                );
                1
            } else {
                println!("  (reclaimable: lease expired or the owning session settled)");
                0
            }
        }
        Err(e) => {
            eprintln!("game lock: {e}");
            1
        }
    }
}

/// Run `f` with the held guard. Fails if this process never took the lock.
pub fn with<T>(f: impl FnOnce(&GameLock) -> Result<T, String>) -> Result<T, String> {
    let g = HELD.lock().map_err(|_| "lock poisoned".to_string())?;
    match g.as_ref() {
        Some(l) => f(l),
        None => Err("no game lock held — acquire it first (one game, one driver)".to_string()),
    }
}

/// The command name of a live process (`/proc/<pid>/comm`), None if gone.
///
/// Retained because the diagnostics around a stuck box still use it; the lock
/// itself no longer depends on any pid.
pub fn pid_comm(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|s| s.trim().to_string())
}

/// Whether a process with this command name would be a fair thing to kill:
/// one of our own tools or a shell wrapping them, never the bridge daemon.
///
/// The lock no longer records or kills pids, so this is now only advisory —
/// but the rule it encodes was learned expensively (killing the bridge daemon
/// took the box from every session at once, 2026-09-08) and is kept for any
/// caller still reasoning about a wedged process.
pub fn killable_holder(comm: &str) -> bool {
    matches!(
        comm,
        "shootctl" | "tinyctl" | "tmdrive" | "jumprig" | "sh" | "bash" | "dash" | "timeout"
    )
}
