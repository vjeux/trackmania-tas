//! The `tmhaul` command line.

use haul::*;
use std::path::PathBuf;

fn usage() -> &'static str {
    "tmhaul — the long-haul harness for the TM2020 autopilot

  The repo is the state of record. Every command below reads and writes
  `autopilot/` inside the checkout; none of them depend on an agent's context.

SETTING UP
  init                       scaffold autopilot/ in this repo (idempotent)
  config get KEY             print one setting from autopilot/config/job.rec

RUNNING
  watch [--detach]           supervise the job: sample, alarm, bank, rotate
       [--lease-expires ISO] when this box's lease ends (drives the stand-down)
       [--max-passes N] [--note TEXT]
  stop                       ask a running supervisor to stand down cleanly
  beat                       what a woken session should read and then do
  selftest-worker            a controllable workload, for demos and alarm tests
       --mode normal|stall|slow|flat|silent|crash --rate N --duration S
       [--switch-after S] [--progress FILE] [--tick-ms N]

STATE
  status [--write]           render the human status page
  journal add --kind K [--f k=v ...] | journal tail [-n N]
  ledger add --what W --config C --produced P --why WHY
       [--claim measured|inferred|unknown|superseded] [--control TEXT]
  ledger list
  queue push --kind K --payload P [--priority N] [--id ID]
  queue claim [--ttl S] | queue complete --id ID [--outcome TEXT]
  queue list | queue reap
  budget show | budget record --evals N --dt S
  lease show

DURABILITY
  bank [--why TEXT]          manifest, commit, mirror, push — with a receipt
  verify                     re-hash every banked file against MANIFEST.md5
  mirror latest | mirror restore
  recover                    take over a run from the repo + newest mirror

ALARMS
  alarms eval                what is firing right now
  claim                      submit a worker result through the acceptance gates
  gates                      watch every gate refuse, here, now
  alarms selftest            fire every alarm from its fixture, here, now
  alarms live-test           fire alarms against real processes on this box
"
}

struct Args {
    words: Vec<String>,
    flags: std::collections::BTreeMap<String, String>,
    multi: Vec<(String, String)>,
}

fn parse_args() -> Args {
    let mut words = Vec::new();
    let mut flags = std::collections::BTreeMap::new();
    let mut multi = Vec::new();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        let short = a.len() == 2 && a.starts_with('-') && !a.starts_with("--");
        if let Some(name) = a.strip_prefix("--").or(if short { a.strip_prefix('-') } else { None }) {
            let (k, v) = match name.split_once('=') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => {
                    let next = argv.get(i + 1);
                    match next {
                        Some(v) if !v.starts_with("--") => {
                            i += 1;
                            (name.to_string(), v.clone())
                        }
                        _ => (name.to_string(), "1".to_string()),
                    }
                }
            };
            if k == "f" {
                if let Some((fk, fv)) = v.split_once('=') {
                    multi.push((fk.to_string(), fv.to_string()));
                }
            } else {
                flags.insert(k, v);
            }
        } else {
            words.push(a.clone());
        }
        i += 1;
    }
    Args { words, flags, multi }
}

impl Args {
    fn word(&self, n: usize) -> &str {
        self.words.get(n).map(String::as_str).unwrap_or("")
    }
    fn flag(&self, k: &str) -> Option<&str> {
        self.flags.get(k).map(String::as_str)
    }
    fn s(&self, k: &str, d: &str) -> String {
        self.flag(k).unwrap_or(d).to_string()
    }
    fn i(&self, k: &str, d: i64) -> i64 {
        self.flag(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    fn on(&self, k: &str) -> bool {
        self.flag(k).map(|v| v != "0").unwrap_or(false)
    }
}

fn layout() -> Result<paths::Layout, String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    if let Ok(r) = std::env::var("TMHAUL_REPO") {
        return Ok(paths::Layout::new(r));
    }
    paths::Layout::discover(&here)
        .ok_or_else(|| format!("no git checkout at or above {}; set TMHAUL_REPO", here.display()))
}

fn job(l: &paths::Layout) -> Result<config::Job, String> {
    if l.job_spec().exists() {
        config::Job::load(&l.job_spec())
    } else {
        Ok(config::Job::default())
    }
}

fn main() {
    match real_main() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("tmhaul: {e}");
            std::process::exit(1);
        }
    }
}

