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

/// Reasons the server gives for DECLINING to simulate a file. Each was
/// observed while establishing rung 0, and each is a container fault rather
/// than a fact about anyone's driving.
const REFUSAL_REASONS: &[&str] = &[
    "cannot validate scripted modes",
    "walltime not set",
    "unexcepted walltime",
    "known-flawed game exe",
    "is not compatible with the current",
    "no inputs available",
    "can't load",
];

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
    /// **This is read off the engine's own code, not guessed from the prose.**
    /// The server writes `wrong simu` at the site that reports a simulated run
    /// which collected too few checkpoints — the neighbouring branch is
    /// `wrong simu, but reached some checkpoints (%d out of %d)`. So a bare
    /// `wrong simu` means *simulated, reached zero checkpoints*: a DNF.
    ///
    /// A REFUSAL is different, and it looks different: the refusal reasons are
    /// appended to the same description, so `wrong simu` followed by
    /// `cannot validate scripted modes` or `walltime not set` is a file the
    /// engine declined to run. Those are container faults and reporting them as
    /// bad driving is how a broken container costs a week.
    pub fn simulated(&self) -> bool {
        if self.time_ms.is_some() {
            return true;
        }
        let d = self.desc.to_ascii_lowercase();
        if REFUSAL_REASONS.iter().any(|r| d.contains(r)) {
            return false;
        }
        d.contains("wrong simu") || d.contains("checkpoint") || d.contains("did not finish")
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

/// **The tape → verdict service.** This is what agents C and D call.
///
/// Hand it tapes; get back one [`Eval`] per tape, in the same order. It
/// synthesizes the containers, pools the dedicated server across this box's
/// cores, and re-associates every answer with the tape that produced it.
///
/// # Why the answers are `Option`
///
/// A tape whose file the server declined to run has **no verdict**, and this
/// function says so rather than inventing `Dnf { cps: 0 }`. Those two look
/// identical to a caller and they mean opposite things: one is a car that drove
/// badly, the other is a container fault. A search that treats a refusal as a
/// deep DNF will happily optimise toward broken containers.
///
/// # Batch size
///
/// Throughput peaks near 30 candidates per server launch and collapses well
/// beyond it (measured previously at 30/worker → 172 val/s, 100/worker →
/// 32 val/s). So this spawns MORE short-lived processes rather than longer
/// batches.
pub fn evaluate(
    map: &Path,
    tapes: &[Vec<crate::tape::Input>],
    min_ticks: usize,
    jobs: usize,
    workdir: &Path,
) -> Result<Vec<Option<Eval>>, String> {
    evaluate_tuned(map, tapes, min_ticks, jobs, DEFAULT_PER_LAUNCH, workdir)
}

/// [`evaluate`], with the **declared time** stated by the caller.
///
/// # Why this had to exist
///
/// `evaluate` builds its container from [`crate::synth::meta_for_map`], which
/// returns `GhostMeta::probe` — and `probe`'s `declared_ms` is **0**. The
/// validator simulates to the DECLARED time, not to the end of the tape, so a
/// declared 0 stops every candidate at race **2.500** however long its tape is.
///
/// That is invisible in a rung-0 probe, where every candidate is a DNF anyway
/// and the answer is the same either way. It is fatal the moment a candidate
/// could have finished: the run is cut off at 2.500 and comes back
/// `Dnf { cps: 0 }`, which reads as bad driving.
///
/// Measured, not inferred: the tape banked as
/// `summer01-finish-45573.artifact.tsv` finishes in **45.573** through this
/// function with `declared_ms = 70_000`, and comes back `Dnf { cps: 0 }`
/// through `evaluate`. Same tape, same map, same box, one field.
///
/// Pass a declared time past the longest run the search could plausibly
/// produce, exactly as `min_ticks` is chosen — the two are the same decision
/// made twice and they must agree, so this function checks them against each
/// other rather than trusting the caller.
pub fn evaluate_declared(
    map: &Path,
    tapes: &[Vec<crate::tape::Input>],
    min_ticks: usize,
    jobs: usize,
    per_launch: usize,
    declared_ms: u32,
    workdir: &Path,
) -> Result<Vec<Option<Eval>>, String> {
    if (min_ticks as u32) * 10 < declared_ms {
        return Err(format!(
            "min_ticks {} is {} ms of tape and the declared time is {} ms: the archive runs \
             out before the horizon, so a DNF would not be about driving. Raise min_ticks to \
             at least {}.",
            min_ticks,
            min_ticks * 10,
            declared_ms,
            (declared_ms as usize + 9) / 10
        ));
    }
    evaluate_inner(map, tapes, min_ticks, jobs, per_launch, Some(declared_ms), workdir)
}

/// How many candidates ride in one server launch.
///
/// **600, and the inherited value was 30.** The earlier rig recorded that
/// throughput "peaks at roughly 30 per invocation and collapses well beyond
/// that" (30/worker -> 172 val/s, 100/worker -> 32 val/s). That does not
/// reproduce here and it is not a small difference: on this box, going from 30
/// to 600 per launch takes 173 evals/s to 2192 evals/s.
///
/// The reason is visible once startup and per-eval are measured apart, which
/// the single-server sweep in `tmauto bench` does: a server launch costs
/// **2.68 s** and a marginal eval costs **~4.3 ms** on a 2000-tick tape. At 30
/// per launch you pay 2.68 s to do 130 ms of work. Nothing about that is
/// tuning; it is amortisation, and bigger is better until memory says stop.
///
/// `tmauto bench` re-measures it on THIS box, because a constant carried over
/// from another machine is a constant nobody re-derived -- which is exactly how
/// the 30 survived.
pub const DEFAULT_PER_LAUNCH: usize = 600;

/// [`evaluate`] with the batch size exposed, for the benchmark.
pub fn evaluate_tuned(
    map: &Path,
    tapes: &[Vec<crate::tape::Input>],
    min_ticks: usize,
    jobs: usize,
    per_launch: usize,
    workdir: &Path,
) -> Result<Vec<Option<Eval>>, String> {
    evaluate_inner(map, tapes, min_ticks, jobs, per_launch, None, workdir)
}

fn evaluate_inner(
    map: &Path,
    tapes: &[Vec<crate::tape::Input>],
    min_ticks: usize,
    jobs: usize,
    per_launch: usize,
    declared_ms: Option<u32>,
    workdir: &Path,
) -> Result<Vec<Option<Eval>>, String> {
    if tapes.is_empty() {
        return Ok(Vec::new());
    }
    let mut meta = crate::synth::meta_for_map(map)?;
    if let Some(d) = declared_ms {
        // set_declared, never a direct assignment: the walltime pair is checked
        // against the declared time and moving one without the other is the
        // "unexcepted walltime" refusal.
        meta.set_declared(d, vec![d as i32 / 2, d as i32]);
    }
    let _ = std::fs::create_dir_all(workdir);
    // Write every candidate first: a name per index, so an answer can never be
    // matched to the wrong tape.
    //
    // This loop is PARALLEL because it was measured to matter: at 6000
    // candidates of 9000 ticks it is ~320 MB of synthesis and I/O ahead of any
    // server work, and with the servers idle for all of it. `TMAUTO_TIMING=1`
    // prints the split so the attribution stays a measurement rather than a
    // story about where time goes.
    let t_write = std::time::Instant::now();
    let files: Vec<PathBuf> =
        (0..tapes.len()).map(|i| workdir.join(format!("cand{:07}.Ghost.Gbx", i))).collect();
    {
        let next = std::sync::atomic::AtomicUsize::new(0);
        let err: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
        let nthreads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8);
        std::thread::scope(|s| {
            for _ in 0..nthreads {
                let next = &next;
                let err = &err;
                let files = &files;
                let meta = &meta;
                s.spawn(move || loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= tapes.len() {
                        return;
                    }
                    let bytes = crate::synth::synthesize(
                        &crate::synth::pad_to(&tapes[i], min_ticks),
                        meta,
                        &crate::synth::ChunkSet::ALL,
                    );
                    if let Err(e) = std::fs::write(&files[i], &bytes) {
                        *err.lock().unwrap() = Some(format!("{}: {}", files[i].display(), e));
                    }
                });
            }
        });
        if let Some(e) = err.into_inner().unwrap() {
            return Err(e);
        }
    }
    let write_ms = t_write.elapsed().as_millis();
    let t_val = std::time::Instant::now();

    let batches: Vec<Vec<PathBuf>> =
        files.chunks(per_launch.max(1)).map(|c| c.to_vec()).collect();
    let jobs = jobs.max(1).min(batches.len().max(1));
    let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let results: std::sync::Arc<std::sync::Mutex<Vec<(String, Answer)>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let server = server_dir();

    std::thread::scope(|s| {
        for w in 0..jobs {
            let next = next.clone();
            let results = results.clone();
            let batches = &batches;
            let server = &server;
            s.spawn(move || loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= batches.len() {
                    return;
                }
                if let Ok(b) =
                    validate_raw(server, &batches[i], Maps::One(map), &format!("eval{}", w))
                {
                    let mut g = results.lock().unwrap();
                    for a in b.answers {
                        g.push((a.file.clone(), a));
                    }
                }
            });
        }
    });

    if std::env::var("TMAUTO_TIMING").is_ok() {
        eprintln!(
            "  [timing] {} candidates: synth+write {} ms, validate {} ms",
            tapes.len(),
            write_ms,
            t_val.elapsed().as_millis()
        );
    }
    let got = results.lock().unwrap();
    let by_name: std::collections::HashMap<&str, &Answer> =
        got.iter().map(|(n, a)| (n.as_str(), a)).collect();
    Ok(files
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            by_name.get(name).and_then(|a| a.eval())
        })
        .collect())
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

    /// The distinction that decides whether a container fault looks like a
    /// container fault or like bad driving — and it is two-sided on purpose.
    ///
    /// **This test previously asserted the opposite** and passed, because it
    /// encoded a guess: that `wrong simu` meant the server refused. Reading the
    /// engine showed `wrong simu` is written at the branch beside `wrong simu,
    /// but reached some checkpoints (%d out of %d)` — it is a simulated run
    /// that collected nothing. The refusals append their reason.
    #[test]
    fn a_bare_wrong_simu_is_a_dnf_and_a_reasoned_one_is_a_refusal() {
        let of = |desc: &str| {
            let text = format!(
                "\n \"ValidatedResult\": null,\n \"Desc\": \"{}\",\n \"FileName\": \"c.Ghost.Gbx\"\n",
                desc
            );
            parse_many(&text).remove(0)
        };

        // POSITIVE half: the engine ran the tape and the car reached nothing.
        let dnf = of("wrong simu");
        assert!(dnf.simulated(), "a bare `wrong simu` is a simulated run");
        assert_eq!(dnf.verdict(), Some(Verdict::Dnf { cps: 0 }));

        let some = of("wrong simu, but reached some checkpoints (2 out of 5)");
        assert!(some.simulated());
        assert_eq!(some.verdict(), Some(Verdict::Dnf { cps: 2 }));

        // NEGATIVE half: every refusal reason observed while establishing
        // rung 0. Each of these is a container fault and has NO verdict.
        for desc in [
            "wrong simu\\ncannot validate scripted modes (settings: 1)\\n",
            "wrong simu\\nwalltime not set\\n",
            "wrong simu\\nusing known-flawed game exe ''\\n",
            "unexcepted walltime (37s)",
            "Replay version TMr.6\\nis not compatible with the current\\ngame version TMr.8.",
            "No inputs available...",
            "Can't load ghost: c.Ghost.Gbx",
        ] {
            let a = of(desc);
            assert!(!a.simulated(), "must read as a REFUSAL: {}", desc);
            assert_eq!(a.verdict(), None, "a refused file has no verdict: {}", desc);
        }
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
