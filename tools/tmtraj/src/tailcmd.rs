//! `tmtraj tail` -- the post-finish / post-tape carrier tail in a transplanted
//! ghost, measured and removed.
//!
//! ## What this is for
//!
//! Our published ghosts were made by transplanting a validated input tape into
//! a CARRIER ghost (somebody else's recording). The carrier's
//! `CPlugEntRecordData` is usually a LONGER recording than our tape, so after
//! the regeneration pass wrote our engine-sampled telemetry over the carrier's,
//! the samples past the end of our tape kept the CARRIER's position. The car
//! therefore drives our line and then, in the last few frames, TELEPORTS to
//! wherever the carrier's car happened to be.
//!
//! ## The measurement (`tail scan`) does not need to know which samples were
//! regenerated
//!
//! It is a physical-continuity test on the record alone:
//!
//! ```text
//!     excess(i) = |p[i+1] - p[i]|  -  vmax(i) * dt(i)
//! ```
//!
//! where `vmax(i)` is the larger of the two samples' own recorded speeds (the
//! `speedLog` field of the transform, which the carrier and the regenerator
//! both write). Over one 50 ms step a car cannot move further than its own
//! speed allows, so `excess` is metres of displacement the record itself says
//! are impossible. A continuous trajectory scores ~0; a teleport scores metres.
//!
//! This can fail for the right reason: it is computed from two INDEPENDENT
//! fields (the f32 position triple and the i16 log-speed), so a file that is
//! internally consistent -- a genuine human recording, or a correctly
//! regenerated one -- cannot produce a large excess by construction. See
//! `tail controls`, which runs it over the 14 untouched human recordings.
//!
//! ## The fix (`tail fix`)
//!
//! Truncate the vehicle entity's sample list at the last sample the tape can
//! account for. Nothing else in the file is touched -- in particular the input
//! tape is never touched -- so the run still re-simulates to its exact
//! validated millisecond.

use crate::entrec::{decode_vehicle_sample, find_entrecord_blob, load_body, parse_record_data};
use crate::entrec::{read_ghost_result, Ent, RecordData, Res, CLASS_CSCENEVEHICLEVIS};

/// One consecutive-sample step and how much of its displacement the record's
/// own speed field can account for.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub i: usize,
    pub t0: i32,
    pub t1: i32,
    /// metres actually moved between the two samples
    pub dist: f64,
    /// metres the faster of the two recorded speeds allows in that interval
    pub allowed: f64,
    /// dist - allowed, in metres; the unexplainable part
    pub excess: f64,
    /// |p[i+1] - (p[i] + v[i]*dt)| -- how far the second sample sits from where
    /// the first one was heading. THE JUMP, in metres.
    pub gap_fwd: f64,
    /// |p[i] - (p[i+1] - v[i+1]*dt)| -- the same seen from the other side.
    pub gap_bwd: f64,
    /// min(gap_fwd, gap_bwd): the displacement neither sample can account for.
    pub gap: f64,
    /// |Δp - (v[i]+v[i+1])/2 * dt| -- the TRAPEZOID residual, which is exact
    /// for any constant acceleration, so it cancels the curvature that makes
    /// `gap_fwd` nonzero on an honest corner. THE JUMP, in metres.
    pub trap: f64,
}

pub struct Scan {
    pub path: String,
    pub race_ms: Option<i32>,
    pub n: usize,
    pub t_first: i32,
    pub t_last: i32,
    pub period: i32,
    pub start_ms: i32,
    pub end_ms: i32,
    /// samples at a NEGATIVE record time (a pre-race / countdown tail)
    pub n_before_zero: usize,
    /// samples strictly after the race time
    pub n_after_finish: usize,
    /// samples whose position or speed is not finite
    pub n_nan: usize,
    pub first_nan: Option<i32>,
    /// every sample time in the vehicle record
    pub sample_times: Vec<i32>,
    pub steps: Vec<Step>,
}

impl Scan {
    /// The step with the largest unexplainable displacement.
    pub fn worst(&self) -> Option<Step> {
        self.steps
            .iter()
            .copied()
            .max_by(|a, b| a.trap.total_cmp(&b.trap))
    }
    /// Every step whose trapezoid residual is over `thr` metres, in time order.
    pub fn over(&self, thr: f64) -> Vec<Step> {
        self.steps.iter().copied().filter(|s| s.trap > thr).collect()
    }
    /// Median trapezoid residual -- the file's own noise floor.
    pub fn median_trap(&self) -> f64 {
        if self.steps.is_empty() {
            return 0.0;
        }
        let mut v: Vec<f64> = self.steps.iter().map(|s| s.trap).collect();
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2]
    }
    /// How many samples are at or before `t`.
    pub fn times_at_or_before(&self, t: i32) -> usize {
        self.sample_times.iter().filter(|x| **x <= t).count()
    }
    /// Worst trapezoid residual strictly after the race time.
    pub fn post_finish_max(&self) -> f64 {
        match self.race_ms {
            None => f64::NAN,
            Some(r) => self
                .steps
                .iter()
                .filter(|s| s.t1 > r)
                .map(|s| s.trap)
                .fold(0.0f64, f64::max),
        }
    }
    /// 99th-percentile trapezoid residual.
    pub fn p99_trap(&self) -> f64 {
        if self.steps.is_empty() {
            return 0.0;
        }
        let mut v: Vec<f64> = self.steps.iter().map(|s| s.trap).collect();
        v.sort_by(|a, b| a.total_cmp(b));
        v[((v.len() as f64 * 0.99) as usize).min(v.len() - 1)]
    }
}

