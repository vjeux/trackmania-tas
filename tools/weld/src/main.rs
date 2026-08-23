//! `weld` — the state-space alignment instrument, and the tape splice it implies.
//!
//! ## Why this exists
//!
//! On map 146612 every upstream gain measured so far is unspendable: a tape
//! that reaches CP3 104 ms earlier is 1.5 s worse fourteen seconds later.
//! The standing reading of that is "the gain anti-composes". There is a second
//! reading, and it is the one this tool tests: **the tail is not a function of
//! the prefix, it is an ASSET** — nineteen seconds of converged, hand-repaired
//! driving that only works from the state it was derived for. An upstream
//! rewrite may only be spent in the currency the tail accepts: its own state.
//!
//! So the question is not "how much time can the prefix find" but "can a
//! prefix reach the tail's own state EARLIER". That is a question about two
//! trajectories, and `align` answers it: for every sample of B, the closest
//! state on A's line, and how many ticks of lead that closeness is worth.
//!
//! `splice` then builds the tape the alignment names: prefix from one tape,
//! tail from another, shifted by a whole number of ticks. It works on the
//! `gtape` text form so that `ghost` stays the only writer of a ghost.
//!
//! Every subcommand prints the control it ran.

use std::collections::HashMap;
use std::fmt::Write as _;

#[derive(Clone, Copy)]
struct S {
    t: i64,
    x: f64,
    y: f64,
    z: f64,
    v: f64,
    vx: f64,
    vy: f64,
    vz: f64,
    q: [f64; 4],
}

fn parse_csv(path: &str) -> Result<Vec<S>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
    let mut out = Vec::new();
    let mut hdr: HashMap<String, usize> = HashMap::new();
    for (i, line) in txt.lines().enumerate() {
        if i == 0 {
            for (j, c) in line.split(',').enumerate() {
                hdr.insert(c.trim().to_string(), j);
            }
            for need in ["time_ms", "x", "y", "z", "speed_ms", "vx", "vy", "vz", "qx", "qy", "qz", "qw"] {
                if !hdr.contains_key(need) {
                    return Err(format!("{}: column {} missing", path, need));
                }
            }
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        let g = |k: &str| -> f64 { f[hdr[k]].parse::<f64>().unwrap_or(f64::NAN) };
        out.push(S {
            t: g("time_ms") as i64,
            x: g("x"),
            y: g("y"),
            z: g("z"),
            v: g("speed_ms"),
            vx: g("vx"),
            vy: g("vy"),
            vz: g("vz"),
            q: [g("qx"), g("qy"), g("qz"), g("qw")],
        });
    }
    if out.is_empty() {
        return Err(format!("{}: no rows", path));
    }
    Ok(out)
}

fn dpos(a: &S, b: &S) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}
fn dvel(a: &S, b: &S) -> f64 {
    ((a.vx - b.vx).powi(2) + (a.vy - b.vy).powi(2) + (a.vz - b.vz).powi(2)).sqrt()
}
/// Angle between two attitudes, in degrees. `q` and `-q` are the same attitude.
fn dang(a: &S, b: &S) -> f64 {
    let d: f64 = (0..4).map(|i| a.q[i] * b.q[i]).sum();
    2.0 * d.abs().min(1.0).acos() * 180.0 / std::f64::consts::PI
}

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}
fn need(args: &[String], k: &str) -> Result<String, String> {
    arg(args, k).ok_or_else(|| format!("{} is required", k))
}
fn numf(args: &[String], k: &str, d: f64) -> Result<f64, String> {
    match arg(args, k) {
        None => Ok(d),
        Some(s) => s.parse::<f64>().map_err(|_| format!("{} wants a number, got {}", k, s)),
    }
}
fn numi(args: &[String], k: &str, d: i64) -> Result<i64, String> {
    match arg(args, k) {
        None => Ok(d),
        Some(s) => s.parse::<i64>().map_err(|_| format!("{} wants an integer, got {}", k, s)),
    }
}

// ---------------------------------------------------------------- align

