//! `tinyctl nadeo-here …` — Nadeo Live/Core service reads and the one campaign
//! write the tiny pipeline needs besides `publish-here`, run ON THE RENDER BOX
//! (the only host with the game's tokens and a route to Nadeo; the devservers'
//! forward proxy refuses `*.nadeo.live` / `*.nadeo.online`).
//!
//! ```text
//! tinyctl nadeo-here club --name everios96                   clubs whose name matches
//! tinyctl nadeo-here campaigns --club ID [--length 100]      the club's campaigns (id, name, maps)
//! tinyctl nadeo-here campaign --club ID --campaign ID        the playlist with each map's core record
//!                    [--fetch DIR [--first N]]                … and the first N map files as NN-<name>.Map.Gbx
//! tinyctl nadeo-here campaign-create --club ID --name NAME   a new (empty) campaign; prints its id
//! tinyctl nadeo-here maps --uids UID,UID…                    core records for a list of uids
//! ```
//!
//! Every command mints fresh tokens through the GhostShooter `/nadeotoken`
//! route under the render lock (a two-second slice, like `publish-here`), and
//! writes what it read as JSON next to the printed table (`--outdir`, default
//! `/home/vjeux/shoot/u10s`) so the devserver side can pull it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::Value;

use crate::publish::{render_lock, token, CORE, LIVE, STORE};

const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";

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

fn get_json(auth: &str, url: &str) -> Result<Value, String> {
    let (body, code) = curl(&["-H", auth, url])?;
    if code != "200" {
        return Err(format!("GET {url}: HTTP {code} {}", &body[..body.len().min(300)]));
    }
    serde_json::from_str(&body).map_err(|e| format!("GET {url}: not JSON ({e}): {}", &body[..body.len().min(200)]))
}

fn post_json(auth: &str, url: &str, body: &str) -> Result<Value, String> {
    let (resp, code) = curl(&["-X", "POST", "-H", auth, "-H", "Content-Type: application/json", "-d", body, url])?;
    if !code.starts_with('2') {
        return Err(format!("POST {url}: HTTP {code} {}", &resp[..resp.len().min(400)]));
    }
    serde_json::from_str(&resp).map_err(|e| format!("POST {url}: not JSON ({e}): {}", &resp[..resp.len().min(200)]))
}

/// Fresh core + live tokens (Authorization header values) from the running
/// game, under the render lock. The plugin's previous token files are removed
/// first so a stale one is never read back as fresh — unless both are younger
/// than `TOKEN_REUSE_SECS` (a Nadeo access token lives about an hour): then
/// they are reused and the lock is not taken at all, so a run of reads costs
/// the shared render lock one slice, not one per command.
const TOKEN_REUSE_SECS: u64 = 40 * 60;

/// The plugin's (core, live) token files when both are younger than
/// `TOKEN_REUSE_SECS` and look like tokens; `None` means mint under the lock.
pub fn fresh_tokens() -> Option<(String, String)> {
    let fresh = |aud: &str| -> Option<String> {
        let p = format!("{STORE}/token-{aud}.txt");
        let age = std::fs::metadata(&p).ok()?.modified().ok()?.elapsed().ok()?;
        let t = std::fs::read_to_string(&p).ok()?.trim().to_string();
        (age.as_secs() < TOKEN_REUSE_SECS && t.len() > 20).then_some(t)
    };
    Some((fresh("NadeoServices")?, fresh("NadeoLiveServices")?))
}

pub fn tokens(shootctl: &str, owner: &str) -> Result<(String, String), String> {
    tokens_opt(shootctl, owner, true)
}

