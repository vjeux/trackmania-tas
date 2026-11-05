//! `tinyctl rethumb-wipe` — new menu thumbnails for maps WITHOUT records, by wipe:
//!
//! ```text
//! tinyctl rethumb-wipe --uids-file U.tsv --work DIR [--token-dir /tmp/tinyctl-tokens]
//!                      [--club 145872] [--chunk 40] [--no-promote]
//! ```
//!
//! Nadeo's menu thumbnail is a separate asset (`core.trackmania.nadeo.live/maps/<id>/
//! thumbnail.jpg`) generated when a map is CREATED and never refreshed by an in-place update
//! (found 2026-11-03: Dirt 01 updated at 05:44, asset still dated Sep 14; a `thumbnail` part
//! on the update is ignored, no thumbnail route exists). Everios96 09:53: "Do it for maps
//! that dont have any records." So, per chunk of maps that have NO record:
//!
//! 1. `rethumb` (no publish) renders the new picture into the live file (render copy → editor
//!    render → JPEG spliced, body = live);
//! 2. each final file is re-uid'd `<Set>S<tail>` ('S' for the thumbnail swap; a uid already
//!    online is refused) — `tmmaps rename-map --uid`;
//! 3. `publish-batch` CREATES them (storage-only; the new asset is rendered from the new file);
//! 4. `nadeo-here reuid-swap` moves every campaign playlist and room from the old uids to
//!    the new ones. The old maps drop out of the club; they had no records to lose.
//!
//! `U.tsv` = `uid<TAB>name` of maps verified to have NO record (the census the caller made);
//! this command re-checks every map's leaderboard right before its wipe and refuses one that
//! gained a record meanwhile. Resumable: a chunk whose `swap.done` exists is skipped.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn tinyctl() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("tinyctl"))
}
fn tmmaps() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("tmmaps"))).filter(|p| p.exists()).unwrap_or_else(|| PathBuf::from("tmmaps"))
}

/// The campaign-name word of a set, from its uid prefix (Everios96's campaigns are
/// "<Set> Part NN"; "Wood" also matches "Wet Wood" — harmless, the uids decide).
fn set_name_of(uid: &str) -> String {
    match &uid[..3] {
        "Dir" => "Dirt",
        "Gia" => "Giant",
        "Gra" => "Grass",
        "Ice" => "Icy",
        "Mag" => "Magnet",
        "Pla" => "Plastic",
        "Tar" => "Tarmac",
        "Tin" => "Tiny",
        "Wet" => "Wet Wood",
        "Woo" => "Wood",
        _ => "",
    }
    .to_string()
}

/// Item references of a map whose files are NOT in its embedded zip: the editor shows
/// "Error while retrieving map! Missing Items … LOAD ANYWAY / CANCEL" and that modal blocks
/// every map opened after it (Giant 495, 2026-11-05: 94 moving-platform items from
/// "TM_Items_Sorted_v8.0" referenced by path, absent from the zip — in Everios96's original
/// too, so they are missing for players as well). Such a map is left over, never rendered.
pub fn missing_embedded_items(tmmaps: &Path, map: &Path) -> Vec<String> {
    let census = Command::new(tmmaps).args(["census", &map.to_string_lossy()]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let refs: std::collections::BTreeSet<String> = census
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() > 2 && f[1].starts_with('i') && f[2].contains('\\')).then(|| f[2].to_string())
        })
        .collect();
    if refs.is_empty() {
        return Vec::new();
    }
    let embedded = Command::new(tmmaps).args(["embedded", &map.to_string_lossy()]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let zip_names: Vec<String> = embedded.lines().filter_map(|l| l.trim_start().strip_prefix("zip  ")).filter_map(|l| l.split('\t').next().map(|n| n.replace('/', "\\"))).collect();
    // the game resolves a reference by its RELATIVE name (`<ref>` or `Items\<ref>`); an entry
    // stored under the original author's absolute path (`C:\Users\David\Documents\Trackmania\
    // Items\TM_Items_Sorted_v8.0\…\1_Ease.Item.Gbx`, Giant 495) is as good as missing
    refs.into_iter().filter(|r| !zip_names.iter().any(|z| z == r || z.strip_prefix("Items\\").map(|t| t == r).unwrap_or(false))).collect()
}

