//! `tmr` CLI — the MODEL arm's tool.
//!
//!   tmr features                         print FEATURES.md (the feature layout, generated)
//!   tmr frame --starts F.tsv             MEASURE which car axis is forward (quaternion convention control)
//!   tmr build [--kind gate|local] [--fv 1|2] [--max-rows N] --reach DIR [--reach DIR2 ..] [--geom G] [--maps M] --out CACHE [--no-geometry]
//!                                        TMR0 shards → labelled rows per map (<uid>.rows + manifest.tsv)
//!   tmr train [--kind gate|local] --cache DIR --out r.tmw [--ablation full|no-probes|no-attitude|distance-only]
//!             [--epochs N] [--hidden 256,256,256] [--batch B] [--lr X] [--wd X] [--noise σ] [--dropout p] [--geo-dropout p] [--no-mirror] [--held-out uid,..] [--report F] [--threads T]
//!   tmr eval [--kind gate|local] --model r.tmw --cache DIR [--held-out uid,..] [--report F]
//!   tmr selftest --model r.tmw           agrees_with (flat vs candle) + the negative half (a perturbed copy must be REFUSED)
//!   tmr plan MAP.Map.Gbx --gates gates.json --model r.tmw [--local rl.tmw --estimator chained [--chain-beam 24] [--p-step 0.05] [--penalty 3000]]
//!            [--top-k 3] [--beam 4000] [--p-floor 0.02] [--out-dir DIR] [--source NAME]
//!                                        the planner over R (tmplan's beam, R as the EdgeEstimator) — the M2 seam
//!   tmr watch --reach DIR .. --cache DIR --bank DIR [--fv 1|2] [--max-rows N] [--max-rows-total N] [--geo-dropout p] [--held-out uid,..] [--batch B] [--lr X] [--force-first] [--interval S] [--once] [--epochs N] [--threads T]
//!                                        rebuild rows for new/changed shards, retrain both heads, publish bank/r-v<N>.tmw + rl-v<N>.tmw + reports
//!   tmr report --bank DIR [--bank DIR2] [--out REPORT.md]   one table per watcher bank: every version's held-out numbers
//!   tmr split UID..                      which maps the fnv1a64 rule holds out
//!   tmr legs MAP.Map.Gbx --gates gates.json --model r.tmw --human-orders F [--local rl.tmw [--beam 24] [--p-step 0.05]]
//!                                        R's estimate of every human leg (order agreement + per-leg p, time)
//! Times print as seconds with a decimal.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tmr::data::{self, Rows};
use tmr::features;
use tmroute::gates::WpKind;
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
        Some("features") => print!("{}", if fv_of(&args) == 2 { tmr::features2::describe2() } else { features::describe() }),
        Some("frame") => cmd_frame(&args),
        Some("build") => cmd_build(&args),
        Some("train") => cmd_train(&args),
        Some("eval") => cmd_eval(&args),
        Some("selftest") => cmd_selftest(&args),
        Some("plan") => cmd_plan(&args),
        Some("legs") => cmd_legs(&args),
        Some("watch") => cmd_watch(&args),
        Some("probe") => cmd_probe(&args),
        Some("report") => cmd_report(&args),
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

struct BuildOpts {
    /// "gate" (rows per uncredited gate, the order prior) or "local" (horizon-native rows from every endpoint).
    kind: String,
    fv: u32,
    /// Row cap per map (0 = none).
    max_rows: usize,
    /// Featurisation threads.
    threads: usize,
    geom: PathBuf,
    maps: Vec<PathBuf>,
    no_geom: bool,
    out: PathBuf,
}

/// Build one shard dir's rows into `<out>/<uid>.rows`; returns the manifest line and the log line.
fn build_one(d: &Path, o: &BuildOpts, log: &mut Vec<String>) -> Result<(String, String), String> {
    let uid = data::shard_map_uid(d).ok_or_else(|| format!("{}: cannot tell its map uid (dir name or FANOUT.log)", d.display()))?;
    let build_hash = data::shard_build_hash(d);
    // FRAME control before anything is built from this shard (fail closed)
    // A convention change shows as ANOTHER axis carrying the velocity (or none); a low +Z mean alone is
    // physics: Spring 2026 - 08 has 140 of 675 starts driving in REVERSE (dot < −0.9), Summer 2026 - 18
    // 297 of 1812 sideways at ~90 m/s (|dot| < 0.3). Fail only when +Z is not the dominant axis.
    let (acc, nf) = data::frame_control_axes(d)?;
    let fz = acc[2];
    let other = acc[0].abs().max(acc[1].abs());
    if nf >= 20 && (fz < 0.3 || other > fz) {
        return Err(format!("{uid} (build {build_hash}): FRAME control FAILED — mean dot(velocity, rotated local axis): +X {:+.3} +Y {:+.3} +Z {:+.3} over {nf} starts; forward is not local +Z, fix frame.rs before building rows", acc[0], acc[1], fz));
    }
    log.push(format!("  {uid}: generator build {build_hash}, frame control +Z {fz:.3} (X {:+.3}, Y {:+.3}) over {nf} starts → PASS{}", acc[0], acc[1], if fz < 0.9 { " (low mean: reverse / sideways driving on this map, not a convention change)" } else { "" }));
    let gdir = o.geom.join(&uid);
    // the bank is an object store another box writes into: a gates.json mid-rewrite reads truncated.
    // Keep the last good copy in the cache and fall back to it, saying so.
    let cached = o.out.join(format!("{uid}.gates.json"));
    let gates = match tmroute::io::read_gates(&gdir.join("gates.json")) {
        Ok(g) => {
            let _ = std::fs::copy(gdir.join("gates.json"), &cached);
            g
        }
        Err(e) => {
            let g = tmroute::io::read_gates(&cached).map_err(|e2| format!("{e} (and no usable cached copy: {e2})"))?;
            log.push(format!("  {uid}: gates.json in the bank is unreadable ({e}); using the cached copy {}", cached.display()));
            g
        }
    };
    let t0 = std::time::Instant::now();
    let geo = if o.no_geom {
        Geometry::None
    } else {
        let mp = find_map(&uid, &o.maps).ok_or_else(|| format!("{uid}: no .Map.Gbx in {:?} (use --maps or --no-geometry)", o.maps))?;
        build_geometry(o.fv, &mp, &gates, false).map_err(|e| format!("{uid}: geometry build failed: {e}"))?
    };
    let feat = featurizer(&geo);
    let (rows, manifest_tail) = if o.kind == "local" {
        let (rows, st) = data::build_local_map(d, &gates, &gdir, &feat, 1, o.max_rows, o.threads, log)?;
        let m = format!("{}\t{}\t{}\t{:.4}\t0\t0\t{}\t0\t0\t0\t{}", st.groups, rows.n, st.positives, st.positives as f64 / rows.n.max(1) as f64, st.negatives, st.rejected_near_endpoint);
        (rows, m)
    } else {
        let (rows, st) = data::build_map(d, &gates, &gdir, &feat, o.max_rows, o.threads, log)?;
        (rows, format!("{}\t{}\t{}\t{:.4}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", st.records, st.rows, st.positives, st.positives as f64 / st.rows.max(1) as f64, st.human_rows, st.human_pos, st.band_rows, st.finish_candidates, st.positives_beyond_400, st.positives_outside_radius, st.unknown_ghost_starts))
    };
    let _ = &rows;
    let f = o.out.join(rows_file_name(&uid, &o.kind));
    if o.fv == 2 { rows.write_half(&f)? } else { rows.write(&f)? }
    let held = data::held_out(&uid);
    let line = format!("{}  [{}]  {:.1} s  → {}", log.last().cloned().unwrap_or_default(), if held { "HELD-OUT by fnv1a64(uid) % 10 == 0" } else { "train" }, t0.elapsed().as_secs_f64(), f.display());
    if let Geometry::V1(s) = &geo {
        for n in &s.notes {
            log.push(format!("    surface note: {n}"));
        }
    }
    let manifest = format!("{}\tv{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", o.kind, o.fv, uid, gates.map_name, held as u8, manifest_tail, d.display(), build_hash, provenance("build"));
    Ok((manifest, line))
}

