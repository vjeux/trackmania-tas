//! The candidate scratch root, and why it is claimed rather than just created.
//!
//! Worker directories are named by index. Two searches sharing one root
//! therefore share `w007/`, and each one's server validates whichever candidate
//! the other wrote last -- crediting the other process's time to its own
//! candidate. That is not a theory: a controlled A/B on one map gave 13 banked
//! bests of which 7 were phantoms on a shared root, and 8 of 8 exact on
//! distinct roots. One of the phantoms re-simulated to the untouched
//! template's own time, which no physics story explains.
//!
//! The default is therefore per-pid, the claim is atomic, and a root owned by a
//! live process is refused instead of wiped.

use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Root {
    pub path: PathBuf,
}

impl Root {
    pub fn default_path() -> PathBuf {
        PathBuf::from(format!("/dev/shm/tmsearch-{}", std::process::id()))
    }

    pub fn claim(path: &Path) -> Result<Root, String> {
        std::fs::create_dir_all(path).map_err(|e| format!("create {}: {}", path.display(), e))?;
        let marker = path.join(".owner");
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&marker) {
            Ok(mut f) => {
                let _ = writeln!(f, "{}", std::process::id());
            }
            Err(_) => {
                let txt = std::fs::read_to_string(&marker).unwrap_or_default();
                let pid: i32 = txt.trim().parse().unwrap_or(0);
                let mine = pid == std::process::id() as i32;
                if pid != 0 && !mine && Path::new(&format!("/proc/{}", pid)).exists() {
                    return Err(format!(
                        "{} belongs to live pid {}. Pass a distinct --root per process: \
                         sharing one makes two searches validate each other's candidates \
                         and bank the result as their own.",
                        path.display(),
                        pid
                    ));
                }
                let mut f = std::fs::File::create(&marker)
                    .map_err(|e| format!("reclaim {}: {}", marker.display(), e))?;
                let _ = writeln!(f, "{}", std::process::id());
            }
        }
        Ok(Root { path: path.to_path_buf() })
    }

    /// Empty the root, keeping the ownership marker -- a plain
    /// `remove_dir_all` deletes your own lock.
    pub fn reset(&self) {
        if let Ok(rd) = std::fs::read_dir(&self.path) {
            for e in rd.flatten() {
                if e.file_name() == ".owner" {
                    continue;
                }
                let p = e.path();
                let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
            }
        }
    }

    pub fn join(&self, s: impl AsRef<Path>) -> PathBuf {
        self.path.join(s)
    }
}
