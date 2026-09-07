//! `tmr` CLI — the MODEL arm's tool.
//!
//!   tmr features                         print FEATURES.md (the feature layout, generated)
//!   tmr frame --starts F.tsv             MEASURE which car axis is forward (quaternion convention control)
//!   tmr build --reach DIR [--geom G] [--maps M] --out CACHE [--no-geometry]
//!                                        TMR0 shards → labelled rows per map (<uid>.rows + manifest.tsv)
//!   tmr train --cache DIR --out r.tmw [--ablation full|no-probes|no-attitude|distance-only]
//!             [--epochs N] [--hidden 256,256,256] [--batch B] [--lr X] [--held-out uid,..] [--report F] [--threads T]
//!   tmr eval --model r.tmw --cache DIR [--held-out uid,..] [--report F]
//!   tmr selftest --model r.tmw           agrees_with (flat vs candle) + the negative half (a perturbed copy must be REFUSED)
//!   tmr plan MAP.Map.Gbx --gates gates.json --model r.tmw [--top-k 3] [--beam 4000] [--p-floor 0.02] [--out-dir DIR] [--source NAME]
//!                                        the planner over R (tmplan's beam, R as the EdgeEstimator) — the M2 seam
//!   tmr split UID..                      which maps the fnv1a64 rule holds out
//!   tmr legs MAP.Map.Gbx --gates gates.json --model r.tmw --human-orders F
//!                                        R's estimate of every human leg (order agreement + per-leg p, time)
//! Times print as seconds with a decimal.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tmr::data::{self, Rows};
use tmr::features;
use tmr::net::Weights;
use tmr::train::{Set, TrainCfg};

pub const GIT_HASH: &str = env!("TMR_GIT_HASH");