fn real_main() -> Result<i32, String> {
    let a = parse_args();
    let now = time::now();
    match a.word(0) {
        "" | "help" | "-h" | "--help" => {
            println!("{}", usage());
            Ok(0)
        }

        "init" => {
            let l = layout()?;
            for d in l.all_dirs() {
                std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
                let keep = d.join(".gitkeep");
                if !keep.exists() {
                    std::fs::write(&keep, "").map_err(|e| e.to_string())?;
                }
            }
            if !l.job_spec().exists() {
                std::fs::write(l.job_spec(), config::Job::default_text()).map_err(|e| e.to_string())?;
            }
            if !l.ops_log().exists() {
                std::fs::write(l.ops_log(), OPS_LOG_SEED).map_err(|e| e.to_string())?;
            }
            println!("initialised {}", l.root().display());
            Ok(0)
        }

        "config" => {
            let l = layout()?;
            let j = job(&l)?;
            let key = a.word(1);
            let v = match key {
                "get" => {
                    let k = a.word(2);
                    match k {
                        "progress_file" => j.progress_file,
                        "worker_cmd" => j.worker_cmd,
                        "worker_dir" => j.worker_dir,
                        "branch" => j.branch,
                        "map_name" => j.map_name,
                        "rung" => j.rung,
                        "push" => watch::resolve_push(&j.push),
                        other => j.extra.get(other).cloned().unwrap_or_default(),
                    }
                }
                _ => return Err("usage: tmhaul config get KEY".into()),
            };
            println!("{v}");
            Ok(0)
        }

        "status" => {
            let l = layout()?;
            let j = job(&l)?;
            let page = status::render(&l, &j, now)?;
            if a.on("write") {
                std::fs::write(l.status_page(), &page).map_err(|e| e.to_string())?;
                println!("wrote {}", l.status_page().display());
            } else {
                print!("{page}");
            }
            Ok(0)
        }

        "journal" => {
            let l = layout()?;
            match a.word(1) {
                "add" => {
                    let node = paths::node_id();
                    let lg = log::Log::shard(&l.journal_dir(), &node, now).map_err(|e| e.to_string())?;
                    let mut r = rec::Rec::new(&a.s("kind", "note"));
                    for (k, v) in &a.multi {
                        r.set(k, v);
                    }
                    if let Some(n) = a.flag("note") {
                        r.set("note", n);
                    }
                    lg.append(&r).map_err(|e| e.to_string())?;
                    println!("{}", r.render());
                    Ok(0)
                }
                "tail" => {
                    let recs = log::read_all(&l.journal_dir())?;
                    let n = a.i("n", 30).max(1) as usize;
                    for r in recs.iter().rev().take(n).rev() {
                        println!("{}", r.render());
                    }
                    Ok(0)
                }
                _ => Err("usage: tmhaul journal add|tail".into()),
            }
        }

        "ledger" => {
            let l = layout()?;
            match a.word(1) {
                "add" => {
                    let e = ledger::Entry {
                        ts: now,
                        what: a.s("what", ""),
                        config: a.s("config", ""),
                        produced: a.s("produced", ""),
                        why: a.s("why", ""),
                        claim: ledger::Claim::parse(&a.s("claim", "unknown"))
                            .unwrap_or(ledger::Claim::Unknown),
                        control: a.s("control", ""),
                        node: paths::node_id(),
                    };
                    if e.what.is_empty() || e.why.is_empty() {
                        return Err("a ledger entry needs at least --what and --why: the why is the part worth reading in three months".into());
                    }
                    ledger::add(&l, &paths::node_id(), now, &e)?;
                    println!("recorded: {} [{}]", e.what, e.claim.as_str());
                    Ok(0)
                }
                "list" => {
                    for e in ledger::all(&l)? {
                        println!(
                            "{}  [{}] {}\n    config: {}\n    produced: {}\n    why: {}",
                            time::iso(e.ts),
                            e.claim.as_str(),
                            e.what,
                            e.config,
                            e.produced,
                            e.why.replace('\n', "\n          ")
                        );
                    }
                    Ok(0)
                }
                _ => Err("usage: tmhaul ledger add|list".into()),
            }
        }

        "queue" => {
            let l = layout()?;
            let j = job(&l)?;
            let q = queue::Queue::open(&l).map_err(|e| e.to_string())?;
            match a.word(1) {
                "push" => {
                    let kind = a.s("kind", "work");
                    let payload = a.s("payload", "");
                    let id = a.flag("id").map(|s| s.to_string())
                        .unwrap_or_else(|| queue::Queue::derive_id(&kind, &payload));
                    let it = queue::Item::new(&id, &kind, &payload, a.i("priority", 0));
                    println!("{}", if q.push(&it)? { format!("pushed {id}") } else { format!("{id} already present") });
                    Ok(0)
                }
                "claim" => {
                    match q.claim(&paths::node_id(), a.i("ttl", j.claim_ttl_s))? {
                        Some(it) => {
                            println!("{}\t{}\t{}", it.id, it.kind, it.payload);
                            Ok(0)
                        }
                        None => {
                            println!("(queue empty)");
                            Ok(1)
                        }
                    }
                }
                "complete" => {
                    let id = a.s("id", "");
                    let ok = q.complete(&id, &a.s("outcome", "done"))?;
                    println!("{}", if ok { "completed" } else { "no such claimed item" });
                    Ok(if ok { 0 } else { 1 })
                }
                "list" => {
                    for (label, items) in
                        [("pending", q.pending()?), ("claimed", q.claimed()?), ("done", q.done()?)]
                    {
                        for it in items {
                            println!(
                                "{label}\t{}\t{}\t{}\tattempts={}{}",
                                it.id,
                                it.kind,
                                it.payload,
                                it.attempts,
                                it.claim_expires
                                    .map(|e| format!("\texpires={}", time::iso(e)))
                                    .unwrap_or_default()
                            );
                        }
                    }
                    Ok(0)
                }
                "reap" => {
                    let r = q.reap(now)?;
                    if r.is_empty() {
                        println!("nothing to reap");
                    } else {
                        for x in &r {
                            println!("reaped {x}");
                        }
                    }
                    Ok(0)
                }
                _ => Err("usage: tmhaul queue push|claim|complete|list|reap".into()),
            }
        }

        "budget" => {
            let l = layout()?;
            let j = job(&l)?;
            match a.word(1) {
                "correct" => {
                    budget::correct(
                        &l.budget_dir(),
                        &paths::node_id(),
                        a.i("evals", 0).max(0) as u64,
                        a.i("productive-s", 0),
                        &a.s("why", ""),
                    )
                    .map_err(|e| e.to_string())?;
                    println!("correction recorded");
                    Ok(0)
                }
                "record" => {
                    budget::record(&l.budget_dir(), &paths::node_id(), a.i("evals", 0) as u64, a.i("dt", 0))
                        .map_err(|e| e.to_string())?;
                    Ok(0)
                }
                _ => {
                    let c = budget::total(&l.budget_dir())?;
                    println!(
                        "evals {} of {}\nproductive {} of {}\nstalled {} (does not spend the budget)\nspent {:.1}%\nswitch reached: {}",
                        c.evals,
                        j.budget.switch_evals,
                        time::dur(c.productive_s),
                        time::dur(j.budget.switch_productive_s),
                        time::dur(c.stalled_s),
                        100.0 * c.spent_fraction(&j.budget),
                        c.switch_reached(&j.budget)
                    );
                    Ok(0)
                }
            }
        }

        "lease" => {
            let l = layout()?;
            // Retiring a box that VANISHED. A clean stand-down retires itself;
            // a box whose lease was reclaimed underneath it never got the
            // chance, and would otherwise sit ACTIVE in the registry forever —
            // firing box_vanished on every heartbeat and counting against the
            // fleet ceiling. Found by the first real rotation.
            if a.word(1) == "retire" {
                let node = a.s("node", "");
                if node.is_empty() {
                    return Err("tmhaul lease retire --node NODE --why TEXT".into());
                }
                let known: Vec<String> = lease::all(&l)?.into_iter().map(|b| b.node).collect();
                if !known.iter().any(|n| n == &node) {
                    // Refuse a name the registry has never seen: a typo would
                    // otherwise write a retirement for a box that never
                    // existed and silently leave the real one active.
                    return Err(format!(
                        "no box named {node:?} in the registry. Known: {}",
                        known.join(", ")
                    ));
                }
                lease::retire(&l, &node, &a.s("why", "retired by hand"))?;
                println!("retired {node}");
                return Ok(0);
            }
            for b in lease::all(&l)? {
                println!(
                    "{}\t{}\tlease={}\tlast_seen={}\t{}",
                    b.node,
                    if b.retired { "retired" } else { "ACTIVE" },
                    b.lease_expires.map(time::iso).unwrap_or_else(|| "-".into()),
                    time::iso(b.last_seen),
                    b.note
                );
            }
            Ok(0)
        }

        "bank" => {
            let l = layout()?;
            let j = job(&l)?;
            let node = paths::node_id();
            let o = bank::Options {
                message: format!("autopilot: {} ({node}, {})", a.s("why", "manual"), time::iso(now)),
                mirror: bank::mirror_from_str(&j.mirror),
                mirror_dir: if j.mirror_dir.is_empty() { None } else { Some(PathBuf::from(&j.mirror_dir)) },
                push: bank::push_from_str(&watch::resolve_push(&a.s("push", &j.push))),
                branch: j.branch.clone(),
            };
            let r = bank::bank(&l, &node, &o)?;
            let lg = log::Log::shard(&l.journal_dir(), &node, now).map_err(|e| e.to_string())?;
            lg.append(&rec::Rec::new("bank").f("why", a.s("why", "manual")).f("receipt", r.summary()))
                .map_err(|e| e.to_string())?;
            println!("{}", r.summary());
            Ok(if r.mirror_error.is_some() || r.push_error.is_some() { 1 } else { 0 })
        }

        "claim" => {
            // The acceptance gates, as the only route by which a worker's
            // result becomes a banked result. A refusal exits non-zero and
            // writes NOTHING to the frontier: the harness records that a
            // claim was refused and why, which is itself a fact worth having.
            let l = layout()?;
            let c = gates::Claim {
                what: a.s("what", ""),
                tape_md5: a.s("tape-md5", ""),
                frame_start_tick: a.flag("frame-start-tick").and_then(|v| v.parse().ok()),
                prefix: match (a.flag("prefix-tape-md5"), a.flag("prefix-at-tick")) {
                    (Some(m), Some(t)) => Some(gates::Prefix {
                        tape_md5: m.to_string(),
                        at_tick: t.parse().unwrap_or(-1),
                    }),
                    _ => None,
                },
                map_md5: a.s("map-md5", ""),
                template_md5: a.s("template-md5", ""),
                live_tick0: match (a.flag("tick0-x"), a.flag("tick0-y"), a.flag("tick0-z")) {
                    (Some(x), Some(y), Some(z)) => Some(gates::Tick0 {
                        x: x.parse().unwrap_or(f64::NAN),
                        y: y.parse().unwrap_or(f64::NAN),
                        z: z.parse().unwrap_or(f64::NAN),
                        dev_from_spawn_m: a.flag("start-dev-m").and_then(|v| v.parse().ok()),
                    }),
                    _ => None,
                },
                drives: !a.on("no-drive"),
                oracle_transcript: match a.flag("transcript-file") {
                    Some(p) => Some(std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?),
                    None => a.flag("transcript").map(|s| s.to_string()),
                },
            };
            let refusals = gates::evaluate(&c, &gates::Policy::default());
            let node = paths::node_id();
            let lg = log::Log::shard(&l.journal_dir(), &node, now).map_err(|e| e.to_string())?;
            if refusals.is_empty() {
                let claims = log::Log::at(l.frontier().join(format!("claims-{node}.rec")));
                claims.append(&gates::to_rec(&c)).map_err(|e| e.to_string())?;
                if let Some(t) = &c.oracle_transcript {
                    // The engine's own bytes, beside the claim, so a later
                    // reader can re-judge it without re-running anything.
                    let d = l.frontier().join("transcripts");
                    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
                    std::fs::write(d.join(format!("{}.txt", md5::md5_hex(t.as_bytes()))), t)
                        .map_err(|e| e.to_string())?;
                }
                lg.append(&rec::Rec::new("claim_accepted").f("what", &c.what).f("tape_md5", &c.tape_md5))
                    .map_err(|e| e.to_string())?;
                println!("accepted: {}", c.what);
                Ok(0)
            } else {
                for r in &refusals {
                    println!("REFUSED [{}] {}", r.gate, r.why);
                    lg.append(
                        &rec::Rec::new("claim_refused")
                            .f("what", &c.what)
                            .f("gate", r.gate)
                            .f("why", &r.why),
                    )
                    .map_err(|e| e.to_string())?;
                }
                Ok(4)
            }
        }

        "gates" => {
            // Watch every gate refuse, here, now — the same discipline the
            // alarms get. A gate nobody has seen refuse is decoration.
            let p = gates::Policy::default();
            let good = gates::Claim {
                what: "a complete claim".into(),
                tape_md5: "a".repeat(32),
                frame_start_tick: Some(0),
                prefix: None,
                map_md5: "b".repeat(32),
                template_md5: "c".repeat(32),
                live_tick0: Some(gates::Tick0 { x: 1584.2, y: 16.0, z: 783.4, dev_from_spawn_m: Some(0.9) }),
                drives: true,
                oracle_transcript: Some("TAS.Ghost.Gbx  IsValid=1  Time=23144  Checkpoints=3  Respawns=0".into()),
            };
            let cases: Vec<(&str, gates::Claim)> = vec![
                ("no frame", gates::Claim { frame_start_tick: None, ..good.clone() }),
                ("no map hash", gates::Claim { map_md5: String::new(), ..good.clone() }),
                ("no live tick 0", gates::Claim { live_tick0: None, ..good.clone() }),
                (
                    "started at a checkpoint",
                    gates::Claim {
                        live_tick0: Some(gates::Tick0 { x: 1359.5, y: 10.0, z: 1103.0, dev_from_spawn_m: Some(390.0) }),
                        ..good.clone()
                    },
                ),
                (
                    "no start control",
                    gates::Claim {
                        live_tick0: Some(gates::Tick0 { x: 0.0, y: 0.0, z: 0.0, dev_from_spawn_m: None }),
                        ..good.clone()
                    },
                ),
                ("no transcript", gates::Claim { oracle_transcript: None, ..good.clone() }),
                ("an empty transcript", gates::Claim { oracle_transcript: Some(String::new()), ..good.clone() }),
            ];
            let mut broken = 0;
            println!("{:<26} {}", "claim", "refused by");
            for (name, c) in &cases {
                let rs = gates::evaluate(c, &p);
                if rs.is_empty() {
                    broken += 1;
                    println!("{name:<26} NOTHING  <-- BROKEN");
                } else {
                    println!("{name:<26} {}", rs.iter().map(|r| r.gate).collect::<Vec<_>>().join(", "));
                }
            }
            let clean = gates::evaluate(&good, &p);
            println!("{:<26} {}", "(control) a good claim", if clean.is_empty() { "accepted".into() } else { format!("REFUSED <-- BROKEN: {:?}", clean) });
            if !clean.is_empty() {
                broken += 1;
            }
            println!("\n{} gate(s), {broken} broken", gates::GATES.len());
            Ok(if broken == 0 { 0 } else { 2 })
        }

        "verify" => {
            let l = layout()?;
            // "Banked" means the bytes git has. The working tree legitimately
            // runs ahead while a supervisor is running — the journal gains a
            // record the instant banking finishes — so checking it would fail
            // on a healthy run and teach everyone to ignore this command.
            let src = if a.on("working-tree") { bank::Source::WorkingTree } else { bank::Source::Committed };
            let bad = bank::verify(&l, src)?;
            if bad.is_empty() {
                let n = std::fs::read_to_string(l.manifest()).map_err(|e| e.to_string())?.lines().count();
                println!(
                    "{n} file(s) verified against MANIFEST.md5 ({})",
                    if src == bank::Source::Committed { "as committed" } else { "working tree" }
                );
                Ok(0)
            } else {
                for b in &bad {
                    println!("MISMATCH {b}");
                }
                if src == bank::Source::WorkingTree && beat::watch_pid().is_some() {
                    println!(
                        "\nA supervisor is running on this box, so the working tree is EXPECTED to be\n\
                         ahead of the manifest. Run `tmhaul verify` without --working-tree, or\n\
                         `tmhaul stop` first."
                    );
                }
                Ok(2)
            }
        }

        "mirror" => match a.word(1) {
            "latest" => {
                match bank::latest_mirror()? {
                    Some((id, title)) => println!("{id}\t{title}"),
                    None => println!("(no mirror found)"),
                }
                Ok(0)
            }
            "restore" => {
                let l = layout()?;
                let rep = recover::recover(&l)?;
                println!("{rep:#?}");
                Ok(0)
            }
            _ => Err("usage: tmhaul mirror latest|restore".into()),
        },

        "recover" => {
            let l = layout()?;
            let j = job(&l)?;
            println!("recovering into {}", l.repo.display());
            match recover::pull(&l, &j.branch) {
                Ok(o) => println!("  git: {}", o.lines().last().unwrap_or("up to date")),
                Err(e) => println!("  git pull failed (continuing from the mirror): {e}"),
            }
            let rep = recover::recover(&l)?;
            println!(
                "  mirror: {}\n  {} files seen · {} written · {} merged (+{} records) · {} identical",
                rep.source, rep.files_seen, rep.files_written, rep.files_merged, rep.records_added, rep.identical
            );
            for c in &rep.conflicts {
                println!("  CONFLICT {c}");
            }
            let q = queue::Queue::open(&l).map_err(|e| e.to_string())?;
            for x in q.reap(now)? {
                println!("  reaped {x}");
            }
            let b = beat::brief(&l, &j, now, beat::watch_pid())?;
            println!("\n{}", b.text);
            Ok(0)
        }

        "beat" => {
            let l = layout()?;
            let j = job(&l)?;
            let b = beat::brief(&l, &j, now, beat::watch_pid())?;
            print!("{}", b.text);
            Ok(if b.critical { 3 } else { 0 })
        }

        "stop" => {
            let l = layout()?;
            let j = job(&l)?;
            let p = PathBuf::from(&j.progress_file).with_file_name("STOP");
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            std::fs::write(&p, format!("{}\n", time::iso(now))).map_err(|e| e.to_string())?;
            println!("asked the supervisor to stand down: {}", p.display());
            let _ = l;
            Ok(0)
        }

        "watch" => {
            let l = layout()?;
            let j = job(&l)?;
            if j.worker_cmd.trim().is_empty() {
                return Err(format!(
                    "no worker_cmd in {} — the supervisor will not invent work to do",
                    l.job_spec().display()
                ));
            }
            // Two supervisors on one box is not a configuration anybody wants,
            // and it is easy to reach by accident: `watch` clears the STOP
            // file at startup, so starting a second one *while the first is
            // standing down* deletes the flag the first was about to read and
            // leaves both alive. Refuse instead.
            if !a.on("force") {
                if let Some(pid) = beat::watch_pid() {
                    return Err(format!(
                        "a supervisor is already running on this box (pid {pid}). \
                         Run `tmhaul stop` and wait for it to stand down, or pass --force."
                    ));
                }
            }
            let stop = PathBuf::from(&j.progress_file).with_file_name("STOP");
            let _ = std::fs::remove_file(&stop);

            if a.on("detach") {
                // Liveness must not depend on this session staying awake.
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                let logp = PathBuf::from(&j.progress_file).with_file_name("watch.log");
                if let Some(d) = logp.parent() {
                    std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
                }
                let out = std::fs::OpenOptions::new().create(true).append(true).open(&logp)
                    .map_err(|e| e.to_string())?;
                let err = out.try_clone().map_err(|e| e.to_string())?;
                let mut cmd = std::process::Command::new("setsid");
                cmd.arg(&exe).arg("watch");
                if let Some(x) = a.flag("lease-expires") {
                    cmd.arg("--lease-expires").arg(x);
                }
                if let Some(x) = a.flag("note") {
                    cmd.arg("--note").arg(x);
                }
                if let Some(x) = a.flag("max-passes") {
                    cmd.arg("--max-passes").arg(x);
                }
                cmd.env("TMHAUL_REPO", &l.repo)
                    .current_dir(&l.repo)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::from(out))
                    .stderr(std::process::Stdio::from(err));
                let child = cmd.spawn().map_err(|e| format!("spawn setsid: {e}"))?;
                println!("supervisor detached, pid {} — log {}", child.id(), logp.display());
                return Ok(0);
            }

            let lease_expires = a.flag("lease-expires").and_then(time::parse_iso);
            let o = watch::Options {
                node: paths::node_id(),
                lease_expires,
                max_passes: a.i("max-passes", 0).max(0) as u64,
                note: a.s("note", ""),
            };
            let mut sup = watch::Supervisor::new(l, j, &o)?;
            sup.run(&o)
        }

        "selftest-worker" => {
            let progress = a.s(
                "progress",
                &std::env::var("TMHAUL_PROGRESS").unwrap_or_else(|_| "/tmp/tmhaul/progress.rec".into()),
            );
            let mode = worker::mode_from_str(&a.s("mode", "normal"))
                .ok_or("--mode must be normal|stall|slow|flat|silent|crash")?;
            let code = worker::run(&worker::Opts {
                progress: PathBuf::from(progress),
                rate: a.i("rate", 100).max(0) as u64,
                tick_ms: a.i("tick-ms", 200).max(10) as u64,
                duration_s: a.i("duration", 60),
                mode,
                switch_after_s: a.i("switch-after", 0),
            });
            Ok(code)
        }

        "alarms" => {
            let l = layout().ok();
            match a.word(1) {
                "eval" => {
                    let l = l.ok_or("alarms eval needs a checkout")?;
                    let j = job(&l)?;
                    let fired = state::alarm_state(&l, now, &j.alarms)?;
                    if fired.is_empty() {
                        println!("nothing firing");
                    }
                    for f in &fired {
                        println!("{:?}\t{}\t{}", f.severity, f.id, f.detail);
                    }
                    Ok(if fired.is_empty() { 0 } else { 3 })
                }
                "selftest" => {
                    // Fire every alarm from its fixture, here, now. An alarm
                    // nobody has seen fire is decoration.
                    let cfg = alarms::Config::default();
                    let mut failures = 0;
                    println!("{:<22} {:<44} {}", "alarm", "state", "fired?");
                    for (id, why, v) in alarms::fixtures::firing_cases() {
                        let fired = alarms::evaluate(&v, &cfg).iter().any(|f| f.id == id);
                        if !fired {
                            failures += 1;
                        }
                        println!("{id:<22} {why:<44} {}", if fired { "YES" } else { "NO  <-- BROKEN" });
                    }
                    let healthy = alarms::evaluate(&alarms::fixtures::healthy(), &cfg);
                    println!("{:<22} {:<44} {}", "(control)", "a healthy run", if healthy.is_empty() { "silent" } else { "FIRED <-- BROKEN" });
                    if !healthy.is_empty() {
                        failures += 1;
                        for f in &healthy {
                            println!("    unexpected: {} — {}", f.id, f.detail);
                        }
                    }
                    println!("\n{} alarm(s), {failures} broken", alarms::ALL.len());
                    Ok(if failures == 0 { 0 } else { 2 })
                }
                "live-test" => live_test(),
                _ => Err("usage: tmhaul alarms eval|selftest|live-test".into()),
            }
        }

        other => Err(format!("unknown command {other:?}\n\n{}", usage())),
    }
}

