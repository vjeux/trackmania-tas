//! Every binary in the workspace answers `--version`, and says who it is.
//!
//! This is a release gate, not a nicety. Before it, `--version` existed
//! nowhere in 120k lines and 19 of 24 crates were still at the `cargo new`
//! default 0.1.0 -- so a bug report could not say which build produced the
//! file it was about.
//!
//! The test runs the built binaries out of the same target directory cargo
//! just wrote, so it costs nothing beyond process spawns and cannot drift from
//! what a release would actually ship.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Every binary the workspace produces. A new binary must be added here; that
/// is the point -- the list is the contract, and a missing entry is a review
/// comment rather than a silent gap.
const BINARIES: &[&str] = &[
    "asmdig",
    "asmshape",
    "chunkswap",
    "clip",
    "dsprobe",
    "ghost",
    "mapgeom",
    "pkz2",
    "playtest",
    "recon",
    "rend",
    "shootctl",
    "strpatch",
    "tmauto",
    "tmhaul",
    "tmmaps",
    "tmresim",
    "tmsite",
    "tmtraj",
    "uwlab",
    "vidread",
    "wincrash",
    "wsx",
];

/// The directory cargo put this test's own executable in is the same one it
/// put the binaries in (`target/<profile>/`), reached from
/// `target/<profile>/deps/<test binary>`.
fn bin_dir() -> PathBuf {
    let mut p = std::env::current_exe().expect("current_exe");
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p
}

fn run(bin: &Path, arg: &str) -> (bool, String, String) {
    match Command::new(bin).arg(arg).output() {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).trim().to_string(),
            String::from_utf8_lossy(&o.stderr).trim().to_string(),
        ),
        Err(e) => (false, String::new(), e.to_string()),
    }
}

#[test]
fn every_binary_answers_version_with_its_own_name() {
    let dir = bin_dir();
    let mut missing = Vec::new();
    let mut bad = Vec::new();

    for name in BINARIES {
        let bin = dir.join(name);
        if !bin.exists() {
            missing.push(*name);
            continue;
        }
        let (ok, out, err) = run(&bin, "--version");
        if !ok {
            bad.push(format!("{name}: --version exited non-zero ({err})"));
            continue;
        }
        // "<binary name> <version> (<build stamp>)" -- the binary's OWN name,
        // not its crate's. They differ for several of these (the `haul` crate
        // ships `tmhaul`, `clip` also ships `playtest`), and a binary that
        // reports its crate name sends a bug report to the wrong place.
        let first = out.lines().next().unwrap_or_default();
        if !first.starts_with(&format!("{name} ")) {
            bad.push(format!("{name}: --version said {first:?}"));
        }
        if !first.contains('(') || !first.contains(')') {
            bad.push(format!("{name}: no build stamp in {first:?}"));
        }
    }

    // A binary that has not been built is not a failure of THIS test -- an
    // individual `cargo test -p ghost` builds only what it needs. Only report
    // when nothing at all is there, which means the harness itself is wrong.
    assert!(
        missing.len() < BINARIES.len(),
        "no workspace binaries found in {} -- run `cargo build --release --workspace` first",
        dir.display()
    );
    // STALE BINARIES LOOK EXACTLY LIKE BROKEN ONES. `cargo test -p gbx` does
    // not rebuild the other crates' binaries, so this test happily runs
    // whatever was in target/ from an older build -- which is how it first
    // reported 14 broken tools that had already been fixed. Say so in the
    // failure rather than making the next person work it out.
    assert!(
        bad.is_empty(),
        "binaries with a broken --version:\n  {}\n\
         (if these were just fixed, the binaries in target/ may be STALE: \
         run `cargo build --release --workspace` and try again)",
        bad.join("\n  ")
    );
}

#[test]
fn every_binary_answers_help_without_failing() {
    let dir = bin_dir();
    let mut bad = Vec::new();
    let mut checked = 0;

    for name in BINARIES {
        let bin = dir.join(name);
        if !bin.exists() {
            continue;
        }
        checked += 1;
        let (ok, out, err) = run(&bin, "--help");
        // Help goes to stdout and exits 0. A tool that prints usage to stderr
        // and exits 2 is indistinguishable from a tool that rejected the flag,
        // which is what most of these did before the release work.
        if !ok {
            bad.push(format!("{name}: --help exited non-zero"));
            continue;
        }
        let text = if out.is_empty() { &err } else { &out };
        if text.is_empty() {
            bad.push(format!("{name}: --help printed nothing"));
        } else if !text.to_lowercase().contains(&name.to_lowercase()) {
            bad.push(format!("{name}: --help never names the tool"));
        }
    }

    assert!(checked > 0, "no workspace binaries found in {}", dir.display());
    assert!(
        bad.is_empty(),
        "binaries with broken --help:\n  {}\n\
         (if these were just fixed, the binaries in target/ may be STALE: \
         run `cargo build --release --workspace` and try again)",
        bad.join("\n  ")
    );
}