/// For every sample of B in the window, the closest state on A's whole line.
///
/// `lead` is `t_A - t_B`: how far ahead of A's schedule B is when it stands
/// where A stood. A positive lead with a small `dpos`, `dvel` and `dang` is a
/// weld candidate — B may hand over to A's tail, shifted by `lead`.
fn cmd_align(args: &[String]) -> Result<(), String> {
    let a = parse_csv(&need(args, "--a")?)?;
    let b = parse_csv(&need(args, "--b")?)?;
    let lo = numi(args, "--lo", i64::MIN)?;
    let hi = numi(args, "--hi", i64::MAX)?;
    let every = numi(args, "--every", 100)?.max(10);
    let maxlead = numi(args, "--maxlead", 2000)?;
    let vw = numf(args, "--velweight", 1.0)?;
    let aw = numf(args, "--angweight", 0.1)?;

    println!("# a={} ({} rows)  b={} ({} rows)", need(args, "--a")?, a.len(), need(args, "--b")?, b.len());
    println!("# key = dpos_m + {} * dvel_ms + {} * dang_deg", vw, aw);
    println!("t_b_ms\tt_a_ms\tlead_ms\tdpos_m\tdvel_ms\tdang_deg\tkey\tspeed_b\tspeed_a");
    let mut best_overall: Option<(f64, i64, i64)> = None;
    for s in b.iter() {
        if s.t < lo || s.t > hi || (s.t - b[0].t) % every != 0 {
            continue;
        }
        let mut best: Option<(f64, &S)> = None;
        for r in a.iter() {
            if (r.t - s.t).abs() > maxlead {
                continue;
            }
            let k = dpos(r, s) + vw * dvel(r, s) + aw * dang(r, s);
            if best.map_or(true, |(bk, _)| k < bk) {
                best = Some((k, r));
            }
        }
        let (k, r) = match best {
            Some(v) => v,
            None => continue,
        };
        println!(
            "{}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.2}\t{:.2}",
            s.t,
            r.t,
            r.t - s.t,
            dpos(r, s),
            dvel(r, s),
            dang(r, s),
            k,
            s.v,
            r.v
        );
        if r.t - s.t > 0 && best_overall.map_or(true, |(bk, _, _)| k < bk) {
            best_overall = Some((k, s.t, r.t));
        }
    }
    match best_overall {
        Some((k, tb, ta)) => println!(
            "\nbest weld with positive lead: B at {} ms == A at {} ms, lead {} ms, key {:.3}",
            tb,
            ta,
            ta - tb,
            k
        ),
        None => println!("\nno sample of B is ever ahead of A on A's own line"),
    }
    Ok(())
}

// ---------------------------------------------------------------- gtape

#[derive(Clone)]
struct GTape {
    head: Vec<String>,
    rows: Vec<String>, // one per tick, in order, with the `t=` field rewritten on write
}

fn read_gtape(path: &str) -> Result<GTape, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
    let mut head = Vec::new();
    let mut rows = Vec::new();
    for line in txt.lines() {
        if line.starts_with("t=") {
            rows.push(line.to_string());
        } else {
            if rows.is_empty() {
                head.push(line.to_string());
            } else {
                return Err(format!("{}: non-tick line after the tick rows: {}", path, line));
            }
        }
    }
    if rows.is_empty() {
        return Err(format!("{}: no tick rows", path));
    }
    for (i, r) in rows.iter().enumerate() {
        let t: i64 = r[2..].split_whitespace().next().unwrap_or("x").parse().map_err(|_| {
            format!("{}: row {} does not start with a tick index: {}", path, i, r)
        })?;
        if t != i as i64 {
            return Err(format!("{}: row {} says t={}; the rows must be dense and in order", path, i, t));
        }
    }
    Ok(GTape { head, rows })
}

fn retick(row: &str, t: usize) -> String {
    let rest = row.split_once(' ').map(|(_, r)| r).unwrap_or("");
    format!("t={} {}", t, rest)
}

fn write_gtape(path: &str, g: &GTape) -> Result<(), String> {
    let mut s = String::new();
    for h in &g.head {
        let _ = writeln!(s, "{}", h);
    }
    for (i, r) in g.rows.iter().enumerate() {
        let _ = writeln!(s, "{}", retick(r, i));
    }
    std::fs::write(path, s).map_err(|e| format!("{}: {}", path, e))
}

/// `out[i] = prefix[i]` for `i < at`, and `out[i] = tail[i + shift]` after it.
///
/// The tape keeps the prefix's length: a run that finishes earlier simply
/// stops reading. Ticks past the end of the shifted tail repeat its last row,
/// which is inert — the run is over by then, and the control below proves it.
fn splice(prefix: &GTape, tail: &GTape, at: usize, shift: i64) -> Result<GTape, String> {
    if at > prefix.rows.len() {
        return Err(format!("--at {} is past the prefix's {} ticks", at, prefix.rows.len()));
    }
    let mut rows = Vec::with_capacity(prefix.rows.len());
    for i in 0..prefix.rows.len() {
        if i < at {
            rows.push(prefix.rows[i].clone());
        } else {
            let j = i as i64 + shift;
            let j = if j < 0 {
                0
            } else if j as usize >= tail.rows.len() {
                tail.rows.len() - 1
            } else {
                j as usize
            };
            rows.push(tail.rows[j].clone());
        }
    }
    Ok(GTape { head: prefix.head.clone(), rows })
}

