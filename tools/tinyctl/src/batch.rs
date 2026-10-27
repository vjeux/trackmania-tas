//! Batch publishing — the whole-club job (Everios96's ~975 u10s maps into the
//! unpublished club "Tiny U10S", 2026-09-13), in two halves like `publish-map`:
//!
//! - `tinyctl publish-batch --manifest M.tsv --results R.tsv [--allow-rename] [--club C --campaign K
//!   --campaign-name N | --no-campaign] [--outdir D] [--detach]` runs ON THE BOX: one token mint
//!   (re-minted on a 401), then per manifest row (`path<TAB>name`) the Nadeo
//!   upload (create, or update when the uid exists) and the stored-bytes md5
//!   readback, one result row each; at the end ONE playlist write with every uid
//!   that uploaded, in manifest order. The readback copies are deleted as it goes
//!   (C: is at 99 %). `--no-campaign` = a STORAGE-ONLY upload, stated: the maps go
//!   into Nadeo map storage under the uploading account (a client joining a server
//!   that runs them fetches them by uid from Nadeo's CDN) and into no playlist — not
//!   listed anywhere in-game (the 16 MK64 Bonk Cup maps for Shengar's dedicated
//!   server, 2026-10-17). Omitting the campaign flags does the same silently (the
//!   in-place UPDATE callers rely on it); the flag is for a human-run batch, so the
//!   intent is in the command and the results file, and it refuses a campaign flag
//!   beside it.
//! - `tinyctl publish-set --parts 01,02,… --out-root R [--tag u10s] [--out-prefix U10S]
//!   --club C [--campaign-prefix "Tiny U10S PART "] [--results DIR]` runs on the
//!   devserver: per part it gates every built map (item-check with the packs, the
//!   25 MiB cap, a `Tin` uid), pushes the maps and a manifest to the box, creates
//!   the part's campaign (or reuses the id in `campaigns.tsv`), runs `publish-batch
//!   --detach`, waits, pulls the results and deletes the box copies.
//!
//! Manifest: `path<TAB>name`; results: `path<TAB>name<TAB>uid<TAB>mapId<TAB>how<TAB>bytes<TAB>md5<TAB>verdict`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::publish::{json_str, CORE, LIVE};
use crate::wsx::Wsx;

const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";
const BOX_BATCH_DIR: &str = "/home/vjeux/shoot/u10s/batch";

