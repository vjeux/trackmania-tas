//! `tinyctl round-publish` — a look-only round published IN PLACE from the devserver.
//!
//! ```text
//! tinyctl round-publish --stage DIR --out STAGED --sets wood,woodwet,magnet --jobs 6
//!     [--pairs-dir ~/u10s-keep/pairs] [--thumbs-from-live] [--results DIR] [--wood-stamp]
//!     [--token-dir DIR] [--no-mint] [--dry-run]
//! ```
//!
//! WHY (vjeux 2026-09-24): the old chain uploaded through the game on the render box, one map
//! at a time (~6 s each: 1,260 maps ≈ 2 h) and held the game lock the whole time; the box's
//! home line is slow. The upload itself is plain HTTPS to Nadeo's core service with a bearer
//! token, and the game is only needed to MINT that token (the GhostShooter plugin writes it to
//! a file). So: the box mints (a 5-second `tmdrive run`, twice per ~20 min), the token files
//! come over the bridge, and the map bytes go up from the datacenter, `--jobs` at a time.
//!
//! What a round is: `DIR/<set>/pNN/<name>.Map.Gbx` built by `tinyctl surface` (the build
//! uid, `PlaD…` etc.). For each map:
//!   1. the LIVE uid: the build uid mapped through the wipe pair files (`reset*/pairs.tsv`:
//!      old uid → new uid, chained) — an in-place update must address the record that
//!      exists; `tmmaps reuid` semantics (first 4 chars replaced, the password chunk goes);
//!   2. wood / wet wood: the 2026-02-02 exe-build stamp (the physics key those sets carry);
//!   3. the thumbnail: the live record's own (downloaded once from Nadeo — no box state);
//!   4. upload: `publish-batch --jobs N --files-only` semantics (create/update + read-back md5).
//! Re-mint + resume until every map reads back IDENTICAL (a token lives ~20 min safely).
//!
//! No shell: the wsx bridge (`Wsx`) and `tmdrive run … shootctl get /nadeotoken` are
//! subprocesses, everything else is this program.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::batch;
use crate::wsx::Wsx;

