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
                // --key=value
                if let Some((kk, v)) = k.split_once('=') {
                    flags.insert(kk.to_string(), v.to_string());
                    i += 1;
                    continue;
                }
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
        "fanout" => cmd_fanout(&a),
        "verify" => cmd_verify(&a),
        "oraclectl" => cmd_oraclectl(&a),
        "gateprobe" => cmd_gateprobe(&a),
        "fitbox" => cmd_fitbox(&a),
        "rejudge" => cmd_rejudge(&a),
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
    let geom = a.get("geom").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{}/persistent/private-30d/tm-route/geom", std::env::var("HOME").unwrap_or_default())));
    let gates = MapGates::load(&map, Some(&geom))?;
    println!("gates from {}", gates.source);
    println!("map {}: {} gates + spawn {:?}", gates.map_uid, gates.gates.len(), gates.spawn.as_ref().map(|s| s.centre));
    for g in &gates.gates {
        println!("  wp{} {:?} {} item={} centre ({:.1}, {:.1}, {:.1}) normal ({:.2}, {:.2}, {:.2}) hw {:.1} hh {:.1} group {}", g.waypoint, g.kind, g.model, g.from_item, g.centre[0], g.centre[1], g.centre[2], g.normal[0], g.normal[1], g.normal[2], g.half_width, g.half_height, g.group);
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
        "worker up in {:.1} s: tape {} ticks, start_offset {} ms, root probe tick {}, root row label {} (tick hook: race {}) at ({:.3}, {:.3}, {:.3}) {:.2} m/s",
        w.startup_s, w.n_ticks(), w.tape.start_offset_ms, w.root_probe, tmreach::secs(w.root_row.time_ms),
        w.root_race_ms_hook.map(tmreach::secs).unwrap_or("n/a (lroundf clock)".into()),
        w.root_row.x, w.root_row.y, w.root_row.z, tmreach::rig::speed(&w.root_row)
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
    let map = PathBuf::from(a.req("map"));
    let ghosts = tmreach::pool::ghosts_in(&PathBuf::from(a.req("ghosts")))?;
    let out = PathBuf::from(a.get("out").unwrap_or("/tmp/tmreach/gatecal"));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let geom = a.get("geom").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{}/persistent/private-30d/tm-route/geom", std::env::var("HOME").unwrap_or_default())));
    let gates = std::sync::Arc::new(MapGates::load(&map, Some(&geom))?);
    println!("gates from {}", gates.source);
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
    println!("\nWHERE IS THE CAR AT THE NOTICE (engine label notice-10 = race time of the notice; s along the GEOM normal from the gate centre, m):");
    for (k, n, mean, sd, mn, mx) in model_stats(&runs, &gates) {
        println!("  {k}: n {n}  s mean {mean:+.3} sd {sd:.3} min {mn:+.3} max {mx:+.3}");
    }
    let prov = format!("tmreach gatecal {} {} ghosts on {} ({}), gates {}", tmreach::GIT_HASH, runs.len(), gates.map_uid, hostname(), gates.source);
    let (det, notes) = fit(&runs, &gates, &prov);
    println!("\nFITTED DETECTOR (plane at per-model s_off from the crediting geometry, credited tick = T-1; lat 10 m road / GEOM item, up -6..+8: see gatecal::fit):");
    for n in &notes {
        println!("  {n}");
    }
    for (m, t) in &det.per_model {
        println!("  {m}: s_off {:+.3} m, depth {:.1}, lat_half {:.1}, up {:+.1}..{:+.1}", t.s_off, t.depth, t.lat_half, t.up_lo, t.up_hi);
    }
    let gr = grade(&runs, &gates, &det);
    println!("GRADE vs the ghosts' own notices (first row inside vs the row before the notice, T-1): {} => {}", gr, if gr.passes() { "PASS (bar: ±2 ticks on ≥95 %, no missed, no extra)" } else { "FAIL" });
    std::fs::write(out.join("detector.json"), det.to_json()).map_err(|e| e.to_string())?;
    std::fs::write(out.join("grade.txt"), format!("{}\n{}\n", gr, if gr.passes() { "PASS" } else { "FAIL" })).map_err(|e| e.to_string())?;
    // the human crossing ORDER per ghost, for the coordinator's 44/44 check
    let mut orders: std::collections::BTreeMap<String, usize> = Default::default();
    for run in &runs {
        let mut cs: Vec<&tmreach::gatecal::Crossing> = run.crossings.iter().collect();
        cs.sort_by_key(|c| c.cp_ms);
        let o = cs.iter().map(|c| c.gate_wp.to_string()).collect::<Vec<_>>().join(",");
        *orders.entry(o).or_default() += 1;
    }
    println!("human gate ORDER by notice time (detector's nearest-gate assignment): {:?}", orders);
    println!("wrote {}/crossings.tsv, detector.json, grade.txt, flat-*.tsv", out.display());
    Ok(())
}

fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname").map(|s| s.trim().to_string()).unwrap_or_default()
}

fn cmd_fanout(a: &Args) -> Result<(), String> {
    use tmreach::fanout::*;
    let map = PathBuf::from(a.req("map"));
    let ghosts = tmreach::pool::ghosts_in(&PathBuf::from(a.req("ghosts")))?;
    let out = PathBuf::from(a.req("out"));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let geom = a.get("geom").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{}/persistent/private-30d/tm-route/geom", std::env::var("HOME").unwrap_or_default())));
    let gates = std::sync::Arc::new(MapGates::load(&map, Some(&geom))?);
    let det_path = PathBuf::from(a.req("detector"));
    let det = std::sync::Arc::new(tmreach::gates::Detector::from_json(&std::fs::read_to_string(&det_path).map_err(|e| e.to_string())?)?);
    let lib = std::sync::Arc::new(tmreach::macros::library_v0());
    let horizons: Vec<u16> = a.get("horizons").unwrap_or("200,400").split(',').map(|s| s.parse().unwrap()).collect();
    let limit = a.get("limit").map(|s| s.parse::<usize>().unwrap()).unwrap_or(usize::MAX);
    let ghosts: Vec<PathBuf> = ghosts.into_iter().take(limit).collect();
    let floor_y = gates.gates.iter().map(|g| g.centre[1]).fold(f64::INFINITY, f64::min) - 40.0;
    let cfg = std::sync::Arc::new(FanoutCfg {
        every_ms: a.get("every").map(|s| s.parse().unwrap()).unwrap_or(500),
        horizons: horizons.clone(),
        lib: lib.clone(),
        det: det.clone(),
        gates: gates.clone(),
        floor_y,
        keep_rows: false,
    });
    let pcfg = pool_cfg(a, &map, "fanout");
    println!(
        "fanout: {} ghosts x {} shards, {} workers, {} macros x horizons {:?}, every {}, gates {} ({}), detector {}",
        ghosts.len(),
        a.get("shards").unwrap_or("1"),
        tmreach::pool::cap(pcfg.workers),
        lib.len(),
        horizons,
        tmreach::secs(cfg.every_ms),
        gates.map_uid,
        gates.source,
        det_path.display()
    );
    std::fs::write(out.join("macros.tsv"), tmreach::macros::macros_tsv(&lib)).map_err(|e| e.to_string())?;
    std::fs::copy(&det_path, out.join("detector.json")).map_err(|e| e.to_string())?;
    let t0 = std::time::Instant::now();
    // start ids: 1000 per ghost slot
    let c2 = cfg.clone();
    // work items: every ghost `shards` times, item i = (ghost i / shards, shard i % shards)
    let shards = a.get("shards").map(|s| s.parse::<usize>().unwrap()).unwrap_or(1).max(1);
    let items: Vec<PathBuf> = ghosts.iter().flat_map(|g| std::iter::repeat(g.clone()).take(shards)).collect();
    let results = tmreach::pool::run_per_ghost(&pcfg, &items, move |i, w, tel| fanout_ghost(w, tel, &c2, (i / shards) as u32 * 1000, (i % shards, shards)));
    let wall = t0.elapsed().as_secs_f64();
    let mut writer = tmreach::tmr::Writer::create(&out.join("samples.tmr"), gates.gates.len() as u8)?;
    let mut starts = String::from(tmreach::starts::starts_tsv_header());
    let mut endpoints = String::from("start_id\tmacro_id\thorizon\tx\ty\tz\n");
    let mut others = String::from("ghost\tstart_id\tmacro_id\thorizon\tgate\tmacro\tnote\n");
    let mut log = String::new();
    let mut tot = Stats::default();
    let mut ok = 0;
    let (mut human_legs, mut human_resp) = (0usize, 0usize);
    for (g, r) in items.iter().zip(results) {
        match r {
            Ok(fo) => {
                ok += 1;
                for s in &fo.starts {
                    let st = &s.state;
                    starts.push_str(&format!(
                        "{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{}\t{}\n",
                        s.start_id, s.ghost_md5, s.tick, st.race_ms, st.pos[0], st.pos[1], st.pos[2], st.vel[0], st.vel[1], st.vel[2], st.quat[0], st.quat[1], st.quat[2], st.quat[3], s.cps_before,
                        if fo.records.iter().any(|r| r.start_id == s.start_id && r.macro_id >= tmreach::human::RESPAWN_MACRO) { "human-leg" } else { "human" }
                    ));
                }
                for r in &fo.records {
                    writer.push(r)?;
                }
                for (sid, m, h, p) in &fo.endpoints {
                    endpoints.push_str(&format!("{sid}\t{m}\t{h}\t{:.2}\t{:.2}\t{:.2}\n", p[0], p[1], p[2]));
                }
                for o in &fo.other_connections {
                    others.push_str(o);
                    others.push('\n');
                }
                for l in &fo.log {
                    log.push_str(l);
                    log.push('\n');
                    println!("{l}");
                }
                tot.rollouts += fo.stats.rollouts;
                tot.noop += fo.stats.noop;
                tot.out_of_tape += fo.stats.out_of_tape;
                tot.errors += fo.stats.errors;
                tot.identity_fail += fo.stats.identity_fail;
                tot.identity_max_m = tot.identity_max_m.max(fo.stats.identity_max_m);
                tot.start_blend_max_m = tot.start_blend_max_m.max(fo.stats.start_blend_max_m);
                for i in 0..5 {
                    tot.outcomes[i] += fo.stats.outcomes[i];
                }
                tot.reached_next += fo.stats.reached_next;
                tot.reached_other += fo.stats.reached_other;
                tot.distinct_cells.extend(fo.stats.distinct_cells.iter());
                tot.switches += fo.stats.switches;
                human_legs += fo.human_legs;
                human_resp += fo.human_respawns;
                tot.rollout_secs += fo.stats.rollout_secs;
            }
            Err(e) => {
                let l = format!("{}: FAILED: {}", g.display(), e);
                println!("{l}");
                log.push_str(&l);
                log.push('\n');
            }
        }
    }
    let count = writer.close()?;
    std::fs::write(out.join("starts.tsv"), starts).map_err(|e| e.to_string())?;
    std::fs::write(out.join("endpoints.tsv"), endpoints).map_err(|e| e.to_string())?;
    std::fs::write(out.join("other-connections.tsv"), others).map_err(|e| e.to_string())?;
    let mut cells = tot.distinct_cells.clone();
    cells.sort();
    let med = cells.get(cells.len() / 2).copied().unwrap_or(0);
    let summary = format!(
        "fanout {} on {} ({}): {}/{} work items (ghost x shard) ok, {} rollouts ({} records) in {:.1} s wall = {:.1} rollouts/s/box with {} workers; per-rollout engine time {:.1} ms mean\n\
         outcomes ok {} crash-stop {} offworld {} finished {} aborted {}; reached the human's next gate {} ({:.1} %), some OTHER gate first {} ({:.2} %); no-op macros {}, out-of-tape {}, errors {}\n\
         identity (macro 0 end state vs the human's trajectory): max {:.4} m, {} fails over {} starts; start-row blend by a macro's first record: max {:.4} m\n\
         distinct end cells (2 m x 2 m x 5 m/s) per start over {} macros x {} horizons: median {}, min {}, max {}; {} starts were switches (<= 2 cells)\n\
         human legs (positives) {}, respawn negatives {}\n",
        tmreach::GIT_HASH, gates.map_uid, hostname(), ok, items.len(), tot.rollouts, count, wall, tot.rollouts as f64 / wall, tmreach::pool::cap(pcfg.workers),
        1000.0 * tot.rollout_secs / tot.rollouts.max(1) as f64,
        tot.outcomes[0], tot.outcomes[1], tot.outcomes[2], tot.outcomes[3], tot.outcomes[4],
        tot.reached_next, 100.0 * tot.reached_next as f64 / tot.rollouts.max(1) as f64,
        tot.reached_other, 100.0 * tot.reached_other as f64 / tot.rollouts.max(1) as f64,
        tot.noop, tot.out_of_tape, tot.errors, tot.identity_max_m, tot.identity_fail, cells.len(), tot.start_blend_max_m,
        lib.len(), horizons.len(), med, cells.first().copied().unwrap_or(0), cells.last().copied().unwrap_or(0), tot.switches, human_legs, human_resp
    );
    print!("{summary}");
    log.push_str(&summary);
    std::fs::write(out.join("FANOUT.log"), log).map_err(|e| e.to_string())?;
    println!("wrote {}/{{samples.tmr, starts.tsv, macros.tsv, endpoints.tsv, other-connections.tsv, detector.json, FANOUT.log}}", out.display());
    Ok(())
}

