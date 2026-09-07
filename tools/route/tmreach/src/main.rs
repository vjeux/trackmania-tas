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
        "campaign" => cmd_campaign(&a),
        "explore" => cmd_explore(&a),
        "replay" => cmd_replay(&a),
        "effects" => cmd_effects(&a),
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
    // GEOM normals pointing AGAINST the humans' travel at their crossings are flipped
    // for the detector (Summer 2026 - 12: a RoadDirtFinish and RoadDirtCheckpoints credited
    // at s = +14.6 and +2.5 "past" the centre -- the normal, not the trigger, was backwards)
    let vels: Vec<(u32, [f64; 3])> = runs.iter().flat_map(|r| r.crossings.iter().filter_map(|c| c.row_step.as_ref().map(|s| (c.gate_wp, [s.vx, s.vy, s.vz])))).collect();
    let flips = gates.against_travel(&vels);
    let mut gates = (*gates).clone();
    gates.apply_flip_list(&flips);
    if !flips.is_empty() {
        println!("GEOM normals flipped for the detector (humans cross them against the normal): {:?}", flips);
    }
    let (mut det, notes) = fit(&runs, &gates, &prov);
    det.flipped = flips;
    println!("\nFITTED DETECTOR (plane at per-model s_off from the crediting geometry, credited tick = T-1; lat 10 m road / GEOM item, up -6..+8: see gatecal::fit):");
    for n in &notes {
        println!("  {n}");
    }
    for (m, t) in &det.per_model {
        println!("  {m}: s_off {:+.3} m, depth {:.1}, lat_half {:.1}, up {:+.1}..{:+.1}", t.s_off, t.depth, t.lat_half, t.up_lo, t.up_hi);
    }
    let gr = grade(&runs, &gates, &det);
    println!("GRADE vs the ghosts' own notices (first row inside vs the row before the notice, T-1): {} => {}", gr, if gr.passes() { "PASS (bar: ±2 ticks on ≥95 %, no missed, no extra)" } else { "FAIL" });
    if a.has("point-probe") {
        println!("\nWHICH POINT OF THE CAR: plane slack per model for the centre shifted l m along a body axis / the velocity");
        for l in probe_point_hypotheses(&runs, &gates) {
            println!("  {l}");
        }
        for l in probe_box_hypotheses(&runs, &gates) {
            println!("  {l}");
        }
        for l in probe_rotation_hypotheses(&runs, &gates) {
            println!("  {l}");
        }
    }
    let cg = counter_grade(&runs, &gates, &det);
    println!("ENGINE COUNTER control (Row::cps steps vs detector rows, finish included): {} => {}", cg, if cg.passes() { "PASS" } else { "FAIL" });
    for l in cg.extra_list.iter().take(12) {
        println!("  extra detection: {l}");
    }
    for l in cg.unmatched_list.iter().take(12) {
        println!("  unmatched step: {l}");
    }
    // the engine counter is the authority when the runs carry it; the notice grade is
    // informational then (a ghost's checkpoints_ms may omit the finish, which the
    // counter steps for -- Summer 2026 - 04)
    let gatecal_pass = if cg.runs_without_counter < runs.len() { cg.passes() } else { gr.passes() };
    println!("GATECAL VERDICT: {} ({})", if gatecal_pass { "PASS" } else { "FAIL" }, if cg.runs_without_counter < runs.len() { "engine counter authoritative; notices informational" } else { "no engine counter: notices" });
    std::fs::write(out.join("detector.json"), det.to_json()).map_err(|e| e.to_string())?;
    std::fs::write(out.join("gate-clouds.tsv"), gate_clouds_tsv(&runs, &gates)).map_err(|e| e.to_string())?;
    std::fs::write(out.join("grade.txt"), format!("notices: {}\n{}\nengine counter: {}\n{}\n", gr, if gr.passes() { "PASS" } else { "FAIL" }, cg, if cg.passes() { "PASS" } else { "FAIL" })).map_err(|e| e.to_string())?;
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
    let det_path = PathBuf::from(a.req("detector"));
    let det = std::sync::Arc::new(tmreach::gates::Detector::from_json(&std::fs::read_to_string(&det_path).map_err(|e| e.to_string())?)?);
    let mut gates_m = MapGates::load(&map, Some(&geom))?;
    gates_m.apply_flips(&det);
    let gates = std::sync::Arc::new(gates_m);
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
        // --long-horizon TICKS,K : horizon TICKS on every K-th start (default 600 on every 3rd)
        long_horizon: match a.get("long-horizon") {
            Some("none") => None,
            Some(s) => {
                let mut p = s.split(',');
                Some((p.next().unwrap().parse().unwrap(), p.next().unwrap_or("3").parse().unwrap()))
            }
            None => Some((600, 3)),
        },
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
    let mut p4 = tmreach::tmr::Path4Writer::create(&out.join("path4.tmp4"))?;
    let mut starts = String::from(tmreach::starts::starts_tsv_header());
    let mut endpoints = String::from("start_id\tmacro_id\thorizon\tx\ty\tz\n");
    let mut others = String::from("ghost\tstart_id\tmacro_id\thorizon\tgate\tmacro\tnote\n");
    let mut log = String::new();
    let mut tot = Stats::default();
    let mut ok = 0;
    let (mut human_legs, mut human_resp) = (0usize, 0usize);
    let mut fam_cells: std::collections::BTreeMap<String, Vec<usize>> = Default::default();
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
                for (i, r) in fo.records.iter().enumerate() {
                    writer.push(r)?;
                    p4.push(fo.paths.get(i).ok_or("a record without its path points")?)?;
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
                for (fam, v) in &fo.stats.family_cells {
                    fam_cells.entry(fam.clone()).or_default().extend(v.iter().copied());
                }
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
    let p4_count = p4.close()?;
    if p4_count != count {
        return Err(format!("path4.tmp4 has {p4_count} records, samples.tmr {count}"));
    }
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
         human legs (positives) {}, respawn negatives {}\n\
         distinct end cells per start by macro family (median over starts): {}\n",
        tmreach::GIT_HASH, gates.map_uid, hostname(), ok, items.len(), tot.rollouts, count, wall, tot.rollouts as f64 / wall, tmreach::pool::cap(pcfg.workers),
        1000.0 * tot.rollout_secs / tot.rollouts.max(1) as f64,
        tot.outcomes[0], tot.outcomes[1], tot.outcomes[2], tot.outcomes[3], tot.outcomes[4],
        tot.reached_next, 100.0 * tot.reached_next as f64 / tot.rollouts.max(1) as f64,
        tot.reached_other, 100.0 * tot.reached_other as f64 / tot.rollouts.max(1) as f64,
        tot.noop, tot.out_of_tape, tot.errors, tot.identity_max_m, tot.identity_fail, cells.len(), tot.start_blend_max_m,
        lib.len(), horizons.len(), med, cells.first().copied().unwrap_or(0), cells.last().copied().unwrap_or(0), tot.switches, human_legs, human_resp,
        fam_cells.iter().map(|(f, v)| { let mut v = v.clone(); v.sort(); format!("{f} {}", v.get(v.len() / 2).copied().unwrap_or(0)) }).collect::<Vec<_>>().join(", ")
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
    // the TMP4 sidecar, when present: same count, last point == the record's end
    let p4_path = dir.join("path4.tmp4");
    if p4_path.exists() {
        let p4 = tmreach::tmr::read_path4(&p4_path)?;
        let mut bad = 0;
        if p4.len() != shard.records.len() {
            println!("path4.tmp4: {} records vs {} in samples.tmr", p4.len(), shard.records.len());
            bad += 1;
        }
        for (r, p) in shard.records.iter().zip(p4.iter()) {
            let e = p[tmreach::tmr::TMP4_POINTS - 1];
            let d = ((e.pos[0] - r.end.pos[0]).powi(2) + (e.pos[1] - r.end.pos[1]).powi(2) + (e.pos[2] - r.end.pos[2]).powi(2)).sqrt();
            if d > 0.001 && r.outcome != tmreach::tmr::OUTCOME_FINISHED {
                bad += 1;
            }
        }
        println!("path4.tmp4: {} records x {} points; last point == end on all but {} (finished rollouts end at the crossing row, their path at the last row)", p4.len(), tmreach::tmr::TMP4_POINTS, bad);
        if p4.len() != shard.records.len() {
            return Err("verify FAILED (path4)".into());
        }
    }
    if let Some(p) = a.get("dump") {
        let mut s = String::from("start_id\tmacro\th\toutcome\trace_ms\tx\ty\tz\tspeed\tcps\tfin\tgates\tpath\tvmin\tvmax\n");
        let mut recs = shard.records.clone();
        recs.sort_by_key(|r| (r.start_id, r.macro_id, r.horizon_ticks));
        for r in &recs {
            let g: Vec<String> = r.gate_tick.iter().enumerate().filter(|(_, t)| **t >= 0).map(|(w, t)| format!("wp{w}")).collect();
            s.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.2}\t{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\n", r.start_id, r.macro_id, r.horizon_ticks, r.outcome, r.end.race_ms, r.end.pos[0], r.end.pos[1], r.end.pos[2], r.end.speed, r.end.cps, r.end.finished as u8, g.join(","), r.path_len_m, r.min_speed, r.max_speed));
        }
        std::fs::write(p, s).map_err(|e| e.to_string())?;
    }
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
    let det_path = PathBuf::from(a.req("detector"));
    let det = std::sync::Arc::new(tmreach::gates::Detector::from_json(&std::fs::read_to_string(&det_path).map_err(|e| e.to_string())?)?);
    let mut gates_m = MapGates::load(&map, Some(&geom))?;
    gates_m.apply_flips(&det);
    let gates = std::sync::Arc::new(gates_m);
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
    // THE BLIND CLASS IS k < floor(N/2): on Summer 2026 - 01 (N = 4) k = 1 read as none and
    // k = 2 was reported; on Summer 2026 - 16 (N = 9) k = 2 and 3 read as none and k = 4 was
    // reported. N from the oracle's own "(k out of N)" text when any case shows it, else
    // the number of gate groups (checkpoint groups + 1).
    let n_total: u32 = cases
        .iter()
        .filter_map(|c| c.oracle_desc.split("out of ").nth(1).and_then(|s| s.split(|ch: char| !ch.is_ascii_digit()).next()).and_then(|d| d.parse::<u32>().ok()))
        .max()
        .unwrap_or_else(|| gates.gates.iter().filter(|g| g.kind != tmreach::gates::GateKind::Finish && g.kind != tmreach::gates::GateKind::Start).map(|g| g.group).collect::<std::collections::BTreeSet<_>>().len() as u32 + 1);
    let blind_below = (n_total / 2).max(2);
    let (mut agree, mut disagree, mut unanswered, mut blind, mut near, mut fin, mut fin_dt) = (0, 0, 0, 0, 0, 0, Vec::new());
    let mut tail = 0;
    let mut after_tape = 0;
    let finished_case = |c: &tmreach::oraclectl::Case| c.oracle_ms.is_some() || c.det_finished;
    for c in &cases {
        s.push_str(&case_tsv_row(c));
        match c.oracle_cps {
            None => unanswered += 1,
            Some(x) if x == c.det_cps => agree += 1,
            Some(0) if c.det_cps >= 1 && c.det_cps < blind_below && !c.det_finished => blind += 1,
            // the oracle credited MORE than the engine did inside the assumed window and the
            // child traced rows past that window: the extra credit fell in the tail whose
            // adjudication end is not pinned (declared + 2.5 s holds on Summer 2026 - 01;
            // Summer 2026 - 11 credited a checkpoint later than that) -- counted apart
            _ if c.finish_after_tape || c.oracle_ms.map(|t| t > c.tape_end_ms).unwrap_or(false) => after_tape += 1,
            Some(x) if x > c.det_cps && c.rows_past_cut > 0 && !finished_case(c) => tail += 1,
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
        "ORACLE CONTROL: {} cases; detector == oracle on {} ({:.1} %), disagree {}, unanswered {}, in the oracle's blind class (1 <= credited < {} of {}, oracle reports none) {}, tail-window ambiguities (oracle credited more, after declared + grace) {}, FINISHES AFTER THE TAPE'S OWN END (batch-dependent oracle, its own class) {}; {} near-misses (< 40 m of an uncredited gate), {} finishes (oracle time − detector finish-row time, ms: {:?})\n(det_cps, oracle_cps) histogram: {:?}\n=> {}",
        cases.len(),
        agree,
        100.0 * agree as f64 / cases.len().max(1) as f64,
        disagree,
        unanswered,
        blind_below,
        n_total,
        blind,
        tail,
        after_tape,
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
    let det = tmreach::gates::Detector::from_json(&std::fs::read_to_string(a.req("detector")).map_err(|e| e.to_string())?)?;
    let mut gates = MapGates::load(&map, Some(&geom))?;
    gates.apply_flips(&det);
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
    let det = tmreach::gates::Detector::from_json(&std::fs::read_to_string(a.req("detector")).map_err(|e| e.to_string())?)?;
    let mut gates = MapGates::load(&map, Some(&geom))?;
    gates.apply_flips(&det);
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
                    Some(forkoracle::layout::Row { time_ms: c[0] as i64, x: c[1], y: c[2], z: c[3], vx: c[4], vy: c[5], vz: c[6], qw: c[7], qx: c[8], qy: c[9], qz: c[10], wetness: c[11], cps: u32::MAX, vis: forkoracle::layout::Vis::UNKNOWN })
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

/// G5: every map that has `resim_verdict == exact` ghosts in the player's
/// manifest — select the ghosts, fit + control the detector (gatecal), a
/// small plain-oracle control (oraclectl), the fan-out, verify, bank to
/// `reach/v0/<mapUid>/` with a CONTROL.md made of the sub-commands' own
/// summaries. Each stage is this binary run as a child (its stdout is the
/// record); a failing stage stops that map, never the campaign.
fn cmd_campaign(a: &Args) -> Result<(), String> {
    let manifest = PathBuf::from(a.req("manifest"));
    let ghosts_root = PathBuf::from(a.req("ghosts-root")); // .../data/v0/maps
    let maps_dir = PathBuf::from(a.req("maps-dir")); // <uid>.Map.Gbx
    let bank = PathBuf::from(a.req("bank")); // .../reach/v0
    let scratch = PathBuf::from(a.get("scratch").unwrap_or("/tmp/tmreach/campaign"));
    let workers = a.get("workers").unwrap_or("64").to_string();
    let shards = a.get("shards").unwrap_or("3").to_string();
    let filter = a.get("maps").map(|s| s.split(',').map(|x| x.to_string()).collect::<Vec<_>>());
    let name_filter = a.get("name-filter").map(|s| s.to_string());
    let redo = a.has("redo");
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // manifest → per map: name, [(rank, declared_ms)] of exact ghosts
    let txt = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let mut lines = txt.lines();
    let header: Vec<&str> = lines.next().ok_or("empty manifest")?.split('\t').collect();
    let col = |n: &str| header.iter().position(|h| *h == n).ok_or(format!("manifest lacks column {n}"));
    let (c_uid, c_name, c_rank, c_decl, c_verdict, c_kb) = (col("map_uid")?, col("map_name")?, col("rank")?, col("declared_ms")?, col("resim_verdict")?, col("keyboard")?);
    let mut maps: std::collections::BTreeMap<String, (String, Vec<(String, String, bool)>, usize)> = Default::default();
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() <= c_kb {
            continue;
        }
        let e = maps.entry(f[c_uid].to_string()).or_insert((f[c_name].to_string(), Vec::new(), 0));
        e.2 += 1;
        if f[c_verdict] == "exact" {
            e.1.push((f[c_rank].to_string(), f[c_decl].to_string(), f[c_kb] == "true"));
        }
    }
    let mut report = String::new();
    // --newest-first: campaigns by (year desc, season Fall > Summer > Spring > Winter), country maps last
    let rank = |name: &str| -> (i64, i64, String) {
        let year: i64 = name.split_whitespace().find_map(|t| t.parse::<i64>().ok()).filter(|y| *y > 2000).unwrap_or(0);
        let season = if name.starts_with("Fall") { 4 } else if name.starts_with("Summer") { 3 } else if name.starts_with("Spring") { 2 } else if name.starts_with("Winter") { 1 } else { 0 };
        let country = if season == 0 { 1 } else { 0 };
        (country, -(year * 10 + season), name.to_string())
    };
    let mut order: Vec<&String> = maps.keys().collect();
    if a.has("newest-first") {
        order.sort_by_key(|u| rank(&maps[*u].0));
    }
    // --partition k/n: this process takes the maps at index k mod n of the ordered list (two
    // campaign processes overlap one's oracle phase with the other's fan-out)
    let (pk, pn): (usize, usize) = a.get("partition").map(|s| { let mut p = s.split('/'); (p.next().unwrap().parse().unwrap(), p.next().unwrap().parse().unwrap()) }).unwrap_or((0, 1));
    for (mi, uid) in order.into_iter().enumerate() {
        if mi % pn != pk {
            continue;
        }
        let (name, exact, total) = &maps[uid];
        if let Some(fl) = &filter {
            if !fl.contains(uid) {
                continue;
            }
        }
        if let Some(nf) = &name_filter {
            if !name.contains(nf.as_str()) {
                continue;
            }
        }
        let out_dir = bank.join(uid);
        if out_dir.join("samples.tmr").exists() && !redo {
            println!("== {name} ({uid}): already banked, skipping (--redo to rebuild)");
            continue;
        }
        println!("== {name} ({uid}): {} exact of {total} ghosts", exact.len());
        if exact.is_empty() {
            report.push_str(&format!("{name}\t{uid}\tSKIPPED: no resim-exact ghost ({total} in the manifest)\n"));
            continue;
        }
        // the map file: <maps-dir>/<uid>.Map.Gbx (cartographer bank) or the player's maps/<uid>/map.Map.Gbx,
        // copied into scratch (a mount read can be partial; tmroute gates then refuses it)
        let map_src = [maps_dir.join(format!("{uid}.Map.Gbx")), ghosts_root.join(uid).join("map.Map.Gbx")].into_iter().find(|p| p.exists());
        let Some(map_src) = map_src else {
            report.push_str(&format!("{name}\t{uid}\tSKIPPED: no map file under {} or {}\n", maps_dir.display(), ghosts_root.join(uid).display()));
            continue;
        };
        let map = scratch.join(uid).join(format!("{uid}.Map.Gbx"));
        std::fs::create_dir_all(map.parent().unwrap()).map_err(|e| e.to_string())?;
        if let Err(e) = std::fs::copy(&map_src, &map) {
            report.push_str(&format!("{name}\t{uid}\tSKIPPED: map copy failed: {e}\n"));
            continue;
        }
        // ghost directory of symlinks: <rank>-<declared>.Ghost.Gbx under the player's maps/<uid>/ghosts
        let gdir = scratch.join(uid).join("ghosts");
        let _ = std::fs::remove_dir_all(&gdir);
        std::fs::create_dir_all(&gdir).map_err(|e| e.to_string())?;
        // the player's durable copy is ghosts.tar (its ghosts/ directory on the mount is
        // sometimes empty or partial): extract it into scratch and link from there
        let tar = ghosts_root.join(uid).join("ghosts.tar");
        let src_dir = if tar.exists() {
            let tdir = scratch.join(uid).join("tar");
            let _ = std::fs::remove_dir_all(&tdir);
            std::fs::create_dir_all(&tdir).map_err(|e| e.to_string())?;
            let st = std::process::Command::new("tar").args(["xf", &tar.to_string_lossy(), "-C", &tdir.to_string_lossy()]).status().map_err(|e| e.to_string())?;
            if !st.success() {
                report.push_str(&format!("{name}\t{uid}\tSKIPPED: ghosts.tar did not extract\n"));
                continue;
            }
            tdir.join("ghosts")
        } else {
            ghosts_root.join(uid).join("ghosts")
        };
        let mut n_linked = 0;
        let mut n_keyboard = 0;
        for (rank, decl, kb) in exact {
            // the player names files <ghost_id?>-<declared>.Ghost.Gbx; match on the declared time
            let mut found = None;
            if let Ok(rd) = std::fs::read_dir(&src_dir) {
                for ent in rd.flatten() {
                    let fname = ent.file_name().to_string_lossy().into_owned();
                    if fname.ends_with(".Ghost.Gbx") && fname.trim_end_matches(".Ghost.Gbx").split('-').nth(1) == Some(decl.as_str()) {
                        found = Some(ent.path());
                        break;
                    }
                }
            }
            let Some(src) = found else {
                report.push_str(&format!("{name}\t{uid}\tghost rank {rank} declared {decl}: file not found under {}\n", src_dir.display()));
                continue;
            };
            let dst = gdir.join(format!("r{:0>3}_{}.Ghost.Gbx", rank, decl));
            std::os::unix::fs::symlink(&src, &dst).map_err(|e| e.to_string())?;
            n_linked += 1;
            if *kb {
                n_keyboard += 1;
            }
        }
        if n_linked == 0 {
            report.push_str(&format!("{name}\t{uid}\tSKIPPED: no ghost file linked\n"));
            continue;
        }
        // GATES: generated LOCALLY from the map with tmroute (the same code the GEOM arm runs;
        // deterministic), because the bank copy on the mount can read as a partial file for
        // minutes after GEOM's rename (manifoldfs cross-box lag). When the bank copy parses, the
        // two gate lists are compared and any difference is a control line, never a block.
        let geom_local = scratch.join("geom");
        let gates_local = geom_local.join(uid).join("gates.json");
        std::fs::create_dir_all(gates_local.parent().unwrap()).map_err(|e| e.to_string())?;
        let tmroute = exe.with_file_name("tmroute");
        let st = std::process::Command::new(&tmroute).args(["gates", &map.to_string_lossy(), "--out", &gates_local.to_string_lossy()]).output().map_err(|e| format!("tmroute: {e}"))?;
        if !st.status.success() {
            report.push_str(&format!("{name}\t{uid}\tFAILED: tmroute gates: {}\n", String::from_utf8_lossy(&st.stderr).lines().last().unwrap_or("")));
            continue;
        }
        let gates_cmp = {
            let local = MapGates::load(&map, Some(&geom_local));
            let bank_geom = a.get("geom").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{}/persistent/private-30d/tm-route/geom", std::env::var("HOME").unwrap_or_default())));
            let bank = MapGates::load_geom(&bank_geom.join(uid).join("gates.json"));
            match (local, bank) {
                (Ok(l), Ok(b)) => {
                    let key = |g: &MapGates| g.gates.iter().map(|x| format!("wp{} {:?} grp {} c ({:.2},{:.2},{:.2}) |n| ({:.2},{:.2},{:.2}) hw {:.1} {}", x.waypoint, x.kind, x.group, x.centre[0], x.centre[1], x.centre[2], x.normal[0].abs(), x.normal[1].abs(), x.normal[2].abs(), x.half_width, x.model)).collect::<Vec<_>>();
                    let (kl, kb) = (key(&l), key(&b));
                    if kl == kb { format!("local tmroute gates == bank gates.json ({} gates; normal signs ignored, the detector measures them)", kl.len()) } else { format!("GATES DIFFER local vs bank: local {:?} bank {:?}", kl, kb) }
                }
                (Ok(_), Err(e)) => format!("bank gates.json unreadable ({e}); local tmroute gates used"),
                (Err(e), _) => format!("LOCAL gates unreadable: {e}"),
            }
        };
        let geom_arg = geom_local.to_string_lossy().into_owned();
        let work = scratch.join(uid).join("work");
        let stage_dir = scratch.join(uid);
        let run = |args: &[&str]| -> Result<String, String> {
            let t = std::time::Instant::now();
            let o = std::process::Command::new(&exe).args(args).output().map_err(|e| e.to_string())?;
            let so = String::from_utf8_lossy(&o.stdout).into_owned();
            let se = String::from_utf8_lossy(&o.stderr).into_owned();
            let _ = std::fs::write(stage_dir.join(format!("{}.log", args[0])), format!("{so}\n[stderr]\n{se}"));
            if !o.status.success() {
                return Err(format!("stage {} failed ({:?}, {:.0} s): {}", args[0], o.status.code(), t.elapsed().as_secs_f64(), se.lines().last().unwrap_or("")));
            }
            Ok(so)
        };
        let g = gdir.to_string_lossy().into_owned();
        let m = map.to_string_lossy().into_owned();
        let w = work.to_string_lossy().into_owned();
        let gc_out = stage_dir.join("gatecal").to_string_lossy().into_owned();
        let oc_out = stage_dir.join("oraclectl").to_string_lossy().into_owned();
        let fo_out = stage_dir.join("fanout").to_string_lossy().into_owned();
        let det = format!("{gc_out}/detector.json");
        let stages: Result<(String, String, String, String), String> = (|| {
            let gc_workers = workers.parse::<usize>().unwrap_or(32).min(n_linked).max(1).to_string();
            let gc = run(&["gatecal", "--map", &m, "--ghosts", &g, "--workers", &gc_workers, "--out", &gc_out, "--work", &w, "--geom", &geom_arg])?;
            let oc = run(&["oraclectl", "--map", &m, "--ghosts", &g, "--detector", &det, "--out", &oc_out, "--work", &w, "--workers", "12", "--ghost-stride", "4", "--every", "2500", "--macros", "0,2,9,21,26,29,33,38", "--geom", &geom_arg])?;
            let fo = run(&["fanout", "--map", &m, "--ghosts", &g, "--detector", &det, "--out", &fo_out, "--work", &w, "--workers", &workers, "--shards", &shards, "--horizons", "200,400", "--geom", &geom_arg])?;
            let ve = run(&["verify", "--dir", &fo_out])?;
            Ok((gc, oc, fo, ve))
        })();
        match stages {
            Err(e) => {
                println!("   FAILED: {e}");
                report.push_str(&format!("{name}\t{uid}\tFAILED: {e}\n"));
                continue;
            }
            Ok((gc, oc, fo, ve)) => {
                let pick = |s: &str, keys: &[&str]| -> String { s.lines().filter(|l| keys.iter().any(|k| l.contains(k))).map(|l| format!("{l}\n")).collect() };
                let gc_pass = gc.contains("GATECAL VERDICT: PASS");
                let oc_pass = oc.lines().any(|l| l.starts_with("=> PASS"));
                // the summary line: "identity (macro 0 ...): max X m, N fails over M starts"
                let id_fails: usize = fo.lines().find(|l| l.starts_with("identity (")).and_then(|l| l.split(", ").nth(1)).and_then(|s| s.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(usize::MAX);
                let items_ok = fo.lines().find(|l| l.starts_with("fanout ")).and_then(|l| l.split(": ").nth(1)).and_then(|s| s.split_whitespace().next()).map(|ab| { let mut p = ab.split('/'); p.next().unwrap_or("0") == p.next().unwrap_or("1") }).unwrap_or(false);
                // a ghost whose startup controls failed (identity / start position) is EXCLUDED, fail
                // closed, and the map still passes when at most a quarter of its ghosts are excluded
                let excluded: Vec<String> = fo.lines().filter(|l| l.contains(": FAILED: ")).map(|l| l.split(": FAILED: ").next().unwrap_or("").rsplit('/').next().unwrap_or("").to_string()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                let other_failures = fo.lines().filter(|l| l.contains(": FAILED: ") && !(l.contains("label shift") || l.contains("IDENTITY") || l.contains("matches the telemetry") || l.contains("startup controls") || l.contains("START-POSITION"))).count();
                let fo_fail = id_fails != 0 || other_failures > 0 || excluded.len() * 4 > n_linked || (items_ok == false && excluded.is_empty());
                let ve_ok = ve.contains("verify OK");
                let verdict = if gc_pass && oc_pass && !fo_fail && ve_ok { "PASS" } else { "FAIL" };
                let ctrl = format!(
                    "# CONTROL.md — {name} ({uid}) — tmreach campaign {} on {}, {}\n\nGhosts: {n_linked} resim-exact of {total} in the player manifest ({n_keyboard} keyboard){}. Verdict: **{verdict}** (bank only on PASS).\n\nGates: {}\n\n## gatecal (planes fitted on the engine counter's rows; controls vs the ghosts' notices and vs the counter)\n{}\n## oraclectl (plain oracle vs the engine-credited, geometry-attributed count; stride 4 ghosts, 8 macros, starts every 2.500 s)\n{}\n## fanout\n{}\n## verify\n{}",
                    tmreach::GIT_HASH,
                    hostname(),
                    chrono_now(),
                    if excluded.is_empty() { String::new() } else { format!("; EXCLUDED (startup controls failed, fail closed): {}", excluded.join(", ")) },
                    gates_cmp,
                    pick(&gc, &["ghosts ok", "slack", "GRADE", "ENGINE COUNTER", "extra detection", "unmatched step", "ORDER", "GATECAL VERDICT"]),
                    oc.lines().filter(|l| l.starts_with("ORACLE CONTROL") || l.starts_with("(det_cps") || l.starts_with("=> ")).map(|l| format!("{l}\n")).collect::<String>(),
                    fo.lines().filter(|l| l.starts_with("fanout ") || l.starts_with("outcomes ") || l.starts_with("identity (") || l.starts_with("distinct end") || l.starts_with("human legs") || l.contains("FAILED") || (l.contains("unattributed steps") && !l.contains(" 0 unattributed steps"))).map(|l| format!("{l}\n")).collect::<String>(),
                    pick(&ve, &["TMR0", "verify"])
                );
                std::fs::write(stage_dir.join("CONTROL.md"), &ctrl).map_err(|e| e.to_string())?;
                if verdict == "PASS" {
                    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
                    for f in ["samples.tmr", "path4.tmp4", "starts.tsv", "macros.tsv", "endpoints.tsv", "other-connections.tsv", "detector.json", "FANOUT.log"] {
                        std::fs::copy(PathBuf::from(&fo_out).join(f), out_dir.join(f)).map_err(|e| format!("bank {f}: {e}"))?;
                    }
                    std::fs::write(out_dir.join("CONTROL.md"), &ctrl).map_err(|e| e.to_string())?;
                    for f in ["crossings.tsv", "grade.txt"] {
                        let _ = std::fs::copy(PathBuf::from(&gc_out).join(f), out_dir.join(format!("gatecal-{f}")));
                    }
                    let _ = std::fs::copy(PathBuf::from(&oc_out).join("cases.tsv"), out_dir.join("oraclectl-cases.tsv"));
                    println!("   banked -> {}", out_dir.display());
                } else {
                    println!("   NOT banked (verdict FAIL); see {}", stage_dir.join("CONTROL.md").display());
                }
                let fo_line = fo.lines().find(|l| l.starts_with("fanout ")).unwrap_or("").to_string();
                report.push_str(&format!("{name}\t{uid}\t{verdict}\t{n_linked} ghosts\t{}\n", fo_line));
            }
        }
        let _ = std::fs::remove_dir_all(&work);
    }
    println!("\nCAMPAIGN REPORT\n{report}");
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    std::fs::write(scratch.join("campaign-report.tsv"), &report).map_err(|e| e.to_string())?;
    Ok(())
}

fn chrono_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // UTC, civil from days (Howard Hinnant's algorithm), no chrono dependency
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}Z", rem / 3600, (rem % 3600) / 60)
}

fn cmd_explore(a: &Args) -> Result<(), String> {
    let map = PathBuf::from(a.req("map"));
    let ghosts_dir = PathBuf::from(a.req("ghosts"));
    let out = PathBuf::from(a.req("out"));
    let geom = a.get("geom").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{}/persistent/private-30d/tm-route/geom", std::env::var("HOME").unwrap_or_default())));
    let det = tmreach::gates::Detector::from_json(&std::fs::read_to_string(a.req("detector")).map_err(|e| e.to_string())?)?;
    let mut gates = MapGates::load(&map, Some(&geom))?;
    gates.apply_flips(&det);
    let budget: usize = a.get("budget").unwrap_or("20000").parse().map_err(|_| "--budget N")?;
    let h: usize = a.get("h").unwrap_or("200").parse().map_err(|_| "--h ticks")?;
    let every: i64 = a.get("every").unwrap_or("1000").parse().map_err(|_| "--every ms")?;
    let seed: u64 = a.get("seed").unwrap_or("1").parse().map_err(|_| "--seed")?;
    let limit = a.get("limit").map(|s| s.parse::<usize>().unwrap());
    let mut ghosts = tmreach::pool::ghosts_in(&ghosts_dir)?;
    if let Some(l) = limit {
        ghosts.truncate(l);
    }
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let cfg = std::sync::Arc::new(tmreach::explore::ExploreCfg { gates, det, macros: tmreach::explore::default_macros(), h, every_ms: every, budget, seed, out: out.clone() });
    let pcfg = pool_cfg(a, &map, "explore");
    println!("explore: {} ghosts, {} workers, budget {} rollouts per ghost, h {} ticks, seeds every {} ms, cells 2 m x 2 m x 5 m/s x 30 deg x cps", ghosts.len(), pcfg.workers, budget, h, every);
    let t0 = std::time::Instant::now();
    let c2 = cfg.clone();
    let results = tmreach::pool::run_per_ghost(&pcfg, &ghosts, move |_i, w, tel| tmreach::explore::explore_ghost(w, tel, &c2));
    let mut conn = String::from(tmreach::explore::connections_header());
    let (mut cells, mut rollouts, mut steps, mut nconn, mut ok) = (0, 0, 0, 0, 0);
    for (g, r) in ghosts.iter().zip(results) {
        match r {
            Ok(o) => {
                ok += 1;
                cells += o.cells;
                rollouts += o.rollouts;
                steps += o.steps;
                nconn += o.connections.len();
                for c in &o.connections {
                    conn.push_str(c);
                    conn.push('\n');
                }
                for l in &o.log {
                    println!("{l}");
                }
            }
            Err(e) => println!("{}: FAILED: {e}", g.display()),
        }
    }
    // the plain oracle on every connection tape (visible only when the credited count
    // reaches floor(N/2); the engine counter is what found them)
    let mut conn = conn;
    if nconn > 0 && !a.has("no-oracle") {
        let (server, _) = engine_paths(a);
        let tapes: Vec<PathBuf> = conn.lines().skip(1).filter_map(|l| l.split('\t').last()).map(PathBuf::from).collect();
        // at most --oracle-max tapes (default 200): a multi-finish map yields thousands of connections
        let omax: usize = a.get("oracle-max").unwrap_or("200").parse().unwrap_or(200);
        let refs: Vec<&std::path::Path> = tapes.iter().take(omax).map(|p| p.as_path()).collect();
        match ghost::oracle::validate_many(&server, &refs, ghost::oracle::MapsMode::One(&map), "tmreach-explore") {
            Ok(res) => {
                let mut s = String::new();
                for (i, l) in conn.lines().enumerate() {
                    if i == 0 {
                        s.push_str(l.trim_end());
                        s.push_str("\toracle\n");
                        continue;
                    }
                    let fname = tapes[i - 1].file_name().unwrap().to_string_lossy().into_owned();
                    let r = res.iter().find(|r| r.file == fname || r.file.ends_with(&fname));
                    let o = r.map(|r| format!("cps {:?} time {:?} {}", r.cps, r.time_ms, r.desc.trim().replace('\n', " "))).unwrap_or("-".into());
                    s.push_str(&format!("{l}\t{o}\n"));
                }
                conn = s;
            }
            Err(e) => println!("oracle on the connection tapes failed: {e}"),
        }
    }
    std::fs::write(out.join("connections.tsv"), &conn).map_err(|e| e.to_string())?;
    // SUMMARY: distinct (credited-before set -> gate) pairs, with counts and the earliest crossing
    {
        let mut pairs: std::collections::BTreeMap<(String, String), (usize, String, String)> = Default::default();
        for l in conn.lines().skip(1) {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 8 {
                continue;
            }
            let e = pairs.entry((f[3].to_string(), f[4].to_string())).or_insert((0, f[7].to_string(), f[5].to_string()));
            e.0 += 1;
            if f[7] < e.1.as_str() {
                e.1 = f[7].to_string();
            }
        }
        let mut s = String::from("credited_before\tgate\thuman_next\tcount\tearliest_cross_race\n");
        for ((before, gate), (n, t, next)) in &pairs {
            s.push_str(&format!("{before}\t{gate}\t{next}\t{n}\t{t}\n"));
        }
        std::fs::write(out.join("connections-summary.tsv"), &s).map_err(|e| e.to_string())?;
        println!("connection pairs (credited-before set -> gate): {}", pairs.len());
        for ((before, gate), (n, t, next)) in pairs.iter().take(30) {
            println!("  [{before}] -> {gate} (human next {next}): {n} rollouts, earliest at race {t}");
        }
    }
    let wall = t0.elapsed().as_secs_f64();
    println!(
        "explore {} on {} ({}): {}/{} ghosts ok, {} rollouts in {:.1} s wall ({:.1}/s), {} explore steps, {} archive cells, {} other-gate connections -> {}",
        tmreach::GIT_HASH, cfg.gates.map_uid, hostname(), ok, ghosts.len(), rollouts, wall, rollouts as f64 / wall.max(1e-9), steps, cells, nconn, out.join("connections.tsv").display()
    );
    Ok(())
}

/// Diagnostic: replay a macro chain from a human savestate and print the rows (with the engine counter).
fn cmd_replay(a: &Args) -> Result<(), String> {
    let map = PathBuf::from(a.req("map"));
    let ghost = PathBuf::from(a.req("ghost"));
    let f: usize = a.req("f").parse().map_err(|_| "--f tick")?;
    let h: usize = a.get("h").unwrap_or("200").parse().map_err(|_| "--h")?;
    let chain: Vec<u16> = a.req("chain").split(',').map(|s| s.parse().unwrap()).collect();
    let (server, shim) = engine_paths(a);
    let work = a.get("work").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("/tmp/tmreach/replay-{}", std::process::id())));
    let mut w = Worker::start(&server, &map, &shim, &work, &ghost, a.has("verbose"))?;
    let n = w.n_ticks();
    let macros = tmreach::explore::default_macros();
    let recs = w.reference_recs(w.root_probe, f - w.root_probe);
    let (rows0, nf) = w.rollout_keep(branch::ROOT, &recs, w.root_probe, (f - w.root_probe) as u64)?;
    let last0 = rows0.last().unwrap();
    println!("prefix to f {f}: floor {} last row label {} race {} cps {} at ({:.2}, {:.2}, {:.2})", w.floor(nf)?, last0.time_ms, tmreach::secs(w.race_of(last0)), last0.cps, last0.x, last0.y, last0.z);
    let mut t = f;
    let mut all: Vec<forkoracle::forksrv::Rec> = Vec::new();
    for mid in &chain {
        let mm = macros.iter().find(|x| x.id == *mid).ok_or("no such macro")?;
        let b: Vec<(u8, u8, u8)> = (t..t + h).map(|k| (w.tape.steer[k.min(n - 1)], w.tape.accel[k.min(n - 1)], w.tape.brake[k.min(n - 1)])).collect();
        match tmreach::macros::build(mm, &b, false) {
            tmreach::macros::Built::Recs(r) => all.extend(r),
            tmreach::macros::Built::NoOp => all.extend(b.iter().map(|&(s, g, br)| forkoracle::forksrv::rec_of(s, g, br))),
        }
        t += h;
    }
    let rolled = w.rollout(nf, &all, f, all.len() as u64)?;
    println!("chain {:?} from f {f}: {} rows, exited {}", chain, rolled.rows.len(), rolled.exited);
    let every: usize = a.get("every").unwrap_or("10").parse().unwrap_or(10);
    let mut prev = u32::MAX;
    for (i, r) in rolled.rows.iter().enumerate() {
        let step = r.cps != u32::MAX && prev != u32::MAX && r.cps != prev;
        if i % every == 0 || step {
            println!("  {} race {} cps {} pos ({:.2}, {:.2}, {:.2}) v {:.1}{}", r.time_ms, tmreach::secs(w.race_of(r)), r.cps, r.x, r.y, r.z, tmreach::rig::speed(r), if step { "   <-- COUNTER STEP" } else { "" });
        }
        prev = r.cps;
    }
    if let Some(out) = a.get("out") {
        tmreach::starts::write_trace(&PathBuf::from(out), &rolled.rows)?;
    }
    Ok(())
}