fn curl(args: &[&str]) -> Result<(String, String), String> {
    // --max-time: `TINYCTL_CURL_MAX_TIME` (default 600 s). 2026-10-24: Nadeo's core service
    // hung one upload in ten for 5-7 minutes (a 4 MB body that normally lands in 8 s); at
    // 600 s a hung worker idled the batch — 2 maps/min against 16 the day before. 150 s
    // with 2 retries turns a hang into a 2.5-minute retry; a re-upload of the same bytes
    // is idempotent (update by uid, md5 read back).
    let max_time = std::env::var("TINYCTL_CURL_MAX_TIME").unwrap_or_else(|_| "600".into());
    let retries = std::env::var("TINYCTL_CURL_RETRY").unwrap_or_else(|_| "1".into());
    let out = Command::new("curl").arg("-s").arg("--max-time").arg(&max_time).arg("--retry").arg(&retries).arg("-w").arg("\n%{http_code}").args(args).output().map_err(|e| format!("curl: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = match text.rfind('\n') {
        Some(i) => (text[..i].to_string(), text[i + 1..].trim().to_string()),
        None => (text.clone(), String::new()),
    };
    if !out.status.success() {
        return Err(format!("curl exit {}: {}", out.status, String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok((body, code))
}

fn md5_hex(data: &[u8]) -> String {
    mapgeom::md5::md5(data).iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Clone)]
struct Tokens {
    core: String,
    live: String,
    me: String,
    shootctl: String,
    /// `--files-only`: the plugin's token files are the only source (no game, no lock);
    /// a 401 ends the batch with "re-mint" instead of calling the plugin
    files_only: bool,
}

impl Tokens {
    /// The plugin's token FILES, no game call: the uploads themselves are plain HTTPS to
    /// Nadeo's core service, so with a young token the batch runs from any process,
    /// outside the game lock and in parallel (vjeux 2026-09-24: "figure out what endpoint
    /// is used for uploading so you can do it without the game").
    fn from_files(dir: &str) -> Result<Tokens, String> {
        let (core, live) = crate::nadeo::fresh_tokens_in(dir).ok_or_else(|| format!("no fresh token files in {dir} (older than 20 min or absent): mint first — a 3-second `shootctl get /nadeotoken?aud=NadeoServices` + `…aud=NadeoLiveServices` under tmdrive"))?;
        let (mine, c) = curl(&["-H", &format!("Authorization: {live}"), &format!("{LIVE}/api/token/club/mine?length=1&offset=0")])?;
        let me = json_str(&mine, "authorAccountId").ok_or_else(|| format!("club/mine: HTTP {c} without authorAccountId (token files stale?)"))?;
        Ok(Tokens { core, live, me, shootctl: String::new(), files_only: true })
    }
    fn mint(shootctl: &str) -> Result<Tokens, String> {
        // The plugin's token files are reused when young, but the game rotates its
        // live token on its own schedule (a 401 on club/mine with 20-minute-old
        // files, 2026-09-13): a reused token is VALIDATED here, and a 401 drops the
        // files and mints fresh once before giving up.
        for attempt in 0..2 {
            let (core, live) = crate::nadeo::batch_tokens(shootctl)?;
            let (mine, c) = curl(&["-H", &format!("Authorization: {live}"), &format!("{LIVE}/api/token/club/mine?length=1&offset=0")])?;
            if let Some(me) = json_str(&mine, "authorAccountId") {
                return Ok(Tokens { core, live, me, shootctl: shootctl.to_string(), files_only: false });
            }
            if attempt == 0 {
                println!("tokens: club/mine HTTP {c} with the reused files — minting fresh");
                for aud in ["NadeoServices", "NadeoLiveServices"] {
                    let _ = std::fs::remove_file(format!("{}/token-{aud}.txt", crate::publish::STORE));
                }
                continue;
            }
            return Err(format!("club/mine: HTTP {c} without authorAccountId: {}", &mine[..mine.len().min(200)]));
        }
        unreachable!()
    }
    /// A 401 means the game rotated its token: drop the files and mint again.
    fn refresh(&mut self) -> Result<(), String> {
        if self.files_only {
            return Err("token expired (HTTP 401) — files-only mode: re-mint and resume".into());
        }
        for aud in ["NadeoServices", "NadeoLiveServices"] {
            let _ = std::fs::remove_file(format!("{}/token-{aud}.txt", crate::publish::STORE));
        }
        let t = Tokens::mint(&self.shootctl)?;
        self.core = t.core;
        self.live = t.live;
        self.me = t.me;
        Ok(())
    }
}

/// One map: create/update + readback. Returns (uid, mapId, how, bytes, md5, verdict).
fn upload_one(t: &mut Tokens, map: &Path, name: &str, outdir: &Path) -> Result<(String, String, String, u64, String, String), String> {
    let path = map.to_str().ok_or("map path is not utf-8")?;
    let hdr = tmmaps::header::read(path)?;
    let uid = hdr.uid.clone();
    if uid == "-" || uid.is_empty() {
        return Err("the map header has no uid".into());
    }
    let num = |s: &str, what: &str| s.parse::<i64>().map_err(|_| format!("header {what} `{s}` is not a number"));
    let (at, gold, silver, bronze) = (num(&hdr.authortime, "authortime")?, num(&hdr.gold, "gold")?, num(&hdr.silver, "silver")?, num(&hdr.bronze, "bronze")?);
    let envir = if hdr.envir == "-" { "Stadium".to_string() } else { hdr.envir.clone() };
    let bytes = std::fs::read(map).map_err(|e| format!("{path}: {e}"))?;
    let md5_local = md5_hex(&bytes);
    // the existing record (a 401 → re-mint once)
    let mut rec_code;
    let mut rec;
    let mut tries = 0;
    loop {
        let auth_core = format!("Authorization: {}", t.core);
        let r = curl(&["-H", &auth_core, &format!("{CORE}/maps/?mapUidList={uid}")])?;
        rec = r.0;
        rec_code = r.1;
        if rec_code == "401" && tries == 0 {
            tries += 1;
            t.refresh()?;
            continue;
        }
        break;
    }
    if rec_code != "200" {
        return Err(format!("GET /maps/?mapUidList: HTTP {rec_code} {}", &rec[..rec.len().min(300)]));
    }
    let existing = json_str(&rec, "mapId");
    // ⛔ THE LIVE-MAP GUARD (2026-10-08). A build's uid is derived from its source's,
    // so a TEST build of map 23 carried the very uid the live Dirt/Grass/Plastic/Wood
    // 23 had in the hunt club — and this update route silently replaced those four
    // live maps with "U10S 23 dirt test" (test medals, unagreed physics) for hours.
    // An update that would CHANGE the stored name is not an in-place fix of our own
    // map, it is a collision with someone's live map: refuse it. A real rename passes
    // `--allow-rename` (TINYCTL_ALLOW_RENAME=1) on purpose.
    if existing.is_some() {
        // …and a record ANOTHER account owns is never ours to update, whatever its name:
        // the MK64 Bonk Cup's fresh uid Ek0fmwVE… (Rainbow Road) was already online under
        // account ac30c580… since 2026-10-08 — same name, someone else's map (2026-10-17).
        // The UPLOADER is the record's `submitter`; `author` is free metadata the upload
        // sets (2026-10-19: six Wet Wood maps created on 2026-09-20 carry Everios96's
        // editor login as `author` — the publish path of that evening sent the header's
        // author — with the bot as `submitter`; the guard read `author` and refused the
        // bot's own maps, and I told Everios he had uploaded them. He had not.)
        let owner = json_str(&rec, "submitter").or_else(|| json_str(&rec, "author")).unwrap_or_default();
        if !owner.is_empty() && !t.me.is_empty() && owner != t.me {
            return Err(format!("REFUSED: uid {uid} is already online as `{}` owned by account {owner}, not {} — someone else's live map; a fresh uid is needed", json_str(&rec, "name").unwrap_or_default(), t.me));
        }
        let stored = json_str(&rec, "name").unwrap_or_default();
        let allow = std::env::var("TINYCTL_ALLOW_RENAME").map(|v| v == "1").unwrap_or(false);
        if !stored.is_empty() && stored != name && !allow {
            return Err(format!(
                "REFUSED: uid {uid} is already online as `{stored}` and this upload would rename it to `{name}` — \
                 that is a LIVE map, not ours to overwrite (the 2026-10-08 map-23 incident). Re-uid the test build, \
                 or pass --allow-rename if the rename is intended."
            ));
        }
    }
    let auth_core = format!("Authorization: {}", t.core);
    let params = format!(
        "{{\"isPlayable\":true,\"author\":\"{}\",\"authorScore\":{at},\"bronzeScore\":{bronze},\"silverScore\":{silver},\"goldScore\":{gold},\"collectionName\":\"{envir}\",\"mapStyle\":\"\",\"mapType\":\"TrackMania\\\\TM_Race\",\"name\":\"{}\",\"mapUid\":\"{uid}\"}}",
        t.me,
        name.replace('"', "\\\"")
    );
    let route = match &existing {
        Some(id) => format!("{CORE}/maps/{id}"),
        None => format!("{CORE}/maps/"),
    };
    let (up, code) = curl(&["-H", &auth_core, "-F", &format!("nadeoservices-core-parameters={params};type=application/json"), "-F", &format!("data=@{path};type=application/octet-stream;filename={}.Map.Gbx", name.replace('/', "-")), &route])?;
    if !code.starts_with('2') {
        return Err(format!("upload {}: HTTP {code} {}", if existing.is_some() { "(update)" } else { "(create)" }, &up[..up.len().min(400)]));
    }
    let how = if existing.is_some() { "update" } else { "create" }.to_string();
    let map_id = json_str(&up, "mapId").or(existing.clone()).unwrap_or_default();
    // read back: the stored bytes must be ours. An UPDATE is not instant at the file URL
    // (the Tiny in-place round of 2026-09-26: 31 of 552 read the OLD bytes right after a
    // 200, all of them ours a minute later) — the readback is retried with a pause.
    let back = outdir.join(format!("readback-{uid}.Map.Gbx"));
    let mut verdict = String::new();
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(20 * attempt as u64));
        }
        let (rec2, _) = curl(&["-H", &auth_core, &format!("{CORE}/maps/?mapUidList={uid}")])?;
        let file_url = json_str(&rec2, "fileUrl").ok_or("record has no fileUrl after upload")?;
        let (_, code) = curl(&["-L", "-H", &auth_core, "-o", back.to_str().unwrap(), &file_url])?;
        let stored = std::fs::read(&back).map_err(|e| format!("{}: {e}", back.display()))?;
        let _ = std::fs::remove_file(&back);
        let md5_stored = md5_hex(&stored);
        if md5_stored == md5_local {
            verdict = "IDENTICAL".to_string();
            break;
        }
        verdict = format!("DIFFERS (stored HTTP {code}, {} bytes, md5 {md5_stored}, {} readbacks)", stored.len(), attempt + 1);
    }
    Ok((uid, map_id, how, bytes.len() as u64, md5_local, verdict))
}