/// `lock = false`: mint WITHOUT the render lock (vjeux holding the lock while he
/// plays and asking for an API fix, 2026-09-12 22:22Z — the mint is a 2-second
/// plugin call that loads nothing).
pub fn tokens_opt(shootctl: &str, owner: &str, lock: bool) -> Result<(String, String), String> {
    if let Some(pair) = fresh_tokens() {
        eprintln!("tokens: reusing the plugin's files (younger than {} min)", TOKEN_REUSE_SECS / 60);
        return Ok(pair);
    }
    if !lock {
        for aud in ["NadeoServices", "NadeoLiveServices"] {
            let _ = std::fs::remove_file(format!("{STORE}/token-{aud}.txt"));
        }
        return token(shootctl, "NadeoServices").and_then(|core| {
            std::thread::sleep(Duration::from_millis(500));
            token(shootctl, "NadeoLiveServices").map(|live| (core, live))
        });
    }
    render_lock(shootctl, owner, "acquire", &["--wait", "1500"]).map_err(|e| format!("render lock: {e}"))?;
    for aud in ["NadeoServices", "NadeoLiveServices"] {
        let _ = std::fs::remove_file(format!("{STORE}/token-{aud}.txt"));
    }
    let r = token(shootctl, "NadeoServices").and_then(|core| {
        std::thread::sleep(Duration::from_millis(500));
        token(shootctl, "NadeoLiveServices").map(|live| (core, live))
    });
    let _ = render_lock(shootctl, owner, "release", &[]);
    r
}

fn s(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(x)) => x.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Null) | None => "-".into(),
        Some(other) => other.to_string(),
    }
}

