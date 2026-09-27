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
pub fn mint_tokens(wsx: &Wsx, token_dir: &Path) -> Result<(), String> {
    // The box may be held by another job for a long time, and one bridge call is capped
    // at 150 s: wait for the box in ≤120-second slices, then mint in one short call
    // (the two `tmdrive run` steps take a few seconds each; a lost race just loops).
    let env = "cd /home/vjeux/shoot/hunt; export TM_SESSION=080a15b8-2c6a-4fff-9d8a-db7d0072ee43; export TM_SESSION_TITLE=\"Tiny u10s publish\"; export PATH=/home/vjeux/bin:$PATH";
    let t0 = Instant::now();
    let mut dead_plugin = 0u32;
    // the game the MINT launched (or relaunched) is closed again once the tokens are
    // pulled: the other lanes on the shared box need the game ABSENT to launch their own
    // (MK64 sessions, 2026-09-26 13:52) — a game that was already running is left alone
    let mut launched_here = false;
    loop {
        // another session's DIRTY-RECOVERY marker (MK64 native runs, 2026-09-26): the box is
        // mid-restore — wait, never launch or mint over it
        let marker = wsx.sh("test -e /mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/TmDriveRecovery.v1 && echo DIRTY || echo clean").unwrap_or_default();
        if marker.contains("DIRTY") {
            println!("  mint: TmDriveRecovery.v1 marker present (another session restoring the box) — waiting");
            std::thread::sleep(std::time::Duration::from_secs(60));
            if t0.elapsed().as_secs() > 3600 {
                return Err("mint: recovery marker still present after 60 min".into());
            }
            continue;
        }
        // the game itself: launched as its own lock step when it is not running
        let up = wsx.sh("tasklist.exe 2>/dev/null | grep -ci trackmania").unwrap_or_default().trim().to_string();
        // a game left running WITHOUT Openplanet by a session that has released the lock
        // (the MK64 session's "without Openplanet" runs, 2026-09-26) never answers the token
        // route: after 3 answered-but-empty mints, relaunch it as own lock steps
        if up != "0" && dead_plugin >= 3 {
            launched_here = true;
            println!("  mint: the game answers no token {dead_plugin}× (no plugin?) — relaunching (own lock steps)");
            let _ = wsx.sh(&format!("{env}; tmdrive kill --purpose \"mint: relaunch with Openplanet\" --wait 100"));
            let _ = wsx.sh(&format!("{env}; sed -i \"s/^DeveloperMode=false/DeveloperMode=true/\" /mnt/c/Users/vjeux/OpenplanetNext/Settings.ini; tmdrive launch --timeout 120 --purpose \"mint: relaunch with Openplanet\" --wait 100"));
            dead_plugin = 0;
        }
        if up == "0" {
            launched_here = true;
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
            dead_plugin += 1;
            println!("  mint: no token files yet ({}) — retrying", r.replace('\n', " ").trim());
        } else if t0.elapsed().as_secs() % 600 < 130 {
            println!("  mint: waiting for the box ({}s)", t0.elapsed().as_secs());
        }
        if t0.elapsed().as_secs() > 3600 {
            return Err("mint: no tokens after 60 min (box held?)".into());
        }
    }
    if launched_here {
        let _ = wsx.sh(&format!("{env}; tmdrive kill --purpose \"mint: closing the game it launched\" --wait 60 > /dev/null 2>&1; true"));
        println!("  mint: the game launched for the mint is closed again");
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

/// `tinyctl port-fix-publish --fixed-dir D --club C --campaign K --campaign-name N --prefix StaF
/// --out-dir O [--jobs 6]` — the fixed Stadium ports (`mapgeom route-fix` output, the LIVE uid in
/// their headers) go up under FRESH uids (the console caches a uid's first bytes: json9841's
/// stale-cache finding of 2026-09-23, records reset agreed), and the campaign playlist is
/// rewritten with the new uids in their slots. Tokens minted on the box, bytes from here.
pub fn port_fix_publish(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let fixed_dir = PathBuf::from(f("--fixed-dir").ok_or("--fixed-dir D")?);
    let club = f("--club").ok_or("--club C")?;
    let campaign = f("--campaign").ok_or("--campaign K")?;
    let cname = f("--campaign-name").ok_or("--campaign-name N")?;
    let prefix = f("--prefix").ok_or("--prefix XXXX (4 chars)")?;
    if prefix.len() != 4 {
        return Err("--prefix is 4 characters".into());
    }
    let out_dir = PathBuf::from(f("--out-dir").ok_or("--out-dir O")?);
    let jobs = f("--jobs").unwrap_or_else(|| "6".into());
    let only: Option<Vec<String>> = f("--only").map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
    let token_dir = out_dir.join("tokens");
    let staged_dir = out_dir.join("staged");
    std::fs::create_dir_all(&token_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&staged_dir).map_err(|e| e.to_string())?;
    let wsx = Wsx::new(args);
    mint_tokens(&wsx, &token_dir)?;
    let (core, live) = crate::nadeo::fresh_tokens_in(token_dir.to_str().unwrap()).ok_or("no tokens after the mint")?;
    let curl = |args: &[&str]| -> Result<String, String> {
        let out = std::process::Command::new("curl").arg("-s").arg("--max-time").arg("120").args(args).output().map_err(|e| format!("curl: {e}"))?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    // the live playlist, in order
    let camp = curl(&["-H", &format!("Authorization: {live}"), &format!("{}/api/token/club/{club}/campaign/{campaign}", crate::publish::LIVE)])?;
    let playlist: Vec<String> = crate::publish::json_strs(&camp, "mapUid");
    if playlist.is_empty() {
        return Err(format!("campaign {campaign}: no playlist read ({})", camp.chars().take(200).collect::<String>()));
    }
    println!("campaign {campaign} `{cname}`: {} maps in the playlist", playlist.len());
    // the live records' NAMES (a rebuilt layer carries a different uid than the live one —
    // the layers were derived from earlier port uids — so files match their slot by name)
    let strip = |n: &str| -> String {
        let mut out = String::new();
        let mut it = n.chars().peekable();
        while let Some(c) = it.next() {
            if c == '$' {
                match it.peek() {
                    Some(&d) if d.is_ascii_hexdigit() => { for _ in 0..3 { it.next(); } }
                    Some(_) => { it.next(); }
                    None => {}
                }
                continue;
            }
            out.push(c);
        }
        out.trim().to_string()
    };
    let mut live_names: Vec<String> = vec![String::new(); playlist.len()];
    for chunk in playlist.chunks(20) {
        let rec = curl(&["-H", &format!("Authorization: {core}"), &format!("{}/maps/?mapUidList={}", crate::publish::CORE, chunk.join(","))])?;
        // records come back in any order: pair name with uid per record object
        for obj in rec.split("{\"").skip(1) {
            let uid = crate::publish::json_str_pub(&format!("{{\"{obj}"), "mapUid");
            let name = crate::publish::json_str_pub(&format!("{{\"{obj}"), "name");
            if let (Some(u), Some(n)) = (uid, name) {
                if let Some(i) = playlist.iter().position(|x| *x == u) {
                    live_names[i] = strip(&n);
                }
            }
        }
    }
    // the fixed files: live uid (or live name) → fresh uid, staged
    let mut fixed: Vec<PathBuf> = std::fs::read_dir(&fixed_dir).map_err(|e| format!("{}: {e}", fixed_dir.display()))?.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(".Map.Gbx")).collect();
    fixed.sort();
    let mut slot_of: BTreeMap<usize, (PathBuf, String)> = BTreeMap::new();
    for p in &fixed {
        let nn: String = p.file_name().unwrap().to_string_lossy().chars().take(2).collect();
        if let Some(o) = &only {
            if !o.contains(&nn) {
                continue;
            }
        }
        let hdr = tmmaps::header::read(p.to_str().unwrap()).map_err(|e| format!("{}: {e}", p.display()))?;
        let by_uid = playlist.iter().position(|u| *u == hdr.uid);
        let by_name = { let want = strip(&hdr.name); live_names.iter().position(|n| !n.is_empty() && *n == want) };
        let Some(slot) = by_uid.or(by_name) else {
            println!("  {}: uid {} / name {:?} not in the playlist — skipped", p.display(), hdr.uid, strip(&hdr.name));
            continue;
        };
        let new_uid = format!("{prefix}{}", &hdr.uid[4..]);
        let mut m = tmmaps::map::MapFile::load(p);
        m.set_map_uid(&new_uid);
        let dst = staged_dir.join(p.file_name().unwrap());
        m.write_to(&dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        let name = if hdr.name == "-" { p.file_name().unwrap().to_string_lossy().trim_end_matches(".Map.Gbx").to_string() } else { hdr.name.clone() };
        println!("  slot {:>2}: {} {} -> {new_uid}", slot + 1, name, hdr.uid);
        slot_of.insert(slot, (dst, name));
    }
    if slot_of.is_empty() {
        return Err("nothing to publish".into());
    }
    // the manifest: every slot, fixed ones as uploads, the rest as skip rows (playlist only)
    let manifest = out_dir.join("manifest.tsv");
    let mut text = String::new();
    for (i, uid) in playlist.iter().enumerate() {
        match slot_of.get(&i) {
            Some((p, name)) => text.push_str(&format!("{}\t{}\n", p.display(), name)),
            None => text.push_str(&format!("-\tslot{}\t{uid}\tskip\n", i + 1)),
        }
    }
    std::fs::write(&manifest, text).map_err(|e| e.to_string())?;
    let results = out_dir.join("results.tsv");
    let batch_args: Vec<String> = vec![
        "publish-batch".into(), "--manifest".into(), manifest.to_string_lossy().into_owned(), "--results".into(), results.to_string_lossy().into_owned(),
        "--outdir".into(), out_dir.join("readback").to_string_lossy().into_owned(), "--jobs".into(), jobs, "--files-only".into(), "--token-dir".into(), token_dir.to_string_lossy().into_owned(),
        "--club".into(), club, "--campaign".into(), campaign, "--campaign-name".into(), cname,
    ];
    batch::publish_batch_cmd(&batch_args)
}

/// `tinyctl round-wipe --stage DIR --out DIR --sets a,b --prefixes a=PlaE,b=DirE --jobs N
/// [--pairs-dir D] [--pairs-out F] [--club 145872] [--dry-run]` — a RECORDS WIPE round, the
/// `resetN.sh` chain as a program (Everios96's call per round: "record wipe for sure").
/// Per staged map: build uid → LIVE uid (the pairs chain) → the live record's thumbnail →
/// a NEW uid `<prefix><live[4..]>` → wood stamp; every file CREATED on Nadeo (6-wide, from
/// here) and read back IDENTICAL; only then `reuid-swap` moves every campaign/room slot
/// from the live uid to the new one, and the (live, new, set, file) pairs are appended to
/// `--pairs-out` (default `<pairs-dir>/resetNN.tsv`, NN = next) so later rounds chain
/// through them. A create failure aborts before the swap: the club keeps the old maps.
pub fn wipe_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
    let stage_root = PathBuf::from(f("--stage").ok_or("round-wipe needs --stage DIR")?);
    let out_root = PathBuf::from(f("--out").ok_or("round-wipe needs --out DIR")?);
    let sets: Vec<String> = f("--sets").ok_or("round-wipe needs --sets a,b")?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let prefixes: BTreeMap<String, String> = f("--prefixes").ok_or("round-wipe needs --prefixes set=XXXX,…")?.split(',').filter_map(|kv| kv.split_once('=').map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))).collect();
    for s in &sets {
        match prefixes.get(s) {
            Some(p) if p.len() == 4 => {}
            _ => return Err(format!("--prefixes: set `{s}` needs a 4-character prefix")),
        }
    }
    let jobs: usize = f("--jobs").and_then(|v| v.parse().ok()).unwrap_or(6).max(1);
    let club = f("--club").unwrap_or_else(|| "145872".into());
    let pairs_dir = PathBuf::from(f("--pairs-dir").unwrap_or_else(|| format!("{home}/u10s-keep/pairs")));
    let dry = tmmaps::cli::has(args, "--dry-run");
    let pairs = load_pairs(&pairs_dir)?;
    let next_n = 1 + pairs.len().max(1);
    let pairs_out = f("--pairs-out").map(PathBuf::from).unwrap_or_else(|| pairs_dir.join(format!("reset{next_n}.tsv")));
    if pairs_out.exists() {
        return Err(format!("{} exists — give --pairs-out", pairs_out.display()));
    }
    std::fs::create_dir_all(&out_root).map_err(|e| e.to_string())?;
    let token_dir = out_root.join("tokens");
    std::fs::create_dir_all(&token_dir).map_err(|e| e.to_string())?;
    let wsx = Wsx::new(args);
    let t0 = Instant::now();
    println!("round-wipe: {} sets, {} chained pair files, new pairs -> {}", sets.len(), pairs.len(), pairs_out.display());
    // ---- stage: live uid, live thumbnail, then the NEW uid
    let mut staged_all: Vec<(String, Staged, String, String)> = Vec::new(); // (set, staged, live, new)
    for set in &sets {
        let src_set = stage_root.join(set);
        let out_set = out_root.join("stage").join(set);
        let list = stage_set(&src_set, &out_set, set, &pairs)?; // copies under the LIVE uid (+ wood stamp)
        println!("[{:>5.0}s] {set}: {} maps staged under their live uid", t0.elapsed().as_secs_f64(), list.len());
        for st in list {
            // the LIVE uid from the SOURCE file through the chain — never from the staged copy
            // (a copy left by a dry run already carries the NEW uid: wipe #12's first run
            // produced (new, new) pairs and swapped nothing, 2026-09-25)
            let src = src_set.join(st.path.strip_prefix(&out_set).map_err(|e| e.to_string())?);
            let src_hdr = tmmaps::header::read(src.to_str().unwrap()).map_err(|e| format!("{}: {e}", src.display()))?;
            let live = chain_uid(&pairs, &src_hdr.uid);
            let new = format!("{}{}", prefixes[set], &live[4..]);
            let hdr = tmmaps::header::read(st.path.to_str().unwrap()).map_err(|e| e.to_string())?;
            if hdr.uid != live && hdr.uid != new {
                return Err(format!("{}: staged uid {} is neither live {live} nor new {new}", st.path.display(), hdr.uid));
            }
            staged_all.push((set.clone(), st, live, new));
        }
    }
    if staged_all.is_empty() {
        return Err("nothing staged".into());
    }
    if crate::nadeo::fresh_tokens_in(token_dir.to_str().unwrap()).is_none() {
        mint_tokens(&wsx, &token_dir)?;
    }
    let refs: Vec<&Staged> = staged_all.iter().map(|(_, st, _, _)| st).collect();
    let n = live_thumbnails(&refs, &token_dir, jobs)?;
    println!("[{:>5.0}s] {n} thumbnails taken from the live records", t0.elapsed().as_secs_f64());
    // the new uid, on a fresh load (after the thumbnail splice)
    for (_, st, live, new) in &staged_all {
        let hdr = tmmaps::header::read(st.path.to_str().unwrap()).map_err(|e| e.to_string())?;
        if hdr.uid == *new {
            continue;
        }
        if hdr.uid != *live {
            return Err(format!("{}: uid {} is neither live {live} nor new {new}", st.path.display(), hdr.uid));
        }
        let mut m = tmmaps::map::MapFile::load(&st.path);
        m.set_map_uid(new);
        let tmp = st.path.with_extension("reuid.tmp.Map.Gbx");
        m.write_to(&tmp).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &st.path).map_err(|e| e.to_string())?;
    }
    println!("[{:>5.0}s] {} maps carry their new uid", t0.elapsed().as_secs_f64(), staged_all.len());
    if dry {
        for (set, st, live, new) in &staged_all {
            println!("  {set}\t{}\t{live} -> {new}", st.name);
        }
        return Ok(());
    }
    // ---- create on Nadeo, per set, IDENTICAL or abort
    let mut created: Vec<(String, String, String, String)> = Vec::new(); // (live, new, set, file)
    for set in &sets {
        let rows: Vec<&(String, Staged, String, String)> = staged_all.iter().filter(|(s, _, _, _)| s == set).collect();
        let manifest = out_root.join(format!("manifest-{set}.tsv"));
        let text: String = rows.iter().map(|(_, st, _, _)| format!("{}\t{}\n", st.path.display(), st.name)).collect();
        std::fs::write(&manifest, text).map_err(|e| e.to_string())?;
        let results = out_root.join(format!("results-{set}.tsv"));
        // a stale token → re-mint and resume (publish-batch skips nothing here: create = new uid)
        for attempt in 0..3 {
            if crate::nadeo::fresh_tokens_in(token_dir.to_str().unwrap()).is_none() {
                mint_tokens(&wsx, &token_dir)?;
            }
            let batch_args: Vec<String> = vec![
                "publish-batch".into(), "--manifest".into(), manifest.to_string_lossy().into_owned(), "--results".into(), results.to_string_lossy().into_owned(),
                "--outdir".into(), out_root.join("readback").to_string_lossy().into_owned(), "--jobs".into(), jobs.to_string(), "--files-only".into(), "--token-dir".into(), token_dir.to_string_lossy().into_owned(),
            ];
            let r = batch::publish_batch_cmd(&batch_args);
            let ok = read_identical(&results);
            if ok.len() == rows.len() {
                break;
            }
            println!("[{:>5.0}s] {set}: {} of {} IDENTICAL after attempt {} ({})", t0.elapsed().as_secs_f64(), ok.len(), rows.len(), attempt + 1, r.err().unwrap_or_default().lines().next().unwrap_or(""));
            // the manifest for the retry: only the rows not yet IDENTICAL
            let text: String = rows.iter().filter(|(_, st, _, _)| !ok.contains_key(&st.path.to_string_lossy().to_string())).map(|(_, st, _, _)| format!("{}\t{}\n", st.path.display(), st.name)).collect();
            if text.is_empty() {
                break;
            }
            std::fs::write(&manifest, text).map_err(|e| e.to_string())?;
        }
        let ok = read_identical(&results);
        let missing: Vec<&str> = rows.iter().filter(|(_, st, _, _)| !ok.contains_key(&st.path.to_string_lossy().to_string())).map(|(_, st, _, _)| st.name.as_str()).collect();
        if !missing.is_empty() {
            return Err(format!("{set}: {} map(s) not created IDENTICAL ({}) — NO swap done, the club is untouched", missing.len(), missing.join(", ")));
        }
        for (s, st, live, new) in &rows {
            created.push((live.clone(), new.clone(), s.clone(), st.path.file_name().unwrap().to_string_lossy().into_owned()));
        }
        println!("[{:>5.0}s] {set}: {} created IDENTICAL", t0.elapsed().as_secs_f64(), rows.len());
    }
    // ---- the swap (campaigns + rooms), then the pairs file
    let swap_pairs = out_root.join("pairs2.tsv");
    std::fs::write(&swap_pairs, created.iter().map(|(l, n, _, _)| format!("{l}\t{n}\n")).collect::<String>()).map_err(|e| e.to_string())?;
    if crate::nadeo::fresh_tokens_in(token_dir.to_str().unwrap()).is_none() {
        mint_tokens(&wsx, &token_dir)?;
    }
    std::env::set_var("TINYCTL_TOKEN_DIR", token_dir.to_str().unwrap());
    let swap_args: Vec<String> = vec!["reuid-swap".into(), "--club".into(), club.clone(), "--pairs".into(), swap_pairs.to_string_lossy().into_owned(), "--no-lock".into(), "--outdir".into(), out_root.join("swap").to_string_lossy().into_owned()];
    std::fs::create_dir_all(out_root.join("swap")).map_err(|e| e.to_string())?;
    let mut swap_ok = false;
    for attempt in 0..2 {
        match crate::nadeo::cmd(&swap_args) {
            Ok(()) => { swap_ok = true; break; }
            Err(e) => {
                println!("[{:>5.0}s] swap attempt {}: {}", t0.elapsed().as_secs_f64(), attempt + 1, e.lines().next().unwrap_or(""));
                mint_tokens(&wsx, &token_dir)?;
            }
        }
    }
    let text: String = created.iter().map(|(l, n, s, fnm)| format!("{l}\t{n}\t{s}\t{fnm}\n")).collect();
    std::fs::write(&pairs_out, &text).map_err(|e| format!("{}: {e}", pairs_out.display()))?;
    println!("[{:>5.0}s] round-wipe: {} maps re-created and swapped{}; pairs -> {}", t0.elapsed().as_secs_f64(), created.len(), if swap_ok { "" } else { " (SWAP FAILED — rerun reuid-swap with pairs2.tsv)" }, pairs_out.display());
    if swap_ok { Ok(()) } else { Err("reuid-swap failed twice".into()) }
}

