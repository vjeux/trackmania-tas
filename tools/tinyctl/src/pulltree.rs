//! `tinyctl pull-tree REMOTE_DIR LOCAL_DIR [--parts p1,p2,…] [--maps-only] [--keep-tars]` — a
//! built set's tree off the render box, one PART at a time (2026-10-17).
//!
//! `wsx pull` buffers the whole file in memory on both ends and a bridge call has a
//! ceiling: the 6.8 GB tar of a 975-map set sat at 1.6 GB after seven minutes and
//! would have died at the timeout. The tree is `REMOTE_DIR/pNN/<map dirs>`, so this
//! tars each `pNN` on the box (uncompressed: Gbx is already compressed), pulls that
//! ~170 MB, extracts it under `LOCAL_DIR` and deletes the remote tar — a part a
//! minute, resumable: a part whose directory already exists locally with the same
//! file count is skipped, so a second run finishes what the first left.
//!
//! `--maps-only`: just the `*.Map.Gbx` files and each map's `report.tsv` (what the
//! live-vs-build diff and the publish need) — a quarter of the bytes (a part is
//! 170 MB with its item libraries and logs, ~45 MB without).
//!
//! Writes `LOCAL_DIR/pull-tree.log` (one line per part: files, bytes, seconds).

use std::path::Path;
use std::time::Instant;

use crate::wsx::Wsx;

pub fn cmd(args: &[String]) -> Result<(), String> {
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let remote = pos.first().ok_or("pull-tree REMOTE_DIR LOCAL_DIR")?.to_string();
    let local = Path::new(pos.get(1).ok_or("pull-tree REMOTE_DIR LOCAL_DIR")?.as_str()).to_path_buf();
    let only: Option<Vec<String>> = tmmaps::cli::flag(args, "--parts").map(|s| s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect());
    let keep = tmmaps::cli::has(args, "--keep-tars");
    let maps_only = tmmaps::cli::has(args, "--maps-only");
    // the find filter for both the count and the tar
    let filter = if maps_only { " \\( -name '*.Map.Gbx' -o -name 'report.tsv' \\)" } else { "" };
    let wsx = Wsx::new(args);
    std::fs::create_dir_all(&local).map_err(|e| format!("{}: {e}", local.display()))?;
    let t0 = Instant::now();
    // the parts, in order, with their file counts
    let listing = wsx.sh(&format!("cd '{remote}' && for p in $(ls -d p* | sort -V); do echo \"$p $(find \"$p\" -type f{filter} | wc -l)\"; done"))?;
    let mut parts: Vec<(String, usize)> = Vec::new();
    for line in listing.lines() {
        let mut it = line.split_whitespace();
        if let (Some(p), Some(n)) = (it.next(), it.next().and_then(|n| n.parse().ok())) {
            if only.as_ref().map(|o| o.iter().any(|x| x == p)).unwrap_or(true) {
                parts.push((p.to_string(), n));
            }
        }
    }
    if parts.is_empty() {
        return Err(format!("{remote}: no pNN parts"));
    }
    let log = local.join("pull-tree.log");
    let mut done = 0usize;
    let mut skipped = 0usize;
    for (p, n) in &parts {
        let dst = local.join(p);
        if dst.is_dir() {
            let have = walk_count(&dst);
            if have == *n && *n > 0 {
                skipped += 1;
                continue;
            }
        }
        let t1 = Instant::now();
        let tar_remote = format!("{remote}/.pull-{p}.tar");
        if maps_only {
            wsx.sh(&format!("cd '{remote}' && rm -f '{tar_remote}' && find '{p}' -type f{filter} -print0 | tar cf '{tar_remote}' --null -T -"))?;
        } else {
            wsx.sh(&format!("cd '{remote}' && rm -f '{tar_remote}' && tar cf '{tar_remote}' '{p}'"))?;
        }
        let tar_local = local.join(format!(".pull-{p}.tar"));
        let bytes = wsx.pull(&tar_remote, &tar_local)?;
        let st = std::process::Command::new("tar").args(["xf", tar_local.to_str().unwrap_or(""), "-C", local.to_str().unwrap_or("")]).status().map_err(|e| format!("tar xf: {e}"))?;
        if !st.success() {
            return Err(format!("{p}: tar xf failed (the tar is kept at {})", tar_local.display()));
        }
        let have = walk_count(&dst);
        if have != *n {
            return Err(format!("{p}: {have} files extracted, the box has {n}"));
        }
        if !keep {
            let _ = std::fs::remove_file(&tar_local);
        }
        wsx.sh(&format!("rm -f '{tar_remote}'"))?;
        done += 1;
        let line = format!("{p}\t{n} files\t{bytes} bytes\t{:.0}s", t1.elapsed().as_secs_f64());
        println!("[{:>5.0}s] {line}", t0.elapsed().as_secs_f64());
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&log).and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{line}")
        });
    }
    // the build ledgers (`build.tsv`, `build-retry.tsv`) live one level UP, at the
    // rollout-build root (`REMOTE_DIR/..`), and `rollout-publish plan` reads them from
    // its `--build ROOT`: pulled too, with the box's paths rewritten to the local root
    // (2026-10-19: a maps-only pull left the plan with "no built file" for every map).
    if let (Some(remote_root), Some(local_root)) = (remote.rsplit_once('/').map(|(r, _)| r.to_string()), local.parent().map(|p| p.to_path_buf())) {
        for tsv in ["build.tsv", "build-retry.tsv"] {
            let rp = format!("{remote_root}/{tsv}");
            let Ok(text) = wsx.sh(&format!("cat '{rp}' 2>/dev/null")) else { continue };
            if text.trim().is_empty() {
                continue;
            }
            let rewritten = text.replace(&format!("{remote_root}/"), &format!("{}/", local_root.display()));
            let lp = local_root.join(tsv);
            // merge: rows of other sets already pulled stay
            let mut rows: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
            let mut header = String::new();
            for existing in std::fs::read_to_string(&lp).unwrap_or_default().lines().chain(rewritten.lines()) {
                if existing.starts_with("set\t") {
                    header = existing.to_string();
                    continue;
                }
                let key: String = existing.split('\t').take(2).collect::<Vec<_>>().join("\t");
                if !key.is_empty() {
                    rows.insert(key, existing.to_string());
                }
            }
            let mut out = String::new();
            if !header.is_empty() {
                out.push_str(&header);
                out.push('\n');
            }
            for r in rows.values() {
                out.push_str(r);
                out.push('\n');
            }
            if let Err(e) = std::fs::write(&lp, out) {
                eprintln!("{}: {e}", lp.display());
            } else {
                println!("ledger {tsv}: {} row(s) → {}", rows.len(), lp.display());
            }
        }
    }
    println!("pull-tree: {done} part(s) pulled, {skipped} already here, {} total, {:.0}s → {}", parts.len(), t0.elapsed().as_secs_f64(), local.display());
    Ok(())
}

fn walk_count(dir: &Path) -> usize {
    let mut n = 0usize;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                n += 1;
            }
        }
    }
    n
}
