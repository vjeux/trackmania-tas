//! The plain oracle: the dedicated server, pooled across this box's cores.
//!
//! # Two things this driver does that a naive one does not
//!
//! **It keeps the raw transcript.** While rung 0 is being established, *why*
//! the server said something matters more than what it said, and a parser that
//! throws the text away leaves you guessing. [`Batch::raw`] is the server's
//! own stdout.
//!
//! **It goes through the gate.** Every file handed to the server is admitted by
//! [`crate::gate::Gate`] first, and a refusal is a hard error rather than a
//! skipped file — a batch that quietly validated three of four candidates and
//! reported three results is an instrument failing toward clean.

use crate::gate::{Decision, Gate};
use crate::verdict::{Eval, Verdict};
use std::path::{Path, PathBuf};
use std::process::Command;

static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// One file's answer, plus everything the server said about it.
#[derive(Clone, Debug, Default)]
pub struct Answer {
    pub file: String,
    /// What the server SIMULATED. `None` is a DNF **or** a refusal to
    /// simulate — [`Answer::desc`] distinguishes them and this driver never
    /// collapses the two.
    pub time_ms: Option<i64>,
    pub cps: Option<u32>,
    pub respawns: Option<u32>,
    /// What the FILE CLAIMS.
    pub declared_ms: Option<i64>,
    pub declared_cps: Option<u32>,
    pub is_valid: Option<bool>,
    pub desc: String,
    /// The engine's echo of the tape it decoded. An identity control that costs
    /// nothing: two files with the same tape produce the same string.
    pub inputs: String,
    pub account_id: String,
    pub login: String,
    pub map_uid: String,
    pub game_build: String,
}

impl Answer {
    /// The verdict, or `None` when the server did not simulate this file at
    /// all.
    ///
    /// **A file the server refused is not a DNF.** A DNF is a car that drove
    /// and did not finish; a refusal is the server declining to run. Returning
    /// `Dnf { cps: 0 }` for both is how a broken container looks like bad
    /// driving for a week.
    pub fn verdict(&self) -> Option<Verdict> {
        match self.time_ms {
            Some(t) if t >= 0 => Some(Verdict::finish(t as u32)),
            _ => {
                if self.simulated() {
                    Some(Verdict::Dnf { cps: self.cps.unwrap_or(0) })
                } else {
                    None
                }
            }
        }
    }

    /// Did the engine actually run this file?
    ///
    /// The server distinguishes "the car did not finish" from "I would not
    /// simulate this": the former reports checkpoint progress in its prose, the
    /// latter says so. Anything we cannot place is reported as NOT simulated,
    /// because the failure that costs is the one that reads as driving.
    pub fn simulated(&self) -> bool {
        if self.time_ms.is_some() {
            return true;
        }
        let d = self.desc.to_ascii_lowercase();
        d.contains("checkpoint") || d.contains("did not finish") || d.contains("not finished")
    }

    pub fn eval(&self) -> Option<Eval> {
        self.verdict().map(Eval::plain)
    }
}

/// A whole batch: the answers and the transcript they were parsed from.
pub struct Batch {
    pub answers: Vec<Answer>,
    /// The server's own stdout, verbatim.
    pub raw: String,
    /// The server's stderr, verbatim. **This is where the file count lives**
    /// (`Starting validation of N ghosts (in M maps)`), and a file the server
    /// declined to parse appears nowhere else at all: stdout is an empty JSON
    /// array and nothing is logged. Without this the only observable
    /// difference between "my container is malformed" and "the directory was
    /// empty" is nothing.
    pub err: String,
}

impl Batch {
    pub fn by_name(&self, name: &str) -> Option<&Answer> {
        self.answers.iter().find(|a| a.file.contains(name))
    }

    /// How many ghosts the server said it would validate, from its own count
    /// line. `None` when the line is absent, which is itself a finding rather
    /// than a zero.
    pub fn ghosts_found(&self) -> Option<u32> {
        let i = self.err.find("Starting validation of ")?;
        self.err[i + 23..].split_whitespace().next()?.parse().ok()
    }
}

/// Which map, if any, to link into `UserData/Maps`.
#[derive(Clone, Copy)]
pub enum Maps<'a> {
    /// Exactly one map linked in.
    One(&'a Path),
    /// No map at all. A file that still validates carried its own.
    None,
}

pub fn server_dir() -> PathBuf {
    std::env::var("TM_SERVER")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/tmoracle/server"))
}