/// Pull the (single, richest) CSceneVehicleVis entity out of a parsed record.
pub fn vehicle_ent(rd: &RecordData) -> Option<&Ent> {
    rd.ents
        .iter()
        .filter(|e| {
            e.sample_size >= 100
                && !e.times.is_empty()
                && rd
                    .descs
                    .get(e.type_.max(0) as usize)
                    .map(|d| d.class_id)
                    == Some(CLASS_CSCENEVEHICLEVIS)
        })
        .max_by_key(|e| e.times.len())
}

/// Index of the vehicle entity inside `rd.ents`.
pub fn vehicle_ent_idx(rd: &RecordData) -> Option<usize> {
    let want = vehicle_ent(rd)? as *const Ent;
    rd.ents.iter().position(|e| std::ptr::eq(e, want))
}

pub fn scan_file(path: &str) -> Res<Scan> {
    let body = load_body(path)?;
    let (ver, blob) = find_entrecord_blob(&body)?;
    let rd = parse_record_data(&blob, ver)?;
    let race_ms = read_ghost_result(&body).0;
    let e = vehicle_ent(&rd).ok_or("no CSceneVehicleVis entity")?;
    let ss = e.sample_size;
    let n = e.times.len();
    let mut pos: Vec<[f64; 3]> = Vec::with_capacity(n);
    let mut spd: Vec<f64> = Vec::with_capacity(n);
    let mut vel: Vec<[f64; 3]> = Vec::with_capacity(n);
    for i in 0..n {
        let s = decode_vehicle_sample(&e.raw[i * ss..(i + 1) * ss]);
        pos.push([s.x, s.y, s.z]);
        spd.push(s.speed_ms);
        vel.push([s.vx, s.vy, s.vz]);
    }
    let dist3 = |a: [f64; 3], b: [f64; 3]| {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    };
    let mut steps = Vec::with_capacity(n.saturating_sub(1));
    for i in 0..n.saturating_sub(1) {
        let dt = (e.times[i + 1] - e.times[i]) as f64 / 1000.0;
        let d = dist3(pos[i + 1], pos[i]);
        let allowed = spd[i].max(spd[i + 1]) * dt;
        let fwd = [
            pos[i][0] + vel[i][0] * dt,
            pos[i][1] + vel[i][1] * dt,
            pos[i][2] + vel[i][2] * dt,
        ];
        let bwd = [
            pos[i + 1][0] - vel[i + 1][0] * dt,
            pos[i + 1][1] - vel[i + 1][1] * dt,
            pos[i + 1][2] - vel[i + 1][2] * dt,
        ];
        let gap_fwd = dist3(pos[i + 1], fwd);
        let gap_bwd = dist3(pos[i], bwd);
        let trapp = [
            pos[i][0] + 0.5 * (vel[i][0] + vel[i + 1][0]) * dt,
            pos[i][1] + 0.5 * (vel[i][1] + vel[i + 1][1]) * dt,
            pos[i][2] + 0.5 * (vel[i][2] + vel[i + 1][2]) * dt,
        ];
        steps.push(Step {
            i,
            t0: e.times[i],
            t1: e.times[i + 1],
            dist: d,
            allowed,
            excess: d - allowed,
            gap_fwd,
            gap_bwd,
            gap: gap_fwd.min(gap_bwd),
            trap: dist3(pos[i + 1], trapp),
        });
    }
    let period = if n > 2 {
        let mut d: Vec<i32> = e.times.windows(2).map(|w| w[1] - w[0]).collect();
        d.sort_unstable();
        d[d.len() / 2]
    } else {
        0
    };
    Ok(Scan {
        path: path.to_string(),
        race_ms,
        n,
        t_first: *e.times.first().unwrap(),
        t_last: *e.times.last().unwrap(),
        period,
        start_ms: rd.start_ms,
        end_ms: rd.end_ms,
        n_before_zero: e.times.iter().filter(|t| **t < 0).count(),
        n_after_finish: match race_ms {
            Some(r) => e.times.iter().filter(|t| **t > r).count(),
            None => 0,
        },
        n_nan: (0..n).filter(|i| !pos[*i].iter().all(|v| v.is_finite()) || !spd[*i].is_finite()).count(),
        first_nan: (0..n)
            .find(|i| !pos[*i].iter().all(|v| v.is_finite()) || !spd[*i].is_finite())
            .map(|i| e.times[i]),
        sample_times: e.times.clone(),
        steps,
    })
}

fn secs(ms: i32) -> String {
    format!("{}.{:03}", ms / 1000, (ms % 1000).abs())
}

// ---------------------------------------------------------------------------
// tail scan
// ---------------------------------------------------------------------------