const MANIFEST_HEADER: &str = "kind\tfv\tmap_uid\tmap_name\theld_out\trecords_or_groups\trows\tpositives\tpos_rate\thuman_rows\thuman_pos\tband_rows\tfinish_candidates\tpos_beyond_400\tpos_outside_radius\tunknown_ghost_starts\tshard\tgenerator_build\tprovenance\n";

fn build_opts(args: &[String]) -> BuildOpts {
    let out = PathBuf::from(flag(args, "--out").or_else(|| flag(args, "--cache")).unwrap_or_else(|| die("--out CACHE")));
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&e.to_string()));
    let mut maps = default_maps();
    if let Some(m) = flag(args, "--maps") {
        maps.insert(0, PathBuf::from(m));
    }
    BuildOpts { kind: kind_of(args), fv: fv_of(args), max_rows: flag(args, "--max-rows").and_then(|s| s.parse().ok()).unwrap_or(0), threads: flag(args, "--build-threads").and_then(|s| s.parse().ok()).unwrap_or(8), geom: flag(args, "--geom").map(PathBuf::from).unwrap_or_else(default_geom), maps, no_geom: has(args, "--no-geometry"), out }
}

fn reach_dirs(args: &[String]) -> Vec<PathBuf> {
    let reaches: Vec<PathBuf> = args.iter().enumerate().filter(|(_, a)| *a == "--reach").filter_map(|(i, _)| args.get(i + 1).map(PathBuf::from)).collect();
    if reaches.is_empty() {
        die("--reach DIR (repeatable: a reach root of <uid>/ dirs, or one shard dir)");
    }
    reaches.iter().flat_map(|r| data::shard_dirs(r)).collect()
}

fn cmd_build(args: &[String]) {
    let o = build_opts(args);
    let dirs = reach_dirs(args);
    if dirs.is_empty() {
        die("no samples.tmr under the --reach dirs");
    }
    let mut manifest = String::from(MANIFEST_HEADER);
    let mut log = Vec::new();
    for d in dirs {
        match build_one(&d, &o, &mut log) {
            Ok((m, line)) => {
                println!("{line}");
                manifest.push_str(&m);
            }
            Err(e) => eprintln!("  {e} — skipped"),
        }
    }
    std::fs::write(o.out.join(format!("manifest-{}.tsv", o.kind)), &manifest).unwrap_or_else(|e| die(&e.to_string()));
    std::fs::write(o.out.join(format!("BUILD-{}.log", o.kind)), log.join("\n") + "\n").unwrap_or_else(|e| die(&e.to_string()));
    println!("wrote {}/{{<uid>.rows, manifest.tsv, BUILD.log}}", o.out.display());
}

/// Load every `<uid>.rows` in the cache, split by the map rule (+ overrides).
/// `held[i]`: 0 train, 1 held out by the fnv rule, 2 held out by --held-out (the FIXED extra maps, BAR.md 08:30Z).
fn load_cache(args: &[String]) -> (Vec<Rows>, Vec<u8>) {
    let kind = kind_of(args);
    let cache = PathBuf::from(flag(args, "--cache").unwrap_or_else(|| die("--cache DIR")));
    let force: Vec<String> = flag(args, "--held-out").map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    let only: Option<Vec<String>> = flag(args, "--maps-only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    let mut files: Vec<PathBuf> = std::fs::read_dir(&cache)
        .unwrap_or_else(|e| die(&format!("{}: {e}", cache.display())))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            n.ends_with(".rows") && (n.ends_with(".local.rows") == (kind == "local"))
        })
        .collect();
    files.sort();
    let mut rows = Vec::new();
    let mut held = Vec::new();
    for f in files {
        let r = Rows::read_any(&f).unwrap_or_else(|e| die(&e));
        if let Some(o) = &only {
            if !o.contains(&r.map_uid) {
                continue;
            }
        }
        let h = if data::held_out(&r.map_uid) { 1 } else if force.contains(&r.map_uid) { 2 } else { 0 };
        held.push(h);
        rows.push(r);
    }
    if rows.is_empty() {
        die("no .rows files in the cache");
    }
    (rows, held)
}

/// Map names for the given uids only: the cache's last-good copies first, the bank per uid second
/// (never a scan of the bank — 363 dirs on the object-store mount took 91 s).
fn map_names_for(cache: Option<&Path>, uids: &[String]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for u in uids {
        let mut cands = Vec::new();
        if let Some(c) = cache {
            cands.push(c.join(format!("{u}.gates.json")));
        }
        cands.push(default_geom().join(u).join("gates.json"));
        for p in cands {
            if let Ok(g) = tmroute::io::read_gates(&p) {
                m.insert(g.map_uid, g.map_name);
                break;
            }
        }
    }
    m
}

#[allow(dead_code)]
fn map_names(cache: Option<&Path>) -> HashMap<String, String> {
    let mut m = HashMap::new();
    // the cache's last-good copies first (the bank's gates.json may be mid-rewrite)
    if let Some(c) = cache {
        if let Ok(rd) = std::fs::read_dir(c) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().ends_with(".gates.json") {
                    if let Ok(g) = tmroute::io::read_gates(&e.path()) {
                        m.insert(g.map_uid, g.map_name);
                    }
                }
            }
        }
    }
    if let Ok(rd) = std::fs::read_dir(default_geom()) {
        for e in rd.flatten() {
            if let Ok(g) = tmroute::io::read_gates(&e.path().join("gates.json")) {
                m.entry(g.map_uid).or_insert(g.map_name);
            }
        }
    }
    m
}

fn held_label(h: u8) -> &'static str {
    match h { 0 => "train", 1 => "HELD-OUT (fnv)", _ => "HELD-OUT (forced)" }
}

