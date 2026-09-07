use std::path::PathBuf;
use tmreach::gates::MapGates;
use tmreach::rig::Worker;
use tmreach::starts::{run_on_worker, starts_tsv_header, starts_tsv_row, StartsOpts};
use tmreach::tele::Telemetry;

fn usage() -> ! {
    eprintln!(
        "tmreach -- the reachability generator (route project, GEN arm)

  tmreach starts --map M --ghost G [--every MS] [--out starts.tsv] [--trace F.tsv] [--work DIR] [-v]
        savestates along a human run + the START-POSITION and IDENTITY controls (fail closed)
  tmreach gatecal --map M --ghosts DIR [--workers W] [--out DIR]
        every ghost: controls, flat engine trajectory, where the car is at each credited
        checkpoint; grades candidate trigger volumes against the credited ticks

Engine: --server DIR [$TM_SERVER]  --shim FILE [$FK_SHIM]
Times print as seconds with a decimal."
    );
    std::process::exit(2)
}

struct Args {
    flags: std::collections::HashMap<String, String>,
    bools: std::collections::HashSet<String>,
}

impl Args {
    fn parse(a: &[String]) -> Args {
        let mut flags = std::collections::HashMap::new();
        let mut bools = std::collections::HashSet::new();
        let mut i = 0;
        while i < a.len() {
            if let Some(k) = a[i].strip_prefix("--") {
                if i + 1 < a.len() && !a[i + 1].starts_with("--") {
                    flags.insert(k.to_string(), a[i + 1].clone());
                    i += 2;
                    continue;
                }
                bools.insert(k.to_string());
            } else if a[i] == "-v" {
                bools.insert("verbose".into());
            }
            i += 1;
        }
        Args { flags, bools }
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.flags.get(k).map(|s| s.as_str())
    }
    fn req(&self, k: &str) -> String {
        match self.get(k) {
            Some(v) => v.to_string(),
            None => {
                eprintln!("tmreach: --{k} is required");
                std::process::exit(2)
            }
        }
    }
    fn has(&self, k: &str) -> bool {
        self.bools.contains(k)
    }
}

fn engine_paths(a: &Args) -> (PathBuf, PathBuf) {
    let server = a
        .get("server")
        .map(PathBuf::from)
        .or_else(|| std::env::var("TM_SERVER").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/tmp/tmp/server"));
    let shim = a
        .get("shim")
        .map(PathBuf::from)
        .or_else(|| std::env::var("FK_SHIM").ok().map(PathBuf::from))
        .or_else(fk::session::default_shim)
        .unwrap_or_else(|| PathBuf::from("libforkshim.so"));
    (server, shim)
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() {
        usage();
    }
    let a = Args::parse(&argv[1..]);
    let r = match argv[0].as_str() {
        "starts" => cmd_starts(&a),
        "gatecal" => cmd_gatecal(&a),
        _ => usage(),
    };
    if let Err(e) = r {
        eprintln!("tmreach: ABORT: {e}");
        std::process::exit(1);
    }
}

fn cmd_starts(a: &Args) -> Result<(), String> {
    let map = PathBuf::from(a.req("map"));
    let ghost = PathBuf::from(a.req("ghost"));
    let (server, shim) = engine_paths(a);
    let work = a
        .get("work")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/tmp/tmreach/starts-{}", std::process::id())));
    let o = StartsOpts {
        every_ms: a.get("every").map(|s| s.parse().unwrap()).unwrap_or(500),
        out: a.get("out").map(PathBuf::from),
        trace_out: a.get("trace").map(PathBuf::from),
        verbose: a.has("verbose"),
    };
    let gates = MapGates::load(&map)?;
    println!("map {}: {} gates + spawn {:?}", gates.map_uid, gates.gates.len(), gates.spawn.as_ref().map(|s| s.centre));
    for g in &gates.gates {
        println!("  wp{} {:?} {} item={} cell {:?} centre ({:.1}, {:.1}, {:.1}) dir {:?} yaw {:?}", g.waypoint, g.kind, g.model, g.from_item, g.cell, g.centre[0], g.centre[1], g.centre[2], g.dir, g.yaw);
    }
    let tel = Telemetry::load(&ghost.to_string_lossy())?;
    println!(
        "ghost {} md5 {} : {} samples, checkpoints {}",
        ghost.display(),
        tel.md5,
        tel.dec.samples.len(),
        tel.checkpoints_ms.iter().map(|c| tmreach::secs(*c as i64)).collect::<Vec<_>>().join(" ")
    );
    let mut w = match a.get("clock") {
        Some(c) => Worker::start_at(&server, &map, &shim, &work, &ghost, o.verbose, c.parse().unwrap())?,
        None => Worker::start(&server, &map, &shim, &work, &ghost, o.verbose)?,
    };
    println!(
        "worker up in {:.1} s: tape {} ticks, start_offset {} ms, root probe tick {}, root row race {} ({:.3}, {:.3}, {:.3}) {:.2} m/s",
        w.startup_s, w.n_ticks(), w.tape.start_offset_ms, w.root_probe, tmreach::secs(w.root_row.time_ms), w.root_row.x, w.root_row.y, w.root_row.z, tmreach::rig::speed(&w.root_row)
    );
    let rep = run_on_worker(&mut w, &tel, &gates, &o)?;
    if !(rep.start_ctrl_pass && rep.identity.passes()) {
        return Err("a startup control FAILED; no starts written".into());
    }
    if let Some(p) = &o.out {
        let mut s = String::from(starts_tsv_header());
        for (i, st) in rep.starts.iter().enumerate() {
            s.push_str(&starts_tsv_row(i as u32, &rep.ghost_md5, st, "human"));
        }
        std::fs::write(p, s).map_err(|e| e.to_string())?;
        println!("wrote {} ({} starts)", p.display(), rep.starts.len());
    }
    for st in rep.starts.iter().take(5) {
        println!("  start tick {} race {} ({:.2}, {:.2}, {:.2}) {:.1} m/s cps {}", st.tick, tmreach::secs(st.row.time_ms), st.row.x, st.row.y, st.row.z, tmreach::rig::speed(&st.row), st.cps_before);
    }
    Ok(())
}

fn pool_cfg(a: &Args, map: &PathBuf, tag: &str) -> tmreach::pool::PoolCfg {
    let (server, shim) = engine_paths(a);
    tmreach::pool::PoolCfg {
        server,
        map: map.clone(),
        shim,
        work_root: a
            .get("work")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("/tmp/tmreach/{}-{}", tag, std::process::id()))),
        workers: a.get("workers").map(|s| s.parse().unwrap()).unwrap_or(16),
        verbose: a.has("verbose"),
    }
}

