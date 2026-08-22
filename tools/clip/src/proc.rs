//! Running other people's binaries, and the two scratch-path helpers every
//! command here needs.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// What a child process left behind.
pub struct Out {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Out {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
    /// The child's own complaint, for an error message. stderr if it said
    /// anything, else stdout, else the exit code -- never an empty string,
    /// because "release upload failed" with no reason is what sent people
    /// hunting through shell history.
    pub fn why(&self) -> String {
        let e = self.stderr.trim();
        if !e.is_empty() {
            return e.to_string();
        }
        let o = self.stdout.trim();
        if !o.is_empty() {
            return o.to_string();
        }
        match self.code {
            Some(c) => format!("exit {c}"),
            None => "killed by a signal".to_string(),
        }
    }
}

/// Run to completion, capturing both streams.
pub fn capture(cmd: &mut Command) -> Result<Out, String> {
    let o = cmd
        .output()
        .map_err(|e| format!("cannot run {:?}: {e}", cmd.get_program()))?;
    Ok(Out {
        code: o.status.code(),
        stdout: String::from_utf8_lossy(&o.stdout).to_string(),
        stderr: String::from_utf8_lossy(&o.stderr).to_string(),
    })
}

/// A name nothing else on the box is using, `$$`-style but finer grained:
/// two clips shipping in the same second must not stage over each other.
pub fn unique_suffix() -> String {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}_{}", std::process::id(), ns)
}

/// A private scratch directory under the system temp dir.
pub fn scratch_dir(prefix: &str) -> Result<PathBuf, String> {
    let d = std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()));
    std::fs::create_dir_all(&d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
    Ok(d)
}

/// Size of a file, refusing rather than guessing.
///
/// (The shell version needed `stat -c%s` on GNU and `stat -f%z` on BSD and got
/// the fallback order wrong once. std has neither problem.)
pub fn filesize(p: &Path) -> Result<u64, String> {
    std::fs::metadata(p)
        .map(|m| m.len())
        .map_err(|e| format!("cannot stat {}: {e}", p.display()))
}