fn split_summary(rows: &[Rows], held: &[u8], names: &HashMap<String, String>) -> String {
    let mut s = String::new();
    for (r, h) in rows.iter().zip(held) {
        let pos = (0..r.n).filter(|&i| r.lab(i)[data::L_Y] > 0.5).count();
        s.push_str(&format!("  {:<18} {}  {}: {} rows, {} positives ({:.1} %), fnv1a64 % 10 = {}\n", held_label(*h), r.map_uid, names.get(&r.map_uid).cloned().unwrap_or_default(), r.n, pos, 100.0 * pos as f64 / r.n.max(1) as f64, data::fnv1a64(&r.map_uid) % 10));
    }
    s
}

/// Per-map two-gate lines, then the pooled numbers: TRAIN maps, ALL held-out
/// maps, and INFORMATIVE held-out maps (distance-only < 99 %) — the gate.
fn eval_sets(w: &Weights, rows: &[Rows], held: &[u8], keep: &[&str], dev: &candle_core::Device, names: &HashMap<String, String>, out: &mut String) {
    let per_map = |r: &Rows| -> tmr::eval::Report {
        let s1 = Set::from_rows(&[r], keep);
        let p1 = tmr::train::predict_all_candle(w, &s1, dev).unwrap_or_else(|e| die(&e));
        tmr::eval::evaluate(&s1, &p1, 7)
    };
    out.push_str("### Per map (two-gate pairs: one random negative per positive; DW = distance-wrong pairs)\n");
    out.push_str("| split | map | pairs | R % | distance % | margin | informative | nearest-neg R/dist | DW pairs (share) | R on DW % | ECE | AUC | human-leg MAE |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    let mut informative_held: Vec<&Rows> = Vec::new();
    for (r, h) in rows.iter().zip(held) {
        let rep = per_map(r);
        let tg = &rep.two_gate;
        if *h > 0 && tg.informative() {
            informative_held.push(r);
        }
        out.push_str(&format!(
            "| {} | {} {} | {} | {:.1} | {:.1} | {:+.1} | {} | {:.1}/{:.1} | {} ({:.1} %) | {:.1} | {:.4} | {:.3} | {:.3} s ({} legs) |\n",
            held_label(*h),
            r.map_uid,
            names.get(&r.map_uid).cloned().unwrap_or_default(),
            tg.pairs, tg.model_pct(), tg.baseline_pct(), tg.margin(),
            if *h == 0 { "—" } else if tg.informative() { "yes" } else { "NO (distance ≥ 99 %)" },
            tg.hard_model_pct(), tg.hard_baseline_pct(),
            tg.dw_pairs, 100.0 * tg.dw_pairs as f64 / tg.all_pairs.max(1) as f64, tg.dw_model_pct(),
            rep.calib.ece, rep.calib.auc, rep.time_human.mae_s, rep.time_human.n
        ));
    }
    out.push('\n');
    let train_rows: Vec<&Rows> = rows.iter().zip(held).filter(|(_, h)| **h == 0).map(|(r, _)| r).collect();
    let held_rows: Vec<&Rows> = rows.iter().zip(held).filter(|(_, h)| **h > 0).map(|(r, _)| r).collect();
    let fnv_rows: Vec<&Rows> = rows.iter().zip(held).filter(|(_, h)| **h == 1).map(|(r, _)| r).collect();
    let forced_rows: Vec<&Rows> = rows.iter().zip(held).filter(|(_, h)| **h == 2).map(|(r, _)| r).collect();
    for (name, set_rows) in [("TRAIN maps", train_rows), ("HELD-OUT maps, all", held_rows), ("HELD-OUT maps, fnv rule", fnv_rows), ("HELD-OUT maps, forced (BAR.md 08:30Z)", forced_rows), ("HELD-OUT maps, INFORMATIVE (the gate)", informative_held)] {
        if set_rows.is_empty() {
            out.push_str(&format!("[{name}] none\n"));
            continue;
        }
        let set = Set::from_rows(&set_rows, keep);
        let pred = tmr::train::predict_all_candle(w, &set, dev).unwrap_or_else(|e| die(&e));
        let rep = tmr::eval::evaluate(&set, &pred, 7);
        out.push_str(&format!("[{name}] {} maps\n", set_rows.len()));
        out.push_str(&tmr::eval::render(name, &rep));
    }
}

fn cmd_train(args: &[String]) {
    let t_load = std::time::Instant::now();
    let (rows, held) = load_cache(args);
    eprintln!("load: {} maps in {:.1} s", rows.len(), t_load.elapsed().as_secs_f64());
    let t_names = std::time::Instant::now();
    let uids: Vec<String> = rows.iter().map(|r| r.map_uid.clone()).collect();
    let names = map_names_for(flag(args, "--cache").map(PathBuf::from).as_deref(), &uids);
    eprintln!("names: {} in {:.1} s", names.len(), t_names.elapsed().as_secs_f64());
    let mut cfg = TrainCfg::default();
    if let Some(a) = flag(args, "--ablation") {
        cfg.ablation = a;
    }
    let fv = rows.first().map_or(1, |r| r.fv);
    let keep = tmr::feat::ablation_keep(fv, &cfg.ablation).unwrap_or_else(|| die(&format!("--ablation {}: unknown for feature version {fv}", cfg.ablation)));
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
    if let Some(v) = flag(args, "--noise") {
        cfg.noise = v.parse().unwrap_or_else(|_| die("--noise σ"));
    }
    if let Some(v) = flag(args, "--geo-dropout") {
        cfg.geo_drop = v.parse().unwrap_or_else(|_| die("--geo-dropout p"));
    }
    if let Some(v) = flag(args, "--dropout") {
        cfg.dropout = v.parse().unwrap_or_else(|_| die("--dropout p"));
    }
    if let Some(w) = flag(args, "--wd") {
        cfg.weight_decay = w.parse().unwrap_or_else(|_| die("--wd X"));
    }
    if let Some(p) = flag(args, "--patience") {
        cfg.patience = p.parse().unwrap_or_else(|_| die("--patience N"));
    }
    let out = PathBuf::from(flag(args, "--out").unwrap_or_else(|| die("--out r.tmw")));
    let dev = candle_core::Device::Cpu;
    let mut report = format!("# tmr train — {}\n\n## Split (by MAP: fnv1a64(uid) % 10 == 0 held out; --held-out adds {:?})\n{}\n", provenance("train"), flag(args, "--held-out").unwrap_or_default(), split_summary(&rows, &held, &names));
    let train_rows: Vec<&Rows> = rows.iter().zip(&held).filter(|(_, h)| **h == 0).map(|(r, _)| r).collect();
    if train_rows.is_empty() {
        die("every map is held out — nothing to train on");
    }
    let mirror = !has(args, "--no-mirror");
    let t_set = std::time::Instant::now();
    let train_set = Set::from_rows_aug(&train_rows, &keep, mirror);
    eprintln!("set: {} rows in {:.1} s", train_set.n, t_set.elapsed().as_secs_f64());
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
        "kind": kind_of(args),
        "mirror_augmentation": mirror,
        "noise": cfg.noise, "dropout": cfg.dropout, "geo_drop": cfg.geo_drop, "weight_decay": cfg.weight_decay, "lr": cfg.lr, "hidden_cfg": cfg.hidden,
        "hidden": cfg.hidden,
        "epochs_run": rep.epochs_run,
        "best_epoch": rep.best_epoch,
        "best_val_loss": rep.best_val,
        "train_maps": train_rows.iter().map(|r| r.map_uid.clone()).collect::<Vec<_>>(),
        "held_out_maps": rows.iter().zip(&held).filter(|(_, h)| **h > 0).map(|(r, _)| r.map_uid.clone()).collect::<Vec<_>>(),
        "held_out_forced": rows.iter().zip(&held).filter(|(_, h)| **h == 2).map(|(r, _)| r.map_uid.clone()).collect::<Vec<_>>(),
        "train_rows": train_set.n,
        "h_min": h_min, "h_max": h_max,
        "feature_version": fv,
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
    let t_ev = std::time::Instant::now();
    let mut ev = String::new();
    eval_sets(&w, &rows, &held, &keep, &dev, &names, &mut ev);
    eprintln!("eval: {:.1} s", t_ev.elapsed().as_secs_f64());
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
    let keep = tmr::feat::ablation_keep(w.fv, &abl).unwrap_or_else(|| die("model meta names an unknown ablation"));
    let (rows, held) = load_cache(args);
    let uids: Vec<String> = rows.iter().map(|r| r.map_uid.clone()).collect();
    let names = map_names_for(flag(args, "--cache").map(PathBuf::from).as_deref(), &uids);
    let dev = candle_core::Device::Cpu;
    let mut report = format!("# tmr eval — {} — model {} ({}), ablation {}\n\n## Split\n{}\n", provenance("eval"), model.display(), meta.get("produced_by").and_then(|p| p.as_str()).unwrap_or("?"), abl, split_summary(&rows, &held, &names));
    eval_sets(&w, &rows, &held, &keep, &dev, &names, &mut report);
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
    println!("model {}: dims {:?}, {} params, feature version {}, meta {}", model.display(), w.dims, w.n_params(), w.fv, w.meta);
    if fails > 0 {
        std::process::exit(1);
    }
}

struct PlanCtx {
    gates: tmroute::gates::GatesFile,
    surf: tmplan::surface::SurfaceModel,
    nodes: tmplan::surface::Nodes,
    w: Weights,
    h_min: u16,
    h_max: u16,
    keep: Vec<&'static str>,
    /// Geometry for the gate model's feature version (the surface model doubles as v1's).
    geo: Geometry,
    /// Geometry for the local model (`--local`), when its feature version differs.
    geo_local: Option<Geometry>,
    local_w: Option<Weights>,
}

fn load_plan_ctx(args: &[String]) -> PlanCtx {
    let map = args.iter().find(|a| a.ends_with(".Map.Gbx")).cloned().unwrap_or_else(|| die("MAP.Map.Gbx required"));
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates gates.json required"));
    let gates = tmroute::io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let model = PathBuf::from(flag(args, "--model").unwrap_or_else(|| die("--model r.tmw")));
    let w = Weights::load(&model).unwrap_or_else(|e| die(&e));
    let meta: serde_json::Value = serde_json::from_str(&w.meta).unwrap_or(serde_json::Value::Null);
    let abl = meta.get("ablation").and_then(|a| a.as_str()).unwrap_or("full").to_string();
    let keep = tmr::feat::ablation_keep(w.fv, &abl).unwrap_or_else(|| die("model meta names an unknown ablation"));
    let h_max = meta.get("h_max").and_then(|v| v.as_f64()).unwrap_or(400.0) as u16;
    let h_min = meta.get("h_min").and_then(|v| v.as_f64()).unwrap_or(200.0) as u16;
    let (surf, nodes) = tmplan::surface::SurfaceModel::build(Path::new(&map), &gates, !has(args, "--quiet"), flag(args, "--grid").map_or(false, |g| g == "deco")).unwrap_or_else(|e| die(&e));
    for n in &surf.notes {
        println!("  note: {n}");
    }
    let quiet = has(args, "--quiet");
    let geo = if w.fv == 2 { build_geometry(2, Path::new(&map), &gates, !quiet).unwrap_or_else(|e| die(&e)) } else { Geometry::None };
    let local_w = flag(args, "--local").map(|p| Weights::load(Path::new(&p)).unwrap_or_else(|e| die(&e)));
    let geo_local = match &local_w {
        Some(lw) if lw.fv != w.fv => Some(if lw.fv == 2 { build_geometry(2, Path::new(&map), &gates, !quiet).unwrap_or_else(|e| die(&e)) } else { Geometry::None }),
        _ => None,
    };
    PlanCtx { gates, surf, nodes, w, h_min, h_max, keep, geo, geo_local, local_w }
}

impl PlanCtx {
    /// The featurizer for the gate model.
    fn feat(&self) -> tmr::feat::Featurizer<'_> {
        match &self.geo {
            Geometry::None => tmr::feat::Featurizer::V1(features::Probe { idx: Some(&self.surf.full), road: &self.surf.road_materials }),
            g => featurizer(g),
        }
    }
    fn feat_local(&self) -> tmr::feat::Featurizer<'_> {
        match &self.geo_local {
            Some(Geometry::None) => tmr::feat::Featurizer::V1(features::Probe { idx: Some(&self.surf.full), road: &self.surf.road_materials }),
            Some(g) => featurizer(g),
            None => self.feat(),
        }
    }
}

