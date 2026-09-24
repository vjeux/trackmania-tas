//! shootctl's view of the game lock — a thin shim over [`tmdrive`].
//!
//! The lock itself used to live here: a `mkdir` on the render box owned by a
//! name of the caller's choosing. It worked, and it was ignored, because
//! nothing made a driver take it. Three separate drivers bypassed it on
//! 2026-09-23 and one of them left it held by a dead process for an hour
//! while two others drove the game anyway.
//!
//! So the lock moved into `tmdrive`, where it is:
//!
//! * tied to the GAME INSTANCE (pid), so a dead holder cannot wedge the box;
//! * tied to the SESSION, so a blocked driver can see who holds it and ask;
//! * mandatory, because every game operation now takes the guard.
//!
//! This module keeps shootctl's existing call shape and holds the guard in a
//! process-global, so the rest of the crate can drive the game without
//! threading a lock object through every function.

use std::sync::Mutex;
use tmdrive::{GameLock, Host};

static HELD: Mutex<Option<GameLock>> = Mutex::new(None);

/// Take the box for this process. `purpose` is shown to any driver that finds
/// the box busy, so make it specific.
pub fn acquire(purpose: &str) -> Result<(), String> {
    let mut g = HELD.lock().map_err(|_| "lock poisoned".to_string())?;
    if g.is_some() {
        return Ok(());
    }
    match tmdrive::acquire(Host::detect(), purpose) {
        Ok(l) => {
            println!("game lock: held by session {}", l.session_id());
            *g = Some(l);
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Give it back. Dropping the process does this too.
pub fn release() -> Result<(), String> {
    let mut g = HELD.lock().map_err(|_| "lock poisoned".to_string())?;
    match g.take() {
        None => Ok(()),
        Some(l) => l.release().map_err(|e| e.to_string()),
    }
}

/// Who holds the box? Exit code 1 when it is held by a live driver, matching
/// the old `shootctl lock status`.
pub fn status() -> i32 {
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
                println!("  (reclaimable: its game is gone, its lease expired, or its session settled)");
                0
            }
        }
        Err(e) => {
            eprintln!("game lock: {e}");
            1
        }
    }
}

/// Run `f` with the held guard. Fails if this process never took the lock,
/// which is the point: a game operation cannot run unlocked.
pub fn with<T>(f: impl FnOnce(&GameLock) -> Result<T, String>) -> Result<T, String> {
    let g = HELD.lock().map_err(|_| "lock poisoned".to_string())?;
    match g.as_ref() {
        Some(l) => f(l),
        None => Err("no game lock held — call shootctl::lock::acquire first \
                     (one game, one driver)"
            .to_string()),
    }
}
