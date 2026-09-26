//! lmperf — the build-level perf driver for `lmtool` (perf engineer 6). Every recipe that used to be a
//! shell line lives here as a subcommand:
//!
//! * `lmperf identity --base BIN --new BIN [--dir /tmp/pd] [--checks g8,detp1,ref,tref,prb] [--regen]`
//!   The bit-identity harness: the five recipes (the giant's 8-direction file, tiny 16's 12-direction
//!   file, the pwc-day 9-direction pass dump, the tiled tiny-16 lightsum dump, the transcribed probe
//!   volumes) baked with BASE (cached under DIR/id/<base name>/) and with NEW, every output compared
//!   byte for byte. Exit 1 on any difference. The mh/harness-refs files are cut on an older base, so
//!   the reference is always a same-source build of the current base.
//! * `lmperf bench --bin A[,B,…] [--runs 2] [--threads 128] [--map g23.Map.Gbx] [--max-dirs 4] [--quality 4] [--dir D] [-- extra…]`
//!   The A/B instrument: the binaries interleaved (A B A B …), the `profile [sweep 0]:` line parsed,
//!   raster / directions total / sweep total per run, min and median per binary.
//! * `lmperf hangloop --bin A[,B] --runs N --timeout S [--dir D] [--threads T] [--logs DIR] -- <lmtool args…>`
//!   The hang hunter: the given lmtool command line N times per binary, interleaved, each run under a wall-clock
//!   timeout; a run that outlives it has its threads' stacks taken (eu-stack), is killed and counted as HUNG; per
//!   binary completed / hung / failed and the walls (engineer P, the pool hang of 2026-09-26).
//! * `lmperf perfstat --bin BIN [--events E] [--threads 128] [--map …] [--max-dirs 4] [--dir D] [-- extra…]`
//!   `perf stat` around the bench bake (IPC, cache misses, DTLB, page faults) plus the raster's visit
//!   counters (LMTOOL_RASTER_STATS) → misses per visit.
//! * `lmperf pgo --tools DIR [--target-cpu znver4] [--dir /tmp/pd] [--profdata PATH] [--out BIN] [--llvm-profdata PATH]`
//!   Profile-guided build: instrumented build, the training bakes (the giant's 4 directions, tiny 16's 6),
//!   `llvm-profdata merge`, the optimised build with the profile.
//! * `lmperf cpu` — what this host's CPU is (the target-cpu the shipped build wants, AVX-512 present or not).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(|s| s.as_str()) {
        Some("identity") => identity(&args[1..]),
        Some("bench") => bench(&args[1..]),
        Some("hangloop") => hangloop(&args[1..]),
        Some("perfstat") => perfstat(&args[1..]),
        Some("pgo") => pgo(&args[1..]),
        Some("concurrent") => concurrent(&args[1..]),
        Some("smt-probe") => smt_probe(&args[1..]),
        Some("scale-probe") => scale_probe(&args[1..]),
        Some("hog") => hog(&args[1..]),
        Some("cpu") => { println!("{}", cpu_report()); 0 }
        _ => {
            eprintln!("usage: lmperf identity|bench|hangloop|perfstat|pgo|concurrent|cpu … (see the module doc in tools/lmperf/src/main.rs)");
            2
        }
    };
    std::process::exit(code);
}

/// `--flag value` and `--flag` lookups over an argument list; `--` starts the extra arguments.
struct Opts {
    kv: BTreeMap<String, String>,
    flags: Vec<String>,
    extra: Vec<String>,
}

fn parse(args: &[String], value_flags: &[&str]) -> Opts {
    let mut o = Opts { kv: BTreeMap::new(), flags: Vec::new(), extra: Vec::new() };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--" {
            o.extra.extend(args[i + 1..].iter().cloned());
            break;
        }
        if let Some(name) = a.strip_prefix("--") {
            if value_flags.contains(&name) {
                let v = args.get(i + 1).unwrap_or_else(|| { eprintln!("--{name} needs a value"); std::process::exit(2) });
                o.kv.insert(name.to_string(), v.clone());
                i += 2;
                continue;
            }
            o.flags.push(name.to_string());
        } else {
            eprintln!("unexpected argument {a}");
            std::process::exit(2);
        }
        i += 1;
    }
    o
}

impl Opts {
    fn get(&self, k: &str) -> Option<&str> { self.kv.get(k).map(|s| s.as_str()) }
    fn get_or<'a>(&'a self, k: &str, d: &'a str) -> &'a str { self.get(k).unwrap_or(d) }
    fn has(&self, k: &str) -> bool { self.flags.iter().any(|f| f == k) }
    fn need(&self, k: &str) -> &str { self.get(k).unwrap_or_else(|| { eprintln!("--{k} is required"); std::process::exit(2) }) }
}

fn abs(p: &str) -> PathBuf {
    let p = PathBuf::from(p);
    if p.is_absolute() { p } else { std::env::current_dir().unwrap().join(p) }
}

/// Runs a command in `dir` with extra environment, capturing stderr (stdout is inherited unless quiet);
/// returns (success, stderr, wall seconds).
fn run_in(dir: &Path, env: &[(&str, &str)], prog: &Path, args: &[String], quiet: bool) -> (bool, String, f64) {
    let t = Instant::now();
    let mut c = Command::new(prog);
    c.args(args).current_dir(dir).stderr(Stdio::piped());
    if quiet { c.stdout(Stdio::null()); }
    for (k, v) in env { c.env(k, v); }
    let out = match c.output() {
        Ok(o) => o,
        Err(e) => { eprintln!("cannot run {}: {e}", prog.display()); return (false, String::new(), 0.0); }
    };
    (out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned(), t.elapsed().as_secs_f64())
}

fn common_args(dir: &Path) -> Vec<String> {
    let s = std::fs::read_to_string(dir.join("COMMON.txt")).unwrap_or_else(|e| { eprintln!("{}/COMMON.txt: {e}", dir.display()); std::process::exit(2) });
    s.split_whitespace().map(|x| x.to_string()).collect()
}