fn order_str(nodes: &tmplan::surface::Nodes, gates: &tmroute::gates::GatesFile, visit: &[usize]) -> (String, String) {
    let groups: Vec<String> = visit.iter().skip(1).map(|&n| nodes.groups[n].to_string()).collect();
    let wps: Vec<String> = visit.iter().skip(1).map(|&n| gates.group_rep(nodes.groups[n]).unwrap().waypoint.to_string()).collect();
    (groups.join(","), wps.join(","))
}

fn cmd_plan(args: &[String]) {
    use tmplan::estimator::{EdgeEstimator, StateBucket};
    let ctx = load_plan_ctx(args);
    let PlanCtx { gates, surf, nodes, w, h_min, h_max, keep, .. } = &ctx;
    let (h_min, h_max, keep) = (*h_min, *h_max, keep.clone());
    let feat = ctx.feat();
    let est = tmr::estimator::REstimator { w, feat: &feat, gates, nodes, surf, h_max, h_min, keep, p_floor: flag(args, "--p-floor").and_then(|s| s.parse().ok()).unwrap_or(0.02) };
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
    // --estimator chained (needs --local rl.tmw): the horizon-native chain prices every edge; the gate head stays the prior
    let feat_local = ctx.feat_local();
    let chained = if flag(args, "--estimator").as_deref() == Some("chained") {
        let lw = ctx.local_w.as_ref().unwrap_or_else(|| die("--estimator chained needs --local rl.tmw"));
        let mut c = tmr::estimator::Chained::new(lw, &feat_local, gates, nodes, surf, tmr::feat::ablation_keep(lw.fv, "full").unwrap());
        if let Some(b) = flag(args, "--chain-beam") { c.beam = b.parse().unwrap_or(24); }
        if let Some(f) = flag(args, "--p-step") { c.p_step_floor = f.parse().unwrap_or(0.05); }
        if let Some(p) = flag(args, "--penalty") { c.penalty_ms = p.parse().unwrap_or(3000.0); }
        Some(c)
    } else {
        None
    };
    // --estimator hybrid (GEOM arm, coordinator 14:52Z): geometric on road legs, R's chain where the surface graph
    // has no path or a detour (tmplan::estimator::Hybrid); needs --local
    let hybrid_on = flag(args, "--estimator").as_deref() == Some("hybrid");
    let (d_m, len_m, drop_m, fields_m) = surf.distance_matrix_full(nodes);
    let dirs_m = surf.directions(nodes, &fields_m);
    let leg_specials = surf.leg_specials(nodes, &fields_m, gates);
    let geo = tmplan::estimator::Geometric { time_model: tmplan::estimator::TimeModel::Cost, d: &d_m, len: &len_m, nodes, flight: None, surface: Some(surf), dirs: Some(&dirs_m), drop: Some(&drop_m), drop_penalty: 0.0, specials: Some((&leg_specials, gates)) };
    let chained_h = if hybrid_on {
        let lw = ctx.local_w.as_ref().unwrap_or_else(|| die("--estimator hybrid needs --local rl.tmw"));
        let mut c = tmr::estimator::Chained::new(lw, &feat_local, gates, nodes, surf, tmr::feat::ablation_keep(lw.fv, "full").unwrap());
        if let Some(b) = flag(args, "--chain-beam") { c.beam = b.parse().unwrap_or(24); }
        if let Some(f) = flag(args, "--p-step") { c.p_step_floor = f.parse().unwrap_or(0.05); }
        if let Some(p) = flag(args, "--penalty") { c.penalty_ms = p.parse().unwrap_or(3000.0); }
        Some(c)
    } else { None };
    let hybrid = chained_h.as_ref().map(|c| tmplan::estimator::Hybrid { geo: &geo, learned: c, detour_ratio: flag(args, "--detour").and_then(|s| s.parse().ok()).unwrap_or(2.0), nodes, len: &len_m, counts: std::cell::Cell::new((0, 0)) });
    let est_dyn: &dyn EdgeEstimator = match (&hybrid, &chained) {
        (Some(h), _) => h,
        (None, Some(c)) => c,
        (None, None) => &est,
    };
    let t0 = std::time::Instant::now();
    let plans = tmplan::planner::beam(nodes, est_dyn, width, top_k, StateBucket::of_speed(0.0));
    println!("{}\t{}\tcp_groups {}\tfinish_groups {}\testimator {}\tbeam {}\tplans {}\t{:.1} s", gates.map_name, gates.map_uid, nodes.n_cp, nodes.n_fin, est_dyn.name(), width, plans.len(), t0.elapsed().as_secs_f64());
    if let Some(h) = &hybrid {
        let (ng, nr) = h.counts.get();
        println!("  hybrid pricing: {ng} edge queries geometric, {nr} learned");
    }
    let prov = provenance("plan");
    let out_dir = flag(args, "--out-dir");
    let (_d, _len, _drop, fields) = surf.distance_matrix_full(&nodes);
    for (k, p) in plans.iter().enumerate() {
        let (g, wp) = order_str(&nodes, &gates, &p.visit);
        let legs: Vec<String> = p.edges.iter().map(|e| format!("{:.2}@{}{}", e.p_reach, tmr::secs(e.expected_ms as i64), match e.kind { tmplan::estimator::EdgeKind::Learned => "R", tmplan::estimator::EdgeKind::Flight => "F", _ => "" })).collect();
        println!("  rank {k}: predicted {}  P(reach) {:.3}  groups [{}]  waypoints [{}]  legs p@t [{}]", tmr::secs(p.total_ms as i64), p.p_reach, g, wp, legs.join(" "));
        if let Some(dir) = &out_dir {
            let source = flag(args, "--source").unwrap_or_else(|| if hybrid_on { "router-plan-hyb".into() } else { "router-plan-r".into() });
            let mut route = tmplan::export::export(gates, nodes, surf, &fields, p, k as u32, &est_dyn.name(), &prov);
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
    let ctx = load_plan_ctx(args);
    let PlanCtx { gates, surf, nodes, w, h_min, h_max, keep, local_w, .. } = &ctx;
    let (h_min, h_max, keep) = (*h_min, *h_max, keep.clone());
    let feat = ctx.feat();
    let feat_local = ctx.feat_local();
    let est = tmr::estimator::REstimator { w, feat: &feat, gates, nodes, surf, h_max, h_min, keep, p_floor: 0.0 };
    let chained = local_w.as_ref().map(|lw| {
        let mut c = tmr::estimator::Chained::new(lw, &feat_local, gates, nodes, surf, tmr::feat::ablation_keep(lw.fv, "full").unwrap());
        if let Some(b) = flag(args, "--beam") { c.beam = b.parse().unwrap_or(24); }
        if let Some(f) = flag(args, "--p-step") { c.p_step_floor = f.parse().unwrap_or(0.05); }
        if let Some(p) = flag(args, "--penalty") { c.penalty_ms = p.parse().unwrap_or(3000.0); }
        c
    });
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
        let (mut chained_total, mut chained_fail) = (0i32, 0usize);
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
            let chained_line = match &chained {
                Some(c) => {
                    let dir = est.heading_public(prev, at);
                    match c.chain(nodes.pos[at], bucket.speed(), dir, to) {
                        Some((p, ticks, v, steps, path)) => {
                            chained_total += ticks * 10;
                            if has(args, "--trace") {
                                println!("      chained path: {}", path.iter().map(|q| format!("({:.0},{:.0},{:.0})", q[0], q[1], q[2])).collect::<Vec<_>>().join(" → "));
                            }
                            format!("; CHAINED p {:.3} time {} ({} steps, arrives {:.0} m/s)", p, tmr::secs(ticks as i64 * 10), steps, v)
                        }
                        None => {
                            chained_fail += 1;
                            "; CHAINED: no path above p_step floor".to_string()
                        }
                    }
                }
                None => String::new(),
            };
            match mine {
                Some((_, p, t, h)) => {
                    total_ms += (*t * 10.0) as i32;
                    println!(
                        "  leg {k}: node {at} → {to} (group {g}): p_reach {:.3}, expected {}, h {}; human {}; R ranks it #{rank} of {} [{}]{chained_line}",
                        p,
                        tmr::secs((*t * 10.0) as i64),
                        h,
                        human_leg.map(|m| tmr::secs(m as i64)).unwrap_or("?".into()),
                        scored.len(),
                        scored.iter().map(|(c, p, _, _)| format!("{c}:{p:.2}")).collect::<Vec<_>>().join(" ")
                    );
                    // the next leg starts at the arrival speed: the chained one when chaining, else the gate head's
                    bucket = StateBucket::of_speed(match &chained {
                        Some(c) => c.chain(nodes.pos[at], bucket.speed(), est.heading_public(prev, at), to).map(|(_, _, v, _, _)| v).unwrap_or(0.0),
                        None => est.query(bucket, prev, at, to).map(|(e, _, _)| e.speed_mu).unwrap_or(0.0).max(0.0),
                    });
                }
                None => println!("  leg {k}: node {at} → {to}: no estimate"),
            }
            prev = Some(at);
            at = to;
            visited.push(to);
        }
        println!("  R ranks the human's next gate first on {}/{} legs; Σ expected (single lookup) {} vs human {}{}", ranked_first, groups.len(), tmr::secs(total_ms as i64), cp_ms.last().map(|m| tmr::secs(*m as i64)).unwrap_or("?".into()), if chained.is_some() { format!("; Σ CHAINED {} ({} legs without a path)", tmr::secs(chained_total as i64), chained_fail) } else { String::new() });
    }
}

