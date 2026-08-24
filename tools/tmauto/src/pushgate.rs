//! `tmauto pushgate` — make the car reach checkpoint *G* **earlier**, with the
//! dedicated server as the only instrument.
//!
//! # The problem this solves
//!
//! `tmauto optimize` hill-climbs on finish time and it works, until it does
//! not: on *Summer 2026 - 01* it took 45.573 to 36.6 and then started returning
//! 0.025 s per twelve thousand evaluations. The reason is structural, not a
//! tuning failure. `tmauto splits` measured where the time actually goes:
//!
//! | gate | segment | mean speed |
//! |---|---|---|
//! | 1 | 8.000 | 63.9 m/s |
//! | **2** | **17.000** | **35.5 m/s** |
//! | 3 | 6.000 | 61.4 m/s |
//! | 4 | 4.000 | — |
//!
//! One segment is half the speed of the others and eats 17 s of a 36.7 s lap.
//! Fixing it means taking a **different line**, and a different line is not
//! reachable by local perturbations each judged on the *finish* time — any
//! change big enough to matter desynchronises the rest of the tape and scores
//! worse, so the climb rejects exactly the moves that would help.
//!
//! # The objective
//!
//! The trick that found the first finish, moved to an intermediate gate. The
//! validator simulates to the **declared time**, so declaring `H` asks the
//! server one clean question:
//!
//! > *had the car collected `G` checkpoints by `H`?*
//!
//! Start `H` just under the incumbent's own crossing of gate `G`. Any candidate
//! that answers yes reached that gate **earlier than the incumbent did**; it
//! becomes the incumbent and `H` drops again. It is a ratchet, every rung is a
//! server verdict, and nothing past `H` is simulated — so the tail of the tape
//! can neither dilute the signal nor veto the move.
//!
//! **This deliberately does not care whether the tape still finishes.** It
//! cannot: the point is to break the tail so a better line through the segment
//! can be found. Re-solving the tail is `tailsearch`'s and `optimize`'s job:
//! `pushgate` → `tailsearch` → `optimize`, in that order.
//!
//! Measured on the first run: gate 2 went from 25.000 to **22.832 in one
//! generation**, against 0.025 s per generation from the finish-time climb.
//!
//! # Controls
//!
//! * **the incumbent rides in every generation** and must still answer yes at
//!   its own crossing time — the standing re-simulation.
//! * **a wrecked copy rides in every generation** (hard left from tick 300) and
//!   must answer no. Without it, a batch where the server said yes to
//!   everything would read as a breakthrough.
//! * **the horizon only moves down**, and only after a bisection has confirmed
//!   the new incumbent at the new value. A ratchet that can slip manufactures
//!   progress.

use std::path::PathBuf;
use tmauto::oracle;
use tmauto::tape::Input;
use tmauto::verdict::Verdict;

use crate::artifact;
use crate::optimize::{mutate_pub, write_artifact_pub, Rng};

fn secs(ms: u32) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn cps_at(
    map: &std::path::Path,
    tapes: &[Vec<Input>],
    ticks: usize,
    jobs: usize,
    per_launch: usize,
    horizon: u32,
    work: &std::path::Path,
) -> Result<Vec<u32>, String> {
    let evs = oracle::evaluate_declared(map, tapes, ticks, jobs, per_launch, horizon, work)?;
    Ok(evs
        .into_iter()
        .map(|e| match e.map(|e| e.verdict) {
            // A finish inside the horizon means every gate plus the line.
            Some(Verdict::Finish { .. }) => u32::MAX,
            Some(Verdict::Dnf { cps }) => cps,
            None => 0,
        })
        .collect())
}