fn die(msg: &str) -> ! {
    eprintln!("tmr: {msg}");
    std::process::exit(2)
}
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}
fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}
fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname").map(|s| s.trim().to_string()).unwrap_or_default()
}
fn now_utc() -> String {
    // seconds since epoch → ISO-ish, without a chrono dependency
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = s / 86400;
    let (h, m) = ((s % 86400) / 3600, (s % 3600) / 60);
    // civil from days (Howard Hinnant)
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}Z")
}
fn provenance(what: &str) -> String {
    format!("tmr {what} {GIT_HASH} {} {}", hostname(), now_utc())
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}
fn default_geom() -> PathBuf {
    home().join("persistent/private-30d/tm-route/geom")
}
fn default_maps() -> Vec<PathBuf> {
    vec![home().join("persistent/private-30d/tm-autopilot/B-cartographer/bank/maps"), home().join("persistent/private-30d/tm-player/data/v0/maps")]
}
fn find_map(uid: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    for d in dirs {
        for c in [d.join(format!("{uid}.Map.Gbx")), d.join(uid).join("map.Map.Gbx")] {
            if c.exists() {
                return Some(c);
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(t) = flag(&args, "--threads") {
        std::env::set_var("RAYON_NUM_THREADS", &t);
    } else if std::env::var("RAYON_NUM_THREADS").is_err() {
        // COMMON-RULES 7: keep ≥ 8 cores free; candle's gemm uses rayon
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8);
        std::env::set_var("RAYON_NUM_THREADS", (n.saturating_sub(8)).clamp(1, 32).to_string());
    }
    match args.first().map(|s| s.as_str()) {
        Some("features") => print!("{}", features::describe()),
        Some("frame") => cmd_frame(&args),
        Some("build") => cmd_build(&args),
        Some("train") => cmd_train(&args),
        Some("eval") => cmd_eval(&args),
        Some("selftest") => cmd_selftest(&args),
        Some("plan") => cmd_plan(&args),
        Some("legs") => cmd_legs(&args),
        Some("split") => {
            for u in args.iter().skip(1) {
                println!("{u}\tfnv1a64 % 10 = {}\t{}", data::fnv1a64(u) % 10, if data::held_out(u) { "HELD-OUT" } else { "train" });
            }
        }
        _ => {
            eprintln!("tmr features | frame | build | train | eval | selftest | plan | legs  (see src/main.rs header)");
            std::process::exit(2)
        }
    }
}

fn cmd_frame(args: &[String]) {
    let p = flag(args, "--starts").unwrap_or_else(|| die("--starts F.tsv"));
    let starts = data::read_starts(Path::new(&p)).unwrap_or_else(|e| die(&e));
    let rows: Vec<([f32; 3], [f32; 4])> = starts.values().map(|s| (s.state.vel, s.state.quat)).collect();
    let (acc, n) = tmr::frame::alignment(&rows, 5.0);
    println!("frame control on {}: {} rows faster than 5 m/s; mean dot(velocity dir, rotated local axis): +X {:+.3}  +Y {:+.3}  +Z {:+.3}", p, n, acc[0], acc[1], acc[2]);
    let (ax, best) = acc.iter().enumerate().max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap()).unwrap();
    let name = ["X", "Y", "Z"][ax];
    println!("forward axis: {}{} (|mean dot| {:.3}) — the code assumes +Z: {}", if *best >= 0.0 { "+" } else { "-" }, name, best.abs(), if ax == 2 && *best > 0.9 { "PASS" } else { "FAIL — fix frame.rs before training" });
}

fn cmd_build(args: &[String]) {
    let reach = PathBuf::from(flag(args, "--reach").unwrap_or_else(|| die("--reach DIR")));
    let out = PathBuf::from(flag(args, "--out").unwrap_or_else(|| die("--out CACHE")));
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&e.to_string()));
    let geom = flag(args, "--geom").map(PathBuf::from).unwrap_or_else(default_geom);
    let mut maps = default_maps();
    if let Some(m) = flag(args, "--maps") {
        maps.insert(0, PathBuf::from(m));
    }
    let no_geom = has(args, "--no-geometry");
    let dirs = data::shard_dirs(&reach);
    if dirs.is_empty() {
        die(&format!("no samples.tmr under {}", reach.display()));
    }
    let mut manifest = String::from("map_uid\tmap_name\theld_out\trecords\trows\tpositives\tpos_rate\thuman_rows\thuman_pos\tband_rows\tfinish_candidates\tpos_beyond_400\tpos_outside_radius\tunknown_ghost_starts\tshard\tprovenance\n");
    let mut log = Vec::new();
    for d in dirs {
        let Some(uid) = data::shard_map_uid(&d) else {
            eprintln!("  {}: cannot tell its map uid (dir name or FANOUT.log) — skipped", d.display());
            continue;
        };
        let gdir = geom.join(&uid);
        let gates = match tmroute::io::read_gates(&gdir.join("gates.json")) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("  {uid}: {e} — skipped");
                continue;
            }
        };
        let t0 = std::time::Instant::now();
        let surf = if no_geom {
            None
        } else {
            let Some(mp) = find_map(&uid, &maps) else {
                eprintln!("  {uid}: no .Map.Gbx in {:?} — skipped (use --maps or --no-geometry)", maps);
                continue;
            };
            match tmplan::surface::SurfaceModel::build(&mp, &gates, false, false) {
                Ok((s, _nodes)) => Some(s),
                Err(e) => {
                    eprintln!("  {uid}: surface build failed: {e} — skipped");
                    continue;
                }
            }
        };
        let probe = match &surf {
            Some(s) => features::Probe { idx: Some(&s.full), road: &s.road_materials },
            None => features::Probe::none(),
        };
        let (rows, st) = match data::build_map(&d, &gates, &gdir, &probe, &mut log) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("  {uid}: {e} — skipped");
                continue;
            }
        };
        let f = out.join(format!("{uid}.rows"));
        rows.write(&f).unwrap_or_else(|e| die(&e));
        let held = data::held_out(&uid);
        println!("{}  [{}]  {:.1} s  → {}", log.last().unwrap(), if held { "HELD-OUT by fnv1a64(uid) % 10 == 0" } else { "train" }, t0.elapsed().as_secs_f64(), f.display());
        if let Some(s) = &surf {
            for n in &s.notes {
                println!("    surface note: {n}");
            }
        }
        manifest.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            uid, gates.map_name, held as u8, st.records, st.rows, st.positives, st.positives as f64 / st.rows.max(1) as f64, st.human_rows, st.human_pos, st.band_rows, st.finish_candidates, st.positives_beyond_400, st.positives_outside_radius, st.unknown_ghost_starts, d.display(), provenance("build")
        ));
    }
    std::fs::write(out.join("manifest.tsv"), &manifest).unwrap_or_else(|e| die(&e.to_string()));
    std::fs::write(out.join("BUILD.log"), log.join("\n") + "\n").unwrap_or_else(|e| die(&e.to_string()));
    println!("wrote {}/{{<uid>.rows, manifest.tsv, BUILD.log}}", out.display());
}