/// Nadeo's `$`-formatting stripped and the rest made file-safe (the
/// `nadeo-fetch-campaign.sh` rule): `$o$fffU10S_$f0001` -> `U10S_01`.
pub fn file_safe_name(name: &str) -> String {
    let mut out = String::new();
    let cs: Vec<char> = name.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '$' {
            // $hhh colour, $x style letter, $$ literal, $[ / $] links
            if i + 3 < cs.len() && cs[i + 1..i + 4].iter().all(|c| c.is_ascii_hexdigit()) {
                i += 4;
                continue;
            }
            if i + 1 < cs.len() && cs[i + 1] == '$' {
                // a literal `$`: not a file-name character here
                i += 2;
                continue;
            }
            i += 2;
            continue;
        }
        let c = cs[i];
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
        } else if c == ' ' {
            out.push('-');
        }
        i += 1;
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() { "map".into() } else { trimmed }
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let sub = args.first().ok_or("nadeo-here needs a subcommand: club | campaigns | campaign | campaign-create | maps | club-mirror")?.clone();
    if sub == "club-mirror" {
        return club_mirror(args);
    }
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| "/home/vjeux/shoot/u10s".into()));
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let shootctl = f("--shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    let owner = format!("nadeo-{}", std::process::id());
    let (core, live) = tokens_opt(&shootctl, &owner, !tmmaps::cli::has(args, "--no-lock"))?;
    let auth_core = format!("Authorization: {core}");
    let auth_live = format!("Authorization: {live}");
    let save = |name: &str, v: &Value| -> Result<(), String> {
        let p = outdir.join(name);
        std::fs::write(&p, serde_json::to_string_pretty(v).unwrap_or_default()).map_err(|e| format!("{}: {e}", p.display()))?;
        eprintln!("wrote {}", p.display());
        Ok(())
    };
    match sub.as_str() {
        "club-mine" => {
            let v = get_json(&auth_live, &format!("{LIVE}/api/token/club/mine?length=50&offset=0"))?;
            save("club-mine.json", &v)?;
            println!("clubId\tname\tstate\ttag\tmembers\tauthorAccountId");
            for c in v.get("clubList").and_then(|l| l.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
                println!("{}\t{}\t{}\t{}\t{}\t{}", s(c, "id"), s(c, "name"), s(c, "state"), s(c, "tag"), s(c, "membersCount"), s(c, "authorAccountId"));
            }
            Ok(())
        }
        "club-create" => {
            // tinyctl nadeo-here club-create --name NAME [--description TEXT] [--state private|public]
            // (the states are `public`, `private-open` (join requests) and `private-closed`;
            // a private club is reachable by its members only — vjeux's "Tiny U10S", 2026-09-13)
            let name = f("--name").ok_or("club-create needs --name NAME")?;
            let desc = f("--description").unwrap_or_default();
            let state = f("--state").unwrap_or_else(|| "private-closed".into());
            let body = format!("{{\"name\":\"{}\",\"description\":\"{}\",\"state\":\"{state}\"}}", name.replace('"', "\\\""), desc.replace('"', "\\\""));
            let v = post_json(&auth_live, &format!("{LIVE}/api/token/club/create"), &body)?;
            save(&format!("club-create-{}.json", file_safe_name(&name)), &v)?;
            println!("clubId\t{}\tname\t{}\tstate\t{}", s(&v, "id"), s(&v, "name"), s(&v, "state"));
            Ok(())
        }
        "campaign-set" => {
            // tinyctl nadeo-here campaign-set --club C --campaign K --name NAME --uids A,B,… : the
            // WHOLE playlist in that order (one write per campaign instead of one per map)
            let club = f("--club").ok_or("campaign-set needs --club ID")?;
            let camp = f("--campaign").ok_or("campaign-set needs --campaign ID")?;
            let name = f("--name").ok_or("campaign-set needs --name NAME")?;
            let uids: Vec<String> = f("--uids").ok_or("campaign-set needs --uids A,B,…")?.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
            let v = campaign_set(&auth_live, &club, &camp, &name, &uids)?;
            save(&format!("club-{club}-campaign-{camp}-set.json"), &v)?;
            let n = serde_json::to_string(&v).unwrap_or_default().matches("\"mapUid\"").count();
            println!("campaign\t{camp}\t{}\t{n} maps in the playlist", s(&v, "name"));
            Ok(())
        }
        "club" => {
            let name = f("--name").ok_or("club needs --name TEXT")?;
            let length = f("--length").unwrap_or_else(|| "20".into());
            let url = format!("{LIVE}/api/token/club?length={length}&offset=0&name={}", urlenc(&name));
            let v = get_json(&auth_live, &url)?;
            save("clubs.json", &v)?;
            println!("clubId\tname\tmembers\tpopularity\tstate\tdescription");
            for c in v.get("clubList").and_then(|l| l.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
                println!("{}\t{}\t{}\t{}\t{}\t{}", s(c, "id"), s(c, "name"), s(c, "membersCount"), s(c, "popularityLevel"), s(c, "state"), s(c, "description").replace('\n', " ").chars().take(80).collect::<String>());
            }
            Ok(())
        }
        "activities" => {
            // the club page's activity list (campaigns, rooms, …), readable for
            // any public club — what the game shows when you open a club
            let club = f("--club").ok_or("activities needs --club ID")?;
            let length = f("--length").unwrap_or_else(|| "100".into());
            let offset = f("--offset").unwrap_or_else(|| "0".into());
            // --inactive lists the inactive (hidden) ones instead
            let active = if tmmaps::cli::has(args, "--inactive") { "false" } else { "true" };
            let url = format!("{LIVE}/api/token/club/{club}/activity?length={length}&offset={offset}&active={active}");
            let v = get_json(&auth_live, &url)?;
            save(&format!("club-{club}-activities-{offset}.json"), &v)?;
            println!("total {}\tmaxPage {}", s(&v, "itemCount"), s(&v, "maxPage"));
            println!("activityId\ttype\tname\tcampaignId\tpublic\tactive\tposition");
            for a in v.get("activityList").and_then(|l| l.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
                println!("{}\t{}\t{}\t{}\t{}\t{}\t{}", s(a, "id"), s(a, "activityType"), s(a, "name"), s(a, "campaignId"), s(a, "public"), s(a, "active"), s(a, "position"));
            }
            Ok(())
        }
        "campaigns" => {
            let length = f("--length").unwrap_or_else(|| "100".into());
            let offset = f("--offset").unwrap_or_else(|| "0".into());
            // --club ID: the club's own list (works for clubs one administers;
            // another club's answers 403 globalAdmin:error-notAllowed —
            // Everios96 18974, 2026-09-12); --name TEXT: the game's club-campaign
            // browser, every club, filtered by campaign name
            let (url, file) = match (f("--club"), f("--name")) {
                (Some(club), _) => (format!("{LIVE}/api/token/club/{club}/campaign?length={length}&offset={offset}"), format!("club-{club}-campaigns.json")),
                (None, Some(name)) => (format!("{LIVE}/api/token/club/campaign?length={length}&offset={offset}&name={}", urlenc(&name)), format!("campaigns-{}.json", file_safe_name(&name))),
                (None, None) => return Err("campaigns needs --club ID or --name TEXT".into()),
            };
            let v = get_json(&auth_live, &url)?;
            save(&file, &v)?;
            println!("total {}\tclub {}", s(&v, "itemCount"), s(&v, "clubName"));
            println!("clubId\tclubName\tcampaignId\tname\tmaps\tpublicationTimestamp\tactivityId");
            for c in v.get("clubCampaignList").and_then(|l| l.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
                let camp = c.get("campaign").cloned().unwrap_or(Value::Null);
                println!("{}\t{}\t{}\t{}\t{}\t{}\t{}", s(c, "clubId"), s(c, "clubName"), s(c, "campaignId"), s(c, "name"), s(&camp, "mapsCount"), s(c, "publicationTimestamp"), s(c, "activityId"));
            }
            Ok(())
        }
        "campaign" => {
            let club = f("--club").ok_or("campaign needs --club ID")?;
            // --campaigns K1,K2,… --fetch-root DIR: several campaigns, each into DIR/pNN
            // (NN = 1-based index in the list) — the whole Everios96 club, 2026-09-13
            if let Some(list) = f("--campaigns") {
                let root = PathBuf::from(f("--fetch-root").ok_or("--campaigns needs --fetch-root DIR")?);
                let ids: Vec<String> = list.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                let first = f("--first");
                let start: usize = f("--start-index").and_then(|x| x.parse().ok()).unwrap_or(1);
                for (i, camp) in ids.iter().enumerate() {
                    let nn = start + i;
                    let dir = root.join(format!("p{nn:02}"));
                    let mut sub: Vec<String> = vec!["campaign".into(), "--club".into(), club.clone(), "--campaign".into(), camp.clone(), "--fetch".into(), dir.display().to_string(), "--no-lock".into(), "--outdir".into(), outdir.display().to_string()];
                    if let Some(n) = &first {
                        sub.push("--first".into());
                        sub.push(n.clone());
                    }
                    println!("===== part {nn:02}: campaign {camp} -> {}", dir.display());
                    if let Err(e) = cmd(&sub) {
                        eprintln!("part {nn:02}: {e}");
                    }
                }
                return Ok(());
            }
            let camp = f("--campaign").ok_or("campaign needs --campaign ID")?;
            let url = format!("{LIVE}/api/token/club/{club}/campaign/{camp}");
            let v = get_json(&auth_live, &url)?;
            save(&format!("club-{club}-campaign-{camp}.json"), &v)?;
            let inner = v.get("campaign").cloned().unwrap_or(Value::Null);
            let mut uids: Vec<(u64, String)> = inner
                .get("playlist")
                .and_then(|l| l.as_array())
                .map(|a| a.iter().map(|e| (e.get("position").and_then(|p| p.as_u64()).unwrap_or(0), s(e, "mapUid"))).collect())
                .unwrap_or_default();
            uids.sort();
            println!("campaign {}\t{}\t{} maps", s(&v, "campaignId"), s(&v, "name"), uids.len());
            let first: usize = f("--first").map(|n| n.parse::<usize>().map_err(|_| "--first N")).transpose()?.unwrap_or(uids.len());
            let wanted: Vec<&(u64, String)> = uids.iter().take(first).collect();
            let recs = core_records(&auth_core, &wanted.iter().map(|(_, u)| u.clone()).collect::<Vec<_>>())?;
            save(&format!("club-{club}-campaign-{camp}-maps.json"), &Value::Array(recs.clone()))?;
            println!("pos\tmapUid\tname\tauthorTime\tgold\tsilver\tbronze\tcollection\tauthor\tmapId\tfile");
            let fetch = f("--fetch").map(PathBuf::from);
            if let Some(d) = &fetch {
                std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
            for (i, (pos, uid)) in wanted.iter().enumerate() {
                let rec = recs.iter().find(|r| s(r, "mapUid") == *uid).cloned().unwrap_or(Value::Null);
                let nn = i + 1;
                let fname = format!("{nn:02}-{}.Map.Gbx", file_safe_name(&s(&rec, "name")));
                let mut file_note = String::from("-");
                if let Some(d) = &fetch {
                    let p = d.join(&fname);
                    if p.exists() && std::fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false) {
                        file_note = format!("have {} B", std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0));
                    } else {
                        let url = s(&rec, "fileUrl");
                        if url == "-" {
                            file_note = "NO fileUrl".into();
                        } else {
                            let (_, code) = curl(&["-L", "-H", &auth_core, "-o", p.to_str().unwrap(), &url])?;
                            let n = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                            file_note = format!("HTTP {code} {n} B");
                        }
                    }
                }
                println!("{pos}\t{uid}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{file_note}", s(&rec, "name"), s(&rec, "authorScore"), s(&rec, "goldScore"), s(&rec, "silverScore"), s(&rec, "bronzeScore"), s(&rec, "collectionName"), s(&rec, "author"), s(&rec, "mapId"), fname);
            }
            Ok(())
        }
        "maps" => {
            let uids: Vec<String> = f("--uids").ok_or("maps needs --uids A,B,…")?.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
            let recs = core_records(&auth_core, &uids)?;
            save("maps.json", &Value::Array(recs.clone()))?;
            println!("mapUid\tname\tauthorTime\tgold\tsilver\tbronze\tcollection\tauthor\tmapId\tfileUrl");
            for r in &recs {
                println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", s(r, "mapUid"), s(r, "name"), s(r, "authorScore"), s(r, "goldScore"), s(r, "silverScore"), s(r, "bronzeScore"), s(r, "collectionName"), s(r, "author"), s(r, "mapId"), s(r, "fileUrl"));
            }
            Ok(())
        }
        "campaign-create" => {
            let club = f("--club").ok_or("campaign-create needs --club ID")?;
            let name = f("--name").ok_or("campaign-create needs --name NAME")?;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let body = format!("{{\"name\":\"{}\",\"description\":\"\",\"color\":\"\",\"useCase\":2,\"publicationTimestamp\":{now},\"mediaUrl\":\"\",\"video\":false}}", name.replace('"', "\\\""));
            let v = post_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/campaign/create"), &body)?;
            save(&format!("club-{club}-campaign-create-{now}.json"), &v)?;
            println!("campaignId\t{}\tname\t{}\tactivityId\t{}", s(&v, "campaignId"), s(&v, "name"), s(&v, "activityId"));
            // A created campaign is an INACTIVE (hidden) club activity: the club
            // page did not list "Tiny u10s everios96" until vjeux found it in the
            // club's management view marked inactive (2026-09-12 22:22Z). Activate
            // and publish it right away (`--hidden` leaves it as created).
            if !tmmaps::cli::has(args, "--hidden") {
                let act = s(&v, "activityId");
                if act != "-" {
                    match activity_edit(&auth_live, &club, &act, true, true) {
                        Ok(a) => println!("activity\t{act}\tactive {}\tpublic {}", s(&a, "active"), s(&a, "public")),
                        Err(e) => eprintln!("activity {act}: could not activate/publish ({e}) — do it in the club's management view"),
                    }
                }
            }
            Ok(())
        }
        "activity-edit" => {
            // tinyctl nadeo-here activity-edit --club ID --activity ID [--active 0|1] [--public 0|1]
            let club = f("--club").ok_or("activity-edit needs --club ID")?;
            let act = f("--activity").ok_or("activity-edit needs --activity ID")?;
            let active = f("--active").map(|x| x != "0").unwrap_or(true);
            let public = f("--public").map(|x| x != "0").unwrap_or(true);
            let a = activity_edit(&auth_live, &club, &act, active, public)?;
            save(&format!("club-{club}-activity-{act}-edit.json"), &a)?;
            println!("activity\t{act}\tactive {}\tpublic {}\tname {}", s(&a, "active"), s(&a, "public"), s(&a, "name"));
            Ok(())
        }
        other => Err(format!("nadeo-here: unknown subcommand `{other}` (club | campaigns | campaign | campaign-create | maps)")),
    }
}

