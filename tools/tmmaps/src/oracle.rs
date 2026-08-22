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

// The server's "never crossed" sentinel -- a huge u32 in a time field -- used
// to be cleaned here, by a `clean()` this module applied to every time it
// parsed. It moved into `ghost::oracle::sane_time` with the parser: a time of
// 4 294 967.295 s read as a finish is a bug in any caller of the shared reader,
// not just in map surgery.

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
///
/// **There is no parser here.** `ghost::oracle` reads the dedicated server's
/// output for the whole toolchain; this is the projection of its result onto
/// the four fields map surgery uses. The copy that used to live here was the
/// sixth in the tree, and it was written for the same reasons all six were.
///
/// Two behaviours were checked when they were merged, because a merge that
/// silently changes an answer is worse than the duplication:
///
/// * `cps = Some(0)` for `wrong simu` is kept HERE, not pushed into the shared
///   parser. It is a sentinel with a local meaning -- "it failed and the
///   message says nothing about how far it got" -- and `ghost::oracle` reports
///   the honest `None` for the same case, which is what a caller reading a
///   checkpoint count wants.
/// * the huge-u32 "never crossed" sentinel moved the OTHER way, into
///   `ghost::oracle::sane_time`, because a time of 4 294 967.295 s read as a
///   finish is a bug in any caller, not just this one.
pub fn parse_output(text: &str) -> Vec<Row> {
    ghost::oracle::parse_many(text)
        .into_iter()
        .map(|r| Row {
            reached_cps: match r.cps {
                Some(n) => Some(n),
                // `wrong simu` on its own: the run failed and the server said
                // nothing about the depth. On one map, 45 of 200 such runs had
                // driven up to 966 m of 1647 m -- so this 0 is a sentinel, and
                // reading it as a distance is the mistake it exists to name.
                None if r.desc.contains("wrong simu") => Some(0),
                None => None,
            },
            file: r.file,
            sim_time: r.time_ms,
            declared_time: r.declared_ms,
        })
        .collect()
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