/// Load every `<uid>.rows` in the cache, split by the map rule (+ overrides).
fn load_cache(args: &[String]) -> (Vec<Rows>, Vec<bool>) {
    let cache = PathBuf::from(flag(args, "--cache").unwrap_or_else(|| die("--cache DIR")));
    let force: Vec<String> = flag(args, "--held-out").map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    let only: Option<Vec<String>> = flag(args, "--maps-only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    let mut files: Vec<PathBuf> = std::fs::read_dir(&cache)
        .unwrap_or_else(|e| die(&format!("{}: {e}", cache.display())))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map_or(false, |x| x == "rows"))
        .collect();
    files.sort();
    let mut rows = Vec::new();
    let mut held = Vec::new();
    for f in files {
        let r = Rows::read(&f).unwrap_or_else(|e| die(&e));
        if let Some(o) = &only {
            if !o.contains(&r.map_uid) {
                continue;
            }
        }
        let h = data::held_out(&r.map_uid) || force.contains(&r.map_uid);
        held.push(h);
        rows.push(r);
    }
    if rows.is_empty() {
        die("no .rows files in the cache");
    }
    (rows, held)
}

fn map_names() -> HashMap<String, String> {
    let mut m = HashMap::new();
    if let Ok(rd) = std::fs::read_dir(default_geom()) {
        for e in rd.flatten() {
            if let Ok(g) = tmroute::io::read_gates(&e.path().join("gates.json")) {
                m.insert(g.map_uid, g.map_name);
            }
        }
    }
    m
}

fn split_summary(rows: &[Rows], held: &[bool], names: &HashMap<String, String>) -> String {
    let mut s = String::new();
    for (r, h) in rows.iter().zip(held) {
        let pos = (0..r.n).filter(|&i| r.lab(i)[data::L_Y] > 0.5).count();
        s.push_str(&format!("  {:<9} {}  {}: {} rows, {} positives ({:.1} %), fnv1a64 % 10 = {}\n", if *h { "HELD-OUT" } else { "train" }, r.map_uid, names.get(&r.map_uid).cloned().unwrap_or_default(), r.n, pos, 100.0 * pos as f64 / r.n.max(1) as f64, data::fnv1a64(&r.map_uid) % 10));
    }
    s
}

fn eval_sets(w: &Weights, rows: &[Rows], held: &[bool], keep: &[&str], dev: &candle_core::Device, out: &mut String) {
    let train_rows: Vec<&Rows> = rows.iter().zip(held).filter(|(_, h)| !**h).map(|(r, _)| r).collect();
    let held_rows: Vec<&Rows> = rows.iter().zip(held).filter(|(_, h)| **h).map(|(r, _)| r).collect();
    for (name, set_rows) in [("TRAIN maps", train_rows), ("HELD-OUT maps", held_rows)] {
        if set_rows.is_empty() {
            out.push_str(&format!("[{name}] none — the two-gate test on held-out maps needs ≥ 1 held-out map (the fnv1a64 rule or --held-out)\n"));
            continue;
        }
        let set = Set::from_rows(&set_rows, keep);
        let pred = tmr::train::predict_all_candle(w, &set, dev).unwrap_or_else(|e| die(&e));
        let rep = tmr::eval::evaluate(&set, &pred, 7);
        out.push_str(&tmr::eval::render(name, &rep));
        // per map inside the set
        if set_rows.len() > 1 {
            for r in &set_rows {
                let s1 = Set::from_rows(&[r], keep);
                let p1 = tmr::train::predict_all_candle(w, &s1, dev).unwrap_or_else(|e| die(&e));
                let r1 = tmr::eval::evaluate(&s1, &p1, 7);
                out.push_str(&format!("[{name}]   {}: two-gate R {:.1} % vs baseline {:.1} % ({} pairs); hard R {:.1} % vs {:.1} %; ECE {:.4} AUC {:.4}; human-leg MAE {:.3} s ({} legs)\n", r.map_uid, r1.two_gate.model_pct(), r1.two_gate.baseline_pct(), r1.two_gate.pairs, r1.two_gate.hard_model_pct(), r1.two_gate.hard_baseline_pct(), r1.calib.ece, r1.calib.auc, r1.time_human.mae_s, r1.time_human.n));
            }
        }
    }
}