pub fn cmd_scan(paths: &[String], tsv: Option<&str>, thr: f64, verbose: bool, around: Option<usize>) -> i32 {
    let mut rows = Vec::new();
    let mut bad = 0usize;
    for p in paths {
        match scan_file(p) {
            Err(e) => {
                eprintln!("{}\tERROR\t{}", p, e);
                rows.push(format!("{}\tERROR\t{}", p, e));
            }
            Ok(sc) => {
                let w = sc.worst();
                let (wt, wj, we, wd) = match w {
                    Some(s) => (s.t1, s.trap, s.excess, s.dist),
                    None => (0, 0.0, 0.0, 0.0),
                };
                if wj > thr {
                    bad += 1;
                }
                let over = sc.over(thr);
                rows.push(format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\t{}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{}\t{}\t{}\t{}\t{}",
                    p,
                    sc.race_ms.map(secs).unwrap_or_else(|| "-".into()),
                    sc.n,
                    secs(sc.t_first),
                    secs(sc.t_last),
                    sc.period,
                    sc.n_before_zero,
                    sc.n_after_finish,
                    wj,
                    secs(wt),
                    we,
                    wd,
                    sc.median_trap(),
                    sc.p99_trap(),
                    over.len(),
                    over.first().map(|s| secs(s.t1)).unwrap_or_else(|| "-".into()),
                    sc.n_nan,
                    format!("{:.4}", sc.post_finish_max()),
                    sc.first_nan.map(secs).unwrap_or_else(|| "-".into()),
                ));
                if verbose {
                    println!(
                        "{}\n  race {}  samples {}  record {}..{}  step {} ms",
                        p,
                        sc.race_ms.map(secs).unwrap_or_else(|| "-".into()),
                        sc.n,
                        secs(sc.t_first),
                        secs(sc.t_last),
                        sc.period
                    );
                    println!(
                        "  samples before 0.000: {}   after the finish: {}   \
                         median residual {:.4} m   p99 {:.4} m",
                        sc.n_before_zero,
                        sc.n_after_finish,
                        sc.median_trap(),
                        sc.p99_trap()
                    );
                    let (lo, hi) = match around {
                        Some(a) => (a.saturating_sub(4), (a + 4).min(sc.steps.len())),
                        None => (sc.steps.len().saturating_sub(12), sc.steps.len()),
                    };
                    println!("     t0      t1     moved   allowed    excess   JUMP(m)");
                    for s in &sc.steps[lo..hi] {
                        println!(
                            "  {:>7} {:>7} {:9.4} {:9.4} {:9.4} {:9.4}{}",
                            secs(s.t0),
                            secs(s.t1),
                            s.dist,
                            s.allowed,
                            s.excess,
                            s.trap,
                            if s.trap > thr { "   <== JUMP" } else { "" }
                        );
                    }
                }
            }
        }
    }
    let hdr = "path\trace_s\tn\tt_first\tt_last\tperiod_ms\tn_before_0\tn_after_finish\t\
               jump_m\tjump_at_s\texcess_m\tmoved_m\tmedian_resid_m\tp99_resid_m\t\
               n_steps_over\tfirst_over_s\tn_nan\tpost_finish_max_m\tfirst_nan_s";
    if let Some(f) = tsv {
        let mut out = String::from(hdr);
        out.push('\n');
        for r in &rows {
            out.push_str(r);
            out.push('\n');
        }
        if let Err(e) = std::fs::write(f, out) {
            eprintln!("{}: {}", f, e);
            return 3;
        }
        eprintln!("wrote {} ({} files, {} with a jump over {} m)", f, rows.len(), bad, thr);
    } else if !verbose {
        println!("{}", hdr);
        for r in &rows {
            println!("{}", r);
        }
    }
    0
}

// ---------------------------------------------------------------------------
// tail fix -- truncate the vehicle entity at the last genuine sample
// ---------------------------------------------------------------------------

/// Truncate `rd`'s vehicle entity to its first `keep` samples.
pub fn truncate_vehicle(rd: &mut RecordData, keep: usize) -> Res<(usize, usize)> {
    let idx = vehicle_ent_idx(rd).ok_or("no CSceneVehicleVis entity")?;
    let e = &mut rd.ents[idx];
    let was = e.times.len();
    if keep >= was {
        return Ok((was, was));
    }
    if keep == 0 {
        return Err("refusing to truncate the vehicle entity to zero samples".into());
    }
    let ss = e.sample_size;
    e.times.truncate(keep);
    e.raw.truncate(keep * ss);
    // u03 is this entity's own end-of-recording stamp; keep it honest.
    let t_last = *e.times.last().unwrap();
    if e.u03 > t_last {
        e.u03 = t_last;
    }
    Ok((was, keep))
}

/// How many samples to keep: every sample at or before `cut_ms`.
pub fn keep_count(times: &[i32], cut_ms: i32) -> usize {
    times.iter().filter(|t| **t <= cut_ms).count()
}

// ---------------------------------------------------------------------------
// tail plan -- cross the physics against the regeneration bookkeeping
// ---------------------------------------------------------------------------