fn cmd_splice(args: &[String]) -> Result<(), String> {
    let pp = need(args, "--prefix")?;
    let tp = need(args, "--tail")?;
    let at = numi(args, "--at", -1)?;
    let shift = numi(args, "--shift", 0)?;
    let out = need(args, "--out")?;
    if at < 0 {
        return Err("--at TICK is required".into());
    }
    let p = read_gtape(&pp)?;
    let t = read_gtape(&tp)?;

    // THE CONTROL, every time: splicing a tape onto itself with no shift must
    // reproduce it row for row. It costs nothing and it is the only check that
    // the row surgery below is a no-op when it should be.
    let ident = splice(&p, &p, at as usize, 0)?;
    let same = ident.rows.iter().enumerate().all(|(i, r)| retick(r, i) == retick(&p.rows[i], i));
    if !same {
        return Err("identity control FAILED: prefix spliced onto itself is not the prefix".into());
    }

    let g = splice(&p, &t, at as usize, shift)?;
    write_gtape(&out, &g)?;
    let changed = g
        .rows
        .iter()
        .enumerate()
        .filter(|(i, r)| retick(r, *i) != retick(&p.rows[*i], *i))
        .count();
    println!(
        "identity control ok\nwrote {}  ({} ticks, {} rows differ from the prefix, tail from tick {} shifted {} ticks)",
        out,
        g.rows.len(),
        changed,
        at,
        shift
    );
    Ok(())
}

/// Print one tape's rows over a tick range, next to another's — for reading a
/// corner by hand rather than by summary statistic.
fn cmd_inputs(args: &[String]) -> Result<(), String> {
    let a = read_gtape(&need(args, "--a")?)?;
    let b = arg(args, "--b").map(|p| read_gtape(&p)).transpose()?;
    let lo = numi(args, "--lo", 0)?.max(0) as usize;
    let hi = numi(args, "--hi", 0)?.max(0) as usize;
    let f = |r: &str| -> String {
        let mut steer = "?".to_string();
        let mut acc = "?".to_string();
        let mut brk = "?".to_string();
        for tok in r.split_whitespace() {
            if let Some(v) = tok.strip_prefix("steer=") {
                steer = v.to_string();
            }
            if let Some(v) = tok.strip_prefix("accel=") {
                acc = v.to_string();
            }
            if let Some(v) = tok.strip_prefix("brake=") {
                brk = v.to_string();
            }
        }
        format!("{:>5} g{} b{}", steer, acc, brk)
    };
    for i in lo..=hi.min(a.rows.len() - 1) {
        match &b {
            None => println!("t={}\t{}", i, f(&a.rows[i])),
            Some(bb) => {
                let br = bb.rows.get(i).map(|r| f(r)).unwrap_or_else(|| "-".into());
                println!("t={}\t{}\t{}", i, f(&a.rows[i]), br)
            }
        }
    }
    Ok(())
}

