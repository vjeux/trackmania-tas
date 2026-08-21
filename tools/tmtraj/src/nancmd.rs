//! `tmtraj nan ...` -- the acceptance instrument for the NaN re-regeneration
//! arm (`nan`, 2026-08-20).
//!
//! WHY THIS EXISTS
//! ---------------
//! Four published ghosts carry a non-finite position and **every gate in the
//! pipeline passed them as "OK, 100 %"**. The reason is spelled out in
//! `tail_FINDING_v1_*`: every comparison involving NaN is false, so a check
//! written as `err > tol -> reject` ACCEPTS NaN. The two spellings of a
//! threshold test are identical on real data and opposite on broken data.
//!
//! And `is_finite()` is necessary but not sufficient. Two of eight
//! regenerations of 270051 produced a trajectory that was exactly (0,0,0) at
//! every instant: finite, internally self-consistent, unit quaternion, passing
//! every structural test, and 1082 m from where the map starts. Zeroed memory
//! has the right SHAPE too.
//!
//! So every gate here tests a RELATIONSHIP a decoy has no reason to satisfy:
//!
//!   G1 FINITE     every position / quaternion / speed component is finite.
//!                 Written as `!v.is_finite() -> reject`, positively, first.
//!   G2 MOVES      the path length over the race exceeds `--minpath` metres
//!                 and the samples are not all the same point. Zeroed memory
//!                 travels zero.
//!   G3 SPAWN      the first in-race sample is within `--spawntol` metres of
//!                 the same map's reference ghost's first sample. A decoy has
//!                 no reason to start where the map starts. (270051's zero
//!                 decoy was 1082 m out.)
//!   G4 AIM        the body points roughly where the car is going: the spread
//!                 of (body heading - velocity heading) over the moving part
//!                 of the run, allowing any constant drift angle, must be
//!                 under `--aimspread` degrees. A unit quaternion belonging to
//!                 something else is still a unit quaternion (fleet notice,
//!                 arm `tg`); the decoy there spread over 90+ degrees.
//!   G5 SMOOTH     the worst trapezoid residual |dp - (v0+v1)/2 dt| strictly
//!                 INSIDE the race is under `--resid` metres. Built from two
//!                 independent fields of the sample, so an internally
//!                 consistent file cannot pass it by construction.
//!
//! `nan cmp` is the discriminator: per-sample position agreement between two
//! ghosts matched on sample time. Used to ask the only question that settles a
//! disagreement between two candidate layouts -- which one reproduces a
//! DOWNLOADED HUMAN GHOST's own recorded bytes when the engine re-simulates
//! that human's own inputs.

use crate::entrec::{find_entrecord_blob, load_body, parse_record_data, read_transform_pub};

/// One decoded vehicle sample: what the gates need and nothing else.
pub struct S {
    pub ms: i64,
    pub pos: [f64; 3],
    pub quat: [f64; 4],
    pub speed: f64,
    pub vel: [f64; 3],
    pub finite: bool,
}

pub fn decode(path: &str) -> Result<Vec<S>, String> {
    let body = load_body(path)?;
    let (ver, blob) = find_entrecord_blob(&body)?;
    let rd = parse_record_data(&blob, ver)?;
    let ent = rd
        .ents
        .iter()
        .filter(|e| e.sample_size >= 100 && !e.times.is_empty())
        .max_by_key(|e| e.times.len())
        .ok_or("no CSceneVehicleVis entity")?;
    let ss = ent.sample_size;
    let mut out = Vec::with_capacity(ent.times.len());
    for (i, t) in ent.times.iter().enumerate() {
        let d = &ent.raw[i * ss..(i + 1) * ss];
        let (pos, quat, speed, vel) = read_transform_pub(d, 47);
        let finite = pos.iter().all(|v| v.is_finite())
            && quat.iter().all(|v| v.is_finite())
            && speed.is_finite()
            && vel.iter().all(|v| v.is_finite());
        out.push(S { ms: *t as i64, pos, quat, speed, vel, finite });
    }
    Ok(out)
}

fn quant(v: &mut Vec<f64>, f: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[(((v.len() - 1) as f64) * f).round() as usize]
}

/// Rotate the body forward axis (0,0,1) by the quaternion (x,y,z,w).
fn forward(q: &[f64; 4]) -> [f64; 3] {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    [
        2.0 * (x * z + w * y),
        2.0 * (y * z - w * x),
        1.0 - 2.0 * (x * x + y * y),
    ]
}

fn wrap180(mut d: f64) -> f64 {
    while d > 180.0 {
        d -= 360.0;
    }
    while d < -180.0 {
        d += 360.0;
    }
    d
}

/// Circular spread of (body heading - velocity heading), in degrees, over the
/// samples where the car is actually moving. Any CONSTANT drift angle is
/// allowed: what is measured is the spread about the circular mean, so a car
/// driven permanently sideways is not condemned.
pub fn aim_spread(s: &[S], minspeed: f64) -> (f64, usize) {
    let mut cs = 0.0f64;
    let mut sn = 0.0f64;
    let mut ds: Vec<f64> = Vec::new();
    for k in s {
        if !k.finite || k.speed < minspeed {
            continue;
        }
        let f = forward(&k.quat);
        let bh = f[0].atan2(f[2]).to_degrees();
        let vh = k.vel[0].atan2(k.vel[2]).to_degrees();
        let d = wrap180(bh - vh);
        ds.push(d);
        cs += d.to_radians().cos();
        sn += d.to_radians().sin();
    }
    if ds.len() < 4 {
        return (f64::NAN, ds.len());
    }
    let mean = sn.atan2(cs).to_degrees();
    let mut about: Vec<f64> = ds.iter().map(|d| wrap180(d - mean).abs()).collect();
    (quant(&mut about, 0.95), ds.len())
}