fn kind_of(args: &[String]) -> String {
    match flag(args, "--kind").as_deref() {
        None | Some("gate") => "gate".into(),
        Some("local") => "local".into(),
        Some(x) => die(&format!("--kind {x}: gate|local")),
    }
}

fn rows_file_name(uid: &str, kind: &str) -> String {
    if kind == "local" { format!("{uid}.local.rows") } else { format!("{uid}.rows") }
}

/// `tmr watch --reach DIR [--reach ..] --cache DIR --bank DIR [--interval S] [--once] [--epochs N] [--threads T]`
///
/// Every interval: find shard dirs whose samples.tmr is closed (header count == body) and
/// newer than the cached rows; rebuild their rows (gate + local); when anything changed,
/// retrain both heads, evaluate, and publish `bank/r-v<N>.tmw`, `bank/rl-v<N>.tmw`,
/// `bank/r-v<N>.md` (per-map table) and append a STATUS line. Never stops at a finished step.
fn cmd_watch(args: &[String]) {
    let mut o = build_opts(args);
    let bank = PathBuf::from(flag(args, "--bank").unwrap_or_else(|| die("--bank DIR")));
    // --max-rows-total N: the per-map cap shrinks as maps arrive so the training set stays ≤ N rows
    // (a v2 row is 2,692 f32 = 10.8 KB; 2.4 M rows ≈ 26 GB in RAM, twice that at the Set copy). A cap change
    // invalidates the cache (rows are re-subsampled), recorded in <cache>/CAP.
    let max_total: usize = flag(args, "--max-rows-total").and_then(|s| s.parse().ok()).unwrap_or(0);
    let base_cap = o.max_rows;
    std::fs::create_dir_all(&bank).unwrap_or_else(|e| die(&e.to_string()));
    let interval: u64 = flag(args, "--interval").and_then(|s| s.parse().ok()).unwrap_or(600);
    let once = has(args, "--once");
    let mut force_first = has(args, "--force-first");
    let epochs = flag(args, "--epochs").unwrap_or_else(|| "30".into());
    let threads = flag(args, "--threads").unwrap_or_else(|| "32".into());
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("tmr"));
    let mut version: u32 = std::fs::read_dir(&bank)
        .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_string_lossy().strip_prefix("r-v").and_then(|s| s.split('.').next().and_then(|n| n.parse::<u32>().ok()))).max().unwrap_or(0))
        .unwrap_or(0);
    loop {
        let mut changed = Vec::new();
        let mut log = Vec::new();
        let dirs = reach_dirs(args);
        if max_total > 0 && !dirs.is_empty() {
            let cap = (max_total / dirs.len()).min(if base_cap > 0 { base_cap } else { usize::MAX });
            let cap_file = o.out.join("CAP");
            let prev: usize = std::fs::read_to_string(&cap_file).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            if prev != cap {
                println!("watch: per-map row cap {prev} → {cap} ({} maps, total ≤ {max_total}): invalidating the row cache", dirs.len());
                if let Ok(rd) = std::fs::read_dir(&o.out) {
                    for e in rd.flatten() {
                        if e.file_name().to_string_lossy().ends_with(".rows") {
                            let _ = std::fs::remove_file(e.path());
                        }
                    }
                }
                let _ = std::fs::write(&cap_file, cap.to_string());
            }
            o.max_rows = cap;
        }
        for d in dirs {
            let Some(uid) = data::shard_map_uid(&d) else { continue };
            let shard = d.join("samples.tmr");
            let Ok(meta) = std::fs::metadata(&shard) else { continue };
            let rows_f = o.out.join(rows_file_name(&uid, "gate"));
            let rows_l = o.out.join(rows_file_name(&uid, "local"));
            let fresh = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok().zip(meta.modified().ok()).map_or(false, |(a, b)| a > b);
            if fresh(&rows_f) && fresh(&rows_l) {
                continue;
            }
            // closed shard? (header count == body length)
            if let Err(e) = tmreach::tmr::read_shard(&shard) {
                log.push(format!("  {uid}: {e} — not closed yet, waiting"));
                continue;
            }
            let mut ok = true;
            for kind in ["gate", "local"] {
                let ok_ = BuildOpts { kind: kind.into(), fv: o.fv, max_rows: o.max_rows, threads: o.threads, geom: o.geom.clone(), maps: o.maps.clone(), no_geom: o.no_geom, out: o.out.clone() };
                match build_one(&d, &ok_, &mut log) {
                    Ok((_, line)) => println!("{line}"),
                    Err(e) => {
                        println!("  {uid} [{kind}]: {e} — will retry next interval");
                        ok = false;
                    }
                }
            }
            if ok {
                changed.push(uid);
            }
        }
        for l in &log {
            println!("{l}");
        }
        if force_first && changed.is_empty() {
            changed.push("(forced retrain: --force-first)".into());
        }
        force_first = false;
        if !changed.is_empty() {
            version += 1;
            let stamp = now_utc();
            println!("watch: {} map(s) changed ({}), training v{version} at {stamp}", changed.len(), changed.join(","));
            let mut summary = format!("## {stamp} — tmr watch v{version}: rebuilt {} map(s) [{}]\n", changed.len(), changed.join(", "));
            // variants: plain, and (when --geo-dropout p is given) the geometry block-dropout A/B
            let mut variants: Vec<(String, Vec<String>)> = vec![(String::new(), vec![])];
            if let Some(p) = flag(args, "--geo-dropout") {
                variants.push(("-gd".into(), vec!["--geo-dropout".into(), p]));
            }
            // the variants of a kind run CONCURRENTLY (a candle CPU training uses ~4 cores whatever the thread
            // count: the matmuls parallelise, the rest does not), the kinds one after the other (RAM).
            for (kind, prefix) in [("gate", "r"), ("local", "rl")] {
            let mut children: Vec<(String, PathBuf, std::process::Child)> = Vec::new();
            for (suffix, extra) in &variants {
                let model = bank.join(format!("{prefix}-v{version}{suffix}.tmw"));
                let report = bank.join(format!("{prefix}-v{version}{suffix}.md"));
                let mut cmd = std::process::Command::new(&exe);
                cmd.args(["train", "--kind", kind, "--cache"]).arg(&o.out).arg("--out").arg(&model).arg("--report").arg(&report).args(["--epochs", &epochs, "--threads", &threads]).args(extra);
                if let Some(h) = flag(args, "--held-out") {
                    cmd.args(["--held-out", &h]);
                }
                for pass in ["--batch", "--lr", "--wd", "--hidden", "--patience"] {
                    if let Some(v) = flag(args, pass) {
                        cmd.args([pass, &v]);
                    }
                }
                if has(args, "--no-mirror") {
                    cmd.arg("--no-mirror");
                }
                cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
                match cmd.spawn() {
                    Ok(ch) => children.push((suffix.clone(), model, ch)),
                    Err(e) => summary.push_str(&format!("[{kind}{suffix}] train could not start: {e}\n")),
                }
            }
            for (suffix, model, ch) in children {
                match ch.wait_with_output() {
                    Ok(out) => {
                        let s = String::from_utf8_lossy(&out.stdout).to_string();
                        for line in s.lines().filter(|l| l.contains("two-gate:") || l.starts_with("| ")) {
                            summary.push_str(&format!("[{kind}{suffix}] {line}\n"));
                        }
                        if !out.status.success() {
                            summary.push_str(&format!("[{kind}{suffix}] train FAILED: {}\n", String::from_utf8_lossy(&out.stderr)));
                        }
                    }
                    Err(e) => summary.push_str(&format!("[{kind}{suffix}] train failed to finish: {e}\n")),
                }
                let _ = std::fs::copy(&model, bank.join(format!("{prefix}-latest{suffix}.tmw")));
            }
            }
            print!("{summary}");
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(bank.join("WATCH.md")) {
                let _ = f.write_all(summary.as_bytes());
                let _ = f.write_all(b"\n");
            }
        } else {
            println!("watch: nothing new at {}", now_utc());
        }
        if once {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(interval));
    }
}