/// Build every weld in a grid and ask the plain oracle about all of them.
///
/// One combination is one file: prefix below `at`, tail from `at + shift`.
/// The grid is the honest way to use the alignment — the alignment ORDERS the
/// candidates, the oracle DECIDES them, and a weld that the alignment likes
/// and the oracle refuses is exactly the measurement worth having.
///
/// Two controls ride in every batch, unasked: the tail alone at shift 0
/// (which must return the incumbent's own time) and the prefix alone (which
/// must return whatever it does on its own). If the first one moves, the
/// splice path is broken and no row in the table means anything.
fn cmd_sweep(args: &[String]) -> Result<(), String> {
    let pp = need(args, "--prefix")?;
    let tp = need(args, "--tail")?;
    let carrier = need(args, "--carrier")?;
    let map = need(args, "--map")?;
    let outdir = need(args, "--outdir")?;
    let tag = arg(args, "--tag").unwrap_or_else(|| "w".into());
    let ghost_bin = arg(args, "--ghost").unwrap_or_else(|| "ghost".into());
    let val_bin = arg(args, "--tmsearch").unwrap_or_else(|| "tmsearch".into());
    let at_lo = numi(args, "--at-lo", -1)?;
    let at_hi = numi(args, "--at-hi", -1)?;
    let at_step = numi(args, "--at-step", 1)?.max(1);
    // Optional THIRD tape: the prefix is itself a graft, `--pre` below `--pre-at`
    // and `--prefix` above it. That is what sweeps the lead/error frontier —
    // adopting more or less of a faster line before handing back to the tail.
    let pre = arg(args, "--pre").map(|p| read_gtape(&p)).transpose()?;
    let pre_ats: Vec<i64> = match arg(args, "--pre-ats") {
        None => vec![-1],
        Some(s) => s
            .split(',')
            .map(|x| x.trim().parse::<i64>().map_err(|_| format!("bad --pre-ats entry {}", x)))
            .collect::<Result<_, _>>()?,
    };
    let shifts: Vec<i64> = need(args, "--shifts")?
        .split(',')
        .map(|s| s.trim().parse::<i64>().map_err(|_| format!("bad --shifts entry {}", s)))
        .collect::<Result<_, _>>()?;
    if at_lo < 0 || at_hi < at_lo {
        return Err("--at-lo and --at-hi are required".into());
    }
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {}", outdir, e))?;
    let p = read_gtape(&pp)?;
    let t = read_gtape(&tp)?;
    let ident = splice(&p, &p, at_lo as usize, 0)?;
    if !ident.rows.iter().enumerate().all(|(i, r)| retick(r, i) == retick(&p.rows[i], i)) {
        return Err("identity control FAILED".into());
    }

    let mut jobs: Vec<(i64, i64, i64, String)> = Vec::new();
    // The control: the tail, spliced onto itself, shift 0. It must come back
    // as the incumbent's own time.
    jobs.push((-1, 0, -1, format!("{}/{}_CONTROL_tail.Ghost.Gbx", outdir, tag)));
    let mut at = at_lo;
    while at <= at_hi {
        for s in &shifts {
            for pa in &pre_ats {
                let name = if *pa < 0 {
                    format!("{}/{}_at{}_s{}.Ghost.Gbx", outdir, tag, at, s)
                } else {
                    format!("{}/{}_pre{}_at{}_s{}.Ghost.Gbx", outdir, tag, pa, at, s)
                };
                jobs.push((at, *s, *pa, name));
            }
        }
        at += at_step;
    }

    let nthreads = numi(args, "--jobs", 32)?.max(1) as usize;
    let jobs_arc = std::sync::Arc::new(jobs.clone());
    let idx = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let pre = std::sync::Arc::new(pre);
    let mut handles = Vec::new();
    for _ in 0..nthreads {
        let jobs = jobs_arc.clone();
        let idx = idx.clone();
        let pre = pre.clone();
        let (p, t, outdir, carrier, ghost_bin) =
            (p.clone(), t.clone(), outdir.clone(), carrier.clone(), ghost_bin.clone());
        handles.push(std::thread::spawn(move || -> Result<(), String> {
            loop {
                let i = idx.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= jobs.len() {
                    return Ok(());
                }
                let (at, shift, pre_at, out) = &jobs[i];
                let g = if *at < 0 {
                    splice(&t, &t, 0, 0)?
                } else {
                    let base = match (pre.as_ref(), *pre_at >= 0) {
                        (Some(pr), true) => splice(pr, &p, *pre_at as usize, 0)?,
                        _ => p.clone(),
                    };
                    splice(&base, &t, *at as usize, *shift)?
                };
                let gt = format!("{}.gtape", out);
                write_gtape(&gt, &g)?;
                let st = std::process::Command::new(&ghost_bin)
                    .args(["tape", "inject", &carrier, out, "--tape", &gt, "--allow-telemetry-mismatch"])
                    .output()
                    .map_err(|e| format!("{}: {}", ghost_bin, e))?;
                if !st.status.success() {
                    return Err(format!(
                        "ghost tape inject failed for {}: {}",
                        out,
                        String::from_utf8_lossy(&st.stderr)
                    ));
                }
            }
        }));
    }
    for h in handles {
        h.join().map_err(|_| "a writer thread panicked".to_string())??;
    }
    println!("wrote {} candidate files into {}", jobs.len(), outdir);

    let mut cmd = std::process::Command::new(&val_bin);
    cmd.arg("validate").arg("--map").arg(&map);
    for (_, _, _, o) in &jobs {
        cmd.arg(o);
    }
    let out = cmd.output().map_err(|e| format!("{}: {}", val_bin, e))?;
    print!("{}", String::from_utf8_lossy(&out.stdout));
    eprint!("{}", String::from_utf8_lossy(&out.stderr));
    Ok(())
}

/// How long a candidate stays on the reference line, and what lead it holds
/// while it does. One line per file: the summary the DNF checkpoint count
/// cannot give.
///
/// `off_at` is the first race time at which the candidate's closest approach to
/// the reference exceeds `--thresh` metres — the moment it stopped driving the
/// incumbent's line. `lead` is the lead in the last 500 ms before that.
fn cmd_survive(args: &[String]) -> Result<(), String> {
    let a = parse_csv(&need(args, "--a")?)?;
    let thresh = numf(args, "--thresh", 3.0)?;
    let from = numi(args, "--from", i64::MIN)?;
    let maxlead = numi(args, "--maxlead", 400)?;
    let files: Vec<String> = args
        .iter()
        .skip(2)
        .filter(|s| s.ends_with(".csv") && **s != need(args, "--a").unwrap_or_default())
        .cloned()
        .collect();
    println!("file\toff_at_ms\tlead_ms\tdpos_m\tdvel_ms");
    for f in files {
        let b = match parse_csv(&f) {
            Ok(v) => v,
            Err(e) => {
                println!("{}\tERR {}", f, e);
                continue;
            }
        };
        let mut off_at = None;
        let mut last: Option<(i64, f64, f64)> = None;
        for s in b.iter() {
            if s.t < from {
                continue;
            }
            let mut best: Option<(f64, &S)> = None;
            for r in a.iter() {
                if (r.t - s.t).abs() > maxlead {
                    continue;
                }
                let d = dpos(r, s);
                if best.map_or(true, |(bd, _)| d < bd) {
                    best = Some((d, r));
                }
            }
            if let Some((d, r)) = best {
                if d <= thresh {
                    last = Some((r.t - s.t, d, dvel(r, s)));
                } else if last.is_some() {
                    off_at = Some(s.t);
                    break;
                }
            }
        }
        let (lead, d, dv) = last.unwrap_or((0, f64::NAN, f64::NAN));
        match off_at {
            Some(t) => println!("{}\t{}\t{}\t{:.3}\t{:.3}", f, t, lead, d, dv),
            None => println!("{}\tstayed\t{}\t{:.3}\t{:.3}", f, lead, d, dv),
        }
    }
    Ok(())
}

