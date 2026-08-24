//! Where everything lives, in one place, so no other module guesses a path.
//!
//! The repo is the state of record. Every durable file this harness writes is
//! under `autopilot/` inside the checkout, and gets committed. A box holds
//! only a working copy plus its own volatile progress file.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Layout {
    pub repo: PathBuf,
}

impl Layout {
    pub fn new(repo: impl Into<PathBuf>) -> Layout {
        Layout { repo: repo.into() }
    }

    /// Find the repo root by walking up from `start` looking for `.git`.
    pub fn discover(start: &Path) -> Option<Layout> {
        let mut p = start.canonicalize().ok()?;
        loop {
            if p.join(".git").exists() {
                return Some(Layout::new(p));
            }
            p = p.parent()?.to_path_buf();
        }
    }

    pub fn root(&self) -> PathBuf {
        self.repo.join("autopilot")
    }
    pub fn config(&self) -> PathBuf {
        self.root().join("config")
    }
    pub fn job_spec(&self) -> PathBuf {
        self.config().join("job.rec")
    }
    pub fn state(&self) -> PathBuf {
        self.root().join("state")
    }
    pub fn journal_dir(&self) -> PathBuf {
        self.state().join("journal")
    }
    pub fn ledger_dir(&self) -> PathBuf {
        self.state().join("ledger")
    }
    pub fn alarm_dir(&self) -> PathBuf {
        self.state().join("alarms")
    }
    pub fn budget_dir(&self) -> PathBuf {
        self.state().join("budget")
    }
    pub fn boxes_dir(&self) -> PathBuf {
        self.state().join("boxes")
    }
    pub fn queue_dir(&self) -> PathBuf {
        self.state().join("queue")
    }
    pub fn queue_pending(&self) -> PathBuf {
        self.queue_dir().join("pending")
    }
    pub fn queue_claimed(&self) -> PathBuf {
        self.queue_dir().join("claimed")
    }
    pub fn queue_done(&self) -> PathBuf {
        self.queue_dir().join("done")
    }
    pub fn frontier(&self) -> PathBuf {
        self.state().join("frontier")
    }
    pub fn manifest(&self) -> PathBuf {
        self.state().join("MANIFEST.md5")
    }
    pub fn status_page(&self) -> PathBuf {
        self.root().join("STATUS.md")
    }
    pub fn ops_log(&self) -> PathBuf {
        self.root().join("OPS-LOG.md")
    }

    pub fn all_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.root(),
            self.config(),
            self.state(),
            self.journal_dir(),
            self.ledger_dir(),
            self.alarm_dir(),
            self.budget_dir(),
            self.boxes_dir(),
            self.queue_pending(),
            self.queue_claimed(),
            self.queue_done(),
            self.frontier(),
        ]
    }
}

/// The identity of this box, used to shard every append-only log so that two
/// boxes writing at once produce a directory union rather than a git conflict.
pub fn node_id() -> String {
    if let Ok(v) = std::env::var("TMHAUL_NODE") {
        if !v.trim().is_empty() {
            return sanitize(&v);
        }
    }
    let host = std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown-host".to_string());
    // `117796.od.fbinfra.net` -> `117796`; a devserver keeps its short name.
    sanitize(host.split('.').next().unwrap_or(&host))
}

pub fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_sits_under_autopilot_in_the_repo() {
        let l = Layout::new("/tmp/repo");
        for d in l.all_dirs() {
            assert!(d.starts_with("/tmp/repo/autopilot"), "{d:?} escaped the state tree");
        }
        assert!(l.status_page().starts_with("/tmp/repo/autopilot"));
    }

    #[test]
    fn node_id_is_filename_safe() {
        assert_eq!(sanitize("117796.od.fbinfra.net"), "117796-od-fbinfra-net");
        assert_eq!(sanitize("dev vm/42752"), "dev-vm-42752");
    }
}