/// The core records of a list of uids, in chunks of 50 (the route's cap).
fn core_records(auth_core: &str, uids: &[String]) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for chunk in uids.chunks(50) {
        let url = format!("{CORE}/maps/?mapUidList={}", chunk.join(","));
        let v = get_json(auth_core, &url)?;
        if let Some(a) = v.as_array() {
            out.extend(a.iter().cloned());
        }
    }
    Ok(out)
}

fn urlenc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[allow(dead_code)]
pub fn outdir_default() -> &'static Path {
    Path::new("/home/vjeux/shoot/u10s")
}

/// `POST /club/{club}/activity/{id}/edit {"active":…,"public":…}` — the club
/// page lists an activity only when it is active AND public.
fn activity_edit(auth_live: &str, club: &str, activity: &str, active: bool, public: bool) -> Result<Value, String> {
    let body = format!("{{\"active\":{active},\"public\":{public}}}");
    post_json(auth_live, &format!("{LIVE}/api/token/club/{club}/activity/{activity}/edit"), &body)
}

/// `POST /club/{club}/campaign/{id}/edit {"name":…,"playlist":[{mapUid,position}…]}` —
/// the campaign's whole playlist in the given order (an entry per uid).
pub fn campaign_set(auth_live: &str, club: &str, campaign: &str, name: &str, uids: &[String]) -> Result<Value, String> {
    let playlist: Vec<String> = uids.iter().enumerate().map(|(i, u)| format!("{{\"mapUid\":\"{u}\",\"position\":{i}}}")).collect();
    let body = format!("{{\"name\":\"{}\",\"playlist\":[{}]}}", name.replace('"', "\\\""), playlist.join(","));
    post_json(auth_live, &format!("{LIVE}/api/token/club/{club}/campaign/{campaign}/edit"), &body)
}