/// Trace many tapes at once. `fk trace` is one process per tape and the locate
/// is the expensive part, so this just runs N of them in parallel and names the
/// CSVs after the tapes.
fn cmd_traces(args: &[String]) -> Result<(), String> {
    let map = need(args, "--map")?;
    let outdir = need(args, "--outdir")?;
    let at = numi(args, "--at", 400)?;
    let fk = arg(args, "--fk").unwrap_or_else(|| "fk".into());
    let nthreads = numi(args, "--jobs", 24)?.max(1) as usize;
    let skip: Vec<String> = vec![map.clone(), outdir.clone(), fk.clone()];
    let files: Vec<String> = args
        .iter()
        .skip(2)
        .filter(|s| s.ends_with(".Ghost.Gbx") && !skip.contains(s))
        .cloned()
        .collect();
    if files.is_empty() {
        return Err("no .Ghost.Gbx arguments".into());
    }
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {}", outdir, e))?;
    let files = std::sync::Arc::new(files);
    let idx = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut hs = Vec::new();
    for _ in 0..nthreads {
        let (files, idx, map, outdir, fk) =
            (files.clone(), idx.clone(), map.clone(), outdir.clone(), fk.clone());
        hs.push(std::thread::spawn(move || {
            loop {
                let i = idx.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= files.len() {
                    return;
                }
                let f = &files[i];
                let base = f.rsplit('/').next().unwrap_or(f).replace(".Ghost.Gbx", "");
                let out = format!("{}/{}.csv", outdir, base);
                let _ = std::process::Command::new(&fk)
                    .args([
                        "trace",
                        "--tape",
                        f,
                        "--map",
                        &map,
                        "--at",
                        &format!("tick:{}", at),
                        "--out",
                        &out,
                    ])
                    .output();
            }
        }));
    }
    for h in hs {
        let _ = h.join();
    }
    println!("traced {} tapes into {}", files.len(), outdir);
    Ok(())
}