const STORE_REMOTE: &str = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
const WOOD_BUILD: &str = "2026-02-02_17_51";
const WOOD_RECORD: &str = "date=2026-02-02_17_51 git=128149-c7d05ad2551 GameVersion=3.3.0";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let stage = PathBuf::from(f("--stage").ok_or("round-publish needs --stage DIR (the build's <set>/pNN/*.Map.Gbx)")?);
    let out_root = PathBuf::from(f("--out").ok_or("round-publish needs --out DIR (the staged, live-uid'd copies)")?);
    let sets: Vec<String> = f("--sets").ok_or("round-publish needs --sets a,b,c")?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let jobs: usize = f("--jobs").and_then(|v| v.parse().ok()).unwrap_or(6).max(1);
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
    let pairs_dir = PathBuf::from(f("--pairs-dir").unwrap_or_else(|| format!("{home}/u10s-keep/pairs")));
    let results_dir = PathBuf::from(f("--results").unwrap_or_else(|| out_root.join("results").to_string_lossy().into_owned()));
    let token_dir = PathBuf::from(f("--token-dir").unwrap_or_else(|| out_root.join("tokens").to_string_lossy().into_owned()));
    let no_mint = tmmaps::cli::has(args, "--no-mint");
    let dry = tmmaps::cli::has(args, "--dry-run");
    let wsx = Wsx::new(args);
    std::fs::create_dir_all(&results_dir).map_err(|e| format!("{}: {e}", results_dir.display()))?;
    std::fs::create_dir_all(&token_dir).map_err(|e| format!("{}: {e}", token_dir.display()))?;
    let pairs = load_pairs(&pairs_dir)?;
    println!("round-publish: {} wipe pairs from {}", pairs.len(), pairs_dir.display());
    let t0 = Instant::now();
    for set in &sets {
        let src_set = stage.join(set);
        if !src_set.is_dir() {
            return Err(format!("{}: no such set directory", src_set.display()));
        }
        // 1–3: stage
        let staged = stage_set(&src_set, &out_root.join(set), set, &pairs)?;
        println!("[{:>5.0}s] {set}: {} maps staged ({} re-uid'd)", t0.elapsed().as_secs_f64(), staged.len(), staged.iter().filter(|s| s.reuided).count());
        if dry {
            continue;
        }
        // 4: upload, re-mint + resume
        let results = results_dir.join(format!("results-{set}.tsv"));
        let mut identical: BTreeMap<String, String> = read_identical(&results);
        for round in 1..=8 {
            let todo: Vec<&Staged> = staged.iter().filter(|s| !identical.contains_key(&s.path.to_string_lossy().into_owned())).collect();
            if todo.is_empty() {
                println!("[{:>5.0}s] {set}: all {} IDENTICAL", t0.elapsed().as_secs_f64(), staged.len());
                break;
            }
            if !no_mint {
                mint_tokens(&wsx, &token_dir)?;
            }
            // the live record's thumbnail onto each staged copy (once): the altered look the
            // club shows today, not the source map's picture the build carries
            if !tmmaps::cli::has(args, "--keep-thumbnails") {
                let n = live_thumbnails(&todo, &token_dir, jobs)?;
                if n > 0 {
                    println!("[{:>5.0}s] {set}: {n} thumbnails taken from the live records", t0.elapsed().as_secs_f64());
                }
            }
            let manifest = out_root.join(format!("manifest-{set}-r{round}.tsv"));
            let mut text = String::new();
            for s in &todo {
                text.push_str(&format!("{}\t{}\n", s.path.display(), s.name));
            }
            std::fs::write(&manifest, text).map_err(|e| format!("{}: {e}", manifest.display()))?;
            let round_results = results_dir.join(format!("round-{set}-r{round}.tsv"));
            let batch_args: Vec<String> = vec![
                "publish-batch".into(),
                "--manifest".into(),
                manifest.to_string_lossy().into_owned(),
                "--results".into(),
                round_results.to_string_lossy().into_owned(),
                "--outdir".into(),
                out_root.join("readback").to_string_lossy().into_owned(),
                "--jobs".into(),
                jobs.to_string(),
                "--files-only".into(),
                "--token-dir".into(),
                token_dir.to_string_lossy().into_owned(),
            ];
            println!("[{:>5.0}s] {set} round {round}: {} maps, {jobs} at a time", t0.elapsed().as_secs_f64(), todo.len());
            if let Err(e) = batch::publish_batch_cmd(&batch_args) {
                println!("  publish-batch: {e}");
            }
            for (path, line) in read_identical(&round_results) {
                identical.insert(path, line);
            }
            let mut merged = String::from("path\tname\tuid\tmapId\thow\tbytes\tmd5\tverdict\n");
            for line in identical.values() {
                merged.push_str(line);
                merged.push('\n');
            }
            std::fs::write(&results, merged).map_err(|e| format!("{}: {e}", results.display()))?;
            println!("[{:>5.0}s] {set} round {round}: {} of {} IDENTICAL so far", t0.elapsed().as_secs_f64(), identical.len(), staged.len());
        }
        let left = staged.iter().filter(|s| !identical.contains_key(&s.path.to_string_lossy().into_owned())).count();
        if left > 0 {
            return Err(format!("{set}: {left} maps still not IDENTICAL after 8 rounds — see {}", results_dir.display()));
        }
    }
    println!("round-publish done in {:.0}s", t0.elapsed().as_secs_f64());
    Ok(())
}

struct Staged {
    path: PathBuf,
    name: String,
    reuided: bool,
}

/// old uid → new uid over every wipe, in file order (`reset`, `reset2`, … `reset11`)
fn load_pairs(dir: &Path) -> Result<Vec<BTreeMap<String, String>>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map(|x| x == "tsv").unwrap_or(false)).collect();
    // reset, reset2, …, reset11: numeric order, `reset` first
    files.sort_by_key(|p| p.file_stem().and_then(|s| s.to_str()).map(|s| s.trim_start_matches("reset").parse::<u32>().unwrap_or(1)).unwrap_or(0));
    let mut out = Vec::new();
    for p in files {
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let m: BTreeMap<String, String> = text.lines().filter_map(|l| l.split_once('\t').map(|(a, b)| (a.trim().to_string(), b.split('\t').next().unwrap_or("").trim().to_string()))).filter(|(a, b)| !a.is_empty() && !b.is_empty()).collect();
        out.push(m);
    }
    Ok(out)
}

fn chain_uid(pairs: &[BTreeMap<String, String>], uid: &str) -> String {
    let mut u = uid.to_string();
    for m in pairs {
        if let Some(n) = m.get(&u) {
            u = n.clone();
        }
    }
    u
}