fn cmd_verify(a: &Args) -> Result<(), String> {
    let dir = PathBuf::from(a.req("dir"));
    let p = dir.join("samples.tmr");
    let bytes = std::fs::read(&p).map_err(|e| format!("{}: {}", p.display(), e))?;
    let md5 = tmreach::tele::md5_hex(&bytes);
    let shard = tmreach::tmr::read_shard(&p)?;
    let mut hist = [0usize; 5];
    let mut bad_ticks = 0;
    let mut gates_hit = 0;
    let mut by_start: std::collections::BTreeMap<u32, usize> = Default::default();
    for r in &shard.records {
        hist[(r.outcome as usize).min(4)] += 1;
        *by_start.entry(r.start_id).or_default() += 1;
        for g in &r.gate_tick {
            if *g >= 0 {
                gates_hit += 1;
                if *g as u16 >= r.horizon_ticks {
                    bad_ticks += 1;
                }
            }
        }
    }
    let starts = std::fs::read_to_string(dir.join("starts.tsv")).map(|s| s.lines().count().saturating_sub(1)).unwrap_or(0);
    println!(
        "{}: TMR0 v{} md5 {} : {} records, {} gates, {} starts in starts.tsv, {} distinct start_ids in the shard; outcomes ok {} crash-stop {} offworld {} finished {} aborted {}; {} gate crossings, {} with a tick outside the horizon",
        p.display(), shard.version, md5, shard.records.len(), shard.n_gates, starts, by_start.len(), hist[0], hist[1], hist[2], hist[3], hist[4], gates_hit, bad_ticks
    );
    if bad_ticks > 0 || by_start.len() != starts {
        return Err("verify FAILED".into());
    }
    println!("verify OK");
    Ok(())
}