/// Every `CPlugEntRecordData` node inside a file — a map, not just a ghost.
///
/// A `.Map.Gbx` can carry the author's own recordings. 146612 carries
/// **thirteen distinct ones**, and the fleet's earlier survey read only their
/// END TIMES, concluded that none of them is the author-time lap, and stopped.
/// That answers "can we extract the AT" and leaves the interesting question
/// untouched: these are the map author's own driving, including mid-map
/// practice segments, on a map whose author time nobody can reach. What LINE
/// do they take?
///
/// So this decodes every node and writes each one's trajectory, rather than
/// printing a table of end times.
fn cmd_maprec(args: &[String]) -> Result<(), String> {
    let path = need(args, "--file")?;
    let outdir = arg(args, "--outdir");
    let body = gbx::record::load_body(&path)?;
    let needle = gbx::record::CLASS_CPLUGENTRECORDDATA.to_le_bytes();
    let mut sites: Vec<usize> = Vec::new();
    let mut i = 0usize;
    while i + 4 <= body.len() {
        if body[i..i + 4] == needle {
            sites.push(i);
        }
        i += 1;
    }
    if let Some(d) = &outdir {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {}", d, e))?;
    }
    println!("{}: {} occurrences of the class id", path, sites.len());
    println!("node\toffset\tver\tstart_s\tend_s\tsamples\tsize\tfirst_xyz\tlast_xyz");
    let mut seen: Vec<u64> = Vec::new();
    let mut n = 0;
    for &hit in &sites {
        // Walk over a repeated class id (node class id then chunk id), the way
        // gbx::record::find_entrecord_blob does.
        let mut q = hit;
        while q + 8 <= body.len() && body[q + 4..q + 8] == needle {
            q += 4;
        }
        let p = q + 4;
        if p + 12 > body.len() {
            continue;
        }
        let g = |o: usize| u32::from_le_bytes(body[o..o + 4].try_into().unwrap()) as usize;
        let (version, usz, csz) = (g(p), g(p + 4), g(p + 8));
        if !(1..=20).contains(&version) || csz == 0 || usz == 0 || p + 12 + csz > body.len() {
            continue;
        }
        if body[p + 12..p + 14] != [0x78, 0x9c] {
            continue;
        }
        let key = ((p as u64) << 32) | csz as u64;
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let blob = match miniz_oxide::inflate::decompress_to_vec_zlib(&body[p + 12..p + 12 + csz])
            .map_err(|e| format!("{:?}", e))
            .and_then(|b| if b.len() == usz { Ok(b) } else { Err(format!("size {} != {}", b.len(), usz)) })
        {
            Ok(b) => b,
            Err(e) => {
                println!("{}\t{}\tzlib error {}", n, p, e);
                n += 1;
                continue;
            }
        };
        let rec = match gbx::record::parse_record_data(&blob, version as u32) {
            Ok(r) => r,
            Err(e) => {
                println!("{}\t{}\tparse error {}", n, p, e);
                n += 1;
                continue;
            }
        };
        // The vehicle entity, by CLASS ID -- never by sample size, which picks
        // a foreign entity on a container that carries a bigger one.
        let mut best: Option<(&gbx::record::Ent, usize)> = None;
        for e in &rec.ents {
            let cid = rec.descs.get(e.type_.max(0) as usize).map(|d| d.class_id);
            if cid == Some(gbx::record::CLASS_CSCENEVEHICLEVIS) && e.times.len() > best.as_ref().map_or(0, |(b, _)| b.times.len()) {
                best = Some((e, e.sample_size));
            }
        }
        let (ent, ssz) = match best {
            Some(v) => v,
            None => {
                println!("{}\t{}\tv{}\t{:.3}\t{:.3}\t-\t-\tno vehicle entity", n, p, version, rec.start_ms as f64 / 1000.0, rec.end_ms as f64 / 1000.0);
                n += 1;
                continue;
            }
        };
        let mut samples = Vec::with_capacity(ent.times.len());
        for (k, t) in ent.times.iter().enumerate() {
            let d = &ent.raw[k * ssz..(k + 1) * ssz];
            let mut s = gbx::record::decode_vehicle_sample(d);
            s.time_ms = *t;
            samples.push(s);
        }
        let f = samples.first();
        let l = samples.last();
        println!(
            "{}\t{}\tv{}\t{:.3}\t{:.3}\t{}\t{}\t{}\t{}",
            n,
            p,
            version,
            rec.start_ms as f64 / 1000.0,
            rec.end_ms as f64 / 1000.0,
            samples.len(),
            ssz,
            f.map(|s| format!("{:.1},{:.1},{:.1}", s.x, s.y, s.z)).unwrap_or_default(),
            l.map(|s| format!("{:.1},{:.1},{:.1}", s.x, s.y, s.z)).unwrap_or_default(),
        );
        if let Some(d) = &outdir {
            let mut out = String::from(
                "time_ms,x,y,z,speed_kmh,speed_ms,vx,vy,vz,yaw,pitch,roll,qx,qy,qz,qw,gear,rpm_raw,steer,gas,brake\n",
            );
            for s in &samples {
                let _ = writeln!(
                    out,
                    "{},{:.4},{:.4},{:.4},{:.3},{:.4},{:.4},{:.4},{:.4},{:.5},{:.5},{:.5},{:.6},{:.6},{:.6},{:.6},{},{},{},{},{}",
                    s.time_ms, s.x, s.y, s.z, s.speed_kmh, s.speed_ms, s.vx, s.vy, s.vz,
                    s.yaw, s.pitch, s.roll, s.qx, s.qy, s.qz, s.qw,
                    s.gear, s.rpm_raw, s.steer, s.gas, s.brake
                );
            }
            std::fs::write(format!("{}/node{:02}.csv", d, n), out)
                .map_err(|e| format!("{}: {}", d, e))?;
        }
        n += 1;
    }
    println!("{} distinct record-data nodes decoded", n);
    Ok(())
}

/// Closest approach to each named point, and when. A checkpoint ladder that
/// needs no segment map and works on a recording nobody can re-simulate —
/// which is the only way to time the author's own embedded practice runs.
fn cmd_cps(args: &[String]) -> Result<(), String> {
    let pts: Vec<[f64; 3]> = need(args, "--pts")?
        .split(';')
        .map(|p| {
            let v: Vec<f64> = p.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if v.len() == 3 {
                Ok([v[0], v[1], v[2]])
            } else {
                Err(format!("bad point {:?}", p))
            }
        })
        .collect::<Result<_, _>>()?;
    let files: Vec<String> = args.iter().skip(2).filter(|s| s.ends_with(".csv")).cloned().collect();
    print!("file\tsamples\tend_s");
    for i in 0..pts.len() {
        print!("\tp{}_t\tp{}_d", i, i);
    }
    println!();
    for f in files {
        let b = parse_csv(&f)?;
        print!("{}\t{}\t{:.3}", f, b.len(), b.last().map(|s| s.t).unwrap_or(0) as f64 / 1000.0);
        for p in &pts {
            let mut best = (f64::MAX, 0i64);
            for s in &b {
                let d = ((s.x - p[0]).powi(2) + (s.y - p[1]).powi(2) + (s.z - p[2]).powi(2)).sqrt();
                if d < best.0 {
                    best = (d, s.t);
                }
            }
            print!("\t{:.3}\t{:.1}", best.1 as f64 / 1000.0, best.0);
        }
        println!();
    }
    Ok(())
}

