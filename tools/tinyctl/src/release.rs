//! `tinyctl release-upload` — the release assets of a club set onto GitHub
//! through the render box (the only host with vjeux's `gh` login and a route
//! to GitHub that keeps its session): each zip pushed to the box, uploaded
//! with `gh release upload --clobber`, the box copy deleted, N streams at a
//! time; at the end `gh release view --json assets` is compared against the
//! local sizes (2026-09-13, the giant U10S release).
//!
//! ```text
//! tinyctl release-upload --dir DIST --tag giant-u10s-maps [--repo vjeux/trackmania-tas]
//!                        [--jobs 3] [--box-dir /home/vjeux/shoot/giant/dist] [--only-missing] [--wsx P] [-v]
//! ```

use std::path::PathBuf;
use std::time::Instant;

use crate::wsx::Wsx;

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let dir = PathBuf::from(f("--dir").ok_or("release-upload needs --dir DIST (the zips)")?);
    let tag = f("--tag").ok_or("release-upload needs --tag TAG")?;
    let repo = f("--repo").unwrap_or_else(|| "vjeux/trackmania-tas".into());
    let jobs: usize = f("--jobs").and_then(|j| j.parse().ok()).unwrap_or(3).max(1);
    let box_dir = f("--box-dir").unwrap_or_else(|| "/home/vjeux/shoot/giant/dist".into());
    let only_missing = tmmaps::cli::has(args, "--only-missing");
    let wsx = Wsx::new(args);
    let mut zips: Vec<(String, PathBuf, u64)> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "zip").unwrap_or(false))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), p.clone(), std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0)))
        .collect();
    zips.sort();
    if zips.is_empty() {
        return Err(format!("{}: no zips", dir.display()));
    }
    // what the release already holds (name -> size)
    let remote = remote_assets(&wsx, &tag, &repo)?;
    let todo: Vec<(String, PathBuf, u64)> = zips.iter().filter(|(n, _, s)| !(only_missing && remote.get(n) == Some(s))).cloned().collect();
    println!("{} zips in {}, {} already on the release with the right size, {} to upload, {} streams", zips.len(), dir.display(), zips.len() - todo.len(), todo.len(), jobs);
    wsx.sh(&format!("mkdir -p '{box_dir}'"))?;
    let queue = std::sync::Mutex::new(std::collections::VecDeque::from(todo.clone()));
    let results = std::sync::Mutex::new(Vec::<String>::new());
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..jobs.min(todo.len().max(1)) {
            s.spawn(|| loop {
                let (name, path, size) = match queue.lock().unwrap().pop_front() {
                    Some(z) => z,
                    None => break,
                };
                let t1 = Instant::now();
                let remote_path = format!("{box_dir}/{name}");
                let r: Result<(), String> = (|| {
                    wsx.push(&path, &remote_path)?;
                    // gh prints the asset URL on success; --clobber replaces an earlier copy
                    let out = wsx.sh(&format!("cd /home/vjeux && ./bin/gh release upload '{tag}' '{remote_path}' -R {repo} --clobber 2>&1; rc=$?; rm -f '{remote_path}'; exit $rc"))?;
                    if out.contains("error") || out.contains("failed") {
                        return Err(format!("gh: {}", out.trim()));
                    }
                    Ok(())
                })();
                let line = match r {
                    Ok(()) => format!("{name}\tOK\t{size}\t{:.0}s", t1.elapsed().as_secs_f64()),
                    Err(e) => {
                        let _ = wsx.sh(&format!("rm -f '{remote_path}'"));
                        format!("{name}\tFAILED\t{size}\t{:.0}s\t{}", t1.elapsed().as_secs_f64(), e.lines().next().unwrap_or(""))
                    }
                };
                eprintln!("[{:>5.0}s] {line}", t0.elapsed().as_secs_f64());
                results.lock().unwrap().push(line);
            });
        }
    });
    let mut lines = results.into_inner().unwrap();
    lines.sort();
    // the verification: every local zip on the release at its local size
    let remote = remote_assets(&wsx, &tag, &repo)?;
    let mut missing = Vec::new();
    for (n, _, s) in &zips {
        match remote.get(n) {
            Some(rs) if rs == s => {}
            Some(rs) => missing.push(format!("{n}: remote {rs} B, local {s} B")),
            None => missing.push(format!("{n}: not on the release")),
        }
    }
    let total: u64 = zips.iter().map(|z| z.2).sum();
    println!("{}", lines.join("\n"));
    println!("release {tag}: {} of {} assets present at the local size ({:.2} GB), {:.0} s", zips.len() - missing.len(), zips.len(), total as f64 / 1e9, t0.elapsed().as_secs_f64());
    if !missing.is_empty() {
        return Err(format!("{} asset(s) wrong or missing: {}", missing.len(), missing.join("; ")));
    }
    Ok(())
}

/// `gh release view --json assets` → name -> size.
fn remote_assets(wsx: &Wsx, tag: &str, repo: &str) -> Result<std::collections::BTreeMap<String, u64>, String> {
    let out = wsx.sh(&format!("cd /home/vjeux && ./bin/gh release view '{tag}' -R {repo} --json assets --jq '.assets[] | \"\\(.name)\\t\\(.size)\"'"))?;
    Ok(out.lines().filter_map(|l| l.split_once('\t').and_then(|(n, s)| s.trim().parse::<u64>().ok().map(|s| (n.trim().to_string(), s)))).collect())
}