/// `tinyctl skip-manifest --maps DIR --prev RESULTS_DIR[,DIR…] --out M.tsv` — a
/// publish-batch manifest for every `*.Map.Gbx` in DIR (name = the file stem),
/// where a map whose bytes Nadeo already holds becomes a `skip` row: the newest
/// previous results row of that NAME (results-*.tsv under the --prev dirs, later
/// dirs override earlier ones; columns path name uid mapId how bytes md5
/// verdict) carries the same md5. The fix passes of 2026-09-14 rebuild a whole
/// set on the box and upload only what changed.
pub fn skip_manifest_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps = PathBuf::from(f("--maps").ok_or("skip-manifest needs --maps DIR")?);
    let out = PathBuf::from(f("--out").ok_or("skip-manifest needs --out M.tsv")?);
    let mut prev: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new(); // name -> (uid, md5)
    for dir in f("--prev").unwrap_or_default().split(',').map(str::trim).filter(|d| !d.is_empty()) {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir).map_err(|e| format!("{dir}: {e}"))?.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("results-") && n.ends_with(".tsv")).unwrap_or(false)).collect();
        files.sort();
        for p in files {
            for line in std::fs::read_to_string(&p).unwrap_or_default().lines().skip(1) {
                let c: Vec<&str> = line.split('\t').collect();
                if c.len() >= 8 && c[7] == "IDENTICAL" && c[6] != "-" {
                    prev.insert(c[1].to_string(), (c[2].to_string(), c[6].to_string()));
                }
            }
        }
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&maps).map_err(|e| format!("{}: {e}", maps.display()))?.flatten().map(|e| e.path()).filter(|p| p.to_str().map(|s| s.ends_with(".Map.Gbx")).unwrap_or(false)).collect();
    entries.sort();
    let (mut skipped, mut upload) = (0usize, 0usize);
    let mut m = String::new();
    for p in &entries {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").trim_end_matches(".Map.Gbx").to_string();
        let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let md5 = md5_hex(&bytes);
        match prev.get(&name) {
            Some((uid, pm)) if *pm == md5 => {
                m.push_str(&format!("{}\t{name}\t{uid}\tskip\t{md5}\n", p.display()));
                skipped += 1;
            }
            _ => {
                m.push_str(&format!("{}\t{name}\n", p.display()));
                upload += 1;
            }
        }
    }
    std::fs::write(&out, m).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("skip-manifest: {} maps, {upload} to upload, {skipped} already on Nadeo -> {}", entries.len(), out.display());
    Ok(())
}