/// Speed envelope over a window — what a `floor` predicate may safely demand.
fn cmd_speed(args: &[String]) -> Result<(), String> {
    let lo = numi(args, "--lo", i64::MIN)?;
    let hi = numi(args, "--hi", i64::MAX)?;
    let files: Vec<String> = args.iter().skip(2).filter(|s| s.ends_with(".csv")).cloned().collect();
    println!("file\tn\tmin_ms\tp05\tmean\tmax");
    for f in files {
        let b = parse_csv(&f)?;
        let mut v: Vec<f64> = b.iter().filter(|s| s.t >= lo && s.t <= hi).map(|s| s.v).collect();
        if v.is_empty() {
            println!("{}\t0", f);
            continue;
        }
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{}\t{}\t{:.1}\t{:.1}\t{:.1}\t{:.1}",
            f,
            v.len(),
            v[0],
            v[(v.len() as f64 * 0.05) as usize],
            mean,
            v[v.len() - 1]
        );
    }
    Ok(())
}

/// Emit a `tmmaps ladder` spec: one rung per sample, as GRID CELLS on the
/// reference line, each rung a curtain of gates laid across the direction of
/// travel.
///
/// `tmmaps rungspec` writes free-block positions, which the mover refuses for
/// a grid-placed Goal block — and this map's four finish gates are all grid.
/// The cell arithmetic is the map's own: centre = (32cx+16, 8cy-62, 32cz+16),
/// so cx = floor(x/32), cz = floor(z/32), cy = floor((y+66)/8), which
/// reproduces every waypoint cell `tmmaps waypoints` prints.
fn cmd_rungcells(args: &[String]) -> Result<(), String> {
    let a = parse_csv(&need(args, "--csv")?)?;
    let from = numi(args, "--from", 0)?;
    let to = numi(args, "--to", 0)?;
    let step = numi(args, "--step", 500)?.max(10);
    let blocks: Vec<String> = need(args, "--blocks")?.split(',').map(|s| s.trim().to_string()).collect();
    let width = numf(args, "--width", 32.0)?;
    println!("# rungs on {} — cell = (floor(x/32), floor((y+66)/8), floor(z/32))", need(args, "--csv")?);
    let mut last_line: Option<String> = None;
    let mut t = from;
    while t <= to {
        let Some(s) = a.iter().min_by_key(|s| (s.t - t).abs()) else { break };
        let n = (s.vx * s.vx + s.vz * s.vz).sqrt().max(1e-6);
        let (px, pz) = (-s.vz / n, s.vx / n);
        let mut cells: Vec<(i64, i64, i64)> = Vec::new();
        for (k, _) in blocks.iter().enumerate() {
            let off = (k as f64 - (blocks.len() as f64 - 1.0) / 2.0) * width;
            let (x, y, z) = (s.x + px * off, s.y, s.z + pz * off);
            let c = ((x / 32.0).floor() as i64, ((y + 66.0) / 8.0).floor() as i64, (z / 32.0).floor() as i64);
            if !cells.contains(&c) {
                cells.push(c);
            }
        }
        let line: Vec<String> = blocks
            .iter()
            .zip(cells.iter().chain(std::iter::repeat(cells.last().unwrap())))
            .map(|(b, c)| format!("{}:{},{},{}", b, c.0, c.1, c.2))
            .collect();
        let joined = line.join(" ");
        // `tmmaps ladder` requires N rungs to produce N distinct maps, and two
        // neighbouring samples often land in the same cells. Dropping the
        // repeat here is the difference between a ladder and an abort.
        if last_line.as_deref() != Some(joined.as_str()) {
            println!("{}   # t={} pos {:.1},{:.1},{:.1}", joined, s.t, s.x, s.y, s.z);
            last_line = Some(joined);
        }
        t += step;
    }
    Ok(())
}

