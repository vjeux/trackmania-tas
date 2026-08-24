//! `tmauto artifact` — a banked run that carries **everything needed to rebuild
//! itself**, and a replay path that refuses anything less.
//!
//! # The defect this exists to close
//!
//! The explorer banked `confirmed_*.tape.tsv` files holding four columns and
//! nothing else. They omit the **frame** — the search's tick 0 is file tick
//! `prefix`, not file tick 0 — and a one-tick shift of a whole tape has already
//! turned a confirmed `cps 3` into `cps 0` in this project. It measured here
//! too: of the offsets {0, 60, 74, 100, 150, 152, 153, 154, 155, 200, 300},
//! only **153** reproduces `cps 3`; 152 and 154 both give 1. A file that does
//! not say which one it meant is not a result, it is a riddle.
//!
//! # What is in the header, and why each field is there
//!
//! | field | why it must be present |
//! |---|---|
//! | `map_uid`, `map_sha256` | a tape is only a run *against a map*. The replay hashes the map it was handed and refuses a different one. |
//! | `container_ticks` | the archive length. Two containers of different length are different runs — measured: they are not, on this tape, but that is a measurement and not a licence to drop the field. |
//! | `prefix` | the frame. See above. |
//! | `declared_ms`, `declared_cps` | the validator simulates to the DECLARED time, so it bounds the run; and the authored checkpoint count is a **reporting filter** (measured in `cpladder --matrix`: a larger count suppresses the reported checkpoints). Both change what you see. |
//! | `template` | how the ticks outside the tape were filled. They are NOT neutral. |
//! | `tape_sha256` | the input array, so a corrupted body is caught before it is simulated. |
//! | `file_sha256` | the container bytes that were actually validated, so "reconstructed identically" is checkable rather than asserted. |
//! | `producer`, `parent` | the provenance chain the no-ghost gate walks. |
//!
//! **The rows are the FULL container input array**, not just the searched
//! segment. That is the difference between an artifact that can be rebuilt and
//! one that needs a formula somebody has to remember: the template, the
//! prefix and the padding are all recorded as provenance, and none of them is
//! *needed* to reproduce the bytes.
//!
//! # The round trip
//!
//! `artifact write` synthesizes the container, records the hash of the bytes it
//! validated, and writes the artifact. `artifact replay` — in a **fresh
//! process**, from the artifact alone — rebuilds the container, requires the
//! rebuilt bytes to hash to the recorded value, and only then hands it to the
//! plain oracle. A missing field is a refusal, not a default.

use std::path::{Path, PathBuf};
use tmauto::oracle::{self, Maps};
use tmauto::synth::{self, ChunkSet, GhostMeta};
use tmauto::tape::Input;

use crate::cpladder::{read_tape, template_inputs};

pub const MAGIC: &str = "#tmauto-artifact 1";

fn sha256_hex(b: &[u8]) -> String {
    tmauto::sha::sha256_hex(b)
}

fn secs(ms: u32) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// The header, as parsed. Every field is required; there is no `Default` and no
/// `unwrap_or`, deliberately — a container whose declared time was defaulted is
/// exactly the defect this project keeps paying for.
pub struct Header {
    pub map_uid: String,
    pub map_sha256: String,
    pub container_ticks: usize,
    pub prefix: usize,
    pub declared_ms: u32,
    pub declared_cps: Vec<i32>,
    pub template: String,
    pub tape_sha256: String,
    pub file_sha256: String,
    pub producer: String,
    pub parent: String,
}

/// A byte encoding of the input array, so `tape_sha256` means one exact thing.
fn encode(inputs: &[Input]) -> Vec<u8> {
    let mut v = Vec::with_capacity(inputs.len() * 4);
    for i in inputs {
        v.push(i.steer as u8);
        v.push(i.gas as u8);
        v.push(i.brake as u8);
        v.push(i.respawn as u8);
    }
    v
}

/// Rebuild the container from a header plus the full input array. This is the
/// ONE place that turns an artifact into bytes, so `write` and `replay` cannot
/// drift apart.
fn build(h: &Header, inputs: &[Input]) -> Vec<u8> {
    let mut meta = GhostMeta::probe(&h.map_uid);
    meta.set_declared(h.declared_ms, h.declared_cps.clone());
    synth::synthesize(inputs, &meta, &ChunkSet::ALL)
}