/// `POST /club/{club}/campaign/create` → (campaignId, activityId); the activity is
/// activated + published like `campaign-create` does.
pub fn campaign_create(auth_live: &str, club: &str, name: &str) -> Result<(String, String), String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let body = format!("{{\"name\":\"{}\",\"description\":\"\",\"color\":\"\",\"useCase\":2,\"publicationTimestamp\":{now},\"mediaUrl\":\"\",\"video\":false}}", name.replace('"', "\\\""));
    let v = post_json(auth_live, &format!("{LIVE}/api/token/club/{club}/campaign/create"), &body)?;
    let camp = s(&v, "campaignId");
    let act = s(&v, "activityId");
    if camp == "-" {
        return Err(format!("campaign create answered without a campaignId: {}", serde_json::to_string(&v).unwrap_or_default().chars().take(300).collect::<String>()));
    }
    if act != "-" {
        if let Err(e) = activity_edit(auth_live, club, &act, true, true) {
            eprintln!("activity {act}: could not activate/publish ({e})");
        }
    }
    Ok((camp, act))
}

/// The fresh (core, live) tokens for a batch: the plugin's files when young, else a
/// mint through the game (no render lock: the batch runs alone on the box).
pub fn batch_tokens(shootctl: &str) -> Result<(String, String), String> {
    tokens_opt(shootctl, &format!("batch-{}", std::process::id()), false)
}

