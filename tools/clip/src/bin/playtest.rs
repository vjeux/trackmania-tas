//! `playtest` -- the trainer page, end to end in a real headless Chrome.
//!
//!     playtest [--trainer <dir>] [--chrome <path>]
//!
//! Prints one line per simulated player. Exit 0 only when the browser actually
//! scored a run.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clip::playtest;

const USAGE: &str = "\
playtest [--trainer <dir>] [--chrome <path>]
    Assemble trainer/index.html with the frame pump and the driver, run it in
    headless Chrome, and print the verdict it reaches. --trainer defaults to
    ./trainer; --chrome to $CHROME, then a browser on PATH, then the Mac's.
";

fn main() -> ExitCode {
    match go() {
        Ok(lines) => {
            for l in lines {
                println!("{l}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn go() -> Result<Vec<String>, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut trainer: Option<PathBuf> = None;
    let mut chrome: Option<String> = std::env::var("CHROME").ok().filter(|s| !s.is_empty());
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--trainer" => {
                trainer = Some(PathBuf::from(
                    args.get(i + 1).ok_or("--trainer needs a directory")?,
                ));
                i += 2;
            }
            "--chrome" => {
                chrome = Some(args.get(i + 1).ok_or("--chrome needs a path")?.clone());
                i += 2;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(vec![]);
            }
            other => return Err(format!("unknown argument {other:?}\n\n{USAGE}")),
        }
    }

    let dir = match trainer {
        Some(d) => d,
        None => playtest::find_trainer_dir()
            .ok_or("no trainer/index.html here — run from the repo root or pass --trainer")?,
    };
    check(&dir)?;
    let browser = playtest::find_chrome(chrome.as_deref())?;
    playtest::run(&dir, &browser)
}

fn check(dir: &Path) -> Result<(), String> {
    for f in ["index.html", "playtest-pump.js", "playtest-drive.js"] {
        if !dir.join(f).is_file() {
            return Err(format!("{} has no {f}", dir.display()));
        }
    }
    Ok(())
}