fn need<'a>(m: &'a std::collections::HashMap<String, String>, k: &str) -> Result<&'a str, String> {
    m.get(k).map(|s| s.as_str()).ok_or_else(|| {
        format!(
            "the artifact has no `{}` field. Every field is required: a replay that defaulted \
             one would be a different run wearing this run's name. Refusing.",
            k
        )
    })
}

pub fn read_artifact(p: &Path) -> Result<(Header, Vec<Input>), String> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
    if !text.starts_with(MAGIC) {
        return Err(format!("{}: not a tmauto artifact (no `{}` first line)", p.display(), MAGIC));
    }
    let mut m = std::collections::HashMap::new();
    let mut inputs = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix('#') {
            let mut it = rest.splitn(2, ' ');
            if let (Some(k), Some(v)) = (it.next(), it.next()) {
                m.insert(k.to_string(), v.trim().to_string());
            }
            continue;
        }
        let line = line.trim();
        if line.is_empty() || line.starts_with("tick") {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 5 {
            return Err(format!("{}: an input row needs 5 columns (tick steer gas brake respawn)", p.display()));
        }
        let tick: usize = f[0].parse().map_err(|_| "bad tick")?;
        if tick != inputs.len() {
            return Err(format!(
                "{}: tick column says {} at row {}. The array has a gap; refusing rather than \
                 compacting it into a different run.",
                p.display(),
                tick,
                inputs.len()
            ));
        }
        inputs.push(Input {
            steer: f[1].parse().map_err(|_| "bad steer")?,
            gas: f[2] != "0",
            brake: f[3] != "0",
            respawn: f[4] != "0",
        });
    }
    let h = Header {
        map_uid: need(&m, "map_uid")?.to_string(),
        map_sha256: need(&m, "map_sha256")?.to_string(),
        container_ticks: need(&m, "container_ticks")?.parse().map_err(|_| "container_ticks")?,
        prefix: need(&m, "prefix")?.parse().map_err(|_| "prefix")?,
        declared_ms: need(&m, "declared_ms")?.parse().map_err(|_| "declared_ms")?,
        declared_cps: {
            let s = need(&m, "declared_cps")?;
            if s.is_empty() {
                Vec::new()
            } else {
                s.split(',').map(|x| x.trim().parse().map_err(|_| "declared_cps")).collect::<Result<_, _>>()?
            }
        },
        template: need(&m, "template")?.to_string(),
        tape_sha256: need(&m, "tape_sha256")?.to_string(),
        file_sha256: need(&m, "file_sha256")?.to_string(),
        producer: need(&m, "producer")?.to_string(),
        parent: need(&m, "parent")?.to_string(),
    };
    if inputs.len() != h.container_ticks {
        return Err(format!(
            "{}: the header says {} ticks and the body has {}. Refusing.",
            p.display(),
            h.container_ticks,
            inputs.len()
        ));
    }
    Ok((h, inputs))
}

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(|s| s.as_str()) {
        Some("write") => write_cmd(&args[1..]),
        Some("replay") => replay_cmd(&args[1..]),
        _ => Err("usage: tmauto artifact write|replay ...".into()),
    }
}

