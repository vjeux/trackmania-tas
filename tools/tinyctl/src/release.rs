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
//! tinyctl hunt-push --extract-list L.tsv --dist DIST --to DIR          (maps only, restartable)
//! tinyctl rezip --zip OLD.zip --replace-dir DIR --out NEW.zip          (on the box)
//! tinyctl release-rebuild --dir DIST --tag TAG --extract-list L.tsv --maps DIR
//! ```
//!
//! The bridge-thrifty form (2026-09-14): `hunt-push` the rebuilt maps alone,
//! then `release-rebuild` has the box download each v1 asset, `rezip` it with
//! those maps (byte-identical to the devserver's zip, md5-checked) and re-upload
//! it; `hunt-update` publishes the same maps to Nadeo. 0.9 GB over the bridge
//! instead of 4.2 GB.
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
                        // unzip everything into a scratch dir (the box has busybox unzip only; no wildcard escaping of
                        // `[Giant]` for unzip's patterns), keep the listed maps
                        let part = part_of(&name);
                        let tmp = format!("{dir}/.tmp-{part}");
                        let dest = format!("{dir}/{part}");
                        let moves: Vec<String> = entries.iter().map(|e| format!("'{tmp}/{}'", e.replace('\'', ""))).collect();
                        let out = wsx.sh(&format!("rm -rf '{tmp}' && mkdir -p '{tmp}' '{dest}' && U=$(command -v unzip || echo 'busybox unzip') && $U -o -q '{remote_path}' -d '{tmp}' && mv -f {} '{dest}/' && rm -rf '{tmp}' && ls '{dest}' | wc -l", moves.join(" ")))?;
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

/// `tinyctl rezip --zip OLD.zip --replace-dir DIR --out NEW.zip` (runs on the box):
/// OLD's entries with every file of DIR that shares an entry name swapped in,
/// written STORED with the same writer as `tinyctl dist` — byte-identical to the
/// devserver's zip of the same content, so the release asset can be rebuilt on
/// the box from the v1 asset (`gh release download`) plus the rebuilt maps alone
/// (2026-09-14: the bridge at 220 KB/s made a 4.2 GB re-push a 5-hour job; the
/// 184 rebuilt maps are 0.9 GB). Verified on 33 zips: every md5 matched.
pub fn rezip_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let old = PathBuf::from(f("--zip").ok_or("rezip needs --zip OLD.zip")?);
    let dir = PathBuf::from(f("--replace-dir").ok_or("rezip needs --replace-dir DIR")?);
    let out = PathBuf::from(f("--out").ok_or("rezip needs --out NEW.zip")?);
    let bytes = std::fs::read(&old).map_err(|e| format!("{}: {e}", old.display()))?;
    let mut files: BTreeMap<String, Vec<u8>> = tmmaps::header::zip_entries(&bytes).into_iter().collect();
    if files.is_empty() {
        return Err(format!("{}: no zip entries", old.display()));
    }
    let mut replaced = 0usize;
    let mut extra = 0usize;
    for e in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().into_owned();
        if !e.path().is_file() {
            continue;
        }
        let data = std::fs::read(e.path()).map_err(|er| format!("{}: {er}", e.path().display()))?;
        if files.insert(name, data).is_some() {
            replaced += 1;
        } else {
            extra += 1;
        }
    }
    let zip = tmmaps::header::stored_zip(&files);
    std::fs::write(&out, &zip).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("{}: {} entries, {replaced} replaced, {extra} added -> {} ({} B)", old.display(), files.len(), out.display(), zip.len());
    Ok(())
}

