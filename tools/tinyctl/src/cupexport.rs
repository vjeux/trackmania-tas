//! `tinyctl cup-export --club C --campaign K --out DIR --prefix Cup5 [--token-dir D] [--zip NAME.zip]`
//! — Everios96 2026-09-26 (scrapie's Altered U10S cup, Danyy): the campaign's LIVE maps,
//! each re-authored for the cup: the world record becomes the author time (medals from it:
//! gold ×1.06, silver ×1.2, bronze ×1.5, rounded up to the next full second like the game —
//! `--medal-step 10` for the 10 ms rounding of the September cups), the WR holder becomes the map's
//! author (account id as the login, display name as the nickname), a fresh uid (`<prefix>` +
//! the live uid's tail). Files land in `OUT/NN-<name>.Map.Gbx`, a manifest in
//! `OUT/manifest.tsv`, and one stored zip of all of them (`--zip`, default `OUT/<campaign>.zip`).
//! Nothing is uploaded anywhere: Danyy uploads them to the cup club himself.

use std::path::{Path, PathBuf};
use std::process::Command;

fn curl_json(auth: &str, url: &str) -> Result<serde_json::Value, String> {
    let out = Command::new("curl").args(["-s", "--max-time", "60", "-H", auth, url]).output().map_err(|e| format!("curl: {e}"))?;
    serde_json::from_slice(&out.stdout).map_err(|e| format!("{url}: {e}: {}", String::from_utf8_lossy(&out.stdout).chars().take(200).collect::<String>()))
}

/// The account id's login form: the 16 UUID bytes, base64url without padding (22 chars).
fn login_of(account_id: &str) -> Result<String, String> {
    let hex: String = account_id.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        return Err(format!("account id `{account_id}` is not a UUID"));
    }
    let bytes: Vec<u8> = (0..16).map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()).collect();
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut s = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        let chars = [T[(n >> 18) as usize & 63], T[(n >> 12) as usize & 63], T[(n >> 6) as usize & 63], T[n as usize & 63]];
        let keep = match chunk.len() { 1 => 2, 2 => 3, _ => 4 };
        for c in &chars[..keep] {
            s.push(*c as char);
        }
    }
    Ok(s)
}

