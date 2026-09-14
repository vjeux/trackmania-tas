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
//!                        [--extract-list L.tsv --extract-to /home/vjeux/shoot/giant/hunt]
//! tinyctl hunt-update --extract-list L.tsv --extract-to DIR --results LOCAL.tsv
//!                     [--box-tinyctl /home/vjeux/shoot/giant/tinyctl] [--wsx P] [-v]
//! ```
//!
//! `--extract-list` (rows `zip<TAB>entry`): while a zip is on the box its listed
//! maps are unzipped into `--extract-to/<part>/` — the Nadeo copies of a set
//! (Everios' hunt-club rooms hold the giant maps by uid) are updated from the
//! same push as the release asset, one bridge pass for both (2026-09-14). A zip
//! already on the release at its size is pushed for the extraction alone.
//! `hunt-update` then writes the `publish-batch` manifest (`path<TAB>name`, the
//! name = the entry without `.Map.Gbx`), runs the batch detached on the box (an
//! update by uid per row + the stored-bytes md5 readback) and pulls the results.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::wsx::Wsx;

/// `zip<TAB>entry` rows → zip name -> entries.
fn read_extract_list(path: &Path) -> Result<BTreeMap<String, Vec<String>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let (z, e) = l.split_once('\t').ok_or_else(|| format!("{}: row without a tab: {l}", path.display()))?;
        m.entry(z.trim().to_string()).or_default().push(e.trim().to_string());
    }
    Ok(m)
}