/// Geometry for a map at a feature version: v1 = tmplan's SurfaceModel (plumb index), v2 = GEOM's LocalScene.
enum Geometry {
    None,
    V1(tmplan::surface::SurfaceModel),
    V2 { scene: mapgeom::local::LocalScene, map: tmmaps::map::MapFile, yoff: f32 },
}

fn open_store() -> Result<mapgeom::store::DataStore, String> {
    let server = std::env::var("TM_SERVER").map_err(|_| "TM_SERVER not set")?;
    let mut paths = Vec::new();
    for name in ["dedicated_TMStadium.pak", "dedicated.pak", "resource.pak"] {
        let p = format!("{server}/Packs/{name}");
        if Path::new(&p).exists() {
            paths.push(p);
        }
    }
    if paths.is_empty() {
        return Err(format!("no .pak in {server}/Packs"));
    }
    mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY)
}

fn build_geometry(fv: u32, map: &Path, gates: &tmroute::gates::GatesFile, verbose: bool) -> Result<Geometry, String> {
    match fv {
        1 => {
            let (s, _nodes) = tmplan::surface::SurfaceModel::build(map, gates, verbose, false)?;
            if s.full.triangle_count() < 1000 {
                return Err("SurfaceModel plumb index is empty — pak or map missing; refusing to build features on it".into());
            }
            Ok(Geometry::V1(s))
        }
        2 => {
            let mut store = open_store()?;
            let m = tmmaps::map::MapFile::load(map);
            let t0 = std::time::Instant::now();
            let scene = mapgeom::local::LocalScene::build(&mut store, &m, gates.yoff, &mapgeom::local::BuildOpts::default());
            if scene.tris.len() < 1000 {
                return Err(format!("LocalScene has only {} triangles — the pak or the map is not what it should be (a wiped /tmp/tmp/server reads as an empty scene); refusing to build features on it", scene.tris.len()));
            }
            if verbose {
                eprintln!("  LocalScene: {} triangles, {} placements, yoff {} in {:.1} s", scene.tris.len(), scene.placements.len(), gates.yoff, t0.elapsed().as_secs_f64());
            }
            Ok(Geometry::V2 { scene, map: m, yoff: gates.yoff })
        }
        _ => Err(format!("feature version {fv}")),
    }
}

