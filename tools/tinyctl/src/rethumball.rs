//! `tinyctl rethumb-all` — the thumbnail rollout over every non-done map, in chunks, for
//! as long as the render box is free:
//!
//! ```text
//! tinyctl rethumb-all --lists DIR --work DIR [--sets dirt,grass,…] [--chunk 40]
//!                     [--plain-sets giant,tiny] [--token-dir /tmp/tinyctl-tokens]
//! ```
//!
//! `DIR/<set>.tsv` = `uid<TAB>name` rows (the live inventory minus Everios96's DONE parts,
//! 2026-11-02). Per set, per chunk of `--chunk` maps: the box gate (no human input for 30
//! minutes, the game lock not held by another session), fresh tokens (≥ 15 min left, else a
//! mint — standalone when the running game's token is the stale one), then `tinyctl rethumb
//! --publish` on the chunk (fetch → render copies → editor renders → JPEG into the live file
//! → in-place update). A chunk whose `results.tsv` is all IDENTICAL is skipped (resume). The
//! Giant and Tiny sets are item-built at another scale, so their render copies are the live
//! files themselves (`--no-promote`). The game is closed when the run ends or waits.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn tinyctl() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("tinyctl"))
}

/// Seconds left on the SHORTER of the two tokens: the Live token (the fetch's) expires
/// before the core one (the upload's) — the driver checked core alone and a chunk's fetch
/// ran on a dead Live token (00:08 on 11-03, c017 "40 live files missing").
pub fn token_left(dir: &str) -> i64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    ["NadeoServices", "NadeoLiveServices"]
        .iter()
        .map(|aud| {
            let t = std::fs::read_to_string(format!("{dir}/token-{aud}.txt")).unwrap_or_default();
            crate::nadeo::jwt_exp(t.trim()).map(|exp| exp as i64 - now).unwrap_or(-1)
        })
        .min()
        .unwrap_or(-1)
}

/// A chunk is done when every map is either published IDENTICAL or listed as a straggler
/// (stragglers.tsv — rendered in a later pass over that file); returns (done, published, stragglers).
fn chunk_state(work: &Path, n: usize) -> (bool, usize, usize) {
    let t = std::fs::read_to_string(work.join("results.tsv")).unwrap_or_default();
    let ok = t.lines().skip(1).filter(|l| l.split('\t').nth(7) == Some("IDENTICAL")).count();
    let strag = std::fs::read_to_string(work.join("stragglers.tsv")).map(|s| s.lines().filter(|l| !l.trim().is_empty()).count()).unwrap_or(0);
    (ok + strag >= n && ok > 0, ok, strag)
}