/// The `partNN` folder of a zip named `<Variant>U10S-partNN.zip`.
fn part_of(zip: &str) -> String {
    zip.rsplit_once('-').map(|(_, r)| r.trim_end_matches(".zip").to_string()).unwrap_or_else(|| zip.trim_end_matches(".zip").to_string())
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let dir = PathBuf::from(f("--dir").ok_or("release-upload needs --dir DIST (the zips)")?);
    let tag = f("--tag").ok_or("release-upload needs --tag TAG")?;
    let repo = f("--repo").unwrap_or_else(|| "vjeux/trackmania-tas".into());
    let jobs: usize = f("--jobs").and_then(|j| j.parse().ok()).unwrap_or(3).max(1);
    let box_dir = f("--box-dir").unwrap_or_else(|| "/home/vjeux/shoot/giant/dist".into());
    let only_missing = tmmaps::cli::has(args, "--only-missing");
    let extract_to = f("--extract-to");
    let extract: BTreeMap<String, Vec<String>> = match f("--extract-list") {
        Some(l) => read_extract_list(&PathBuf::from(l))?,
        None => Default::default(),
    };
    if !extract.is_empty() && extract_to.is_none() {
        return Err("--extract-list needs --extract-to DIR (on the box)".into());
    }
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
    // (zip, path, size, upload?) — a zip on the release at its size is skipped, unless
    // it has maps to extract: then it is pushed and unzipped, not uploaded again
    let todo: Vec<(String, PathBuf, u64, bool)> = zips
        .iter()
        .filter_map(|(n, p, s)| {
            let up = !(only_missing && remote.get(n) == Some(s));
            (up || extract.contains_key(n)).then(|| (n.clone(), p.clone(), *s, up))
        })
        .collect();
    println!(
        "{} zips in {}, {} already on the release with the right size, {} to push ({} to upload, {} extraction-only), {} streams",
        zips.len(),
        dir.display(),
        zips.iter().filter(|(n, _, s)| remote.get(n) == Some(s)).count(),
        todo.len(),
        todo.iter().filter(|t| t.3).count(),
        todo.iter().filter(|t| !t.3).count(),
        jobs
    );
    wsx.sh(&format!("mkdir -p '{box_dir}'"))?;
    let queue = std::sync::Mutex::new(std::collections::VecDeque::from(todo.clone()));
    let results = std::sync::Mutex::new(Vec::<String>::new());
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..jobs.min(todo.len().max(1)) {
            s.spawn(|| loop {
                let (name, path, size, upload) = match queue.lock().unwrap().pop_front() {
                    Some(z) => z,
                    None => break,
                };
                let t1 = Instant::now();
                let remote_path = format!("{box_dir}/{name}");
                let r: Result<(), String> = (|| {
                    wsx.push(&path, &remote_path)?;
                    if let (Some(entries), Some(dir)) = (extract.get(&name), extract_to.as_deref()) {
                        // unzip everything into a scratch dir (no wildcard escaping of
                        // `[Giant]` for unzip's patterns), keep the listed maps
                        let part = part_of(&name);
                        let tmp = format!("{dir}/.tmp-{part}");
                        let dest = format!("{dir}/{part}");
                        let moves: Vec<String> = entries.iter().map(|e| format!("'{tmp}/{}'", e.replace('\'', ""))).collect();
                        let out = wsx.sh(&format!("rm -rf '{tmp}' && mkdir -p '{tmp}' '{dest}' && unzip -o -q '{remote_path}' -d '{tmp}' && mv -f {} '{dest}/' && rm -rf '{tmp}' && ls '{dest}' | wc -l", moves.join(" ")))?;
                        eprintln!("  {name}: {} maps extracted to {dest} ({} there now)", entries.len(), out.trim());
                    }
                    if !upload {
                        wsx.sh(&format!("rm -f '{remote_path}'"))?;
                        return Ok(());
                    }
                    // gh runs DETACHED on the box: a 120 MB upload to GitHub can take
                    // longer than the bridge's 90 s answer window, and a command that
                    // does not answer is retried by wsx — two uploads of one asset at
                    // once (2026-09-13, parts 05/06). The done file carries gh's rc.
                    let done = format!("{remote_path}.done");
                    let log = format!("{remote_path}.log");
                    wsx.sh(&format!("rm -f '{done}'; cd /home/vjeux && nohup setsid sh -c './bin/gh release upload \"{tag}\" \"{remote_path}\" -R {repo} --clobber; rc=$?; if [ $rc = 0 ]; then echo OK uploaded > \"{done}\"; else echo FAILED rc=$rc > \"{done}\"; fi; rm -f \"{remote_path}\"' > '{log}' 2>&1 < /dev/null & echo started"))?;
                    wsx.wait_done(&done, &log, Duration::from_secs(1800), &format!("gh upload {name}"))?;
                    let _ = wsx.sh(&format!("rm -f '{done}' '{log}'"));
                    Ok(())
                })();
                let line = match r {
                    Ok(()) => format!("{name}\t{}\t{size}\t{:.0}s", if upload { "OK" } else { "EXTRACTED" }, t1.elapsed().as_secs_f64()),
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
fn remote_assets(wsx: &Wsx, tag: &str, repo: &str) -> Result<BTreeMap<String, u64>, String> {
    let out = wsx.sh(&format!("cd /home/vjeux && ./bin/gh release view '{tag}' -R {repo} --json assets --jq '.assets[] | \"\\(.name)\\t\\(.size)\"'"))?;
    Ok(out.lines().filter_map(|l| l.split_once('\t').and_then(|(n, s)| s.trim().parse::<u64>().ok().map(|s| (n.trim().to_string(), s)))).collect())
}

/// `tinyctl hunt-update`: the extracted maps' Nadeo records updated in place.
pub fn hunt_update_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let extract = read_extract_list(&PathBuf::from(f("--extract-list").ok_or("hunt-update needs --extract-list L.tsv")?))?;
    let dir = f("--extract-to").ok_or("hunt-update needs --extract-to DIR (on the box)")?;
    let results_local = PathBuf::from(f("--results").ok_or("hunt-update needs --results LOCAL.tsv")?);
    let box_tinyctl = f("--box-tinyctl").unwrap_or_else(|| "/home/vjeux/shoot/giant/tinyctl".into());
    let wsx = Wsx::new(args);
    // the manifest: every extracted map, part by part, in zip order
    let mut manifest = String::new();
    let mut n = 0usize;
    for (zip, entries) in &extract {
        let part = part_of(zip);
        for e in entries {
            let name = e.trim_end_matches(".Map.Gbx");
            manifest.push_str(&format!("{dir}/{part}/{e}\t{name}\n"));
            n += 1;
        }
    }
    let local_manifest = results_local.with_extension("manifest.tsv");
    std::fs::write(&local_manifest, &manifest).map_err(|e| format!("{}: {e}", local_manifest.display()))?;
    let remote_manifest = format!("{dir}/manifest.tsv");
    let remote_results = format!("{dir}/results.tsv");
    wsx.push(&local_manifest, &remote_manifest)?;
    // every manifest path must be on the box before the batch starts
    let present = wsx.sh(&format!("n=0; while IFS=$(printf '\\t') read -r p _; do [ -f \"$p\" ] && n=$((n+1)); done < '{remote_manifest}'; echo $n"))?;
    let present: usize = present.trim().parse().unwrap_or(0);
    if present != n {
        return Err(format!("{present} of {n} manifest maps are on the box under {dir} — run release-upload --extract-list first"));
    }
    println!("{n} maps on the box; publish-batch (update by uid, md5 readback) starting detached");
    let t0 = Instant::now();
    wsx.sh(&format!("rm -f '{remote_results}' '{dir}/results.done'; mkdir -p '{dir}/batch'; {box_tinyctl} publish-batch --detach --manifest '{remote_manifest}' --results '{remote_results}' --outdir '{dir}/batch'"))?;
    let done = wsx.wait_done(&format!("{dir}/results.done"), &format!("{dir}/results.log"), Duration::from_secs(4 * 3600), "hunt-update publish-batch")?;
    wsx.pull(&remote_results, &results_local)?;
    let text = std::fs::read_to_string(&results_local).map_err(|e| e.to_string())?;
    let identical = text.lines().filter(|l| l.ends_with("\tIDENTICAL")).count();
    let failed = text.lines().filter(|l| l.contains("\tFAILED") || l.ends_with("\tDIFFERENT")).count();
    println!("{}", done.trim());
    println!("hunt-update: {identical} of {n} maps stored IDENTICAL, {failed} failed, {:.0} s — {}", t0.elapsed().as_secs_f64(), results_local.display());
    if identical != n {
        return Err(format!("{} map(s) not IDENTICAL", n - identical));
    }
    Ok(())
}