fn featurizer<'a>(g: &'a Geometry) -> tmr::feat::Featurizer<'a> {
    match g {
        Geometry::None => tmr::feat::Featurizer::V1(features::Probe::none()),
        Geometry::V1(s) => tmr::feat::Featurizer::V1(features::Probe { idx: Some(&s.full), road: &s.road_materials }),
        Geometry::V2 { scene, map, yoff } => tmr::feat::Featurizer::V2(tmr::features2::Geo2::new(scene, map, *yoff)),
    }
}

fn fv_of(args: &[String]) -> u32 {
    flag(args, "--fv").and_then(|s| s.parse().ok()).unwrap_or(1)
}

/// `tmr probe --fv 2 MAP.Map.Gbx --gates gates.json --starts starts.tsv [--n 3]`: print the v2 geometry
/// blocks for a few real start states — the eyeball control on the featurizer.
fn cmd_probe(args: &[String]) {
    use tmr::features2::*;
    let map = args.iter().find(|a| a.ends_with(".Map.Gbx")).cloned().unwrap_or_else(|| die("MAP.Map.Gbx required"));
    let gates = tmroute::io::read_gates(Path::new(&flag(args, "--gates").unwrap_or_else(|| die("--gates")))).unwrap_or_else(|e| die(&e));
    let starts = data::read_starts(Path::new(&flag(args, "--starts").unwrap_or_else(|| die("--starts")))).unwrap_or_else(|e| die(&e));
    let n: usize = flag(args, "--n").and_then(|s| s.parse().ok()).unwrap_or(3);
    let geo = build_geometry(2, Path::new(&map), &gates, true).unwrap_or_else(|e| die(&e));
    let feat = featurizer(&geo);
    if let Geometry::V2 { scene, .. } = &geo {
        let g2 = tmr::features2::Geo2::new(scene, match &geo { Geometry::V2 { map, .. } => map, _ => unreachable!() }, gates.yoff);
        println!("obstacles (collidable items ≤ 60 m radius): {}; block cells: {}", g2.obstacles.len(), g2.cells.len());
    }
    let mut ids: Vec<&u32> = starts.keys().collect();
    ids.sort();
    let step = (ids.len() / n.max(1)).max(1);
    let g0 = gates.gates.iter().find(|g| g.kind != WpKind::Start).unwrap();
    let t = tmr::feat::TargetSpec { centre: g0.centre, normal: g0.normal, half_width: g0.half_width, group_size: 1, kind: TargetKind::Checkpoint, collected_share: 0.0 };
    let mut x = vec![0f32; feat.dim()];
    for &id in ids.iter().step_by(step).take(n) {
        let s = &starts[id];
        let t0 = std::time::Instant::now();
        feat.fill(&s.state, &t, 200, &mut x);
        let us = t0.elapsed().as_micros();
        println!("start {id} race {} pos ({:.0},{:.1},{:.0}) speed {:.1} m/s — features in {us} µs", tmr::secs(s.race_ms as i64), s.state.pos[0], s.state.pos[1], s.state.pos[2], s.state.speed);
        // path: centre lateral
        let mut line = String::from("  path (centre): ");
        for k in 0..PATH_N {
            let o = OFF2_PATH + (k * PATH_LAT.len() + 1) * PER_SAMPLE;
            let hit = x[o + 6] > 0.5;
            let fam = (0..N_FAM).find(|f| x[o + 7 + N_MAT + f] > 0.5).unwrap_or(7);
            line.push_str(&format!("[{} h{:+.1} n·y{:.2} fam{}] ", if hit { "hit" } else { "GAP" }, x[o + 3] * 20.0, x[o + 1], fam));
        }
        println!("{line}");
        for (pi, pitch) in RAY_PITCH_DEG.iter().enumerate() {
            let mut line = format!("  rays pitch {:+3.0}: ", pitch);
            for yi in 0..RAY_YAWS_DEG.len() {
                let o = OFF2_RAYS + (yi * RAY_PITCH_DEG.len() + pi) * PER_RAY;
                if x[o + 1] > 0.5 {
                    let fam = (0..N_FAM).find(|f| x[o + 3 + N_MAT + f] > 0.5).unwrap_or(7);
                    line.push_str(&format!("{:>4.0}m/f{fam} ", x[o] * RAY_MAX_M));
                } else {
                    line.push_str("   --    ");
                }
            }
            println!("{line}");
        }
        let nob = (0..N_OBST).filter(|k| x[OFF2_OBST + k * (4 + N_FAM) + 3] > 0.0).count();
        let cells_known = (0..(CELLS.0 * CELLS.1 * CELLS.2) as usize).filter(|c| x[OFF2_CELLS + c * N_FAM + 7] < 0.5).count();
        println!("  obstacles within 80 m: {nob}; cells with a block: {cells_known}/75; target rel ({:.0},{:.0},{:.0}) m", x[OFF2_TARGET] * 100.0, x[OFF2_TARGET + 1] * 100.0, x[OFF2_TARGET + 2] * 100.0);
    }
}