fn cmd_train(args: &[String]) {
    let (rows, held) = load_cache(args);
    let names = map_names();
    let mut cfg = TrainCfg::default();
    if let Some(a) = flag(args, "--ablation") {
        cfg.ablation = a;
    }
    let keep = features::ablation_keep(&cfg.ablation).unwrap_or_else(|| die(&format!("--ablation {}: full|no-probes|no-attitude|distance-only", cfg.ablation)));
    if let Some(e) = flag(args, "--epochs") {
        cfg.epochs = e.parse().unwrap_or_else(|_| die("--epochs N"));
    }
    if let Some(h) = flag(args, "--hidden") {
        cfg.hidden = h.split(',').map(|x| x.parse().unwrap_or_else(|_| die("--hidden a,b,c"))).collect();
    }
    if let Some(b) = flag(args, "--batch") {
        cfg.batch = b.parse().unwrap_or_else(|_| die("--batch B"));
    }
    if let Some(l) = flag(args, "--lr") {
        cfg.lr = l.parse().unwrap_or_else(|_| die("--lr X"));
    }
    if let Some(s) = flag(args, "--seed") {
        cfg.seed = s.parse().unwrap_or_else(|_| die("--seed N"));
    }
    if let Some(p) = flag(args, "--patience") {
        cfg.patience = p.parse().unwrap_or_else(|_| die("--patience N"));
    }
    let out = PathBuf::from(flag(args, "--out").unwrap_or_else(|| die("--out r.tmw")));
    let dev = candle_core::Device::Cpu;
    let mut report = format!("# tmr train — {}\n\n## Split (by MAP: fnv1a64(uid) % 10 == 0 held out; --held-out adds {:?})\n{}\n", provenance("train"), flag(args, "--held-out").unwrap_or_default(), split_summary(&rows, &held, &names));
    let train_rows: Vec<&Rows> = rows.iter().zip(&held).filter(|(_, h)| !**h).map(|(r, _)| r).collect();
    if train_rows.is_empty() {
        die("every map is held out — nothing to train on");
    }
    let train_set = Set::from_rows(&train_rows, &keep);
    let h_max = (0..train_set.n).map(|i| train_set.lab(i)[data::L_H]).fold(0f32, f32::max);
    let h_min = (0..train_set.n).map(|i| train_set.lab(i)[data::L_H]).fold(f32::INFINITY, f32::min);
    print!("{report}");
    let t0 = std::time::Instant::now();
    let rep = tmr::train::train(&train_set, &cfg, &dev, true).unwrap_or_else(|e| die(&e));
    let secs = t0.elapsed().as_secs_f64();
    report.push_str(&format!("## Training\n{}\nbest epoch {} (val loss {:.4}), {} epochs run, {:.0} s wall\n\n", rep.log.join("\n"), rep.best_epoch, rep.best_val, rep.epochs_run, secs));
    let mut w = rep.weights;
    let meta = serde_json::json!({
        "produced_by": provenance("train"),
        "ablation": cfg.ablation,
        "hidden": cfg.hidden,
        "epochs_run": rep.epochs_run,
        "best_epoch": rep.best_epoch,
        "best_val_loss": rep.best_val,
        "train_maps": train_rows.iter().map(|r| r.map_uid.clone()).collect::<Vec<_>>(),
        "held_out_maps": rows.iter().zip(&held).filter(|(_, h)| **h).map(|(r, _)| r.map_uid.clone()).collect::<Vec<_>>(),
        "train_rows": train_set.n,
        "h_min": h_min, "h_max": h_max,
        "feature_version": features::FEATURE_VERSION,
    });
    w.meta = meta.to_string();
    w.save(&out).unwrap_or_else(|e| die(&e));
    // the negative half of the control, every time a model is written
    let tb = tmr::net::Trainable::from_weights(&w, &dev).unwrap_or_else(|e| die(&e.to_string()));
    let mut bad = w.clone();
    bad.perturb_first_bias(1e-3);
    let neg = match bad.agrees_with(&tb, &dev, 64, 1e-4) {
        Err(_) => "REFUSED (PASS)".to_string(),
        Ok(v) => format!("ACCEPTED with worst {v:.3e} — the check cannot fail; FAIL"),
    };
    report.push_str(&format!("agrees_with negative half (first bias +1e-3): {neg}\n\n## Evaluation\n"));
    let mut ev = String::new();
    eval_sets(&w, &rows, &held, &keep, &dev, &mut ev);
    print!("{ev}");
    report.push_str(&ev);
    println!("wrote {} ({} params, {} bytes of meta)", out.display(), w.n_params(), w.meta.len());
    if let Some(r) = flag(args, "--report") {
        std::fs::write(&r, &report).unwrap_or_else(|e| die(&e.to_string()));
        println!("wrote {r}");
    }
}

