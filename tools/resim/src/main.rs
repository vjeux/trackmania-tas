//! `tmresim` — the standing re-simulation sweep, as a long-haul worker.

use resim::sweep::{summarise, Sweep};
use std::path::PathBuf;

fn main() {
    let mut repo = std::env::var("TMHAUL_REPO").unwrap_or_else(|_| "/tmp/tmtas".to_string());
    let mut corpus: Vec<PathBuf> = Vec::new();
    let mut server = std::env::var("TM_SERVER").unwrap_or_else(|_| "/tmp/tmoracle/server".into());
    let mut progress = std::env::var("TMHAUL_PROGRESS").ok();
    let mut results: Option<String> = None;
    let mut only: Option<String> = None;
    let mut repeat = false;
    let mut pause_s = 60u64;
    let mut passes = 0u64;
    let mut start_dev_max = 32.0f64;

    // --version / -V. Compile-time only: CARGO_PKG_* come from the crate's
    // Cargo.toml (which inherits the one workspace version), and TAS_BUILD is
    // the git hash the release build sets. option_env! means an ordinary
    // `cargo build` still works and simply reports "dev". No dependency.
    if std::env::args().any(|x| x == "--version" || x == "-V") {
        println!(
            "{} {} ({})",
            option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            env!("CARGO_PKG_VERSION"),
            option_env!("TAS_BUILD").unwrap_or("dev")
        );
        std::process::exit(0);
    }
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("maps") {
        std::process::exit(maps_cmd(&argv[1..]));
    }
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].as_str();
        let mut next = || {
            i += 1;
            argv.get(i).cloned().unwrap_or_default()
        };
        match a {
            "--repo" => repo = next(),
            "--corpus" => corpus.push(PathBuf::from(next())),
            "--server" => server = next(),
            "--progress" => progress = Some(next()),
            "--results" => results = Some(next()),
            "--map" => only = Some(next()),
            "--loop" => repeat = true,
            "--pause-s" => pause_s = next().parse().unwrap_or(60),
            "--passes" => passes = next().parse().unwrap_or(0),
            "--start-dev-max" => start_dev_max = next().parse().unwrap_or(32.0),
            "--help" | "-h" => {
                println!(
                    "tmresim — re-simulate every banked result through the plain oracle

  --repo DIR        the checkout (default $TMHAUL_REPO or /tmp/tmtas)
  --corpus DIR      where the .Map.Gbx files live; repeatable
  --server DIR      the dedicated server (default $TM_SERVER)
  --progress FILE   append harness progress records here
  --results FILE    append one record per tape here
  --map DIR         only this map directory
  --loop            keep sweeping; standing, not a spot check
  --pause-s N       seconds between passes (default 60)
  --passes N        stop after N passes

A human's recording is refused by the gate, not skipped in silence."
                );
                return;
            }
            other => {
                eprintln!("tmresim: unknown flag {other:?}");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    if corpus.is_empty() {
        let home = std::env::var("HOME").unwrap_or_default();
        corpus.push(PathBuf::from(format!("{home}/persistent/private-30d/tm-unbeaten")));
    }

    // A worker resumes from where the banked state says the run got to, so a
    // box dying costs one banking window and not the whole run.
    let mut evals: u64 = std::env::var("TMHAUL_RESUME_EVALS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let s = Sweep {
        repo: PathBuf::from(&repo),
        corpus,
        server: PathBuf::from(&server),
        progress: progress.map(PathBuf::from),
        results: results.map(PathBuf::from),
        only_map: only,
        registry: {
            let p = PathBuf::from(&repo).join("autopilot/config/maps.rec");
            match resim::maps::read_registry(&p) {
                Ok(r) => r,
                Err(e) => {
                    // Loud: without the registry there is no start line to
                    // check against, and a silently absent check is the bug
                    // class this project keeps paying for.
                    eprintln!("tmresim: no map registry ({e}) — the start-position check cannot run");
                    Vec::new()
                }
            }
        },
        start_dev_max_m: start_dev_max,
    };

    let mut pass = 0u64;
    loop {
        pass += 1;
        let t0 = haul::time::now();
        match s.run(evals) {
            Ok((rows, e)) => {
                evals = e;
                println!(
                    "\npass {pass}: {} — {} in {}",
                    summarise(&rows),
                    format!("{} tapes", rows.len()),
                    haul::time::dur(haul::time::now() - t0)
                );
            }
            Err(e) => {
                eprintln!("tmresim: sweep failed: {e}");
                std::process::exit(2);
            }
        }
        if !repeat || (passes > 0 && pass >= passes) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(pause_s));
    }
}