fn link(target: &Path, at: &Path) {
    if std::fs::symlink_metadata(at).is_err() {
        let _ = std::os::unix::fs::symlink(target, at);
    }
}

/// The name a file must be linked under for the server to read it at all.
///
/// **The server only looks at files with the right extension.** A candidate
/// called `out.try3` is not read, and the result is indistinguishable from a
/// run that did not finish.
fn server_name(orig: &str) -> String {
    if orig.ends_with(".Ghost.Gbx") || orig.ends_with(".Replay.Gbx") {
        orig.to_string()
    } else {
        format!("{}.Ghost.Gbx", orig.replace(['/', ' '], "_"))
    }
}

/// Validate a batch, WITHOUT the gate. Only the gate's own two-sided test and
/// the rung-0 probes may call this; everything else goes through
/// [`validate_gated`].
pub fn validate_raw(
    server: &Path,
    files: &[PathBuf],
    maps: Maps,
    tag: &str,
) -> Result<Batch, String> {
    let bin = server.join("TrackmaniaServer");
    if !bin.exists() {
        return Err(format!("no dedicated server at {}", server.display()));
    }
    if files.is_empty() {
        return Ok(Batch { answers: Vec::new(), raw: String::new(), err: String::new() });
    }
    let root = std::env::temp_dir().join(format!(
        "tmauto-oracle-{}-{}-{}",
        std::process::id(),
        tag,
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    let replays = root.join("UserData").join("Replays");
    let mapsdir = root.join("UserData").join("Maps");
    std::fs::create_dir_all(&replays).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&mapsdir).map_err(|e| e.to_string())?;
    link(&server.join("Packs"), &root.join("Packs"));
    link(&bin, &root.join("TrackmaniaServer"));
    if let Maps::One(m) = maps {
        let abs = std::fs::canonicalize(m).map_err(|e| format!("{}: {}", m.display(), e))?;
        link(&abs, &mapsdir.join(abs.file_name().unwrap()));
    }
    let mut names: Vec<String> = Vec::with_capacity(files.len());
    for (i, g) in files.iter().enumerate() {
        let abs = std::fs::canonicalize(g).map_err(|e| format!("{}: {}", g.display(), e))?;
        let base = server_name(abs.file_name().unwrap().to_string_lossy().as_ref());
        let name = if names.contains(&base) { format!("{}_{}", i, base) } else { base };
        link(&abs, &replays.join(&name));
        names.push(name);
    }
    let out = Command::new("./TrackmaniaServer")
        .args(["/nodaemon", "/validatepath=."])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("launching the server: {}", e))?;
    let raw = String::from_utf8_lossy(&out.stdout).to_string();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&root);
    Ok(Batch { answers: parse_many(&raw), raw, err })
}

/// Validate a batch, every file admitted by the gate first.
pub fn validate_gated(
    gate: &Gate,
    server: &Path,
    files: &[PathBuf],
    maps: Maps,
    tag: &str,
) -> Result<Batch, String> {
    for f in files {
        if let Decision::Refuse(r) = gate.admit(f) {
            return Err(format!("GATE REFUSED {}: {}", f.display(), r.reason()));
        }
    }
    validate_raw(server, files, maps, tag)
}

#[derive(PartialEq, Clone, Copy)]
enum Block {
    None,
    Validated,
    Declared,
}