fn sv(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

// ---------------------------------------------------------------------------------------------------
// identity

/// One recipe: the bake's arguments (relative to DIR), its environment, and the output to compare —
/// a file or a directory — named `out` inside the per-binary result directory.
struct Recipe {
    name: &'static str,
    env: Vec<(&'static str, String)>,
    /// The arguments, with `{OUT}` standing for the result path inside the result directory.
    args: Vec<String>,
    out: &'static str,
}

fn recipes(dir: &Path, threads: &str) -> Vec<Recipe> {
    let common = common_args(dir);
    let t = |s: &str| s.to_string();
    vec![
        Recipe { name: "g8", env: vec![("LMTOOL_BAKE_TIME", t("1790000000")), ("LMTOOL_THREADS", t(threads))], args: sv(&["bake", "g23.Map.Gbx", "--raster", "--quality", "4", "--game-peel", "--max-dirs", "8", "--out", "{OUT}"]), out: "g8.Map.Gbx" },
        Recipe { name: "detp1", env: vec![("LMTOOL_BAKE_TIME", t("1790000000"))], args: sv(&["bake", "t16.Map.Gbx", "--raster", "--quality", "4", "--game-peel", "--max-dirs", "12", "--out", "{OUT}"]), out: "detp1.Map.Gbx" },
        Recipe { name: "ref", env: vec![], args: { let mut a = vec![t("bake")]; a.extend(common.iter().cloned()); a.extend(sv(&["--dump-passes", "{OUT}", "--out", "{OUT}.Map.Gbx"])); a }, out: "dump-ref" },
        Recipe { name: "tref", env: vec![], args: sv(&["bake", "t16.Map.Gbx", "--raster", "--quality", "4", "--game-peel", "--max-dirs", "6", "--dump-passes", "{OUT}", "--dump-dirs", "0", "--dump-lightsum-after", "5", "--out", "{OUT}.Map.Gbx"]), out: "dump-tref" },
        Recipe { name: "prb", env: vec![("LMTOOL_PROBE_DUMP_DIR", t("{OUT}"))], args: { let mut a = vec![t("bake")]; a.extend(common.iter().cloned()); a.extend(sv(&["--per-subsample", "--probes", "transcribed", "--out", "{OUT}.Map.Gbx"])); a }, out: "prb" },
    ]
}

/// Bakes one recipe with `bin` into `res_dir`; returns Ok(seconds) or the error text.
fn bake_recipe(r: &Recipe, bin: &Path, dir: &Path, res_dir: &Path) -> Result<f64, String> {
    std::fs::create_dir_all(res_dir).map_err(|e| e.to_string())?;
    let out = res_dir.join(r.out);
    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_file(&out);
    let outs = out.to_string_lossy().to_string();
    let args: Vec<String> = r.args.iter().map(|a| a.replace("{OUT}", &outs)).collect();
    let env: Vec<(String, String)> = r.env.iter().map(|(k, v)| (k.to_string(), v.replace("{OUT}", &outs))).collect();
    let env_ref: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let (ok, err, secs) = run_in(dir, &env_ref, bin, &args, true);
    if !ok {
        return Err(err.lines().rev().take(5).collect::<Vec<_>>().join(" | "));
    }
    if !out.exists() {
        return Err(format!("{} was not written", out.display()));
    }
    Ok(secs)
}

/// Every regular file under `p`, relative paths sorted.
fn walk(p: &Path) -> Vec<PathBuf> {
    fn rec(base: &Path, d: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let path = e.path();
                if path.is_dir() { rec(base, &path, out); } else { out.push(path.strip_prefix(base).unwrap().to_path_buf()); }
            }
        }
    }
    let mut v = Vec::new();
    if p.is_dir() { rec(p, p, &mut v); } else { v.push(PathBuf::new()); }
    v.sort();
    v
}

fn same_bytes(a: &Path, b: &Path) -> Result<Option<u64>, String> {
    let x = std::fs::read(a).map_err(|e| format!("{}: {e}", a.display()))?;
    let y = std::fs::read(b).map_err(|e| format!("{}: {e}", b.display()))?;
    if x == y { return Ok(None); }
    let pos = x.iter().zip(&y).position(|(p, q)| p != q).map(|p| p as u64).unwrap_or(x.len().min(y.len()) as u64);
    Ok(Some(pos))
}

/// Compares two outputs (file or directory tree); returns the differences as text lines.
fn compare(a: &Path, b: &Path) -> Vec<String> {
    let mut diffs = Vec::new();
    if a.is_dir() != b.is_dir() { return vec![format!("{} and {}: one is a directory", a.display(), b.display())]; }
    if !a.is_dir() {
        match same_bytes(a, b) {
            Ok(None) => {}
            Ok(Some(pos)) => diffs.push(format!("{} differs from {} at byte {pos} (sizes {} / {})", b.display(), a.display(), a.metadata().map(|m| m.len()).unwrap_or(0), b.metadata().map(|m| m.len()).unwrap_or(0))),
            Err(e) => diffs.push(e),
        }
        return diffs;
    }
    // (sorted walks: a set difference by merge — the tiled tiny-16 dump has 113 k files; `contains` per file was quadratic)
    let (fa, fb) = (walk(a), walk(b));
    let sb: std::collections::BTreeSet<&PathBuf> = fb.iter().collect();
    let sa: std::collections::BTreeSet<&PathBuf> = fa.iter().collect();
    for f in &fa { if !sb.contains(f) { diffs.push(format!("only in the reference: {}", f.display())); } }
    for f in &fb { if !sa.contains(f) { diffs.push(format!("only in the new output: {}", f.display())); } }
    for f in fa.iter().filter(|f| sb.contains(f)) {
        match same_bytes(&a.join(f), &b.join(f)) {
            Ok(None) => {}
            Ok(Some(pos)) => diffs.push(format!("{} differs at byte {pos}", f.display())),
            Err(e) => diffs.push(e),
        }
    }
    diffs
}

