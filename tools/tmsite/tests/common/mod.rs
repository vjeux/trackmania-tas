//! Shared test helpers.
//!
//! FIXTURE POLICY: a fixture that is not there is a FAILURE, never a skip. A
//! test that quietly passes because its input vanished is worse than no test --
//! it reports "green" for work it did not do.

#![allow(dead_code)] // each integration test binary uses a subset

use std::path::{Path, PathBuf};
use std::process::Command;

/// A path inside the crate (`tmsite/...`), asserted to exist.
pub fn fixture(rel: &str) -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    assert!(
        p.exists(),
        "missing fixture {}\n  expected at tmsite/{} (checked in with the crate).\n  A test whose fixture is absent must fail, not pass.",
        p.display(),
        rel
    );
    p
}

/// A path inside the repo but outside the crate (the ghost corpus under
/// `<repo>/<map>/replays/`), asserted to exist.
pub fn repo_fixture(rel: &str) -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    assert!(
        p.exists(),
        "missing repo fixture {}\n  expected at <repo>/{} (checked in).\n  A test whose fixture is absent must fail, not pass.",
        p.display(),
        rel
    );
    p
}

pub fn fixture_str(rel: &str) -> String {
    fixture(rel).to_string_lossy().into_owned()
}

pub fn repo_fixture_str(rel: &str) -> String {
    repo_fixture(rel).to_string_lossy().into_owned()
}

pub struct Run {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn ok(&self, what: &str) -> &Run {
        assert!(
            self.status.success(),
            "{} exited {:?}\n--- stdout\n{}\n--- stderr\n{}",
            what,
            self.status.code(),
            self.stdout,
            self.stderr
        );
        self
    }
    pub fn failed(&self, what: &str) -> &Run {
        assert!(
            !self.status.success(),
            "{} was expected to fail but exited 0\n--- stdout\n{}\n--- stderr\n{}",
            what,
            self.stdout,
            self.stderr
        );
        self
    }
}

/// Run the `tmsite` binary this test was built alongside.
pub fn tmsite(args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_tmsite"))
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("cannot run tmsite {:?}: {}", args, e));
    Run {
        status: out.status,
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Byte-compare against a committed golden, with a diff that says where.
pub fn assert_golden(golden_rel: &str, got: &[u8]) {
    let p = fixture(golden_rel);
    let want = std::fs::read(&p).unwrap_or_else(|e| panic!("read {}: {}", p.display(), e));
    if want == got {
        return;
    }
    let wl: Vec<&[u8]> = want.split(|b| *b == b'\n').collect();
    let gl: Vec<&[u8]> = got.split(|b| *b == b'\n').collect();
    let mut detail = String::new();
    for i in 0..wl.len().max(gl.len()) {
        let a = wl.get(i).map(|s| String::from_utf8_lossy(s).into_owned());
        let b = gl.get(i).map(|s| String::from_utf8_lossy(s).into_owned());
        if a != b {
            let trim = |s: Option<String>| {
                s.map(|x| x.chars().take(160).collect::<String>())
                    .unwrap_or_else(|| "<no such line>".into())
            };
            detail = format!(
                "first difference at line {}\n  golden: {}\n  got   : {}",
                i + 1,
                trim(a),
                trim(b)
            );
            break;
        }
    }
    panic!(
        "output does not match the committed golden tmsite/{}\n  golden {} bytes, got {} bytes\n{}\n\
         If this change is intended, re-bless the golden ON PURPOSE and say why in the commit.",
        golden_rel,
        want.len(),
        got.len(),
        detail
    );
}

/// A scratch directory unique to one test, removed if it already exists.
pub fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tmsite-test-{}", name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
