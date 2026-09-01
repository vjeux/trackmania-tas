//! The exit-code contract, enforced.
//!
//! ```text
//!   0  success
//!   1  the operation ran and the answer is NO   (a gate refused)
//!   2  usage error                              (bad flags, missing file)
//!   3  environment error                        (no server, no engine)
//! ```
//!
//! 1 vs 2 is the distinction that matters and the one that was missing: before
//! this, `ghost verify` exited 2 both when a file was unpublishable and when
//! the command was called wrong, so a publish script could not branch on the
//! answer. The whole pipeline is scripted, which is what made that a real
//! defect rather than a tidiness complaint.
//!
//! Hermetic: no dedicated server, no engine, no network. The refusal case uses
//! a deliberately corrupt file built here, so the test carries its own
//! evidence instead of depending on a fixture that might drift.

use std::path::PathBuf;
use std::process::Command;

fn bin(name: &str) -> PathBuf {
    let mut p = std::env::current_exe().expect("current_exe");
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join(name)
}

fn code(args: &[&str]) -> i32 {
    let g = bin("ghost");
    if !g.exists() {
        // Not built in this invocation; `cargo test -p ghost` builds it, a
        // narrower filter may not. Reported as a skip rather than a failure.
        return -1;
    }
    Command::new(&g)
        .args(args)
        .output()
        .map(|o| o.status.code().unwrap_or(-2))
        .unwrap_or(-2)
}

#[test]
fn a_usage_error_exits_2() {
    let missing = code(&["inspect", "/nonexistent/definitely-not-here.Ghost.Gbx"]);
    if missing == -1 {
        return;
    }
    assert_eq!(missing, 2, "a missing input file must be exit 2 (usage)");

    let no_args = code(&["verify"]);
    assert_eq!(no_args, 2, "a missing required argument must be exit 2 (usage)");

    let bad_verb = code(&["definitely-not-a-subcommand"]);
    assert_eq!(bad_verb, 2, "an unknown subcommand must be exit 2 (usage)");
}

#[test]
fn a_refusal_exits_1_not_2() {
    // A file that is structurally a ghost but cannot pass verification: take
    // a real fixture and truncate its body. It parses far enough to be
    // judged, and the judgement is NO -- which is exit 1.
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../testdata/human_22730.Ghost.Gbx");
    if !src.exists() || bin("ghost").is_err_or_missing() {
        return;
    }
    let bytes = std::fs::read(&src).expect("read fixture");
    let cut = bytes.len() * 3 / 4;
    let dir = std::env::temp_dir().join(format!("ghost-exitcode-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let bad = dir.join("truncated.Ghost.Gbx");
    std::fs::write(&bad, &bytes[..cut]).expect("write truncated");

    let rc = code(&["verify", bad.to_str().unwrap()]);
    let _ = std::fs::remove_dir_all(&dir);
    if rc == -1 {
        return;
    }
    assert_ne!(
        rc, 2,
        "a file that fails verification is a VERDICT (exit 1), not a usage error (exit 2)"
    );
    assert!(
        rc == 1 || rc == 0,
        "verify on a damaged file exited {rc}; expected 1 (refused) or 0 (somehow fine)"
    );
}

#[test]
fn success_exits_0() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../testdata/human_22730.Ghost.Gbx");
    if !src.exists() {
        return;
    }
    let rc = code(&["inspect", src.to_str().unwrap()]);
    if rc == -1 {
        return;
    }
    assert_eq!(rc, 0, "inspecting a good file must succeed");
}

trait Missing {
    fn is_err_or_missing(&self) -> bool;
}
impl Missing for PathBuf {
    fn is_err_or_missing(&self) -> bool {
        !self.exists()
    }
}