/// The incumbent's own crossing of gate `g`, bisected on the horizon. Measured
/// each time, never carried over.
fn crossing(
    map: &std::path::Path,
    tape: &[Input],
    g: u32,
    ticks: usize,
    jobs: usize,
    work: &std::path::Path,
) -> Result<u32, String> {
    let one = vec![tape.to_vec()];
    let mut lo = 300u32;
    let mut hi = (ticks as u32) * 10 - 200;
    let top = cps_at(map, &one, ticks, jobs, 1, hi, work)?[0];
    if top < g {
        return Err(format!(
            "this tape never reaches gate {} at all (best {} by {}). pushgate needs an \
             incumbent that already crosses the gate it is asked to make earlier.",
            g,
            top,
            secs(hi)
        ));
    }
    while hi - lo > 100 {
        let mid = (lo + hi) / 2;
        if cps_at(map, &one, ticks, jobs, 1, mid, work)?[0] >= g {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(hi)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let apath = PathBuf::from(arg(args, "--artifact").ok_or("--artifact is required")?);
    let bank = PathBuf::from(arg(args, "--bank").ok_or("--bank is required")?);
    let work = PathBuf::from(arg(args, "--work").unwrap_or_else(|| "/tmp/c2/pushwork".into()));
    let gate: u32 = arg(args, "--gate").ok_or("--gate is required")?.parse().map_err(|_| "--gate")?;
    let lambda: usize = arg(args, "--lambda").unwrap_or_else(|| "12000".into()).parse().map_err(|_| "--lambda")?;
    let jobs: usize = arg(args, "--jobs").unwrap_or_else(|| "60".into()).parse().map_err(|_| "--jobs")?;
    let minutes: u64 = arg(args, "--minutes").unwrap_or_else(|| "60".into()).parse().map_err(|_| "--minutes")?;
    let step: u32 = arg(args, "--step-ms").unwrap_or_else(|| "100".into()).parse().map_err(|_| "--step-ms")?;
    let seed: u64 = arg(args, "--seed").and_then(|s| s.parse().ok()).unwrap_or(0xC2_60A7);
    let per_launch = ((lambda + 2) / jobs.max(1)).max(1);
    std::fs::create_dir_all(&bank).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;

    let (h, inputs0) = artifact::read_artifact(&apath)?;
    if tmauto::sha::sha256_hex(&std::fs::read(&map).map_err(|e| e.to_string())?) != h.map_sha256 {
        return Err("the artifact was produced against a different map file. Refusing.".into());
    }
    let (mut best, h) = match arg(args, "--ticks") {
        None => (inputs0, h),
        Some(s) => {
            let n: usize = s.parse().map_err(|_| "--ticks")?;
            if n > inputs0.len() {
                return Err("--ticks truncates, it does not extend".into());
            }
            let d = h.declared_ms.min((n as u32) * 10 - 200);
            let mut h2 = h;
            h2.declared_ms = d;
            h2.declared_cps = vec![d as i32 / 2, d as i32];
            h2.container_ticks = n;
            (inputs0[..n].to_vec(), h2)
        }
    };
    let ticks = best.len();

    let mut cross = crossing(&map, &best, gate, ticks, jobs, &work)?;
    println!("MAP        {}  ({})", map.display(), h.map_uid);
    println!("GATE       {}", gate);
    println!("INCUMBENT  crosses gate {} by {} (bisected here, not inherited)", gate, secs(cross));
    println!("CONTAINER  {} ticks, lambda {}, {} jobs, {} per launch\n", ticks, lambda, jobs, per_launch);

    let mut rng = Rng::new(seed);
    let t0 = std::time::Instant::now();
    let mut gen = 0u64;
    let mut evals = 0u64;

    while t0.elapsed().as_secs() < minutes * 60 {
        gen += 1;
        let horizon = cross.saturating_sub(step).max(400);
        // Mutations live before the horizon: nothing after it is simulated, so
        // an edit there is a wasted candidate rather than a neutral one.
        let active = ((horizon / 10) as usize + 20).min(ticks);
        let focus = Some((0usize, active));

        let mut cands: Vec<Vec<Input>> = Vec::with_capacity(lambda + 2);
        cands.push(best.clone());
        let mut wrecked = best.clone();
        for t in wrecked.iter_mut().skip(300) {
            t.steer = -128;
        }
        cands.push(wrecked);
        for _ in 0..lambda {
            cands.push(mutate_pub(&best, active, focus, &mut rng));
        }

        let cs = cps_at(&map, &cands, ticks, jobs, per_launch, horizon, &work)?;
        evals += cands.len() as u64;

        if cs[1] >= gate {
            return Err(format!(
                "NEGATIVE CONTROL FAILED at generation {}: the incumbent with hard left held \
                 from tick 300 reached gate {} by {}. This batch cannot tell a wrecked run from \
                 a good one; stopping.",
                gen,
                gate,
                secs(horizon)
            ));
        }

        match cs.iter().enumerate().skip(2).find(|(_, c)| **c >= gate) {
            Some((i, _)) => {
                best = cands[i].clone();
                // Re-bisect rather than assume the winner sits exactly at the
                // horizon: it may be a lot earlier, and assuming otherwise
                // throws the surplus away one step at a time.
                cross = crossing(&map, &best, gate, ticks, jobs, &work)?;
                let name = format!("gate{}-{}.artifact.tsv", gate, cross);
                write_artifact_pub(&bank.join(&name), &h, &best, cross, &apath)?;
                println!(
                    "[{:>5}s] gen {:>4} {:>9} evals   *** gate {} now crossed by {} ***  banked {}",
                    t0.elapsed().as_secs(),
                    gen,
                    evals,
                    gate,
                    secs(cross),
                    name
                );
            }
            None => {
                if cs[0] >= gate {
                    println!(
                        "[{:>5}s] gen {:>4} {:>9} evals   the incumbent itself answered yes at \
                         this horizon — the bisection was stale; re-bisecting",
                        t0.elapsed().as_secs(),
                        gen,
                        evals
                    );
                    cross = crossing(&map, &best, gate, ticks, jobs, &work)?;
                } else {
                    println!(
                        "[{:>5}s] gen {:>4} {:>9} evals   nothing reached gate {} by {} (incumbent {})",
                        t0.elapsed().as_secs(),
                        gen,
                        evals,
                        gate,
                        secs(horizon),
                        secs(cross)
                    );
                }
            }
        }
    }

    println!("\nDONE  {} generations, {} evals. Gate {} is crossed by {}.", gen, evals, gate, secs(cross));
    println!("      NEXT: the tail of this tape is broken by design. Re-solve it with");
    println!("      `tmauto tailsearch --artifact <banked>` and then `tmauto optimize`.");
    Ok(())
}