pub struct Gate {
    pub path: String,
    pub n: usize,
    pub n_nan: usize,
    pub n_zero: usize,
    pub n_distinct: usize,
    pub path_len: f64,
    pub first: [f64; 3],
    pub spawn_err: f64,
    pub aim: f64,
    pub aim_n: usize,
    pub resid: f64,
    pub resid_at: f64,
    pub verdict: String,
}

#[allow(clippy::too_many_arguments)]
pub fn gate_one(
    path: &str,
    race_ms: Option<i64>,
    refspawn: Option<[f64; 3]>,
    minpath: f64,
    spawntol: f64,
    aimspread: f64,
    residtol: f64,
) -> Gate {
    let mut g = Gate {
        path: path.to_string(),
        n: 0,
        n_nan: 0,
        n_zero: 0,
        n_distinct: 0,
        path_len: 0.0,
        first: [f64::NAN; 3],
        spawn_err: f64::NAN,
        aim: f64::NAN,
        aim_n: 0,
        resid: f64::NAN,
        resid_at: f64::NAN,
        verdict: String::new(),
    };
    let s = match decode(path) {
        Ok(s) => s,
        Err(e) => {
            g.verdict = format!("ERROR {}", e);
            return g;
        }
    };
    g.n = s.len();
    // ---- G1 FINITE, first, and spelled positively --------------------------
    g.n_nan = s.iter().filter(|k| !k.finite).count();
    // Everything below is measured on the in-race part only: the post-finish
    // tail is a separate defect with its own arm, and it must not be able to
    // make a race look bad or good.
    let hi = race_ms.unwrap_or(i64::MAX);
    let inrace: Vec<&S> = s.iter().filter(|k| k.ms <= hi).collect();
    if inrace.is_empty() {
        g.verdict = "ERROR no in-race sample".into();
        return g;
    }
    if g.n_nan > 0 {
        g.verdict = format!("FAIL_NONFINITE {} of {}", g.n_nan, g.n);
        return g;
    }
    g.first = inrace[0].pos;
    // ---- G2 MOVES ----------------------------------------------------------
    let mut seen: Vec<[u64; 3]> = Vec::new();
    for k in &inrace {
        if k.pos.iter().all(|v| *v == 0.0) {
            g.n_zero += 1;
        }
        let key = [
            k.pos[0].to_bits(),
            k.pos[1].to_bits(),
            k.pos[2].to_bits(),
        ];
        if !seen.contains(&key) {
            seen.push(key);
        }
    }
    g.n_distinct = seen.len();
    for w in inrace.windows(2) {
        let d: f64 = (0..3)
            .map(|i| (w[1].pos[i] - w[0].pos[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        g.path_len += d;
    }
    // ---- G3 SPAWN ----------------------------------------------------------
    if let Some(r) = refspawn {
        g.spawn_err = (0..3)
            .map(|i| (g.first[i] - r[i]).powi(2))
            .sum::<f64>()
            .sqrt();
    }
    // ---- G4 AIM ------------------------------------------------------------
    let owned: Vec<S> = inrace
        .iter()
        .map(|k| S {
            ms: k.ms,
            pos: k.pos,
            quat: k.quat,
            speed: k.speed,
            vel: k.vel,
            finite: k.finite,
        })
        .collect();
    let (a, an) = aim_spread(&owned, 5.0);
    g.aim = a;
    g.aim_n = an;
    // ---- G5 SMOOTH ---------------------------------------------------------
    for w in owned.windows(2) {
        let dt = (w[1].ms - w[0].ms) as f64 / 1000.0;
        if dt <= 0.0 {
            continue;
        }
        let mut r = 0.0f64;
        for i in 0..3 {
            let dp = w[1].pos[i] - w[0].pos[i];
            let pred = 0.5 * (w[0].vel[i] + w[1].vel[i]) * dt;
            r += (dp - pred).powi(2);
        }
        let r = r.sqrt();
        if !(g.resid >= r) {
            g.resid = r;
            g.resid_at = w[1].ms as f64 / 1000.0;
        }
    }
    // ---- verdict -----------------------------------------------------------
    let mut fails: Vec<String> = Vec::new();
    if g.n_zero > 0 {
        fails.push(format!("DEGENERATE_ZERO {} samples at the origin", g.n_zero));
    }
    if g.n_distinct < 2 || g.path_len < minpath {
        fails.push(format!(
            "DEGENERATE_STILL path {:.3} m over {} distinct point(s)",
            g.path_len, g.n_distinct
        ));
    }
    if refspawn.is_some() && !(g.spawn_err <= spawntol) {
        fails.push(format!("SPAWN {:.3} m from the map's start", g.spawn_err));
    }
    if g.aim_n >= 4 && !(g.aim <= aimspread) {
        fails.push(format!("AIM body-vs-velocity spread {:.1} deg", g.aim));
    }
    if g.resid.is_finite() && !(g.resid <= residtol) {
        fails.push(format!(
            "DISCONTINUOUS {:.4} m at {:.3}",
            g.resid, g.resid_at
        ));
    }
    g.verdict = if fails.is_empty() { "PASS".into() } else { format!("FAIL {}", fails.join("; ")) };
    g
}

fn flag(args: &[String], n: &str) -> Option<String> {
    args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
}

fn numflag(args: &[String], n: &str, d: f64) -> f64 {
    flag(args, n).and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// Calibrate the two SHAPE-dependent gates from a real recording of the same
/// map: a fixed aim-spread or continuity threshold cannot work across a corpus
/// that contains both a 4-second sprint and a ballistic flight map where the
/// car tumbles (227969 real recordings spread 123 deg) or a Trial map whose
/// car legitimately teleports on a respawn. What a decoy cannot imitate is
/// being no worse than a real recording of the SAME map.
pub fn calib(path: &str, race: Option<i64>) -> (f64, f64) {
    let Ok(s) = decode(path) else { return (f64::NAN, f64::NAN) };
    let hi = race.unwrap_or(i64::MAX);
    let inr: Vec<S> = s
        .into_iter()
        .filter(|k| k.ms <= hi && k.finite)
        .collect();
    let (a, _) = aim_spread(&inr, 5.0);
    let mut worst = 0.0f64;
    for w in inr.windows(2) {
        let dt = (w[1].ms - w[0].ms) as f64 / 1000.0;
        if dt <= 0.0 {
            continue;
        }
        let r: f64 = (0..3)
            .map(|i| (w[1].pos[i] - w[0].pos[i] - 0.5 * (w[0].vel[i] + w[1].vel[i]) * dt).powi(2))
            .sum::<f64>()
            .sqrt();
        if r > worst {
            worst = r;
        }
    }
    (a, worst)
}

pub fn cmd(args: &[String]) {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    match sub {
        "gate" => cmd_gate(&rest),
        "cmp" => cmd_cmp(&rest),
        "pick" => cmd_pick(&rest),
        "spawn" => cmd_spawn(&rest),
        "vres" => cmd_vres(&rest),
        "csvcmp" => cmd_csvcmp(&rest),
        "lag" => cmd_lag(&rest),
        _ => {
            eprintln!(
                "tmtraj nan gate  GHOST... [--ref REF.Ghost.Gbx] [--race MS] [--tsv OUT]\n\
                 \t\t[--minpath M] [--spawntol M] [--aimspread DEG] [--resid M]\n\
                 \t  G1 finite, G2 moves, G3 starts at the map's start, G4 body aims\n\
                 \t  along the velocity, G5 continuous. Every gate tests a relationship\n\
                 \t  a decoy has no reason to satisfy.\n\
                 tmtraj nan cmp   A.Ghost.Gbx B.Ghost.Gbx [--race MS] [--csv OUT]\n\
                 \t  per-sample position agreement, matched on sample time.\n\
                 tmtraj nan spawn GHOST...\n\
                 \t  the first in-race sample position of each file."
            );
            std::process::exit(2);
        }
    }
}

fn cmd_spawn(args: &[String]) {    for p in args.iter().filter(|a| !a.starts_with("--")) {
        match decode(p) {
            Ok(s) if !s.is_empty() => println!(
                "{}\t{}\t{:.4}\t{:.4}\t{:.4}",
                p, s[0].ms, s[0].pos[0], s[0].pos[1], s[0].pos[2]
            ),
            Ok(_) => println!("{}\tEMPTY", p),
            Err(e) => println!("{}\tERROR\t{}", p, e),
        }
    }
}

fn cmd_gate(args: &[String]) {
    const VALUED: [&str; 9] =
        ["--ref", "--race", "--times", "--tsv", "--minpath", "--spawntol", "--aimspread", "--resid",
         "--calib"];
    let mut files: Vec<String> = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
            continue;
        }
        if a.starts_with("--") {
            skip = VALUED.contains(&a.as_str());
            continue;
        }
        files.push(a.clone());
    }
    let refspawn = flag(args, "--ref").and_then(|r| {
        decode(&r).ok().and_then(|s| s.first().map(|k| k.pos))
    });
    if flag(args, "--ref").is_some() && refspawn.is_none() {
        println!("ABORT: --ref given but it could not be decoded");
        std::process::exit(3);
    }
    let race = flag(args, "--race").and_then(|v| v.parse::<i64>().ok());
    // Per-file race times: `relative/path \t ms`, matched by path suffix. The
    // finish is what separates "our run" from "the carrier's tail", and it is
    // per file, so a corpus scan needs it per file rather than one --race.
    let times: Vec<(String, i64)> = match flag(args, "--times") {
        Some(p) => std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("{}: {}", p, e))
            .lines()
            .filter_map(|l| {
                let mut it = l.split('\t');
                let k = it.next()?.trim().to_string();
                let v: i64 = it.next()?.trim().parse().ok()?;
                Some((k, v))
            })
            .collect(),
        None => Vec::new(),
    };
    let minpath = numflag(args, "--minpath", 5.0);
    let spawntol = numflag(args, "--spawntol", 1.0);
    let mut aimspread = numflag(args, "--aimspread", 45.0);
    let mut resid = numflag(args, "--resid", 0.5);
    if let Some(c) = flag(args, "--calib") {
        let (a, r) = calib(&c, race);
        if a.is_finite() {
            aimspread = aimspread.max(1.5 * a + 10.0);
        }
        if r.is_finite() {
            resid = resid.max(3.0 * r);
        }
        println!(
            "calibrated on {}: aim <= {:.1} deg, residual <= {:.4} m",
            c.rsplit('/').next().unwrap_or(&c),
            aimspread,
            resid
        );
    }
    let mut tsv = String::from(
        "path\tn\tn_nan\tn_zero\tn_distinct\tpath_len_m\tfirst_x\tfirst_y\tfirst_z\tspawn_err_m\taim_p95_deg\taim_n\tworst_resid_m\tresid_at_s\tverdict\n",
    );
    let mut npass = 0;
    let mut nfail = 0;
    for f in &files {
        let r = times
            .iter()
            .find(|(k, _)| f.ends_with(k.as_str()))
            .map(|(_, v)| *v)
            .or(race);
        let g = gate_one(f, r, refspawn, minpath, spawntol, aimspread, resid);
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.2}\t{}\t{:.4}\t{:.3}\t{}\n",
            g.path, g.n, g.n_nan, g.n_zero, g.n_distinct, g.path_len, g.first[0], g.first[1],
            g.first[2], g.spawn_err, g.aim, g.aim_n, g.resid, g.resid_at, g.verdict
        ));
        println!(
            "{:<70} {:>6} samples  {}",
            g.path.rsplit('/').next().unwrap_or(&g.path),
            g.n,
            g.verdict
        );
        if g.verdict == "PASS" {
            npass += 1
        } else {
            nfail += 1
        }
    }
    if let Some(o) = flag(args, "--tsv") {
        std::fs::write(&o, tsv).unwrap();
        println!("wrote {}", o);
    }
    println!("--- {} PASS, {} FAIL", npass, nfail);
    if nfail > 0 {
        std::process::exit(1);
    }
}