/// One line of `tg_coverage_v3.tsv`: how many of a file's samples the
/// regeneration pass actually wrote.
pub struct Cov {
    pub regen: usize,
    pub total: usize,
}

pub fn load_coverage(path: &str) -> Res<std::collections::HashMap<String, Cov>> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
    let mut m = std::collections::HashMap::new();
    for line in txt.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 3 {
            continue;
        }
        // field 2 is "477 of 557"
        let w: Vec<&str> = f[2].split_whitespace().collect();
        if w.len() != 3 {
            continue;
        }
        let (Ok(regen), Ok(total)) = (w[0].parse::<usize>(), w[2].parse::<usize>()) else {
            continue;
        };
        m.insert(f[0].to_string(), Cov { regen, total });
    }
    Ok(m)
}

/// Key a full path by the last three components (`<map>/replays/<file>`),
/// which is how the coverage table names a file.
pub fn cov_key(path: &str) -> String {
    let parts: Vec<&str> = path.rsplit('/').take(3).collect();
    parts.into_iter().rev().collect::<Vec<_>>().join("/")
}

pub fn cmd_plan(paths: &[String], cov_path: &str, thr: f64, tsv: Option<&str>) -> i32 {
    let cov = match load_coverage(cov_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return 3;
        }
    };
    let mut rows = Vec::new();
    let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
    for p in paths {
        let sc = match scan_file(p) {
            Ok(s) => s,
            Err(e) => {
                rows.push(format!("{}\tERROR\t\t\t\t\t\t{}", cov_key(p), e));
                *counts.entry("ERROR").or_default() += 1;
                continue;
            }
        };
        let c = cov.get(&cov_key(p));
        // physics AT THE BOOKKEEPING BOUNDARY: the step from the last
        // regenerated sample to the first one the pass could not write.
        let boundary = c.and_then(|c| {
            if c.regen >= 1 && c.regen <= sc.steps.len() {
                Some(sc.steps[c.regen - 1])
            } else {
                None
            }
        });
        // the largest residual anywhere BEFORE the boundary -- the file's own
        // worst legitimate discontinuity (a respawn, a spliced cut), which is
        // what the boundary has to stand out from
        let pre_max = c
            .map(|c| {
                sc.steps[..(c.regen.saturating_sub(1)).min(sc.steps.len())]
                    .iter()
                    .map(|s| s.trap)
                    .fold(0.0f64, f64::max)
            })
            .unwrap_or(0.0);
        let (verdict, keep, note) = match c {
            None => ("NO_COVERAGE", None, "not in the regeneration table".to_string()),
            Some(c) if c.total != sc.n => (
                "COUNT_MISMATCH",
                None,
                format!("table says {} samples, file has {}", c.total, sc.n),
            ),
            Some(c) if c.regen >= sc.n => ("CLEAN", None, String::new()),
            Some(_) => match boundary {
                Some(b) if b.trap > thr => (
                    "TAIL_CONFIRMED",
                    Some(c.unwrap().regen),
                    format!("jump {:.3} m at {}", b.trap, secs(b.t1)),
                ),
                Some(b) => (
                    "TAIL_NO_JUMP",
                    Some(c.unwrap().regen),
                    format!("boundary residual only {:.4} m at {}", b.trap, secs(b.t1)),
                ),
                None => ("NO_BOUNDARY", None, "coverage index out of range".to_string()),
            },
        };
        *counts.entry(verdict).or_default() += 1;
        rows.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{}\t{}",
            cov_key(p),
            verdict,
            sc.n,
            c.map(|c| c.regen.to_string()).unwrap_or_else(|| "-".into()),
            keep.map(|k| k.to_string()).unwrap_or_else(|| "-".into()),
            boundary.map(|b| b.trap).unwrap_or(f64::NAN),
            pre_max,
            sc.median_trap(),
            sc.race_ms.map(secs).unwrap_or_else(|| "-".into()),
            note
        ));
    }
    let hdr = "file\tverdict\tn_samples\tcov_regen\tkeep\tboundary_jump_m\tpre_boundary_max_m\tmedian_resid_m\trace_s\tnote";
    if let Some(f) = tsv {
        let mut out = String::from(hdr);
        out.push('\n');
        for r in &rows {
            out.push_str(r);
            out.push('\n');
        }
        if let Err(e) = std::fs::write(f, out) {
            eprintln!("{}: {}", f, e);
            return 3;
        }
        eprintln!("wrote {}", f);
    } else {
        println!("{}", hdr);
        for r in &rows {
            println!("{}", r);
        }
    }
    eprintln!("--- verdicts ---");
    for (k, v) in &counts {
        eprintln!("{:>20}  {}", k, v);
    }
    0
}

// ---------------------------------------------------------------------------
// tail apply -- decide, cut, and re-read every file
// ---------------------------------------------------------------------------

/// The decision for one file, made from the two independent routes.
pub struct Decision {
    pub verdict: &'static str,
    pub keep: Option<usize>,
    pub n: usize,
    pub boundary_jump: f64,
    pub median: f64,
    pub note: String,
}

