//! `tmauto optimize` — drive the map FASTER, with the dedicated server as the
//! only judge.
//!
//! # Why this can exist now and could not before
//!
//! Until there was a finish, the objective was flat: every candidate on
//! *Summer 2026 - 01* came back `Dnf { cps: 3 }` and nothing distinguished a
//! car 10 m from the line from one 300 m away. That is why the explorer needed
//! a position readout at all, and the position readout is the component this
//! project spent a day discovering it could not trust.
//!
//! **A finish time is a dense objective.** Once one tape crosses the line,
//! every improvement is visible to the server alone, and the whole
//! memory-reading apparatus — the fork server, `fk::locate`, `progress()`,
//! agent B's route — drops out of the loop. Nothing in this file reads process
//! memory or a `.Ghost.Gbx` that anybody drove.
//!
//! # The loop
//!
//! A (1 + λ) hill climb. Each generation mutates the incumbent input array λ
//! ways, hands the whole generation to the pooled server in one call, and keeps
//! the best answer if it beats the incumbent under [`tmauto::verdict::Verdict`]
//! — whose `Ord` puts every finisher above every non-finisher **by
//! construction**, so there is no arithmetic on a score and no sentinel.
//!
//! # Two controls, in every generation, not once at the start
//!
//! * **the incumbent rides in its own batch.** Candidate 0 of every generation
//!   IS the current best, re-synthesized and re-simulated from scratch. So the
//!   standing claim — *a banked incumbent is not a result until the plain
//!   oracle re-simulates the written tape* — is re-established every few
//!   seconds rather than spot-checked. If it ever stops reproducing, the run
//!   stops.
//! * **a known-bad rider.** Candidate 1 is the incumbent with hard left held
//!   from tick 300. It must NOT finish. A generation where the wrecked copy
//!   finishes is a generation whose answers mean nothing, and it aborts rather
//!   than being averaged in.
//!
//! Together those are two-sided: the first says the instrument still reads the
//! thing it read before, the second says it can still tell two different runs
//! apart.
//!
//! # The horizon moves with the incumbent, deliberately
//!
//! The validator simulates to the declared time. The declared time is set a
//! few seconds past the incumbent, so a mutant slower than that comes back a
//! DNF. That is not a loss of information — it is below the incumbent either
//! way — and it makes every generation cheaper as the run gets faster.

use std::path::PathBuf;
use tmauto::oracle;
use tmauto::synth::{self, ChunkSet, GhostMeta};
use tmauto::tape::Input;
use tmauto::verdict::Verdict;

use crate::artifact;

fn secs(ms: u32) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// xorshift64*. Deterministic from a seed, so any run here can be repeated
/// exactly — a result that cannot be re-derived is a result we may not keep.
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(if seed == 0 { 0xC2C2C2 } else { seed })
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % ((hi - lo).max(1) as u64)) as i32
    }
}

/// One mutation of the incumbent, over a window of the DRIVEN region.
///
/// The operators are deliberately coarse and few. A branch node in the fork
/// search cost 6.953 ms of which 4.220 ms was a boundary probe flat in `k`, so
/// that search wanted few long macros; here a node costs a container write and
/// a share of a server launch, and the same conclusion holds for a different
/// reason — the marginal eval is ~4 ms and the write is not free, so a
/// generation should be large and each mutation should be worth a whole
/// generation's wait.
fn mutate(base: &[Input], active: usize, focus: Option<(usize, usize)>, rng: &mut Rng) -> Vec<Input> {
    let mut v = base.to_vec();
    let n = active.min(v.len());
    if n < 32 {
        return v;
    }
    // One to three operators per candidate. A single local edit is the right
    // move while the climb is finding easy ground and the wrong one once it
    // plateaus: an open-loop tape is a chain, and a gain at tick t
    // desynchronises everything after t, so the edit that fixes it has to be
    // in the same candidate.
    let nops = 1 + rng.below(3);
    for _ in 0..nops {
        apply_one(&mut v, n, focus, rng);
    }
    v
}