fn cmd_cmp(args: &[String]) {
    let files: Vec<&String> = args.iter().filter(|a| a.ends_with(".Gbx")).collect();
    if files.len() != 2 {
        println!("ABORT: need exactly two ghosts");
        std::process::exit(2);
    }
    let a = decode(files[0]).unwrap_or_else(|e| panic!("{}: {}", files[0], e));
    let b = decode(files[1]).unwrap_or_else(|e| panic!("{}: {}", files[1], e));
    let race = flag(args, "--race").and_then(|v| v.parse::<i64>().ok()).unwrap_or(i64::MAX);
    let bm: std::collections::HashMap<i64, &S> = b.iter().map(|k| (k.ms, k)).collect();
    let mut d: Vec<f64> = Vec::new();
    let mut ang: Vec<f64> = Vec::new();
    let mut along: Vec<f64> = Vec::new();
    let mut exact = 0usize;
    let mut nan_a = 0usize;
    let mut nan_b = 0usize;
    let mut csv = String::from("ms,dist_m,angle_deg\n");
    for k in a.iter().filter(|k| k.ms <= race) {
        let Some(o) = bm.get(&k.ms) else { continue };
        if !k.finite {
            nan_a += 1;
            continue;
        }
        if !o.finite {
            nan_b += 1;
            continue;
        }
        let e: f64 = (0..3).map(|i| (k.pos[i] - o.pos[i]).powi(2)).sum::<f64>().sqrt();
        if k.pos == o.pos {
            exact += 1;
        }
        let dot: f64 = (0..4).map(|i| k.quat[i] * o.quat[i]).sum::<f64>().abs().clamp(0.0, 1.0);
        let ad = 2.0 * dot.acos().to_degrees();
        csv.push_str(&format!("{},{:.6},{:.4}\n", k.ms, e, ad));
        // Signed component of (A - B) along B's own velocity. A STALE copy of a
        // double-buffered car state is one tick BEHIND the live one, so it sits
        // at a negative offset along the velocity of a fixed size; a decoy
        // belonging to another object has no reason to be systematically
        // behind. This is the reference-free half of the discriminator.
        let sp = (o.vel[0].powi(2) + o.vel[1].powi(2) + o.vel[2].powi(2)).sqrt();
        if sp > 1.0 {
            along.push(
                (0..3).map(|i| (k.pos[i] - o.pos[i]) * o.vel[i]).sum::<f64>() / sp,
            );
        }
        d.push(e);
        ang.push(ad);
    }
    if d.is_empty() {
        println!("ABORT: no common sample instant (nan_a {} nan_b {})", nan_a, nan_b);
        std::process::exit(3);
    }
    println!(
        "matched {} samples ({} bit-identical position), non-finite: A {}, B {}",
        d.len(),
        exact,
        nan_a,
        nan_b
    );
    println!(
        "POSITION  median {:.6} m   p90 {:.6}   p99 {:.6}   max {:.6}",
        quant(&mut d.clone(), 0.5),
        quant(&mut d.clone(), 0.9),
        quant(&mut d.clone(), 0.99),
        quant(&mut d.clone(), 1.0)
    );
    println!(
        "ORIENT    median {:.4} deg  p99 {:.4}   max {:.4}",
        quant(&mut ang.clone(), 0.5),
        quant(&mut ang.clone(), 0.99),
        quant(&mut ang.clone(), 1.0)
    );
    if !along.is_empty() {
        println!(
            "ALONG-V   median signed offset of A relative to B  {:+.6} m   (p10 {:+.6}, p90 {:+.6})",
            quant(&mut along.clone(), 0.5),
            quant(&mut along.clone(), 0.1),
            quant(&mut along.clone(), 0.9)
        );
    }
    if let Some(o) = flag(args, "--csv") {        std::fs::write(&o, csv).unwrap();
        println!("wrote {}", o);
    }
}

