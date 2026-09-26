//! `tinyctl gist-publish --gist ID --dir DIR [--files a.md,b.md] [--gh BIN]` — every file of
//! DIR replaces its namesake in the gist (`gh gist edit -f`), a file the gist lacks is added
//! (`-a`), and the file list is VERIFIED afterwards (3 passes: `gh gist edit -f` on the big
//! files has silently dropped them before — 2026-09-22 — so a missing file is re-added).
//! The public Altered U10S change log (gist 5919163bc2067f3c9511a9a680826c8d); was the box's
//! `changelog-publish.sh`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn gist_files(gh: &Path, gist: &str) -> Vec<String> {
    Command::new(gh).args(["gist", "view", gist, "--files"]).output().map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default()
}

fn gh_edit(gh: &Path, gist: &str, file: &Path, replace: bool) -> bool {
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    let st = if replace {
        Command::new(gh).args(["gist", "edit", gist, "-f", &name]).arg(file).status()
    } else {
        Command::new(gh).args(["gist", "edit", gist, "-a"]).arg(file).status()
    };
    st.map(|s| s.success()).unwrap_or(false)
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let gist = f("--gist").ok_or("gist-publish needs --gist ID")?;
    let dir = PathBuf::from(f("--dir").ok_or("gist-publish needs --dir DIR")?);
    let gh = PathBuf::from(f("--gh").unwrap_or_else(|| "/home/vjeux/bin/gh".into()));
    let files: Vec<PathBuf> = match f("--files") {
        Some(list) => list.split(',').map(|n| dir.join(n.trim())).collect(),
        None => {
            let mut v: Vec<PathBuf> = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "md").unwrap_or(false)).collect();
            v.sort();
            v
        }
    };
    let files: Vec<PathBuf> = files.into_iter().filter(|p| p.is_file()).collect();
    if files.is_empty() {
        return Err(format!("no files under {}", dir.display()));
    }
    let mut have = gist_files(&gh, &gist);
    if have.is_empty() {
        return Err(format!("gist {gist}: no file list (gh not authenticated?)"));
    }
    for p in &files {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let ok = if have.iter().any(|h| *h == name) {
            gh_edit(&gh, &gist, p, true) || { std::thread::sleep(Duration::from_secs(2)); gh_edit(&gh, &gist, p, false) }
        } else {
            gh_edit(&gh, &gist, p, false)
        };
        println!("{name}: {}", if ok { "ok" } else { "FAILED" });
        std::thread::sleep(Duration::from_secs(2));
    }
    // verify: the edit of a big file has silently dropped it before — re-add what is missing
    for pass in 1..=3 {
        std::thread::sleep(Duration::from_secs(15));
        have = gist_files(&gh, &gist);
        let missing: Vec<&PathBuf> = files.iter().filter(|p| !have.iter().any(|h| *h == p.file_name().unwrap().to_string_lossy())).collect();
        if missing.is_empty() {
            println!("verified: all {} files present in gist {gist}", files.len());
            return Ok(());
        }
        for p in missing {
            std::thread::sleep(Duration::from_secs(3));
            let ok = gh_edit(&gh, &gist, p, false);
            println!("{}: {} (pass {pass})", p.file_name().unwrap().to_string_lossy(), if ok { "re-added" } else { "STILL MISSING" });
        }
    }
    Err("some files are still missing from the gist after 3 passes".into())
}