pub fn publish_batch_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let manifest = PathBuf::from(f("--manifest").ok_or("publish-batch needs --manifest M.tsv")?);
    let results = PathBuf::from(f("--results").ok_or("publish-batch needs --results R.tsv")?);
    let no_campaign = tmmaps::cli::has(args, "--no-campaign");
    if no_campaign && (f("--club").is_some() || f("--campaign").is_some() || f("--campaign-name").is_some()) {
        return Err("publish-batch: --no-campaign (storage-only upload) and --club/--campaign/--campaign-name together: pick one".into());
    }
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| BOX_BATCH_DIR.into()));
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let done = results.with_extension("done");
    let _ = std::fs::remove_file(&done);
    if tmmaps::cli::has(args, "--detach") {
        use std::os::unix::process::CommandExt;
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        let rest: Vec<String> = std::env::args().skip(1).filter(|a| a != "--detach").collect();
        let log = results.with_extension("log");
        let out = std::fs::File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let child = Command::new(exe).args(&rest).stdin(std::process::Stdio::null()).stdout(out).stderr(err).process_group(0).spawn().map_err(|e| format!("spawn: {e}"))?;
        println!("detached pid {} — log {} — done file {}", child.id(), log.display(), done.display());
        return Ok(());
    }
    let t0 = Instant::now();
    let shootctl = f("--shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    // rows: path<TAB>name[<TAB>uid<TAB>skip] — a `skip` row is already on Nadeo: no upload,
    // its uid still takes its place in the playlist
    let rows: Vec<(PathBuf, String, Option<String>)> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c.len() >= 2).then(|| (PathBuf::from(c[0].trim()), c[1].trim().to_string(), (c.len() >= 4 && c[3].trim() == "skip").then(|| format!("{}\t{}", c[2].trim(), c.get(4).map(|s| s.trim()).unwrap_or("-")))))
        })
        .collect();
    let jobs: usize = f("--jobs").and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
    let files_only = tmmaps::cli::has(args, "--files-only");
    // `--allow-rename`: an update may change the stored name (see the guard in upload_one)
    if tmmaps::cli::has(args, "--allow-rename") {
        std::env::set_var("TINYCTL_ALLOW_RENAME", "1");
    }
    let summary = (|| -> Result<String, String> {
        let token_dir = f("--token-dir").unwrap_or_else(|| crate::publish::STORE.to_string());
        let mut t = if files_only { Tokens::from_files(&token_dir)? } else { Tokens::mint(&shootctl)? };
        let mut out = String::from("path\tname\tuid\tmapId\thow\tbytes\tmd5\tverdict\n");
        let mut uids: Vec<String> = Vec::new();
        let mut failed = 0usize;
        if jobs > 1 {
            // `--jobs N`: N uploads at once, each worker with its own copy of the tokens (plain
            // HTTPS, no game); results land in manifest order, the file is rewritten as they come
            let n = rows.len();
            let next = std::sync::atomic::AtomicUsize::new(0);
            let slots: std::sync::Mutex<Vec<Option<(String, bool, Option<String>)>>> = std::sync::Mutex::new(vec![None; n]);
            let stop = std::sync::atomic::AtomicBool::new(false);
            std::thread::scope(|sc| {
                for _w in 0..jobs.min(n.max(1)) {
                    let mut tw = t.clone();
                    let (rows, slots, next, stop, outdir, results) = (&rows, &slots, &next, &stop, &outdir, &results);
                    sc.spawn(move || loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if i >= n || stop.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }
                        let (p, name, skip) = &rows[i];
                        let t1 = Instant::now();
                        let entry = if let Some(skip) = skip {
                            let (uid, md5) = skip.split_once('\t').unwrap_or((skip.as_str(), "-"));
                            println!("[{:>4.0}s] {}/{} {name}\t{uid}\talready on Nadeo — playlist only", t0.elapsed().as_secs_f64(), i + 1, n);
                            (format!("{}\t{name}\t{uid}\t-\tskip\t0\t{md5}\tIDENTICAL\n", p.display()), true, Some(uid.to_string()))
                        } else {
                            // a curl transport failure (HTTP/2 PROTOCOL_ERROR, SSL_read reset — 5 of
                            // 44 uploads on 2026-09-25) is retried up to 3 times; a map that still
                            // fails keeps its slot empty in the playlist call below
                            let mut r = upload_one(&mut tw, p, name, outdir);
                            for _ in 0..3 {
                                match &r {
                                    Err(e) if e.contains("curl") && !e.contains("token expired") => {
                                        println!("[{:>4.0}s] {}/{} {name}\tretrying after: {}", t0.elapsed().as_secs_f64(), i + 1, n, e.lines().next().unwrap_or("").chars().take(80).collect::<String>());
                                        r = upload_one(&mut tw, p, name, outdir);
                                    }
                                    _ => break,
                                }
                            }
                            match r {
                                Ok((uid, map_id, how, bytes, md5, verdict)) => {
                                    println!("[{:>4.0}s] {}/{} {name}\t{uid}\t{how}\t{bytes} B\t{verdict}\t({:.0}s)", t0.elapsed().as_secs_f64(), i + 1, n, t1.elapsed().as_secs_f64());
                                    let ok = verdict == "IDENTICAL";
                                    (format!("{}\t{name}\t{uid}\t{map_id}\t{how}\t{bytes}\t{md5}\t{verdict}\n", p.display()), ok, ok.then_some(uid))
                                }
                                Err(e) => {
                                    println!("[{:>4.0}s] {}/{} {name}\tFAILED\t{e}", t0.elapsed().as_secs_f64(), i + 1, n);
                                    if e.contains("token expired") {
                                        stop.store(true, std::sync::atomic::Ordering::SeqCst);
                                    }
                                    (format!("{}\t{name}\t-\t-\t-\t0\t-\tFAILED {}\n", p.display(), e.lines().next().unwrap_or("").replace('\t', " ")), false, None)
                                }
                            }
                        };
                        slots.lock().unwrap()[i] = Some(entry);
                        // the results file after every map, in manifest order (the done ones so far)
                        let mut text = String::from("path\tname\tuid\tmapId\thow\tbytes\tmd5\tverdict\n");
                        for e in slots.lock().unwrap().iter().flatten() {
                            text.push_str(&e.0);
                        }
                        let _ = std::fs::write(results, &text);
                    });
                }
            });
            let done = slots.into_inner().unwrap();
            for e in done.iter() {
                match e {
                    Some((line, ok, uid)) => {
                        out.push_str(line);
                        if *ok {
                            if let Some(u) = uid {
                                uids.push(u.clone());
                            }
                        } else {
                            failed += 1;
                        }
                    }
                    None => failed += 1,
                }
            }
            std::fs::write(&results, &out).map_err(|e| format!("{}: {e}", results.display()))?;
        } else {
        // `--remint`: a long files-only batch (609 Tiny+Giant in-place updates, 2026-10-26)
        // outlives one token (~1 h): when the live token's own expiry is under 4 min away,
        // or an upload comes back 401, the box mints fresh tokens (roundpub::mint_tokens,
        // the same mint `round-publish` uses) and the batch goes on with the same row.
        let remint = f("--remint").is_some() || args.iter().any(|a| a == "--remint");
        let wsx = crate::wsx::Wsx::new(args);
        let near_expiry = |dir: &str| -> bool {
            let p = format!("{dir}/token-live.txt");
            let tok = std::fs::read_to_string(&p).unwrap_or_default();
            match crate::nadeo::jwt_exp(tok.trim()) {
                Some(exp) => {
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                    exp < now + 240
                }
                None => false,
            }
        };
        for (i, (p, name, skip)) in rows.iter().enumerate() {
            let t1 = Instant::now();
            if let Some(skip) = skip {
                let (uid, md5) = skip.split_once('\t').unwrap_or((skip.as_str(), "-"));
                println!("[{:>4.0}s] {}/{} {name}\t{uid}\talready on Nadeo — playlist only", t0.elapsed().as_secs_f64(), i + 1, rows.len());
                uids.push(uid.to_string());
                out.push_str(&format!("{}\t{name}\t{uid}\t-\tskip\t0\t{md5}\tIDENTICAL\n", p.display()));
                continue;
            }
            if remint && t.files_only && near_expiry(&token_dir) {
                println!("[{:>4.0}s] token expiry under 4 min — re-minting on the box", t0.elapsed().as_secs_f64());
                crate::roundpub::mint_tokens(&wsx, std::path::Path::new(&token_dir))?;
                t = Tokens::from_files(&token_dir)?;
            }
            let mut attempt = upload_one(&mut t, p, name, &outdir);
            if remint && t.files_only {
                if let Err(e) = &attempt {
                    if e.contains("401") {
                        println!("[{:>4.0}s] {name}: 401 — re-minting on the box and retrying", t0.elapsed().as_secs_f64());
                        crate::roundpub::mint_tokens(&wsx, std::path::Path::new(&token_dir))?;
                        t = Tokens::from_files(&token_dir)?;
                        attempt = upload_one(&mut t, p, name, &outdir);
                    }
                }
            }
            match attempt {
                Ok((uid, map_id, how, bytes, md5, verdict)) => {
                    println!("[{:>4.0}s] {}/{} {name}\t{uid}\t{how}\t{bytes} B\t{verdict}\t({:.0}s)", t0.elapsed().as_secs_f64(), i + 1, rows.len(), t1.elapsed().as_secs_f64());
                    if verdict == "IDENTICAL" {
                        uids.push(uid.clone());
                    } else {
                        failed += 1;
                    }
                    out.push_str(&format!("{}\t{name}\t{uid}\t{map_id}\t{how}\t{bytes}\t{md5}\t{verdict}\n", p.display()));
                }
                Err(e) => {
                    failed += 1;
                    println!("[{:>4.0}s] {}/{} {name}\tFAILED\t{e}", t0.elapsed().as_secs_f64(), i + 1, rows.len());
                    out.push_str(&format!("{}\t{name}\t-\t-\t-\t0\t-\tFAILED {}\n", p.display(), e.lines().next().unwrap_or("").replace('\t', " ")));
                }
            }
            std::fs::write(&results, &out).map_err(|e| format!("{}: {e}", results.display()))?;
        }
        }
        let mut camp_line = String::from(if no_campaign { "campaign\tnone (--no-campaign: storage-only upload, no playlist)" } else { "campaign\tnot set" });
        if failed > 0 && f("--campaign").is_some() {
            // a playlist written with the failed maps missing DROPPED them from the campaign
            // (Summer Wood 2026-09-25: 20 of 22 in the playlist) — the campaign is left as it
            // was; rerun with the failed rows and the rest as skip rows
            camp_line = format!("campaign\t{}\tNOT SET: {failed} upload(s) failed — the playlist would lose their slots; rerun the failed rows", f("--campaign").unwrap_or_default());
            println!("{camp_line}");
            let tail = format!("{}\n", camp_line);
            out.push_str(&tail);
            std::fs::write(&results, &out).map_err(|e| format!("{}: {e}", results.display()))?;
            return Err(format!("{failed} upload(s) failed; {camp_line}"));
        }
        if let (Some(club), Some(camp), Some(cname)) = (f("--club"), f("--campaign"), f("--campaign-name")) {
            let auth_live = format!("Authorization: {}", t.live);
            match crate::nadeo::campaign_set(&auth_live, &club, &camp, &cname, &uids) {
                Ok(v) => {
                    let n = serde_json::to_string(&v).unwrap_or_default().matches("\"mapUid\"").count();
                    camp_line = format!("campaign\t{camp}\t{cname}\t{n} maps in the playlist");
                }
                Err(e) => {
                    failed += 1;
                    camp_line = format!("campaign\t{camp}\tFAILED {}", e.lines().next().unwrap_or(""));
                }
            }
        }
        println!("{camp_line}");
        out.push_str(&format!("# {camp_line}\n"));
        std::fs::write(&results, &out).map_err(|e| format!("{}: {e}", results.display()))?;
        Ok(format!("{} of {} uploaded IDENTICAL, {failed} failed; {camp_line}", uids.len(), rows.len()))
    })();
    let text = match &summary {
        Ok(s) => format!("OK in {:.0}s\n{s}\n", t0.elapsed().as_secs_f64()),
        Err(e) => format!("FAILED after {:.0}s: {e}\n", t0.elapsed().as_secs_f64()),
    };
    print!("{text}");
    let tmp = done.with_extension("tmp");
    std::fs::write(&tmp, &text).and_then(|_| std::fs::rename(&tmp, &done)).map_err(|e| format!("{}: {e}", done.display()))?;
    summary.map(|_| ())
}