fn identity(args: &[String]) -> i32 {
    let o = parse(args, &["base", "new", "dir", "checks", "threads"]);
    let dir = abs(o.get_or("dir", "/tmp/pd"));
    let base = abs(o.need("base"));
    let new = abs(o.need("new"));
    let threads = o.get_or("threads", "128");
    let checks: Vec<&str> = o.get_or("checks", "g8,detp1,ref,tref,prb").split(',').collect();
    let name = |p: &Path| p.file_name().unwrap().to_string_lossy().to_string();
    let base_dir = dir.join("id").join(name(&base));
    let new_dir = dir.join("id").join(format!("new-{}", name(&new)));
    let mut failed = 0;
    let t_all = Instant::now();
    for r in recipes(&dir, threads).iter().filter(|r| checks.contains(&r.name)) {
        let ref_out = base_dir.join(r.out);
        if o.has("regen") || !ref_out.exists() {
            match bake_recipe(r, &base, &dir, &base_dir) {
                Ok(s) => eprintln!("identity: {:<6} reference baked with {} in {s:.1}s", r.name, name(&base)),
                Err(e) => { eprintln!("identity: {:<6} REFERENCE BAKE FAILED: {e}", r.name); failed += 1; continue; }
            }
        }
        match bake_recipe(r, &new, &dir, &new_dir) {
            Ok(s) => {
                let d = compare(&ref_out, &new_dir.join(r.out));
                if d.is_empty() {
                    println!("identity: {:<6} IDENTICAL ({} files, {s:.1}s)", r.name, walk(&ref_out).len());
                } else {
                    failed += 1;
                    println!("identity: {:<6} DIFFERENT — {} difference(s):", r.name, d.len());
                    for l in d.iter().take(12) { println!("    {l}"); }
                    if d.len() > 12 { println!("    … {} more", d.len() - 12); }
                }
            }
            Err(e) => { println!("identity: {:<6} BAKE FAILED: {e}", r.name); failed += 1; }
        }
    }
    println!("identity: {} vs {}: {} check(s) failed ({:.0}s)", name(&new), name(&base), failed, t_all.elapsed().as_secs_f64());
    if failed > 0 { 1 } else { 0 }
}

// ---------------------------------------------------------------------------------------------------
// bench

/// The numbers on a `profile [sweep 0]:` line: raster, directions total, sweep total (seconds).
fn parse_profile(err: &str) -> Option<(f64, f64, f64)> {
    let line = err.lines().find(|l| l.starts_with("profile [sweep 0]: "))?;
    let grab = |key: &str| -> Option<f64> {
        let p = line.find(key)? + key.len();
        let rest = &line[p..];
        let end = rest.find('s').unwrap_or(rest.len());
        rest[..end].trim().parse().ok()
    };
    Some((grab("raster ")?, grab("directions total ")?, grab("sweep total ")?))
}

fn median(v: &mut Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if v.is_empty() { 0.0 } else { v[v.len() / 2] }
}

fn bench_bake_args(o: &Opts) -> Vec<String> {
    let mut a = sv(&["bake", o.get_or("map", "g23.Map.Gbx"), "--raster", "--quality", o.get_or("quality", "4"), "--game-peel", "--profile", "--max-dirs", o.get_or("max-dirs", "4"), "--out", o.get_or("out", "bench-out.Map.Gbx")]);
    a.extend(o.extra.iter().cloned());
    a
}

fn bench(args: &[String]) -> i32 {
    let o = parse(args, &["bin", "runs", "threads", "map", "max-dirs", "quality", "dir", "out"]);
    let dir = abs(o.get_or("dir", "."));
    // --threads takes a comma list too (a thread-count study): every (binary, thread count) pair is a row
    let bins0: Vec<PathBuf> = o.need("bin").split(',').map(abs).collect();
    let thread_list: Vec<String> = o.get_or("threads", "128").split(',').map(|s| s.to_string()).collect();
    let bins: Vec<PathBuf> = bins0.iter().flat_map(|b| thread_list.iter().map(move |_| b.clone())).collect();
    let bin_threads: Vec<String> = bins0.iter().flat_map(|_| thread_list.iter().cloned()).collect();
    let runs: usize = o.get_or("runs", "2").parse().unwrap_or(2);
    let threads = thread_list.join(",");
    let bake = bench_bake_args(&o);
    let mut res: Vec<Vec<(f64, f64, f64, f64)>> = vec![Vec::new(); bins.len()];
    for r in 0..runs {
        for (k, b) in bins.iter().enumerate() {
            let (ok, err, wall) = run_in(&dir, &[("LMTOOL_THREADS", &bin_threads[k])], b, &bake, true);
            if !ok { eprintln!("bench: {} failed: {}", b.display(), err.lines().last().unwrap_or("")); return 1; }
            match parse_profile(&err) {
                Some((ra, di, sw)) => { eprintln!("bench: run {} {:<28} T={:<4} raster {ra:.3}  dirs {di:.3}  sweep {sw:.3}  wall {wall:.1}", r + 1, b.file_name().unwrap().to_string_lossy(), bin_threads[k]); res[k].push((ra, di, sw, wall)); }
                None => { eprintln!("bench: {}: no `profile [sweep 0]:` line", b.display()); return 1; }
            }
        }
    }
    println!("{:<28} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}   ({} run(s), LMTOOL_THREADS={threads}, {})", "binary", "raster", "min", "dirs", "min", "sweep", "min", runs, bake.join(" "));
    let mut first: Option<(f64, f64)> = None;
    for (k, b) in bins.iter().enumerate() {
        let mut ra: Vec<f64> = res[k].iter().map(|x| x.0).collect();
        let mut di: Vec<f64> = res[k].iter().map(|x| x.1).collect();
        let mut sw: Vec<f64> = res[k].iter().map(|x| x.2).collect();
        let (mra, mdi, msw) = (median(&mut ra), median(&mut di), median(&mut sw));
        let rel = match first { None => { first = Some((mra, mdi)); String::new() } Some((r0, d0)) => format!("   raster {:+.1} %, dirs {:+.1} % vs the first", 100.0 * (mra / r0 - 1.0), 100.0 * (mdi / d0 - 1.0)) };
        println!("{:<28} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3}{rel}", format!("{} T={}", b.file_name().unwrap().to_string_lossy(), bin_threads[k]), mra, ra[0], mdi, di[0], msw, sw[0]);
    }
    0
}