/// Walk a rung ladder: search against rung k, take the best tape it banks,
/// seed rung k+1 with it, and keep going. The crawl the fleet has run by hand
/// on three maps, as one command that keeps its own log.
///
/// Each stage's objective is a TIME (the rung is the map's finish), which is
/// what keeps the car fast — a progress-only objective is maximised by a
/// candidate that survives slowly, and on this map that is what ate the lead.
fn cmd_crawl(args: &[String]) -> Result<(), String> {
    let seed = need(args, "--seed")?;
    let maps: Vec<String> = need(args, "--maps")?.split(',').map(|s| s.trim().to_string()).collect();
    let lo = need(args, "--lo")?;
    let hi = need(args, "--hi")?;
    let workers = arg(args, "--workers").unwrap_or_else(|| "88".into());
    let mins = arg(args, "--minutes-per").unwrap_or_else(|| "12".into());
    let root = arg(args, "--root").unwrap_or_else(|| "/dev/shm/crawl".into());
    let outdir = need(args, "--outdir")?;
    let bin = arg(args, "--tmsearch").unwrap_or_else(|| "tmsearch".into());
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {}", outdir, e))?;
    let mut cur = seed.clone();
    for (i, m) in maps.iter().enumerate() {
        let bd = format!("{}/stage{:02}", outdir, i);
        let _ = std::fs::remove_dir_all(&bd);
        let st = std::process::Command::new(&bin)
            .args([
                "search", "--template", &cur, "--map", m, "--lo", &lo, "--hi", &hi, "--workers",
                &workers, "--minutes", &mins, "--bestdir", &bd, "--log",
                &format!("{}/stage{:02}.jsonl", outdir, i), "--root",
                &format!("{}-{}", root, i),
            ])
            .output()
            .map_err(|e| format!("{}: {}", bin, e))?;
        let tail: Vec<&str> = String::from_utf8_lossy(&st.stdout).lines().rev().take(2).map(|s| s.to_string()).collect::<Vec<_>>().iter().map(|s| Box::leak(s.clone().into_boxed_str()) as &str).collect();
        // Best = the lowest time banked this stage. A stage that banks nothing
        // keeps the incumbent and the crawl stops: a rung nothing reaches is
        // the answer, not a reason to skip ahead.
        let mut best: Option<(String, String)> = None;
        if let Ok(rd) = std::fs::read_dir(&bd) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if let Some(t) = n.strip_prefix("best_").and_then(|s| s.strip_suffix(".Ghost.Gbx")) {
                    if t.chars().next().map_or(false, |c| c.is_ascii_digit())
                        && best.as_ref().map_or(true, |(bt, _)| t < bt.as_str())
                    {
                        best = Some((t.to_string(), e.path().to_string_lossy().to_string()));
                    }
                }
            }
        }
        match best {
            Some((t, p)) => {
                println!("stage {:02} {}: best {}  ({})   [{}]", i, m, t.replace('_', "."), p, tail.join(" | "));
                cur = p;
            }
            None => {
                println!("stage {:02} {}: NOTHING REACHED IT — crawl stops here", i, m);
                return Ok(());
            }
        }
    }
    println!("crawl finished; last tape {}", cur);
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let r = match args.get(1).map(|s| s.as_str()) {
        Some("align") => cmd_align(&args),
        Some("splice") => cmd_splice(&args),
        Some("sweep") => cmd_sweep(&args),
        Some("survive") => cmd_survive(&args),
        Some("traces") => cmd_traces(&args),
        Some("maprec") => cmd_maprec(&args),
        Some("cps") => cmd_cps(&args),
        Some("speed") => cmd_speed(&args),
        Some("rungcells") => cmd_rungcells(&args),
        Some("crawl") => cmd_crawl(&args),
        Some("inputs") => cmd_inputs(&args),
        _ => {
            eprintln!(
                "weld -- state-space alignment of two runs, and the tape splice it implies\n\
                 \n\
                 weld align  --a A.csv --b B.csv [--lo MS --hi MS] [--every MS] [--maxlead MS]\n\
                 \t\t[--velweight W] [--angweight W]\n\
                 \t\tFor every sample of B, the closest state on A's line and the LEAD it\n\
                 \t\tis worth. A weld candidate is a positive lead at a small distance.\n\
                 weld splice --prefix P.gtape --tail T.gtape --at TICK --shift K --out O.gtape\n\
                 \t\tout[i] = prefix[i] below TICK, tail[i+K] above it. Runs an identity\n\
                 \t\tcontrol (prefix onto itself, shift 0) before it writes.\n\
                 weld sweep  --prefix P.gtape --tail T.gtape --carrier C.Ghost.Gbx --map M --outdir D\n\t\t--at-lo T --at-hi T [--at-step N] --shifts K,K,K [--jobs N]\n\t\tEvery weld in a grid, through the plain oracle, with the tail-only\n\t\tcontrol in the same batch.\n weld inputs --a A.gtape [--b B.gtape] --lo T --hi T\n\
                 \t\tsteer/gas/brake per tick, two tapes side by side."
            );
            std::process::exit(2);
        }
    };
    if let Err(e) = r {
        eprintln!("weld: ABORT: {}", e);
        std::process::exit(2);
    }
}