fn apply_one(v: &mut [Input], n: usize, focus: Option<(usize, usize)>, rng: &mut Rng) {
    let op = rng.below(8);
    // WHERE to edit. `tmauto splits` measures a mean speed per segment; on the
    // first drive of Summer 2026 - 01 three segments ran at 61-64 m/s and one
    // ran at 35.5, eating 17 s of a 36.7 s lap. Spending most of the budget
    // there is the point of measuring the splits.
    //
    // Not ALL of it: an edit inside the window desynchronises everything after
    // it, so a quarter of the edits stay global to repair the downstream. A
    // focus that never leaves its window optimises a segment of a run that
    // then fails to finish.
    let a = match focus {
        Some((lo, hi)) if rng.below(4) != 0 => {
            let lo = lo.min(n.saturating_sub(4));
            let hi = hi.min(n.saturating_sub(4)).max(lo + 1);
            lo + rng.below(hi - lo)
        }
        _ => rng.below(n.saturating_sub(4)),
    };
    // TAIL RESOLVE, and it is two of the eight because it is the operator that
    // has already earned its place: `tailsearch` found the first finish on this
    // map by doing exactly this — cut the tape and hold one steer to the end —
    // and it took 45.573 out of a tape that had never crossed the line. It is
    // also the only operator that repairs the desynchronisation an earlier edit
    // causes, because it re-solves everything downstream of a point in one go.
    if op >= 6 {
        let s = rng.range(-40, 41) as i8;
        for t in v[a..].iter_mut() {
            t.steer = s;
            t.gas = true;
            t.brake = false;
        }
        return;
    }
    let len = match op {
        2 | 3 => rng.range(3, 40) as usize,
        _ => rng.range(4, 240) as usize,
    };
    let b = (a + len).min(n);
    match op {
        0 => {
            // hold one steer value across the window
            let s = rng.range(-128, 128) as i8;
            for t in &mut v[a..b] {
                t.steer = s;
            }
        }
        1 => {
            // nudge the whole window, keeping its shape
            let d = rng.range(-28, 29);
            for t in &mut v[a..b] {
                t.steer = (t.steer as i32 + d).clamp(-128, 127) as i8;
            }
        }
        2 => {
            // a short brake
            for t in &mut v[a..b] {
                t.brake = true;
                t.gas = false;
            }
        }
        3 => {
            // a short lift
            for t in &mut v[a..b] {
                t.gas = false;
                t.brake = false;
            }
        }
        4 => {
            // full throttle across the window, steering untouched
            for t in &mut v[a..b] {
                t.gas = true;
                t.brake = false;
            }
        }
        _ => {
            // ramp between the endpoints: the crude tapes this starts from are
            // full of steer discontinuities and a Stadium car pays for them
            let s0 = v[a].steer as i32;
            let s1 = v[b.saturating_sub(1)].steer as i32;
            let w = (b - a).max(1) as i32;
            for (i, t) in v[a..b].iter_mut().enumerate() {
                t.steer = (s0 + (s1 - s0) * i as i32 / w).clamp(-128, 127) as i8;
            }
        }
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let apath = PathBuf::from(arg(args, "--artifact").ok_or("--artifact is required")?);
    let bank = PathBuf::from(arg(args, "--bank").ok_or("--bank is required")?);
    let work = PathBuf::from(arg(args, "--work").unwrap_or_else(|| "/tmp/c2/optwork".into()));
    let lambda: usize = arg(args, "--lambda").unwrap_or_else(|| "1200".into()).parse().map_err(|_| "--lambda")?;
    let jobs: usize = arg(args, "--jobs").unwrap_or_else(|| "40".into()).parse().map_err(|_| "--jobs")?;
    // A measured that a server launch costs 2.68 s and a marginal eval ~4.3 ms,
    // so bigger batches amortise better — but that is only true while there are
    // enough batches to fill the jobs. The first run of this optimiser used
    // lambda 1200 at 600 per launch, which is THREE batches over forty workers:
    // 12.8 evals/s, with 37 cores idle. The default now fills every job; the
    // flag stays so it can be re-measured rather than believed.
    let per_launch: usize = match arg(args, "--per-launch") {
        Some(s) => s.parse().map_err(|_| "--per-launch")?,
        None => ((lambda + 2) / jobs.max(1)).max(1),
    };
    let minutes: u64 = arg(args, "--minutes").unwrap_or_else(|| "30".into()).parse().map_err(|_| "--minutes")?;
    let mut seed: u64 = arg(args, "--seed").unwrap_or_else(|| "0xC2C2C2".into()).trim_start_matches("0x").parse().unwrap_or(0xC2C2C2);
    if seed == 0 {
        seed = 0xC2C2C2;
    }
    std::fs::create_dir_all(&bank).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;

    let (h, inputs0) = artifact::read_artifact(&apath)?;
    let map_bytes = std::fs::read(&map).map_err(|e| e.to_string())?;
    if tmauto::sha::sha256_hex(&map_bytes) != h.map_sha256 {
        return Err("the artifact was produced against a different map file. Refusing.".into());
    }
    let uid = h.map_uid.clone();

    // The archive can be shortened once the run is comfortably inside it, and
    // it is worth doing: every candidate is synthesized and written before any
    // server starts, so the container length is a direct multiplier on the
    // slowest serial part of a generation. It is a REFUSAL rather than a clamp
    // if the horizon would not fit — a silently shortened archive stops the run
    // early and reports a DNF, which reads as bad driving.
    let (inputs0, h) = match arg(args, "--ticks") {
        None => (inputs0, h),
        Some(s) => {
            let n: usize = s.parse().map_err(|_| "--ticks")?;
            if n > inputs0.len() {
                return Err(format!(
                    "--ticks {} is longer than the artifact's {} — this truncates, it does not extend",
                    n,
                    inputs0.len()
                ));
            }
            let d = h.declared_ms.min((n as u32) * 10 - 200);
            let mut h2 = h;
            h2.declared_ms = d;
            h2.declared_cps = vec![d as i32 / 2, d as i32];
            h2.container_ticks = n;
            (inputs0[..n].to_vec(), h2)
        }
    };

    // Establish the incumbent's time through this code path before optimising
    // anything: an optimiser started from a number it inherited rather than
    // measured is tuning against a car that may not exist.
    let ticks = inputs0.len();
    let horizon0 = h.declared_ms;
    let first = eval_batch(&map, &uid, &[inputs0.clone()], ticks, jobs, per_launch, horizon0, &work)?;
    let mut best_v = first[0].ok_or("the incumbent was not simulated at all — refusing to optimise from an unmeasured baseline")?;
    let mut best = inputs0.clone();
    let Verdict::Finish { ms: mut best_ms } = best_v else {
        return Err(format!(
            "the incumbent does not finish through this path ({:?}). This optimiser hill-climbs \
             on finish time and has no objective without one.",
            best_v
        ));
    };
    println!("INCUMBENT  {} (re-simulated here, not inherited)", secs(best_ms));
    println!("MAP        {}  ({})", map.display(), uid);
    println!("CONTAINER  {} ticks, {} per launch, {} jobs, lambda {}", ticks, per_launch, jobs, lambda);
    println!("SEED       {:#x}\n", seed);

    let focus: Option<(usize, usize)> = arg(args, "--focus").map(|s| {
        let mut it = s.split(',');
        (
            it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0),
            it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0),
        )
    });
    if let Some((a, b)) = focus {
        println!("FOCUS      ticks {}..{} get three quarters of the edits (from `tmauto splits`)\n", a, b);
    }

    let mut rng = Rng::new(seed);
    let t0 = std::time::Instant::now();
    let mut gen = 0u64;
    let mut evals = 0u64;
    let mut improvements = 0u64;

    while t0.elapsed().as_secs() < minutes * 60 {
        gen += 1;
        // The horizon rides just above the incumbent. Anything slower is worse
        // than the incumbent anyway, so cutting it off loses no ordering.
        let horizon = (best_ms + 4000).min((ticks as u32) * 10 - 200);
        let active = ((best_ms / 10) as usize + 200).min(ticks);

        let mut cands: Vec<Vec<Input>> = Vec::with_capacity(lambda + 2);
        cands.push(best.clone()); // control 0: the standing re-simulation
        let mut wrecked = best.clone();
        for t in wrecked.iter_mut().skip(300) {
            t.steer = -128;
        }
        cands.push(wrecked); // control 1: must NOT finish
        for _ in 0..lambda {
            cands.push(mutate(&best, active, focus, &mut rng));
        }

        let evs = eval_batch(&map, &uid, &cands, ticks, jobs, per_launch, horizon, &work)?;
        evals += cands.len() as u64;

        match evs[0] {
            Some(Verdict::Finish { ms }) if ms == best_ms => {}
            other => {
                return Err(format!(
                    "STANDING CONTROL FAILED at generation {}: the incumbent re-simulated to \
                     {:?} and it was {}. The instrument has changed under us; stopping rather \
                     than continuing to collect answers from it.",
                    gen,
                    other,
                    secs(best_ms)
                ));
            }
        }
        if let Some(Verdict::Finish { ms }) = evs[1] {
            return Err(format!(
                "NEGATIVE CONTROL FAILED at generation {}: the incumbent with hard left held \
                 from tick 300 finished in {}. A batch that cannot tell a wrecked run from a \
                 good one certifies nothing; stopping.",
                gen,
                secs(ms)
            ));
        }

        let mut moved = false;
        for (i, e) in evs.iter().enumerate().skip(2) {
            if let Some(v) = e {
                if *v > best_v {
                    best_v = *v;
                    best = cands[i].clone();
                    if let Verdict::Finish { ms } = v {
                        best_ms = *ms;
                    }
                    moved = true;
                }
            }
        }

        if moved {
            improvements += 1;
            let name = format!("summer01-finish-{}.artifact.tsv", best_ms);
            write_artifact(&bank.join(&name), &h, &best, best_ms, &apath)?;
            println!(
                "[{:>5}s] gen {:>4}  {:>8} evals   *** {} ***  banked {}",
                t0.elapsed().as_secs(),
                gen,
                evals,
                secs(best_ms),
                name
            );
        } else {
            println!(
                "[{:>5}s] gen {:>4}  {:>8} evals ({:.0}/s)  best {}  improvements {}",
                t0.elapsed().as_secs(),
                gen,
                evals,
                evals as f64 / t0.elapsed().as_secs_f64().max(0.001),
                secs(best_ms),
                improvements
            );
        }
    }

    println!(
        "\nDONE  {} generations, {} evals, {} improvements, best {}",
        gen, evals, improvements, secs(best_ms)
    );
    println!("      every improvement is banked in {} as its own self-contained artifact.", bank.display());
    println!("      Verify any of them from scratch:  tmauto artifact replay --artifact <A> --map <MAP>");
    Ok(())
}

