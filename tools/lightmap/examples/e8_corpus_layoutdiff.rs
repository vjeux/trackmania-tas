//! `e8_corpus_layoutdiff --corpus CORPUS.tsv --bin LMTOOL [--refs DIR] [--paks DIR] [--out DIR] [--jobs N] [--env K=V]...`
//! THE LAYOUT ATTRIBUTION of a layout-side change over the corpus (E8, 2026-10-01): every cell with an oracle gets its product
//! bake command (`corpusgate::bake_args`) run to `--layout-tsv` only (the layout stops the bake; the DayTime word plays no part
//! in a layout) TWICE — under the candidate's default and under the baseline env given by `--env` (e.g. LMTOOL_SUM_ORDER=asc) —
//! and the two tables are compared byte for byte: the cells whose layout moves are the cells whose bytes move.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let flag = |n: &str| a.iter().position(|x| x == n).and_then(|i| a.get(i + 1).cloned());
    let corpus = flag("--corpus").expect("--corpus");
    let bin = flag("--bin").expect("--bin");
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
    let refs = std::path::PathBuf::from(flag("--refs").unwrap_or_else(|| format!("{home}/persistent/private-30d/tm-player/tiny/lightmap-re/refs")));
    let paks = std::path::PathBuf::from(flag("--paks").unwrap_or_else(|| "/tmp/paks".into()));
    let out = std::path::PathBuf::from(flag("--out").unwrap_or_else(|| "/tmp/e8-layoutdiff".into()));
    let jobs: usize = flag("--jobs").map(|v| v.parse().unwrap()).unwrap_or(6);
    let envs: Vec<(String, String)> = a.iter().enumerate().filter(|(_, x)| *x == "--env").filter_map(|(i, _)| a.get(i + 1)).filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
    if envs.is_empty() { eprintln!("--env K=V names the BASELINE knob (the old behaviour); none given"); std::process::exit(2); }
    std::fs::create_dir_all(&out).unwrap();
    let cells = lightmap::corpusgate::read_corpus(std::path::Path::new(&corpus), &refs).unwrap_or_else(|e| panic!("{e}"));
    let todo: Vec<lightmap::corpusgate::Cell> = cells.into_iter().filter(|c| c.oracle.is_some() && !c.compare_only).collect();
    eprintln!("{} cells with an oracle; {} jobs; baseline env {:?}", todo.len(), jobs, envs);
    let results = std::sync::Mutex::new(Vec::<(String, String)>::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= todo.len() { break; }
                let c = &todo[i];
                let cdir = out.join(&c.name);
                let _ = std::fs::create_dir_all(&cdir);
                let mut verdicts = Vec::new();
                let mut tsvs: Vec<std::path::PathBuf> = Vec::new();
                for (label, base_env) in [("cand", false), ("base", true)] {
                    let tsv = cdir.join(format!("layout-{label}.tsv"));
                    let records = cdir.join(format!("records-{label}.tsv"));
                    let o = cdir.join("x.Map.Gbx");
                    let mut args = lightmap::corpusgate::bake_args(c, &c.source, &paks, &records, &o);
                    args.push("--layout-tsv".into()); args.push(tsv.to_string_lossy().into());
                    let mut cmd = std::process::Command::new(&bin);
                    cmd.args(&args);
                    for (k, v) in &c.env { cmd.env(k, v); }
                    if base_env { for (k, v) in &envs { cmd.env(k, v); } }
                    let t0 = std::time::Instant::now();
                    let res = cmd.output();
                    match res {
                        Ok(o) => {
                            let log = String::from_utf8_lossy(&o.stderr);
                            let _ = std::fs::write(cdir.join(format!("log-{label}.txt")), log.as_bytes());
                            let lg = log.lines().find(|l| l.starts_with("layout-game:")).unwrap_or("").to_string();
                            let grouped = log.lines().find(|l| l.starts_with("layout grouped:")).unwrap_or("").to_string();
                            verdicts.push(format!("{label}: {} {:.0}s | {lg} | {grouped}", if o.status.success() { "ok" } else { "FAILED" }, t0.elapsed().as_secs_f32()));
                        }
                        Err(e) => verdicts.push(format!("{label}: spawn error {e}")),
                    }
                    tsvs.push(tsv);
                }
                let same = match (std::fs::read(&tsvs[0]), std::fs::read(&tsvs[1])) { (Ok(x), Ok(y)) => if x == y { "IDENTICAL".to_string() } else { let (lx, ly) = (String::from_utf8_lossy(&x), String::from_utf8_lossy(&y)); let n = lx.lines().zip(ly.lines()).filter(|(p, q)| p != q).count(); format!("DIFFERS ({n} rows)") }, _ => "MISSING".into() };
                let line = format!("{}\t{same}\t{}", c.name, verdicts.join(" || "));
                eprintln!("{line}");
                results.lock().unwrap().push((c.name.clone(), line));
            });
        }
    });
    let mut r = results.into_inner().unwrap();
    r.sort();
    let mut txt = String::from("cell\tlayout\tcandidate | baseline\n");
    for (_, l) in &r { txt.push_str(l); txt.push('\n'); }
    std::fs::write(out.join("summary.tsv"), &txt).unwrap();
    println!("{txt}");
}
