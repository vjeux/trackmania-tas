//! `fk ladder` — deep fork points (`forkoracle::ladder`) against the root fork
//! and the plain oracle, on the real engine.
//!
//! * `check` — *does a candidate forked from a node deep in the tape get the
//!   answer the root fork and a full validation get?* Every candidate is run
//!   three ways and the three must agree on the time and, for a DNF, on the
//!   checkpoint count. The candidate set perturbs the reference at a position
//!   drawn uniformly over the whole tape, so the ladder is exercised at every
//!   depth, and the cost is reported per depth band -- which is the number the
//!   ladder exists for.
//!
//! The mechanism itself (a node consumes exactly the records the root would
//! have; a node of another lineage is never used) is proven against `shimhost`
//! in `forkshim/tests/ladder.rs` on every `cargo test`. This is the engine's
//! word on the same question, plus the cost.

use crate::cmd::server::{make_candidate, Rng};
use crate::oracle::validate_batch;
use crate::session::{Checkpoint, Engine, Session};
use crate::tape::Tape;
use forkoracle::forksrv::{parse_result, rec_of, Rec};
use forkoracle::inputs::Inputs;
use forkoracle::ladder::Ladder;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct CheckOpts {
    pub n: usize,
    pub seed: u64,
    pub span: usize,
    pub spacing: usize,
    pub cap: usize,
}

fn records_from(steer: &[u8], accel: &[u8], brake: &[u8], from: usize) -> Vec<Rec> {
    (from..steer.len()).map(|t| rec_of(steer[t], accel[t], brake[t])).collect()
}

fn full_times(engine: &Engine, files: &[PathBuf], tag: &str) -> Result<HashMap<String, (Option<i64>, Option<u32>)>, String> {
    let refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
    Ok(validate_batch(&engine.server, &engine.map, &refs, tag)?
        .into_iter()
        .map(|r| (r.file, (r.time_ms, r.cps)))
        .collect())
}

fn show(v: (Option<i64>, Option<u32>)) -> String {
    match v {
        (Some(t), _) => crate::secs(t),
        (None, Some(c)) => format!("DNF cps={}", c),
        (None, None) => "DNF".into(),
    }
}

/// The depth bands the cost is reported in, as fractions of the editable tape.
const BANDS: [(f64, f64); 5] = [(0.0, 0.25), (0.25, 0.5), (0.5, 0.75), (0.75, 0.9), (0.9, 1.01)];

