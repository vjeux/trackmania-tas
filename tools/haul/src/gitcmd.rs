//! Running external tools, with the failure path written out deliberately.
//!
//! `.ok()?` is `2>/dev/null` with a nicer spelling, and this project has
//! already lost a corpus scan to it: a subcommand that did not exist returned
//! `None`, `None` meant "identical", and the whole corpus came back clean.
//! So every call here returns the exit status, stdout AND stderr, and a
//! non-zero exit is an `Err` carrying the stderr.

use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run(dir: &Path, prog: &str, args: &[&str]) -> Result<Output, String> {
    let out = Command::new(prog)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("spawn {prog} {args:?}: {e}"))?;
    let o = Output {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    };
    if o.code != 0 {
        return Err(format!(
            "{prog} {} exited {}: {}",
            args.join(" "),
            o.code,
            o.stderr.trim()
        ));
    }
    Ok(o)
}

/// Like `run`, but a non-zero exit is a value rather than an error — for the
/// handful of commands where "no" is an answer (`git diff --quiet`).
pub fn try_run(dir: &Path, prog: &str, args: &[&str]) -> Result<Output, String> {
    let out = Command::new(prog)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("spawn {prog} {args:?}: {e}"))?;
    Ok(Output {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    })
}

pub fn git(dir: &Path, args: &[&str]) -> Result<Output, String> {
    run(dir, "git", args)
}

pub fn head_sha(dir: &Path) -> Result<String, String> {
    Ok(git(dir, &["rev-parse", "HEAD"])?.stdout.trim().to_string())
}

pub fn branch(dir: &Path) -> Result<String, String> {
    Ok(git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])?.stdout.trim().to_string())
}

pub fn is_clean(dir: &Path) -> Result<bool, String> {
    Ok(git(dir, &["status", "--porcelain"])?.stdout.trim().is_empty())
}

/// Commits on this branch that the named remote-tracking ref does not have.
pub fn unpushed(dir: &Path, upstream: &str) -> Result<usize, String> {
    let o = try_run(dir, "git", &["rev-list", "--count", &format!("{upstream}..HEAD")])?;
    if o.code != 0 {
        return Ok(usize::MAX); // no such upstream ref: treat as "unknown, assume behind"
    }
    Ok(o.stdout.trim().parse().unwrap_or(0))
}