fn cmd_oraclectl(a: &Args) -> Result<(), String> {
    use tmreach::oraclectl::*;
    let map = PathBuf::from(a.req("map"));
    let ghosts_all = tmreach::pool::ghosts_in(&PathBuf::from(a.req("ghosts")))?;
    let out = PathBuf::from(a.req("out"));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let geom = a.get("geom").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{}/persistent/private-30d/tm-route/geom", std::env::var("HOME").unwrap_or_default())));
    let gates = std::sync::Arc::new(MapGates::load(&map, Some(&geom))?);
    let det_path = PathBuf::from(a.req("detector"));
    let det = std::sync::Arc::new(tmreach::gates::Detector::from_json(&std::fs::read_to_string(&det_path).map_err(|e| e.to_string())?)?);
    let lib = std::sync::Arc::new(tmreach::macros::library_v0());
    // every k-th ghost, a stratified macro subset: hold gas straight / hard left / hard right / brake, base-steer, ramp, doublet, reference
    let stride = a.get("ghost-stride").map(|s| s.parse::<usize>().unwrap()).unwrap_or(4);
    let goff = a.get("ghost-offset").map(|s| s.parse::<usize>().unwrap()).unwrap_or(0);
    let ghosts: Vec<PathBuf> = ghosts_all.iter().enumerate().filter(|(i, _)| i % stride == goff).map(|(_, p)| p.clone()).collect();
    let macro_ids: Vec<u16> = a
        .get("macros")
        .map(|s| s.split(',').map(|x| x.parse().unwrap()).collect())
        .unwrap_or_else(|| vec![0, 1, 3, 13, 15, 25, 27, 29, 32, 36, 38, 42]);
    let cfg = std::sync::Arc::new(CtlCfg {
        lib: lib.clone(),
        det: det.clone(),
        gates: gates.clone(),
        out: out.clone(),
        every_ms: a.get("every").map(|s| s.parse().unwrap()).unwrap_or(2000),
        horizon: a.get("horizon").map(|s| s.parse().unwrap()).unwrap_or(300),
        macro_ids: macro_ids.clone(),
        save_rows: a.has("save-rows"),
    });
    let pcfg = pool_cfg(a, &map, "oraclectl");
    println!("oraclectl: {} ghosts (stride {}), macros {:?}, every {}, horizon {} ticks, detector {}", ghosts.len(), stride, macro_ids, tmreach::secs(cfg.every_ms), cfg.horizon, det_path.display());
    let c2 = cfg.clone();
    let t0 = std::time::Instant::now();
    let results = tmreach::pool::run_per_ghost(&pcfg, &ghosts, move |gi, w, tel| cases_for_ghost(w, tel, &c2, gi));
    let mut cases: Vec<Case> = Vec::new();
    for (g, r) in ghosts.iter().zip(results) {
        match r {
            Ok(v) => cases.extend(v),
            Err(e) => println!("  {}: FAILED: {}", g.display(), e),
        }
    }
    println!("{} cases built in {:.1} s; adjudicating with the plain oracle ...", cases.len(), t0.elapsed().as_secs_f64());
    let (server, _) = engine_paths(a);
    let t1 = std::time::Instant::now();
    adjudicate(&server, &map, &mut cases)?;
    println!("oracle done in {:.1} s", t1.elapsed().as_secs_f64());
    let mut s = String::from(case_tsv_header());
    // THE PLAIN ORACLE CANNOT SEE ONE CHECKPOINT: a DNF with exactly one
    // credited checkpoint reports "wrong simu" like a DNF with none ("reached
    // SOME checkpoints (k out of N)" appears only for k >= 2; measured on
    // p00001 with brake-from-tick tapes: 700..1400 -> plain, 1450 -> 2 of 4).
    // So (det 1, oracle 0) is the oracle's blind class, counted apart.
    let (mut agree, mut disagree, mut unanswered, mut blind, mut near, mut fin, mut fin_dt) = (0, 0, 0, 0, 0, 0, Vec::new());
    for c in &cases {
        s.push_str(&case_tsv_row(c));
        match c.oracle_cps {
            None => unanswered += 1,
            Some(x) if x == c.det_cps => agree += 1,
            Some(0) if c.det_cps == 1 && !c.det_finished => blind += 1,
            Some(_) => disagree += 1,
        }
        if c.near_miss_m < 40.0 {
            near += 1;
        }
        if c.det_finished {
            fin += 1;
            if let (Some(d), Some(o)) = (c.det_finish_ms, c.oracle_ms) {
                fin_dt.push(o - d);
            }
        }
    }
    std::fs::write(out.join("cases.tsv"), &s).map_err(|e| e.to_string())?;
    let mut hist: std::collections::BTreeMap<(u32, Option<u32>), usize> = Default::default();
    for c in &cases {
        *hist.entry((c.det_cps, c.oracle_cps)).or_default() += 1;
    }
    let verdict = format!(
        "ORACLE CONTROL: {} cases; detector == oracle on {} ({:.1} %), disagree {}, unanswered {}, in the oracle's blind class (det 1 / oracle reports none) {}; {} near-misses (< 40 m of an uncredited gate), {} finishes (oracle time − detector finish-row time, ms: {:?})\n(det_cps, oracle_cps) histogram: {:?}\n=> {}",
        cases.len(),
        agree,
        100.0 * agree as f64 / cases.len().max(1) as f64,
        disagree,
        unanswered,
        blind,
        near,
        fin,
        fin_dt,
        hist,
        if disagree == 0 && unanswered == 0 && cases.len() >= 200 { "PASS (bar N/N, N >= 200)" } else { "FAIL" }
    );
    println!("{verdict}");
    std::fs::write(out.join("VERDICT.txt"), format!("{verdict}\n")).map_err(|e| e.to_string())?;
    Ok(())
}