/// THE RULE, in one place.
///
/// > **A ghost's record ends at the finish line. Keep every sample at or
/// > before the finish and drop the rest.**
///
/// This is not a threshold, a heuristic or a tuned parameter -- it is the
/// measured native shape of a Trackmania ghost. Six untouched recordings
/// downloaded from the Nadeo leaderboards, 178 129 samples between them
/// (including one 8790.769 s session of 175 598 samples), carry **zero**
/// samples after their own race time. Not "few": zero, on all six, with the
/// last sample always on the 50 ms grid immediately before the finish.
///
/// Every post-finish sample in our published set is therefore an artefact of
/// the transplant, and there are two kinds, both removed by this one rule:
///
/// * **the carrier's tail** -- the carrier was a LONGER recording than our run,
///   so its record keeps going after our finish with a stranger's car in it.
///   That is the teleport: up to 1760.925 m in one 50 ms frame.
/// * **the engine's terminated-race state** -- on some files the last few
///   samples the regeneration pass could take, after the line, are themselves
///   discontinuous (worst measured: 2.155 m).
///
/// The finish is `max(the ghost header's race time, the ORACLE's validated
/// time)`. They disagree on 16 files because the header is still the carrier's,
/// and on four of those the header is EARLIER than our own finish -- taking it
/// alone would cut into the race. Taking the later of the two cannot.
///
/// The coverage table and the discontinuity measurement are NOT inputs to this
/// rule. They are the independent evidence that the tail was wrong and the
/// proof that it is gone, and they are reported per file rather than trusted.
pub fn decide(
    sc: &Scan,
    cov: Option<&Cov>,
    oracle_ms: Option<Option<i32>>,
    abs_thr: f64,
    rel_thr: f64,
) -> Decision {
    let median = sc.median_trap();
    let d = |verdict, keep, bj: f64, note: String| Decision {
        verdict,
        keep,
        n: sc.n,
        boundary_jump: bj,
        median,
        note,
    };
    if sc.n_nan > 0 {
        return d(
            "REFUSED_NONFINITE",
            None,
            f64::NAN,
            format!(
                "{} of {} samples carry a non-finite position; this file needs \
                 re-regeneration, not a cut",
                sc.n_nan, sc.n
            ),
        );
    }
    let finish = match (sc.race_ms, oracle_ms) {
        (Some(h), Some(Some(o))) => h.max(o),
        (Some(h), _) => h,
        (None, Some(Some(o))) => o,
        (None, _) => {
            return d(
                "SKIP_NO_FINISH",
                None,
                f64::NAN,
                "no race time in the ghost and the oracle gives none".into(),
            )
        }
    };
    let keep = sc.times_at_or_before(finish);
    let post = sc.n - keep;
    // the evidence, measured but not used to decide
    let need = abs_thr.max(rel_thr * median);
    let worst_post = sc
        .steps
        .iter()
        .filter(|s| s.t1 > finish)
        .map(|s| s.trap)
        .fold(0.0f64, f64::max);
    let covnote = match cov {
        Some(c) if c.total == sc.n && c.regen < sc.n => {
            format!("; carrier telemetry from sample {} of {}", c.regen, c.total)
        }
        Some(c) if c.total == sc.n => "; regenerated end to end".into(),
        Some(c) => format!("; TABLE DISAGREES on the sample count ({})", c.total),
        None => "; never regenerated -- telemetry is the carrier's throughout".into(),
    };
    if keep == 0 {
        return d(
            "REFUSED_EMPTY",
            None,
            worst_post,
            "no sample at or before the finish".into(),
        );
    }
    if post == 0 {
        return d(
            "CLEAN",
            None,
            0.0,
            format!("already ends at the finish{}", covnote),
        );
    }
    d(
        "CUT",
        Some(keep),
        worst_post,
        format!(
            "finish {} -> keep {} of {}, drop {} post-finish sample(s); worst post-finish \
             discontinuity removed: {:.4} m (threshold for calling it one: {:.4} m){}",
            secs(finish),
            keep,
            sc.n,
            post,
            worst_post,
            need,
            covnote
        ),
    )
}

/// Load `key<TAB>ms|DNF` -- the oracle's validated time per file.
pub fn load_times(path: &str) -> Res<std::collections::HashMap<String, Option<i32>>> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
    let mut m = std::collections::HashMap::new();
    for l in txt.lines() {
        let Some((k, v)) = l.split_once('\t') else { continue };
        m.insert(cov_key(k), v.trim().parse::<i32>().ok());
    }
    Ok(m)
}

/// Everything `tail apply` measures about one file, before and after.
pub struct Applied {
    pub key: String,
    pub verdict: &'static str,
    pub n_before: usize,
    pub n_after: usize,
    pub jump_before: f64,
    pub jump_after: f64,
    pub median: f64,
    pub prefix_identical: bool,
    pub note: String,
}

/// Copy `src` to `dst`, creating the parent directory.
fn copy_through(src: &str, dst: &str) -> Res<()> {
    if let Some(p) = std::path::Path::new(dst).parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {}", p.display(), e))?;
    }
    std::fs::copy(src, dst).map_err(|e| format!("{} -> {}: {}", src, dst, e))?;
    Ok(())
}