// ---------------------------------------------------------------- devserver

/// The gate `publish-map` runs, for one built map: a `Tin` uid, times present, the
/// 25 MiB cap, item-check over the library with the collection's packs.
fn gate(map: &Path, items_dir: &Path, paks: &str) -> Result<(String, String), String> {
    let hdr = tmmaps::header::read(map.to_str().ok_or("map path")?)?;
    if hdr.uid == "-" || hdr.authortime == "-" || hdr.envir == "-" {
        return Err("the header lacks uid / authortime / envir".into());
    }
    if !hdr.uid.starts_with("Tin") {
        return Err(format!("uid {} does not start with `Tin`", hdr.uid));
    }
    const NADEO_MAX_BYTES: u64 = 25 * 1024 * 1024;
    let size = std::fs::metadata(map).map(|m| m.len()).unwrap_or(0);
    if size > NADEO_MAX_BYTES {
        return Err(format!("{size} bytes is over Nadeo's 25 MiB cap"));
    }
    let mut items: Vec<String> = std::fs::read_dir(items_dir).map_err(|e| format!("{}: {e}", items_dir.display()))?.filter_map(|e| e.ok()).map(|e| e.path().to_string_lossy().into_owned()).filter(|p| p.ends_with(".Item.Gbx")).collect();
    items.sort();
    if items.is_empty() {
        return Err(format!("{}: no .Item.Gbx files", items_dir.display()));
    }
    let mut rest: Vec<String> = vec!["item-check".into()];
    rest.extend(items.iter().cloned());
    let mut open = || {
        let mut store = mapgeom::store::DataStore::empty();
        let toks: Vec<&str> = paks.split_whitespace().collect();
        let mut i = 0;
        while i < toks.len() {
            if toks[i] == "--pak" {
                if let Some((p, k)) = toks.get(i + 1).and_then(|s| s.rsplit_once(':')) {
                    if let Err(e) = store.add_pak(p, k) {
                        eprintln!("--paks: {p}: {e}");
                    }
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        store
    };
    // item-check prints one line per item; keep the log quiet
    mapgeom::static_item::check::run(&rest, &mut open).map_err(|e| format!("item-check refused the library ({e})"))?;
    Ok((hdr.name.clone(), hdr.uid.clone()))
}

/// The campaign ids already created, part -> (campaignId, activityId), in
/// `campaigns.tsv` next to the results.
struct Campaigns {
    path: PathBuf,
    known: std::sync::Mutex<std::collections::BTreeMap<String, (String, String)>>,
}

impl Campaigns {
    fn load(path: PathBuf) -> Campaigns {
        let known = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                (c.len() >= 3).then(|| (c[0].to_string(), (c[1].to_string(), c[2].to_string())))
            })
            .collect();
        Campaigns { path, known: std::sync::Mutex::new(known) }
    }
    fn get(&self, part: &str) -> Option<(String, String)> {
        self.known.lock().unwrap().get(part).cloned()
    }
    fn put(&self, part: &str, id: String, act: String) {
        let mut k = self.known.lock().unwrap();
        k.insert(part.to_string(), (id, act));
        let mut rows = String::new();
        for (p, (i, a)) in k.iter() {
            rows.push_str(&format!("{p}\t{i}\t{a}\n"));
        }
        let _ = std::fs::write(&self.path, rows);
    }
}

struct SetCfg {
    out_root: PathBuf,
    tag: String,
    prefix: String,
    club: String,
    camp_prefix: String,
    results_dir: PathBuf,
    box_tinyctl: String,
    paks: String,
    force: bool,
    args: Vec<String>,
}

/// One part: gate, manifest (previous IDENTICAL rows become skip rows), push,
/// campaign, batch, pull, cleanup. Returns the verdict line.
fn publish_part(cfg: &SetCfg, camps: &Campaigns, part: &str) -> String {
    let t0 = Instant::now();
    let wsx = Wsx::new(&cfg.args);
    let part_dir = cfg.out_root.join(format!("p{part}"));
    let camp_name = format!("{}{}", cfg.camp_prefix, part.trim_start_matches('0'));
    println!("\n===== part {part}: {camp_name} ({}) =====", part_dir.display());
    let mut maps: Vec<(String, PathBuf, PathBuf)> = Vec::new();
    for nn in 1..=99usize {
        let d = part_dir.join(format!("tiny{nn:02}")).join(&cfg.tag);
        let m = d.join(format!("{}-{nn:02}-Tiny.Map.Gbx", cfg.prefix));
        if m.exists() {
            maps.push((format!("{nn:02}"), m, d.join("libx").join("Items")));
        }
    }
    if maps.is_empty() {
        return format!("{part}\tFAILED\tno built maps under {}", part_dir.display());
    }
    // what an earlier run already put on Nadeo for this part (path -> uid)
    let prev_results = cfg.results_dir.join(format!("results-p{part}.tsv"));
    let done_before: std::collections::HashMap<String, String> = if cfg.force {
        Default::default()
    } else {
        std::fs::read_to_string(&prev_results)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                (c.len() >= 8 && c[7] == "IDENTICAL" && c[2] != "-").then(|| (c[0].rsplit('/').next().unwrap_or(c[0]).to_string(), format!("{}\t{}", c[2], c[6])))
            })
            .collect()
    };
    let mut manifest = String::new();
    let mut to_push: Vec<(String, PathBuf)> = Vec::new();
    let mut refused: Vec<String> = Vec::new();
    let mut skipped = 0usize;
    for (nn, m, items) in &maps {
        let remote = format!("{BOX_BATCH_DIR}/p{part}/{}-{nn}-Tiny.Map.Gbx", cfg.prefix);
        let file = format!("{}-{nn}-Tiny.Map.Gbx", cfg.prefix);
        // an already-uploaded map whose bytes did not change: playlist only
        if let Some(prev) = done_before.get(&file) {
            let (uid, prev_md5) = prev.split_once('\t').unwrap_or((prev.as_str(), ""));
            // same uid AND same bytes: playlist only (a rebuilt map re-uploads)
            let cur_md5 = std::fs::read(m).map(|b| md5_hex(&b)).unwrap_or_default();
            if let Ok(h) = tmmaps::header::read(m.to_str().unwrap_or("")) {
                if h.uid == uid && cur_md5 == prev_md5 {
                    manifest.push_str(&format!("{remote}\t{}\t{uid}\tskip\t{cur_md5}\n", h.name));
                    skipped += 1;
                    continue;
                }
            }
        }
        // the item gate needs the unpacked library; a build whose libx was cleaned up
        // re-extracts it from lib.zip
        if std::fs::read_dir(items).map(|rd| rd.count()).unwrap_or(0) == 0 {
            if let Some(zip) = items.parent().and_then(|l| l.parent()).map(|d| d.join("lib.zip")).filter(|z| z.exists()) {
                let _ = std::fs::create_dir_all(items.parent().unwrap());
                let _ = std::process::Command::new("unzip").arg("-q").arg("-o").arg(&zip).arg("-d").arg(items.parent().unwrap()).output();
            }
        }
        match gate(m, items, &cfg.paks) {
            Ok((name, _uid)) => {
                manifest.push_str(&format!("{remote}\t{name}\n"));
                to_push.push((nn.clone(), m.clone()));
            }
            Err(e) => {
                eprintln!("  p{part} {nn}: refused: {}", e.lines().next().unwrap_or(""));
                refused.push(format!("{nn}: {}", e.lines().next().unwrap_or("")));
            }
        }
    }
    println!("  p{part}: {} to upload, {skipped} already on Nadeo, {} refused", to_push.len(), refused.len());
    if !refused.is_empty() {
        let p = cfg.results_dir.join(format!("refused-p{part}.txt"));
        let _ = std::fs::write(&p, refused.join("\n") + "\n");
    }
    if to_push.is_empty() && skipped == 0 {
        return format!("{part}\tFAILED\tevery map refused by the gate");
    }
    let local_manifest = cfg.results_dir.join(format!("manifest-p{part}.tsv"));
    if let Err(e) = std::fs::write(&local_manifest, &manifest) {
        return format!("{part}\tFAILED\t{}: {e}", local_manifest.display());
    }
    let step = (|| -> Result<String, String> {
        wsx.sh(&format!("mkdir -p {BOX_BATCH_DIR}/p{part}"))?;
        for (nn, m) in &to_push {
            wsx.push(m, &format!("{BOX_BATCH_DIR}/p{part}/{}-{nn}-Tiny.Map.Gbx", cfg.prefix))?;
        }
        let remote_manifest = format!("{BOX_BATCH_DIR}/p{part}/manifest.tsv");
        wsx.push(&local_manifest, &remote_manifest)?;
        let (camp_id, act_id) = match camps.get(part) {
            Some(x) => x,
            None => {
                let out = wsx.sh(&format!("cd /home/vjeux/shoot/u10s && {} nadeo-here campaign-create --club {} --name '{}' --no-lock --outdir {BOX_BATCH_DIR}", cfg.box_tinyctl, cfg.club, camp_name.replace('\'', "")))?;
                let toks: Vec<&str> = out.split_whitespace().collect();
                let id = toks.iter().position(|t| *t == "campaignId").and_then(|i| toks.get(i + 1)).map(|s| s.to_string()).ok_or_else(|| format!("campaign create: {}", out.lines().last().unwrap_or("").chars().take(200).collect::<String>()))?;
                let act = toks.iter().position(|t| *t == "activityId").and_then(|i| toks.get(i + 1)).map(|s| s.to_string()).unwrap_or_else(|| "-".into());
                camps.put(part, id.clone(), act.clone());
                (id, act)
            }
        };
        println!("  p{part}: campaign {camp_id} (activity {act_id}) `{camp_name}`");
        let remote_results = format!("{BOX_BATCH_DIR}/p{part}/results.tsv");
        let cmd = format!("{} publish-batch --detach --manifest {remote_manifest} --results {remote_results} --club {} --campaign {camp_id} --campaign-name '{}' --outdir {BOX_BATCH_DIR}/p{part}", cfg.box_tinyctl, cfg.club, camp_name.replace('\'', ""));
        wsx.sh(&cmd)?;
        let done = wsx.wait_done(&format!("{BOX_BATCH_DIR}/p{part}/results.done"), &format!("{BOX_BATCH_DIR}/p{part}/results.log"), Duration::from_secs(5400), &format!("publish-batch p{part}"));
        let local_results = cfg.results_dir.join(format!("results-p{part}.tsv"));
        let pulled = wsx.pull(&remote_results, &local_results);
        let _ = wsx.sh(&format!("rm -rf {BOX_BATCH_DIR}/p{part}"));
        let text = done?;
        pulled?;
        let identical = std::fs::read_to_string(&local_results).map(|t| t.lines().filter(|l| l.ends_with("\tIDENTICAL")).count()).unwrap_or(0);
        Ok(format!("{part}\tOK\t{:.0}s\tcampaign {camp_id}\t{identical}/{} identical\t{} refused\t{}", t0.elapsed().as_secs_f64(), to_push.len() + skipped, refused.len(), text.trim().replace('\n', " | ")))
    })();
    match step {
        Ok(v) => v,
        Err(e) => format!("{part}\tFAILED\t{:.0}s\t{}", t0.elapsed().as_secs_f64(), e.lines().next().unwrap_or("")),
    }
}

