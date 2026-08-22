//! oracle.rs -- drive the TM2020 dedicated server as a headless physics
//! oracle, over (map, ghosts) pairs. Port of segoracle.py's `run_maps` /
//! `Oracle`, following the approach already used by /tmp/tmsearch/src/oracle.rs.
//!
//! Each pair gets its OWN worker directory holding exactly one map, because
//! every segment map keeps the original mapUid (that is what lets unmodified
//! ghosts resolve against it) -- two segment maps in one UserData/Maps and the
//! server binds the uid to whichever it found first. This is a hard
//! requirement, not a nicety.
//!
//! The server is invoked as `./TrackmaniaServer /nodaemon /validatepath=.`
//! **from** the worker directory: an absolute path makes it use its own
//! directory's UserData and silently validate the wrong files.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const DEFAULT_SERVER: &str = "/tmp/tmoracle/server";

#[derive(Clone, Debug)]
pub struct Row {
    pub file: String,
    pub sim_time: Option<i64>,
    pub reached_cps: Option<u32>,
    #[allow(dead_code)]
    pub declared_time: Option<i64>,
}

/// The server's "never crossed" sentinel shows up as a huge u32.
pub const BAD_TIME: i64 = 4_294_967_000;

pub fn clean(t: Option<i64>) -> Option<i64> {
    match t {
        Some(v) if v > BAD_TIME || v < 0 => None,
        other => other,
    }
}

fn link(target: &Path, at: &Path) {
    if std::fs::symlink_metadata(at).is_err() {
        let _ = std::os::unix::fs::symlink(target, at);
    }
}

/// The server reads a candidate only if its name ends `.Ghost.Gbx` or
/// `.Replay.Gbx`. A file with any other name is **ignored**, and an ignored
/// file produces no result row at all — which every caller here reads as a
/// plain DNF, indistinguishable from a run that genuinely did not finish.
///
/// That matters most for map A/B work: "the gate removal changed nothing" and
/// "the server never opened the file" look identical. So the name is checked
/// before anything is staged, and a bad one is an error rather than a quiet
/// zero. The check reads the path in front of it — it takes no promise from a
/// caller about what the file is.
pub fn readable_name(g: &Path) -> Result<String, String> {
    let n = g
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} has no file name", g.display()))?;
    if n.ends_with(".Ghost.Gbx") || n.ends_with(".Replay.Gbx") {
        Ok(n)
    } else {
        Err(format!(
            "{}: the dedicated server ignores any candidate not named *.Ghost.Gbx or \
             *.Replay.Gbx, and an ignored file comes back as a plain DNF. Rename or link it \
             before validating.",
            g.display()
        ))
    }
}

fn stage(dir: &Path, server: &Path, map: &Path, ghosts: &[PathBuf]) {
    let replays = dir.join("UserData").join("Replays");
    let maps = dir.join("UserData").join("Maps");
    std::fs::create_dir_all(&replays).unwrap();
    std::fs::create_dir_all(&maps).unwrap();
    link(&server.join("Packs"), &dir.join("Packs"));
    link(
        &server.join("TrackmaniaServer"),
        &dir.join("TrackmaniaServer"),
    );
    let m = std::fs::canonicalize(map).unwrap();
    link(&m, &maps.join(m.file_name().unwrap()));
    for g in ghosts {
        let name = readable_name(g).unwrap_or_else(|e| panic!("{}", e));
        let g = std::fs::canonicalize(g).unwrap_or_else(|e| panic!("{}: {}", g.display(), e));
        link(&g, &replays.join(name));
    }
}