/// `tmr report --bank DIR [--bank DIR2 ..] --out REPORT.md`: one table per bank from its r-v*/rl-v* reports —
/// version, variant, train/held-out map counts, per held-out map (R, distance, margin, informative), the pooled
/// INFORMATIVE margin, held-out ECE/AUC. The coordinator's view of the watchers.
fn cmd_report(args: &[String]) {
    let banks: Vec<PathBuf> = args.iter().enumerate().filter(|(_, a)| *a == "--bank").filter_map(|(i, _)| args.get(i + 1).map(PathBuf::from)).collect();
    let out = flag(args, "--out");
    let mut s = format!("# tmr report — {}\n\n", provenance("report"));
    for bank in banks {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&bank).map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |x| x == "md") && p.file_name().map_or(false, |n| n.to_string_lossy().starts_with("r"))).collect()).unwrap_or_default();
        files.sort_by_key(|p| {
            let n = p.file_stem().unwrap().to_string_lossy().to_string();
            let v: u32 = n.split("-v").nth(1).and_then(|x| x.split('-').next()).and_then(|x| x.parse().ok()).unwrap_or(0);
            (v, n)
        });
        s.push_str(&format!("## {}\n\n| report | train maps | held-out maps | per held-out map (R, distance) margin [informative] | pooled INFORMATIVE (R, distance) margin | held-out ECE / AUC | epochs / wall |\n|---|---|---|---|---|---|---|\n", bank.display()));
        for f in files {
            let Ok(t) = std::fs::read_to_string(&f) else { continue };
            let name = f.file_stem().unwrap().to_string_lossy().to_string();
            if name == "WATCH" {
                continue;
            }
            let train = t.lines().filter(|l| l.starts_with("| train |")).count();
            let held: Vec<String> = t
                .lines()
                .filter(|l| l.starts_with("| HELD-OUT"))
                .map(|l| {
                    let c: Vec<&str> = l.split('|').map(|x| x.trim()).collect();
                    // | split | map | pairs | R | dist | margin | informative | ...
                    let map = c.get(2).map(|m| m.split_whitespace().skip(1).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                    let tag = c.get(1).map(|x| x.replace("HELD-OUT", "").trim().to_string()).unwrap_or_default();
                    format!("{}{}: ({}, {}) {} [{}]", if tag.is_empty() { String::new() } else { format!("{tag} ") }, if map.is_empty() { c.get(2).cloned().unwrap_or("?").to_string() } else { map }, c.get(4).unwrap_or(&"?"), c.get(5).unwrap_or(&"?"), c.get(6).unwrap_or(&"?"), c.get(7).map(|x| if x.starts_with("NO") { "uninformative" } else { x }).unwrap_or("?"))
                })
                .collect();
            let pooled = t.lines().find(|l| l.starts_with("[HELD-OUT maps, INFORMATIVE") && l.contains("two-gate")).map(|l| {
                let a = l.find("(R ").map(|i| &l[i..]).unwrap_or("");
                a.split(';').next().unwrap_or("").to_string()
            }).unwrap_or_else(|| if t.contains("[HELD-OUT maps, INFORMATIVE (the gate)] none") { "none".into() } else { "?".into() });
            let cal = t.lines().find(|l| l.starts_with("[HELD-OUT maps, all] calibration")).map(|l| {
                let ece = l.split("ECE ").nth(1).and_then(|x| x.split(',').next()).unwrap_or("?");
                let auc = l.split("AUC ").nth(1).unwrap_or("?");
                format!("{ece} / {auc}")
            }).unwrap_or_else(|| "—".into());
            let ep = t.lines().find(|l| l.starts_with("best epoch")).map(|l| l.replace("best epoch ", "best ").to_string()).unwrap_or_default();
            s.push_str(&format!("| {name} | {train} | {} | {} | {} | {} | {} |\n", held.len(), if held.is_empty() { "—".to_string() } else { held.join("<br>") }, pooled, cal, ep));
        }
        s.push('\n');
    }
    print!("{s}");
    if let Some(o) = out {
        std::fs::write(&o, &s).unwrap_or_else(|e| die(&e.to_string()));
    }
}
