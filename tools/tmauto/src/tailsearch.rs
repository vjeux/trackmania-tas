//! `tmauto tailsearch` — extend a banked tape and ask the plain oracle whether
//! it finishes.
//!
//! # Why this is a separate thing from the explorer
//!
//! The explorer searches through a fork server and reads the car's position out
//! of process memory. Both of those are currently under suspicion: the locate
//! can lock onto a self-consistent decoy (`fk::locate::locate_candidates`'
//! own comment says so, naming map 126859), and every progress number the
//! search uses is downstream of that one readout.
//!
//! This command reads **no memory at all**. It writes candidate containers,
//! hands them to the dedicated server, and reports what the server says. That
//! is slower per node and it cannot see where the car is — but what it does
//! see, it sees through an instrument with no shared source with the one that
//! is in doubt, and a finish is self-certifying: **a Trackmania car cannot
//! cross the finish line without having crossed the start and collected every
//! checkpoint.** So a `Finish` here answers the start-position question, the
//! checkpoint-order question and the entity question in one shot, and answers
//! them from the server rather than from us.
//!
//! # The shape of the search
//!
//! Cut the banked tape at `k` and replace everything after it with a constant
//! macro — one steer value, throttle on or off — for the rest of the archive.
//! Sweep `k` and the macro. This is deliberately the crudest possible
//! continuation, because the segment it is aimed at is a straight: on
//! *Summer 2026 - 01* the last checkpoint at (1360, 1104) and the Goal at
//! (1360, 688) share an x and are 416 m apart in z. If the car is on that
//! straight and pointed down it, a constant input finishes the map, and
//! anything cleverer is unnecessary machinery in front of a measurement.
//!
//! If it does **not** finish, that is information too, and the sweep says which
//! cut points and macros were tried rather than reporting a bare failure.

use std::path::PathBuf;
use tmauto::oracle::{self, Maps};
use tmauto::synth::{self, ChunkSet, GhostMeta};
use tmauto::tape::Input;

use crate::cpladder::{read_tape, template_inputs};

fn secs(ms: u32) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

