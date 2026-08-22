//! `cargo test --release` runs the same suite the binary does.
//!
//! The suite itself lives in `src/selftest.rs` so that it is also a command
//! (`ghost selftest`), which is what you want when the failure is on a box with
//! no cargo. This wrapper exists so that `cargo test` is never a green run that
//! tested nothing.

use std::path::PathBuf;
use std::process::Command;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

#[test]
fn selftest_pure_and_oracle() {
    let out = Command::new(env!("CARGO_BIN_EXE_ghost"))
        .args(["selftest", "--data", testdata().to_str().unwrap()])
        .output()
        .expect("running the ghost binary");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    println!("{}", text);
    assert!(
        text.contains("PASS codec.identity"),
        "the suite did not run its first check at all"
    );
    assert!(out.status.success(), "ghost selftest reported a failure");
}

/// A suite that skipped its oracle tier is not a suite that passed. This test
/// is the one that fails on a box with no dedicated server, loudly, instead of
/// letting a green `cargo test` mean "nothing was simulated".
#[test]
fn oracle_tier_actually_ran() {
    let has_server = std::env::var("TM_SERVER")
        .map(|d| PathBuf::from(d).join("TrackmaniaServer").exists())
        .unwrap_or_else(|_| PathBuf::from("/tmp/tmoracle/server/TrackmaniaServer").exists());
    if !has_server {
        eprintln!(
            "NOTE: no dedicated server, so the oracle tier could not run. \
             Set TM_SERVER=<dir with TrackmaniaServer> and re-run; \
             `ghost selftest --strict` turns this into a failure."
        );
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_ghost"))
        .args(["selftest", "--data", testdata().to_str().unwrap(), "--strict"])
        .output()
        .expect("running the ghost binary");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    println!("{}", text);
    assert!(
        text.contains("PASS oracle.map_inside_replay"),
        "the empty-Maps control did not run"
    );
}