fn eval_batch(
    map: &std::path::Path,
    _uid: &str,
    tapes: &[Vec<Input>],
    ticks: usize,
    jobs: usize,
    per_launch: usize,
    declared: u32,
    work: &std::path::Path,
) -> Result<Vec<Option<Verdict>>, String> {
    let evs = oracle::evaluate_declared(map, tapes, ticks, jobs, per_launch, declared, work)?;
    Ok(evs.into_iter().map(|e| e.map(|e| e.verdict)).collect())
}

fn write_artifact(
    out: &std::path::Path,
    h: &artifact::Header,
    inputs: &[Input],
    ms: u32,
    parent_path: &std::path::Path,
) -> Result<(), String> {
    // Rebuild with the SAME declared time the artifact records, so the file the
    // artifact names is the file a replay produces. The optimiser's own moving
    // horizon is a search device and never leaks into a banked result.
    let mut meta = GhostMeta::probe(&h.map_uid);
    meta.set_declared(h.declared_ms, h.declared_cps.clone());
    let bytes = synth::synthesize(inputs, &meta, &ChunkSet::ALL);
    let file_sha = tmauto::sha::sha256_hex(&bytes);
    let mut enc = Vec::with_capacity(inputs.len() * 4);
    for i in inputs {
        enc.push(i.steer as u8);
        enc.push(i.gas as u8);
        enc.push(i.brake as u8);
        enc.push(i.respawn as u8);
    }
    let mut s = String::new();
    s.push_str(artifact::MAGIC);
    s.push('\n');
    s.push_str(&format!("#map_uid {}\n", h.map_uid));
    s.push_str(&format!("#map_sha256 {}\n", h.map_sha256));
    s.push_str(&format!("#container_ticks {}\n", inputs.len()));
    s.push_str(&format!("#prefix {}\n", h.prefix));
    s.push_str(&format!("#declared_ms {}\n", h.declared_ms));
    s.push_str(&format!(
        "#declared_cps {}\n",
        h.declared_cps.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")
    ));
    s.push_str(&format!("#template {}\n", h.template));
    s.push_str(&format!("#tape_sha256 {}\n", tmauto::sha::sha256_hex(&enc)));
    s.push_str(&format!("#file_sha256 {}\n", file_sha));
    s.push_str("#producer tmauto optimize\n");
    s.push_str(&format!("#parent {}\n", h.file_sha256));
    s.push_str(&format!("#parent_path {}\n", parent_path.display()));
    s.push_str("#cut n/a\n#macro_steer n/a\n");
    s.push_str(&format!(
        "#note (1+lambda) hill climb on finish time, plain oracle only, no memory readout; \
         server-simulated finish {} at the moment of banking\n",
        secs(ms)
    ));
    s.push_str("tick\tsteer\tgas\tbrake\trespawn\n");
    for (i, t) in inputs.iter().enumerate() {
        s.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", i, t.steer, t.gas as u8, t.brake as u8, t.respawn as u8));
    }
    std::fs::write(out, s).map_err(|e| e.to_string())
}

// ---- what `pushgate` needs from this module ----
//
// Re-exported rather than copied. Two implementations of one mutation operator
// is how this project got silent divergence before.
pub fn mutate_pub(base: &[Input], active: usize, focus: Option<(usize, usize)>, rng: &mut Rng) -> Vec<Input> {
    mutate(base, active, focus, rng)
}

pub fn write_artifact_pub(
    out: &std::path::Path,
    h: &artifact::Header,
    inputs: &[Input],
    ms: u32,
    parent_path: &std::path::Path,
) -> Result<(), String> {
    write_artifact(out, h, inputs, ms, parent_path)
}