pub fn check(engine: &Engine, tape: Tape, at: Checkpoint, o: CheckOpts) -> Result<bool, String> {
    engine.check()?;
    std::fs::create_dir_all(&engine.work).map_err(|e| e.to_string())?;
    let refp = engine.work.join("reference.Ghost.Gbx");
    tape.write_reference(&refp)?;
    let n = tape.n();
    let ref_time = full_times(engine, &[refp.clone()], "ref")?.values().next().cloned().unwrap_or((None, None));
    println!("reference: {} ticks, oracle says {}", n, show(ref_time));

    let mut s = Session::start(engine, tape, at)?;
    // THE ROOT'S OWN TICK, from the probe and the hook agreeing.
    let root_tick = s.probe_tick()?;
    let from = root_tick;
    println!(
        "fork server up: input array {:#x}, checkpoint clock #{}, root tick {} (race {})",
        s.srv.base,
        s.checkpoint_clock,
        root_tick,
        crate::secs(s.tape.race_ms(root_tick))
    );

    // Candidates, spread over the WHOLE editable tape.
    let mut rng = Rng::new(o.seed);
    let reference = Inputs::from_arrays(&s.tape.steer, &s.tape.accel, &s.tape.brake);
    let mut cands: Vec<(PathBuf, Inputs, usize)> = Vec::new();
    for i in 0..o.n {
        let (st, ac, br) = make_candidate(&s.tape, from, o.span, &mut rng);
        let p = engine.work.join(format!("c{:04}.Ghost.Gbx", i));
        s.tape.write_candidate(&st, &ac, &br, &p)?;
        let inp = Inputs::from_arrays(&st, &ac, &br);
        let first = inp.distance_from(&reference).first_diff_tick.unwrap_or(n);
        cands.push((p, inp, first));
    }
    let editable = (n - from).max(1) as f64;
    let depth = |first: usize| (first.saturating_sub(from) as f64 / editable).min(1.0);
    {
        let mut ds: Vec<usize> = cands.iter().map(|c| c.2).collect();
        ds.sort_unstable();
        println!(
            "candidates: {} with a first edit at tick {} .. {} (median {}); grid every {} ticks from {}, cap {}",
            o.n,
            ds.first().copied().unwrap_or(0),
            ds.last().copied().unwrap_or(0),
            ds.get(ds.len() / 2).copied().unwrap_or(0),
            o.spacing,
            from,
            o.cap
        );
    }
    let files: Vec<PathBuf> = cands.iter().map(|c| c.0.clone()).collect();

    // THE PLAIN ORACLE, twice: a comparison against an oracle that disagrees
    // with itself is not a comparison.
    let tg = Instant::now();
    let full = full_times(engine, &files, "full")?;
    let full_secs = tg.elapsed().as_secs_f64();
    let again = full_times(engine, &files, "again")?;
    let unstable = full.iter().filter(|(k, v)| again.get(*k).cloned() != Some(**v)).count();
    println!("oracle repeatability: {} of {} candidates differ between two full runs", unstable, o.n);

    // THE ROOT FORK: every candidate from the checkpoint, as the search did.
    let mut root_res = Vec::with_capacity(o.n);
    let mut root_ms = Vec::with_capacity(o.n);
    for (_, inp, _) in &cands {
        let t0 = Instant::now();
        let out = s.srv.run(from, &records_from(&inp.steer_u8(), &inp.gas_u8(), &inp.brake_u8(), from));
        root_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        root_res.push(parse_result(&out));
    }

    // THE LADDER: the same candidates, in the same order, each from the deepest
    // node that agrees with it. `prepare` is told the candidate's own first
    // edit as the prefix it "shares" -- in the search that is the batch's
    // common prefix; here each candidate is its own batch of one, and the grid
    // fills in as candidates land in new cells.
    let mut ladder = Ladder::new(&engine.work.join("ladder"), root_tick, from, o.spacing, o.cap, false)?;
    let mut lad_res = Vec::with_capacity(o.n);
    let mut lad_ms = Vec::with_capacity(o.n);
    let mut lad_at = Vec::with_capacity(o.n);
    let mut prep_ms = 0.0f64;
    for (_, inp, first) in &cands {
        let t0 = Instant::now();
        ladder.prepare(&mut s.srv, inp, *first);
        prep_ms += t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = Instant::now();
        let (out, at) = ladder.run(&mut s.srv, inp);
        lad_ms.push(t1.elapsed().as_secs_f64() * 1000.0);
        lad_res.push(parse_result(&out));
        lad_at.push(at);
    }
    let st = ladder.stats();

    // EXACTNESS, three ways.
    let (mut ok, mut bad, mut finished) = (0usize, 0usize, 0usize);
    for (i, (p, _, first)) in cands.iter().enumerate() {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let g = full.get(&name).cloned().unwrap_or((None, None));
        if g.0.is_some() {
            finished += 1;
        }
        let r = root_res[i];
        let l = lad_res[i];
        let same_time = r.0 == g.0 && l.0 == g.0;
        // A DNF's checkpoint count is compared too: root and ladder always
        // (same parser), and against the plain oracle when it reported one.
        let same_cps = g.0.is_some()
            || match g.1 {
                Some(c) => r.1 == Some(c) && l.1 == Some(c),
                None => r.1 == l.1,
            };
        if same_time && same_cps {
            ok += 1;
        } else {
            bad += 1;
            if bad <= 5 {
                println!(
                    "MISMATCH {} (edit at tick {}, forked at {}): full {}  root {}  ladder {}",
                    name,
                    first,
                    lad_at[i],
                    show(g),
                    show(r),
                    show(l)
                );
            }
        }
    }
    println!(
        "exactness: {}/{} identical across full validation, root fork and ladder ({} finished, {} DNF), {} MISMATCHES",
        ok, o.n, finished, o.n - finished, bad
    );

    // COST, per depth band.
    println!("\n  first edit at        n    root ms   ladder ms   speedup   forked at (mean tick)");
    for (lo, hi) in BANDS {
        let idx: Vec<usize> = (0..o.n).filter(|&i| { let d = depth(cands[i].2); d >= lo && d < hi }).collect();
        if idx.is_empty() {
            continue;
        }
        let m = idx.len() as f64;
        let r: f64 = idx.iter().map(|&i| root_ms[i]).sum::<f64>() / m;
        let l: f64 = idx.iter().map(|&i| lad_ms[i]).sum::<f64>() / m;
        let a: f64 = idx.iter().map(|&i| lad_at[i] as f64).sum::<f64>() / m;
        println!(
            "  {:>3.0}-{:<3.0}% of tape  {:>4}   {:>7.2}   {:>9.2}   {:>6.2}x   {:>8.0}",
            lo * 100.0,
            (hi.min(1.0)) * 100.0,
            idx.len(),
            r,
            l,
            r / l.max(1e-9),
            a
        );
    }
    let root_total: f64 = root_ms.iter().sum();
    let lad_total: f64 = lad_ms.iter().sum();
    println!(
        "\nall {} candidates: full {:.1} ms/cand | root fork {:.2} ms/cand | ladder {:.2} ms/cand run + {:.2} ms/cand making nodes = {:.2} | speedup {:.2}x over the root fork",
        o.n,
        1000.0 * full_secs / o.n as f64,
        root_total / o.n as f64,
        lad_total / o.n as f64,
        prep_ms / o.n as f64,
        (lad_total + prep_ms) / o.n as f64,
        root_total / (lad_total + prep_ms).max(1e-9)
    );
    println!("ladder: {}; {} nodes live at the end at ticks {:?}", st, ladder.live(), ladder.rung_ticks());
    drop(ladder);
    s.srv.quit();
    Ok(bad == 0 && unstable == 0)
}
