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
    let mut root_census = Vec::with_capacity(o.n);
    let mut root_gaps = Vec::with_capacity(o.n);
    for (_, inp, _) in &cands {
        let t0 = Instant::now();
        let out = s.srv.run(from, &records_from(&inp.steer_u8(), &inp.gas_u8(), &inp.brake_u8(), from));
        root_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        root_res.push(parse_result(&out));
        root_census.push(census_of(&out));
        root_gaps.push(gaps_of(&out));
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
    let mut lad_census = Vec::with_capacity(o.n);
    let mut lad_gaps = Vec::with_capacity(o.n);
    let mut prep_ms = 0.0f64;
    for (_, inp, first) in &cands {
        let t0 = Instant::now();
        ladder.prepare(&mut s.srv, inp, *first);
        prep_ms += t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = Instant::now();
        let (out, at) = ladder.run(&mut s.srv, inp);
        lad_ms.push(t1.elapsed().as_secs_f64() * 1000.0);
        lad_res.push(parse_result(&out));
        lad_census.push(census_of(&out));
        lad_gaps.push(gaps_of(&out));
        lad_at.push(at);
    }
    let st = ladder.stats();

    // EXACTNESS, three ways -- and one class kept apart.
    //
    // A finish whose millisecond is PAST THE TAPE'S OWN END (the engine drives
    // on past the last record, on heap contents) is not a finish: the shim
    // reports it as a DNF (TICKHOOK.md, the exhausted word), the plain oracle
    // reports a time that depends on what else was in its batch (PERF.md §1).
    // Root and ladder must still agree with each other there; against the
    // plain oracle that class is reported, not counted.
    let tape_end_ms = s.tape.start_offset_ms as i64 + 10 * n as i64;
    let (mut ok, mut bad, mut finished, mut past_end, mut past_end_split) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for (i, (p, _, first)) in cands.iter().enumerate() {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let g = full.get(&name).cloned().unwrap_or((None, None));
        let r = root_res[i];
        let l = lad_res[i];
        // The DNF checkpoint count: root and ladder always agree (same
        // parser, same counter); against the plain oracle only when it
        // reported one -- and since the fork reads the engine's counter the
        // oracle's Desc line is a LOWER BOUND, so >= is the test there.
        let fork_same = r.0 == l.0 && (r.0.is_some() || r.1 == l.1);
        if let Some(t) = g.0 {
            if t > tape_end_ms {
                past_end += 1;
                if !fork_same {
                    past_end_split += 1;
                }
                if past_end <= 3 {
                    println!(
                        "after its own tape: {} (edit at tick {}) -- plain oracle {} past the tape's end at {}; root {}  ladder {}{}",
                        name,
                        first,
                        crate::secs(t),
                        crate::secs(tape_end_ms),
                        show(r),
                        show(l),
                        if fork_same { "" } else { "  ROOT AND LADDER DISAGREE" }
                    );
                }
                continue;
            }
            finished += 1;
        }
        let same_time = r.0 == g.0 && l.0 == g.0;
        let same_cps = g.0.is_some()
            || match g.1 {
                Some(c) => r.1.map(|x| x >= c).unwrap_or(false) && l.1 == r.1,
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
        "exactness: {}/{} identical across full validation, root fork and ladder ({} finished, {} DNF), {} MISMATCHES; {} more finished AFTER their own tape ended (kept apart: the plain oracle's verdict there is batch-dependent), root and ladder disagreeing on {} of them",
        ok,
        o.n - past_end,
        finished,
        o.n - past_end - finished,
        bad,
        past_end,
        past_end_split
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
    if let (Some(r), Some(l)) = (census_mean(&root_census), census_mean(&lad_census)) {
        println!(
            "census (FKSHIM_CENSUS): a root child faulted {:.0} pages, resident {:.1} MB, dirtied {:.1} MB; a ladder child faulted {:.0} pages, resident {:.1} MB, dirtied {:.1} MB",
            r.0, r.1 / 1024.0, r.2 / 1024.0, l.0, l.1 / 1024.0, l.2 / 1024.0
        );
    }
    if let (Some(r), Some(l)) = (census_mean(&root_gaps), census_mean(&lad_gaps)) {
        println!(
            "inter-tick gaps > 80 us: a root child had {:.1} of them, {:.0} us in excess of 80 in total, the largest {:.0} us; a ladder child {:.1} / {:.0} us / {:.0} us",
            r.0, r.1, r.2, l.0, l.1, l.2
        );
    }
    drop(ladder);
    s.srv.quit();
    Ok(bad == 0 && unstable == 0 && past_end_split == 0)
}

// ---------------------------------------------------------------- fk ladder watched

pub struct WatchedOpts {
    pub n: usize,
    pub seed: u64,
    pub span: usize,
    pub spacing: usize,
    pub cap: usize,
    /// The reference line, as `fk trace` writes it.
    pub refcsv: String,
    /// The search's shipped predicate set unless told otherwise.
    pub preds: Vec<String>,
    pub finishmargin: f32,
}

/// THE WATCHED CONTROL: root `'W'` against warm-node `'W'`, summary for
/// summary, byte for byte.
///
/// The watchdog is stateful -- a speed window, consecutive-tick counters,
/// progress along the line, the gate and event records -- so a fork point deep
/// in the tape only returns the root's verdict if the node it forks from
/// carries the watchdog's state up to its own tick. That is what
/// `BranchReq::watched` is for, and this is the measurement that says it
/// works: the same candidates, the same armed predicates, one run from the
/// checkpoint and one from the deepest node that agrees with the candidate,
/// and the 148-byte summaries (trip, tick, value, progress, travelled, speeds,
/// gate, event, plane crossing) must be IDENTICAL, as must the time.
pub fn watched(engine: &Engine, tape: Tape, at: Checkpoint, o: WatchedOpts) -> Result<bool, String> {
    use forkoracle::blind::{bounds_from, locate_blind};
    use forkoracle::layout::{segments, tail_recs, Row, REC_LEN, R_CLOCK, R_POS, R_QUAT, R_VEL};
    use forkoracle::pred::{outcome, parse_spec, RefLineData, Watch};
    use forkoracle::pred_core::SUMMARY_BYTES;

    engine.check()?;
    std::fs::create_dir_all(&engine.work).map_err(|e| e.to_string())?;
    let refp = engine.work.join("reference.Ghost.Gbx");
    tape.write_reference(&refp)?;
    let n = tape.n();
    let ref_time = full_times(engine, &[refp.clone()], "ref")?.values().next().cloned().unwrap_or((None, None)).0;
    println!("reference: {} ticks, oracle says {}", n, crate::secs_opt(ref_time));

    let mut s = Session::start(engine, tape, at)?;
    let root_tick = s.probe_tick()?;
    let from = root_tick;
    let start_offset_ms = s.tape.start_offset_ms;
    println!(
        "fork server up: input array {:#x}, checkpoint clock #{}, root tick {} (race {})",
        s.srv.base, s.checkpoint_clock, root_tick, crate::secs(s.tape.race_ms(root_tick))
    );

    // The reference line, the car, the watchdog: armed exactly as the search
    // arms them (`tmsearch::forkeval::ForkEval::start`).
    let refline = RefLineData::from_csv(&o.refcsv, start_offset_ms, n)?;
    let rows: Vec<Row> = (0..refline.n)
        .map(|i| Row {
            time_ms: 0,
            x: refline.xyz[3 * i] as f64,
            y: refline.xyz[3 * i + 1] as f64,
            z: refline.xyz[3 * i + 2] as f64,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            qx: 0.0,
            qy: 0.0,
            qz: 0.0,
            qw: 0.0,
            wetness: 0.0,
        })
        .collect();
    let bounds = bounds_from(&rows, 200.0);
    let lrecs = tail_recs(&s.tape.steer, &s.tape.accel, &s.tape.brake, from);
    let layout = locate_blind(&mut s.srv, from, &lrecs, start_offset_ms, 1, bounds, false)
        .map_err(|e| format!("the car's state was not located: {}", e))?;
    if let Some(ms) = ref_time {
        match forkoracle::finish::calibrate(&mut s.srv, from, &lrecs, ms) {
            Ok((addr, _)) => println!("exit-at-finish armed on {:#x}", addr),
            Err(e) => println!("exit-at-finish not armed ({})", e),
        }
    }
    let mut watch = Watch::new();
    watch.corridor = 40.0;
    watch.refline = refline;
    watch.finish_s = match ref_time {
        Some(t) => {
            let tick = ((t - start_offset_ms as i64) / 10).max(0) as usize;
            (watch.refline.s_at_tick(tick) - o.finishmargin).max(1.0)
        }
        None => 0.0,
    };
    for p in &o.preds {
        watch.preds.push(parse_spec(p)?);
    }
    print!("{}", watch.describe());
    let ack = s.srv.arm(&watch.arm_payload(
        layout.clock_bias + start_offset_ms as i64,
        R_CLOCK as u32,
        R_QUAT as u32,
        R_POS as u32,
        R_VEL as u32,
        REC_LEN as u32,
        &segments(&layout),
    ));
    if !ack.starts_with("ARMED") {
        return Err(format!("arming the watchdog failed: {}", ack));
    }
    println!("arm: {}", ack.trim());

    let mut rng = Rng::new(o.seed);
    let reference = Inputs::from_arrays(&s.tape.steer, &s.tape.accel, &s.tape.brake);
    let mut cands: Vec<(Inputs, usize)> = Vec::new();
    for _ in 0..o.n {
        let (st, ac, br) = make_candidate(&s.tape, from, o.span, &mut rng);
        let inp = Inputs::from_arrays(&st, &ac, &br);
        let first = inp.distance_from(&reference).first_diff_tick.unwrap_or(n);
        cands.push((inp, first));
    }

    // Root, watched.
    let mut root: Vec<(Option<i64>, Vec<u8>, f64)> = Vec::with_capacity(o.n);
    for (inp, _) in &cands {
        let t0 = Instant::now();
        let (j, b) = s.srv.run_watched(from, &records_from(&inp.steer_u8(), &inp.gas_u8(), &inp.brake_u8(), from));
        let dt = t0.elapsed().as_secs_f64() * 1000.0;
        root.push((outcome(&j, &b).time, b, dt));
    }
    // Ladder, watched, warm nodes.
    let mut ladder = Ladder::new(&engine.work.join("ladder"), root_tick, from, o.spacing, o.cap, true)?;
    let mut lad: Vec<(Option<i64>, Vec<u8>, f64, usize)> = Vec::with_capacity(o.n);
    let mut prep_ms = 0.0;
    for (inp, first) in &cands {
        let t0 = Instant::now();
        ladder.prepare(&mut s.srv, inp, *first);
        prep_ms += t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = Instant::now();
        let (j, b, at) = ladder.run_watched(&mut s.srv, inp);
        let dt = t1.elapsed().as_secs_f64() * 1000.0;
        lad.push((outcome(&j, &b).time, b, dt, at));
    }

    let (mut same, mut diff, mut trips, mut finished, mut deep) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for i in 0..o.n {
        let r = &root[i];
        let l = &lad[i];
        let ro = outcome("", &r.1);
        if ro.tripped().is_some() {
            trips += 1;
        }
        if r.0.is_some() {
            finished += 1;
        }
        if l.3 > from {
            deep += 1;
        }
        if r.0 == l.0 && r.1[..SUMMARY_BYTES.min(r.1.len())] == l.1[..SUMMARY_BYTES.min(l.1.len())] {
            same += 1;
        } else {
            diff += 1;
            if diff <= 5 {
                let lo = outcome("", &l.1);
                println!(
                    "DIFFERENT candidate {} (edit at tick {}, forked at {}): root time {} trip {:?} progress {:.2} travelled {:.2} nticks {} | ladder time {} trip {:?} progress {:.2} travelled {:.2} nticks {}",
                    i,
                    cands[i].1,
                    l.3,
                    crate::secs_opt(r.0),
                    ro.tripped(),
                    ro.progress(),
                    ro.travelled(),
                    ro.sum.map(|s| s.nticks).unwrap_or(0),
                    crate::secs_opt(l.0),
                    lo.tripped(),
                    lo.progress(),
                    lo.travelled(),
                    lo.sum.map(|s| s.nticks).unwrap_or(0),
                );
            }
        }
    }
    println!(
        "watched equivalence: {}/{} candidates with IDENTICAL time and byte-identical {}-byte summary ({} tripped, {} finished, {} forked deeper than the root), {} DIFFERENT",
        same, o.n, SUMMARY_BYTES, trips, finished, deep, diff
    );
    let rt: f64 = root.iter().map(|r| r.2).sum::<f64>() / o.n as f64;
    let lt: f64 = lad.iter().map(|l| l.2).sum::<f64>() / o.n as f64;
    println!(
        "cost: root {:.2} ms/cand | ladder {:.2} ms/cand run + {:.2} making nodes | speedup {:.2}x",
        rt,
        lt,
        prep_ms / o.n as f64,
        rt / (lt + prep_ms / o.n as f64).max(1e-9)
    );
    println!("ladder: {}", ladder.stats());
    drop(ladder);
    s.srv.quit();
    Ok(diff == 0)
}

/// The census fields the shim appends to FKTIME under `FKSHIM_CENSUS=1`:
/// `(minflt, rss_kb, private_dirty_kb)`.
fn census_of(out: &str) -> Option<(u64, u64, u64)> {
    let line = out.lines().find(|l| l.contains(" minflt "))?;
    let f = |k: &str| -> Option<u64> {
        let i = line.find(k)? + k.len();
        line[i..].split_whitespace().next()?.parse().ok()
    };
    Some((f(" minflt ")?, f(" rss_kb ")?, f(" pdirty_kb ")?))
}

/// The inter-tick gap fields beside the census: `(gap_big, gap_excess_us, gap_max_us)`.
fn gaps_of(out: &str) -> Option<(u64, u64, u64)> {
    let line = out.lines().find(|l| l.contains(" gap_big "))?;
    let f = |k: &str| -> Option<u64> {
        let i = line.find(k)? + k.len();
        line[i..].split_whitespace().next()?.parse().ok()
    };
    Some((f(" gap_big ")?, f(" gap_excess_us ")?, f(" gap_max_us ")?))
}

/// Mean of the census over a set of replies, when every reply carried one.
fn census_mean(rows: &[Option<(u64, u64, u64)>]) -> Option<(f64, f64, f64)> {
    let v: Vec<(u64, u64, u64)> = rows.iter().filter_map(|r| *r).collect();
    if v.is_empty() {
        return None;
    }
    let n = v.len() as f64;
    Some((
        v.iter().map(|x| x.0 as f64).sum::<f64>() / n,
        v.iter().map(|x| x.1 as f64).sum::<f64>() / n,
        v.iter().map(|x| x.2 as f64).sum::<f64>() / n,
    ))
}
