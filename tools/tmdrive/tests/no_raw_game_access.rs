//! THE CONTRACT: nothing but `tmdrive` reaches the game directly.
//!
//! The Rust guard ([`tmdrive::GameLock`]) makes it impossible to *call* a game
//! operation without the lock — but it cannot stop a crate from shelling out
//! to the same scripts behind its back. That is not hypothetical: it is
//! exactly how the jump-test harness drove the game over a held lock on
//! 2026-09-23, and how the publishing chains drove an unlocked instance at the
//! same time.
//!
//! So this test fails the build when any crate other than `tmdrive` names a
//! raw path to the game. Adding one means editing this test, which is a
//! deliberate act with a reviewer attached rather than an accident.
//!
//! If you are here because this test failed: do not add your file to the
//! allowlist. Route the call through `tmdrive::ops`, which takes the lock.

use std::path::{Path, PathBuf};

/// Ways to reach the game that bypass the lock.
///
/// These are matched as plain substrings against code (comments stripped).
/// They are deliberately the TOOL NAMES rather than whole command lines: the
/// first version of this list matched `"/F /IM Trackmania.exe"` and sailed
/// past a planted `.args(["/F", "/IM", "Trackmania.exe"])`, because separate
/// string literals never form that substring. A detector that cannot catch
/// the bypass it was written for is worse than none, so match the smallest
/// distinctive token instead.
const FORBIDDEN: &[(&str, &str)] = &[
    ("explaunch", "launches the game"),
    ("hplnav", "sends OS input to the game window"),
    ("inject.ps1", "injects a DLL into the game"),
    ("taskkill", "kills processes — the game among them"),
    ("Trackmania.exe", "the game executable"),
    ("TmForever.exe", "the game executable"),
    ("29800", "the in-game plugin command channel"),
    ("explorer.exe", "how the game is started"),
];

/// Files that may name these paths, with the reason each is allowed.
///
/// `tmdrive` itself is the implementation. The others are documentation,
/// crash-dump analysis of an exe file at rest, and the plugin source — none of
/// which drive a running game.
fn allowed(rel: &Path) -> bool {
    let p = rel.to_string_lossy().replace('\\', "/");
    p.starts_with("tmdrive/")
        // mapgeom is OFFLINE analysis: crash dumps, .exe files at rest, map
        // geometry. It names the executable because it parses it, and it has
        // no path to a running game at all — there is nothing here for a lock
        // to protect.
        || p.starts_with("mapgeom/")
        // The bridge transport: generic, and forbidden from knowing about the
        // game -- guarded by `wsx_is_pure_transport` below.
        || p.starts_with("wsx/")
        // The contract itself.
        || p.contains("tests/no_raw_game_access.rs")
}

fn tools_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if p.is_dir() {
            if name == "target" || name == ".git" || name == "node_modules" {
                continue;
            }
            walk(&p, out);
        } else if name.ends_with(".rs") {
            out.push(p);
        }
    }
}

#[test]
fn only_tmdrive_touches_the_game() {
    let root = tools_root();
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(files.len() > 50, "expected to scan the whole workspace, found {}", files.len());

    let mut violations = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&root).unwrap_or(f);
        if allowed(rel) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        for (line_no, line) in text.lines().enumerate() {
            // Comments describe; they do not drive.
            let code = line.split("//").next().unwrap_or("");
            // A DELIBERATE, REVIEWABLE ESCAPE HATCH.
            //
            // Some of these names appear in code that is not driving the game
            // at all -- a profiler killing its own `typeperf` child, say. The
            // rule must not be weakened for those (a broad exemption for
            // "taskkill" would let a real bypass through), so instead the line
            // carries `tmdrive-allow: <why>` and the reason is in the diff
            // where a reviewer sees it.
            let allowed_here = line.contains("tmdrive-allow:")
                || (line_no > 0
                    && text.lines().nth(line_no - 1).map_or(false, |p| p.contains("tmdrive-allow:")));
            if allowed_here {
                continue;
            }
            for (needle, what) in FORBIDDEN {
                if code.contains(needle) {
                    violations.push(format!(
                        "{}:{}: names `{}` ({}) — route it through tmdrive::ops, which holds the lock",
                        rel.display(),
                        line_no + 1,
                        needle,
                        what
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "ONE GAME, ONE DRIVER — {} crate(s) reach the game without the lock:\n{}",
        violations.len(),
        violations.join("\n")
    );
}

/// `wsx` is a transport to a machine. It must not learn what a game is, or
/// what a lock is: the moment it does, the layering that keeps the lock
/// enforceable in one place is gone.
#[test]
fn wsx_is_pure_transport() {
    let root = tools_root();
    let mut files = Vec::new();
    walk(&root.join("wsx"), &mut files);
    assert!(!files.is_empty(), "wsx sources not found");

    let mut found = Vec::new();
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        for (line_no, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            let low = code.to_lowercase();
            // The repo is *called* trackmania-tas, so match the game rather
            // than the name: an exe, a window, a process, the plugin channel.
            for needle in [
                "trackmania.exe",
                "trackmania\\",
                "games/trackmania",
                "tmdrive",
                "game_lock",
                "gamelock",
                "render-lock",
                "shootctl-render",
                "29800",
            ] {
                if low.contains(needle) {
                    found.push(format!("wsx/{}:{}: mentions `{}`", 
                        f.strip_prefix(&root.join("wsx")).unwrap_or(f).display(), line_no + 1, needle));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "wsx must stay a generic transport with no game or lock knowledge:\n{}",
        found.join("\n")
    );
}

/// The guard must be the only way in: every mutating op in `tmdrive::ops`
/// takes `&GameLock`. A new op added without it would be callable unlocked.
#[test]
fn every_op_requires_the_guard() {
    let ops = tools_root().join("tmdrive/src/ops.rs");
    let text = std::fs::read_to_string(&ops).expect("tmdrive/src/ops.rs");
    let mut bad = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let l = line.trim();
        if !l.starts_with("pub fn ") {
            continue;
        }
        if !l.contains("&GameLock") && !l.contains("lock: &GameLock") {
            bad.push(format!("ops.rs:{}: `{}` does not take &GameLock", i + 1, l));
        }
    }
    assert!(
        bad.is_empty(),
        "every public operation in tmdrive::ops must require the lock:\n{}",
        bad.join("\n")
    );
}