/// Median position distance and median signed along-velocity offset of A
/// relative to B, over their common in-race sample instants.
fn pair(a: &[S], b: &[S], race: i64) -> (f64, f64, usize) {
    let bm: std::collections::HashMap<i64, &S> = b.iter().map(|k| (k.ms, k)).collect();
    let mut d = Vec::new();
    let mut al = Vec::new();
    for k in a.iter().filter(|k| k.ms <= race && k.finite) {
        let Some(o) = bm.get(&k.ms) else { continue };
        if !o.finite {
            continue;
        }
        d.push((0..3).map(|i| (k.pos[i] - o.pos[i]).powi(2)).sum::<f64>().sqrt());
        let sp = (o.vel[0].powi(2) + o.vel[1].powi(2) + o.vel[2].powi(2)).sqrt();
        if sp > 1.0 {
            al.push((0..3).map(|i| (k.pos[i] - o.pos[i]) * o.vel[i]).sum::<f64>() / sp);
        }
    }
    let n = d.len();
    (quant(&mut d, 0.5), quant(&mut al, 0.5), n)
}

/// `tmtraj nan pick` -- choose between the candidate regenerations of ONE file.
///
/// The problem this solves, in one sentence: several independent runs of the
/// locate produce SEVERAL different self-consistent answers, all of which pass
/// every structural test, and a majority among them is not evidence.
///
/// The rule, in order:
///   1. every candidate must pass the gates (`nan gate`);
///   2. a candidate is only usable if an INDEPENDENT run reproduced it -- the
///      passing candidates are clustered at `--agree` metres and a cluster
///      needs `--minruns` members;
///   3. between two reproduced clusters, the accepted one is the one no other
///      is AHEAD of. A stale copy of a double-buffered car state is one tick
///      BEHIND the live one along its own velocity, by a fixed distance; the
///      live copy cannot be behind itself. (Measured on 270051: the losing
///      family sits -0.3298 m along the velocity, every sample, and is 0.32 m
///      from a downloaded human ghost's own recorded bytes where the winning
///      family is 0.49 mm.)
///
/// If that does not leave exactly one cluster, this REFUSES. A named exception
/// beats an invented fix.
fn cmd_pick(args: &[String]) {
    const VALUED: [&str; 11] =
        ["--race", "--ref", "--agree", "--minruns", "--stale", "--tsv", "--out", "--label",
         "--calib", "--staleband", "--lagtol"];
    let mut files: Vec<String> = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
            continue;
        }
        if a.starts_with("--") {
            skip = VALUED.contains(&a.as_str());
            continue;
        }
        files.push(a.clone());
    }
    let race = flag(args, "--race").and_then(|v| v.parse::<i64>().ok()).unwrap_or(i64::MAX);
    let agree = numflag(args, "--agree", 0.001);
    let stale = numflag(args, "--stale", 0.02);
    let minruns = numflag(args, "--minruns", 2.0) as usize;
    let label = flag(args, "--label").unwrap_or_else(|| "-".into());
    let refspawn = flag(args, "--ref").and_then(|r| decode(&r).ok().and_then(|s| s.first().map(|k| k.pos)));
    let mut aimtol = 45.0f64;
    let mut residtol = 1.0f64;
    if let Some(c) = flag(args, "--calib") {
        let (a, r) = calib(&c, Some(race));
        if a.is_finite() {
            aimtol = aimtol.max(1.5 * a + 10.0);
        }
        if r.is_finite() {
            residtol = residtol.max(3.0 * r);
        }
        println!("calibrated on a real recording of this map: aim <= {:.1} deg, residual <= {:.4} m", aimtol, residtol);
    }

    // ---- 1. gate -----------------------------------------------------------
    let mut ok: Vec<(String, Vec<S>)> = Vec::new();
    for f in &files {
        let g = gate_one(f, Some(race), refspawn, 5.0, 1.0, aimtol, residtol);
        let short = f.rsplit('/').next().unwrap_or(f).to_string();
        if g.verdict == "PASS" {
            match decode(f) {
                Ok(s) => ok.push((f.clone(), s)),
                Err(e) => println!("  {:<44} decode failed: {}", short, e),
            }
        } else {
            println!("  {:<44} rejected: {}", short, g.verdict);
        }
    }
    println!("{} of {} candidates pass the gates", ok.len(), files.len());
    if ok.is_empty() {
        println!("REFUSE {} no candidate passed the gates", label);
        std::process::exit(4);
    }
    // ---- 2. cluster --------------------------------------------------------
    let mut cl: Vec<Vec<usize>> = Vec::new();
    for i in 0..ok.len() {
        let mut placed = false;
        for c in cl.iter_mut() {
            let (d, _, n) = pair(&ok[i].1, &ok[c[0]].1, race);
            if n > 0 && d <= agree {
                c.push(i);
                placed = true;
                break;
            }
        }
        if !placed {
            cl.push(vec![i]);
        }
    }
    println!("{} distinct answer(s) among them:", cl.len());
    for (ci, c) in cl.iter().enumerate() {
        println!(
            "  cluster {}: {} run(s)  [{}]",
            ci,
            c.len(),
            c.iter()
                .map(|i| ok[*i].0.rsplit('/').next().unwrap_or("").to_string())
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    // ---- 3. the DOUBLE-BUFFER rule, applied BEFORE reproduction ------------
    //
    // ORDER MATTERS AND I GOT IT WRONG THE FIRST TIME. I required an answer to
    // be reproduced by two runs before the live-copy rule could look at it, and
    // on 270051/m270051_4830 that chose a THREE-run cluster over a ONE-run
    // cluster sitting 0.331 m AHEAD of it along its own velocity -- i.e. it
    // chose the stale half of the double buffer because the stale half came up
    // more often. A majority is not evidence, and a reproduction count is a
    // majority. So supersession runs first:
    //
    //   cluster A SUPERSEDES cluster B when they are within `staleband` of each
    //   other AND A is ahead of B along B's own velocity by more than `stale`.
    //   That is the double-buffer relation and nothing else has that shape: a
    //   decoy belonging to another object is not 0.3 m away and collinear with
    //   the car's velocity, and the stale copy cannot be ahead of itself.
    //
    // A survivor may then be accepted on a single run PROVIDED it supersedes a
    // cluster that was itself reproduced -- because in that case the repeated
    // answer is demonstrably this answer, one tick late.
    let staleband = numflag(args, "--staleband", 1.0);
    let lagtol = numflag(args, "--lagtol", 0.01);
    let mut superseded = vec![false; cl.len()];
    let mut supersedes: Vec<Vec<usize>> = vec![Vec::new(); cl.len()];
    for a in 0..cl.len() {
        for b in 0..cl.len() {
            if a == b {
                continue;
            }
            let (d, al, _) = pair(&ok[cl[a][0]].1, &ok[cl[b][0]].1, race);
            if a < b {
                println!(
                    "  cluster {} vs {}: {:.6} m apart, {:+.6} m along the other's velocity",
                    a, b, d, al
                );
            }
            // (i) the SUB-SAMPLE stale copy: same place, a fraction of a sample
            //     behind along its own velocity (the classic double buffer --
            //     0.32 m on 270051, 0.09 m on 249521).
            let mut stale_of_a = d <= staleband && al > stale;
            // (ii) the INTEGER-SAMPLE lagged copy: B's own path, shifted whole
            //      samples into the past. Measured on 145875 and 191465 as
            //      EXACTLY 0.000000 m at a lag of two samples (100 ms), while
            //      the two sit 7.6 m and 8.6 m apart at lag zero -- far outside
            //      any "they are close together" band, which is why the first
            //      rule alone let them through. A different OBJECT does not
            //      reproduce the car's path exactly at any shift.
            if !stale_of_a {
                let prof = lag_profile(&ok[cl[b][0]].1, &ok[cl[a][0]].1, race, 8);
                if let Some((k, m, _)) = prof
                    .iter()
                    .filter(|(k, _, _)| *k < 0)
                    .min_by(|x, y| x.1.total_cmp(&y.1))
                {
                    if *m <= lagtol && d > *m {
                        println!(
                            "  cluster {} is cluster {}'s own path delayed by {} sample(s) (residual {:.6} m)",
                            b, a, -k, m
                        );
                        stale_of_a = true;
                    }
                }
            }
            if stale_of_a {
                superseded[b] = true;
                supersedes[a].push(b);
            }
        }
    }
    for (i, s) in superseded.iter().enumerate() {
        if *s {
            println!("  cluster {} is a STALE COPY of another candidate (behind it along its own velocity)", i);
        }
    }
    let live: Vec<usize> = (0..cl.len()).filter(|i| !superseded[*i]).collect();
    let good: Vec<usize> = live
        .iter()
        .copied()
        .filter(|i| cl[*i].len() >= minruns || supersedes[*i].iter().any(|j| cl[*j].len() >= minruns))
        .collect();
    if good.is_empty() {
        println!(
            "REFUSE {} no answer was reproduced by {} independent runs, and none supersedes one that was",
            label, minruns
        );
        std::process::exit(4);
    }
    if good.len() != 1 {
        println!(
            "REFUSE {} {} independent answers survive the live-copy rule, not one",
            label,
            good.len()
        );
        std::process::exit(4);
    }
    let win = good[0];
    let chosen = &ok[cl[win][0]].0;
    println!(
        "CHOSEN {} {}  (cluster {}, {} independent runs agreeing to <= {} m)",
        label,
        chosen,
        win,
        cl[win].len(),
        agree
    );
    if let Some(o) = flag(args, "--out") {
        std::fs::copy(chosen, &o).unwrap_or_else(|e| panic!("copy to {}: {}", o, e));
        println!("wrote {}", o);
    }
}

/// `tmtraj nan vres` -- is a trajectory's velocity field consistent with its own
/// motion, and by WHICH test?
///
/// The locator's acceptance test is `median |dp/dt - v0| < max(0.02*speed, 0.25)`.
/// That is a FIRST-ORDER comparison: `dp/dt` is the AVERAGE velocity over the
/// step and `v0` is the INSTANTANEOUS velocity at its start, so for any run at
/// all the two differ by `|a|*dt/2`. On a gentle map that is invisible; on a map
/// whose furniture accelerates the car it is metres per second, and the test
/// then rejects the real car for being real.
///
/// So this prints BOTH residuals on the same data:
///   FIRST-ORDER   |dp/dt - v0|              -- what the locator tests
///   TRAPEZOID     |dp/dt - (v0+v1)/2|       -- exact for constant acceleration
/// If the first is large and the second is small, the run is fine and the TEST
/// is what needs fixing. If BOTH are large, the car is genuinely not following
/// its own velocity, and where that happens is the finding.
fn cmd_vres(args: &[String]) {
    if let Some(g) = flag(args, "--ghost") {
        // The same test on a ghost's OWN recorded samples. This is how the
        // control is obtained: a DOWNLOADED human recording, made by the game,
        // carries position and velocity in every sample, so it says what the
        // residual looks like on a run nobody disputes.
        let s0 = decode(&g).unwrap_or_else(|e| panic!("{}: {}", g, e));
        let lo = numflag(args, "--from-ms", f64::MIN);
        let hi = numflag(args, "--to-ms", f64::MAX);
        let s: Vec<&S> = s0
            .iter()
            .filter(|k| k.finite && (k.ms as f64) >= lo && (k.ms as f64) <= hi)
            .collect();
        let t: Vec<f64> = s.iter().map(|k| k.ms as f64).collect();
        let p: Vec<[f64; 3]> = s.iter().map(|k| k.pos).collect();
        let v: Vec<[f64; 3]> = s.iter().map(|k| k.vel).collect();
        vres_report(&t, &p, &v);
        return;
    }
    let path = flag(args, "--csv").expect("--csv FILE (time_ms,x,y,z,...,vx,vy,vz,...)");
    let txt = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {}", path, e));
    let mut lines = txt.lines();
    let hdr: Vec<&str> = lines.next().expect("header").split(',').collect();
    let col = |n: &str| hdr.iter().position(|h| *h == n).unwrap_or_else(|| panic!("no column {}", n));
    let (ct, cx, cy, cz) = (col("time_ms"), col("x"), col("y"), col("z"));
    let (cvx, cvy, cvz) = (col("vx"), col("vy"), col("vz"));
    let mut t: Vec<f64> = Vec::new();
    let mut p: Vec<[f64; 3]> = Vec::new();
    let mut v: Vec<[f64; 3]> = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        if f.len() <= cvz {
            continue;
        }
        let g = |i: usize| f[i].trim().parse::<f64>().unwrap_or(f64::NAN);
        let (tt, pp, vv) = (g(ct), [g(cx), g(cy), g(cz)], [g(cvx), g(cvy), g(cvz)]);
        if !tt.is_finite() || pp.iter().chain(vv.iter()).any(|x| !x.is_finite()) {
            continue;
        }
        if tt < numflag(args, "--from-ms", f64::MIN) || tt > numflag(args, "--to-ms", f64::MAX) {
            continue;
        }
        t.push(tt);
        p.push(pp);
        v.push(vv);
    }
    vres_report(&t, &p, &v);
}