fn write_cmd(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let tape_path = PathBuf::from(arg(args, "--tape").ok_or("--tape is required")?);
    let prefix: usize = arg(args, "--prefix").ok_or("--prefix is required")?.parse().map_err(|_| "--prefix")?;
    let ticks: usize = arg(args, "--ticks").ok_or("--ticks is required")?.parse().map_err(|_| "--ticks")?;
    let declared: u32 = arg(args, "--declared").ok_or("--declared is required")?.parse().map_err(|_| "--declared")?;
    let cut: usize = arg(args, "--cut").ok_or("--cut is required")?.parse().map_err(|_| "--cut")?;
    let steer: i8 = arg(args, "--steer").ok_or("--steer is required")?.parse().map_err(|_| "--steer")?;
    let out = PathBuf::from(arg(args, "--out").ok_or("--out is required")?);
    let note = arg(args, "--note").unwrap_or_else(|| "tailsearch constant-macro continuation".into());

    let map_bytes = std::fs::read(&map).map_err(|e| format!("{}: {}", map.display(), e))?;
    let uid = crate::map_uid(&map)?;
    let tape = read_tape(&tape_path)?;

    let mut inputs = template_inputs(ticks);
    for (i, t) in tape.iter().take(cut).enumerate() {
        if prefix + i < ticks {
            inputs[prefix + i] = *t;
        }
    }
    for slot in inputs.iter_mut().skip(prefix + cut) {
        *slot = Input { steer, gas: true, brake: false, respawn: false };
    }

    let declared_cps: Vec<i32> = vec![declared as i32 / 2, declared as i32];
    let h = Header {
        map_uid: uid.clone(),
        map_sha256: sha256_hex(&map_bytes),
        container_ticks: ticks,
        prefix,
        declared_ms: declared,
        declared_cps: declared_cps.clone(),
        template: format!("wobble7919 gas=on ticks={}", ticks),
        tape_sha256: sha256_hex(&encode(&inputs)),
        file_sha256: String::new(),
        producer: "tmauto tailsearch".into(),
        parent: sha256_hex(&std::fs::read(&tape_path).map_err(|e| e.to_string())?),
    };
    let bytes = build(&h, &inputs);
    let file_sha = sha256_hex(&bytes);

    let mut s = String::new();
    s.push_str(MAGIC);
    s.push('\n');
    s.push_str(&format!("#map_uid {}\n", h.map_uid));
    s.push_str(&format!("#map_sha256 {}\n", h.map_sha256));
    s.push_str(&format!("#container_ticks {}\n", h.container_ticks));
    s.push_str(&format!("#prefix {}\n", h.prefix));
    s.push_str(&format!("#declared_ms {}\n", h.declared_ms));
    s.push_str(&format!(
        "#declared_cps {}\n",
        declared_cps.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")
    ));
    s.push_str(&format!("#template {}\n", h.template));
    s.push_str(&format!("#tape_sha256 {}\n", h.tape_sha256));
    s.push_str(&format!("#file_sha256 {}\n", file_sha));
    s.push_str(&format!("#producer {}\n", h.producer));
    s.push_str(&format!("#parent {}\n", h.parent));
    s.push_str(&format!("#parent_path {}\n", tape_path.display()));
    s.push_str(&format!("#cut {}\n", cut));
    s.push_str(&format!("#macro_steer {}\n", steer));
    s.push_str(&format!("#note {}\n", note));
    s.push_str("tick\tsteer\tgas\tbrake\trespawn\n");
    for (i, t) in inputs.iter().enumerate() {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            i,
            t.steer,
            t.gas as u8,
            t.brake as u8,
            t.respawn as u8
        ));
    }
    std::fs::write(&out, s).map_err(|e| e.to_string())?;
    println!("wrote {}", out.display());
    println!("  map_uid        {}", h.map_uid);
    println!("  map_sha256     {}", h.map_sha256);
    println!("  container      {} ticks, prefix {}", ticks, prefix);
    println!("  declared       {}", secs(declared));
    println!("  tape_sha256    {}", h.tape_sha256);
    println!("  file_sha256    {}", file_sha);
    println!("\nnow verify it in a FRESH PROCESS:  tmauto artifact replay --artifact {} --map {}", out.display(), map.display());
    Ok(())
}