fn cmd_eval(args: &[String]) {
    let model = PathBuf::from(flag(args, "--model").unwrap_or_else(|| die("--model r.tmw")));
    let w = Weights::load(&model).unwrap_or_else(|e| die(&e));
    let meta: serde_json::Value = serde_json::from_str(&w.meta).unwrap_or(serde_json::Value::Null);
    let abl = meta.get("ablation").and_then(|a| a.as_str()).unwrap_or("full").to_string();
    let keep = features::ablation_keep(&abl).unwrap_or_else(|| die("model meta names an unknown ablation"));
    let (rows, held) = load_cache(args);
    let names = map_names();
    let dev = candle_core::Device::Cpu;
    let mut report = format!("# tmr eval — {} — model {} ({}), ablation {}\n\n## Split\n{}\n", provenance("eval"), model.display(), meta.get("produced_by").and_then(|p| p.as_str()).unwrap_or("?"), abl, split_summary(&rows, &held, &names));
    eval_sets(&w, &rows, &held, &keep, &dev, &mut report);
    print!("{report}");
    if let Some(r) = flag(args, "--report") {
        std::fs::write(&r, &report).unwrap_or_else(|e| die(&e.to_string()));
    }
}

fn cmd_selftest(args: &[String]) {
    let model = PathBuf::from(flag(args, "--model").unwrap_or_else(|| die("--model r.tmw")));
    let w = Weights::load(&model).unwrap_or_else(|e| die(&e));
    let dev = candle_core::Device::Cpu;
    let t = tmr::net::Trainable::from_weights(&w, &dev).unwrap_or_else(|e| die(&e.to_string()));
    let mut fails = 0;
    match w.agrees_with(&t, &dev, 64, 1e-4) {
        Ok(v) => println!("agrees_with: flat forward == candle forward, worst |Δ| {v:.3e} over 64 random inputs (tol 1e-4) → PASS"),
        Err(e) => {
            println!("agrees_with: {e} → FAIL");
            fails += 1;
        }
    }
    let mut bad = w.clone();
    bad.perturb_first_bias(1e-3);
    match bad.agrees_with(&t, &dev, 64, 1e-4) {
        Err(_) => println!("negative half: a copy with its first bias +1e-3 is REFUSED → PASS"),
        Ok(v) => {
            println!("negative half: the perturbed copy was ACCEPTED (worst {v:.3e}) — the check cannot fail → FAIL");
            fails += 1;
        }
    }
    println!("model {}: dims {:?}, {} params, feature version {}, meta {}", model.display(), w.dims, w.n_params(), features::FEATURE_VERSION, w.meta);
    if fails > 0 {
        std::process::exit(1);
    }
}

fn load_plan_ctx(args: &[String]) -> (tmroute::gates::GatesFile, tmplan::surface::SurfaceModel, tmplan::surface::Nodes, Weights, u16, u16, Vec<&'static str>) {
    let map = args.iter().find(|a| a.ends_with(".Map.Gbx")).cloned().unwrap_or_else(|| die("MAP.Map.Gbx required"));
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates gates.json required"));
    let gates = tmroute::io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let model = PathBuf::from(flag(args, "--model").unwrap_or_else(|| die("--model r.tmw")));
    let w = Weights::load(&model).unwrap_or_else(|e| die(&e));
    let meta: serde_json::Value = serde_json::from_str(&w.meta).unwrap_or(serde_json::Value::Null);
    let abl = meta.get("ablation").and_then(|a| a.as_str()).unwrap_or("full").to_string();
    let keep = features::ablation_keep(&abl).unwrap_or_else(|| die("model meta names an unknown ablation"));
    let h_max = meta.get("h_max").and_then(|v| v.as_f64()).unwrap_or(400.0) as u16;
    let h_min = meta.get("h_min").and_then(|v| v.as_f64()).unwrap_or(200.0) as u16;
    let (surf, nodes) = tmplan::surface::SurfaceModel::build(Path::new(&map), &gates, !has(args, "--quiet"), flag(args, "--grid").map_or(false, |g| g == "deco")).unwrap_or_else(|e| die(&e));
    for n in &surf.notes {
        println!("  note: {n}");
    }
    (gates, surf, nodes, w, h_min, h_max, keep)
}

