//! Batch publishing — the whole-club job (Everios96's ~975 u10s maps into the
//! unpublished club "Tiny U10S", 2026-09-13), in two halves like `publish-map`:
//!
//! - `tinyctl publish-batch --manifest M.tsv --results R.tsv [--club C --campaign K
//!   --campaign-name N] [--outdir D] [--detach]` runs ON THE BOX: one token mint
//!   (re-minted on a 401), then per manifest row (`path<TAB>name`) the Nadeo
//!   upload (create, or update when the uid exists) and the stored-bytes md5
//!   readback, one result row each; at the end ONE playlist write with every uid
//!   that uploaded, in manifest order. The readback copies are deleted as it goes
//!   (C: is at 99 %).
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
    let out = Command::new("curl").arg("-s").arg("--max-time").arg("600").arg("--retry").arg("1").arg("-w").arg("\n%{http_code}").args(args).output().map_err(|e| format!("curl: {e}"))?;
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

struct Tokens {
    core: String,
    live: String,
    me: String,
    shootctl: String,
}

impl Tokens {
    fn mint(shootctl: &str) -> Result<Tokens, String> {
        // The plugin's token files are reused when young, but the game rotates its
        // live token on its own schedule (a 401 on club/mine with 20-minute-old
        // files, 2026-09-13): a reused token is VALIDATED here, and a 401 drops the
        // files and mints fresh once before giving up.
        for attempt in 0..2 {
            let (core, live) = crate::nadeo::batch_tokens(shootctl)?;
            let (mine, c) = curl(&["-H", &format!("Authorization: {live}"), &format!("{LIVE}/api/token/club/mine?length=1&offset=0")])?;
            if let Some(me) = json_str(&mine, "authorAccountId") {
                return Ok(Tokens { core, live, me, shootctl: shootctl.to_string() });
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
    // read back: the stored bytes must be ours
    let (rec2, _) = curl(&["-H", &auth_core, &format!("{CORE}/maps/?mapUidList={uid}")])?;
    let file_url = json_str(&rec2, "fileUrl").ok_or("record has no fileUrl after upload")?;
    let back = outdir.join(format!("readback-{uid}.Map.Gbx"));
    let (_, code) = curl(&["-L", "-H", &auth_core, "-o", back.to_str().unwrap(), &file_url])?;
    let stored = std::fs::read(&back).map_err(|e| format!("{}: {e}", back.display()))?;
    let _ = std::fs::remove_file(&back);
    let md5_stored = md5_hex(&stored);
    let verdict = if md5_stored == md5_local { "IDENTICAL".to_string() } else { format!("DIFFERS (stored HTTP {code}, {} bytes, md5 {md5_stored})", stored.len()) };
    Ok((uid, map_id, how, bytes.len() as u64, md5_local, verdict))
}

pub fn publish_batch_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let manifest = PathBuf::from(f("--manifest").ok_or("publish-batch needs --manifest M.tsv")?);
    let results = PathBuf::from(f("--results").ok_or("publish-batch needs --results R.tsv")?);
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
    let summary = (|| -> Result<String, String> {
        let mut t = Tokens::mint(&shootctl)?;
        let mut out = String::from("path\tname\tuid\tmapId\thow\tbytes\tmd5\tverdict\n");
        let mut uids: Vec<String> = Vec::new();
        let mut failed = 0usize;
        for (i, (p, name, skip)) in rows.iter().enumerate() {
            let t1 = Instant::now();
            if let Some(skip) = skip {
                let (uid, md5) = skip.split_once('\t').unwrap_or((skip.as_str(), "-"));
                println!("[{:>4.0}s] {}/{} {name}\t{uid}\talready on Nadeo — playlist only", t0.elapsed().as_secs_f64(), i + 1, rows.len());
                uids.push(uid.to_string());
                out.push_str(&format!("{}\t{name}\t{uid}\t-\tskip\t0\t{md5}\tIDENTICAL\n", p.display()));
                continue;
            }
            match upload_one(&mut t, p, name, &outdir) {
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
        let mut camp_line = String::from("campaign\tnot set");
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
    if !(hdr.uid.starts_with("Tin") || hdr.uid.starts_with("Gia") || hdr.uid.starts_with("Sam")) {
        return Err(format!("uid {} does not start with `Tin`/`Gia`/`Sam`", hdr.uid));
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

/// `tinyctl publish-dir --maps A.Map.Gbx,B.Map.Gbx,… --items-dirs D1,D2,… --club C --campaign-name NAME
///                     [--campaign ID] [--results DIR] [--tag giant-x2] [--box-tinyctl P] [--force]`
/// — one campaign from an explicit list of built maps (the giant Summer
/// campaigns, 2026-09-22: 25 files per scale, five collections, so the packs
/// of the item-check gate come from each map's own collection). The same
/// halves as `publish-set`: the gate here (item-check, the 25 MiB cap, a
/// `Gia`/`Tin` uid), the maps and a manifest pushed to the box, the campaign
/// created (or `--campaign ID` reused; the id is remembered in
/// `<results>/campaigns.tsv` under the campaign name), `publish-batch --detach`
/// there (one token mint, create-or-update per map, stored-bytes md5 readback,
/// ONE playlist write in manifest order), the results pulled back. Maps whose
/// previous result was IDENTICAL at the same md5 are playlist-only skip rows
/// unless `--force`.
pub fn publish_dir_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let t0 = Instant::now();
    let maps: Vec<PathBuf> = f("--maps").ok_or("publish-dir needs --maps A,B,…")?.split(',').filter(|s| !s.trim().is_empty()).map(|s| PathBuf::from(s.trim())).collect();
    let items_dirs: Vec<PathBuf> = f("--items-dirs").map(|s| s.split(',').filter(|s| !s.trim().is_empty()).map(|s| PathBuf::from(s.trim())).collect()).unwrap_or_else(|| maps.iter().map(|m| m.parent().unwrap_or(Path::new(".")).join("libx").join("Items")).collect());
    if items_dirs.len() != maps.len() {
        return Err(format!("--items-dirs has {} entries for {} maps", items_dirs.len(), maps.len()));
    }
    let club = f("--club").ok_or("publish-dir needs --club ID")?;
    let camp_name = f("--campaign-name").ok_or("publish-dir needs --campaign-name NAME")?;
    let tag = f("--tag").unwrap_or_else(|| camp_name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase());
    let results_dir = PathBuf::from(f("--results").unwrap_or_else(|| "/tmp/giant/publish".into()));
    std::fs::create_dir_all(&results_dir).map_err(|e| format!("{}: {e}", results_dir.display()))?;
    let box_tinyctl = f("--box-tinyctl").unwrap_or_else(|| "/home/vjeux/shoot/u10s/tinyctl".into());
    let force = tmmaps::cli::has(args, "--force");
    let wsx = Wsx::new(args);
    let remote_dir = format!("{BOX_BATCH_DIR}/{tag}");
    println!("\n===== {camp_name}: {} maps -> club {club} =====", maps.len());
    // previous IDENTICAL results: playlist-only skip rows
    let prev_results = results_dir.join(format!("results-{tag}.tsv"));
    let done_before: std::collections::HashMap<String, String> = if force {
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
    let mut to_push: Vec<(PathBuf, String)> = Vec::new();
    let mut refused: Vec<String> = Vec::new();
    let mut skipped = 0usize;
    for (m, items) in maps.iter().zip(items_dirs.iter()) {
        let file = m.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("map path")?;
        let remote = format!("{remote_dir}/{file}");
        if let Some(prev) = done_before.get(&file) {
            let (uid, prev_md5) = prev.split_once('\t').unwrap_or((prev.as_str(), ""));
            let cur_md5 = std::fs::read(m).map(|b| md5_hex(&b)).unwrap_or_default();
            if let Ok(h) = tmmaps::header::read(m.to_str().unwrap_or("")) {
                if h.uid == uid && cur_md5 == prev_md5 {
                    manifest.push_str(&format!("{remote}\t{}\t{uid}\tskip\t{cur_md5}\n", h.name));
                    skipped += 1;
                    continue;
                }
            }
        }
        // the packs of the map's own collection
        let paks = match crate::build::paks_for(crate::views::collection_of(&tmmaps::map::MapFile::load(m))) {
            Ok(p) => p.join(" "),
            Err(e) => {
                refused.push(format!("{file}: {e}"));
                continue;
            }
        };
        match gate(m, items, &paks) {
            Ok((name, _uid)) => {
                manifest.push_str(&format!("{remote}\t{name}\n"));
                to_push.push((m.clone(), file));
            }
            Err(e) => {
                eprintln!("  {file}: refused: {}", e.lines().next().unwrap_or(""));
                refused.push(format!("{file}: {}", e.lines().next().unwrap_or("")));
            }
        }
    }
    println!("  {}: {} to upload, {skipped} already on Nadeo, {} refused", camp_name, to_push.len(), refused.len());
    if !refused.is_empty() {
        let p = results_dir.join(format!("refused-{tag}.txt"));
        let _ = std::fs::write(&p, refused.join("\n") + "\n");
        return Err(format!("{} maps refused by the gate — see {}", refused.len(), p.display()));
    }
    let local_manifest = results_dir.join(format!("manifest-{tag}.tsv"));
    std::fs::write(&local_manifest, &manifest).map_err(|e| format!("{}: {e}", local_manifest.display()))?;
    wsx.sh(&format!("mkdir -p {remote_dir}"))?;
    // a file already on the box at the same size (a retried run) is not pushed again
    let remote_sizes: std::collections::HashMap<String, u64> = wsx.sh(&format!("cd {remote_dir} 2>/dev/null && stat -c '%n %s' *.Map.Gbx 2>/dev/null || true")).unwrap_or_default().lines().filter_map(|l| { let (n, s) = l.rsplit_once(' ')?; Some((n.to_string(), s.parse().ok()?)) }).collect();
    for (m, file) in &to_push {
        let local = std::fs::metadata(m).map(|x| x.len()).unwrap_or(0);
        if remote_sizes.get(file) == Some(&local) {
            eprintln!("{file}: already on the box ({local} B), not pushed again");
            continue;
        }
        eprintln!("pushing {file} …");
        wsx.push(m, &format!("{remote_dir}/{file}"))?;
    }
    let remote_manifest = format!("{remote_dir}/manifest.tsv");
    wsx.push(&local_manifest, &remote_manifest)?;
    let camps = Campaigns::load(results_dir.join("campaigns.tsv"));
    let (camp_id, act_id) = match f("--campaign").map(|id| (id, "-".to_string())).or_else(|| camps.get(&camp_name)) {
        Some(x) => x,
        None => {
            let out = wsx.sh(&format!("cd /home/vjeux/shoot/u10s && {} nadeo-here campaign-create --club {} --name '{}' --no-lock --outdir {remote_dir}", box_tinyctl, club, camp_name.replace('\'', "")))?;
            let toks: Vec<&str> = out.split_whitespace().collect();
            let id = toks.iter().position(|t| *t == "campaignId").and_then(|i| toks.get(i + 1)).map(|s| s.to_string()).ok_or_else(|| format!("campaign create: {}", out.lines().last().unwrap_or("").chars().take(200).collect::<String>()))?;
            let act = toks.iter().position(|t| *t == "activityId").and_then(|i| toks.get(i + 1)).map(|s| s.to_string()).unwrap_or_else(|| "-".into());
            camps.put(&camp_name, id.clone(), act.clone());
            (id, act)
        }
    };
    println!("  campaign {camp_id} (activity {act_id}) `{camp_name}`");
    let remote_results = format!("{remote_dir}/results.tsv");
    let cmd = format!("{} publish-batch --detach --manifest {remote_manifest} --results {remote_results} --club {} --campaign {camp_id} --campaign-name '{}' --outdir {remote_dir}", box_tinyctl, club, camp_name.replace('\'', ""));
    wsx.sh(&cmd)?;
    let done = wsx.wait_done(&format!("{remote_dir}/results.done"), &format!("{remote_dir}/results.log"), Duration::from_secs(5400), &format!("publish-batch {tag}"));
    let pulled = wsx.pull(&remote_results, &prev_results);
    let _ = wsx.sh(&format!("rm -rf {remote_dir}"));
    let text = done?;
    pulled?;
    let rows = std::fs::read_to_string(&prev_results).unwrap_or_default();
    let identical = rows.lines().filter(|l| l.ends_with("\tIDENTICAL")).count();
    let total = to_push.len() + skipped;
    println!("{camp_name}: campaign {camp_id}, {identical}/{total} identical, {:.0} s — {}", t0.elapsed().as_secs_f64(), text.trim().replace('\n', " | "));
    for l in rows.lines() {
        println!("  {}", l);
    }
    if identical + skipped < total {
        return Err(format!("{} of {total} maps did not read back IDENTICAL — see {}", total - identical - skipped, prev_results.display()));
    }
    Ok(())
}