// ---------------------------------------------------------------------------------------------------
// hangloop

/// `lmperf hangloop --bin A[,B] --runs N --timeout S [--dir D] [--threads T] [--logs DIR] -- <lmtool args…>`: the given
/// lmtool command line N times per binary, interleaved (A B A B …), each run under a wall-clock timeout — a run that
/// outlives it is a HANG: its threads' stacks are taken (`eu-stack -p`), it is killed, its log's last progress line
/// says where it stood. Every run's stderr goes to LOGS/<binary>-run<k>.log (default DIR/hangloop/). The report per
/// binary: completed / hung / failed, the walls' min / median / max. Exit 1 when any run hung or failed.
/// (The pool hang of 2026-09-26: `lmperf hangloop --bin lmtool-base,lmtool-new --runs 15 --timeout 600 -- bake stpad-day.Map.Gbx …`.)
fn hangloop(args: &[String]) -> i32 {
    let o = parse(args, &["bin", "runs", "timeout", "dir", "threads", "logs"]);
    let dir = abs(o.get_or("dir", "."));
    let bins: Vec<PathBuf> = o.need("bin").split(',').map(abs).collect();
    let runs: usize = o.get_or("runs", "10").parse().unwrap_or(10);
    let timeout = std::time::Duration::from_secs_f64(o.get_or("timeout", "600").parse().unwrap_or(600.0));
    let logs = o.get("logs").map(abs).unwrap_or_else(|| dir.join("hangloop"));
    if let Err(e) = std::fs::create_dir_all(&logs) { eprintln!("hangloop: {}: {e}", logs.display()); return 2; }
    if o.extra.is_empty() { eprintln!("hangloop: the lmtool arguments follow `--`"); return 2; }
    #[derive(Default)]
    struct Tally { walls: Vec<f64>, hung: usize, failed: usize }
    let mut tally: Vec<Tally> = bins.iter().map(|_| Tally::default()).collect();
    let name = |b: &Path| b.file_name().unwrap().to_string_lossy().to_string();
    for r in 0..runs {
        for (k, b) in bins.iter().enumerate() {
            let log = logs.join(format!("{}-run{:02}.log", name(b), r + 1));
            let file = match std::fs::File::create(&log) { Ok(f) => f, Err(e) => { eprintln!("hangloop: {}: {e}", log.display()); return 2; } };
            let mut c = Command::new(b);
            c.args(&o.extra).current_dir(&dir).stdout(Stdio::null()).stderr(Stdio::from(file));
            if let Some(t) = o.get("threads") { c.env("LMTOOL_THREADS", t); }
            let t0 = Instant::now();
            let mut child = match c.spawn() { Ok(c) => c, Err(e) => { eprintln!("hangloop: cannot run {}: {e}", b.display()); return 2; } };
            let status = loop {
                match child.try_wait() {
                    Ok(Some(s)) => break Some(s),
                    Ok(None) => {}
                    Err(e) => { eprintln!("hangloop: wait: {e}"); break None; }
                }
                if t0.elapsed() > timeout { break None; }
                std::thread::sleep(std::time::Duration::from_millis(200));
            };
            let wall = t0.elapsed().as_secs_f64();
            let last_line = || std::fs::read_to_string(&log).ok().and_then(|s| s.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.to_string())).unwrap_or_default();
            match status {
                Some(s) if s.success() => {
                    eprintln!("hangloop: run {:>2} {:<20} completed in {wall:.1}s", r + 1, name(b));
                    tally[k].walls.push(wall);
                }
                Some(s) => {
                    eprintln!("hangloop: run {:>2} {:<20} FAILED ({s}) after {wall:.1}s: {}", r + 1, name(b), last_line());
                    tally[k].failed += 1;
                }
                None => {
                    // the stacks of the live process, then the kill
                    let stack_path = logs.join(format!("{}-run{:02}.eu-stack", name(b), r + 1));
                    match Command::new("eu-stack").arg("-p").arg(child.id().to_string()).output() {
                        Ok(out) => { let _ = std::fs::write(&stack_path, [out.stdout, out.stderr].concat()); }
                        Err(e) => { let _ = std::fs::write(&stack_path, format!("eu-stack failed: {e}\n")); }
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    eprintln!("hangloop: run {:>2} {:<20} HUNG (killed after {wall:.0}s; stacks in {}); last line: {}", r + 1, name(b), stack_path.display(), last_line());
                    tally[k].hung += 1;
                }
            }
        }
    }
    let mut bad = false;
    println!("{:<20} {:>9} {:>5} {:>6} {:>9} {:>9} {:>9}   ({runs} run(s) each, timeout {:.0}s, {} {})", "binary", "completed", "hung", "failed", "min s", "median s", "max s", timeout.as_secs_f64(), if let Some(t) = o.get("threads") { format!("LMTOOL_THREADS={t},") } else { String::new() }, o.extra.join(" "));
    for (k, b) in bins.iter().enumerate() {
        let t = &tally[k];
        let mut w = t.walls.clone();
        let (mn, md, mx) = if w.is_empty() { (0.0, 0.0, 0.0) } else { (w.iter().cloned().fold(f64::MAX, f64::min), median(&mut w), w.iter().cloned().fold(0.0, f64::max)) };
        println!("{:<20} {:>9} {:>5} {:>6} {:>9.1} {:>9.1} {:>9.1}", name(b), t.walls.len(), t.hung, t.failed, mn, md, mx);
        bad |= t.hung > 0 || t.failed > 0;
    }
    if bad { 1 } else { 0 }
}

// ---------------------------------------------------------------------------------------------------
// perfstat