fn order_str(nodes: &tmplan::surface::Nodes, gates: &tmroute::gates::GatesFile, visit: &[usize]) -> (String, String) {
    let groups: Vec<String> = visit.iter().skip(1).map(|&n| nodes.groups[n].to_string()).collect();
    let wps: Vec<String> = visit.iter().skip(1).map(|&n| gates.group_rep(nodes.groups[n]).unwrap().waypoint.to_string()).collect();
    (groups.join(","), wps.join(","))
}

fn cmd_plan(args: &[String]) {
    use tmplan::estimator::{EdgeEstimator, StateBucket};
    let (gates, surf, nodes, w, h_min, h_max, keep) = load_plan_ctx(args);
    let est = tmr::estimator::REstimator { w: &w, gates: &gates, nodes: &nodes, surf: &surf, h_max, h_min, keep, p_floor: flag(args, "--p-floor").and_then(|s| s.parse().ok()).unwrap_or(0.02) };
    let width: usize = flag(args, "--beam").and_then(|s| s.parse().ok()).unwrap_or(4000);
    let top_k: usize = flag(args, "--top-k").and_then(|s| s.parse().ok()).unwrap_or(3);
    if has(args, "--matrix") {
        println!("R edge matrix from rest (p_reach / expected s / h used), spawn = node 0:");
        for i in 0..nodes.pos.len() {
            let mut line = format!("  {i:>2}:");
            for j in 0..nodes.pos.len() {
                if i == j {
                    line.push_str("        -        ");
                    continue;
                }
                match est.query(StateBucket::of_speed(0.0), None, i, j) {
                    Some((e, h, _)) => line.push_str(&format!(" {:.2}/{:>6}/{:>3}", e.p_reach, tmr::secs((e.expected_ticks * 10.0) as i64), h)),
                    None => line.push_str("      n/a       "),
                }
            }
            println!("{line}");
        }
    }
    let plans = tmplan::planner::beam(&nodes, &est, width, top_k, StateBucket::of_speed(0.0));
    println!("{}\t{}\tcp_groups {}\tfinish_groups {}\testimator {}\tbeam {}\tplans {}", gates.map_name, gates.map_uid, nodes.n_cp, nodes.n_fin, est.name(), width, plans.len());
    let prov = provenance("plan");
    let out_dir = flag(args, "--out-dir");
    let (_d, _len, _drop, fields) = surf.distance_matrix_full(&nodes);
    for (k, p) in plans.iter().enumerate() {
        let (g, wp) = order_str(&nodes, &gates, &p.visit);
        let legs: Vec<String> = p.edges.iter().map(|e| format!("{:.2}@{}", e.p_reach, tmr::secs(e.expected_ms as i64))).collect();
        println!("  rank {k}: predicted {}  P(reach) {:.3}  groups [{}]  waypoints [{}]  legs p@t [{}]", tmr::secs(p.total_ms as i64), p.p_reach, g, wp, legs.join(" "));
        if let Some(dir) = &out_dir {
            let source = flag(args, "--source").unwrap_or_else(|| "router-plan-r".into());
            let mut route = tmplan::export::export(&gates, &nodes, &surf, &fields, p, k as u32, &est.name(), &prov);
            route.source = source.clone();
            if let Some(r) = route.route.as_mut() {
                r.source = source.clone();
            }
            let f = Path::new(dir).join(&gates.map_uid).join(tmroute::io::route_file_name(&source, k as u32));
            tmroute::io::write_route(&f, &route).unwrap_or_else(|e| die(&e));
            println!("    wrote {}", f.display());
        }
    }
    if plans.is_empty() {
        println!("  NO PLAN under R (every tour has a leg below p_floor). Not a verdict.");
        std::process::exit(1);
    }
}