/// The decompressed body with the record node's enclosing skippable chunk
/// removed. Everything a viewer sees that is NOT the trajectory -- the skin
/// FileRef, the trigram, the club tag, the zone, the login, the race time and
/// splits, the INPUT TAPE -- lives in this remainder, so comparing it
/// byte-for-byte before and after is the airtight form of "only the record
/// changed".
pub fn body_without_record(path: &str) -> Res<Vec<u8>> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {}", path, e))?;
    let g = crate::gbx::Gbx::parse(&data);
    let site = crate::recwrite::find_rec_site(&g.body)?;
    let (lo, hi) = match site.skip_chunk {
        // drop the whole framing chunk: its own size field legitimately changes
        Some((_, coff, _, sz)) => (coff, coff + 12 + sz),
        None => (site.hdr, site.hdr + 12 + site.csize),
    };
    let mut out = g.body[..lo].to_vec();
    out.extend_from_slice(&g.body[hi.min(g.body.len())..]);
    Ok(out)
}

/// The vehicle entity's raw sample bytes, for the prefix-identity check.
fn vehicle_raw(path: &str) -> Res<(Vec<i32>, Vec<u8>, usize)> {
    let body = load_body(path)?;
    let (v, blob) = find_entrecord_blob(&body)?;
    let rd = parse_record_data(&blob, v)?;
    let e = vehicle_ent(&rd).ok_or("no CSceneVehicleVis entity")?;
    Ok((e.times.clone(), e.raw.clone(), e.sample_size))
}

#[allow(clippy::too_many_arguments)]
pub fn cmd_apply(
    paths: &[String],
    in_root: &str,
    out_root: &str,
    cov_path: &str,
    ours_path: Option<&str>,
    times_path: Option<&str>,
    abs_thr: f64,
    rel_thr: f64,
    tsv: Option<&str>,
) -> i32 {
    let cov = match load_coverage(cov_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return 3;
        }
    };
    // the files we are allowed to touch. Anything not on this list is copied
    // byte-for-byte -- that is how the 14 human recordings stay untouched.
    let ours: Option<std::collections::HashSet<String>> = match ours_path {
        None => None,
        Some(p) => match std::fs::read_to_string(p) {
            Ok(t) => Some(t.lines().map(cov_key).collect()),
            Err(e) => {
                eprintln!("{}: {}", p, e);
                return 3;
            }
        },
    };
    let times = match times_path {
        None => Default::default(),
        Some(p) => match load_times(p) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{}", e);
                return 3;
            }
        },
    };
    let mut rows: Vec<Applied> = Vec::new();
    let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
    for p in paths {
        let key = cov_key(p);
        let rel = p.strip_prefix(in_root).unwrap_or(p).trim_start_matches('/');
        let dst = format!("{}/{}", out_root.trim_end_matches('/'), rel);
        let mine = ours.as_ref().map_or(true, |s| s.contains(&key));
        let push = |rows: &mut Vec<Applied>,
                    counts: &mut std::collections::BTreeMap<&str, usize>,
                    a: Applied| {
            *counts.entry(a.verdict).or_default() += 1;
            rows.push(a);
        };
        if !mine {
            if let Err(e) = copy_through(p, &dst) {
                eprintln!("{}", e);
                return 3;
            }
            push(
                &mut rows,
                &mut counts,
                Applied {
                    key,
                    verdict: "UNTOUCHED_NOT_OURS",
                    n_before: 0,
                    n_after: 0,
                    jump_before: f64::NAN,
                    jump_after: f64::NAN,
                    median: f64::NAN,
                    prefix_identical: true,
                    note: "human / author recording".into(),
                },
            );
            continue;
        }
        let sc = match scan_file(p) {
            Ok(s) => s,
            Err(e) => {
                if let Err(e) = copy_through(p, &dst) {
                    eprintln!("{}", e);
                    return 3;
                }
                push(
                    &mut rows,
                    &mut counts,
                    Applied {
                        key,
                        verdict: "UNDECODABLE",
                        n_before: 0,
                        n_after: 0,
                        jump_before: f64::NAN,
                        jump_after: f64::NAN,
                        median: f64::NAN,
                        prefix_identical: true,
                        note: e,
                    },
                );
                continue;
            }
        };
        let dec = decide(&sc, cov.get(&key), times.get(&key).copied(), abs_thr, rel_thr);
        let Some(keep) = dec.keep else {
            if let Err(e) = copy_through(p, &dst) {
                eprintln!("{}", e);
                return 3;
            }
            push(
                &mut rows,
                &mut counts,
                Applied {
                    key,
                    verdict: dec.verdict,
                    n_before: sc.n,
                    n_after: sc.n,
                    jump_before: dec.boundary_jump,
                    jump_after: dec.boundary_jump,
                    median: dec.median,
                    prefix_identical: true,
                    note: dec.note,
                },
            );
            continue;
        };
        if let Some(par) = std::path::Path::new(&dst).parent() {
            let _ = std::fs::create_dir_all(par);
        }
        if let Err(e) = crate::recwrite::rewrite_ghost(p, &dst, |rd| {
            truncate_vehicle(rd, keep).map(|_| ())
        }) {
            eprintln!("{}: {}", p, e);
            return 3;
        }
        // --- READ THE FILE WE JUST WROTE, from disk ---
        let after = match scan_file(&dst) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: rewritten file will not decode: {}", dst, e);
                return 3;
            }
        };
        // the kept samples must be BYTE-IDENTICAL to the original's first
        // `keep`: this is what proves the rewrite truncated and did nothing else
        let prefix_identical = match (vehicle_raw(p), vehicle_raw(&dst)) {
            (Ok((t0, r0, s0)), Ok((t1, r1, s1))) => {
                s0 == s1 && t1[..] == t0[..keep] && r1[..] == r0[..keep * s0]
            }
            _ => false,
        };
        let jump_after = after
            .steps
            .last()
            .map(|s| s.trap)
            .unwrap_or(0.0)
            .max(after.worst().map(|s| s.trap).unwrap_or(0.0) * 0.0)
            .max(0.0);
        // report the residual of the file's NEW last step, and the worst
        // residual anywhere at or after the old boundary (there is none left)
        let last_step_resid = after.steps.last().map(|s| s.trap).unwrap_or(0.0);
        let _ = jump_after;
        push(
            &mut rows,
            &mut counts,
            Applied {
                key,
                verdict: dec.verdict,
                n_before: sc.n,
                n_after: after.n,
                jump_before: dec.boundary_jump,
                jump_after: last_step_resid,
                median: dec.median,
                prefix_identical,
                note: dec.note,
            },
        );
    }
    let hdr = "file\tverdict\tn_before\tn_after\tjump_before_m\tlast_step_resid_after_m\t\
               median_resid_m\tkept_prefix_byte_identical\tnote";
    let mut out = String::from(hdr);
    out.push('\n');
    for a in &rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{}\t{}\n",
            a.key,
            a.verdict,
            a.n_before,
            a.n_after,
            a.jump_before,
            a.jump_after,
            a.median,
            a.prefix_identical,
            a.note
        ));
    }
    match tsv {
        Some(f) => {
            if let Err(e) = std::fs::write(f, &out) {
                eprintln!("{}: {}", f, e);
                return 3;
            }
            eprintln!("wrote {}", f);
        }
        None => print!("{}", out),
    }
    let bad = rows.iter().filter(|a| !a.prefix_identical).count();
    eprintln!("--- verdicts ---");
    for (k, v) in &counts {
        eprintln!("{:>24}  {}", k, v);
    }
    eprintln!("kept prefix NOT byte-identical: {}", bad);
    i32::from(bad != 0)
}