fn perfstat(args: &[String]) -> i32 {
    let o = parse(args, &["bin", "events", "threads", "map", "max-dirs", "quality", "dir", "out", "perf"]);
    let dir = abs(o.get_or("dir", "."));
    let bin = abs(o.need("bin"));
    let threads = o.get_or("threads", "128").to_string();
    let events = o.get_or("events", "task-clock,cycles,instructions,cache-references,cache-misses,L1-dcache-loads,L1-dcache-load-misses,dTLB-loads,dTLB-load-misses,page-faults,context-switches,cpu-migrations");
    let perf = o.get_or("perf", "perf");
    let bake = bench_bake_args(&o);
    let mut pargs: Vec<String> = sv(&["stat", "-x", ";", "-e", events, "--"]);
    pargs.push(bin.to_string_lossy().to_string());
    pargs.extend(bake.iter().cloned());
    let (ok, err, wall) = run_in(&dir, &[("LMTOOL_THREADS", &threads), ("LMTOOL_RASTER_STATS", "1")], Path::new(perf), &pargs, true);
    if !ok { eprintln!("perfstat: failed:\n{}", err.lines().rev().take(8).collect::<Vec<_>>().join("\n")); return 1; }
    // the raster's counters: "raster stats (sparse, N bands): T triangles rasterised, B bbox pixels tested, V pixel visits, …"
    let (mut tris, mut tested, mut visits) = (0u64, 0u64, 0u64);
    for l in err.lines().filter(|l| l.starts_with("raster stats")) {
        let num = |key: &str| -> u64 { l.find(key).and_then(|p| l[..p].rsplit(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse().ok())).unwrap_or(0) };
        tris += num(" triangles rasterised");
        tested += num(" bbox pixels tested");
        visits += num(" pixel visits");
    }
    let prof = parse_profile(&err);
    let mut counts: BTreeMap<String, f64> = BTreeMap::new();
    for l in err.lines() {
        let f: Vec<&str> = l.split(';').collect();
        if f.len() >= 3 {
            if let Ok(v) = f[0].trim().parse::<f64>() { counts.insert(f[2].trim().to_string(), v); }
        }
    }
    if counts.is_empty() { eprintln!("perfstat: no counters parsed (is `perf` usable here? try --perf PATH):\n{}", err.lines().rev().take(10).collect::<Vec<_>>().join("\n")); return 1; }
    println!("perfstat: {} {} (LMTOOL_THREADS={threads}); wall {wall:.1}s{}", bin.file_name().unwrap().to_string_lossy(), bake.join(" "), prof.map(|(r, d, s)| format!("; profile sweep 0: raster {r:.3} dirs {d:.3} sweep {s:.3}")).unwrap_or_default());
    let get = |k: &str| counts.iter().find(|(n, _)| n.starts_with(k)).map(|(_, v)| *v).unwrap_or(0.0);
    for (k, v) in &counts { println!("  {:<28} {:>18.0}", k, v); }
    let (cyc, ins) = (get("cycles"), get("instructions"));
    if cyc > 0.0 { println!("  IPC {:.2}", ins / cyc); }
    if visits > 0 {
        println!("  raster counters (whole run): {tris} triangles rasterised, {tested} bbox pixels tested, {visits} pixel visits");
        for k in ["cache-misses", "L1-dcache-load-misses", "dTLB-load-misses", "instructions", "cycles"] {
            let v = get(k);
            if v > 0.0 { println!("  {k} per pixel visit: {:.2}", v / visits as f64); }
        }
    }
    let (l1, l1m) = (get("L1-dcache-loads"), get("L1-dcache-load-misses"));
    if l1 > 0.0 { println!("  L1d miss rate {:.2} %", 100.0 * l1m / l1); }
    let (cr, cm) = (get("cache-references"), get("cache-misses"));
    if cr > 0.0 { println!("  LLC miss rate {:.2} % of references", 100.0 * cm / cr); }
    let (tl, tm) = (get("dTLB-loads"), get("dTLB-load-misses"));
    if tl > 0.0 { println!("  dTLB miss rate {:.3} % of loads", 100.0 * tm / tl); }
    0
}

// ---------------------------------------------------------------------------------------------------
// pgo

fn cargo_env() -> (PathBuf, PathBuf) {
    let home = std::env::var("HOME").unwrap_or_default();
    let cargo = std::env::var("CARGO").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(format!("{home}/bin/cargo")));
    let rustc = std::env::var("RUSTC").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(format!("{home}/bin/rustc")));
    (cargo, rustc)
}

fn cargo_build(tools: &Path, target_dir: &str, rustflags: &str, extra_env: &[(&str, &str)]) -> Result<PathBuf, String> {
    let (cargo, rustc) = cargo_env();
    let mut env: Vec<(&str, &str)> = vec![("RUSTFLAGS", rustflags), ("CARGO_TARGET_DIR", target_dir)];
    let rustc_s = rustc.to_string_lossy().to_string();
    env.push(("RUSTC", &rustc_s));
    env.extend(extra_env.iter().cloned());
    eprintln!("pgo: cargo build --release -p lightmap --offline  (RUSTFLAGS='{rustflags}', target {target_dir})");
    let (ok, err, secs) = run_in(tools, &env, &cargo, &sv(&["build", "--release", "-p", "lightmap", "--offline"]), false);
    if !ok { return Err(format!("build failed:\n{}", err.lines().filter(|l| l.contains("error")).take(20).collect::<Vec<_>>().join("\n"))); }
    eprintln!("pgo: built in {secs:.0}s");
    Ok(tools.join(target_dir).join("release").join("lmtool"))
}