/// Parse a whole transcript. One record per `"FileName"`.
///
/// **The server prints two results per file and the second is the file's own
/// claim.** A parser that takes `"Time"` lines as they come reports the file's
/// declaration as the world's answer. This one tracks which block it is inside.
pub fn parse_many(text: &str) -> Vec<Answer> {
    let mut out = Vec::new();
    let mut cur = Answer::default();
    let mut block = Block::None;
    let field = |t: &str| -> String {
        t.splitn(2, ':')
            .nth(1)
            .map(|s| s.trim().trim_end_matches(',').trim_matches('"').to_string())
            .unwrap_or_default()
    };
    let numf = |t: &str| -> Option<i64> {
        t.splitn(2, ':').nth(1).and_then(|s| s.trim().trim_end_matches(',').parse::<i64>().ok())
    };
    // The server's "never crossed the line" sentinel, read as a value would be
    // a finish at 4 294 967.295 seconds: a DNF reported as the best run ever.
    const BAD_TIME_MS: i64 = 4_294_967_000;
    let sane = |v: Option<i64>| match v {
        Some(t) if t > BAD_TIME_MS || t < 0 => None,
        other => other,
    };
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("\"ValidatedResult\"") {
            block = if t.contains("null") { Block::None } else { Block::Validated };
        } else if t.starts_with("\"DeclaredResult\"") {
            block = if t.contains("null") { Block::None } else { Block::Declared };
        } else if t.starts_with("\"Time\"") {
            match block {
                Block::Validated => cur.time_ms = sane(numf(t)),
                Block::Declared => cur.declared_ms = sane(numf(t)),
                Block::None => {}
            }
        } else if t.starts_with("\"NbCheckpoints\"") {
            match block {
                Block::Validated => cur.cps = numf(t).map(|v| v as u32),
                Block::Declared => cur.declared_cps = numf(t).map(|v| v as u32),
                Block::None => {}
            }
        } else if t.starts_with("\"NbRespawns\"") {
            if block == Block::Validated {
                cur.respawns = numf(t).map(|v| v as u32);
            }
        } else if t.starts_with("\"IsValid\"") {
            cur.is_valid = Some(t.contains("true"));
            block = Block::None;
        } else if t.starts_with("\"Desc\"") {
            cur.desc = field(t).replace("\\n", " ").trim().to_string();
            block = Block::None;
        } else if t.starts_with("\"Inputs\"") {
            cur.inputs = field(t);
        } else if t.starts_with("\"AccountId\"") {
            cur.account_id = field(t);
        } else if t.starts_with("\"Login\"") {
            cur.login = field(t);
        } else if t.starts_with("\"GameBuild\"") {
            cur.game_build = field(t);
        } else if t.starts_with("\"MapUid\"") {
            cur.map_uid = field(t);
        } else if t.starts_with("\"FileName\"") {
            cur.file = field(t);
            if cur.cps.is_none() {
                cur.cps = cps_from_desc(&cur.desc);
            }
            out.push(std::mem::take(&mut cur));
            block = Block::None;
        }
    }
    out
}

/// The checkpoint count the server mentions in its own prose. On a DNF there is
/// no `ValidatedResult`, so `NbCheckpoints` never appears — but the `Desc` says
/// `reached some checkpoints (2 out of 5)`.
fn cps_from_desc(desc: &str) -> Option<u32> {
    let i = desc.find("checkpoint")?;
    let rest = &desc[i..];
    let open = rest.find('(')?;
    let close = rest[open..].find(')')? + open;
    rest[open + 1..close].split_whitespace().next().and_then(|v| v.parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The parser must keep the two result blocks apart. This transcript
    /// declares one time and simulates another, which is exactly the case a
    /// naive parser gets backwards.
    #[test]
    fn declared_and_validated_do_not_get_swapped() {
        let text = r#"
 "ValidatedResult": {
   "Time": 22738,
   "NbCheckpoints": 3,
   "NbRespawns": 0
 },
 "DeclaredResult": {
   "Time": 22730,
   "NbCheckpoints": 3
 },
 "IsValid": false,
 "Desc": "race finished, time is worse.",
 "FileName": "cand.Ghost.Gbx"
"#;
        let a = &parse_many(text)[0];
        assert_eq!(a.time_ms, Some(22738), "the SIMULATED time");
        assert_eq!(a.declared_ms, Some(22730), "the DECLARED time");
        assert_eq!(a.verdict(), Some(Verdict::finish(22738)));
    }

    /// A DNF: no validated block, checkpoint progress only in the prose.
    #[test]
    fn a_dnf_keeps_its_checkpoint_count() {
        let text = r#"
 "ValidatedResult": null,
 "Desc": "reached some checkpoints (2 out of 5)",
 "FileName": "cand.Ghost.Gbx"
"#;
        let a = &parse_many(text)[0];
        assert!(a.simulated(), "the engine ran this one");
        assert_eq!(a.verdict(), Some(Verdict::Dnf { cps: 2 }));
    }

    /// A REFUSAL is not a DNF. This is the distinction that decides whether a
    /// container problem looks like a container problem or like bad driving.
    #[test]
    fn a_refusal_is_not_a_dnf() {
        let text = r#"
 "ValidatedResult": null,
 "Desc": "wrong simu",
 "FileName": "cand.Ghost.Gbx"
"#;
        let a = &parse_many(text)[0];
        assert!(!a.simulated());
        assert_eq!(a.verdict(), None, "a file the server would not run has no verdict");
    }

    #[test]
    fn the_sentinel_is_not_a_finish() {
        let text = r#"
 "ValidatedResult": {
   "Time": 4294967295
 },
 "Desc": "x",
 "FileName": "c.Ghost.Gbx"
"#;
        assert_eq!(parse_many(text)[0].time_ms, None);
    }
}