/// `tinyctl nadeo-here club-mirror --club C --campaign-prefix "Tiny U10S PART " --parts-dir D
/// [--only 01,02] [--outdir O]` — MIRROR an already-published set into another club:
/// for each `D/results-pNN.tsv` (the publish-batch results: path, name, uid, mapId,
/// how, bytes, md5, verdict) create the campaign `<prefix><N>` in club C (or reuse
/// the id recorded in `O/club-<C>-campaigns.tsv`) and write its playlist = the
/// part's IDENTICAL uids in file order. No map bytes move: the records already
/// exist on Nadeo (vjeux, 2026-09-13: "altered u10s at hunt" gets the whole tiny
/// set, organised like the Tiny club). Runs ON THE BOX (tokens).
pub fn club_mirror(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let club = f("--club").ok_or("club-mirror needs --club ID")?;
    let prefix = f("--campaign-prefix").unwrap_or_else(|| "Tiny U10S PART ".into());
    let parts_dir = std::path::PathBuf::from(f("--parts-dir").ok_or("club-mirror needs --parts-dir D (results-pNN.tsv files)")?);
    let outdir = std::path::PathBuf::from(f("--outdir").unwrap_or_else(|| "/home/vjeux/shoot/u10s".into()));
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let only: Option<Vec<String>> = f("--only").map(|s| s.split(',').map(|x| format!("{:02}", x.trim().parse::<usize>().unwrap_or(0))).collect());
    let shootctl = f("--shootctl").unwrap_or_else(|| "/home/vjeux/trackmania-tas/tools/target/release/shootctl".into());
    let (_core, live) = tokens_opt(&shootctl, "club-mirror", false)?;
    let auth_live = format!("Authorization: {live}");
    let known_path = outdir.join(format!("club-{club}-campaigns.tsv"));
    let mut known: std::collections::BTreeMap<String, (String, String)> = std::fs::read_to_string(&known_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c.len() >= 3).then(|| (c[0].to_string(), (c[1].to_string(), c[2].to_string())))
        })
        .collect();
    let mut parts: Vec<(String, std::path::PathBuf)> = std::fs::read_dir(&parts_dir)
        .map_err(|e| format!("{}: {e}", parts_dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.strip_prefix("results-p").and_then(|r| r.strip_suffix(".tsv")).map(|p| (p.to_string(), e.path()))
        })
        .filter(|(p, _)| only.as_ref().map(|o| o.contains(p)).unwrap_or(true))
        .collect();
    parts.sort();
    println!("part\tcampaign\tactivity\tmaps\tverdict");
    let mut failed = 0usize;
    for (part, path) in &parts {
        let uids: Vec<String> = std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .skip(1)
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                (c.len() >= 8 && c[7] == "IDENTICAL" && c[2] != "-").then(|| c[2].to_string())
            })
            .collect();
        if uids.is_empty() {
            println!("{part}\t-\t-\t0\tno IDENTICAL rows");
            failed += 1;
            continue;
        }
        let name = format!("{prefix}{}", part.trim_start_matches('0'));
        let (camp, act) = match known.get(part) {
            Some(x) => x.clone(),
            None => match campaign_create(&auth_live, &club, &name) {
                Ok(x) => {
                    known.insert(part.clone(), x.clone());
                    let rows: String = known.iter().map(|(p, (c, a))| format!("{p}\t{c}\t{a}\n")).collect();
                    let _ = std::fs::write(&known_path, rows);
                    x
                }
                Err(e) => {
                    println!("{part}\t-\t-\t{}\tcampaign create FAILED: {}", uids.len(), e.lines().next().unwrap_or(""));
                    failed += 1;
                    continue;
                }
            },
        };
        match campaign_set(&auth_live, &club, &camp, &name, &uids) {
            Ok(v) => {
                let n = serde_json::to_string(&v).unwrap_or_default().matches("\"mapUid\"").count();
                println!("{part}\t{camp}\t{act}\t{n}\t{}", if n == uids.len() { "OK" } else { "COUNT MISMATCH" });
                if n != uids.len() {
                    failed += 1;
                }
            }
            Err(e) => {
                println!("{part}\t{camp}\t{act}\t{}\tplaylist FAILED: {}", uids.len(), e.lines().next().unwrap_or(""));
                failed += 1;
            }
        }
    }
    if failed > 0 {
        return Err(format!("{failed} of {} parts failed", parts.len()));
    }
    Ok(())
}
