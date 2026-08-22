//! The PLAIN ORACLE: the TM2020 dedicated server, validating a file exactly the
//! way it would validate a submitted replay.
//!
//! Two things this module exists to make easy, because both are controls the
//! project has needed and not had:
//!
//!   * `MapsMode::Empty` -- validate with an EMPTY `UserData/Maps`. If the file
//!     still returns a time, the map it ran on came out of the file itself.
//!     This is the one-command proof behind "THE MAP IS INSIDE THE REPLAY".
//!   * every run is on THE WRITTEN FILE. Nothing here takes a time from a
//!     search log or a header; a banked incumbent is not a result until the
//!     plain oracle re-simulates the tape on disk.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MapsMode<'a> {
    /// exactly one map linked in
    One(&'a Path),
    /// a Maps directory containing zero files
    Empty,
}

#[derive(Debug, Clone)]
pub struct SimResult {
    pub file: String,
    pub time_ms: Option<i64>,
    pub cps: Option<u32>,
    pub map_uid: Option<String>,
    pub raw: String,
}

impl SimResult {
    pub fn secs(&self) -> String {
        match self.time_ms {
            None => "DNF".into(),
            Some(t) => crate::container::secs(t),
        }
    }
}

pub fn server_dir(explicit: Option<&str>) -> PathBuf {
    if let Some(d) = explicit {
        return PathBuf::from(d);
    }
    if let Ok(d) = std::env::var("TM_SERVER") {
        return PathBuf::from(d);
    }
    PathBuf::from("/tmp/tmoracle/server")
}

fn link(target: &Path, at: &Path) {
    if std::fs::symlink_metadata(at).is_err() {
        let _ = std::os::unix::fs::symlink(target, at);
    }
}

/// Run the plain oracle on one file. Returns the server's own answer.
pub fn validate(
    server: &Path,
    ghost: &Path,
    maps: MapsMode,
    tag: &str,
) -> Result<SimResult, String> {
    let bin = server.join("TrackmaniaServer");
    if !bin.exists() {
        return Err(format!(
            "no dedicated server at {} -- set TM_SERVER or pass --server DIR",
            server.display()
        ));
    }
    let root = std::env::temp_dir().join(format!("ghostapi-oracle-{}-{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&root);
    let replays = root.join("UserData").join("Replays");
    let mapsdir = root.join("UserData").join("Maps");
    std::fs::create_dir_all(&replays).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&mapsdir).map_err(|e| e.to_string())?;
    link(&server.join("Packs"), &root.join("Packs"));
    link(&bin, &root.join("TrackmaniaServer"));
    if let MapsMode::One(m) = maps {
        let abs = std::fs::canonicalize(m).map_err(|e| format!("{}: {}", m.display(), e))?;
        link(&abs, &mapsdir.join(abs.file_name().unwrap()));
    }
    let gabs = std::fs::canonicalize(ghost).map_err(|e| format!("{}: {}", ghost.display(), e))?;
    link(&gabs, &replays.join(gabs.file_name().unwrap()));

    let out = Command::new("./TrackmaniaServer")
        .args(["/nodaemon", "/validatepath=."])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("launching the server: {}", e))?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_dir_all(&root);
    Ok(parse_one(
        &text,
        gabs.file_name().unwrap().to_string_lossy().as_ref(),
    ))
}

fn parse_one(text: &str, name: &str) -> SimResult {
    let mut time = None;
    let mut cps = None;
    let mut uid = None;
    let mut in_res = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("\"ValidatedResult\"") {
            in_res = !t.contains("null");
        } else if t.starts_with("\"MapUid\"") {
            uid = t
                .split(':')
                .nth(1)
                .map(|s| s.trim().trim_end_matches(',').trim_matches('"').to_string());
        } else if in_res && t.starts_with("\"Time\"") {
            time = t
                .split(':')
                .nth(1)
                .and_then(|s| s.trim().trim_end_matches(',').parse::<i64>().ok());
        } else if in_res && t.starts_with("\"NbCheckpoints\"") {
            cps = t
                .split(':')
                .nth(1)
                .and_then(|s| s.trim().trim_end_matches(',').parse::<u32>().ok());
        }
    }
    SimResult { file: name.into(), time_ms: time, cps, map_uid: uid, raw: text.into() }
}