/// `tinyctl hunt-push --extract-list L.tsv --dist DIST --to DIR`: the listed maps
/// (and each part's MAPS.tsv) out of the local zips onto the box, one file at a
/// time, skipping files already there at the right size — restartable (the
/// bridge dropped the stream twice on 2026-09-14; three runs finished the 219 files).
pub fn hunt_push_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let extract = read_extract_list(&PathBuf::from(f("--extract-list").ok_or("hunt-push needs --extract-list L.tsv")?))?;
    let dist = PathBuf::from(f("--dist").ok_or("hunt-push needs --dist DIST (the local zips)")?);
    let to = f("--to").ok_or("hunt-push needs --to DIR (on the box)")?;
    let wsx = Wsx::new(args);
    let tmp = std::env::temp_dir().join(format!("hunt-push-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let (mut pushed, mut skipped, mut total) = (0usize, 0usize, 0u64);
    for (zip, entries) in &extract {
        let part = part_of(zip);
        let bytes = std::fs::read(dist.join(zip)).map_err(|e| format!("{zip}: {e}"))?;
        let files: BTreeMap<String, Vec<u8>> = tmmaps::header::zip_entries(&bytes).into_iter().collect();
        let dest = format!("{to}/{part}");
        // what is there already: name<TAB>size
        let have: BTreeMap<String, u64> = wsx
            .sh(&format!("mkdir -p '{dest}' && cd '{dest}' && for f in *; do [ -f \"$f\" ] && printf '%s\\t%s\\n' \"$f\" \"$(stat -c %s \"$f\")\"; done; true"))?
            .lines()
            .filter_map(|l| l.split_once('\t').and_then(|(n, s)| s.trim().parse::<u64>().ok().map(|s| (n.to_string(), s))))
            .collect();
        let mut wanted: Vec<&str> = entries.iter().map(|s| s.as_str()).collect();
        wanted.push("MAPS.tsv");
        for name in wanted {
            let data = files.get(name).ok_or_else(|| format!("{zip}: no entry {name}"))?;
            if have.get(name) == Some(&(data.len() as u64)) {
                skipped += 1;
                continue;
            }
            let local = tmp.join(name.replace('/', "_"));
            std::fs::write(&local, data).map_err(|e| e.to_string())?;
            wsx.push(&local, &format!("{dest}/{name}"))?;
            let _ = std::fs::remove_file(&local);
            pushed += 1;
            total += data.len() as u64;
            eprintln!("[{:>5.0}s] {part}/{name} {} B", t0.elapsed().as_secs_f64(), data.len());
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    println!("hunt-push: {pushed} files pushed ({:.1} MB), {skipped} already there, {:.0} s", total as f64 / 1e6, t0.elapsed().as_secs_f64());
    Ok(())
}

/// `tinyctl release-rebuild --dir DIST --tag TAG --extract-list L.tsv --maps DIR
/// [--box-tinyctl P] [--box-dir D] [--repo R]`: for every zip of the list, on the
/// box and detached: `gh release download` of the current asset, `rezip` with
/// the maps under `--maps/<part>/` (pushed by `hunt-push`), md5 against the
/// local zip, `gh release upload --clobber`; polled from here. Zips already on
/// the release at the local size are skipped. Bridge traffic: commands only.
pub fn release_rebuild_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let dir = PathBuf::from(f("--dir").ok_or("release-rebuild needs --dir DIST (the local zips)")?);
    let tag = f("--tag").ok_or("release-rebuild needs --tag TAG")?;
    let repo = f("--repo").unwrap_or_else(|| "vjeux/trackmania-tas".into());
    let extract = read_extract_list(&PathBuf::from(f("--extract-list").ok_or("release-rebuild needs --extract-list L.tsv")?))?;
    let maps = f("--maps").ok_or("release-rebuild needs --maps DIR (the pushed maps, on the box)")?;
    let box_tinyctl = f("--box-tinyctl").unwrap_or_else(|| "/home/vjeux/shoot/giant/tinyctl".into());
    let box_dir = f("--box-dir").unwrap_or_else(|| "/home/vjeux/shoot/giant/dist".into());
    let wsx = Wsx::new(args);
    let remote = remote_assets(&wsx, &tag, &repo)?;
    let t0 = Instant::now();
    let mut lines: Vec<String> = Vec::new();
    for zip in extract.keys() {
        let local = dir.join(zip);
        let size = std::fs::metadata(&local).map(|m| m.len()).map_err(|e| format!("{}: {e}", local.display()))?;
        if remote.get(zip) == Some(&size) {
            lines.push(format!("{zip}\tSKIP\t{size}\talready on the release at the local size"));
            continue;
        }
        let md5 = crate::publish::md5_hex(&std::fs::read(&local).map_err(|e| format!("{}: {e}", local.display()))?);
        let part = part_of(zip);
        let t1 = Instant::now();
        let v1 = format!("{box_dir}/v1-{zip}");
        let v2 = format!("{box_dir}/{zip}");
        let done = format!("{box_dir}/{zip}.done");
        let log = format!("{box_dir}/{zip}.log");
        let script = format!(
            "cd /home/vjeux && rm -f '{v1}' '{v2}' && ./bin/gh release download '{tag}' -R {repo} -p '{zip}' -O '{v1}' && {box_tinyctl} rezip --zip '{v1}' --replace-dir '{maps}/{part}' --out '{v2}' && rm -f '{v1}' && m=$(md5sum '{v2}' | cut -c1-32) && if [ \"$m\" != '{md5}' ]; then echo FAILED md5 $m > '{done}'; rm -f '{v2}'; exit 1; fi && ./bin/gh release upload '{tag}' '{v2}' -R {repo} --clobber && echo OK $m > '{done}' || echo FAILED rc=$? > '{done}'; rm -f '{v2}' '{v1}'"
        );
        // the script travels base64-encoded: inside a double-quoted `sh -c` the
        // outer shell expanded `$(md5sum …)` before the file existed (2026-09-14)
        let b64 = base64_std(script.as_bytes());
        let script_path = format!("{box_dir}/{zip}.sh");
        wsx.sh(&format!("rm -f '{done}'; mkdir -p '{box_dir}'; echo {b64} | base64 -d > '{script_path}'; nohup setsid sh '{script_path}' > '{log}' 2>&1 < /dev/null & echo started"))?;
        let line = match wsx.wait_done(&done, &log, Duration::from_secs(3600), &format!("release-rebuild {zip}")) {
            Ok(text) => format!("{zip}\tOK\t{size}\t{:.0}s\t{}", t1.elapsed().as_secs_f64(), text.trim()),
            Err(e) => format!("{zip}\tFAILED\t{size}\t{:.0}s\t{}", t1.elapsed().as_secs_f64(), e.lines().next().unwrap_or("")),
        };
        let _ = wsx.sh(&format!("rm -f '{done}' '{log}' '{script_path}'"));
        eprintln!("[{:>5.0}s] {line}", t0.elapsed().as_secs_f64());
        lines.push(line);
    }
    let remote = remote_assets(&wsx, &tag, &repo)?;
    let mut bad = 0usize;
    for zip in extract.keys() {
        let size = std::fs::metadata(dir.join(zip)).map(|m| m.len()).unwrap_or(0);
        if remote.get(zip) != Some(&size) {
            bad += 1;
            lines.push(format!("{zip}\tMISMATCH\t{size}\tremote {:?}", remote.get(zip)));
        }
    }
    println!("{}", lines.join("\n"));
    println!("release-rebuild: {} zips, {bad} not at the local size on the release, {:.0} s", extract.len(), t0.elapsed().as_secs_f64());
    if bad > 0 {
        return Err(format!("{bad} asset(s) wrong"));
    }
    Ok(())
}

/// Standard base64 (no external crate: the CLI keeps its dependency list short).
fn base64_std(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}
