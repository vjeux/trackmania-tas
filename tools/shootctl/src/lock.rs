//! `shootctl lock` — one game, one driver.
//!
//! # Why this has to exist
//!
//! The render box runs ONE Trackmania instance. Everything in `shootctl`
//! assumes it is the only thing talking to it: `setup` imports ghosts into the
//! running map, `shoot` starts a capture and then watches the screenshots
//! folder for a new `.webm`. Two drivers at once do not fail — they SUCCEED,
//! wrongly. The second one's `setup` lands its ghosts in the first one's scene,
//! and whichever `shoot` finishes second picks up a `.webm` the other one made.
//! You get two clips, both plausible, at least one of them of the wrong run,
//! and nothing anywhere says so.
//!
//! That is the exact failure shape this project keeps paying for: a wrong
//! artefact that passes every check because no check is looking. So the lock is
//! not a convenience for tidy scheduling. It is the guard.
//!
//! # How it works
//!
//! `mkdir` on the render box's own filesystem, which is atomic on every
//! filesystem WSL exposes — the directory either gets created or it does not,
//! with no window between checking and creating. The owner's id and the
//! acquisition time go INSIDE it, so a held lock can always say who holds it
//! and for how long; a lock that cannot name its owner is one nobody dares
//! break, and it gets deleted by hand five minutes later.
//!
//! A lock is not a lease with a heartbeat. A crashed driver leaves its lock
//! behind, so `--max-age` breaks one older than a stated age — and breaking is
//! LOUD, printing whose lock it was and how old, because a broken lock means
//! somebody's render died and that is worth knowing.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Where the lock lives on the render box. Not `/tmp`: this must outlive a
/// reboot of nothing in particular and be visible to every driver, and `/tmp`
/// on this box is swept.
pub fn lock_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("SHOOTCTL_LOCK").unwrap_or_else(|_| "/home/vjeux/.shootctl-render.lock".into()),
    )
}