/// `tinyctl round-inplace --stage DIR --out DIR --sets a,b [--jobs N] [--pairs-dir D]` — an
/// IN-PLACE round: every staged map goes up under its LIVE uid (the pairs chain), with the
/// live record's thumbnail, records untouched (Everios96's "in place" answer). The same
/// staging as `round-wipe` minus the new uid and the swap; `publish-batch --files-only`
/// with IDENTICAL read-back, a stale token re-minted between sets.
pub fn inplace_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
    let stage_root = PathBuf::from(f("--stage").ok_or("round-inplace needs --stage DIR")?);
    let out_root = PathBuf::from(f("--out").ok_or("round-inplace needs --out DIR")?);
    let sets: Vec<String> = f("--sets").ok_or("round-inplace needs --sets a,b")?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let jobs: usize = f("--jobs").and_then(|v| v.parse().ok()).unwrap_or(6).max(1);
    let pairs_dir = PathBuf::from(f("--pairs-dir").unwrap_or_else(|| format!("{home}/u10s-keep/pairs")));
    let pairs = load_pairs(&pairs_dir)?;
    std::fs::create_dir_all(&out_root).map_err(|e| e.to_string())?;
    let token_dir = out_root.join("tokens");
    std::fs::create_dir_all(&token_dir).map_err(|e| e.to_string())?;
    let wsx = Wsx::new(args);
    let t0 = Instant::now();
    let mut all: Vec<(String, Staged)> = Vec::new();
    for set in &sets {
        let list = stage_set(&stage_root.join(set), &out_root.join("stage").join(set), set, &pairs)?;
        println!("[{:>5.0}s] {set}: {} maps staged under their live uid", t0.elapsed().as_secs_f64(), list.len());
        for st in list {
            all.push((set.clone(), st));
        }
    }
    if all.is_empty() {
        return Err("nothing staged".into());
    }
    if crate::nadeo::fresh_tokens_in(token_dir.to_str().unwrap()).is_none() {
        mint_tokens(&wsx, &token_dir)?;
    }
    let refs: Vec<&Staged> = all.iter().map(|(_, st)| st).collect();
    let n = live_thumbnails(&refs, &token_dir, jobs)?;
    println!("[{:>5.0}s] {n} thumbnails taken from the live records", t0.elapsed().as_secs_f64());
    let mut total_ok = 0usize;
    for set in &sets {
        let rows: Vec<&Staged> = all.iter().filter(|(s, _)| s == set).map(|(_, st)| st).collect();
        let manifest = out_root.join(format!("manifest-{set}.tsv"));
        let results = out_root.join(format!("results-{set}.tsv"));
        std::fs::write(&manifest, rows.iter().map(|st| format!("{}\t{}\n", st.path.display(), st.name)).collect::<String>()).map_err(|e| e.to_string())?;
        // the IDENTICAL set is the UNION over the attempts (publish-batch rewrites the results
        // file per run — attempt 3 of the first Tiny round re-uploaded the 521 good maps)
        let mut ok_all: BTreeMap<String, String> = BTreeMap::new();
        for attempt in 0..3 {
            if crate::nadeo::fresh_tokens_in(token_dir.to_str().unwrap()).is_none() {
                mint_tokens(&wsx, &token_dir)?;
            }
            let batch_args: Vec<String> = vec![
                "publish-batch".into(), "--manifest".into(), manifest.to_string_lossy().into_owned(), "--results".into(), results.to_string_lossy().into_owned(),
                "--outdir".into(), out_root.join("readback").to_string_lossy().into_owned(), "--jobs".into(), jobs.to_string(), "--files-only".into(), "--token-dir".into(), token_dir.to_string_lossy().into_owned(),
            ];
            let r = batch::publish_batch_cmd(&batch_args);
            ok_all.extend(read_identical(&results));
            let _ = std::fs::write(out_root.join(format!("results-{set}-attempt{}.tsv", attempt + 1)), std::fs::read(&results).unwrap_or_default());
            if ok_all.len() >= rows.len() {
                break;
            }
            println!("[{:>5.0}s] {set}: {} of {} IDENTICAL after attempt {} ({})", t0.elapsed().as_secs_f64(), ok_all.len(), rows.len(), attempt + 1, r.err().unwrap_or_default().lines().next().unwrap_or(""));
            let text: String = rows.iter().filter(|st| !ok_all.contains_key(&st.path.to_string_lossy().to_string())).map(|st| format!("{}\t{}\n", st.path.display(), st.name)).collect();
            if text.is_empty() {
                break;
            }
            std::fs::write(&manifest, text).map_err(|e| e.to_string())?;
        }
        let ok = ok_all;
        let missing: Vec<&str> = rows.iter().filter(|st| !ok.contains_key(&st.path.to_string_lossy().to_string())).map(|st| st.name.as_str()).collect();
        println!("[{:>5.0}s] {set}: {} of {} updated in place IDENTICAL{}", t0.elapsed().as_secs_f64(), rows.len() - missing.len(), rows.len(), if missing.is_empty() { String::new() } else { format!("; MISSING: {}", missing.join(", ")) });
        total_ok += rows.len() - missing.len();
    }
    println!("[{:>5.0}s] round-inplace: {total_ok} of {} maps updated", t0.elapsed().as_secs_f64(), all.len());
    if total_ok == all.len() { Ok(()) } else { Err(format!("{} map(s) not updated", all.len() - total_ok)) }
}
