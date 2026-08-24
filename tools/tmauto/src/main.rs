//! `tmauto` — the autopilot's oracle, provenance and container CLI.

use std::path::{Path, PathBuf};
use tmauto::oracle::{self, Maps};
use tmauto::synth::{self, ChunkSet, GhostMeta, UidEnc};
use tmauto::tape::Input;

mod artifact;
mod cpladder;
mod evalbug;
mod optimize;
mod pushgate;
mod splits;
mod tailsearch;
mod startprobe;

fn usage() -> ! {
    eprintln!(
        r#"tmauto -- oracle, provenance and container layer

RUNG 0  (synthesizing a container with no human provenance)
  tmauto synth probe --map MAP.Map.Gbx [--ticks N] [--out DIR] [--raw]
        Synthesize a container from nothing and ask the dedicated server what
        it thinks of it. Prints the server's own transcript with --raw.
  tmauto synth write --map MAP.Map.Gbx --out FILE [--ticks N] [--tape T.tsv]
                     [--declared MS] [--seed N] [--no-CHUNK ...]
        Write one synthesized container.

ORACLE
  tmauto verdict FILE... --map MAP.Map.Gbx
        Validate files through the gate and print verdicts.

GATE
  tmauto gate selftest --clean DIR --human FILE
        The two-sided test: a human recording must be REFUSED and one of our
        own chain-rooted tapes must be ACCEPTED, in the same run.

AUDIT
  tmauto audit fs --clean DIR
  tmauto audit reads --clean DIR
"#
    );
    std::process::exit(2)
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}
fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let r = match (args[0].as_str(), args.get(1).map(|s| s.as_str())) {
        ("synth", Some("probe")) => cmd_synth_probe(&args[2..]),
        ("synth", Some("respond")) => cmd_synth_respond(&args[2..]),
        ("synth", Some("reachcp")) => cmd_synth_reachcp(&args[2..]),
        ("bench", _) => cmd_bench(&args[1..]),
        ("synth", Some("matrix")) => cmd_synth_matrix(&args[2..]),
        ("synth", Some("write")) => cmd_synth_write(&args[2..]),
        ("startprobe", _) => startprobe::run(&args[1..]),
        ("cpladder", _) => cpladder::run(&args[1..]),
        ("tailsearch", _) => tailsearch::run(&args[1..]),
        ("artifact", _) => artifact::run(&args[1..]),
        ("optimize", _) => optimize::run(&args[1..]),
        ("evalbug", _) => evalbug::run(&args[1..]),
        ("splits", _) => splits::run(&args[1..]),
        ("pushgate", _) => pushgate::run(&args[1..]),
        _ => usage(),
    };
    if let Err(e) = r {
        eprintln!("tmauto: {}", e);
        std::process::exit(1);
    }
}

/// The map uid, read from the map file itself.
fn map_uid(map: &Path) -> Result<String, String> {
    let data = std::fs::read(map).map_err(|e| format!("{}: {}", map.display(), e))?;
    gbx::map_uid_of(&data).ok_or_else(|| {
        format!("{}: no map uid found -- this is a harness limit, not a fact about the file", map.display())
    })
}

/// A tape that just holds full throttle. The simplest thing that could possibly
/// move a car, and therefore the right first probe: if the server simulates it
/// at all, the container works, whatever the car then does.
fn full_gas(ticks: usize) -> Vec<Input> {
    vec![Input::FULL_GAS; ticks]
}