fn cmd_gatecal(a: &Args) -> Result<(), String> {
    use tmreach::gatecal::*;
    use tmreach::gates::Volume;
    let map = PathBuf::from(a.req("map"));
    let ghosts = tmreach::pool::ghosts_in(&PathBuf::from(a.req("ghosts")))?;
    let out = PathBuf::from(a.get("out").unwrap_or("/tmp/tmreach/gatecal"));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let gates = std::sync::Arc::new(MapGates::load(&map)?);
    let cfg = pool_cfg(a, &map, "gatecal");
    println!("gatecal: {} ghosts, {} workers, map {}", ghosts.len(), tmreach::pool::cap(cfg.workers), gates.map_uid);
    let g2 = gates.clone();
    let t0 = std::time::Instant::now();
    let results = tmreach::pool::run_per_ghost(&cfg, &ghosts, move |_, w, tel| ghost_run(w, tel, &g2));
    let mut runs = Vec::new();
    let mut failed = 0;
    for (g, r) in ghosts.iter().zip(results) {
        match r {
            Ok(run) => {
                println!(
                    "  {}: start d {:.2} m, identity RMS {:.4} max {:.3} m, {} crossings, startup {:.1} s, flat {:.2} s",
                    run.ghost, run.start_d, run.identity_rms, run.identity_max, run.crossings.len(), run.startup_s, run.flat_s
                );
                runs.push(run);
            }
            Err(e) => {
                failed += 1;
                println!("  {}: FAILED: {}", g.display(), e);
            }
        }
    }
    println!("{} ghosts ok, {} failed, {:.1} s wall", runs.len(), failed, t0.elapsed().as_secs_f64());
    // the crossings table
    let mut s = String::from(crossings_tsv_header());
    for run in &runs {
        for c in &run.crossings {
            s.push_str(&crossing_tsv_row(c, &gates));
        }
    }
    std::fs::write(out.join("crossings.tsv"), &s).map_err(|e| e.to_string())?;
    // per-ghost flat traces (for the record and for the model arm)
    for run in &runs {
        tmreach::starts::write_trace(&out.join(format!("flat-{}.tsv", run.ghost.trim_end_matches(".Ghost.Gbx"))), &run.flat)?;
    }
    // grade candidate volumes
    println!("\nGRADING candidate trigger volumes (first entry vs credited tick, ±2 ticks, no missed/extra):");
    let mut best: Option<(String, Grade)> = None;
    let mut report = String::from("volume\tcrossings\twithin2\tmissed\textra\tmax_abs_dt\thist\n");
    for hy in [4.0, 8.0, 16.0, 64.0] {
        for h in [4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0] {
            for (name, v) in [
                (format!("box h{h} hy{hy}"), Volume::Box { hx: h, hy, hz: h }),
                (format!("cyl r{h} hy{hy}"), Volume::Cylinder { r: h, hy }),
            ] {
                let gr = grade(&runs, &gates, &|_| v);
                report.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{:?}\n", name, gr.crossings, gr.within2, gr.missed, gr.extra, gr.max_abs_dt, gr.dt_hist));
                let better = match &best {
                    None => true,
                    Some((_, b)) => (gr.within2, -(gr.missed as i64 + gr.extra as i64)) > (b.within2, -(b.missed as i64 + b.extra as i64)),
                };
                if better {
                    best = Some((name, gr));
                }
            }
        }
    }
    std::fs::write(out.join("grades.tsv"), &report).map_err(|e| e.to_string())?;
    if let Some((n, g)) = best {
        println!("best uniform volume: {} -> {}  => {}", n, g, if g.passes() { "PASS" } else { "FAIL" });
    }
    println!("wrote {}/crossings.tsv, grades.tsv, flat-*.tsv", out.display());
    Ok(())
}