/// `tmresim maps scan|verify|show` — the registry that replaces putting
/// Nadeo's map files in a public repo.
fn maps_cmd(argv: &[String]) -> i32 {
    let mut corpus: Vec<PathBuf> = Vec::new();
    let mut out: Option<PathBuf> = None;
    let action = argv.first().cloned().unwrap_or_else(|| "show".into());
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--corpus" => {
                i += 1;
                corpus.push(PathBuf::from(argv.get(i).cloned().unwrap_or_default()));
            }
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(argv.get(i).cloned().unwrap_or_default()));
            }
            other => {
                eprintln!("tmresim maps: unknown flag {other:?}");
                return 1;
            }
        }
        i += 1;
    }
    if corpus.is_empty() {
        let home = std::env::var("HOME").unwrap_or_default();
        corpus.push(PathBuf::from(format!("{home}/persistent/private-30d/tm-unbeaten")));
        corpus.push(PathBuf::from(format!(
            "{home}/persistent/private-30d/tm-autopilot/B-cartographer/bank/maps"
        )));
    }
    let repo = std::env::var("TMHAUL_REPO").unwrap_or_else(|_| "/tmp/tmtas".into());
    let registry = out.unwrap_or_else(|| PathBuf::from(&repo).join("autopilot/config/maps.rec"));

    match action.as_str() {
        "scan" => {
            let rows = match resim::maps::scan(&corpus) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("tmresim maps: {e}");
                    return 2;
                }
            };
            for r in &rows {
                println!(
                    "{:<12} {:<30} author {:>10}  cps {:<3} {}",
                    r.id,
                    r.name,
                    r.author_ms.map(haul::time::ms_as_seconds).unwrap_or_else(|| "unknown".into()),
                    r.cps,
                    r.spawn
                        .map(|(x, z)| format!("spawn ({x}, {z})"))
                        .unwrap_or_else(|| "spawn unknown".into())
                );
            }
            if let Err(e) = resim::maps::write(&rows, &registry) {
                eprintln!("tmresim maps: {e}");
                return 2;
            }
            println!("\n{} map(s) -> {}", rows.len(), registry.display());
            0
        }
        "verify" => {
            let rows = match resim::maps::read_registry(&registry) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("tmresim maps: {e}");
                    return 2;
                }
            };
            let checks = resim::maps::verify(&rows, &corpus);
            let mut bad = 0;
            for (id, c) in &checks {
                match c {
                    resim::maps::Check::Ok => {}
                    resim::maps::Check::Missing => {
                        bad += 1;
                        let row = rows.iter().find(|r| &r.id == id);
                        println!(
                            "MISSING  {id}  refetch: {}",
                            row.map(|r| r.url.clone()).unwrap_or_default()
                        );
                    }
                    resim::maps::Check::Changed { got } => {
                        bad += 1;
                        println!("CHANGED  {id}  md5 {got}, the registry says something else");
                    }
                }
            }
            println!(
                "{} of {} map(s) verified against the registry",
                checks.len() - bad,
                checks.len()
            );
            if bad == 0 {
                0
            } else {
                2
            }
        }
        "show" => {
            match resim::maps::read_registry(&registry) {
                Ok(rows) => {
                    for r in &rows {
                        println!(
                            "{:<12} {:<28} {:>10}  md5 {}  {}",
                            r.id,
                            r.name,
                            r.author_ms.map(haul::time::ms_as_seconds).unwrap_or_else(|| "unknown".into()),
                            &r.md5[..8.min(r.md5.len())],
                            r.url
                        );
                    }
                    println!("{} map(s) in {}", rows.len(), registry.display());
                    0
                }
                Err(e) => {
                    eprintln!("tmresim maps: {e}");
                    2
                }
            }
        }
        other => {
            eprintln!("tmresim maps: expected scan|verify|show, got {other:?}");
            1
        }
    }
}
