//! `tmauto startprobe` — **where does the server put the car at tick 0?**,
//! asked with nothing but the server's own verdict.
//!
//! # Why this exists
//!
//! Agent C's `cps 3` on *Summer 2026 - 01* was retracted because a memory
//! trace put the car ~360 m from the map's spawn, on the finish straight. Two
//! hypotheses were live:
//!
//! * **wrong start** — the server initialises at waypoint *index* 0 (which on
//!   that map is a Checkpoint on the finish straight) rather than at the
//!   `Spawn`-tagged waypoint;
//! * **wrong entity** — `fk::locate` locked onto a self-consistent decoy and
//!   the car was at the spawn all along.
//!
//! Every instrument we had for the first question read the same memory as the
//! second one. **This one reads none of it.** It asks the dedicated server to
//! simulate a tape and reports what the server says, and nothing else.
//!
//! # The measurement
//!
//! On *Summer 2026 - 01* the checkpoint at waypoint index 0 sits at
//! `x 1360, z 1104` and the Goal sits at `x 1360, z 688`: **the same x, 416 m
//! apart in z.** They are the two ends of one straight.
//!
//! So a tape of *pure full throttle with zero steer* has a sharp, prearranged
//! meaning here:
//!
//! | server says | what it means |
//! |---|---|
//! | **Finish** | the car started on the finish straight, facing the Goal. The wrong-start hypothesis is CONFIRMED and no memory was read to say so. |
//! | DNF | the car did not start on that straight facing that way. The hypothesis is not supported by this test. |
//!
//! That is a prediction made **before** the run, from map geometry alone, whose
//! two outcomes are distinguishable. It is not a check that any outcome
//! satisfies.
//!
//! # The controls that ship with it
//!
//! * **positive (does the instrument respond at all?)** — `left` and `right`
//!   are the same tape with the steering pinned to each stop. If all three
//!   variants come back with an identical verdict *and* an identical decoded
//!   input echo, the server is not reading our tape and every reading here is
//!   UNMEASURED.
//! * **negative (does a car that cannot move score?)** — `parked` holds no
//!   throttle. A checkpoint credited to a car that never moved would mean the
//!   count is not about driving.
//! * **the declared-checkpoint minimal pair** — `straight` is emitted at
//!   several values of `--declared-cps` with the tape held byte-identical.
//!   `DeclaredResult.NbCheckpoints` is authored by us; if the *simulated*
//!   count moves with it, the count was never evidence about driving.
//!
//! Every variant's raw server transcript is written to the output directory,
//! because "`wrong simu`" with the text discarded is a result nobody can
//! reclassify later.

use tmauto::oracle::{self, Answer, Maps};
use tmauto::synth::{self, ChunkSet, GhostMeta};
use tmauto::tape::Input;
use std::path::{Path, PathBuf};

/// One thing we ask the server, and the prediction that makes its answer mean
/// something.
struct Variant {
    name: &'static str,
    /// What the tape is, in one line, for the report.
    what: &'static str,
    inputs: Vec<Input>,
    /// How many checkpoints the FILE claims. Authored by us; never evidence.
    declared_cps: usize,
}

/// The steer pattern the explorer's own template uses, reproduced here so the
/// probe can include the exact tape the retracted run was built on rather than
/// a paraphrase of it.
fn wobble(ticks: usize) -> Vec<Input> {
    (0..ticks)
        .map(|t| Input {
            steer: ((t as u64 * 7919 + 13) % 25) as i8 - 12,
            gas: true,
            brake: false,
            respawn: false,
        })
        .collect()
}

fn constant(ticks: usize, steer: i8, gas: bool, brake: bool) -> Vec<Input> {
    vec![Input { steer, gas, brake, respawn: false }; ticks]
}

