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
        Some("perfstat") => perfstat(&args[1..]),
        Some("pgo") => pgo(&args[1..]),
        Some("cpu") => { println!("{}", cpu_report()); 0 }
        _ => {
            eprintln!("usage: lmperf identity|bench|perfstat|pgo|cpu … (see the module doc in tools/lmperf/src/main.rs)");
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
    let (fa, fb) = (walk(a), walk(b));
    for f in &fa { if !fb.contains(f) { diffs.push(format!("only in the reference: {}", f.display())); } }
    for f in &fb { if !fa.contains(f) { diffs.push(format!("only in the new output: {}", f.display())); } }
    for f in fa.iter().filter(|f| fb.contains(f)) {
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
    let bins: Vec<PathBuf> = o.need("bin").split(',').map(abs).collect();
    let runs: usize = o.get_or("runs", "2").parse().unwrap_or(2);
    let threads = o.get_or("threads", "128").to_string();
    let bake = bench_bake_args(&o);
    let mut res: Vec<Vec<(f64, f64, f64, f64)>> = vec![Vec::new(); bins.len()];
    for r in 0..runs {
        for (k, b) in bins.iter().enumerate() {
            let (ok, err, wall) = run_in(&dir, &[("LMTOOL_THREADS", &threads)], b, &bake, true);
            if !ok { eprintln!("bench: {} failed: {}", b.display(), err.lines().last().unwrap_or("")); return 1; }
            match parse_profile(&err) {
                Some((ra, di, sw)) => { eprintln!("bench: run {} {:<28} raster {ra:.3}  dirs {di:.3}  sweep {sw:.3}  wall {wall:.1}", r + 1, b.file_name().unwrap().to_string_lossy()); res[k].push((ra, di, sw, wall)); }
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
        println!("{:<28} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3}{rel}", b.file_name().unwrap().to_string_lossy(), mra, ra[0], mdi, di[0], msw, sw[0]);
    }
    0
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
    let o = parse(args, &["tools", "target-cpu", "dir", "profdata", "out", "llvm-profdata", "threads", "extra-rustflags"]);
    let tools = abs(o.get_or("tools", "."));
    let dir = abs(o.get_or("dir", "/tmp/pd"));
    let cpu = o.get_or("target-cpu", "znver4");
    let threads = o.get_or("threads", "128");
    let extra = o.get_or("extra-rustflags", "");
    let raw_dir = tools.join("target-pgo-raw");
    let profdata = abs(o.get_or("profdata", "target-pgo/lmtool.profdata"));
    let llvm_profdata = o.get_or("llvm-profdata", "/opt/llvm/bin/llvm-profdata");
    let _ = std::fs::remove_dir_all(&raw_dir);
    std::fs::create_dir_all(&raw_dir).unwrap();
    // 1. the instrumented build
    let gen_flags = format!("-C target-cpu={cpu} -C profile-generate={} {extra}", raw_dir.display());
    let inst = match cargo_build(&tools, "target-pgo-gen", &gen_flags, &[]) { Ok(p) => p, Err(e) => { eprintln!("pgo: {e}"); return 1; } };
    // 2. the training runs: the giant's first four directions and tiny 16's first six (the bench's shape
    //    and a small map's shape; the raw profiles land in raw_dir, one file per process)
    let train: Vec<(Vec<(&str, &str)>, Vec<String>)> = vec![
        (vec![("LMTOOL_THREADS", threads)], sv(&["bake", "g23.Map.Gbx", "--raster", "--quality", "4", "--game-peel", "--max-dirs", "4", "--out", "pgo-train-g.Map.Gbx"])),
        (vec![], sv(&["bake", "t16.Map.Gbx", "--raster", "--quality", "4", "--game-peel", "--max-dirs", "6", "--out", "pgo-train-t.Map.Gbx"])),
    ];
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
    let use_flags = format!("-C target-cpu={cpu} -C profile-use={} -C llvm-args=-pgo-warn-missing-function {extra}", profdata.display());
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