fn now_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn read_owner(d: &Path) -> (String, u64) {
    let who = std::fs::read_to_string(d.join("owner")).unwrap_or_else(|_| "unknown".into());
    let at: u64 = std::fs::read_to_string(d.join("since"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    (who.trim().to_string(), at)
}

/// The holder's pid, when the lock recorded one (locks taken before 2026-09-07
/// have none).
fn read_pid(d: &Path) -> Option<u32> {
    std::fs::read_to_string(d.join("pid")).ok().and_then(|s| s.trim().parse().ok())
}

/// Whether a process with this pid is alive on THIS machine. The lock and every
/// driver live on the render box, so `/proc/<pid>` is the truth; a pid recorded
/// by another machine would read as dead — which is why the pid is only
/// written by drivers running on the box (all of them do).
fn pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// The command name of a live process (`/proc/<pid>/comm`), None if gone.
pub fn pid_comm(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|s| s.trim().to_string())
}

/// Whether a process with this command name is a legitimate lock holder — one
/// of our drivers, or a shell wrapping one. The WhiteStick bridge daemon
/// (`navi-node`, `whitestick`, `node`…) is NOT: it dispatches every remote
/// command, so a bare `wsx sh 'shootctl lock acquire'` has it as parent, and a
/// lock naming it invites the one kill that takes the whole box offline
/// (2026-09-08 05:05Z: every session lost the game for the night's remainder
/// until vjeux restarted the daemon by hand).
pub fn killable_holder(comm: &str) -> bool {
    matches!(comm, "shootctl" | "tinyctl" | "tmmaps" | "mapgeom" | "sh" | "bash" | "dash" | "zsh" | "timeout" | "setsid" | "nohup")
}

/// A held lock whose holder PROCESS is gone. Eight sessions queue on this one
/// game (2026-09-07): a driver killed by the bridge's timeout, a `pkill`, or a
/// panic leaves its lock behind, and with no `--max-age` every waiter sat on it
/// for its full 1500 s while the game idled. The pid inside the lock makes the
/// check exact: no age guessing, a live long render is never broken, a dead
/// holder is broken at once. Locks without a pid fall back to `--max-age`.
fn holder_dead(d: &Path) -> Option<u32> {
    match read_pid(d) {
        Some(pid) if !pid_alive(pid) => Some(pid),
        _ => None,
    }
}

/// Take the lock, or say who has it. Returns `Ok(())` only when this process
/// now holds it.
///
/// `wait_s` > 0 polls until the holder releases or the wait runs out. Polling
/// rather than any notification mechanism because the holder may be on another
/// machine entirely — the arms driving this box are separate agents on separate
/// nodes, and the only thing they share is this filesystem.
pub fn acquire(d: &Path, owner: &str, wait_s: u64, max_age_s: u64) -> Result<(), String> {
    let deadline = now_s() + wait_s;
    loop {
        match std::fs::create_dir(d) {
            Ok(()) => {
                // The contents are written AFTER the directory exists, so there
                // is a moment where the lock is held by someone unnamed. That
                // is the right order: an unnamed holder is a held lock, and the
                // reverse order would let a second driver create the directory
                // while the first was still writing its name into it.
                let _ = std::fs::write(d.join("owner"), owner);
                let _ = std::fs::write(d.join("since"), now_s().to_string());
                let _ = std::fs::write(d.join("pid"), std::process::id().to_string());
                println!("render lock: held by {owner}");
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let (who, at) = read_owner(d);
                let age = now_s().saturating_sub(at);
                if let Some(pid) = holder_dead(d) {
                    // The holder died: the lock is a leftover, not a render.
                    // Loud all the same — a dead driver is worth knowing about.
                    println!(
                        "render lock: BREAKING a lock held by {who} (pid {pid}, gone) after {age}s \
                         -- that driver died mid-render, which is worth looking into"
                    );
                    let _ = std::fs::remove_dir_all(d);
                    continue;
                }
                if max_age_s > 0 && at > 0 && age > max_age_s {
                    // BREAKING IS LOUD. A stale lock means a render died, and
                    // silently stepping over it hides that.
                    println!(
                        "render lock: BREAKING a lock held by {who} for {age}s (over the \
                         {max_age_s}s limit) -- that driver died mid-render, which is worth \
                         looking into"
                    );
                    let _ = std::fs::remove_dir_all(d);
                    continue;
                }
                if now_s() >= deadline {
                    return Err(format!(
                        "the render box is held by {who} ({age}s). One game, one driver: \
                         rendering anyway would put your ghosts in their scene and hand one of \
                         you the other's .webm. Wait, or --max-age to break a dead one."
                    ));
                }
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
            Err(e) => return Err(format!("{}: {e}", d.display())),
        }
    }
}

/// Give it up. Releasing a lock you do not hold is refused rather than ignored:
/// it means two drivers disagree about who is rendering, and the quiet version
/// of that is one of them stepping on the other.
pub fn release(d: &Path, owner: &str) -> Result<(), String> {
    if !d.exists() {
        return Err("no render lock is held".into());
    }
    let (who, at) = read_owner(d);
    if who != owner {
        return Err(format!(
            "the render lock is held by {who}, not by {owner} -- refusing to release somebody \
             else's lock"
        ));
    }
    std::fs::remove_dir_all(d).map_err(|e| e.to_string())?;
    println!("render lock: released by {owner} after {}s", now_s().saturating_sub(at));
    Ok(())
}

/// Who holds it, if anyone — and whether that holder is still alive.
pub fn status(d: &Path) -> i32 {
    if !d.exists() {
        println!("render lock: free");
        return 0;
    }
    let (who, at) = read_owner(d);
    let alive = match read_pid(d) {
        Some(pid) if pid_alive(pid) => {
            let comm = pid_comm(pid).unwrap_or_else(|| "?".into());
            if killable_holder(&comm) {
                format!("pid {pid} `{comm}`, alive")
            } else {
                format!("pid {pid} `{comm}`, alive -- NOT A DRIVER: DO NOT KILL IT (it is the bridge or a system process; wait for --max-age instead)")
            }
        }
        Some(pid) => format!("pid {pid}, DEAD -- the next acquire breaks it"),
        None => "no pid recorded".to_string(),
    };
    println!("render lock: held by {who} for {}s ({alive})", now_s().saturating_sub(at));
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lock path of this test's own. NOT an env var: these tests run in
    /// PARALLEL in one process, so a global would have them share a lock and
    /// fail each other -- which is what the first version did, and the failure
    /// read as "the refusal does not name the holder" rather than as a test
    /// bug. It is also why the functions take a path: reading the environment
    /// inside them made the lock un-testable in the shape it is used.
    fn tmp_lock(name: &str) -> PathBuf {
        let p = std::env::temp_dir()
            .join(format!("shootctl-lock-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    /// A lock whose holder process is gone is broken by the next acquire, at
    /// once, without any --max-age: eight sessions queue on one game, and a
    /// dead holder used to cost every waiter its full wait.
    #[test]
    fn a_dead_holders_lock_is_broken_at_once() {
        let d = tmp_lock("dead");
        let d = d.as_path();
        acquire(d, "arm-dead", 0, 0).expect("first acquire must succeed");
        // forge a holder pid that cannot be alive (pid_max is 2^22 on Linux)
        std::fs::write(d.join("pid"), "4194000").unwrap();
        acquire(d, "arm-b", 0, 0).expect("a dead holder's lock must be broken");
        let (who, _) = read_owner(d);
        assert_eq!(who, "arm-b");
        release(d, "arm-b").unwrap();
        // and a LIVE holder (this very process) is still refused
        acquire(d, "arm-a", 0, 0).unwrap();
        acquire(d, "arm-b", 0, 0).expect_err("a live holder is never broken");
        release(d, "arm-a").unwrap();
    }

    /// THE SECOND DRIVER MUST BE REFUSED. This is the whole point: two
    /// simultaneous renders do not error, they produce two plausible clips of
    /// which one is of the wrong run.
    #[test]
    fn a_second_driver_cannot_take_a_held_lock() {
        let d = tmp_lock("second");
        let d = d.as_path();
            acquire(d, "arm-a", 0, 0).expect("first acquire must succeed");
            let e = acquire(d, "arm-b", 0, 0).expect_err("the second acquire must be REFUSED");
            assert!(e.contains("arm-a"), "the refusal must name the holder: {e}");
            release(d, "arm-a").unwrap();
            acquire(d, "arm-b", 0, 0).expect("after release the lock is free");
            release(d, "arm-b").unwrap();
    }

    /// Releasing somebody else's lock is a refusal, not a no-op. Two drivers
    /// disagreeing about who renders is the bug; letting one clear the other's
    /// lock makes it silent.
    #[test]
    fn nobody_releases_a_lock_they_do_not_hold() {
        let d = tmp_lock("release");
        let d = d.as_path();
            acquire(d, "arm-a", 0, 0).unwrap();
            let e = release(d, "arm-b").expect_err("releasing another's lock must be refused");
            assert!(e.contains("arm-a"));
            release(d, "arm-a").unwrap();
            assert!(release(d, "arm-a").is_err(), "releasing a free lock is an error too");
    }

    /// A dead driver's lock can be broken, and only when it is genuinely old.
    #[test]
    fn a_stale_lock_is_breakable_and_a_fresh_one_is_not() {
        let d = tmp_lock("stale");
        let d = d.as_path();
            acquire(d, "arm-dead", 0, 0).unwrap();
            // Fresh: --max-age must NOT break it.
            assert!(
                acquire(d, "arm-b", 0, 3600).is_err(),
                "a lock seconds old must not be broken by a one-hour limit"
            );
            // Backdate it and try again.
            std::fs::write(d.join("since"), (now_s() - 7200).to_string()).unwrap();
            acquire(d, "arm-b", 0, 3600).expect("a two-hour-old lock must break under a one-hour limit");
            release(d, "arm-b").unwrap();
    }
}