/// A medal time from the author time: the GAME's rule — ×1.06 / ×1.2 / ×1.5 rounded UP to
/// the next full second (30 of 30 of Everios96's editor-saved originals match it; he wants
/// the exports "the same way the game handles it without custom medals", 2026-10-30 01:32).
/// `--medal-step 10` restores the September cups' 10 ms rounding.
fn medal(at: u32, factor: f64, step_ms: u32) -> u32 {
    let step = step_ms.max(1) as f64;
    let v = (at as f64 * factor / step - 1e-9).ceil() * step;
    v as u32
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let club = f("--club").ok_or("cup-export needs --club ID")?;
    // --campaigns A,B,C: one export directory and one zip per campaign (Everios96
    // 2026-09-27: parts 6, 7, 8 in one run), each with its own uid prefix suffix
    if let Some(list) = f("--campaigns") {
        let camps: Vec<String> = list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let base_out = PathBuf::from(f("--out").ok_or("cup-export needs --out DIR")?);
        let mut rest: Vec<String> = args.iter().filter(|a| !a.starts_with("--campaigns") && !a.starts_with("--out") && !a.starts_with("--zip")).cloned().collect();
        // drop the VALUES of the flags removed above
        let mut cleaned: Vec<String> = Vec::new();
        let mut skip = false;
        for (i, a) in args.iter().enumerate() {
            if skip {
                skip = false;
                continue;
            }
            if a == "--campaigns" || a == "--out" || a == "--zip" {
                skip = true;
                continue;
            }
            if a.starts_with("--campaigns=") || a.starts_with("--out=") || a.starts_with("--zip=") {
                continue;
            }
            let _ = i;
            cleaned.push(a.clone());
        }
        rest = cleaned;
        for (i, c) in camps.iter().enumerate() {
            let out_i = base_out.join(format!("campaign-{c}"));
            let mut a2 = rest.clone();
            a2.push("--campaign".into());
            a2.push(c.clone());
            a2.push("--out".into());
            a2.push(out_i.to_string_lossy().into_owned());
            println!("=== campaign {c} ({}/{}) -> {}", i + 1, camps.len(), out_i.display());
            cmd(&a2)?;
        }
        return Ok(());
    }
    let camp = f("--campaign").ok_or("cup-export needs --campaign ID or --campaigns A,B,C")?;
    let out = PathBuf::from(f("--out").ok_or("cup-export needs --out DIR")?);
    let medal_step: u32 = f("--medal-step").map(|v| v.parse::<u32>().map_err(|_| "--medal-step wants milliseconds (1000 = the game's full seconds, 10 = the cups' 10 ms)")).transpose()?.unwrap_or(1000);
    let prefix = f("--prefix").unwrap_or_else(|| "Cup5".into());
    if prefix.len() != 4 {
        return Err("--prefix is 4 characters".into());
    }
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let token_dir = f("--token-dir").map(PathBuf::from).unwrap_or_else(|| out.join("tokens"));
    std::fs::create_dir_all(&token_dir).map_err(|e| e.to_string())?;
    if crate::nadeo::fresh_tokens_in(token_dir.to_str().unwrap()).is_none() {
        let wsx = crate::wsx::Wsx::new(args);
        crate::roundpub::mint_tokens(&wsx, &token_dir)?;
    }
    let (core, live) = crate::nadeo::fresh_tokens_in(token_dir.to_str().unwrap()).ok_or("no tokens")?;
    let (auth_core, auth_live) = (format!("Authorization: {core}"), format!("Authorization: {live}"));
    // 1. the playlist + core records + files
    let src_dir = out.join("live");
    std::fs::create_dir_all(&src_dir).map_err(|e| e.to_string())?;
    std::env::set_var("TINYCTL_TOKEN_DIR", token_dir.to_str().unwrap());
    let fetch_args: Vec<String> = vec!["campaign".into(), "--club".into(), club.clone(), "--campaign".into(), camp.clone(), "--fetch".into(), src_dir.to_string_lossy().into_owned(), "--no-lock".into()];
    crate::nadeo::cmd(&fetch_args)?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(&src_dir).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(".Map.Gbx")).collect();
    files.sort();
    if files.is_empty() {
        return Err("no live files fetched".into());
    }
    println!("{} live maps fetched", files.len());
    // 2. the world record of each
    struct Wr { account: String, score: u32 }
    let mut wrs: Vec<(PathBuf, tmmaps::header::MapHeader, Option<Wr>)> = Vec::new();
    for p in &files {
        let hdr = tmmaps::header::read(p.to_str().unwrap()).map_err(|e| format!("{}: {e}", p.display()))?;
        let url = format!("{}/api/token/leaderboard/group/Personal_Best/map/{}/top?length=1&onlyWorld=true", crate::publish::LIVE, hdr.uid);
        let v = curl_json(&auth_live, &url)?;
        let top = v.get("tops").and_then(|t| t.as_array()).and_then(|t| t.first()).and_then(|t| t.get("top")).and_then(|t| t.as_array()).and_then(|t| t.first()).cloned();
        let wr = top.and_then(|t| Some(Wr { account: t.get("accountId")?.as_str()?.to_string(), score: t.get("score")?.as_u64()? as u32 }));
        match &wr {
            Some(w) => println!("  {}: WR {} ms by {}", hdr.name, w.score, w.account),
            None => println!("  {}: NO record — kept as is (author time {})", hdr.name, hdr.authortime),
        }
        wrs.push((p.clone(), hdr, wr));
    }
    // 3. display names — trackmania.io (Nadeo's core displayNames endpoint rejects game
    // tokens); `--names F.tsv` (accountId<TAB>name) overrides / pre-seeds
    let accounts: Vec<String> = { let mut a: Vec<String> = wrs.iter().filter_map(|(_, _, w)| w.as_ref().map(|w| w.account.clone())).collect(); a.sort(); a.dedup(); a };
    let mut names: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Some(nf) = f("--names") {
        for l in std::fs::read_to_string(&nf).map_err(|e| format!("{nf}: {e}"))?.lines() {
            if let Some((id, n)) = l.split_once('\t') {
                if !n.trim().is_empty() {
                    names.insert(id.trim().to_string(), n.trim().to_string());
                }
            }
        }
    }
    let _ = &auth_core;
    for id in &accounts {
        if names.contains_key(id) {
            continue;
        }
        let out = Command::new("curl").args(["-s", "--max-time", "30", "-A", "tinyctl cup-export (vjeux, u10s tools)", &format!("https://trackmania.io/api/player/{id}")]).output().map_err(|e| format!("curl: {e}"))?;
        match serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            Ok(v) => {
                if let Some(n) = v.get("displayname").and_then(|x| x.as_str()) {
                    names.insert(id.clone(), n.to_string());
                    println!("  {id} = {n}");
                } else {
                    eprintln!("  {id}: no displayname in {}", String::from_utf8_lossy(&out.stdout).chars().take(120).collect::<String>());
                }
            }
            Err(e) => eprintln!("  {id}: trackmania.io: {e}"),
        }
        std::thread::sleep(std::time::Duration::from_millis(1600)); // 40/min
    }
    let missing: Vec<&String> = accounts.iter().filter(|a| !names.contains_key(*a)).collect();
    if !missing.is_empty() {
        return Err(format!("no display name for {} account(s): {} — pass --names", missing.len(), missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
    }
    // 4. the exports
    let mut manifest = String::from("file\tname\told_uid\tnew_uid\twr_ms\thunter\taccount\tlogin\n");
    let mut made: Vec<PathBuf> = Vec::new();
    // file names: the map's own name (`U10S_26 By Everios96 [Wet Wood].Map.Gbx`, the way
    // the club's files are named) — the fetch's `01-U10S_26-By-Everios96-Wet-Wood` form
    // "seems off" to the cup uploader (Danyy via Everios96, 2026-10-30 02:17);
    // `--file-names fetch` keeps the fetched names
    let fetch_names = f("--file-names").map(|v| v == "fetch").unwrap_or(false);
    for (p, hdr, wr) in &wrs {
        let fname = if fetch_names {
            p.file_name().unwrap().to_string_lossy().into_owned()
        } else {
            let clean: String = hdr.name.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect();
            format!("{}.Map.Gbx", clean.trim())
        };
        let dst = out.join(&fname);
        let new_uid = format!("{prefix}{}", &hdr.uid[4..]);
        // pass 1: the uid
        let mut m = tmmaps::map::MapFile::load(p);
        m.set_map_uid(&new_uid);
        m.write_to(&dst).map_err(|e| format!("{}: {e}", dst.display()))?;
        let (mut hunter, mut account, mut login, mut wr_ms) = (String::from("-"), String::from("-"), String::from("-"), String::from("-"));
        if let Some(w) = wr {
            let nick = names.get(&w.account).cloned().unwrap_or_else(|| w.account.clone());
            let new_login = login_of(&w.account)?;
            let old_at: u32 = hdr.authortime.parse().map_err(|_| format!("{fname}: author time `{}`", hdr.authortime))?;
            let times = [w.score, medal(w.score, 1.06, medal_step), medal(w.score, 1.2, medal_step), medal(w.score, 1.5, medal_step)];
            // pass 2: author + times on a fresh load (splices of another length)
            let mut m2 = tmmaps::map::MapFile::load(&dst);
            let old_nick = m2.author_nickname().unwrap_or_else(|| "Everios96".to_string());
            let (hh, bh) = m2.set_author_and_times(&hdr.author, &new_login, &old_nick, &nick, old_at, times)?;
            // the times the GAME reads live in the body's challenge parameters; the header
            // alone leaves every map playing with the original author time (2026-09-28)
            m2.set_body_times(old_at, times)?;
            m2.write_to(&dst).map_err(|e| format!("{}: {e}", dst.display()))?;
            // read back: the body must now carry exactly what we advertise in the header
            let back = tmmaps::map::MapFile::load(&dst);
            match back.body_times() {
                Some(t) if t == times => {}
                Some(t) => return Err(format!("{fname}: body times {t:?} != {times:?} after the write")),
                None => return Err(format!("{fname}: no body challenge parameters to read back")),
            }
            println!("  {fname}: uid {} -> {new_uid}; AT {old_at} -> {} ({}); author {} -> {nick} ({new_login}); header hits {hh}, body hits {bh}", hdr.uid, w.score, tmmaps::secs::secs_str(&w.score.to_string()), old_nick);
            hunter = nick;
            account = w.account.clone();
            login = new_login;
            wr_ms = w.score.to_string();
        }
        // read back
        let h2 = tmmaps::header::read(dst.to_str().unwrap()).map_err(|e| format!("{}: {e}", dst.display()))?;
        if h2.uid != new_uid {
            return Err(format!("{fname}: uid readback {} != {new_uid}", h2.uid));
        }
        manifest.push_str(&format!("{fname}\t{}\t{}\t{new_uid}\t{wr_ms}\t{hunter}\t{account}\t{login}\n", hdr.name, hdr.uid));
        made.push(dst);
    }
    std::fs::write(out.join("manifest.tsv"), &manifest).map_err(|e| e.to_string())?;
    // 5. one stored zip
    let zip_path = f("--zip").map(PathBuf::from).unwrap_or_else(|| out.join(format!("campaign-{camp}.zip")));
    let mut entries: std::collections::BTreeMap<String, Vec<u8>> = std::collections::BTreeMap::new();
    for p in &made {
        entries.insert(p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read(p).map_err(|e| e.to_string())?);
    }
    entries.insert("manifest.tsv".into(), manifest.into_bytes());
    std::fs::write(&zip_path, tmmaps::header::stored_zip(&entries)).map_err(|e| e.to_string())?;
    println!("{} maps exported -> {} ; zip {} ({} bytes)", made.len(), out.display(), zip_path.display(), std::fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0));
    Ok(())
}

#[allow(dead_code)]
fn _unused(_: &Path) {}