// ---------------------------------------------------------------------------
// tail finishcheck -- are the REGENERATED post-finish samples any good?
// ---------------------------------------------------------------------------
//
// The cut point is a choice between "the last sample the engine gave us" and
// "the finish line". That choice should be made on evidence, so: split each
// file's regenerated span at its own race time and compare the two halves'
// worst continuity residual. If the post-finish half is as clean as the racing
// half, keep it; if the engine's terminated-race state produces junk, cut at
// the finish.

pub fn cmd_finishcheck(paths: &[String], cov_path: &str, tsv: Option<&str>) -> i32 {
    let cov = match load_coverage(cov_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return 3;
        }
    };
    let mut out = String::from(
        "file\trace_s\tn\tregen\tn_post_finish_kept\tpre_finish_max_m\tpre_finish_p99_m\t\
         post_finish_max_m\tworst_post_at_s\n",
    );
    let (mut nfiles, mut nbad) = (0usize, 0usize);
    for p in paths {
        let Ok(sc) = scan_file(p) else { continue };
        if sc.n_nan > 0 {
            continue;
        }
        let Some(c) = cov.get(&cov_key(p)) else { continue };
        if c.total != sc.n {
            continue;
        }
        let Some(race) = sc.race_ms else { continue };
        // steps ENTIRELY inside the regenerated span: step k joins samples k and
        // k+1, so the last one wholly regenerated is k = regen-2.
        let end = c.regen.saturating_sub(1).min(sc.steps.len());
        let kept = &sc.steps[..end];
        let pre: Vec<f64> = kept.iter().filter(|s| s.t1 <= race).map(|s| s.trap).collect();
        let post: Vec<&Step> = kept.iter().filter(|s| s.t1 > race).collect();
        if pre.is_empty() {
            continue;
        }
        let mut v = pre.clone();
        v.sort_by(|a, b| a.total_cmp(b));
        let pre_max = *v.last().unwrap();
        let pre_p99 = v[((v.len() as f64 * 0.99) as usize).min(v.len() - 1)];
        let worst_post = post.iter().copied().max_by(|a, b| a.trap.total_cmp(&b.trap));
        let post_max = worst_post.map(|s| s.trap).unwrap_or(0.0);
        nfiles += 1;
        if post_max > pre_max.max(0.30) {
            nbad += 1;
        }
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{}\n",
            cov_key(p),
            secs(race),
            sc.n,
            c.regen,
            post.len(),
            pre_max,
            pre_p99,
            post_max,
            worst_post.map(|s| secs(s.t1)).unwrap_or_else(|| "-".into())
        ));
    }
    match tsv {
        Some(f) => {
            if let Err(e) = std::fs::write(f, &out) {
                eprintln!("{}: {}", f, e);
                return 3;
            }
            eprintln!("wrote {}", f);
        }
        None => print!("{}", out),
    }
    eprintln!(
        "{} files; {} whose regenerated POST-FINISH span is worse than their whole race",
        nfiles, nbad
    );
    0
}