fn pgo(args: &[String]) -> i32 {
    let o = parse(args, &["tools", "target-cpu", "dir", "profdata", "out", "llvm-profdata", "threads", "extra-rustflags", "train-dirs", "train"]);
    let tools = abs(o.get_or("tools", "."));
    let dir = abs(o.get_or("dir", "/tmp/pd"));
    let cpu = o.get_or("target-cpu", "znver4");
    // (the instrumented binary's counters are plain increments shared by every thread: at 128 threads the
    // giant's 4 directions ran 10 minutes without finishing — false sharing on the counters; 16 threads
    // and two directions profile the same paths in about a minute)
    let threads = o.get_or("threads", "16");
    let train_dirs = o.get_or("train-dirs", "2");
    let extra = o.get_or("extra-rustflags", "");
    let raw_dir = tools.join("target-pgo-raw");
    let profdata = abs(o.get_or("profdata", "target-pgo/lmtool.profdata"));
    // (the raw profile format must match the compiler's LLVM: rustc 1.98 = LLVM 22 writes raw version 10, which the
    // LLVM 21 tool in fbsource reads and /opt/llvm's LLVM 23 refuses; the indexed output is read by newer LLVMs)
    let llvm_profdata = o.get_or("llvm-profdata", "/home/vjeux/fbsource/fbcode/third-party-buck/platform010/build/llvm-fb/21/bin/llvm-profdata");
    let _ = std::fs::remove_dir_all(&raw_dir);
    std::fs::create_dir_all(&raw_dir).unwrap();
    // 1. the instrumented build
    let gen_flags = format!("-C target-cpu={cpu} -C profile-generate={} {extra}", raw_dir.display());
    let inst = match cargo_build(&tools, "target-pgo-gen", &gen_flags, &[]) { Ok(p) => p, Err(e) => { eprintln!("pgo: {e}"); return 1; } };
    // 2. the training runs: the giant's first four directions and tiny 16's first six (the bench's shape
    //    and a small map's shape; the raw profiles land in raw_dir, one file per process)
    // --train tiny (default) | giant | both: the instrumented giant did not finish ONE direction in 14 minutes at
    // 4 threads (its 27 M-triangle setup and 500 M visits per direction, every counter a shared increment);
    // tiny 16 walks the same code (raster, alpha test, count, layers, gather) in a couple of minutes.
    let which = o.get_or("train", "tiny");
    let mut train: Vec<(Vec<(&str, &str)>, Vec<String>)> = Vec::new();
    if which == "giant" || which == "both" {
        train.push((vec![("LMTOOL_THREADS", threads)], sv(&["bake", "g23.Map.Gbx", "--raster", "--quality", "4", "--game-peel", "--max-dirs", train_dirs, "--out", "pgo-train-g.Map.Gbx"])));
    }
    if which == "tiny" || which == "both" {
        train.push((vec![("LMTOOL_THREADS", threads)], sv(&["bake", "t16.Map.Gbx", "--raster", "--quality", "4", "--game-peel", "--max-dirs", train_dirs, "--out", "pgo-train-t.Map.Gbx"])));
    }
    for (env, a) in &train {
        eprintln!("pgo: training: lmtool {}", a.join(" "));
        let (ok, err, secs) = run_in(&dir, env, &inst, a, true);
        if !ok { eprintln!("pgo: training run failed: {}", err.lines().rev().take(5).collect::<Vec<_>>().join(" | ")); return 1; }
        eprintln!("pgo: {secs:.0}s");
    }
    let raws: Vec<PathBuf> = walk(&raw_dir).into_iter().filter(|p| p.extension().map(|e| e == "profraw").unwrap_or(false)).map(|p| raw_dir.join(p)).collect();
    if raws.is_empty() { eprintln!("pgo: no .profraw written under {}", raw_dir.display()); return 1; }
    eprintln!("pgo: {} raw profile(s)", raws.len());
    // 3. merge
    std::fs::create_dir_all(profdata.parent().unwrap()).unwrap();
    let mut margs = sv(&["merge", "-o", &profdata.to_string_lossy()]);
    margs.extend(raws.iter().map(|p| p.to_string_lossy().to_string()));
    let (ok, err, _) = run_in(&dir, &[], Path::new(llvm_profdata), &margs, false);
    if !ok { eprintln!("pgo: llvm-profdata merge failed: {err}"); return 1; }
    eprintln!("pgo: merged profile {} ({} bytes)", profdata.display(), profdata.metadata().map(|m| m.len()).unwrap_or(0));
    // 4. the optimised build with the profile
    let use_flags = format!("-C target-cpu={cpu} -C profile-use={} {extra}", profdata.display());
    let opt = match cargo_build(&tools, "target-pgo-use", &use_flags, &[]) { Ok(p) => p, Err(e) => { eprintln!("pgo: {e}"); return 1; } };
    if let Some(out) = o.get("out") {
        std::fs::copy(&opt, abs(out)).unwrap();
        println!("pgo: {}", abs(out).display());
    } else {
        println!("pgo: {}", opt.display());
    }
    0
}

// ---------------------------------------------------------------------------------------------------
// cpu

fn cpu_report() -> String {
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = info.lines().find(|l| l.starts_with("model name")).map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string()).unwrap_or_default();
    let flags = info.lines().find(|l| l.starts_with("flags")).map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string()).unwrap_or_default();
    let has = |f: &str| flags.split_whitespace().any(|x| x == f);
    let v3 = ["avx2", "fma", "bmi1", "bmi2", "movbe", "f16c"].iter().all(|f| has(f));
    let znver4 = ["avx512f", "avx512bw", "avx512dq", "avx512vl", "avx512vbmi", "avx512_vbmi2", "avx512_vnni", "avx512_bitalg", "avx512_vpopcntdq", "avx512_bf16", "gfni", "vaes", "vpclmulqdq"].iter().all(|f| has(f));
    let cpus = info.lines().filter(|l| l.starts_with("processor")).count();
    format!("{model}; {cpus} logical cpus; x86-64-v3 {}; znver4 feature set {} → build with -C target-cpu={}", if v3 { "yes" } else { "NO" }, if znver4 { "yes" } else { "no" }, if znver4 { "znver4" } else if v3 { "x86-64-v3" } else { "x86-64" })
}

// ---------------------------------------------------------------------------------------------------
// concurrent