/// Read an input tape from the tab-separated form the explorer writes when it
/// banks a confirmed candidate: a `tick steer gas brake` header followed by one
/// row per tick.
///
/// Rows are placed by their own `tick` column rather than by file order, and
/// any tick the file does not mention is neutral — a tape whose rows are
/// reordered or sparse therefore produces the same container as the dense one,
/// which a positional read would not. Ticks are validated as strictly
/// increasing so a duplicated row cannot silently overwrite a different input.
fn tape_from_tsv(path: &std::path::Path) -> Result<Vec<Input>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows: Vec<(usize, Input)> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("tick") {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 {
            return Err(format!("{}:{}: expected 4 tab-separated columns, got {}", path.display(), n + 1, f.len()));
        }
        let num = |i: usize, what: &str| -> Result<i64, String> {
            f[i].parse::<i64>().map_err(|_| format!("{}:{}: {what} {:?} is not an integer", path.display(), n + 1, f[i]))
        };
        let tick = num(0, "tick")? as usize;
        let steer = num(1, "steer")?;
        if !(-128..=127).contains(&steer) {
            return Err(format!("{}:{}: steer {steer} is outside i8", path.display(), n + 1));
        }
        rows.push((tick, Input::new(steer as i8, num(2, "gas")? != 0, num(3, "brake")? != 0)));
    }
    if rows.is_empty() {
        return Err(format!("{}: no input rows", path.display()));
    }
    rows.sort_by_key(|(t, _)| *t);
    for w in rows.windows(2) {
        if w[0].0 == w[1].0 {
            return Err(format!("{}: tick {} appears more than once", path.display(), w[0].0));
        }
    }
    let last = rows.last().map(|(t, _)| *t).unwrap_or(0);
    let mut inputs = vec![Input::NEUTRAL; last + 1];
    for (t, i) in rows {
        inputs[t] = i;
    }
    Ok(inputs)
}

fn cmd_synth_probe(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let ticks: usize = arg(args, "--ticks").unwrap_or_else(|| "600".into()).parse().map_err(|_| "--ticks")?;
    let out = PathBuf::from(arg(args, "--out").unwrap_or_else(|| "/tmp/tmauto-probe".into()));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    let uid = map_uid(&map)?;
    println!("map      {}", map.display());
    println!("map uid  {}", uid);
    println!("ticks    {} ({} ms of tape)", ticks, ticks * 10);

    let inputs = full_gas(ticks);
    let meta = GhostMeta::probe(&uid);
    let set = ChunkSet {
        version_chunk: !flag(args, "--no-version"),
        validation: !flag(args, "--no-validation"),
        new_chunks_skippable: !flag(args, "--inline-new"),
        result: !flag(args, "--no-result"),
        racetime: !flag(args, "--no-racetime"),
        login: !flag(args, "--no-login"),
        ..ChunkSet::ALL
    };
    let bytes = synth::synthesize(&inputs, &meta, &set);
    let path = out.join("synth_probe.Ghost.Gbx");
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    println!("wrote    {} ({} bytes)", path.display(), bytes.len());

    let batch = oracle::validate_raw(&oracle::server_dir(), &[path], Maps::One(&map), "probe")?;
    if flag(args, "--raw") || batch.answers.is_empty() {
        println!("\n--- server transcript ---\n{}", batch.raw);
    }
    if batch.answers.is_empty() {
        return Err("the server reported NOTHING for this file -- it did not read it".into());
    }
    for a in &batch.answers {
        println!(
            "\nfile     {}\nsimulated {:?}\ncps      {:?}\ndeclared {:?}\nis_valid {:?}\ndesc     {}\nmap uid  {}\nlogin    {}\ninputs   {}",
            a.file, a.time_ms, a.cps, a.declared_ms, a.is_valid, a.desc, a.map_uid, a.login,
            if a.inputs.len() > 60 { format!("{}...", &a.inputs[..60]) } else { a.inputs.clone() }
        );
        match a.verdict() {
            Some(v) => println!("VERDICT  {}", v.secs()),
            None => println!("VERDICT  none -- the server did not simulate this file"),
        }
    }
    Ok(())
}

