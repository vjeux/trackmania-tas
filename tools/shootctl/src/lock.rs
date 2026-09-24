//! shootctl's view of the game lock — a thin shim over [`tmdrive`].
//!
//! # Why this is a shim now
//!
//! The lock used to live here: a `mkdir` on the render box owned by a name of
//! the caller's choosing. It worked, and it was ignored, because nothing made
//! a driver take it — three drivers bypassed it on 2026-09-23, and one left it
//! held by a dead process for an hour while two others drove the game anyway.
//!
//! It also spent months fighting a question it could not answer: WHICH PID is
//! the holder? The CLI `acquire` exits immediately, so its own pid is dead a
//! millisecond later; recording the parent worked until the parent was the
//! bridge daemon, and a stale-lock sweeper killed it and took the box from
//! every session at once (2026-09-08).
//!
//! `tmdrive` dissolves that question rather than answering it: the owner is an
//! agentcloud SESSION, not a process. A session outlives any one command, can
//! be asked for the box, and its liveness is a fact the platform knows. No pid
//! heuristics, and nothing to kill.
//!
//! # Two ways to hold it, because there are two shapes of caller
//!
//! * [`acquire`] — IN-PROCESS. The guard lives in this process and renews
//!   itself; dropping it (or exiting) releases the box. This is what every
//!   `acquire(); work; release()` inside one shootctl run wants.
//! * [`acquire_cli`] — ACROSS PROCESSES, for
//!   `shootctl lock acquire; job; shootctl lock release`. The guard cannot
//!   live in any of those three processes, so tmdrive spawns a detached
//!   renewer that beats until the lock is released. Without it the lease would
//!   expire mid-job and hand the box to somebody else.
//!
//! # Why the signatures did not change
//!
//! Every existing call site keeps working unmodified — the `&Path` and `owner`
//! arguments are accepted and mapped onto the session lock. This crate had
//! active uncommitted work in flight from another session when the lock
//! landed, and a signature change would have collided with it for no benefit.

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

/// Take the box for THIS PROCESS. Released when the process exits.
///
/// `_d` and `_max_age_s` are ignored: the location is tmdrive's, and staleness
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

/// Give it back (the in-process guard).
pub fn release(_d: &Path, _owner: &str) -> Result<(), String> {
    let mut g = HELD.lock().map_err(|_| "lock poisoned".to_string())?;
    match g.take() {
        None => Ok(()),
        Some(l) => l.release().map_err(|e| e.to_string()),
    }
}

/// Take the box from the COMMAND LINE, keeping it after this process exits.
/// See the module docs.
pub fn acquire_cli(_d: &Path, owner: &str, wait_s: u64) -> Result<(), String> {
    let host = Host::detect();
    if wait_s > 0 {
        if let Ok(me) = tmdrive::Identity::from_env() {
            tmdrive::wait_until_free(&host, &me.session_id, wait_s).map_err(|e| e.to_string())?;
        }
    }
    tmdrive::acquire_detached(host, owner).map_err(|e| e.to_string())?;
    println!("game lock: held ({owner}) — renewed in the background until released");
    Ok(())
}

/// Release a lock taken by [`acquire_cli`], from a different process.
pub fn release_cli(_d: &Path, _owner: &str) -> Result<(), String> {
    match tmdrive::release_detached(&Host::detect()) {
        Ok(msg) => {
            println!("game lock: {msg}");
            Ok(())
        }
        Err(e) => Err(e.to_string()),
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

/// Run `f` with the guard this process holds.
///
/// Falls back to a transient guard when the lock is already ours but was taken
/// by ANOTHER process (the `shootctl lock acquire` shell pattern): re-acquiring
/// is a no-op for the same session, and the transient guard does not release
/// the outer hold when it drops.
pub fn with<T>(f: impl FnOnce(&GameLock) -> Result<T, String>) -> Result<T, String> {
    {
        let g = HELD.lock().map_err(|_| "lock poisoned".to_string())?;
        if let Some(l) = g.as_ref() {
            return f(l);
        }
    }
    let lock = tmdrive::acquire(Host::detect(), "shootctl").map_err(|e| e.to_string())?;
    f(&lock)
}

/// The command name of a live process (`/proc/<pid>/comm`), None if gone.
///
/// Retained because diagnostics around a stuck box still use it; the lock
/// itself no longer depends on any pid.
pub fn pid_comm(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|s| s.trim().to_string())
}

/// Whether a process with this command name would be a fair thing to kill:
/// one of our own tools or a shell wrapping them, never the bridge daemon.
///
/// Advisory now — the lock records and kills no pids — but the rule it encodes
/// was learned expensively (killing the bridge daemon took the box from every
/// session at once, 2026-09-08).
pub fn killable_holder(comm: &str) -> bool {
    matches!(
        comm,
        "shootctl" | "tinyctl" | "tmdrive" | "jumprig" | "sh" | "bash" | "dash" | "timeout"
    )
}