pub fn run(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    // Two ways in: a raw banked tape plus its frame, or a self-contained
    // artifact (whose body IS the full container array, so its frame is 0).
    // The artifact form closes the pushgate -> tailsearch -> optimize pipeline,
    // because `pushgate` breaks the tail deliberately and hands back a tape
    // that may not finish -- which is exactly what this command repairs.
    let art = arg(args, "--artifact").map(PathBuf::from);
    let (tape, prefix, ticks, tape_path) = match &art {
        Some(p) => {
            let (_hh, ii) = crate::artifact::read_artifact(p)?;
            let n = ii.len();
            (ii, 0usize, n, p.clone())
        }
        None => {
            let tp = PathBuf::from(arg(args, "--tape").ok_or("--tape or --artifact is required")?);
            let pf: usize = arg(args, "--prefix").ok_or("--prefix is required with --tape")?.parse().map_err(|_| "--prefix")?;
            let tk: usize = arg(args, "--ticks").unwrap_or_else(|| "9000".into()).parse().map_err(|_| "--ticks")?;
            let t = read_tape(&tp)?;
            (t, pf, tk, tp)
        }
    };
    let declared: u32 =
        arg(args, "--declared").unwrap_or_else(|| "70000".into()).parse().map_err(|_| "--declared")?;
    let out = PathBuf::from(arg(args, "--out").unwrap_or_else(|| "/tmp/c2/tailsearch".into()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let uid = crate::map_uid(&map)?;
    let ks: Vec<usize> = arg(args, "--ks")
        .unwrap_or_else(|| {
            let n = tape.len();
            (0..12).map(|i| (n - i * 100).to_string()).collect::<Vec<_>>().join(",")
        })
        .split(',')
        .map(|s| s.trim().parse().map_err(|_| "--ks"))
        .collect::<Result<_, _>>()?;
    let steers: Vec<i8> = arg(args, "--steers")
        .unwrap_or_else(|| "-64,-48,-32,-24,-16,-12,-8,-4,0,4,8,12,16,24,32,48,64".into())
        .split(',')
        .map(|s| s.trim().parse().map_err(|_| "--steers"))
        .collect::<Result<_, _>>()?;

    println!("MAP    {}  ({})", map.display(), uid);
    println!("TAPE   {} ({} ticks), prefix {}", tape_path.display(), tape.len(), prefix);
    println!("SWEEP  {} cut points x {} constant macros = {} candidates", ks.len(), steers.len(), ks.len() * steers.len());
    println!("       declared {} (the validator simulates to the DECLARED time, so this bounds the run)\n", secs(declared));

    let mut files = Vec::new();
    let mut cells = Vec::new();
    for k in &ks {
        for s in &steers {
            let mut inputs = template_inputs(ticks);
            for (i, t) in tape.iter().take(*k).enumerate() {
                if prefix + i < ticks {
                    inputs[prefix + i] = *t;
                }
            }
            for slot in inputs.iter_mut().skip(prefix + *k) {
                *slot = Input { steer: *s, gas: true, brake: false, respawn: false };
            }
            let mut meta = GhostMeta::probe(&uid);
            // One authored checkpoint time and the finish: the matrix in
            // `cpladder` measured that a LARGER authored count suppresses the
            // reported checkpoints, so the most permissive setting is used
            // here and the number itself is never read as evidence.
            meta.set_declared(declared, vec![declared as i32 / 2, declared as i32]);
            let bytes = synth::synthesize(&inputs, &meta, &ChunkSet::ALL);
            let p = out.join(format!("t_k{}_s{}.Ghost.Gbx", k, s));
            std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
            files.push(p);
            cells.push((*k, *s));
        }
    }

    let b = oracle::validate_raw(&oracle::server_dir(), &files, Maps::One(&map), "tail")?;
    std::fs::write(
        out.join("transcript_tail.txt"),
        format!("===== stdout =====\n{}\n===== stderr =====\n{}\n", b.raw, b.err),
    )
    .map_err(|e| e.to_string())?;

    let mut finishes: Vec<(usize, i8, i64, String)> = Vec::new();
    let mut best_cps = 0u32;
    let mut grid: Vec<(usize, Vec<String>)> = Vec::new();
    for k in &ks {
        let mut row = Vec::new();
        for s in &steers {
            let i = cells.iter().position(|c| c == &(*k, *s)).unwrap();
            let a = b.by_name(files[i].file_name().unwrap().to_str().unwrap());
            match a {
                None => row.push("?".to_string()),
                Some(a) => match a.time_ms {
                    Some(t) if t >= 0 => {
                        finishes.push((*k, *s, t, files[i].display().to_string()));
                        row.push(format!("F{}", secs(t as u32)));
                    }
                    _ => {
                        let c = a.cps.unwrap_or(0);
                        best_cps = best_cps.max(c);
                        row.push(c.to_string());
                    }
                },
            }
        }
        grid.push((*k, row));
    }

    print!("{:>7}", "k \\ st");
    for s in &steers {
        print!("{:>10}", s);
    }
    println!();
    for (k, row) in &grid {
        print!("{:>7}", k);
        for c in row {
            print!("{:>10}", c);
        }
        println!();
    }

    println!("\n--- controls ---");
    println!(
        "  the sweep contains its own two-sided pair: hard left and hard right cut from the \
         SAME tick. If every cell of a row is identical the continuation is not reaching the \
         car and that row is UNMEASURED."
    );
    let flat: Vec<usize> = grid
        .iter()
        .filter(|(_, r)| r.windows(2).all(|w| w[0] == w[1]))
        .map(|(k, _)| *k)
        .collect();
    if flat.is_empty() {
        println!("  PASS  every cut point responds to the macro — no flat row");
    } else {
        println!("  note  these cut points gave the same answer for every macro: {:?}", flat);
    }

    if finishes.is_empty() {
        println!(
            "\nNO FINISH. Best non-finishing answer was cps {}. That is a real negative: {} \
             constant continuations from {} cut points were simulated by the server and none \
             crossed the line. The next move is a continuation that is not constant, or a \
             progress signal on this segment — not a bigger constant sweep.",
            best_cps,
            cells.len(),
            ks.len()
        );
    } else {
        finishes.sort_by_key(|f| f.2);
        println!("\n*** {} FINISH(ES) ***", finishes.len());
        for (k, s, t, f) in &finishes {
            println!("  cut {:>5}, steer {:>4} -> {}   {}", k, s, secs(*t as u32), f);
        }
        let (k, s, t, _) = &finishes[0];
        println!(
            "\nBEST {} (author time for this map is a number in the map file; the comparison \
             belongs in the report, not here). Reconstruct it with: cut {} steer {}.",
            secs(*t as u32),
            k,
            s
        );
    }
    println!("\ntranscript banked in {}", out.display());
    Ok(())
}