/// `lmperf concurrent --bin BIN --n N [--threads T] [--dir D] -- bake args…` runs N copies of the bake at
/// once (each with its own --out), reports every copy's wall and the whole batch's; the maps-per-box
/// question for the fleet: does a box run two tiny maps in the time of one?
fn concurrent(args: &[String]) -> i32 {
    let o = parse(args, &["bin", "n", "threads", "dir", "map", "max-dirs", "quality"]);
    let dir = abs(o.get_or("dir", "."));
    let bin = abs(o.need("bin"));
    let n: usize = o.get_or("n", "2").parse().unwrap_or(2);
    let threads = o.get_or("threads", "128").to_string();
    let t0 = Instant::now();
    let mut children = Vec::new();
    for k in 0..n {
        let mut a = sv(&["bake", o.get_or("map", "t16.Map.Gbx"), "--raster", "--quality", o.get_or("quality", "4"), "--game-peel", "--profile", "--out", &format!("concurrent-{k}.Map.Gbx")]);
        if let Some(m) = o.get("max-dirs") { a.push("--max-dirs".into()); a.push(m.into()); }
        a.extend(o.extra.iter().cloned());
        let c = Command::new(&bin).args(&a).current_dir(&dir).env("LMTOOL_THREADS", &threads).stdout(Stdio::null()).stderr(Stdio::piped()).spawn();
        match c {
            Ok(c) => children.push((k, c)),
            Err(e) => { eprintln!("concurrent: cannot start copy {k}: {e}"); return 1; }
        }
    }
    let mut walls = Vec::new();
    for (k, c) in children {
        let out = c.wait_with_output().unwrap();
        let wall = t0.elapsed().as_secs_f64();
        let err = String::from_utf8_lossy(&out.stderr);
        let sweeps: Vec<String> = err.lines().filter(|l| l.starts_with("profile [sweep ")).filter_map(|l| l.rfind("sweep total ").map(|p| l[p + 12..].trim().to_string())).collect();
        println!("concurrent: copy {k} {} after {wall:.1}s (sweep totals {})", if out.status.success() { "ok" } else { "FAILED" }, sweeps.join(" / "));
        walls.push(wall);
    }
    println!("concurrent: {n} × `lmtool bake {}` at LMTOOL_THREADS={threads}: batch wall {:.1}s, per copy {:.1}s of machine time", o.get_or("map", "t16.Map.Gbx"), t0.elapsed().as_secs_f64(), t0.elapsed().as_secs_f64() / n as f64);
    0
}

// ---------------------------------------------------------------------------------------------------
// smt-probe

#[cfg(target_os = "linux")]
fn pin_to(cpu: usize) -> bool {
    extern "C" {
        fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const u64) -> i32;
    }
    let mut mask = [0u64; 16];
    mask[cpu / 64] |= 1u64 << (cpu % 64);
    unsafe { sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()) == 0 }
}

/// A fixed compute kernel of ~`iters` steps, HIGH IPC by design (eight independent integer chains + eight
/// independent f64 chains per step: it fills a core's issue width like the raster does at IPC 2+); returns its
/// wall in seconds. Two such threads on SMT siblings each run ~1.5–2× slower than alone — a single
/// dependency chain (the first version of this kernel) shares a core with another chain at NO loss and
/// therefore saw no siblings at all, while the bake lost 44 % beside 96 such hogs.
fn kernel(iters: u64) -> f64 {
    let t = Instant::now();
    let mut a = [1u64, 2, 3, 4, 5, 6, 7, 8];
    let mut f = [1.0f64, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7];
    for i in 0..iters {
        for k in 0..8 {
            a[k] = a[k].wrapping_mul(6364136223846793005).wrapping_add(i ^ k as u64);
            f[k] = f[k] * 1.000000001 + 1e-9;
        }
    }
    std::hint::black_box((a, f));
    t.elapsed().as_secs_f64()
}

/// `lmperf smt-probe [--cpus N] [--ref 0] [--ms 60]`: which logical CPUs share a physical core with the
/// reference CPU — a guest sees "1 thread per core" whatever the host does, so the answer comes from
/// measurement: the reference kernel on CPU `ref` alone, then beside a load on every other CPU; the
/// siblings slow it down. Prints the slowdown per CPU and the sibling(s) found; with `--all` probes
/// every CPU as the reference (N² / 2 runs) and prints the pairing.
fn smt_probe(args: &[String]) -> i32 {
    let o = parse(args, &["cpus", "ref", "ms"]);
    let ncpu: usize = o.get("cpus").and_then(|s| s.parse().ok()).unwrap_or_else(|| std::thread::available_parallelism().map(|x| x.get()).unwrap_or(1));
    let ms: u64 = o.get_or("ms", "60").parse().unwrap_or(60);
    // calibrate the kernel to ~ms on the reference cpu
    let refcpu: usize = o.get_or("ref", "0").parse().unwrap_or(0);
    let calib = std::thread::spawn(move || { pin_to(refcpu); let mut it = 1_000_000u64; loop { let s = kernel(it); if s > 0.02 { return (it as f64 * (ms as f64 / 1e3) / s) as u64; } it *= 4; } }).join().unwrap();
    let alone = std::thread::spawn(move || { pin_to(refcpu); let mut v = Vec::new(); for _ in 0..3 { v.push(kernel(calib)); } v.into_iter().fold(f64::MAX, f64::min) }).join().unwrap();
    let probe_one = |refcpu: usize, k: usize| -> f64 {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let st = stop.clone();
        let load = std::thread::spawn(move || { pin_to(k); while !st.load(std::sync::atomic::Ordering::Relaxed) { kernel(calib / 8); } });
        let t = std::thread::spawn(move || { pin_to(refcpu); std::thread::sleep(std::time::Duration::from_millis(5)); let mut v = Vec::new(); for _ in 0..2 { v.push(kernel(calib)); } v.into_iter().fold(f64::MAX, f64::min) }).join().unwrap();
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        load.join().unwrap();
        t
    };
    if o.has("all") {
        // every cpu's worst partner
        let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
        for r in 0..ncpu {
            let mut worst = (r, 1.0f64);
            for k in 0..ncpu {
                if k == r { continue; }
                let s = probe_one(r, k) / alone;
                if s > worst.1 { worst = (k, s); }
            }
            pairs.push((r, worst.0, worst.1));
            eprintln!("smt-probe: cpu {r}: worst partner {} at {:.2}×", worst.0, worst.1);
        }
        println!("smt-probe: siblings (cpu, partner, slowdown): {}", pairs.iter().map(|(a, b, s)| format!("{a}-{b} {s:.2}")).collect::<Vec<_>>().join(", "));
        return 0;
    }
    println!("smt-probe: reference cpu {refcpu}, kernel alone {:.1} ms ({} cpus)", alone * 1e3, ncpu);
    let mut rows: Vec<(usize, f64)> = Vec::new();
    for k in 0..ncpu {
        if k == refcpu { continue; }
        let s = probe_one(refcpu, k) / alone;
        rows.push((k, s));
    }
    let mut sorted = rows.clone();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    println!("smt-probe: slowdown of cpu {refcpu} with a load on cpu k — the five worst: {}", sorted.iter().take(5).map(|(k, s)| format!("cpu {k}: {s:.2}×")).collect::<Vec<_>>().join(", "));
    let median = { let mut v: Vec<f64> = rows.iter().map(|r| r.1).collect(); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[v.len() / 2] };
    let sib: Vec<usize> = rows.iter().filter(|(_, s)| *s > median * 1.25).map(|(k, _)| *k).collect();
    println!("smt-probe: median slowdown {median:.2}×; cpus more than 25 % worse than the median (the siblings): {:?}", sib);
    println!("smt-probe: full row: {}", rows.iter().map(|(k, s)| format!("{k}:{s:.2}")).collect::<Vec<_>>().join(" "));
    0
}