/// The rung-0 matrix: vary ONE thing about the container at a time and ask the
/// server what it thinks of each variant.
///
/// The observable is the server's own count line — `Starting validation of N
/// ghosts` — because a container the server declines to parse produces NOTHING
/// else: stdout is an empty JSON array, the validate log is empty, and no
/// message is printed. **A control established that the count cannot tell a
/// malformed container from an empty directory**: a file of eleven bytes of
/// ASCII garbage produces the identical output to a carefully built one. So the
/// count is only meaningful as a DIFFERENCE across variants, which is what this
/// command produces.
fn cmd_synth_matrix(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let ticks: usize =
        arg(args, "--ticks").unwrap_or_else(|| "600".into()).parse().map_err(|_| "--ticks")?;
    let uid = map_uid(&map)?;
    let dir = PathBuf::from("/tmp/tmauto-matrix");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let inputs = full_gas(ticks);
    let meta = GhostMeta::probe(&uid);

    let base = ChunkSet::ALL;
    let ghost = synth::CLASS_CGAMECTNGHOST;

    // The observable is the count line. It became meaningful the moment the
    // loader was shown to succeed: a file whose ghost loads but whose map uid
    // stays unresolved is skipped SILENTLY, so 0 vs 1 here is exactly the uid
    // question and nothing else.
    let mut variants: Vec<(String, ChunkSet, GhostMeta)> = Vec::new();
    let good = ChunkSet { class_id: ghost, uid_enc: UidEnc::IdWithVersion, ..base.clone() };
    for cv in [4u32, 5, 6, 7, 8, 9, 10] {
        for fv in [11u32] {
            let mut m = meta.clone();
            m.input_chunk_version = cv;
            m.format_version = fv;
            variants.push((format!("chunkver={} fmt={}", cv, fv), good.clone(), m));
        }
    }

    println!("{:<24} {:>7}  {:>6}  {}", "variant", "bytes", "ghosts", "note");
    println!("{}", "-".repeat(66));
    for (name, set, m) in &variants {
        let bytes = synth::synthesize(&inputs, m, set);
        let p = dir.join(format!("{}.Ghost.Gbx", name.replace(['/', ' ', '='], "_")));
        std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
        let b = oracle::validate_raw(&oracle::server_dir(), &[p], Maps::One(&map), "matrix")?;
        let n = b.ghosts_found();
        let note = if b.answers.is_empty() {
            String::new()
        } else {
            format!("{:?} | {}", b.answers[0].time_ms, b.answers[0].desc)
        };
        println!(
            "{:<24} {:>7}  {:>6}  {}",
            name,
            bytes.len(),
            n.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
            note
        );
    }
    Ok(())
}

/// **The control that a synthesized container actually carries our driving.**
///
/// Acceptance is not enough. A container the server reads, and then simulates
/// something that has nothing to do with the tape inside it, would pass every
/// "is the file valid?" check and be worthless — and this project has already
/// been burned by an instrument that returned a real-looking answer about a run
/// nobody asked for.
///
/// So the claim under test is not "the server accepts the file". It is:
///
/// > **different tapes in the same container shape produce different answers,
/// > and the differences are the ones the inputs imply.**
///
/// The batch below is built so that a container ignoring its tape produces one
/// repeated row, and a container conveying it cannot: the tapes differ in
/// length, in throttle, and in steering direction. The engine's own `Inputs`
/// echo is printed beside each verdict as a second, independent witness — it is
/// the engine's rendering of the tape it decoded, so it discriminates "our
/// bytes arrived" from "our bytes changed the physics" separately.
fn cmd_synth_respond(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let uid = map_uid(&map)?;
    let dir = PathBuf::from("/tmp/tmauto-respond");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let meta = GhostMeta::probe(&uid);

    let steer = |n: usize, s: i8, gas: bool, brake: bool| -> Vec<Input> {
        vec![Input { steer: s, gas, brake, respawn: false }; n]
    };
    let cases: Vec<(String, Vec<Input>)> = vec![
        ("nothing-500".into(), vec![Input::NEUTRAL; 500]),
        ("brake-500".into(), steer(500, 0, false, true)),
        ("gas-100".into(), steer(100, 0, true, false)),
        ("gas-500".into(), steer(500, 0, true, false)),
        ("gas-1500".into(), steer(1500, 0, true, false)),
        ("gas-3000".into(), steer(3000, 0, true, false)),
        ("gas-6000".into(), steer(6000, 0, true, false)),
        ("gas-left-3000".into(), steer(3000, -127, true, false)),
        ("gas-right-3000".into(), steer(3000, 127, true, false)),
    ];

    let mut files = Vec::new();
    for (name, inputs) in &cases {
        let bytes = synth::synthesize(inputs, &meta, &ChunkSet::ALL);
        let p = dir.join(format!("{}.Ghost.Gbx", name));
        std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
        files.push(p);
    }
    let b = oracle::validate_raw(&oracle::server_dir(), &files, Maps::One(&map), "respond")?;
    println!("ghosts the server read: {:?} of {}", b.ghosts_found(), cases.len());
    println!("\n{:<16} {:>6} {:>7}  {:<10} {}", "tape", "ticks", "inputs", "verdict", "server said");
    println!("{}", "-".repeat(88));
    let mut seen = std::collections::BTreeSet::new();
    for ((name, inputs), f) in cases.iter().zip(&files) {
        let a = b.by_name(f.file_name().unwrap().to_str().unwrap());
        match a {
            None => println!("{:<16} {:>6} {:>7}  {:<10} (the server never mentioned this file)", name, inputs.len(), "-", "-"),
            Some(a) => {
                seen.insert(format!("{}|{}", a.desc, a.inputs));
                println!(
                    "{:<16} {:>6} {:>7}  {:<10} {}",
                    name,
                    inputs.len(),
                    a.inputs,
                    a.verdict().map(|v| v.secs()).unwrap_or_else(|| "none".into()),
                    a.desc
                );
            }
        }
    }
    println!("\ndistinct (desc, input-echo) pairs: {} of {}", seen.len(), cases.len());
    if seen.len() <= 1 {
        println!(
            "REFUTED: every tape produced the same answer. A container that answers \
             identically for a 100-tick coast and a 6000-tick full-throttle run is not \
             carrying its tape."
        );
    }
    Ok(())
}

