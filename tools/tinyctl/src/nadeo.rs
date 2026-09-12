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
/// first so a stale one is never read back as fresh.
pub fn tokens(shootctl: &str, owner: &str) -> Result<(String, String), String> {
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
            if i + 3 < cs.len() + 0 && cs[i + 1..].iter().take(3).all(|c| c.is_ascii_hexdigit()) && cs.len() >= i + 4 {
                i += 4;
                continue;
            }
            if i + 1 < cs.len() && cs[i + 1] == '$' {
                out.push('$');
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
    let sub = args.first().ok_or("nadeo-here needs a subcommand: club | campaigns | campaign | campaign-create | maps")?.clone();
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| "/home/vjeux/shoot/u10s".into()));
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let shootctl = f("--shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    let owner = format!("nadeo-{}", std::process::id());
    let (core, live) = tokens(&shootctl, &owner)?;
    let auth_core = format!("Authorization: {core}");
    let auth_live = format!("Authorization: {live}");
    let save = |name: &str, v: &Value| -> Result<(), String> {
        let p = outdir.join(name);
        std::fs::write(&p, serde_json::to_string_pretty(v).unwrap_or_default()).map_err(|e| format!("{}: {e}", p.display()))?;
        eprintln!("wrote {}", p.display());
        Ok(())
    };
    match sub.as_str() {
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
        "campaigns" => {
            let club = f("--club").ok_or("campaigns needs --club ID")?;
            let length = f("--length").unwrap_or_else(|| "100".into());
            let offset = f("--offset").unwrap_or_else(|| "0".into());
            let url = format!("{LIVE}/api/token/club/{club}/campaign?length={length}&offset={offset}");
            let v = get_json(&auth_live, &url)?;
            save(&format!("club-{club}-campaigns.json"), &v)?;
            println!("total {}\tclub {}", s(&v, "itemCount"), s(&v, "clubName"));
            println!("campaignId\tname\tmaps\tpublicationTimestamp\tactivityId");
            for c in v.get("clubCampaignList").and_then(|l| l.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
                let camp = c.get("campaign").cloned().unwrap_or(Value::Null);
                println!("{}\t{}\t{}\t{}\t{}", s(c, "campaignId"), s(c, "name"), s(&camp, "mapsCount"), s(c, "publicationTimestamp"), s(c, "activityId"));
            }
            Ok(())
        }
        "campaign" => {
            let club = f("--club").ok_or("campaign needs --club ID")?;
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