fn cmd_effects(a: &Args) -> Result<(), String> {
    let map = PathBuf::from(a.req("map"));
    let ghost = PathBuf::from(a.req("ghost"));
    let out = PathBuf::from(a.req("out"));
    let t0: usize = a.req("t0").parse().map_err(|_| "--t0 tick")?;
    let n: usize = a.get("n").unwrap_or("300").parse().map_err(|_| "--n ticks")?;
    let at: usize = a.get("at").unwrap_or("100").parse().map_err(|_| "--at idx of the crossing within the window")?;
    let (server, shim) = engine_paths(a);
    let work = a.get("work").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("/tmp/tmreach/effects-{}", std::process::id())));
    let mut w = Worker::start(&server, &map, &shim, &work, &ghost, a.has("verbose"))?;
    let prov = w.car.clone();
    let layout = w.forest.layout().cloned().ok_or("no layout")?;
    println!("car {prov}; layout pos {:#x} vis {:#x} clock {:#x} cps {:#x}", layout.pos, layout.vis, layout.clock, layout.cps);
    let windows = vec![
        tmreach::effects::Window { name: "vehicle", base: prov.phy.saturating_sub(0x8000), len: 0x18000 },
        tmreach::effects::Window { name: "participant", base: prov.participant.saturating_sub(0x8000), len: 0x18000 },
        tmreach::effects::Window { name: "controller", base: prov.controller.saturating_sub(0x4000), len: 0x8000 },
        tmreach::effects::Window { name: "sim", base: prov.sim.saturating_sub(0x4000), len: 0x8000 },
        tmreach::effects::Window { name: "scene", base: prov.scene.saturating_sub(0x2000), len: 0x4000 },
        tmreach::effects::Window { name: "playground", base: prov.playground.saturating_sub(0x4000), len: 0x8000 },
    ];
    let t = std::time::Instant::now();
    let o = tmreach::effects::scan(&mut w, t0, n, windows)?;
    println!("scanned {} ticks ({} .. {}) race {} .. {} in {:.1} s", o.ticks.len(), o.ticks.first().unwrap_or(&0), o.ticks.last().unwrap_or(&0), tmreach::secs(*o.race_ms.first().unwrap_or(&0)), tmreach::secs(*o.race_ms.last().unwrap_or(&0)), t.elapsed().as_secs_f64());
    tmreach::effects::write_dumps(&o, &out)?;
    let timers = tmreach::effects::find_timers(&o, a.get("min-run").unwrap_or("20").parse().unwrap_or(20));
    println!("\nTIMER candidates (jump then a constant step per tick):");
    for c in &timers {
        println!("  {} +{:#x} ({}): idx {} race {}: {}   series {:?}", c.window, c.offset, c.kind, c.first_tick_idx, tmreach::secs(o.race_ms[c.first_tick_idx]), c.note, c.series.iter().take(8).map(|v| format!("{v}")).collect::<Vec<_>>());
    }
    let flags = tmreach::effects::find_flags(&o, at, a.get("radius").unwrap_or("15").parse().unwrap_or(15));
    println!("\nFLAG candidates (a byte that changes once or twice, within ±radius ticks of idx {at}):");
    for c in flags.iter().take(60) {
        println!("  {} +{:#x}: {}", c.window, c.offset, c.note);
    }
    let (alo, ahi): (f64, f64) = (a.get("appear-lo").unwrap_or("7000").parse().unwrap_or(7000.0), a.get("appear-hi").unwrap_or("9000").parse().unwrap_or(9000.0));
    let app = tmreach::effects::find_appearing(&o, alo, ahi, at.saturating_sub(20), at + 40);
    println!("\nAPPEARING candidates (a value in [{alo}, {ahi}] appearing within idx {}..{}):", at.saturating_sub(20), at + 40);
    for c in app.iter().take(60) {
        println!("  {} +{:#x} ({}): {}  series {:?}", c.window, c.offset, c.kind, c.note, c.series.iter().map(|v| format!("{v:.3}")).collect::<Vec<_>>());
    }
    println!("{} timer candidates, {} flag candidates, {} appearing; dumps in {}", timers.len(), flags.len(), app.len(), out.display());
    Ok(())
}
