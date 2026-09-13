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
        let (core, live) = crate::nadeo::batch_tokens(shootctl)?;
        let (mine, c) = curl(&["-H", &format!("Authorization: {live}"), &format!("{LIVE}/api/token/club/mine?length=1&offset=0")])?;
        let me = json_str(&mine, "authorAccountId").ok_or_else(|| format!("club/mine: HTTP {c} without authorAccountId: {}", &mine[..mine.len().min(200)]))?;
        Ok(Tokens { core, live, me, shootctl: shootctl.to_string() })
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
    let rows: Vec<(PathBuf, String)> = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).filter_map(|l| l.split_once('\t').map(|(p, n)| (PathBuf::from(p.trim()), n.split('\t').next().unwrap_or("").trim().to_string()))).collect();
    let summary = (|| -> Result<String, String> {
        let mut t = Tokens::mint(&shootctl)?;
        let mut out = String::from("path\tname\tuid\tmapId\thow\tbytes\tmd5\tverdict\n");
        let mut uids: Vec<String> = Vec::new();
        let mut failed = 0usize;
        for (i, (p, name)) in rows.iter().enumerate() {
            let t1 = Instant::now();
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
                    let n = v.get("playlist").and_then(|p| p.as_array()).map(|a| a.len()).unwrap_or(0);
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

pub fn publish_set_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let parts: Vec<String> = f("--parts").ok_or("publish-set needs --parts 01,02,… (the part directories under --out-root)")?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let out_root = PathBuf::from(f("--out-root").ok_or("publish-set needs --out-root R (R/pNN/tinyNN/<tag>/<prefix>-NN-Tiny.Map.Gbx)")?);
    let tag = f("--tag").unwrap_or_else(|| "u10s".into());
    let prefix = f("--out-prefix").unwrap_or_else(|| "U10S".into());
    let club = f("--club").ok_or("publish-set needs --club ID")?;
    let camp_prefix = f("--campaign-prefix").unwrap_or_else(|| "Tiny U10S PART ".into());
    let results_dir = PathBuf::from(f("--results").unwrap_or_else(|| out_root.join("publish").display().to_string()));
    std::fs::create_dir_all(&results_dir).map_err(|e| format!("{}: {e}", results_dir.display()))?;
    let campaigns_tsv = results_dir.join("campaigns.tsv");
    let mut known: std::collections::BTreeMap<String, (String, String)> = std::fs::read_to_string(&campaigns_tsv)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c.len() >= 3).then(|| (c[0].to_string(), (c[1].to_string(), c[2].to_string())))
        })
        .collect();
    let wsx = Wsx::new(args);
    let box_tinyctl = f("--box-tinyctl").unwrap_or_else(|| "/home/vjeux/shoot/u10s/tinyctl".into());
    let paks = f("--paks").unwrap_or_else(|| "--pak /tmp/current-Stadium.pak:B773D73047A4104857722366D78D28A6".into());
    let mut verdicts: Vec<String> = Vec::new();
    for part in &parts {
        let t0 = Instant::now();
        let part_dir = out_root.join(format!("p{part}"));
        let camp_name = format!("{camp_prefix}{}", part.trim_start_matches('0'));
        println!("\n===== part {part}: {} ({}) =====", camp_name, part_dir.display());
        // the built maps of the part, in map order
        let mut maps: Vec<(String, PathBuf, PathBuf)> = Vec::new();
        for nn in 1..=99usize {
            let d = part_dir.join(format!("tiny{nn:02}")).join(&tag);
            let m = d.join(format!("{prefix}-{nn:02}-Tiny.Map.Gbx"));
            if m.exists() {
                maps.push((format!("{nn:02}"), m, d.join("libx").join("Items")));
            }
        }
        if maps.is_empty() {
            verdicts.push(format!("{part}\tFAILED\tno built maps under {}", part_dir.display()));
            continue;
        }
        // gate + manifest
        let mut manifest = String::new();
        let mut gated: Vec<(String, PathBuf, String)> = Vec::new();
        let mut refused: Vec<String> = Vec::new();
        for (nn, m, items) in &maps {
            match gate(m, items, &paks) {
                Ok((name, uid)) => {
                    let remote = format!("{BOX_BATCH_DIR}/p{part}/{prefix}-{nn}-Tiny.Map.Gbx");
                    manifest.push_str(&format!("{remote}\t{name}\n"));
                    gated.push((nn.clone(), m.clone(), uid));
                }
                Err(e) => {
                    eprintln!("  {nn}: refused: {}", e.lines().next().unwrap_or(""));
                    refused.push(format!("{nn}: {}", e.lines().next().unwrap_or("")));
                }
            }
        }
        println!("  {} maps gated ok, {} refused", gated.len(), refused.len());
        if gated.is_empty() {
            verdicts.push(format!("{part}\tFAILED\tevery map refused by the gate"));
            continue;
        }
        let local_manifest = results_dir.join(format!("manifest-p{part}.tsv"));
        std::fs::write(&local_manifest, &manifest).map_err(|e| format!("{}: {e}", local_manifest.display()))?;
        // push
        wsx.sh(&format!("mkdir -p {BOX_BATCH_DIR}/p{part}"))?;
        for (nn, m, _) in &gated {
            wsx.push(m, &format!("{BOX_BATCH_DIR}/p{part}/{prefix}-{nn}-Tiny.Map.Gbx"))?;
        }
        let remote_manifest = format!("{BOX_BATCH_DIR}/p{part}/manifest.tsv");
        wsx.push(&local_manifest, &remote_manifest)?;
        // the campaign: known id, or create one (the box has the tokens)
        let (camp_id, act_id) = match known.get(part) {
            Some(x) => x.clone(),
            None => {
                let out = wsx.sh(&format!("cd /home/vjeux/shoot/u10s && {box_tinyctl} nadeo-here campaign-create --club {club} --name '{}' --no-lock --outdir {BOX_BATCH_DIR}", camp_name.replace('\'', "")))?;
                let toks: Vec<&str> = out.split_whitespace().collect();
                let id = toks.iter().position(|t| *t == "campaignId").and_then(|i| toks.get(i + 1)).map(|s| s.to_string());
                let act = toks.iter().position(|t| *t == "activityId").and_then(|i| toks.get(i + 1)).map(|s| s.to_string()).unwrap_or_else(|| "-".into());
                match id {
                    Some(id) => {
                        known.insert(part.clone(), (id.clone(), act.clone()));
                        let mut rows = String::new();
                        for (p, (i, a)) in &known {
                            rows.push_str(&format!("{p}\t{i}\t{a}\n"));
                        }
                        std::fs::write(&campaigns_tsv, rows).map_err(|e| format!("{}: {e}", campaigns_tsv.display()))?;
                        (id, act)
                    }
                    None => {
                        verdicts.push(format!("{part}\tFAILED\tcampaign create: {}", out.lines().last().unwrap_or("").chars().take(200).collect::<String>()));
                        continue;
                    }
                }
            }
        };
        println!("  campaign {camp_id} (activity {act_id}) `{camp_name}`");
        // the batch on the box
        let remote_results = format!("{BOX_BATCH_DIR}/p{part}/results.tsv");
        let cmd = format!("{box_tinyctl} publish-batch --detach --manifest {remote_manifest} --results {remote_results} --club {club} --campaign {camp_id} --campaign-name '{}' --outdir {BOX_BATCH_DIR}/p{part}", camp_name.replace('\'', ""));
        let started = wsx.sh(&cmd)?;
        if wsx.verbose {
            eprintln!("{}", started.trim());
        }
        let done = wsx.wait_done(&format!("{BOX_BATCH_DIR}/p{part}/results.done"), &format!("{BOX_BATCH_DIR}/p{part}/results.log"), Duration::from_secs(3600), &format!("publish-batch p{part}"));
        let local_results = results_dir.join(format!("results-p{part}.tsv"));
        let pulled = wsx.pull(&remote_results, &local_results);
        // the box copies go (C: at 99 %)
        let _ = wsx.sh(&format!("rm -rf {BOX_BATCH_DIR}/p{part}"));
        match (done, pulled) {
            (Ok(text), Ok(_)) => {
                let identical = std::fs::read_to_string(&local_results).map(|t| t.lines().filter(|l| l.ends_with("\tIDENTICAL")).count()).unwrap_or(0);
                verdicts.push(format!("{part}\tOK\t{:.0}s\tcampaign {camp_id}\t{identical}/{} identical\t{} refused\t{}", t0.elapsed().as_secs_f64(), gated.len(), refused.len(), text.trim().replace('\n', " | ")));
            }
            (Err(e), _) | (_, Err(e)) => verdicts.push(format!("{part}\tFAILED\t{:.0}s\tcampaign {camp_id}\t{}", t0.elapsed().as_secs_f64(), e.lines().next().unwrap_or(""))),
        }
        if !refused.is_empty() {
            let p = results_dir.join(format!("refused-p{part}.txt"));
            let _ = std::fs::write(&p, refused.join("\n") + "\n");
        }
        println!("{}", verdicts.last().unwrap());
    }
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
