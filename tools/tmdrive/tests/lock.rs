//! THE LOCK, ADVERSARIALLY — every way it has actually failed, as a test.
//!
//! Each case is an incident: an anonymous holder nobody can reach, two
//! sessions at once, a holder that stopped renewing, a released lock whose key
//! still worked, a nested acquire that changed the token, a nested release that
//! freed the outer hold, a second run from the same session trampling the
//! first, a holder losing the box while restarting its own game.
//!
//! This replaces `locktest.sh`. The shell version's cleanup helper was a bare
//! `rm -rf` of the lock directory, and on 2026-09-24 it deleted another
//! session's live record three times while their publish was mid-flight. The
//! one thing on the box exempt from the lock was the thing breaking it. Here
//! there is no such helper: the suite acquires through the same API everything
//! else uses, refuses to start over a live foreign hold, and only ever removes
//! records it created under its own fake session ids.
//!
//! RUNS ON THE BOX ONLY (`Host::detect()` must be `Local`; the lock directory
//! is a Windows path under PluginStorage). Anywhere else every case is skipped
//! rather than failed. Non-destructive by construction: nothing here kills the
//! game.
//!
//!     cargo test --release -p tmdrive --test lock -- --test-threads=1
//!
//! Serial on purpose: the cases share one real lock.

use std::process::Command;
use std::time::{Duration, Instant};
use tmdrive::{acquire_as, holder, Host, Identity, Reclaim, LOCK_DIR};

const A: &str = "11111111-aaaa-4aaa-8aaa-111111111111";
const B: &str = "22222222-bbbb-4bbb-8bbb-222222222222";

fn ident(id: &str, title: &str) -> Identity {
    Identity { session_id: id.to_string(), title: title.to_string() }
}

fn on_box() -> Option<Host> {
    let h = Host::detect();
    if !matches!(h, Host::Local) {
        eprintln!("skipped: the lock suite runs on the box only");
        return None;
    }
    if !std::path::Path::new("/mnt/c/Users/vjeux/OpenplanetNext").is_dir() {
        eprintln!("skipped: no OpenplanetNext on this host");
        return None;
    }
    Some(h)
}