/// A tiny random search for a tape that reaches a checkpoint.
///
/// This is not the explorer — that is agent C's job and this is a hundred lines
/// of nothing clever. It exists as **the physics half of rung 0's control**:
/// the input echo already shows the engine decodes our tape, but an echo is
/// about bytes, not about a car. A tape that collects a checkpoint shows the
/// simulation is responding to our driving, and it does it with the server's
/// own sentence — `reached some checkpoints (N out of M)`.
///
/// Macro tapes: hold one `(steer, gas, brake)` for `k` ticks, repeat. The
/// alphabet is deliberately coarse.
fn cmd_synth_reachcp(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let uid = map_uid(&map)?;
    let rounds: usize =
        arg(args, "--rounds").unwrap_or_else(|| "8".into()).parse().map_err(|_| "--rounds")?;
    let batch: usize =
        arg(args, "--batch").unwrap_or_else(|| "60".into()).parse().map_err(|_| "--batch")?;
    let ticks: usize =
        arg(args, "--ticks").unwrap_or_else(|| "2000".into()).parse().map_err(|_| "--ticks")?;
    let macro_len: usize =
        arg(args, "--macro").unwrap_or_else(|| "25".into()).parse().map_err(|_| "--macro")?;
    let dir = PathBuf::from("/tmp/tmauto-reachcp");
    let meta = GhostMeta::probe(&uid);

    // A tiny deterministic PRNG, so a run that finds something can be repeated
    // exactly from its seed. A search whose result cannot be re-derived is a
    // result we are not allowed to keep.
    let mut state: u64 = arg(args, "--seed").and_then(|s| s.parse().ok()).unwrap_or(0x5EED_1234);
    let mut rng = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    let steer_ladder: [i8; 9] = [-127, -96, -64, -32, 0, 32, 64, 96, 127];
    let mut best: Option<(u32, String, Vec<Input>)> = None;

    for round in 0..rounds {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut files = Vec::new();
        let mut tapes = Vec::new();
        for c in 0..batch {
            let mut inputs = Vec::with_capacity(ticks);
            while inputs.len() < ticks {
                let r = rng();
                let s = steer_ladder[(r % 9) as usize];
                // Throttle is heavily biased on: a car that is not moving
                // reaches nothing, and an unbiased alphabet spends most of its
                // budget proving that.
                let gas = (r >> 8) % 10 != 0;
                let brake = (r >> 16) % 20 == 0;
                let n = macro_len.min(ticks - inputs.len());
                inputs.extend(std::iter::repeat(Input { steer: s, gas, brake, respawn: false }).take(n));
            }
            let bytes = synth::synthesize(&inputs, &meta, &ChunkSet::ALL);
            let p = dir.join(format!("r{}c{}.Ghost.Gbx", round, c));
            std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
            files.push(p);
            tapes.push(inputs);
        }
        let b = oracle::validate_raw(&oracle::server_dir(), &files, Maps::One(&map), "reachcp")?;
        let mut round_best = 0u32;
        for (t, f) in tapes.iter().zip(&files) {
            let a = match b.by_name(f.file_name().unwrap().to_str().unwrap()) {
                Some(a) => a,
                None => continue,
            };
            let cps = match a.verdict() {
                Some(v) => v.dnf_cps().unwrap_or(u32::MAX),
                None => continue,
            };
            round_best = round_best.max(if cps == u32::MAX { 999 } else { cps });
            if best.as_ref().map(|(c, _, _)| cps > *c).unwrap_or(true) {
                best = Some((cps, a.desc.clone(), t.clone()));
            }
        }
        println!(
            "round {:>2}: {} candidates, {} read, best cps this round = {}",
            round,
            batch,
            b.ghosts_found().unwrap_or(0),
            round_best
        );
    }
    match &best {
        Some((cps, desc, t)) => {
            println!("\nbest: cps={} over {} ticks\nserver said: {}", cps, t.len(), desc);
            if *cps > 0 {
                let bytes = synth::synthesize(t, &meta, &ChunkSet::ALL);
                let out = PathBuf::from("/tmp/tmauto-reachcp-best.Ghost.Gbx");
                std::fs::write(&out, &bytes).map_err(|e| e.to_string())?;
                println!("wrote {}", out.display());
            }
        }
        None => println!("\nno candidate was simulated at all"),
    }
    Ok(())
}