/// R's view of the human legs: for each human order (human-orders.tsv), the
/// per-leg p_reach and time, and whether R ranks the human's next gate first
/// among the uncredited gates at each step.
fn cmd_legs(args: &[String]) {
    use tmplan::estimator::StateBucket;
    let (gates, surf, nodes, w, h_min, h_max, keep) = load_plan_ctx(args);
    let est = tmr::estimator::REstimator { w: &w, gates: &gates, nodes: &nodes, surf: &surf, h_max, h_min, keep, p_floor: 0.0 };
    let ho = flag(args, "--human-orders").unwrap_or_else(|| die("--human-orders human-orders.tsv"));
    let s = std::fs::read_to_string(&ho).unwrap_or_else(|e| die(&e.to_string()));
    // group → node index
    let node_of_group: HashMap<u32, usize> = nodes.groups.iter().enumerate().skip(1).map(|(i, g)| (*g, i)).collect();
    let mut orders: HashMap<Vec<u32>, (usize, Vec<i32>)> = HashMap::new(); // order (groups) → (count, cp_ms of the first)
    for (i, line) in s.lines().enumerate() {
        if i == 0 {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 6 {
            continue;
        }
        let wps: Vec<u32> = f[3].split(',').filter_map(|x| x.parse().ok()).collect();
        let groups: Vec<u32> = wps.iter().filter_map(|w| gates.by_waypoint(*w).map(|g| g.group)).collect();
        let cp_ms: Vec<i32> = f[5].split(',').filter_map(|x| x.parse().ok()).collect();
        let e = orders.entry(groups).or_insert((0, cp_ms));
        e.0 += 1;
    }
    let mut ords: Vec<(&Vec<u32>, &(usize, Vec<i32>))> = orders.iter().collect();
    ords.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    for (groups, (count, cp_ms)) in ords {
        println!("{}: human order groups {:?} ({} runs), best cp_ms {:?}", gates.map_name, groups, count, cp_ms);
        let mut at = 0usize;
        let mut prev: Option<usize> = None;
        let mut bucket = StateBucket::of_speed(0.0);
        let mut visited: Vec<usize> = vec![0];
        let mut ranked_first = 0;
        let mut total_ms = 0i32;
        for (k, g) in groups.iter().enumerate() {
            let Some(&to) = node_of_group.get(g) else {
                println!("  leg {k}: group {g} is not a planner node");
                continue;
            };
            // rank among the uncredited nodes (+ finish when all cps done)
            let n_left = nodes.n_cp - visited.iter().filter(|v| **v >= 1 && **v <= nodes.n_cp).count();
            let cands: Vec<usize> = (1..nodes.pos.len()).filter(|c| !visited.contains(c) && (*c <= nodes.n_cp || n_left == 0)).collect();
            let mut scored: Vec<(usize, f32, f32, u16)> = cands.iter().filter_map(|&c| est.query(bucket, prev, at, c).map(|(e, h, _)| (c, e.p_reach, e.expected_ticks, h))).collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let rank = scored.iter().position(|x| x.0 == to).map(|r| r + 1).unwrap_or(0);
            if rank == 1 {
                ranked_first += 1;
            }
            let mine = scored.iter().find(|x| x.0 == to);
            let human_leg = if k == 0 { cp_ms.get(0).cloned() } else { cp_ms.get(k).zip(cp_ms.get(k - 1)).map(|(a, b)| a - b) };
            match mine {
                Some((_, p, t, h)) => {
                    total_ms += (*t * 10.0) as i32;
                    println!(
                        "  leg {k}: node {at} → {to} (group {g}): p_reach {:.3}, expected {}, h {}; human {}; R ranks it #{rank} of {} [{}]",
                        p,
                        tmr::secs((*t * 10.0) as i64),
                        h,
                        human_leg.map(|m| tmr::secs(m as i64)).unwrap_or("?".into()),
                        scored.len(),
                        scored.iter().map(|(c, p, _, _)| format!("{c}:{p:.2}")).collect::<Vec<_>>().join(" ")
                    );
                    bucket = StateBucket::of_speed(est.query(bucket, prev, at, to).map(|(e, _, _)| e.speed_mu).unwrap_or(0.0).max(0.0));
                }
                None => println!("  leg {k}: node {at} → {to}: no estimate"),
            }
            prev = Some(at);
            at = to;
            visited.push(to);
        }
        println!("  R ranks the human's next gate first on {}/{} legs; Σ expected {} vs human {}", ranked_first, groups.len(), tmr::secs(total_ms as i64), cp_ms.last().map(|m| tmr::secs(*m as i64)).unwrap_or("?".into()));
    }
}