fn cmd_gateprobe(a: &Args) -> Result<(), String> {
    let map = PathBuf::from(a.req("map"));
    let ghost = PathBuf::from(a.req("ghost"));
    let out = PathBuf::from(a.req("out"));
    let gate_wp: u32 = a.req("gate").parse().map_err(|_| "--gate N")?;
    let targets: Vec<f64> = a.get("stops").unwrap_or("-9,-7,-5,-3,-1,1").split(',').map(|s| s.parse().unwrap()).collect();
    let geom = a.get("geom").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{}/persistent/private-30d/tm-route/geom", std::env::var("HOME").unwrap_or_default())));
    let gates = MapGates::load(&map, Some(&geom))?;
    let det = tmreach::gates::Detector::from_json(&std::fs::read_to_string(a.req("detector")).map_err(|e| e.to_string())?)?;
    let (server, shim) = engine_paths(a);
    let work = a.get("work").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("/tmp/tmreach/probe-{}", std::process::id())));
    let tel = Telemetry::load(&ghost.to_string_lossy())?;
    let mut w = Worker::start(&server, &map, &shim, &work, &ghost, a.has("verbose"))?;
    let mut probes = tmreach::gateprobe::probe_gate(&mut w, &tel, &gates, &det, gate_wp, &targets, &out)?;
    drop(w);
    tmreach::gateprobe::adjudicate(&server, &map, &mut probes)?;
    let g = gates.gate(gate_wp).unwrap();
    println!("GATE PROBE wp{gate_wp} {} ({}): car braked to rest at s along the normal from the GEOM centre; oracle count vs cps_before", g.model, ghost.file_name().unwrap().to_string_lossy());
    let mut s = String::from("target_s\tbrake_tick\trest_s\trest_lat\trest_up\trest_speed\tcps_before\toracle_cps\tcredited\tdesc\ttape\n");
    for p in &probes {
        let credited = match p.oracle_cps {
            Some(c) if c > p.cps_before => "YES",
            Some(_) => "no",
            None => "?",
        };
        println!("  target {:+.1}: brake tick {} -> rest s {:+.2} lat {:+.2} up {:+.2} v {:.1} m/s; oracle {} vs before {} => credited {} ({})", p.target_s, p.brake_tick, p.rest_s, p.rest_lat, p.rest_up, p.rest_speed, p.oracle_cps.map(|x| x.to_string()).unwrap_or("-".into()), p.cps_before, credited, p.oracle_desc);
        s.push_str(&format!("{:+.1}\t{}\t{:+.3}\t{:+.3}\t{:+.3}\t{:.2}\t{}\t{}\t{}\t{}\t{}\n", p.target_s, p.brake_tick, p.rest_s, p.rest_lat, p.rest_up, p.rest_speed, p.cps_before, p.oracle_cps.map(|x| x.to_string()).unwrap_or("-".into()), credited, p.oracle_desc, p.tape.display()));
    }
    std::fs::write(out.join(format!("probe-wp{gate_wp}.tsv")), s).map_err(|e| e.to_string())?;
    Ok(())
}

