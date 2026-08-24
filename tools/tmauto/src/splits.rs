//! `tmauto splits` — **where is the time going?**, from the server alone.
//!
//! The truncation ladder in [`crate::cpladder`] establishes that a checkpoint
//! count is a causal function of the tape. Run at a fine tick resolution
//! against a finishing artifact, the same machinery is a **split timer**: the
//! tick at which `cps` steps from `i` to `i+1` is the crossing of gate `i+1`.
//!
//! Paired with the route's arc lengths — which agent B derives from the map and
//! the game's own paks, with no ghost anywhere — that gives a mean speed per
//! segment, and a mean speed per segment says which part of the lap is throwing
//! the time away. On the first end-to-end drive of *Summer 2026 - 01* the
//! answer was not evenly spread: three of the four segments ran at 61–64 m/s
//! and one ran at 35.5.
//!
//! **The arc lengths are commentary, not evidence.** They come from a grid path
//! with a leg-to-chord ratio of 1.0–1.7, so a "mean speed" here is not a
//! measurement of the car — one segment reads 104 m/s, which a Stadium car does
//! not do, and that is the arc length being wrong rather than the car being
//! fast. They are printed to RANK segments against each other. The split TIMES
//! are the measurement.
//!
//! # The one-sided error, and why split times are taken at a STABLE transition
//!
//! `cpladder` originally claimed this construction is monotone by construction
//! because physics is causal. **It is not, and this command is what caught it**:
//! at 100-tick resolution, `k = 600` read 1 where `k = 500` and `k = 700` read 0.
//!
//! The residue after the cut is NEUTRAL, not nothing, and the horizon leaves
//! 200 ms of it — 12 m of coasting in a straight line at 60 m/s, where the real
//! tape was steering. The coast clipped a gate the steered path reached later.
//!
//! The error is **one-sided**: a gate can register early, never late. So a
//! floor ("nothing collected by k") is exact, and a split is taken at the first
//! **stable** transition — the first `k` from which the level never drops again.
//! Isolated high rungs are printed, not smoothed away.

use std::path::PathBuf;
use tmauto::oracle::{self, Maps};
use tmauto::synth::{self, ChunkSet, GhostMeta};
use tmauto::tape::Input;

fn secs(ms: u32) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