/// `lmperf scale-probe [--threads 16,64,128,160] [--ms 200]`: does the box's throughput scale with the
/// thread count? Two kernels, each run on N threads at once: pure compute (the smt-probe kernel) and a
/// random-read kernel over a 512 MB table (the raster's `tris[ti]` pattern: one dependent 64-byte
/// load per step). Prints per-thread throughput relative to one thread — 1.00 = perfect scaling.
fn scale_probe(args: &[String]) -> i32 {
    let o = parse(args, &["threads", "ms"]);
    let list: Vec<usize> = o.get_or("threads", "1,16,32,64,88,128,160").split(',').filter_map(|s| s.parse().ok()).collect();
    let ms: u64 = o.get_or("ms", "200").parse().unwrap_or(200);
    let calib = { let mut it = 1_000_000u64; loop { let s = kernel(it); if s > 0.02 { break (it as f64 * (ms as f64 / 1e3) / s) as u64; } it *= 4; } };
    // the random-read table: 512 MB of u32 indices forming random chains
    let n = 128usize << 20;
    let table: std::sync::Arc<Vec<u32>> = std::sync::Arc::new({
        let mut v: Vec<u32> = (0..n as u32).collect();
        let mut s = 0x9e3779b97f4a7c15u64;
        for i in (1..n).rev() { s ^= s << 13; s ^= s >> 7; s ^= s << 17; let j = (s % (i as u64 + 1)) as usize; v.swap(i, j); }
        v
    });
    let chase = |t: &Vec<u32>, steps: u64, start: u32| -> f64 {
        let t0 = Instant::now();
        let mut i = start;
        for _ in 0..steps { i = t[i as usize]; }
        std::hint::black_box(i);
        t0.elapsed().as_secs_f64()
    };
    let steps = 2_000_000u64;
    println!("{:<8} {:>14} {:>14}   (per-thread throughput relative to 1 thread; compute kernel / random 64-byte reads over 512 MB)", "threads", "compute", "random-read");
    let (mut c1, mut r1) = (0.0f64, 0.0f64);
    for &nt in &list {
        let hs: Vec<_> = (0..nt).map(|k| { let tb = table.clone(); std::thread::spawn(move || { pin_to(k); (kernel(calib), chase(&tb, steps, (k as u32 * 7919) % (n as u32))) }) }).collect();
        let rs: Vec<(f64, f64)> = hs.into_iter().map(|h| h.join().unwrap()).collect();
        let c = rs.iter().map(|r| r.0).sum::<f64>() / nt as f64;
        let r = rs.iter().map(|r| r.1).sum::<f64>() / nt as f64;
        if c1 == 0.0 { c1 = c; r1 = r; }
        println!("{:<8} {:>14.2} {:>14.2}   ({:.0} ns per random read)", nt, c1 / c, r1 / r, r / steps as f64 * 1e9);
    }
    0
}

/// `lmperf hog --kind compute|random|stream --threads N [--first-cpu 64] [--secs 60]`: N threads of background
/// load (pinned to consecutive CPUs from --first-cpu) while a bake runs beside them — separates the bake's
/// concurrency inflation into frequency (compute hogs), DRAM latency/bandwidth (random / streaming hogs)
/// and the bake's own shared-cache footprint (inflation with neither).
fn hog(args: &[String]) -> i32 {
    let o = parse(args, &["kind", "threads", "first-cpu", "secs"]);
    let kind = o.get_or("kind", "compute").to_string();
    let n: usize = o.get_or("threads", "96").parse().unwrap_or(96);
    let first: usize = o.get_or("first-cpu", "64").parse().unwrap_or(64);
    let secs: u64 = o.get_or("secs", "60").parse().unwrap_or(60);
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let hs: Vec<_> = (0..n).map(|k| {
        let st = stop.clone();
        let kind = kind.clone();
        std::thread::spawn(move || {
            pin_to(first + k);
            match kind.as_str() {
                "random" => {
                    let m = 32usize << 20; // 128 MB per thread
                    let mut t: Vec<u32> = (0..m as u32).collect();
                    let mut s = 0x9e3779b97f4a7c15u64 ^ (k as u64 * 0x1234567);
                    for i in (1..m).rev() { s ^= s << 13; s ^= s >> 7; s ^= s << 17; let j = (s % (i as u64 + 1)) as usize; t.swap(i, j); }
                    let mut i = 0u32;
                    while !st.load(std::sync::atomic::Ordering::Relaxed) { for _ in 0..100_000 { i = t[i as usize]; } std::hint::black_box(i); }
                }
                "stream" => {
                    let mut v = vec![1u64; 8 << 20]; // 64 MB per thread
                    let mut acc = 0u64;
                    while !st.load(std::sync::atomic::Ordering::Relaxed) { for x in v.iter_mut() { *x = x.wrapping_mul(3).wrapping_add(1); acc ^= *x; } std::hint::black_box(acc); }
                }
                _ => { while !st.load(std::sync::atomic::Ordering::Relaxed) { kernel(10_000_000); } }
            }
        })
    }).collect();
    eprintln!("hog: {n} {kind} threads on cpus {first}..{} for {secs}s", first + n - 1);
    std::thread::sleep(std::time::Duration::from_secs(secs));
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    for h in hs { h.join().unwrap(); }
    0
}