fn vres_report(t: &[f64], p: &[[f64; 3]], v: &[[f64; 3]]) {
    println!("{} usable rows, {:.3} .. {:.3} s", t.len(), t[0] / 1000.0, t[t.len() - 1] / 1000.0);
    let mut fo: Vec<f64> = Vec::new();
    let mut tz: Vec<f64> = Vec::new();
    let mut acc: Vec<f64> = Vec::new();
    let mut worst: Vec<(f64, f64, f64, f64)> = Vec::new(); // t, trapezoid, first-order, speed
    for i in 0..t.len() - 1 {
        let dt = (t[i + 1] - t[i]) / 1000.0;
        if dt <= 0.0 {
            continue;
        }
        let mut a = 0.0;
        let mut b = 0.0;
        let mut c = 0.0;
        for k in 0..3 {
            let dp = (p[i + 1][k] - p[i][k]) / dt;
            a += (dp - v[i][k]).powi(2);
            b += (dp - 0.5 * (v[i][k] + v[i + 1][k])).powi(2);
            c += ((v[i + 1][k] - v[i][k]) / dt).powi(2);
        }
        let (a, b, c) = (a.sqrt(), b.sqrt(), c.sqrt());
        fo.push(a);
        tz.push(b);
        acc.push(c);
        let sp = (v[i][0].powi(2) + v[i][1].powi(2) + v[i][2].powi(2)).sqrt();
        worst.push((t[i + 1] / 1000.0, b, a, sp));
    }
    let sp: Vec<f64> = v
        .iter()
        .map(|k| (k[0].powi(2) + k[1].powi(2) + k[2].powi(2)).sqrt())
        .collect();
    println!("mean speed {:.2} m/s", sp.iter().sum::<f64>() / sp.len() as f64);
    for (n, x) in [("FIRST-ORDER |dp/dt - v0|   ", fo.clone()), ("TRAPEZOID   |dp/dt - v̄|    ", tz.clone()), ("|acceleration|             ", acc.clone())] {
        println!(
            "{} median {:8.4}   p90 {:8.4}   p99 {:9.4}   max {:11.4}",
            n,
            quant(&mut x.clone(), 0.5),
            quant(&mut x.clone(), 0.9),
            quant(&mut x.clone(), 0.99),
            quant(&mut x.clone(), 1.0)
        );
    }
    // The prediction the first-order test makes if the ONLY thing wrong with it
    // is that it ignores the acceleration over the step.
    let dt = (t[1] - t[0]) / 1000.0;
    let mut pred: Vec<f64> = acc.iter().map(|a| a * dt / 2.0).collect();
    println!(
        "predicted first-order residual from |a|*dt/2 at dt={:.0} ms: median {:.4}, p90 {:.4}",
        dt * 1000.0,
        quant(&mut pred.clone(), 0.5),
        quant(&mut pred, 0.9)
    );
    // LAG SWEEP. If the velocity field is simply LABELLED at a different
    // instant from the position (pre- vs post-integration inside a tick, or a
    // one-tick buffer offset), the residual collapses at some shift. That is a
    // labelling fact about the readout, not a physics fact about the run, and
    // the two look identical until you sweep.
    println!("\nlag sweep -- median |dp/dt - v(i+k)| for a shift of k ticks:");
    for k in -2i64..=2 {
        let mut r: Vec<f64> = Vec::new();
        for i in 0..t.len() - 1 {
            let j = i as i64 + k;
            if j < 0 || j as usize >= v.len() {
                continue;
            }
            let dt = (t[i + 1] - t[i]) / 1000.0;
            if dt <= 0.0 {
                continue;
            }
            let e: f64 = (0..3)
                .map(|c| ((p[i + 1][c] - p[i][c]) / dt - v[j as usize][c]).powi(2))
                .sum::<f64>()
                .sqrt();
            r.push(e);
        }
        println!("  k = {:+}   median {:8.4}   p90 {:8.4}", k, quant(&mut r.clone(), 0.5), quant(&mut r, 0.9));
    }
    worst.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("\nworst TRAPEZOID residuals (a real discontinuity, not an acceleration):");
    println!("   t (s)   trapezoid   first-order   speed m/s");
    for w in worst.iter().take(12) {
        println!("  {:7.3} {:11.4} {:13.4} {:11.2}", w.0, w.1, w.2, w.3);
    }
}