pub fn run(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let apath = PathBuf::from(arg(args, "--artifact").ok_or("--artifact is required")?);
    let step: usize = arg(args, "--step").unwrap_or_else(|| "25".into()).parse().map_err(|_| "--step")?;
    let out = PathBuf::from(arg(args, "--out").unwrap_or_else(|| "/tmp/c2/splits".into()));
    let gate_s: Vec<f64> = arg(args, "--gate-s")
        .map(|s| s.split(',').map(|x| x.trim().parse().unwrap_or(0.0)).collect())
        .unwrap_or_default();
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    let (h, inputs) = crate::artifact::read_artifact(&apath)?;
    let map_bytes = std::fs::read(&map).map_err(|e| e.to_string())?;
    if tmauto::sha::sha256_hex(&map_bytes) != h.map_sha256 {
        return Err("the artifact was produced against a different map file. Refusing.".into());
    }

    let n = inputs.len();
    let ks: Vec<usize> = (0..=n).step_by(step).chain(std::iter::once(n)).collect();
    let mut files = Vec::new();
    for k in &ks {
        let mut v = inputs.clone();
        for slot in v.iter_mut().skip(*k) {
            *slot = Input { steer: 0, gas: false, brake: false, respawn: false };
        }
        let hz = (*k as u32) * 10 + 200;
        let mut meta = GhostMeta::probe(&h.map_uid);
        // The most permissive authored count. `cpladder --matrix` measured that
        // a larger one suppresses reported checkpoints; using it here would
        // move the splits and it would look like driving.
        meta.set_declared(hz, vec![hz as i32]);
        let bytes = synth::synthesize(&v, &meta, &ChunkSet::ALL);
        let p = out.join(format!("s_{}.Ghost.Gbx", k));
        std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
        files.push(p);
    }
    let b = oracle::validate_raw(&oracle::server_dir(), &files, Maps::One(&map), "splits")?;
    std::fs::write(
        out.join("transcript_splits.txt"),
        format!("===== stdout =====\n{}\n===== stderr =====\n{}\n", b.raw, b.err),
    )
    .map_err(|e| e.to_string())?;

    let mut cs: Vec<u32> = Vec::new();
    let mut fins: Vec<Option<i64>> = Vec::new();
    for f in &files {
        let a = b.by_name(f.file_name().unwrap().to_str().unwrap());
        cs.push(a.map(|a| a.cps.unwrap_or(0)).unwrap_or(0));
        fins.push(a.and_then(|a| a.time_ms).filter(|t| *t >= 0));
    }
    let fin: Option<i64> = fins.iter().flatten().copied().min();

    println!("ARTIFACT {}", apath.display());
    println!("resolution {} ticks = {} of race\n", step, secs(step as u32 * 10));

    let mut isolated: Vec<String> = Vec::new();
    for i in 0..cs.len() {
        if cs[i..].iter().any(|c| *c < cs[i]) {
            isolated.push(format!("k {} read {}", ks[i], cs[i]));
        }
    }
    if !isolated.is_empty() || arg(args, "--dump").is_some() {
        println!("{:>8} {:>9} {:>4} {:>10}", "k", "= race", "cps", "finish");
        for ((k, c), f) in ks.iter().zip(&cs).zip(&fins) {
            println!(
                "{:>8} {:>9} {:>4} {:>10}",
                k,
                secs(*k as u32 * 10),
                c,
                f.map(|t| secs(t as u32)).unwrap_or_else(|| "-".into())
            );
        }
        println!();
    }
    if isolated.is_empty() {
        println!("  PASS  monotone — no rung was later contradicted");
    } else {
        println!(
            "  note  {} rung(s) read HIGH and were later contradicted ({}). That is the coast \
             clipping a gate; it is one-sided, and splits below use the first STABLE transition.",
            isolated.len(),
            isolated.join("; ")
        );
    }
    match fin {
        Some(t) => println!("  PASS  the ladder reaches a finish at {}", secs(t as u32)),
        None => println!("  note  no rung finished — this tape does not cross the line here"),
    }

    // First STABLE transition.
    let mut splits: Vec<(u32, u32)> = Vec::new();
    let top = *cs.last().unwrap_or(&0);
    for g in 1..=top {
        if let Some(i) = (0..cs.len()).find(|i| cs[*i..].iter().all(|c| *c >= g)) {
            splits.push((g, ks[i] as u32 * 10));
        }
    }

    println!("\n{:>6} {:>10} {:>10} {:>12} {:>10}", "gate", "at", "segment", "arc length", "mean m/s");
    let mut last_ms = 0u32;
    let mut last_s = 0.0f64;
    for (g, ms) in &splits {
        let seg = ms.saturating_sub(last_ms);
        let (arc, sp) = match gate_s.get(*g as usize - 1) {
            Some(s) => {
                let d = s - last_s;
                last_s = *s;
                (format!("{:.1} m", d), format!("{:.1}", d / (seg as f64 / 1000.0).max(1e-6)))
            }
            None => ("-".to_string(), "-".to_string()),
        };
        println!("{:>6} {:>10} {:>10} {:>12} {:>10}", g, secs(*ms), secs(seg), arc, sp);
        last_ms = *ms;
    }

    if splits.len() >= 2 && !gate_s.is_empty() {
        let mut worst = (0usize, f64::MAX);
        let mut lm = 0u32;
        let mut ls = 0.0;
        for (i, (g, ms)) in splits.iter().enumerate() {
            if let Some(s) = gate_s.get(*g as usize - 1) {
                let sp = (s - ls) / ((ms - lm) as f64 / 1000.0).max(1e-6);
                if sp < worst.1 {
                    worst = (i, sp);
                }
                ls = *s;
                lm = *ms;
            }
        }
        let a = if worst.0 == 0 { 0 } else { splits[worst.0 - 1].1 / 10 };
        println!(
            "\n  SLOWEST SEGMENT ends at gate {} — {:.1} m/s. Aim the search there:\n  \
             `optimize --focus {},{}`, or `pushgate --gate {}` to attack it directly.",
            splits[worst.0].0,
            worst.1,
            a,
            splits[worst.0].1 / 10,
            splits[worst.0].0
        );
    }
    println!("\ntranscript banked in {}", out.display());
    Ok(())
}