/// The staged copy of every map of one set: live uid, wood stamp, the live thumbnail.
fn stage_set(src_set: &Path, out_set: &Path, set: &str, pairs: &[BTreeMap<String, String>]) -> Result<Vec<Staged>, String> {
    let mut out = Vec::new();
    let mut parts: Vec<PathBuf> = std::fs::read_dir(src_set).map_err(|e| format!("{}: {e}", src_set.display()))?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
    parts.sort();
    for part in parts {
        let mut maps: Vec<PathBuf> = std::fs::read_dir(&part).map_err(|e| e.to_string())?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.to_string_lossy().ends_with(".Map.Gbx")).collect();
        maps.sort();
        let out_part = out_set.join(part.file_name().unwrap());
        std::fs::create_dir_all(&out_part).map_err(|e| e.to_string())?;
        for src in maps {
            let dst = out_part.join(src.file_name().unwrap());
            let name = src.file_name().unwrap().to_string_lossy().trim_end_matches(".Map.Gbx").to_string();
            // already staged (same source bytes newer than the copy? keep it simple: a copy exists → reuse)
            if dst.exists() {
                out.push(Staged { path: dst, name, reuided: false });
                continue;
            }
            let hdr = tmmaps::header::read(src.to_str().unwrap()).map_err(|e| format!("{}: {e}", src.display()))?;
            let live = chain_uid(pairs, &hdr.uid);
            // pass 1: the live uid (a rename; `write_to` also drops the stale password chunk)
            let mut m = tmmaps::map::MapFile::load(&src);
            let mut reuided = false;
            if live != hdr.uid {
                let new = format!("{}{}", &live[..4], &hdr.uid[4..]);
                m.set_map_uid(&new);
                reuided = true;
            }
            m.write_to(&dst).map_err(|e| format!("{}: {e}", dst.display()))?;
            // pass 2 (wood / wet wood): the exe-build stamp — variable-length splices, which
            // the writer wants on a fresh load, not mixed with renames
            if set == "wood" || set == "woodwet" {
                let mut m2 = tmmaps::map::MapFile::load(&dst);
                m2.set_exebuild(WOOD_BUILD, None);
                let _ = m2.set_build_record(WOOD_RECORD);
                m2.write_to(&dst).map_err(|e| format!("{}: {e}", dst.display()))?;
            }
            out.push(Staged { path: dst, name, reuided });
        }
    }
    Ok(out)
}

fn read_identical(results: &Path) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string(results) {
        for line in text.lines().skip(1) {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() >= 8 && c[7] == "IDENTICAL" {
                m.insert(c[0].to_string(), line.to_string());
            }
        }
    }
    m
}

/// The plugin mints both tokens under the game lock (a few seconds), the files come over
/// the bridge into `token_dir` (`token-NadeoServices.txt`, `token-NadeoLiveServices.txt`).
fn mint_tokens(wsx: &Wsx, token_dir: &Path) -> Result<(), String> {
    // The box may be held by another job for a long time, and one bridge call is capped
    // at 150 s: wait for the box in ≤120-second slices, then mint in one short call
    // (the two `tmdrive run` steps take a few seconds each; a lost race just loops).
    let env = "cd /home/vjeux/shoot/hunt; export TM_SESSION=080a15b8-2c6a-4fff-9d8a-db7d0072ee43; export TM_SESSION_TITLE=\"Tiny u10s publish\"; export PATH=/home/vjeux/bin:$PATH";
    let t0 = Instant::now();
    loop {
        // the game itself: launched as its own lock step when it is not running
        let up = wsx.sh("tasklist.exe 2>/dev/null | grep -ci trackmania").unwrap_or_default().trim().to_string();
        if up == "0" {
            println!("  mint: the game is not running — launching (own lock step)");
            let _ = wsx.sh(&format!("{env}; sed -i \"s/^DeveloperMode=false/DeveloperMode=true/\" /mnt/c/Users/vjeux/OpenplanetNext/Settings.ini; tmdrive launch --timeout 120 --purpose \"mint (launch)\" --wait 100"));
        }
        let out = wsx.sh(&format!("{env}; tmdrive wait --timeout 120 > /dev/null 2>&1; echo RC=$?")).unwrap_or_default();
        if out.contains("RC=0") {
            let mint = format!(
                "{env}; rm -f {STORE_REMOTE}/token-*.txt; tmdrive run --purpose \"mint Nadeo tokens\" --wait 60 -- /home/vjeux/bin/shootctl get /nadeotoken?aud=NadeoServices > /dev/null 2>&1; echo RC1=$?; tmdrive run --purpose \"mint Nadeo tokens\" --wait 60 -- /home/vjeux/bin/shootctl get /nadeotoken?aud=NadeoLiveServices > /dev/null 2>&1; echo RC2=$?; for i in 1 2 3 4 5 6 7 8 9 10; do [ -s {STORE_REMOTE}/token-NadeoLiveServices.txt ] && [ -s {STORE_REMOTE}/token-NadeoServices.txt ] && break; sleep 1; done; ls {STORE_REMOTE}/token-*.txt 2>/dev/null | wc -l"
            );
            let r = wsx.sh(&mint).unwrap_or_default();
            if r.lines().last().map(|l| l.trim() == "2").unwrap_or(false) {
                break;
            }
            println!("  mint: no token files yet ({}) — retrying", r.replace('\n', " ").trim());
        } else if t0.elapsed().as_secs() % 600 < 130 {
            println!("  mint: waiting for the box ({}s)", t0.elapsed().as_secs());
        }
        if t0.elapsed().as_secs() > 3600 {
            return Err("mint: no tokens after 60 min (box held?)".into());
        }
    }
    for aud in ["NadeoServices", "NadeoLiveServices"] {
        let local = token_dir.join(format!("token-{aud}.txt"));
        let n = wsx.pull(&format!("{STORE_REMOTE}/token-{aud}.txt"), &local)?;
        if n < 20 {
            return Err(format!("token-{aud}.txt: {n} bytes"));
        }
        // the files' mtime is what `fresh_tokens` judges: touched now
        let bytes = std::fs::read(&local).map_err(|e| e.to_string())?;
        std::fs::write(&local, bytes).map_err(|e| e.to_string())?;
    }
    println!("tokens minted and pulled to {} ({}s)", token_dir.display(), t0.elapsed().as_secs());
    Ok(())
}