pub fn publish_set_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let parts_arg = f("--parts").ok_or("publish-set needs --parts 01,02,… or 01-39 (the part directories under --out-root)")?;
    let parts: Vec<String> = if let Some((a, b)) = parts_arg.split_once('-') {
        let (a, b): (usize, usize) = (a.trim().parse().map_err(|_| "--parts A-B")?, b.trim().parse().map_err(|_| "--parts A-B")?);
        (a..=b).map(|n| format!("{n:02}")).collect()
    } else {
        parts_arg.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    };
    let out_root = PathBuf::from(f("--out-root").ok_or("publish-set needs --out-root R (R/pNN/tinyNN/<tag>/<prefix>-NN-Tiny.Map.Gbx)")?);
    let results_dir = PathBuf::from(f("--results").unwrap_or_else(|| out_root.join("publish").display().to_string()));
    std::fs::create_dir_all(&results_dir).map_err(|e| format!("{}: {e}", results_dir.display()))?;
    let cfg = SetCfg {
        tag: f("--tag").unwrap_or_else(|| "u10s".into()),
        prefix: f("--out-prefix").unwrap_or_else(|| "U10S".into()),
        club: f("--club").ok_or("publish-set needs --club ID")?,
        camp_prefix: f("--campaign-prefix").unwrap_or_else(|| "Tiny U10S PART ".into()),
        box_tinyctl: f("--box-tinyctl").unwrap_or_else(|| "/home/vjeux/shoot/u10s/tinyctl".into()),
        paks: f("--paks").unwrap_or_else(|| "--pak /tmp/current-Stadium.pak:B773D73047A4104857722366D78D28A6".into()),
        force: tmmaps::cli::has(args, "--force"),
        results_dir: results_dir.clone(),
        out_root,
        args: args.to_vec(),
    };
    let jobs: usize = f("--jobs").and_then(|j| j.parse().ok()).unwrap_or(1).max(1);
    let camps = Campaigns::load(results_dir.join("campaigns.tsv"));
    let queue = std::sync::Mutex::new(std::collections::VecDeque::from(parts.clone()));
    let verdicts = std::sync::Mutex::new(Vec::<String>::new());
    std::thread::scope(|s| {
        for _ in 0..jobs.min(parts.len()) {
            s.spawn(|| loop {
                let part = match queue.lock().unwrap().pop_front() {
                    Some(p) => p,
                    None => break,
                };
                let v = publish_part(&cfg, &camps, &part);
                println!("{v}");
                verdicts.lock().unwrap().push(v);
            });
        }
    });
    let mut verdicts = verdicts.into_inner().unwrap();
    verdicts.sort();
    println!("\n===== publish-set: {} parts =====", parts.len());
    for v in &verdicts {
        println!("{v}");
    }
    let summary = results_dir.join("PARTS.tsv");
    let mut prev = std::fs::read_to_string(&summary).unwrap_or_default();
    for v in &verdicts {
        prev.push_str(v);
        prev.push('\n');
    }
    std::fs::write(&summary, prev).map_err(|e| format!("{}: {e}", summary.display()))?;
    let failed = verdicts.iter().filter(|v| v.contains("\tFAILED\t")).count();
    if failed > 0 {
        return Err(format!("{failed} of {} parts failed", parts.len()));
    }
    Ok(())
}