/// Fire alarms against **real processes and real files** on this box.
///
/// The fixtures prove the predicates; this proves the plumbing between a live
/// worker, the progress file, the journal and the alarm evaluator. They are
/// different claims and this project has been burned by treating one as the
/// other.
fn live_test() -> Result<i32, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let root = std::env::temp_dir().join(format!("tmhaul-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).map_err(|e| e.to_string())?;
    gitcmd::git(&repo, &["init", "-q"])?;
    gitcmd::git(&repo, &["config", "user.email", "tmhaul@localhost"])?;
    gitcmd::git(&repo, &["config", "user.name", "tmhaul live-test"])?;
    let l = paths::Layout::new(&repo);
    for d in l.all_dirs() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }

    let mut fails = 0;
    for (mode, want, switch_after) in [
        ("normal", None, 0),
        ("stall", Some("zero_throughput"), 3),
        ("crash", Some("worker_died"), 3),
        ("silent", Some("zero_throughput"), 0),
    ] {
        let progress = root.join(format!("{mode}/progress.rec"));
        std::fs::create_dir_all(progress.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut j = config::Job {
            worker_cmd: format!(
                "{} selftest-worker --mode {mode} --rate 500 --tick-ms 200 --duration 30 --switch-after {switch_after}",
                exe.display()
            ),
            worker_dir: root.to_string_lossy().to_string(),
            progress_file: progress.to_string_lossy().to_string(),
            sample_s: 1,
            bank_s: 100_000,
            mirror: "none".into(),
            push: "none".into(),
            restart_max: 0,
            ..config::Job::default()
        };
        j.alarms.zero_window_s = 4;
        j.alarms.bank_max_gap_s = 100_000;

        // A separate state tree per mode, so one run's journal cannot answer
        // for another's.
        let sub = paths::Layout::new(repo.join(mode));
        for d in sub.all_dirs() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        gitcmd::git(&repo, &["add", "-A"]).ok();

        let o = watch::Options {
            node: format!("live-{mode}"),
            lease_expires: None,
            max_passes: 9,
            note: "live-test".into(),
        };
        let mut sup = watch::Supervisor::new(sub.clone(), j.clone(), &o)?;
        let _ = sup.run(&o)?;

        let recorded = log::read_all(&sub.alarm_dir())?;
        let ids: Vec<String> = recorded
            .iter()
            .filter(|r| r.kind == "alarm")
            .filter_map(|r| r.get("id").map(|s| s.to_string()))
            .collect();
        let ok = match want {
            Some(w) => ids.iter().any(|i| i == w),
            None => ids.is_empty(),
        };
        if !ok {
            fails += 1;
        }
        println!(
            "{:<8} worker → expected {:<18} recorded {:<40} {}",
            mode,
            want.unwrap_or("no alarm at all"),
            format!("{ids:?}"),
            if ok { "OK" } else { "FAILED" }
        );
    }
    println!(
        "\nlive-test: {}",
        if fails == 0 {
            "every alarm fired against a real process, and the firing is on disk"
        } else {
            "SOME ALARMS DID NOT FIRE"
        }
    );
    let _ = std::fs::remove_dir_all(&root);
    Ok(if fails == 0 { 0 } else { 2 })
}

const OPS_LOG_SEED: &str = r#"# Operations log

Recurring failures and their fixes, so a future session does not rediscover
them. Newest at the top. Each entry: **what broke**, **how it presented**,
**the fix**. If it cost more than ten minutes, it belongs here.

---

## `INFINITY` through a comparison makes every test false

A comparison against `INFINITY` is false in both directions, so a filter built
on one accepts nothing and a "best so far" seeded with one is never beaten —
and the code returns whatever came first, with no error anywhere. Seed with an
`Option` and let the type system make the empty case explicit.

## A constant steer channel makes the shim lock onto the wrong memory

The shim finds the input array by looking for the bytes it expects to be
changing. Drive with a steer value that never changes and it locks onto some
other region that happens to match, then reports confidently about it.
Vary the channel, or pin the array by identity rather than by content.

## The validator simulates until the *declared* time, not until the tape ends

A tape longer than its declared result is truncated silently; a tape shorter
than it runs past its own end. Either way the verdict is about a run nobody
asked for. Always set the declared time from the tape you are actually
submitting.

## cargo does not inherit the shell proxy

`https_proxy` in the environment gets the crates.io *index* nowhere: cargo
retries three times and dies on a random crate, which reads as a flaky network.
Write `~/.cargo/config.toml` with `[http] proxy` and `[net] git-fetch-with-cli`.
`SETUP.md` has the exact file.

## Clone into /tmp, never into ~/persistent

A clone into the persistent mount dies with `premature end of pack file` at a
different byte count every time. Three agents have tuned postBuffer, HTTP/2 and
`--filter` chasing it. Work in `/tmp`; bank to persistent and to the repo.
"#;