fn cmd_fitbox(a: &Args) -> Result<(), String> {
    let map = PathBuf::from(a.req("map"));
    let geom = a.get("geom").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{}/persistent/private-30d/tm-route/geom", std::env::var("HOME").unwrap_or_default())));
    let gates = MapGates::load(&map, Some(&geom))?;
    let dirs: Vec<PathBuf> = a.req("cases").split(',').map(PathBuf::from).collect();
    let crossings = tmreach::fitbox::load_crossings(&PathBuf::from(a.req("crossings")))?;
    // per-ghost start offsets, from the tapes themselves
    let mut offsets: std::collections::HashMap<String, (i64, usize)> = Default::default();
    for g in tmreach::pool::ghosts_in(&PathBuf::from(a.req("ghosts")))? {
        let stem = g.file_name().unwrap().to_string_lossy().trim_end_matches(".Ghost.Gbx").to_string();
        if let Ok(t) = fk::tape::Tape::load(&g.to_string_lossy()) {
            offsets.insert(stem, (t.start_offset_ms as i64, t.declared_ms.unwrap_or(0) as usize));
        }
    }
    for (gi, g) in gates.gates.iter().enumerate() {
        let mut samples = Vec::new();
        for d in &dirs {
            samples.extend(tmreach::fitbox::load_cases(d, &gates, gi, &crossings, &offsets)?);
        }
        let nc = samples.iter().filter(|s| s.credited).count();
        println!("\nwp{} {}: {} usable samples ({} credited, {} refused)", g.waypoint, g.model, samples.len(), nc, samples.len() - nc);
        if samples.is_empty() {
            continue;
        }
        let res = tmreach::fitbox::grid_fit(&samples);
        let best = res[0].1 + res[0].2;
        let consistent: Vec<&(tmreach::fitbox::Box3, usize, usize)> = res.iter().filter(|(_, m, e)| m + e == best).collect();
        let rng = |f: &dyn Fn(&tmreach::fitbox::Box3) -> f64| {
            let v: Vec<f64> = consistent.iter().map(|(b, _, _)| f(b)).collect();
            (v.iter().cloned().fold(f64::INFINITY, f64::min), v.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
        };
        println!("  best inconsistency {} (miss+extra) on {} boxes; over those: s_lo {:?} s_hi {:?} lat_lo {:?} lat_hi {:?}", best, consistent.len(), rng(&|b| b.s_lo), rng(&|b| b.s_hi), rng(&|b| b.lat_lo), rng(&|b| b.lat_hi));
        for (b, m, e) in res.iter().take(3) {
            println!("  {:?} miss {} extra {}", b, m, e);
        }
        if best > 0 {
            for s in &samples {
                let any = s.pts.iter().any(|p| tmreach::fitbox::inside(&res[0].0, *p));
                if s.credited != any {
                    let near = s.pts.iter().map(|p| (p.0 * p.0 + p.1 * p.1).sqrt()).fold(f64::INFINITY, f64::min);
                    println!("    inconsistent: {} credited={} closest {:.1} m", s.name, s.credited, near);
                }
            }
        }
    }
    Ok(())
}

/// Re-grade saved oracle-control cases with another detector (no engine).
fn cmd_rejudge(a: &Args) -> Result<(), String> {
    let map = PathBuf::from(a.req("map"));
    let geom = a.get("geom").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{}/persistent/private-30d/tm-route/geom", std::env::var("HOME").unwrap_or_default())));
    let gates = MapGates::load(&map, Some(&geom))?;
    let det = tmreach::gates::Detector::from_json(&std::fs::read_to_string(a.req("detector")).map_err(|e| e.to_string())?)?;
    let crossings = tmreach::fitbox::load_crossings(&PathBuf::from(a.req("crossings")))?;
    let mut offsets: std::collections::HashMap<String, i64> = Default::default();
    let mut declared: std::collections::HashMap<String, i64> = Default::default();
    for g in tmreach::pool::ghosts_in(&PathBuf::from(a.req("ghosts")))? {
        let stem = g.file_name().unwrap().to_string_lossy().trim_end_matches(".Ghost.Gbx").to_string();
        if let Ok(t) = fk::tape::Tape::load(&g.to_string_lossy()) {
            offsets.insert(stem.clone(), t.start_offset_ms as i64);
            declared.insert(stem.clone(), t.declared_ms.unwrap_or(0) as i64);
        }
    }
    let ng = gates.gates.len();
    let (mut agree, mut disagree, mut blind, mut total, mut fin_agree, mut fin_dis) = (0, 0, 0, 0, 0, 0);
    let mut hist: std::collections::BTreeMap<(u32, Option<u32>), usize> = Default::default();
    let mut bad = Vec::new();
    for d in a.req("cases").split(',') {
        let txt = std::fs::read_to_string(PathBuf::from(d).join("cases.tsv")).map_err(|e| e.to_string())?;
        for line in txt.lines().skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 16 {
                continue;
            }
            let ghost = f[0];
            let start_tick: usize = f[1].parse().unwrap_or(0);
            let oracle: Option<u32> = f[4].parse().ok();
            let finished = f[8] != "-";
            let exited = f[12] == "true";
            let tape = PathBuf::from(f[15]);
            let Ok(rows_txt) = std::fs::read_to_string(tape.with_extension("rows.tsv")) else { continue };
            let off = offsets.get(ghost).copied().unwrap_or(-1550);
            let start_label = start_tick as i64 * 10 + off;
            let prefix: Vec<u32> = crossings.get(ghost).map(|v| v.iter().filter(|(_, ms)| *ms < start_label).map(|(w, _)| *w).collect()).unwrap_or_default();
            let already: Vec<bool> = gates.gates.iter().map(|g| prefix.contains(&g.waypoint)).collect();
            let mut rows: Vec<forkoracle::layout::Row> = rows_txt
                .lines()
                .skip(1)
                .filter_map(|l| {
                    let c: Vec<f64> = l.split('\t').filter_map(|x| x.parse().ok()).collect();
                    if c.len() < 12 {
                        return None;
                    }
                    Some(forkoracle::layout::Row { time_ms: c[0] as i64, x: c[1], y: c[2], z: c[3], vx: c[4], vy: c[5], vz: c[6], qw: c[7], qx: c[8], qy: c[9], qz: c[10], wetness: c[11] })
                })
                .collect();
            if exited {
                tmreach::rig::extrapolate_exit(&mut rows);
            }
            // the oracle adjudicates nothing later than ~2.5 s after the DECLARED time
            let cut = declared.get(ghost).copied().unwrap_or(i64::MAX / 2) - 20 + tmreach::oraclectl::ADJUDICATION_GRACE_MS;
            rows.retain(|r| r.time_ms <= cut);
            let first = det.first_crossings(&gates, &rows, &already);
            let new = first.iter().filter(|t| **t >= 0).count() as u32;
            let det_cps = prefix.len() as u32 + new;
            let det_fin = first.iter().enumerate().any(|(gi, t)| *t >= 0 && gates.gates[gi].kind == tmreach::gates::GateKind::Finish);
            total += 1;
            *hist.entry((det_cps, oracle)).or_default() += 1;
            let ok = match oracle {
                Some(x) if x == det_cps && det_fin == finished => true,
                Some(0) if det_cps == 1 && !det_fin => {
                    blind += 1;
                    continue;
                }
                _ => false,
            };
            if ok {
                agree += 1;
                if finished {
                    fin_agree += 1;
                }
            } else {
                disagree += 1;
                if finished || det_fin {
                    fin_dis += 1;
                }
                bad.push(format!("{} t{} m{}: det {} (fin {}) vs oracle {:?} (fin {}) prefix {:?} new {:?}", ghost, start_tick, f[2], det_cps, det_fin, oracle, finished, prefix, first));
            }
        }
    }
    println!("REJUDGE with {}: {} cases; agree {} (finishes {}), disagree {} (finish-related {}), blind {}", a.req("detector"), total, agree, fin_agree, disagree, fin_dis, blind);
    println!("(det_cps, oracle_cps) histogram: {:?}", hist);
    for b in bad.iter().take(40) {
        println!("  {b}");
    }
    println!("=> {}", if disagree == 0 { "PASS" } else { "FAIL" });
    Ok(())
}