/// Measure the plain oracle's throughput on THIS box.
///
/// Reports evals/s and the per-eval cost broken into the two parts a caller can
/// actually act on: what a server launch costs regardless of how many
/// candidates ride in it, and what each extra candidate adds. Those two are
/// separated by measuring the same work at several batch sizes — a single
/// evals/s figure hides which one you are paying.
///
/// Load conditions are printed with the number, because a throughput measured
/// on a busy box is a number about the box.
fn cmd_bench(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let n: usize = arg(args, "--n").unwrap_or_else(|| "600".into()).parse().map_err(|_| "--n")?;
    let ticks: usize =
        arg(args, "--ticks").unwrap_or_else(|| "2000".into()).parse().map_err(|_| "--ticks")?;
    let dir = PathBuf::from("/tmp/tmauto-bench");

    println!("box: {} cpus", std::thread::available_parallelism().map(|v| v.get()).unwrap_or(0));
    if let Ok(l) = std::fs::read_to_string("/proc/loadavg") {
        println!("loadavg at start: {}", l.trim());
    }
    println!("map: {}   candidates: {}   tape: {} ticks ({} s of race)", map.display(), n, ticks, ticks / 100);

    // Distinct tapes, so nothing can be served from a cache keyed on content.
    let tapes: Vec<Vec<Input>> = (0..n)
        .map(|i| {
            let mut t = vec![Input::FULL_GAS; ticks];
            t[i % ticks].steer = (i % 251) as i8;
            t
        })
        .collect();

    let jobs_list: Vec<usize> = match arg(args, "--jobs") {
        Some(s) => s.split(',').filter_map(|v| v.parse().ok()).collect(),
        None => vec![20, 40, 80, 120],
    };
    let per_list: Vec<usize> = match arg(args, "--per-launch") {
        Some(s) => s.split(',').filter_map(|v| v.parse().ok()).collect(),
        None => vec![1, 5, 15, 30, 60],
    };

    println!("\n{:>6} {:>11} {:>10} {:>11} {:>9} {:>9}", "jobs", "per-launch", "wall s", "evals/s", "ms/eval", "answered");
    println!("{}", "-".repeat(64));
    let mut best = (0.0f64, 0usize, 0usize);
    for &jobs in &jobs_list {
        for &per in &per_list {
            let _ = std::fs::remove_dir_all(&dir);
            let t0 = std::time::Instant::now();
            let out = oracle::evaluate_tuned(&map, &tapes, ticks, jobs, per, &dir)?;
            let dt = t0.elapsed().as_secs_f64();
            let answered = out.iter().filter(|e| e.is_some()).count();
            let eps = n as f64 / dt;
            println!(
                "{:>6} {:>11} {:>10.2} {:>11.1} {:>9.2} {:>9}",
                jobs, per, dt, eps, 1000.0 * dt / n as f64, answered
            );
            // An answered count below n means the server dropped candidates.
            // A throughput measured over dropped work is not a throughput.
            if answered < n {
                println!("        ^ WARNING: {} of {} candidates went unanswered; this row is NOT a throughput", n - answered, n);
            } else if eps > best.0 {
                best = (eps, jobs, per);
            }
        }
    }
    if best.0 > 0.0 {
        println!("\nbest fully-answered: {:.1} evals/s at jobs={} per-launch={}", best.0, best.1, best.2);
    } else {
        println!("\nNO row answered every candidate -- UNMEASURED.");
    }
    if let Ok(l) = std::fs::read_to_string("/proc/loadavg") {
        println!("loadavg at end: {}", l.trim());
    }
    Ok(())
}

