//! `tmauto` — the autopilot's oracle, provenance and container CLI.

use std::path::{Path, PathBuf};
use tmauto::oracle::{self, Maps};
use tmauto::synth::{self, ChunkSet, GhostMeta};
use tmauto::tape::Input;

fn usage() -> ! {
    eprintln!(
        r#"tmauto -- oracle, provenance and container layer

RUNG 0  (synthesizing a container with no human provenance)
  tmauto synth probe --map MAP.Map.Gbx [--ticks N] [--out DIR] [--raw]
        Synthesize a container from nothing and ask the dedicated server what
        it thinks of it. Prints the server's own transcript with --raw.
  tmauto synth write --map MAP.Map.Gbx --out FILE [--ticks N]
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
        ("synth", Some("matrix")) => cmd_synth_matrix(&args[2..]),
        ("synth", Some("write")) => cmd_synth_write(&args[2..]),
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
    let bytes = synth::synthesize(&inputs, &meta, &ChunkSet::ALL);
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
    let replay = synth::CLASS_CGAMECTNREPLAYRECORD;

    let variants: Vec<(&str, ChunkSet)> = vec![
        ("ghost/all/inline-ident", base.clone()),
        ("ghost/all/skip-ident", ChunkSet { identity_skippable: true, ..base.clone() }),
        ("ghost/tape-only", ChunkSet { class_id: ghost, ..ChunkSet::TAPE_ONLY }),
        (
            "ghost/no-login",
            ChunkSet { login: false, identity_skippable: true, ..base.clone() },
        ),
        (
            "ghost/no-uid",
            ChunkSet { validate_uid: false, identity_skippable: true, ..base.clone() },
        ),
        ("ghost/no-racetime", ChunkSet { racetime: false, identity_skippable: true, ..base.clone() }),
        ("ghost/no-result", ChunkSet { result: false, identity_skippable: true, ..base.clone() }),
        ("ghost/nodes-0", ChunkSet { num_nodes: 0, identity_skippable: true, ..base.clone() }),
        ("replay/all/skip-ident", ChunkSet { class_id: replay, identity_skippable: true, ..base.clone() }),
        ("replay/tape-only", ChunkSet { class_id: replay, ..ChunkSet::TAPE_ONLY }),
    ];

    println!("{:<28} {:>7}  {:>6}  {}", "variant", "bytes", "ghosts", "note");
    println!("{}", "-".repeat(72));
    for (name, set) in &variants {
        let bytes = synth::synthesize(&inputs, &meta, set);
        let p = dir.join(format!("{}.Ghost.Gbx", name.replace('/', "_")));
        std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
        let b = oracle::validate_raw(&oracle::server_dir(), &[p], Maps::One(&map), "matrix")?;
        let n = b.ghosts_found();
        let note = if b.answers.is_empty() {
            String::new()
        } else {
            format!("{} answer(s): {}", b.answers.len(), b.answers[0].desc)
        };
        println!(
            "{:<28} {:>7}  {:>6}  {}",
            name,
            bytes.len(),
            n.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
            note
        );
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
        inputs: true,
        result: !flag(args, "--no-result"),
        identity_skippable: flag(args, "--skip-ident"),
        class_id: if flag(args, "--replay") {
            synth::CLASS_CGAMECTNREPLAYRECORD
        } else {
            synth::CLASS_CGAMECTNGHOST
        },
        num_nodes: arg(args, "--nodes").and_then(|v| v.parse().ok()).unwrap_or(1),
    };
    let bytes = synth::synthesize(&full_gas(ticks), &meta, &set);
    std::fs::write(&out, &bytes).map_err(|e| e.to_string())?;
    println!("wrote {} ({} bytes)", out.display(), bytes.len());
    Ok(())
}