/// Run every (map, ghosts) pair, at most `jobs` servers at a time.
/// Returns the rows per pair, in input order.
pub fn run_maps(pairs: &[(PathBuf, Vec<PathBuf>)], jobs: usize, server_dir: &str) -> Vec<Vec<Row>> {
    let server = std::fs::canonicalize(server_dir).unwrap();
    // unique per call: two batches running concurrently in one process (the
    // test binary does exactly that) must not share -- or wipe -- a work root.
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "tmmaps-run-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    let mut out: Vec<Vec<Row>> = vec![Vec::new(); pairs.len()];
    let mut running: Vec<(usize, std::process::Child)> = Vec::new();
    let jobs = jobs.max(1);
    for (k, (map, ghosts)) in pairs.iter().enumerate() {
        while running.len() >= jobs {
            let (i, ch) = running.remove(0);
            out[i] = finish(ch);
        }
        let dir = root.join(format!("m{:04}", k));
        stage(&dir, &server, map, ghosts);
        let child = Command::new("./TrackmaniaServer")
            .args(["/nodaemon", "/validatepath=."])
            .current_dir(&dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn TrackmaniaServer");
        running.push((k, child));
    }
    for (i, ch) in running {
        out[i] = finish(ch);
    }
    let _ = std::fs::remove_dir_all(&root);
    out
}

fn finish(ch: std::process::Child) -> Vec<Row> {
    let o = ch.wait_with_output().expect("wait");
    parse_output(&String::from_utf8_lossy(&o.stdout))
}

/// Parse the `{ "ValidatedResult" ... }` blocks the server prints.
pub fn parse_output(text: &str) -> Vec<Row> {
    let mut out = Vec::new();
    let mut cur_time: Option<i64> = None;
    let mut cur_cps: Option<u32> = None;
    let mut declared: Option<i64> = None;
    let mut in_validated = false;
    let mut in_declared = false;

    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("\"ValidatedResult\"") {
            in_declared = false;
            if t.contains("null") {
                cur_time = None;
            } else {
                in_validated = true;
            }
        } else if t.starts_with("\"DeclaredResult\"") {
            in_validated = false;
            in_declared = true;
        } else if t.starts_with("\"Time\"") {
            let v = t
                .split(':')
                .nth(1)
                .and_then(|s| s.trim().trim_end_matches(',').parse::<i64>().ok());
            if in_validated {
                cur_time = v;
                in_validated = false;
            } else if in_declared {
                declared = v;
                in_declared = false;
            }
        } else if t.starts_with("\"Desc\"") {
            if let Some(p) = t.find("reached some checkpoints (") {
                let rest = &t[p + "reached some checkpoints (".len()..];
                cur_cps = rest.split(' ').next().and_then(|s| s.trim().parse::<u32>().ok());
            } else if t.contains("wrong simu") {
                cur_cps = Some(0);
            }
        } else if t.starts_with("\"FileName\"") {
            let name = t
                .split(':')
                .nth(1)
                .map(|s| s.trim().trim_matches(|c| c == '"' || c == ',' || c == ' '))
                .unwrap_or("")
                .to_string();
            out.push(Row {
                file: name,
                sim_time: clean(cur_time),
                reached_cps: cur_cps,
                declared_time: declared,
            });
            cur_time = None;
            cur_cps = None;
            declared = None;
            in_validated = false;
            in_declared = false;
        }
    }
    out
}

/// Validate MANY ghosts against ONE map by sharding them across `jobs`
/// servers (segoracle.py's `Oracle.run`: the throughput path a search uses,
/// where one segment map is fixed and the candidates are the batch).
pub fn run_map_sharded(map: &Path, ghosts: &[PathBuf], jobs: usize, server_dir: &str) -> Vec<Row> {
    if ghosts.is_empty() {
        return Vec::new();
    }
    let n = jobs.max(1).min(ghosts.len());
    let mut shards: Vec<Vec<PathBuf>> = vec![Vec::new(); n];
    for (k, g) in ghosts.iter().enumerate() {
        shards[k % n].push(g.clone());
    }
    let pairs: Vec<(PathBuf, Vec<PathBuf>)> =
        shards.into_iter().map(|s| (map.to_path_buf(), s)).collect();
    run_maps(&pairs, n, server_dir).concat()
}

/// Convenience: {ghost file name -> time} for one (map, ghosts) pair.
pub fn times(rows: &[Row]) -> std::collections::HashMap<String, Option<i64>> {
    rows.iter().map(|r| (r.file.clone(), r.sim_time)).collect()
}