// ---------------------------------------------------------------------------
// tail verify -- re-read both trees from disk and check the invariants
// ---------------------------------------------------------------------------
//
// This does not trust `tail apply`'s own bookkeeping: it decodes the shipped
// file and the original independently and asserts four things per file.
//
//   V1  every sample at or before the file's finish survives -- the cut can
//       never touch the race
//   V2  the shipped record has NO post-finish discontinuity left above the
//       same threshold the cut used
//   V3  no sample in the shipped file has a non-finite position
//   V4  the shipped samples are BYTE-IDENTICAL to the original's first N --
//       the rewrite truncated and did nothing else
//
// V1/V4 are the ones that can fail for the right reason: they compare against
// the original bytes, which this pipeline did not produce.

pub fn cmd_verify(
    paths: &[String],
    before_root: &str,
    after_root: &str,
    times_path: Option<&str>,
    abs_thr: f64,
    rel_thr: f64,
    tsv: Option<&str>,
) -> i32 {
    let times = match times_path {
        None => Default::default(),
        Some(p) => match load_times(p) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{}", e);
                return 3;
            }
        },
    };
    let mut out = String::from(
        "file\tn_before\tn_after\tfinish_s\tsamples_at_or_before_finish\t\
         V1_race_intact\tV2_no_post_finish_jump\tV3_all_finite\tV4_prefix_identical\tV5_rest_of_body_identical\t\
         worst_post_finish_after_m\tworst_post_finish_before_m\n",
    );
    let (mut ok, mut bad) = (0usize, 0usize);
    for p in paths {
        let key = cov_key(p);
        let a_path = format!("{}/{}", after_root.trim_end_matches('/'), key);
        let (Ok(b), Ok(a)) = (scan_file(p), scan_file(&a_path)) else {
            out.push_str(&format!("{}\t-\t-\t-\t-\tSKIP\tSKIP\tSKIP\tSKIP\t-\t-\n", key));
            continue;
        };
        let finish = match (b.race_ms, times.get(&key).copied()) {
            (_, Some(None)) => b.race_ms.unwrap_or(i32::MAX),
            (Some(h), Some(Some(o))) => h.max(o),
            (Some(h), None) => h,
            (None, Some(Some(o))) => o,
            (None, None) => i32::MAX,
        };
        let need = abs_thr.max(rel_thr * b.median_trap());
        // V1
        let n_race = {
            let (Ok((tb, _, _)), Ok((ta, _, _))) = (vehicle_raw(p), vehicle_raw(&a_path)) else {
                out.push_str(&format!("{}\t-\t-\t-\t-\tSKIP\tSKIP\tSKIP\tSKIP\t-\t-\n", key));
                continue;
            };
            let n = tb.iter().filter(|t| **t <= finish).count();
            let _ = ta;
            n
        };
        let v1 = a.n >= n_race;
        // V2
        let worst_after = a
            .steps
            .iter()
            .filter(|s| s.t1 > finish)
            .map(|s| s.trap)
            .fold(0.0f64, f64::max);
        let worst_before = b
            .steps
            .iter()
            .filter(|s| s.t1 > finish)
            .map(|s| s.trap)
            .fold(0.0f64, f64::max);
        let v2 = worst_after <= need;
        // V3
        let v3 = a.n_nan == 0;
        // V4
        let v4 = match (vehicle_raw(p), vehicle_raw(&a_path)) {
            (Ok((tb, rb, sb)), Ok((ta, ra, sa))) => {
                sa == sb && ta.len() <= tb.len() && ta[..] == tb[..ta.len()] && ra[..] == rb[..ta.len() * sb]
            }
            _ => false,
        };
        let v5 = match (body_without_record(p), body_without_record(&a_path)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        };
        if v1 && v2 && v3 && v4 && v5 {
            ok += 1;
        } else {
            bad += 1;
        }
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\t{:.4}\n",
            key,
            b.n,
            a.n,
            if finish == i32::MAX { "-".into() } else { secs(finish) },
            n_race,
            v1,
            v2,
            v3,
            v4,
            v5,
            worst_after,
            worst_before
        ));
    }
    match tsv {
        Some(f) => {
            if let Err(e) = std::fs::write(f, &out) {
                eprintln!("{}: {}", f, e);
                return 3;
            }
            eprintln!("wrote {}", f);
        }
        None => print!("{}", out),
    }
    let _ = before_root;
    eprintln!("VERIFY: {} files all-pass, {} with a failing invariant", ok, bad);
    i32::from(bad != 0)
}