fn core_token(dir: &str) -> Result<String, String> {
    let t = std::fs::read_to_string(format!("{dir}/token-NadeoServices.txt")).map_err(|e| e.to_string())?;
    Ok(t.trim().to_string())
}

/// The online map record for a uid: `None` when free, `Some(name)` when a map holds it.
fn uid_online(tok: &str, uid: &str) -> Result<Option<String>, String> {
    let out = Command::new("curl").args(["-s", "-m", "30", "-H", &format!("Authorization: {tok}"), &format!("https://prod.trackmania.core.nadeo.online/maps/?mapUidList={uid}")]).output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    if !text.trim_start().starts_with('[') {
        return Err(format!("maps lookup {uid}: {}", text.chars().take(120).collect::<String>()));
    }
    if text.trim() == "[]" {
        return Ok(None);
    }
    // the record's name, so a candidate WE created in an earlier, half-failed attempt is
    // reused (publish-batch then updates it in place to identical bytes) instead of the
    // next letter orphaning it (w006 on 2026-11-04: Nadeo's upload hung mid-chunk)
    let name = text.split("\"name\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").replace("\\\\", "\\");
    Ok(Some(name))
}

/// One line to Everios96's DM (his ask: every wait announced).
fn discord(msg: &str) {
    let cmd = format!("~/bin/discordctl send 1548403109309579264 '{}'", msg.replace('\'', ""));
    let _ = Command::new("/home/vjeux/bin/whitestick").arg(&cmd).output();
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let uids_file = PathBuf::from(f("--uids-file").ok_or("rethumb-wipe needs --uids-file U.tsv (uid<TAB>name, NO-record maps)")?);
    let work = PathBuf::from(f("--work").ok_or("rethumb-wipe needs --work DIR")?);
    let token_dir = f("--token-dir").unwrap_or_else(|| "/tmp/tinyctl-tokens".into());
    let club = f("--club").unwrap_or_else(|| "145872".into());
    let chunk: usize = f("--chunk").and_then(|v| v.parse().ok()).unwrap_or(40);
    let no_promote = tmmaps::cli::has(args, "--no-promote");
    let t0 = Instant::now();
    let rows: Vec<(String, String)> = std::fs::read_to_string(&uids_file)
        .map_err(|e| format!("{}: {e}", uids_file.display()))?
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut it = l.split('\t');
            (it.next().unwrap_or("").trim().to_string(), it.next().unwrap_or("").trim().to_string())
        })
        .filter(|(u, n)| !u.is_empty() && !n.is_empty())
        .collect();
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let mut total_swapped = 0usize;
    for (k, c) in rows.chunks(chunk).enumerate() {
        let cw = work.join(format!("w{k:03}"));
        std::fs::create_dir_all(&cw).map_err(|e| e.to_string())?;
        if cw.join("swap.done").exists() {
            total_swapped += c.len();
            continue;
        }
        let uids = cw.join("uids.tsv");
        std::fs::write(&uids, c.iter().map(|(u, n)| format!("{u}\t{n}\n")).collect::<String>()).map_err(|e| e.to_string())?;
        // tokens: the shorter of core/Live must have ≥ 15 min left (a chunk is ~10 min); a
        // mint touches the game, so it is a registered run of ours (10:55 on 11-03: the Live
        // token expired mid-set and the fetch of w001 came back empty)
        if crate::rethumball::token_left(&token_dir) < 30 * 60 {
            let _ = Command::new(tinyctl()).args(["mint-tokens", "--token-dir", &token_dir]).output();
            if crate::rethumball::token_left(&token_dir) < 30 * 60 {
                let _ = Command::new(tinyctl()).args(["mint-tokens", "--standalone", "--token-dir", &token_dir]).output();
            }
            println!("[{:>4.0}s] w{k:03}: tokens {} s left", t0.elapsed().as_secs_f64(), crate::rethumball::token_left(&token_dir));
        }
        // the human gate (the box's own idle, my runs excluded): a person at the machine
        // pauses the wipe until 30 min untouched
        let mut waited = 0u64;
        loop {
            let out = Command::new(tinyctl()).args(["box-idle", "--window-s", "1800"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
            let last = out.lines().rev().find(|l| l.contains("verdict=")).unwrap_or("").to_string();
            if last.contains("verdict=free") {
                if waited > 0 {
                    discord(&format!("Thumbnail wipes resumed after {} min. -- ai", waited / 60));
                }
                break;
            }
            println!("[{:>4.0}s] w{k:03}: waiting for the box ({})", t0.elapsed().as_secs_f64(), last.chars().take(90).collect::<String>());
            if waited == 0 {
                // Everios96 2026-11-03 23:01: "let me know whenever you wait for something"
                let idle = last.split_whitespace().find_map(|w| w.strip_prefix("kbm_idle_s=")).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                if last.is_empty() {
                    discord("Thumbnail wipes paused: the link to the render machine gave no reading; retrying every 5 min. -- ai");
                } else {
                    discord(&format!("Thumbnail wipes paused: the render machine saw input {:.0} min ago (my own runs excluded); resuming after 30 min untouched. -- ai", idle / 60.0));
                }
            }
            let _ = Command::new(tinyctl()).args(["box-close-game", "--idle-s", "60"]).output();
            std::thread::sleep(std::time::Duration::from_secs(300));
            waited += 300;
        }
        {
            let out = Command::new(tinyctl()).args(["box-idle", "--window-s", "60"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
            let idle = out.split_whitespace().find_map(|w| w.strip_prefix("kbm_idle_s=")).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            crate::inputidle::runlog::start(&format!("tinyctl rethumb-wipe w{k:03}"), idle);
        }
        // 1. the renders (no publish)
        if !cw.join("manifest.tsv").exists() {
            let mut cmd = Command::new(tinyctl());
            cmd.args(["rethumb", "--uids-file", &uids.to_string_lossy(), "--work", &cw.to_string_lossy(), "--token-dir", &token_dir]);
            cmd.env("RETHUMB_RUN_REGISTERED", "1");
            if no_promote {
                cmd.arg("--no-promote");
            }
            let out = cmd.output().map_err(|e| e.to_string())?;
            if !out.status.success() {
                let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                return Err(format!("w{k:03} render: {}", text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("")));
            }
        }
        // maps box-prep/prep refused to render (items referenced but not embedded): leftovers
        // with the reason, so the final pass knows not to try them again blindly
        for cand in [cw.join("prep-skipped.tsv"), cw.join("rendered").join("prep-skipped.tsv")] {
            if let Ok(text) = std::fs::read_to_string(&cand) {
                let leftover = work.join("leftover.tsv");
                if let Ok(mut lf) = std::fs::OpenOptions::new().create(true).append(true).open(&leftover) {
                    use std::io::Write;
                    let existing = std::fs::read_to_string(&leftover).unwrap_or_default();
                    for l in text.lines().filter(|l| !l.trim().is_empty()) {
                        if !existing.contains(l.split('\t').next().unwrap_or("")) {
                            let _ = writeln!(lf, "{l}");
                        }
                    }
                }
                let n = text.lines().filter(|l| !l.trim().is_empty()).count();
                if n > 0 {
                    println!("[{:>4.0}s] w{k:03}: {n} map(s) with non-embedded items left over (see {})", t0.elapsed().as_secs_f64(), leftover.display());
                }
                break;
            }
        }
        // 2. re-uid + the no-record re-check — on a live token (a render with retries can
        // outlast the hour: w006 on 2026-11-05 came back to "Invalid credentials")
        if crate::rethumball::token_left(&token_dir) < 20 * 60 {
            let _ = Command::new(tinyctl()).args(["mint-tokens", "--token-dir", &token_dir]).output();
            if crate::rethumball::token_left(&token_dir) < 20 * 60 {
                let _ = Command::new(tinyctl()).args(["mint-tokens", "--standalone", "--token-dir", &token_dir]).output();
            }
        }
        let tok = core_token(&token_dir)?;
        let final_dir = cw.join("final");
        let wipe_dir = cw.join("wipe");
        std::fs::create_dir_all(&wipe_dir).map_err(|e| e.to_string())?;
        let mut pairs = String::new();
        let mut manifest = String::new();
        let mut skipped = 0usize;
        for (u, n) in c {
            let fin = final_dir.join(format!("{n}.Map.Gbx"));
            if !fin.exists() {
                skipped += 1; // a straggler: no render, no wipe
                continue;
            }
            // the live leaderboard right now: a record gained since the census = hands off
            let lb = Command::new("curl").args(["-s", "-m", "30", "-H", &format!("Authorization: {}", std::fs::read_to_string(format!("{token_dir}/token-NadeoLiveServices.txt")).unwrap_or_default().trim()), &format!("https://live-services.trackmania.nadeo.live/api/token/leaderboard/group/Personal_Best/map/{u}/top?length=1&onlyWorld=true")]).output().map_err(|e| e.to_string())?;
            let lb_text = String::from_utf8_lossy(&lb.stdout);
            if lb_text.contains("\"accountId\"") {
                println!("  {n}: gained a record since the census — left alone");
                skipped += 1;
                continue;
            }
            // the uid's 4th character is the generation marker (2,3,4,F,H,S,W,X… are taken by
            // earlier wipes — Dirt 53 is already `DirS…`): 'T' (thumbnails) first, then the
            // next free letter, each checked online before use
            let mut new = String::new();
            for g in ['T', 'U', 'V', 'Y', 'Z', 'Q', 'R', 'N', 'M', 'K'] {
                let cand = format!("{}{}{}", &u[..3], g, &u[4..]);
                match uid_online(&tok, &cand)? {
                    None => {
                        new = cand;
                        break;
                    }
                    Some(online_name) if online_name == *n => {
                        // ours, from an earlier attempt of this very chunk: reuse
                        new = cand;
                        break;
                    }
                    Some(_) => {}
                }
            }
            if new.is_empty() {
                return Err(format!("{n}: no free generation letter for {u}"));
            }
            let dst = wipe_dir.join(format!("{n}.Map.Gbx"));
            let o = Command::new(tmmaps()).args(["rename-map", &fin.to_string_lossy(), "--out", &dst.to_string_lossy(), "--name", n, "--uid", &new]).output().map_err(|e| e.to_string())?;
            if !o.status.success() || !dst.exists() {
                return Err(format!("rename-map {n}: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("")));
            }
            pairs.push_str(&format!("{u}\t{new}\n"));
            manifest.push_str(&format!("{}\t{n}\n", dst.display()));
        }
        if manifest.is_empty() {
            println!("[{:>4.0}s] w{k:03}: nothing to wipe ({skipped} skipped)", t0.elapsed().as_secs_f64());
            std::fs::write(cw.join("swap.done"), "empty").map_err(|e| e.to_string())?;
            continue;
        }
        std::fs::write(cw.join("pairs.tsv"), &pairs).map_err(|e| e.to_string())?;
        std::fs::write(cw.join("wipe-manifest.tsv"), &manifest).map_err(|e| e.to_string())?;
        // 3. the creates — on a token with ≥ 20 min left (the render took ~10 min of the
        // hour; a slow Nadeo made 40 uploads outlive the token on 2026-11-04). The game is
        // free at this point (the render is done), so a mint can run.
        if crate::rethumball::token_left(&token_dir) < 20 * 60 {
            let _ = Command::new(tinyctl()).args(["mint-tokens", "--token-dir", &token_dir]).output();
            if crate::rethumball::token_left(&token_dir) < 20 * 60 {
                let _ = Command::new(tinyctl()).args(["mint-tokens", "--standalone", "--token-dir", &token_dir]).output();
            }
            println!("[{:>4.0}s] w{k:03}: tokens {} s left before the creates", t0.elapsed().as_secs_f64(), crate::rethumball::token_left(&token_dir));
        }
        // 3. the creates. ONE failed upload must not fail the chunk: Nadeo's upload endpoint
        // ran at 40–80 s per map on 2026-11-04 (7 s that morning) and a single 408 at
        // 39/40 cost three one-hour attempts. The failed maps get one more try on their own
        // manifest; what still fails goes to `<set>/leftover.tsv` for a final pass, and the
        // chunk swaps the ones that landed.
        let results = cw.join("wipe-results.tsv");
        let read_results = |p: &Path| -> std::collections::HashMap<String, String> {
            std::fs::read_to_string(p).unwrap_or_default().lines().skip(1).filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                (f.len() >= 8).then(|| (f[1].to_string(), f[7].to_string()))
            }).collect()
        };
        let _ = Command::new(tinyctl())
            .args(["publish-batch", "--manifest", &cw.join("wipe-manifest.tsv").to_string_lossy(), "--results", &results.to_string_lossy(), "--no-campaign", "--files-only", "--token-dir", &token_dir, "--jobs", "4"])
            .status()
            .map_err(|e| e.to_string())?;
        let mut verdicts = read_results(&results);
        let want = manifest.lines().count();
        let mut failed: Vec<String> = manifest.lines().filter_map(|l| l.split('\t').nth(1)).filter(|n| verdicts.get(*n).map(|v| v != "IDENTICAL").unwrap_or(true)).map(String::from).collect();
        if !failed.is_empty() {
            println!("[{:>4.0}s] w{k:03}: {} of {want} created; retrying the {} that failed", t0.elapsed().as_secs_f64(), want - failed.len(), failed.len());
            let retry_manifest: String = manifest.lines().filter(|l| l.split('\t').nth(1).map(|n| failed.iter().any(|f| f == n)).unwrap_or(false)).map(|l| format!("{l}\n")).collect();
            std::fs::write(cw.join("retry-manifest.tsv"), &retry_manifest).map_err(|e| e.to_string())?;
            let retry_results = cw.join("retry-results.tsv");
            if crate::rethumball::token_left(&token_dir) < 15 * 60 {
                let _ = Command::new(tinyctl()).args(["mint-tokens", "--token-dir", &token_dir]).output();
            }
            let _ = Command::new(tinyctl())
                .args(["publish-batch", "--manifest", &cw.join("retry-manifest.tsv").to_string_lossy(), "--results", &retry_results.to_string_lossy(), "--no-campaign", "--files-only", "--token-dir", &token_dir, "--jobs", "2"])
                .status()
                .map_err(|e| e.to_string())?;
            for (n, v) in read_results(&retry_results) {
                verdicts.insert(n, v);
            }
            failed = manifest.lines().filter_map(|l| l.split('\t').nth(1)).filter(|n| verdicts.get(*n).map(|v| v != "IDENTICAL").unwrap_or(true)).map(String::from).collect();
        }
        let ok = want - failed.len();
        if ok == 0 {
            return Err(format!("w{k:03}: 0 of {want} created IDENTICAL — swap NOT done (see {})", results.display()));
        }
        if !failed.is_empty() {
            // leftovers: old uid + name, for the set's final pass; their pairs leave this swap
            let leftover = work.join("leftover.tsv");
            let mut lf = std::fs::OpenOptions::new().create(true).append(true).open(&leftover).map_err(|e| e.to_string())?;
            use std::io::Write;
            let kept: String = pairs.lines().filter(|l| {
                let old = l.split('\t').next().unwrap_or("");
                let n = c.iter().find(|(u, _)| u == old).map(|(_, n)| n.as_str()).unwrap_or("");
                if failed.iter().any(|f| f == n) {
                    let _ = writeln!(lf, "{old}\t{n}");
                    false
                } else {
                    true
                }
            }).map(|l| format!("{l}\n")).collect();
            std::fs::write(cw.join("pairs.tsv"), &kept).map_err(|e| e.to_string())?;
            println!("[{:>4.0}s] w{k:03}: {} map(s) left for the final pass ({})", t0.elapsed().as_secs_f64(), failed.len(), leftover.display());
        }
        // 4. the playlist/room swap — only for the uids that landed
        let mut env = std::collections::HashMap::new();
        env.insert("TINYCTL_TOKEN_DIR", token_dir.clone());
        let o = Command::new(tinyctl())
            .envs(env)
            .args(["nadeo-here", "reuid-swap", "--club", &club, "--pairs", &cw.join("pairs.tsv").to_string_lossy(), "--no-lock", "--outdir", &cw.to_string_lossy(), "--name-filter", &set_name_of(&c[0].0)])
            .output()
            .map_err(|e| e.to_string())?;
        let so = String::from_utf8_lossy(&o.stdout).to_string();
        if !o.status.success() || !so.contains("rewritten") {
            return Err(format!("w{k:03}: reuid-swap: {}", so.lines().last().unwrap_or("")));
        }
        std::fs::write(cw.join("swap.done"), &so).map_err(|e| e.to_string())?;
        total_swapped += ok;
        println!("[{:>4.0}s] w{k:03}: {ok} maps wiped to new uids with new thumbnails ({skipped} skipped, {} left over); {}", failed.len(), t0.elapsed().as_secs_f64(), so.lines().last().unwrap_or(""));
    }
    println!("rethumb-wipe: {total_swapped} maps swapped in {:.1} h", t0.elapsed().as_secs_f64() / 3600.0);
    Ok(())
}