/// `tmtraj nan csvcmp GHOST --csv ROUTE.csv` -- compare a regenerated ghost
/// against an INDEPENDENTLY produced per-tick route dump of the same run.
///
/// This is a control that can fail: the route comes from a different readout
/// path (`fk btraj2`, a forked-resume locate) on a different machine, and the
/// ghost comes from the parent-sampled clean run. They share the input tape and
/// the map and nothing else. Also reports the body-vs-velocity aim spread of
/// BOTH, so an aim figure can be attributed to the RUN rather than to the
/// regeneration.
fn cmd_csvcmp(args: &[String]) {
    let ghost = args
        .iter()
        .find(|a| a.ends_with(".Gbx"))
        .expect("a ghost path");
    let path = flag(args, "--csv").expect("--csv ROUTE.csv");
    let race = flag(args, "--race").and_then(|v| v.parse::<i64>().ok()).unwrap_or(i64::MAX);
    let txt = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {}", path, e));
    let mut lines = txt.lines();
    let hdr: Vec<&str> = lines.next().expect("header").split(',').collect();
    let col = |n: &str| hdr.iter().position(|h| *h == n);
    let (ct, cx, cy, cz) = (col("time_ms").unwrap(), col("x").unwrap(), col("y").unwrap(), col("z").unwrap());
    let (cvx, cvy, cvz) = (col("vx").unwrap(), col("vy").unwrap(), col("vz").unwrap());
    let qcols = (col("qx"), col("qy"), col("qz"), col("qw"));
    let mut route: std::collections::HashMap<i64, ([f64; 3], [f64; 3], Option<[f64; 4]>)> =
        std::collections::HashMap::new();
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        if f.len() <= cvz {
            continue;
        }
        let g = |i: usize| f[i].trim().parse::<f64>().unwrap_or(f64::NAN);
        let t = g(ct);
        if !t.is_finite() {
            continue;
        }
        let q = match qcols {
            (Some(a), Some(b), Some(c), Some(d)) if f.len() > d => Some([g(a), g(b), g(c), g(d)]),
            _ => None,
        };
        route.insert(t as i64, ([g(cx), g(cy), g(cz)], [g(cvx), g(cvy), g(cvz)], q));
    }
    let s = decode(ghost).unwrap_or_else(|e| panic!("{}: {}", ghost, e));
    let mut d = Vec::new();
    let mut ang = Vec::new();
    for k in s.iter().filter(|k| k.ms <= race && k.finite) {
        let Some((p, _v, q)) = route.get(&k.ms) else { continue };
        d.push((0..3).map(|i| (k.pos[i] - p[i]).powi(2)).sum::<f64>().sqrt());
        if let Some(q) = q {
            let dot: f64 = (0..4).map(|i| k.quat[i] * q[i]).sum::<f64>().abs().clamp(0.0, 1.0);
            ang.push(2.0 * dot.acos().to_degrees());
        }
    }
    if d.is_empty() {
        println!("ABORT: no sample instant is in the route dump");
        std::process::exit(3);
    }
    println!(
        "matched {} of {} in-race samples against the route dump",
        d.len(),
        s.iter().filter(|k| k.ms <= race).count()
    );
    println!(
        "POSITION  median {:.6} m   p90 {:.6}   p99 {:.6}   max {:.6}",
        quant(&mut d.clone(), 0.5),
        quant(&mut d.clone(), 0.9),
        quant(&mut d.clone(), 0.99),
        quant(&mut d.clone(), 1.0)
    );
    if !ang.is_empty() {
        println!(
            "ORIENT    median {:.4} deg  p99 {:.4}   max {:.4}",
            quant(&mut ang.clone(), 0.5),
            quant(&mut ang.clone(), 0.99),
            quant(&mut ang.clone(), 1.0)
        );
    }
    // The aim spread of the ROUTE's own quaternions: attributes an aim figure
    // to the run rather than to the regeneration.
    let mut rs: Vec<S> = Vec::new();
    let mut ks: Vec<i64> = route.keys().copied().collect();
    ks.sort();
    for t in ks {
        let (p, v, q) = route[&t];
        let Some(q) = q else { continue };
        let sp = (v[0].powi(2) + v[1].powi(2) + v[2].powi(2)).sqrt();
        rs.push(S { ms: t, pos: p, quat: q, speed: sp, vel: v, finite: true });
    }
    let inr: Vec<S> = rs.into_iter().filter(|k| k.ms <= race).collect();
    let (a, n) = aim_spread(&inr, 5.0);
    let gs: Vec<S> = s
        .iter()
        .filter(|k| k.ms <= race && k.finite)
        .map(|k| S { ms: k.ms, pos: k.pos, quat: k.quat, speed: k.speed, vel: k.vel, finite: true })
        .collect();
    let (b, m) = aim_spread(&gs, 5.0);
    println!(
        "AIM spread (body vs velocity, p95 about the circular mean): route dump {:.1} deg over {} \
         samples, regenerated ghost {:.1} deg over {}",
        a, n, b, m
    );
}