/// For every staged map without a `.thumb.ok` marker: GET the live record by uid, download
/// the stored file, splice its `<Thumbnail.jpg>` into the staged copy. `jobs` at a time.
fn live_thumbnails(todo: &[&Staged], token_dir: &Path, jobs: usize) -> Result<usize, String> {
    let (core, _live) = crate::nadeo::fresh_tokens_in(token_dir.to_str().unwrap_or("")).ok_or("no fresh tokens for the thumbnail pass")?;
    let auth = format!("Authorization: {core}");
    let pending: Vec<&Staged> = todo.iter().copied().filter(|s| !s.path.with_extension("thumb.ok").exists()).collect();
    if pending.is_empty() {
        return Ok(0);
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|sc| {
        for _ in 0..jobs.min(pending.len()) {
            let (pending, next, done, auth) = (&pending, &next, &done, &auth);
            sc.spawn(move || loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= pending.len() {
                    break;
                }
                let st = pending[i];
                if let Err(e) = live_thumbnail_one(st, auth) {
                    println!("  thumbnail {}: {e}", st.name);
                } else {
                    done.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            });
        }
    });
    Ok(done.into_inner())
}

fn live_thumbnail_one(st: &Staged, auth: &str) -> Result<(), String> {
    let hdr = tmmaps::header::read(st.path.to_str().unwrap()).map_err(|e| e.to_string())?;
    let curl = |args: &[&str]| -> Result<Vec<u8>, String> {
        let out = std::process::Command::new("curl").arg("-s").arg("--max-time").arg("300").args(args).output().map_err(|e| format!("curl: {e}"))?;
        Ok(out.stdout)
    };
    let rec = String::from_utf8_lossy(&curl(&["-H", auth, &format!("{}/maps/?mapUidList={}", crate::publish::CORE, hdr.uid)])?).into_owned();
    let Some(url) = crate::publish::json_str_pub(&rec, "fileUrl") else {
        // no live record (a map published for the first time): the build's picture stays
        std::fs::write(st.path.with_extension("thumb.ok"), "no live record").map_err(|e| e.to_string())?;
        return Ok(());
    };
    let bytes = curl(&["-L", "-H", auth, &url])?;
    let start = bytes.windows(15).position(|w| w == b"<Thumbnail.jpg>").ok_or("live file: no <Thumbnail.jpg>")? + 15;
    let end = start + bytes[start..].windows(16).position(|w| w == b"</Thumbnail.jpg>").ok_or("live file: no </Thumbnail.jpg>")?;
    let jpeg = &bytes[start..end];
    if jpeg.len() < 1000 {
        return Err(format!("live thumbnail is {} bytes", jpeg.len()));
    }
    let mut m = tmmaps::map::MapFile::load(&st.path);
    m.set_thumbnail(jpeg)?;
    let tmp = st.path.with_extension("thumb.tmp.Map.Gbx");
    m.write_to(&tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &st.path).map_err(|e| e.to_string())?;
    std::fs::write(st.path.with_extension("thumb.ok"), format!("{} bytes", jpeg.len())).map_err(|e| e.to_string())?;
    Ok(())
}