/// `tinyctl tracker-club --publish-dir D --out-root O --club C --club-name N [--source-club S]
/// [--out MD] [--maps-out TSV]` — the whole-club tracker: one Markdown row per part
/// (campaign id, uploaded / built / source counts, the maps left out and why) and
/// one TSV row per map (part, nn, source name, source uid, times, tiny uid, mapId,
/// bytes, verdict).
pub fn tracker_club_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let pub_dir = PathBuf::from(f("--publish-dir").ok_or("tracker-club needs --publish-dir D")?);
    let out_root = PathBuf::from(f("--out-root").ok_or("tracker-club needs --out-root O")?);
    let club = f("--club").unwrap_or_default();
    let club_name = f("--club-name").unwrap_or_else(|| "Tiny U10S".into());
    let camps: std::collections::BTreeMap<String, (String, String)> = std::fs::read_to_string(pub_dir.join("campaigns.tsv"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c.len() >= 3).then(|| (c[0].to_string(), (c[1].to_string(), c[2].to_string())))
        })
        .collect();
    let mut md = format!("# {club_name} — club {club}: every Everios96 u10s part as a half-scale campaign\n\n| part | campaign | on Nadeo | built | source | left out |\n|---|---|---|---|---|---|\n");
    let mut tsv = String::from("part\tnn\tsource_file\tsource_name\tsource_uid\tauthortime_ms\tgold\tsilver\tbronze\ttiny_name\ttiny_uid\ttiny_bytes\titems\tmapId\tnadeo\n");
    let (mut tot_src, mut tot_built, mut tot_up) = (0usize, 0usize, 0usize);
    let mut left_out_all: Vec<String> = Vec::new();
    for part in 1..=99usize {
        let p = format!("{part:02}");
        let part_out = out_root.join(format!("p{p}"));
        let tracker = part_out.join("tracker.tsv");
        if !tracker.exists() {
            continue;
        }
        // built maps: the pipeline tracker (17 columns)
        let rows: Vec<Vec<String>> = std::fs::read_to_string(&tracker).unwrap_or_default().lines().skip(1).map(|l| l.split('\t').map(String::from).collect()).filter(|c: &Vec<String>| c.len() >= 17).collect();
        // uploaded: results-pNN.tsv (path name uid mapId how bytes md5 verdict)
        let results: std::collections::HashMap<String, (String, String)> = std::fs::read_to_string(pub_dir.join(format!("results-p{p}.tsv")))
            .unwrap_or_default()
            .lines()
            .skip(1)
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                (c.len() >= 8).then(|| (c[2].to_string(), (c[3].to_string(), c[7].to_string())))
            })
            .collect();
        let (camp_id, _) = camps.get(&p).cloned().unwrap_or_else(|| ("-".into(), "-".into()));
        let mut built = 0usize;
        let mut up = 0usize;
        let mut left: Vec<String> = Vec::new();
        for c in &rows {
            let (nn, src_file, src_name, src_uid, at, gold, silver, bronze, tiny_name, tiny_uid, bytes, items, note) = (&c[0], &c[1], &c[2], &c[3], &c[4], &c[5], &c[6], &c[7], &c[8], &c[9], &c[10], &c[11], &c[16]);
            let ok = tiny_uid != "-" && !note.contains("FAILED");
            if ok {
                built += 1;
            }
            let (map_id, verdict) = results.get(tiny_uid).cloned().unwrap_or_else(|| ("-".into(), if ok { "not uploaded".into() } else { "not built".into() }));
            if verdict == "IDENTICAL" {
                up += 1;
            } else {
                let why = if !ok { note.replace("FAILED build: 1 of 1 maps failed", "conversion failed").to_string() } else { verdict.clone() };
                left.push(format!("{src_name} ({why})"));
            }
            tsv.push_str(&format!("{p}\t{nn}\t{src_file}\t{src_name}\t{src_uid}\t{at}\t{gold}\t{silver}\t{bronze}\t{tiny_name}\t{tiny_uid}\t{bytes}\t{items}\t{map_id}\t{verdict}\n"));
        }
        let src_n = std::fs::read_dir(out_root.parent().unwrap_or(&out_root).join("src").join(format!("p{p}"))).map(|rd| rd.filter(|e| e.as_ref().map(|e| e.file_name().to_string_lossy().ends_with(".Map.Gbx")).unwrap_or(false)).count()).unwrap_or(rows.len());
        tot_src += src_n;
        tot_built += built;
        tot_up += up;
        let camp_cell = if camp_id == "-" { "-".to_string() } else { format!("Tiny U10S PART {part} (`{camp_id}`)") };
        md.push_str(&format!("| {part} | {camp_cell} | {up} | {built} | {src_n} | {} |\n", if left.is_empty() { "—".to_string() } else { left.join("; ") }));
        for l in &left {
            left_out_all.push(format!("part {part}: {l}"));
        }
    }
    md.push_str(&format!("| **all** | | **{tot_up}** | **{tot_built}** | **{tot_src}** | {} |\n", left_out_all.len()));
    if let Some(out) = f("--out") {
        std::fs::write(&out, &md).map_err(|e| format!("{out}: {e}"))?;
    } else {
        print!("{md}");
    }
    if let Some(out) = f("--maps-out") {
        std::fs::write(&out, &tsv).map_err(|e| format!("{out}: {e}"))?;
    }
    println!("{tot_up} uploaded, {tot_built} built, {tot_src} sources; left out: {}", left_out_all.len());
    for l in &left_out_all {
        println!("  {l}");
    }
    Ok(())
}