/// Median distance from A(t) to B(t + k samples), for a range of integer lags.
/// A copy of the car's own state held one or more ticks behind is not merely
/// "near" the live one -- it is the SAME PATH, shifted in time -- so its
/// distance collapses at some k > 0 while a genuinely different object's does
/// not. This subsumes the one-tick double-buffer case and catches the deeper
/// history buffers (measured: 145875 lags by ~7.5 m, 191465 by ~8.6 m, both
/// far outside any "they are close together" band).
pub fn lag_profile(a: &[S], b: &[S], race: i64, kmax: i64) -> Vec<(i64, f64, usize)> {
    let bm: std::collections::HashMap<i64, &S> = b.iter().map(|k| (k.ms, k)).collect();
    let dt = if a.len() > 1 { a[1].ms - a[0].ms } else { 50 };
    let mut out = Vec::new();
    for k in -kmax..=kmax {
        let mut d = Vec::new();
        for s in a.iter().filter(|s| s.ms <= race && s.finite) {
            let Some(o) = bm.get(&(s.ms + k * dt)) else { continue };
            if !o.finite {
                continue;
            }
            d.push((0..3).map(|i| (s.pos[i] - o.pos[i]).powi(2)).sum::<f64>().sqrt());
        }
        let n = d.len();
        if n >= 8 {
            out.push((k, quant(&mut d, 0.5), n));
        }
    }
    out
}

fn cmd_lag(args: &[String]) {
    let files: Vec<&String> = args.iter().filter(|a| a.ends_with(".Gbx")).collect();
    let race = flag(args, "--race").and_then(|v| v.parse::<i64>().ok()).unwrap_or(i64::MAX);
    let a = decode(files[0]).unwrap();
    let b = decode(files[1]).unwrap();
    println!("lag   median |A(t) - B(t+k)|   n");
    for (k, d, n) in lag_profile(&a, &b, race, 20) {
        if d < 40.0 {
            println!("{:+4}   {:18.6}   {}", k, d, n);
        }
    }
}