fn cmd_synth_write(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let out = PathBuf::from(arg(args, "--out").ok_or("--out is required")?);
    let ticks: usize = arg(args, "--ticks").unwrap_or_else(|| "600".into()).parse().map_err(|_| "--ticks")?;
    let uid = map_uid(&map)?;
    let mut meta = GhostMeta::probe(&uid);
    if let Some(d) = arg(args, "--declared") {
        meta.declared_ms = d.parse().map_err(|_| "--declared")?;
    }
    if let Some(s) = arg(args, "--seed") {
        meta.validation_seed = s.parse().map_err(|_| "--seed")?;
    }
    let set = ChunkSet {
        login: !flag(args, "--no-login"),
        validate_uid: !flag(args, "--no-uid"),
        racetime: !flag(args, "--no-racetime"),
        version_chunk: !flag(args, "--no-version"),
        validation: !flag(args, "--no-validation"),
        new_chunks_skippable: !flag(args, "--inline-new"),
        inputs: true,
        result: !flag(args, "--no-result"),
        identity_skippable: flag(args, "--skip-ident"),
        uid_enc: match arg(args, "--uid-enc").as_deref() {
            Some("plain") => UidEnc::PlainString,
            Some("id") => UidEnc::IdNoVersion,
            _ => UidEnc::IdWithVersion,
        },
        class_id: if flag(args, "--replay") {
            synth::CLASS_CGAMECTNREPLAYRECORD
        } else {
            synth::CLASS_CGAMECTNGHOST
        },
        num_nodes: arg(args, "--nodes").and_then(|v| v.parse().ok()).unwrap_or(1),
    };
    let inputs = match arg(args, "--tape") {
        Some(t) => tape_from_tsv(std::path::Path::new(&t))?,
        None => full_gas(ticks),
    };
    // The declared time governs how long the validator simulates -- not the
    // tape's length -- and `set_declared` is what also moves the walltime pair
    // with it. Setting `declared_ms` alone leaves a zero-length walltime and
    // the server refuses the file with "unexcepted walltime (0s)", which reads
    // as a bad drive rather than as a malformed container.
    if arg(args, "--tape").is_some() {
        let ms = match arg(args, "--declared") {
            Some(d) => d.parse().map_err(|_| "--declared")?,
            None => (inputs.len() as u32) * 10,
        };
        let ncp: i32 = arg(args, "--cps").and_then(|v| v.parse().ok()).unwrap_or(3);
        let cps: Vec<i32> = (1..=ncp).map(|i| (ms as i32 / (ncp + 1)) * i).chain(std::iter::once(ms as i32)).collect();
        meta.set_declared(ms, cps);
    }
    let bytes = synth::synthesize(&inputs, &meta, &set);
    std::fs::write(&out, &bytes).map_err(|e| e.to_string())?;
    println!(
        "wrote {} ({} bytes, {} ticks, declared {} ms)",
        out.display(),
        bytes.len(),
        inputs.len(),
        meta.declared_ms
    );
    Ok(())
}
