//! Every `tmtraj` subcommand answers `--help` on stdout with exit 0.
//!
//! `--help` was broken on the whole subcommand layer: the top-level binary
//! answered it (that was the R1 release pass), but `tmtraj export --help`
//! replied
//!
//! ```text
//! tmtraj export: unknown flag --help
//! usage: tmtraj export GHOST [--csv F] [--json F] [--full-json F]
//! ```
//!
//! -- printing usage that lists the flag it had just rejected, and exiting 2.
//! Six of twenty commands failed in four different ways: an unknown flag, an
//! "unknown manifest subcommand", a filesystem walk that reported
//! `no <mapid>-<slug>/replays/*.Ghost.Gbx`, and one that tried to OPEN a file
//! called `--help` (`No such file or directory`).
//!
//! The distinction this pins is the one the exit-code contract cares about:
//! **asking for help is exit 0; forgetting the arguments is exit 2.** Several
//! commands had collapsed those into a single branch.

use std::path::PathBuf;
use std::process::Command;

/// Every subcommand the top-level usage advertises.
const SUBCOMMANDS: &[&str] = &[
    "show",
    "export",
    "csvdiff",
    "fields",
    "bytes",
    "diff",
    "spawn",
    "inputs",
    "check",
    "gate",
    "manifest",
    "motion",
    "samplescan",
    "provenance",
    "impacts",
    "wheels",
    "facing",
    "route",
    "splits",
    "corpus",
];

fn tmtraj() -> Option<PathBuf> {
    let mut p = std::env::current_exe().ok()?;
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    let b = p.join("tmtraj");
    b.exists().then_some(b)
}

#[test]
fn every_subcommand_answers_help_with_exit_0() {
    let Some(bin) = tmtraj() else { return };
    let mut bad = Vec::new();
    for c in SUBCOMMANDS {
        let out = match Command::new(&bin).args([c, "--help"]).output() {
            Ok(o) => o,
            Err(e) => {
                bad.push(format!("{c}: could not run ({e})"));
                continue;
            }
        };
        let code = out.status.code().unwrap_or(-1);
        if code != 0 {
            let first = String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or_default()
                .to_string();
            bad.push(format!("{c}: exit {code} -- {first}"));
            continue;
        }
        // Help goes to STDOUT. A tool that prints usage to stderr is
        // indistinguishable from one that rejected the flag.
        if String::from_utf8_lossy(&out.stdout).trim().is_empty() {
            bad.push(format!("{c}: exit 0 but printed nothing to stdout"));
        }
    }
    assert!(
        bad.is_empty(),
        "subcommands that do not answer --help:\n  {}\n\
         (if these were just fixed, the binary in target/ may be STALE: run \
         `cargo build --release -p tmtraj`)",
        bad.join("\n  ")
    );
}

/// The other half of the contract, and the reason this is not just
/// "make everything exit 0": a command invoked with NO arguments has been
/// called wrongly, and must still say so.
#[test]
fn missing_arguments_is_still_a_usage_error() {
    let Some(bin) = tmtraj() else { return };
    let mut wrong = Vec::new();
    // Commands that require an operand. (`fields` and `show` legitimately do
    // something useful with none, so they are not in this list.)
    for c in ["check", "gate", "manifest", "export", "diff"] {
        let Ok(out) = Command::new(&bin).arg(c).output() else { continue };
        if out.status.code() != Some(2) {
            wrong.push(format!("{c}: exit {:?}, expected 2", out.status.code()));
        }
    }
    assert!(
        wrong.is_empty(),
        "commands that no longer report a usage error:\n  {}",
        wrong.join("\n  ")
    );
}