fn replay_cmd(args: &[String]) -> Result<(), String> {
    let apath = PathBuf::from(arg(args, "--artifact").ok_or("--artifact is required")?);
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let out = PathBuf::from(arg(args, "--out").unwrap_or_else(|| "/tmp/c2/replay".into()));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    let (h, inputs) = read_artifact(&apath)?;
    println!("ARTIFACT {}", apath.display());
    println!("  producer {}   parent {}", h.producer, &h.parent[..16.min(h.parent.len())]);

    // ---- the map is part of the run ----
    let map_bytes = std::fs::read(&map).map_err(|e| format!("{}: {}", map.display(), e))?;
    let got = sha256_hex(&map_bytes);
    if got != h.map_sha256 {
        return Err(format!(
            "MAP MISMATCH: the artifact was produced against sha256 {} and this file is {}. \
             A tape is only a run against a map; refusing.",
            h.map_sha256, got
        ));
    }
    println!("  PASS  the map hashes to the value the artifact records");

    // ---- the body is what the header says it is ----
    let tape_sha = sha256_hex(&encode(&inputs));
    if tape_sha != h.tape_sha256 {
        return Err(format!(
            "TAPE MISMATCH: body hashes to {} and the header says {}. Refusing.",
            tape_sha, h.tape_sha256
        ));
    }
    println!("  PASS  the input array hashes to the value the artifact records");

    // ---- byte-identical reconstruction ----
    let bytes = build(&h, &inputs);
    let file_sha = sha256_hex(&bytes);
    if file_sha != h.file_sha256 {
        return Err(format!(
            "RECONSTRUCTION MISMATCH: rebuilding this artifact produced a container hashing to \
             {}, and the artifact says the validated container hashed to {}. The bytes the \
             server saw are not the bytes this artifact rebuilds. Refusing.",
            file_sha, h.file_sha256
        ));
    }
    println!("  PASS  the container rebuilds BYTE-IDENTICALLY ({}…)", &file_sha[..16]);

    let p = out.join(format!("replay_{}.Ghost.Gbx", &file_sha[..12]));
    std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;

    // ---- and the negative half: a deliberately perturbed copy must NOT pass ----
    // A check that only ever sees the good file certifies nothing.
    let mut bad = inputs.clone();
    let mid = bad.len() / 2;
    bad[mid].steer = bad[mid].steer.wrapping_add(1);
    let bad_bytes = build(&h, &bad);
    if sha256_hex(&bad_bytes) == h.file_sha256 {
        return Err("the reconstruction check PASSES for a tape with a tick changed. It is not \
                    a check. Refusing."
            .into());
    }
    println!("  PASS  a copy with ONE tick perturbed does NOT reconstruct — the check can fail");

    let bp = out.join(format!("perturbed_{}.Ghost.Gbx", &file_sha[..12]));
    std::fs::write(&bp, &bad_bytes).map_err(|e| e.to_string())?;

    // ---- the oracle ----
    let b = oracle::validate_raw(&oracle::server_dir(), &[p.clone(), bp.clone()], Maps::One(&map), "replay")?;
    std::fs::write(
        out.join(format!("transcript_{}.txt", &file_sha[..12])),
        format!(
            "# replay of {}\n# map {}\n# file_sha256 {}\n\n===== stdout =====\n{}\n===== stderr =====\n{}\n",
            apath.display(),
            map.display(),
            file_sha,
            b.raw,
            b.err
        ),
    )
    .map_err(|e| e.to_string())?;

    let a = b
        .by_name(p.file_name().unwrap().to_str().unwrap())
        .ok_or("the server did not report on the reconstructed file at all")?;
    let bad_a = b.by_name(bp.file_name().unwrap().to_str().unwrap());

    println!("\n  ValidatedResult time  {:?}", a.time_ms);
    println!("  ValidatedResult cps   {:?}", a.cps);
    println!("  DeclaredResult  time  {:?}   (what the FILE claims — not evidence)", a.declared_ms);
    println!("  DeclaredResult  cps   {:?}   (authored by us — not evidence)", a.declared_cps);
    println!("  Desc                  {}", a.desc.trim().replace('\n', " | "));
    println!("  Inputs echo           {}", a.inputs);
    println!("  map uid the server ran {}", a.map_uid);
    if let Some(bad_a) = bad_a {
        println!(
            "  perturbed copy         time {:?}  cps {:?}  ({})",
            bad_a.time_ms,
            bad_a.cps,
            bad_a.desc.trim()
        );
    }

    if a.map_uid != h.map_uid {
        return Err(format!(
            "the server says it validated map {} and the artifact is for {}",
            a.map_uid, h.map_uid
        ));
    }
    println!("  PASS  the server confirms it ran the map this artifact names");

    match a.time_ms {
        Some(t) if t >= 0 => {
            println!("\n*** FINISH REPRODUCED: {} ***", secs(t as u32));
            println!("    From the artifact alone, in a fresh process, byte-identically, and the");
            println!("    server simulated it. Transcript banked in {}", out.display());
            Ok(())
        }
        _ => Err(format!(
            "the reconstruction did NOT finish: {:?} / {}. The artifact rebuilds the bytes but \
             the server does not reproduce the result — that is a real failure and not a \
             rounding difference.",
            a.cps,
            a.desc.trim()
        )),
    }
}