fn sh(cmd: &str) -> String {
    let out = Command::new("/bin/sh").arg("-c").arg(cmd).output().expect("sh");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn read(name: &str) -> String {
    sh(&format!("cat '{LOCK_DIR}/{name}' 2>/dev/null"))
}

/// Remove the record ONLY if it belongs to one of the suite's fake sessions.
/// A live record from anyone else is an abort, never a deletion.
fn clear_ours_or_abort() {
    let owner = read("session");
    if owner.is_empty() {
        return;
    }
    if owner == A || owner == B {
        sh(&format!("rm -rf '{LOCK_DIR}'"));
        return;
    }
    panic!(
        "ABORT: the box is held by session {} ('{}') — not ours to remove. \
         The suite refuses to run over a live foreign hold.",
        &owner[..8.min(owner.len())],
        read("purpose")
    );
}

/// Wait for the box to be free of anyone but us, up to `max`, then clear ours.
fn setup(max: Duration) -> Option<Host> {
    let host = on_box()?;
    let start = Instant::now();
    loop {
        let owner = read("session");
        if owner.is_empty() || owner == A || owner == B {
            break;
        }
        // Someone real holds it. Wait; the suite never takes a live record.
        if let Ok(Some(h)) = holder(&host) {
            if h.reclaimable.is_some() {
                break; // acquire_as will reclaim it legitimately
            }
        }
        if start.elapsed() > max {
            panic!("the box stayed held by a foreign session for {max:?}; not running");
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    clear_ours_or_abort();
    Some(host)
}

fn write_fake_record(session: &str, title: &str, purpose: &str, renewed_ago_s: u64, owner_pid: Option<u32>) {
    let now = tmdrive::now_s();
    let renewed = now - renewed_ago_s;
    let pid = owner_pid.map(|p| p.to_string()).unwrap_or_default();
    sh(&format!(
        "mkdir -p '{LOCK_DIR}'; \
         printf '%s' '{session}' > '{LOCK_DIR}/session'; \
         printf '%s' '{title}' > '{LOCK_DIR}/title'; \
         printf '%s' '{purpose}' > '{LOCK_DIR}/purpose'; \
         printf '%s' '999999' > '{LOCK_DIR}/game_pid'; \
         printf '%s' '{now}' > '{LOCK_DIR}/acquired_at'; \
         printf '%s' '{renewed}' > '{LOCK_DIR}/renewed_at'; \
         printf '%s' '{pid}' > '{LOCK_DIR}/owner_pid'; \
         printf '%s' '{session}:deadbeef' > '{LOCK_DIR}/token'"
    ));
}

// ---------------------------------------------------------------------------

#[test]
fn anonymous_holders_are_refused() {
    let Some(_h) = setup(Duration::from_secs(1800)) else { return };
    // No session in the environment → no lock. An anonymous holder is one
    // nobody can ask for the box, which is how the old lock got wedged.
    std::env::remove_var("TM_SESSION");
    std::env::remove_var("AGENTCLOUD_SESSION_ID");
    let r = Identity::from_env();
    assert!(r.is_err(), "an identity with no session must be refused");
    clear_ours_or_abort();
}

#[test]
fn two_sessions_cannot_both_hold_it() {
    let Some(host) = setup(Duration::from_secs(1800)) else { return };
    let a = acquire_as(host.clone(), ident(A, "A"), "first").expect("A takes the box");
    let b = acquire_as(host.clone(), ident(B, "B"), "second");
    match b {
        Err(tmdrive::Error::Busy(h)) => {
            assert_eq!(h.session_id, A, "the refusal names the holder");
            assert_eq!(h.purpose, "first", "and what it is doing");
        }
        Ok(_) => panic!("B must be refused with BUSY, but it acquired the lock"),
        Err(e) => panic!("B must be refused with BUSY, got {e}"),
    }
    drop(a);
    assert!(read("session").is_empty(), "released on drop");
    clear_ours_or_abort();
}

#[test]
fn a_dead_game_alone_does_not_free_the_box() {
    // The MK64 fix: a holder that restarts the game as part of its own work
    // has no game process for a while. That used to hand the box away.
    let Some(host) = setup(Duration::from_secs(1800)) else { return };
    write_fake_record(A, "A", "game restarting", 0, Some(std::process::id()));
    let h = holder(&host).unwrap().expect("record present");
    assert!(h.game_gone, "no game pid 999999 exists");
    assert!(h.reclaimable.is_none(), "a fresh lease keeps the box even with no game: {:?}", h.reclaimable);
    let b = acquire_as(host.clone(), ident(B, "B"), "barging in");
    assert!(matches!(b, Err(tmdrive::Error::Busy(_))), "B is still refused");
    clear_ours_or_abort();
}

#[test]
fn an_unrenewed_lease_is_reclaimable() {
    let Some(host) = setup(Duration::from_secs(1800)) else { return };
    write_fake_record(A, "A", "wedged", tmdrive::LEASE_S + 5, None);
    let h = holder(&host).unwrap().expect("record present");
    assert_eq!(h.reclaimable, Some(Reclaim::LeaseExpired));
    let b = acquire_as(host.clone(), ident(B, "B"), "reclaiming").expect("B reclaims an expired lock");
    assert_eq!(read("session"), B);
    drop(b);
    clear_ours_or_abort();
}

#[test]
fn a_released_lock_leaves_no_key_behind() {
    let Some(host) = setup(Duration::from_secs(1800)) else { return };
    let a = acquire_as(host.clone(), ident(A, "A"), "brief").expect("A");
    assert!(!read("token").is_empty(), "a token exists while held");
    a.release().expect("release");
    assert!(read("token").is_empty(), "no token after release — a stale key opened the game once");
    clear_ours_or_abort();
}

#[test]
fn a_nested_acquire_keeps_the_token_and_the_hold() {
    // u10s 2026-09-24: the nested path minted a fresh token, the outer run's
    // TM_LOCK_TOKEN went stale, and every later command was refused.
    let Some(host) = setup(Duration::from_secs(1800)) else { return };
    let outer = acquire_as(host.clone(), ident(A, "A"), "outer").expect("outer");
    let t1 = read("token");
    {
        let inner = acquire_as(host.clone(), ident(A, "A"), "nested").expect("nested acquire is re-entrant");
        assert_eq!(inner.token(), t1, "the nested guard adopts the outer token");
        assert_eq!(read("token"), t1, "and does not rewrite the file");
    } // inner dropped here
    assert_eq!(read("session"), A, "a nested release does NOT end the outer hold");
    assert_eq!(read("token"), t1, "token unchanged after the nested guard dropped");
    drop(outer);
    assert!(read("session").is_empty(), "the outer release does");
    clear_ours_or_abort();
}

#[test]
fn the_lease_is_renewed_in_the_background() {
    let Some(host) = setup(Duration::from_secs(1800)) else { return };
    let a = acquire_as(host.clone(), ident(A, "A"), "long work").expect("A");
    let first: u64 = read("renewed_at").parse().unwrap();
    // One renewal period plus slack. The renewer beats every LEASE_S/3.
    std::thread::sleep(Duration::from_secs(tmdrive::LEASE_S / 3 + 8));
    let later: u64 = read("renewed_at").parse().unwrap();
    assert!(later > first, "renewed_at must advance while held ({first} -> {later})");
    let h = holder(&host).unwrap().unwrap();
    assert!(h.keeper_alive, "the keeper is this process");
    drop(a);
    clear_ours_or_abort();
}

#[test]
fn a_second_run_from_the_same_session_is_refused() {
    // u10s 2026-09-24: a probe run started while the publisher's run was live
    // joined its hold and restarted the game under it, twice.
    let Some(host) = setup(Duration::from_secs(1800)) else { return };
    let outer = acquire_as(host.clone(), ident(A, "A"), "publisher").expect("outer");
    let tm = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/release/tmdrive");
    let out = Command::new(tm)
        .args(["run", "--purpose", "probe", "--", "/bin/sh", "-c", "echo NESTED_RAN"])
        .env("TM_SESSION", A)
        .env("TM_SESSION_TITLE", "A")
        .env_remove("TM_LOCK_TOKEN")
        .output()
        .expect("run tmdrive");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(75), "a second run is refused with BUSY: {stderr}");
    assert!(stderr.contains("ALREADY holds"), "and it says why: {stderr}");
    let nested = Command::new(tm)
        .args(["run", "--nested", "--purpose", "probe", "--", "/bin/sh", "-c", "echo NESTED_RAN"])
        .env("TM_SESSION", A)
        .env("TM_SESSION_TITLE", "A")
        .env_remove("TM_LOCK_TOKEN")
        .output()
        .expect("run tmdrive");
    assert!(String::from_utf8_lossy(&nested.stdout).contains("NESTED_RAN"), "--nested opts in");
    assert_eq!(read("session"), A, "the outer hold survived both");
    drop(outer);
    clear_ours_or_abort();
}

#[test]
fn map_paths_outside_the_user_dir_are_refused() {
    // 2026-09-24: a map in a stray Documents\Trackmania was accepted by the
    // game and loaded nothing — no error, no dialog, ctx 0 forever.
    let bad = tmdrive::loadable_map_path("C:/Users/vjeux/Documents/Trackmania/Maps/x.Map.Gbx");
    assert!(bad.is_err(), "outside the game's user dir must be refused");
    assert!(bad.unwrap_err().contains("loads NOTHING"), "and the refusal explains the trap");
    let good = tmdrive::loadable_map_path("/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/x.Map.Gbx");
    assert_eq!(good.unwrap(), "C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/x.Map.Gbx");
    let wsl = tmdrive::game_path("/mnt/c/Users/vjeux/a b/c.Map.Gbx").unwrap();
    assert_eq!(wsl, "C:/Users/vjeux/a b/c.Map.Gbx", "WSL spelling is converted");
    let back = tmdrive::game_path("C:\\Users\\vjeux\\x.Map.Gbx").unwrap();
    assert_eq!(back, "C:/Users/vjeux/x.Map.Gbx", "backslashes are normalised");
}

#[test]
fn a_killed_run_releases_the_box() {
    // 2026-09-24: `kill <tmdrive run pid>` left a record with a dead keeper --
    // destructors do not run on a signal -- and everyone saw HELD for 120 s.
    let Some(_host) = setup(Duration::from_secs(1800)) else { return };
    let tm = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/release/tmdrive");
    let mut run = Command::new(tm)
        .args(["run", "--purpose", "about to be killed", "--", "/bin/sh", "-c", "sleep 60"])
        .env("TM_SESSION", A)
        .env("TM_SESSION_TITLE", "A")
        .env_remove("TM_LOCK_TOKEN")
        .spawn()
        .expect("spawn run");
    // Wait until it holds the box.
    let t0 = Instant::now();
    while read("session") != A {
        assert!(t0.elapsed() < Duration::from_secs(30), "the run never took the box");
        std::thread::sleep(Duration::from_millis(200));
    }
    unsafe {
        libc::kill(run.id() as i32, libc::SIGTERM);
    }
    let st = run.wait().expect("wait");
    assert!(!st.success(), "a signalled run does not report success");
    let t1 = Instant::now();
    while !read("session").is_empty() {
        assert!(t1.elapsed() < Duration::from_secs(5), "the box must be FREE within seconds of SIGTERM, not after the lease");
        std::thread::sleep(Duration::from_millis(100));
    }
    clear_ours_or_abort();
}
