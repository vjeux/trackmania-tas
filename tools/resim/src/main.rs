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

    let argv: Vec<String> = std::env::args().skip(1).collect();
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