/// The devserver gate's reading. A reading that did not come back (a bridge hiccup gave an
/// EMPTY line at 23:15 on 11-02 and the driver announced "input 0 min ago" — Everios96:
/// "BRO IT WAS YOU AGAIN") is retried up to 3 times 20 s apart; still nothing = (false,
/// "no reading"), which the caller reports as such, never as a person.
fn box_free() -> (bool, String) {
    for attempt in 0..3 {
        let out = Command::new(tinyctl()).args(["box-idle", "--window-s", "1800"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
        let last = out.lines().rev().find(|l| l.contains("verdict=")).unwrap_or("").to_string();
        if !last.is_empty() && !last.contains("verdict=error") {
            return (last.contains("verdict=free"), last);
        }
        // what came back instead (03:14 on 11-03: three empty readings while the bridge
        // answered a hand-run check fine — the error text is the evidence we lacked)
        let err = out.lines().last().unwrap_or("").chars().take(160).collect::<String>();
        if let Ok(mut fh) = std::fs::OpenOptions::new().create(true).append(true).open("/home/vjeux/tt-home/rethumb/all/gate-errors.log") {
            use std::io::Write;
            let _ = writeln!(fh, "{} attempt {attempt}: {err}", chrono_like());
        }
        if attempt < 2 {
            std::thread::sleep(Duration::from_secs(20));
        }
    }
    (false, "no reading (bridge)".to_string())
}

fn lock_foreign() -> bool {
    // tmdrive's status line names the holder; ours (this session) or FREE/DEAD is fine
    let out = Command::new("/home/vjeux/bin/whitestick").args(["tmdrive status | head -1"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    out.starts_with("HELD") && !out.contains("080a15b8-2c6a-4fff-9d8a-db7d0072ee43")
}

fn close_game() {
    let _ = Command::new(tinyctl()).args(["box-close-game", "--idle-s", "60"]).output();
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let lists = PathBuf::from(f("--lists").ok_or("rethumb-all needs --lists DIR")?);
    let work = PathBuf::from(f("--work").ok_or("rethumb-all needs --work DIR")?);
    let token_dir = f("--token-dir").unwrap_or_else(|| "/tmp/tinyctl-tokens".into());
    let chunk: usize = f("--chunk").and_then(|v| v.parse().ok()).unwrap_or(40);
    let sets: Vec<String> = f("--sets").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_else(|| vec!["tarmac", "dirt", "grass", "magnet", "icy", "plastic", "wood", "wetwood", "giant", "tiny"].into_iter().map(String::from).collect());
    let plain: Vec<String> = f("--plain-sets").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_else(|| vec!["giant".into(), "tiny".into()]);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let log_path = work.join("rethumb-all.log");
    let log = |line: &str| {
        let stamp = chrono_like();
        println!("[{stamp}] {line}");
        if let Ok(mut fh) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
            use std::io::Write;
            let _ = writeln!(fh, "[{stamp}] {line}");
        }
    };
    let t0 = Instant::now();
    let mut n_done_total = 0usize;
    for set in &sets {
        let list = lists.join(format!("{set}.tsv"));
        let rows: Vec<String> = std::fs::read_to_string(&list).map_err(|e| format!("{}: {e}", list.display()))?.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect();
        let chunks: Vec<&[String]> = rows.chunks(chunk).collect();
        log(&format!("set {set}: {} maps in {} chunks", rows.len(), chunks.len()));
        let mut set_ok = 0usize;
        for (k, c) in chunks.iter().enumerate() {
            let cw = work.join(set).join(format!("c{k:03}"));
            std::fs::create_dir_all(&cw).map_err(|e| e.to_string())?;
            let uids = cw.join("uids.tsv");
            std::fs::write(&uids, c.join("\n") + "\n").map_err(|e| e.to_string())?;
            let (done0, ok0, _) = chunk_state(&cw, c.len());
            if done0 {
                set_ok += ok0;
                continue;
            }
            // the box gate
            let mut waited = 0u64;
            loop {
                let (free, line) = box_free();
                let foreign = lock_foreign();
                if free && !foreign {
                    break;
                }
                if waited == 0 {
                    let no_reading = line.starts_with("no reading");
                    let why = if foreign { "lock held by another session".to_string() } else if no_reading { line.clone() } else { format!("in use: {}", line.chars().take(80).collect::<String>()) };
                    log(&format!("{set} c{k:03}: waiting for the box ({why})"));
                    close_game();
                    // Everios96 2026-11-02 23:01: "let me know whenever you wait for something"
                    let idle = line.split_whitespace().find_map(|w| w.strip_prefix("kbm_idle_s=")).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                    let msg = if foreign {
                        format!("Thumbnails paused ({set} {}/{}): another job holds the render machine; resuming when it is free. -- ai", set_ok, rows.len())
                    } else if no_reading {
                        format!("Thumbnails paused ({set} {}/{}): the bridge to the render machine gave no reading; retrying every 5 min. -- ai", set_ok, rows.len())
                    } else {
                        format!("Thumbnails paused ({set} {}/{}): the render machine saw input {:.0} min ago (last real input, my own runs excluded); resuming after 30 min untouched. -- ai", set_ok, rows.len(), idle / 60.0)
                    };
                    discord(&msg);
                }
                std::thread::sleep(Duration::from_secs(300));
                waited += 300;
                if waited % 3600 == 0 {
                    log(&format!("{set} c{k:03}: still waiting ({} min)", waited / 60));
                }
            }
            if waited > 0 {
                discord(&format!("Thumbnails resumed ({set} {}/{}) after {} min. -- ai", set_ok, rows.len(), waited / 60));
            }
            // THIS CHUNK'S WHOLE BOX ACTIVITY (mint, launch, renders) is ONE logged run of ours,
            // registered BEFORE the first box action with the gate's human-attributed idle: the
            // 22:46 run logged itself after its mint and read the mint's input as a person's
            // (Everios96 23:03: "YOU KEEP SEEING YOUR OWN INPUTS")
            {
                let (_, line) = box_free();
                let idle_eff = line.split_whitespace().find_map(|w| w.strip_prefix("kbm_idle_s=")).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                crate::inputidle::runlog::start(&format!("tinyctl rethumb-all {set} c{k:03}"), idle_eff);
            }
            // tokens: ≥ 15 min left, else a mint (standalone when the plain one came back short)
            if token_left(&token_dir) < 15 * 60 {
                let _ = Command::new(tinyctl()).args(["mint-tokens"]).output();
                if token_left(&token_dir) < 15 * 60 {
                    log(&format!("{set} c{k:03}: the running game's token is short ({} s) — standalone mint", token_left(&token_dir)));
                    let _ = Command::new(tinyctl()).args(["mint-tokens", "--standalone"]).output();
                }
                log(&format!("{set} c{k:03}: tokens {} s left", token_left(&token_dir)));
            }
            let mut attempt = 0;
            loop {
                attempt += 1;
                let mut cmd = Command::new(tinyctl());
                cmd.args(["rethumb", "--uids-file", &uids.to_string_lossy(), "--work", &cw.to_string_lossy(), "--publish", "--token-dir", &token_dir]);
                cmd.env("RETHUMB_RUN_REGISTERED", "1");
                if plain.iter().any(|p| p == set) {
                    cmd.arg("--no-promote");
                }
                let tc = Instant::now();
                let out = cmd.output().map_err(|e| e.to_string())?;
                let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                let last = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
                let (done, ok, strag) = chunk_state(&cw, c.len());
                if out.status.success() && done {
                    set_ok += ok;
                    n_done_total += ok;
                    log(&format!("{set} c{k:03}: {ok} maps ok{} in {:.0} s (set {set_ok}/{}, total {n_done_total}, {:.1} h elapsed)", if strag > 0 { format!(" + {strag} straggler(s)") } else { String::new() }, tc.elapsed().as_secs_f64(), rows.len(), t0.elapsed().as_secs_f64() / 3600.0));
                    break;
                }
                if text.contains("HUMAN on the box") {
                    // not a failure of the chunk: back to the gate (30 min of idle), same chunk again
                    log(&format!("{set} c{k:03}: a person took the box — pausing"));
                    attempt -= 1;
                    close_game();
                    loop {
                        std::thread::sleep(Duration::from_secs(300));
                        let (free, _) = box_free();
                        if free && !lock_foreign() {
                            break;
                        }
                    }
                    log(&format!("{set} c{k:03}: box free again — resuming"));
                    continue;
                }
                // Nadeo's servers down (HTTP 503/502/504 on the upload or the fetch, 01:15 on
                // 11-03): not this chunk's fault — wait them out, 10 min a probe, no attempt spent
                let results_text = std::fs::read_to_string(cw.join("results.tsv")).unwrap_or_default();
                if results_text.contains("HTTP 503") || results_text.contains("HTTP 502") || results_text.contains("HTTP 504") || text.contains("HTTP 503") {
                    attempt -= 1;
                    log(&format!("{set} c{k:03}: Nadeo servers answer 5xx — waiting 10 min"));
                    discord(&format!("Thumbnails paused ({set} {}/{}): Nadeo's servers answer errors (5xx) — not the render machine; retrying every 10 min. -- ai", set_ok, rows.len()));
                    loop {
                        std::thread::sleep(Duration::from_secs(600));
                        let probe = Command::new("curl").args(["-s", "-m", "30", "-o", "/dev/null", "-w", "%{http_code}", "https://prod.trackmania.core.nadeo.online/"]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
                        if !probe.starts_with('5') {
                            log(&format!("{set} c{k:03}: Nadeo answers {probe} — resuming"));
                            discord(&format!("Thumbnails resumed ({set} {}/{}): Nadeo's servers are back. -- ai", set_ok, rows.len()));
                            break;
                        }
                    }
                    continue;
                }
                log(&format!("{set} c{k:03}: attempt {attempt} failed — {}", last.chars().take(160).collect::<String>()));
                if attempt >= 3 {
                    log(&format!("{set} c{k:03}: GIVING UP on this chunk (see {})", cw.display()));
                    break;
                }
                std::thread::sleep(Duration::from_secs(300));
            }
        }
        log(&format!("set {set}: {set_ok}/{} maps done", rows.len()));
        // announced once: a restart re-walks finished sets (every chunk already done) and
        // must not repeat the line (03:04 on 11-03)
        let marker = work.join(set).join("announced");
        if !marker.exists() {
            discord(&format!("Thumbnails: {set} done — {set_ok} of {} maps have new pictures (live, in place; the rest are stragglers for a final pass). -- ai", rows.len()));
            let _ = std::fs::write(&marker, "1");
        }
    }
    close_game();
    log(&format!("rethumb-all: {n_done_total} maps this run, {:.1} h", t0.elapsed().as_secs_f64() / 3600.0));
    Ok(())
}

/// One line to Everios96's DM (the rollout's waits and finishes; his ask 2026-11-02 23:01).
fn discord(msg: &str) {
    let cmd = format!("~/bin/discordctl send 1548403109309579264 '{}'", msg.replace('\'', ""));
    let _ = Command::new("/home/vjeux/bin/whitestick").arg(&cmd).output();
}

fn chrono_like() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // PT (UTC−8 in November) wall clock, hh:mm:ss
    let local = secs as i64 - 8 * 3600;
    let s = local.rem_euclid(86400);
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}