pub fn run(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let ticks: usize =
        arg(args, "--ticks").unwrap_or_else(|| "4000".into()).parse().map_err(|_| "--ticks")?;
    let declared_ms: u32 = arg(args, "--declared")
        .unwrap_or_else(|| "40000".into())
        .parse()
        .map_err(|_| "--declared")?;
    let out = PathBuf::from(
        arg(args, "--out").unwrap_or_else(|| "/tmp/tmauto-startprobe".into()),
    );
    let tag = arg(args, "--tag").unwrap_or_else(|| "startprobe".into());
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    let uid = crate::map_uid(&map)?;

    // The declared time governs how long the server simulates: the validator
    // stops at the DECLARED time, not at the end of the tape. A tape shorter
    // than that window then stops early for a second, different reason, so the
    // two are stated together and checked against each other.
    let tape_ms = ticks as u32 * 10;
    if tape_ms < declared_ms {
        return Err(format!(
            "the tape is {} of race and the file declares {}: the tape runs out first, \
             so a DNF here would not be about driving. Raise --ticks to at least {}.",
            secs(tape_ms),
            secs(declared_ms),
            (declared_ms as usize + 9) / 10
        ));
    }

    let mut vs: Vec<Variant> = vec![
        Variant {
            name: "straight",
            what: "full throttle, steer 0 — goes wherever the car is pointed at tick 0",
            inputs: constant(ticks, 0, true, false),
            declared_cps: 3,
        },
        Variant {
            name: "left",
            what: "full throttle, steer at the left stop (positive control: the tape is read)",
            inputs: constant(ticks, -127, true, false),
            declared_cps: 3,
        },
        Variant {
            name: "right",
            what: "full throttle, steer at the right stop (mirror of `left`)",
            inputs: constant(ticks, 127, true, false),
            declared_cps: 3,
        },
        Variant {
            name: "parked",
            what: "no throttle at all (negative control: a car that cannot move must score nothing)",
            inputs: constant(ticks, 0, false, false),
            declared_cps: 3,
        },
        Variant {
            name: "wobble",
            what: "the explorer template's own steer cycle — the tape the retracted run was built on",
            inputs: wobble(ticks),
            declared_cps: 3,
        },
    ];
    // The declared-checkpoint minimal pair: the SAME tape, one field moved.
    for n in [0usize, 1, 6] {
        vs.push(Variant {
            name: Box::leak(format!("straight_dcp{}", n).into_boxed_str()),
            what: "byte-identical to `straight`; only DeclaredResult.NbCheckpoints differs",
            inputs: constant(ticks, 0, true, false),
            declared_cps: n,
        });
    }

    println!("MAP        {}", map.display());
    println!("map uid    {}", uid);
    println!("tape       {} ticks = {} of race", ticks, secs(tape_ms));
    println!("declared   {}", secs(declared_ms));
    println!("variants   {}\n", vs.len());

    let mut files = Vec::new();
    for v in &vs {
        let mut meta = GhostMeta::probe(&uid);
        // set_declared, never a direct assignment: the walltime pair is checked
        // against the declared time and setting one without the other is the
        // "unexcepted walltime" refusal, which reads as a container bug.
        let cps: Vec<i32> = (1..=v.declared_cps)
            .map(|i| (declared_ms as i32 / (v.declared_cps as i32 + 1)) * i as i32)
            .chain(std::iter::once(declared_ms as i32))
            .collect();
        meta.set_declared(declared_ms, cps);
        let bytes = synth::synthesize(&v.inputs, &meta, &ChunkSet::ALL);
        let p = out.join(format!("{}.Ghost.Gbx", v.name));
        std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
        files.push(p);
    }

    let batch = oracle::validate_raw(&oracle::server_dir(), &files, Maps::One(&map), &tag)?;

    // THE TRANSCRIPT IS THE RESULT. Bank it before interpreting it: a bare
    // `wrong simu` with the text thrown away is a reading nobody can reclassify.
    let tpath = out.join("transcript.txt");
    std::fs::write(
        &tpath,
        format!(
            "# tmauto startprobe transcript\n# map {}\n# uid {}\n# ticks {}\n# declared_ms {}\n\
             \n===== stdout =====\n{}\n===== stderr =====\n{}\n",
            map.display(),
            uid,
            ticks,
            declared_ms,
            batch.raw,
            batch.err
        ),
    )
    .map_err(|e| e.to_string())?;
    println!("transcript banked at {}", tpath.display());
    println!(
        "server read {} of {} files (its own count line)\n",
        batch.ghosts_found().map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
        files.len()
    );

    println!(
        "{:<16} {:>9} {:>5} {:>9} {:>5}  {:<10} {}",
        "variant", "simulated", "cps", "declared", "dcps", "echo", "desc"
    );
    let mut rows: Vec<(&Variant, Option<&Answer>)> = Vec::new();
    for (v, f) in vs.iter().zip(&files) {
        let a = batch.by_name(f.file_name().unwrap().to_str().unwrap());
        rows.push((v, a));
        match a {
            None => println!("{:<16} {:>9}", v.name, "NOT READ"),
            Some(a) => println!(
                "{:<16} {:>9} {:>5} {:>9} {:>5}  {:<10} {}",
                v.name,
                a.time_ms.map(|t| secs(t as u32)).unwrap_or_else(|| "-".into()),
                a.cps.map(|c| c.to_string()).unwrap_or_else(|| "-".into()),
                a.declared_ms.map(|t| secs(t as u32)).unwrap_or_else(|| "-".into()),
                a.declared_cps.map(|c| c.to_string()).unwrap_or_else(|| "-".into()),
                short(&a.inputs),
                a.desc.trim().replace('\n', " | ")
            ),
        }
    }

    println!("\n--- what each variant is ---");
    for v in &vs {
        println!("  {:<16} {}", v.name, v.what);
    }

    // ---- the controls, stated as pass/fail rather than left to the reader ----
    println!("\n--- controls ---");

    let echo = |n: &str| -> Option<String> {
        rows.iter().find(|(v, _)| v.name == n).and_then(|(_, a)| a.map(|a| a.inputs.clone()))
    };
    let verd = |n: &str| -> Option<String> {
        rows.iter().find(|(v, _)| v.name == n).and_then(|(_, a)| {
            a.map(|a| format!("{:?}/{:?}/{}", a.time_ms, a.cps, a.desc.trim()))
        })
    };

    // POSITIVE: the server must distinguish two tapes we know are different.
    match (echo("left"), echo("right")) {
        (Some(l), Some(r)) if l != r => {
            println!("  PASS  the decoded input echo differs between `left` and `right` — the server is reading OUR tape")
        }
        (Some(_), Some(_)) => println!(
            "  FAIL  `left` and `right` decode to the SAME echo. The server is not reading our \
             tape; every reading in this table is UNMEASURED."
        ),
        _ => println!("  FAIL  one of `left`/`right` was not read at all — UNMEASURED"),
    }
    match (verd("left"), verd("right")) {
        (Some(l), Some(r)) if l != r => {
            println!("  PASS  `left` and `right` produce DIFFERENT server verdicts — the simulation responds to steering")
        }
        (Some(_), Some(_)) => println!(
            "  note  `left` and `right` produce the same verdict. Not fatal (both may simply \
             crash immediately) but it carries no information about steering."
        ),
        _ => println!("  FAIL  one of `left`/`right` was not read at all — UNMEASURED"),
    }

    // NEGATIVE: a car that never moved must not be credited with a checkpoint.
    match rows.iter().find(|(v, _)| v.name == "parked").and_then(|(_, a)| *a) {
        Some(a) if a.time_ms.is_some() || a.cps.unwrap_or(0) > 0 => println!(
            "  FAIL  `parked` (no throttle at any tick) was credited with {:?} checkpoints / \
             a time of {:?}. A count that a motionless car can earn is not a measurement of driving.",
            a.cps, a.time_ms
        ),
        Some(_) => println!("  PASS  `parked` scored nothing — the count requires the car to move"),
        None => println!("  FAIL  `parked` was not read — UNMEASURED"),
    }

    // THE MINIMAL PAIR: same tape, only the authored count moves.
    let dcp: Vec<(usize, Option<u32>, Option<i64>)> = rows
        .iter()
        .filter(|(v, _)| v.name.starts_with("straight"))
        .map(|(v, a)| (v.declared_cps, a.and_then(|a| a.cps), a.and_then(|a| a.time_ms)))
        .collect();
    let simulated: Vec<Option<u32>> = dcp.iter().map(|d| d.1).collect();
    let all_same = simulated.windows(2).all(|w| w[0] == w[1]);
    println!(
        "  minimal pair on DeclaredResult.NbCheckpoints: {}",
        dcp.iter()
            .map(|(d, c, t)| format!("declared {} -> simulated cps {:?}, time {:?}", d, c, t))
            .collect::<Vec<_>>()
            .join("; ")
    );
    if all_same {
        println!(
            "  PASS  the SIMULATED count does not move with the count we authored — \
             `declared_cps` is not the cause"
        );
    } else {
        println!(
            "  HIT   the simulated count MOVES with the count we authored. The reported \
             checkpoints were partly our own declaration, not driving."
        );
    }

    // The prearranged reading of `straight`.
    println!("\n--- the prearranged reading ---");
    match rows.iter().find(|(v, _)| v.name == "straight").and_then(|(_, a)| *a) {
        Some(a) if a.time_ms.map(|t| t >= 0).unwrap_or(false) => println!(
            "  A PURE FULL-THROTTLE ZERO-STEER TAPE FINISHED THE MAP in {}.\n  \
             A car at the map's spawn does not finish by driving straight. Read together with \
             this map's geometry — waypoint index 0 and the Goal share an x and are 416 m apart \
             in z — this says the car started ON THE FINISH STRAIGHT.",
            secs(a.time_ms.unwrap() as u32)
        ),
        Some(a) => println!(
            "  `straight` did not finish (cps {:?}, desc {:?}). The car did not start on the \
             finish straight facing the Goal. This does NOT by itself say the start is right — \
             it rules out one specific wrong start.",
            a.cps,
            a.desc.trim()
        ),
        None => println!("  `straight` was not read — UNMEASURED"),
    }

    Ok(())
}

fn short(s: &str) -> String {
    let s = s.trim();
    if s.len() > 10 {
        format!("{}…", &s[..9])
    } else {
        s.to_string()
    }
}

/// Times as seconds with a decimal, never raw milliseconds.
fn secs(ms: u32) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

#[allow(dead_code)]
fn unused(_: &Path) {}
