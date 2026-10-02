//! Where the game is, and how commands get there.
//!
//! A driver runs either ON the render box (inside its WSL distro) or on a
//! devserver reaching it across the WhiteStick bridge. Only the transport
//! differs, so it is isolated here and everything above it is written once.
//!
//! The bridge client (`wsx`) is a pure file/command transport: it knows
//! nothing about Trackmania and nothing about the lock, and must not. All the
//! game's semantics live in this crate; `wsx` is the pipe it happens to use.

use std::process::Command;

#[derive(Debug, Clone)]
pub enum Host {
    /// Running on the box itself: commands go to the local shell.
    Local,
    /// Running elsewhere: commands cross the bridge via the `wsx` binary.
    Bridge { wsx: String },
}

impl Host {
    /// Pick the transport by looking for the box's own filesystem. Being on
    /// the box is a fact we can observe, not a flag to be passed wrongly.
    pub fn detect() -> Host {
        if std::path::Path::new("/mnt/c/Windows").exists() {
            Host::Local
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
            Host::Bridge { wsx: format!("{home}/bin/wsx") }
        }
    }

    pub fn bridge_with(wsx: &str) -> Host {
        Host::Bridge { wsx: wsx.to_string() }
    }

    /// Run a shell command on the box and return its stdout.
    ///
    /// Named `read_cmd` rather than `run` as a reminder of the rule this crate
    /// enforces: reads are free, but anything that *drives* the game must go
    /// through a [`crate::GameLock`] method in [`crate::ops`], never through
    /// this directly.
    pub fn read_cmd(&self, cmd: &str) -> Result<String, String> {
        match self {
            Host::Local => {
                let out = Command::new("/bin/sh")
                    .arg("-c")
                    .arg(cmd)
                    .output()
                    .map_err(|e| format!("sh: {e}"))?;
                if !out.status.success() {
                    return Err(format!(
                        "command failed ({}): {}",
                        out.status,
                        String::from_utf8_lossy(&out.stderr).trim()
                    ));
                }
                Ok(String::from_utf8_lossy(&out.stdout).into_owned())
            }
            Host::Bridge { wsx } => {
                let out = Command::new(wsx)
                    .arg("sh")
                    .arg(cmd)
                    .output()
                    .map_err(|e| format!("{wsx}: {e}"))?;
                if !out.status.success() {
                    return Err(format!(
                        "bridge command failed ({}): {}",
                        out.status,
                        String::from_utf8_lossy(&out.stderr).trim()
                    ));
                }
                Ok(String::from_utf8_lossy(&out.stdout).into_owned())
            }
        }
    }

    /// Copy a local file onto the box.
    pub fn push(&self, local: &str, remote: &str) -> Result<(), String> {
        match self {
            Host::Local => {
                std::fs::copy(local, remote).map_err(|e| format!("copy: {e}"))?;
                Ok(())
            }
            Host::Bridge { wsx } => {
                let out = Command::new(wsx)
                    .args(["push", local, remote])
                    .output()
                    .map_err(|e| format!("{wsx}: {e}"))?;
                if !out.status.success() {
                    return Err(format!(
                        "push failed: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    ));
                }
                Ok(())
            }
        }
    }

    /// Copy a file back off the box.
    pub fn pull(&self, remote: &str, local: &str) -> Result<(), String> {
        match self {
            Host::Local => {
                std::fs::copy(remote, local).map_err(|e| format!("copy: {e}"))?;
                Ok(())
            }
            Host::Bridge { wsx } => {
                let out = Command::new(wsx)
                    .args(["pull", remote, local])
                    .output()
                    .map_err(|e| format!("{wsx}: {e}"))?;
                if !out.status.success() {
                    return Err(format!(
                        "pull failed: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    ));
                }
                Ok(())
            }
        }
    }
}
