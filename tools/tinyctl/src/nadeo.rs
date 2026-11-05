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
    // RETRIED (2026-10-21): the Live API answers a GET with an HTML error page (a 5xx
    // through the CDN) or a stray 401 now and then; a playlist swap reads every
    // campaign and room of the club (300+ GETs), so one such answer failed the whole
    // swap and STOPped the apply twice tonight (Plastic 156579, 156580). Five tries,
    // 3-8 s apart; a verdict only after the last.
    let mut last = String::new();
    for attempt in 0..5u32 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(3 + 5 * u64::from(attempt - 1)));
        }
        let (body, code) = match curl(&["-H", auth, url]) {
            Ok(r) => r,
            Err(e) => {
                last = e;
                continue;
            }
        };
        if code != "200" {
            last = format!("GET {url}: HTTP {code} {}", &body[..body.len().min(300)]);
            if code == "404" || code == "403" {
                return Err(last);
            }
            continue;
        }
        match serde_json::from_str(&body) {
            Ok(v) => return Ok(v),
            Err(e) => last = format!("GET {url}: not JSON ({e}): {}", &body[..body.len().min(200)]),
        }
    }
    Err(format!("{last} (after 5 tries)"))
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
/// than `TOKEN_REUSE_SECS`: then they are reused and the lock is not taken at
/// all, so a run of reads costs the shared render lock one slice, not one per
/// command. 40 min was too long: the Ice U10S publishes of 21–24 (2026-09-13,
/// tokens 26–40 min old) all answered `club/mine: HTTP 401` — the plugin's
/// token is not a fresh one at mint time, so its remaining life is shorter
/// than an hour; 20 min has never failed.
const TOKEN_REUSE_SECS: u64 = 20 * 60;

/// The plugin's (core, live) token files when both are younger than
/// `TOKEN_REUSE_SECS` and look like tokens; `None` means mint under the lock.
pub fn fresh_tokens() -> Option<(String, String)> {
    fresh_tokens_in(STORE)
}

/// The same, from any directory (the devserver's copy pulled over the bridge —
/// `tinyctl round-publish`, 2026-09-24).
pub fn fresh_tokens_in(dir: &str) -> Option<(String, String)> {
    let fresh = |aud: &str| -> Option<String> {
        let p = format!("{dir}/token-{aud}.txt");
        let t = std::fs::read_to_string(&p).ok()?.trim().to_string();
        if t.len() <= 20 {
            return None;
        }
        // a token is fresh while ITS OWN expiry (the JWT `exp` claim) is ≥ 90 s away; the
        // file-age rule (TOKEN_REUSE_SECS) is only the fallback for a token that does not
        // decode. The age rule refused a 46-min-old token that was still valid for 13 min
        // while the box was in human use and no re-mint was possible (2026-09-27 15:02).
        match jwt_exp(&t) {
            Some(exp) => {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
                (exp > now + 90).then_some(t)
            }
            None => {
                let age = std::fs::metadata(&p).ok()?.modified().ok()?.elapsed().ok()?;
                (age.as_secs() < TOKEN_REUSE_SECS).then_some(t)
            }
        }
    };
    Some((fresh("NadeoServices")?, fresh("NadeoLiveServices")?))
}

/// The `exp` claim of a JWT (`[nadeo_v1 t=]header.payload.sig`), seconds since the epoch.
pub fn jwt_exp(token: &str) -> Option<u64> {
    let raw = token.split_whitespace().last()?;
    let raw = raw.strip_prefix("t=").unwrap_or(raw);
    let payload = raw.split('.').nth(1)?;
    let bytes = b64url_decode(payload)?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v.get("exp")?.as_u64()
}

fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

pub fn tokens(shootctl: &str, owner: &str) -> Result<(String, String), String> {
    tokens_opt(shootctl, owner, true)
}

/// `lock = false`: mint WITHOUT the render lock (vjeux holding the lock while he
/// plays and asking for an API fix, 2026-09-12 22:22Z — the mint is a 2-second
/// plugin call that loads nothing).
pub fn tokens_opt(shootctl: &str, owner: &str, lock: bool) -> Result<(String, String), String> {
    // TINYCTL_TOKEN_DIR=DIR: tokens minted elsewhere (round-publish / port-fix-publish pull
    // them from the box) — the devserver has no plugin to mint from (2026-09-25)
    if let Ok(dir) = std::env::var("TINYCTL_TOKEN_DIR") {
        return fresh_tokens_in(&dir).ok_or_else(|| format!("no fresh tokens in {dir} (expiry ≥ 90 s away, or younger than {} min)", TOKEN_REUSE_SECS / 60));
    }
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
    if sub == "club-layout" {
        return club_layout(args);
    }
    if sub == "club-rooms" {
        return club_rooms(args);
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
            // --parent ID: the activities inside a folder (Everios' hunt-club layout: one
            // folder per alteration, its rooms inside); the room rows add their map count
            let parent = f("--parent").map(|p| format!("&parentActivityId={p}")).unwrap_or_default();
            let url = format!("{LIVE}/api/token/club/{club}/activity?length={length}&offset={offset}&active={active}{parent}");
            let v = get_json(&auth_live, &url)?;
            save(&format!("club-{club}-activities-{offset}.json"), &v)?;
            println!("total {}\tmaxPage {}", s(&v, "itemCount"), s(&v, "maxPage"));
            println!("activityId\ttype\tname\tcampaignId\tpublic\tactive\tposition\tmaps");
            for a in v.get("activityList").and_then(|l| l.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
                let maps = a.get("room").and_then(|r| r.get("maps")).and_then(|m| m.as_array()).map(|m| m.len().to_string()).unwrap_or_else(|| "-".into());
                println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", s(a, "id"), s(a, "activityType"), s(a, "name"), s(a, "campaignId"), s(a, "public"), s(a, "active"), s(a, "position"), maps);
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
        "club-inventory" => {
            // tinyctl nadeo-here club-inventory --club ID --out INV.tsv [--sets "Wet Wood,Wood"]
            //   [--fetch-root DIR] [--no-lock]
            // The LIVE inventory of a hunt club in `rollout-publish --inventory` form —
            // every campaign (paginated: the club list is 100 a page, the U10S club has
            // 351), its playlist in position order, the core record per uid: one row
            // `campaignId campaign pos uid name authorScore gold silver bronze collection
            // author mapId file`. `--sets` keeps the campaigns whose name starts with one
            // of the words (`<set> Part NN`); `--fetch-root DIR` downloads each live file
            // to `DIR/<set-dir>/pNN/<pos>-<name>.Map.Gbx` (set-dir = the set word without
            // spaces, lower-case; NN = the part number) — the thumbnails and the
            // live-vs-build diff need the files (2026-10-17: the 10-10 inventory was stale
            // after the Wet Wood/Wood wipes, and /tmp/ww/live-* died with the box).
            let club = f("--club").ok_or("club-inventory needs --club ID")?;
            let out = PathBuf::from(f("--out").ok_or("club-inventory needs --out INV.tsv")?);
            let sets: Option<Vec<String>> = f("--sets").map(|x| x.split(',').map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect());
            let root = f("--fetch-root").map(PathBuf::from);
            let mut camps: Vec<(String, String)> = Vec::new();
            let mut offset = 0usize;
            loop {
                let v = get_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/campaign?length=100&offset={offset}"))?;
                let list = v.get("clubCampaignList").and_then(|l| l.as_array()).cloned().unwrap_or_default();
                for c in &list {
                    camps.push((s(c, "campaignId"), s(c, "name")));
                }
                let total: usize = s(&v, "itemCount").parse().unwrap_or(0);
                offset += list.len();
                if list.is_empty() || offset >= total {
                    break;
                }
            }
            let part_of = |name: &str| -> Option<(String, usize)> {
                // `<set> Part NN` (the hunt clubs) or `<set> PART NN` (the Tiny U10S club, 145858)
                let (set, n) = name.rsplit_once(" Part ").or_else(|| name.rsplit_once(" PART "))?;
                Some((set.to_string(), n.trim().parse().ok()?))
            };
            let mut rows = String::from("campaignId\tcampaign\tpos\tuid\tname\tauthorScore\tgold\tsilver\tbronze\tcollection\tauthor\tmapId\tfile\tsubmitter\n");
            let (mut n_maps, mut n_files) = (0usize, 0usize);
            let mut wanted: Vec<(String, String, String, usize)> = Vec::new();
            for (id, name) in &camps {
                let Some((set, part)) = part_of(name) else { continue };
                if sets.as_ref().map(|ss| !ss.iter().any(|w| *w == set)).unwrap_or(false) {
                    continue;
                }
                wanted.push((id.clone(), name.clone(), set, part));
            }
            wanted.sort_by(|a, b| a.2.cmp(&b.2).then(a.3.cmp(&b.3)));
            println!("club {club}: {} campaigns, {} in the wanted sets", camps.len(), wanted.len());
            for (id, name, set, part) in &wanted {
                let v = get_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/campaign/{id}"))?;
                let inner = v.get("campaign").cloned().unwrap_or(Value::Null);
                let mut uids: Vec<(u64, String)> = inner
                    .get("playlist")
                    .and_then(|l| l.as_array())
                    .map(|a| a.iter().map(|e| (e.get("position").and_then(|p| p.as_u64()).unwrap_or(0), s(e, "mapUid"))).collect())
                    .unwrap_or_default();
                uids.sort();
                let recs = core_records(&auth_core, &uids.iter().map(|(_, u)| u.clone()).collect::<Vec<_>>())?;
                let dir = root.as_ref().map(|r| r.join(set.to_lowercase().replace(' ', "")).join(format!("p{part}")));
                if let Some(d) = &dir {
                    std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
                }
                for (pos, uid) in &uids {
                    let rec = recs.iter().find(|r| s(r, "mapUid") == *uid).cloned().unwrap_or(Value::Null);
                    let fname = format!("{:02}-{}.Map.Gbx", pos + 1, file_safe_name(&s(&rec, "name")));
                    let mut file_note = String::from("-");
                    if let Some(d) = &dir {
                        let p = d.join(&fname);
                        let url = s(&rec, "fileUrl");
                        if url == "-" {
                            file_note = "NO fileUrl".into();
                        } else if p.exists() && std::fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false) {
                            file_note = format!("have {}", p.display());
                        } else {
                            let (_, code) = curl(&["-L", "-H", &auth_core, "-o", p.to_str().unwrap(), &url])?;
                            let n = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                            if !code.starts_with('2') || n == 0 {
                                let _ = std::fs::remove_file(&p);
                                return Err(format!("{name} pos {pos} {uid}: download HTTP {code}, {n} bytes"));
                            }
                            n_files += 1;
                            file_note = p.display().to_string();
                        }
                    }
                    rows.push_str(&format!(
                        "{id}\t{name}\t{pos}\t{uid}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{file_note}\t{}\n",
                        s(&rec, "name"), s(&rec, "authorScore"), s(&rec, "goldScore"), s(&rec, "silverScore"), s(&rec, "bronzeScore"), s(&rec, "collectionName"), s(&rec, "author"), s(&rec, "mapId"), s(&rec, "submitter")
                    ));
                    n_maps += 1;
                }
                std::fs::write(&out, &rows).map_err(|e| format!("{}: {e}", out.display()))?;
                println!("{set} part {part} ({id}): {} maps", uids.len());
            }
            println!("club-inventory: {} campaigns, {n_maps} maps, {n_files} files downloaded → {}", wanted.len(), out.display());
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
                // --fetch-only SUBSTR[,SUBSTR…]: download only the maps whose name contains one
                let only_ok = f("--fetch-only").map(|v| v.split(',').any(|w| s(&rec, "name").contains(w.trim()))).unwrap_or(true);
                if let (Some(d), true) = (&fetch, only_ok) {
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
            // --uids A,B,… or --uids-file F (one uid per line, extra columns ignored);
            // --fetch DIR downloads each map's live file to DIR/<uid>.Map.Gbx (the live
            // bytes of an in-place edit, 2026-10-29: the staged copies die with /tmp)
            let uids: Vec<String> = match (f("--uids"), f("--uids-file")) {
                (Some(u), _) => u.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
                (None, Some(file)) => std::fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?.lines().filter_map(|l| l.split('\t').next().map(|x| x.trim().to_string())).filter(|x| !x.is_empty() && !x.starts_with('#')).collect(),
                (None, None) => return Err("maps needs --uids A,B,… or --uids-file F".into()),
            };
            let recs = core_records(&auth_core, &uids)?;
            save("maps.json", &Value::Array(recs.clone()))?;
            let fetch = f("--fetch").map(PathBuf::from);
            if let Some(d) = &fetch {
                std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
            println!("mapUid\tname\tauthorTime\tgold\tsilver\tbronze\tcollection\tauthor\tmapId\tfileUrl{}", if fetch.is_some() { "\tfile" } else { "" });
            let mut n_files = 0usize;
            for r in &recs {
                let mut file_note = String::new();
                if let Some(d) = &fetch {
                    let p = d.join(format!("{}.Map.Gbx", s(r, "mapUid")));
                    let url = s(r, "fileUrl");
                    if url == "-" {
                        file_note = "\tNO fileUrl".into();
                    } else {
                        let (_, code) = curl(&["-L", "-H", &auth_core, "-o", p.to_str().unwrap(), &url])?;
                        let n = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                        if !code.starts_with('2') || n == 0 {
                            let _ = std::fs::remove_file(&p);
                            return Err(format!("{}: download HTTP {code}, {n} bytes", s(r, "mapUid")));
                        }
                        n_files += 1;
                        file_note = format!("\t{}", p.display());
                    }
                }
                println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}{file_note}", s(r, "mapUid"), s(r, "name"), s(r, "authorScore"), s(r, "goldScore"), s(r, "silverScore"), s(r, "bronzeScore"), s(r, "collectionName"), s(r, "author"), s(r, "mapId"), s(r, "fileUrl"));
            }
            if fetch.is_some() {
                eprintln!("maps: {n_files} of {} files downloaded", recs.len());
            }
            Ok(())
        }
        // tinyctl nadeo-here leaderboards --uids-file F --out records.tsv [--names names.tsv] [--no-lock]
        // every map's world Personal_Best board, paged 100 at a time (accountId, position, score),
        // then the display names of every account seen (core `accounts/displayNames`, 50 per call).
        // For the U10S map-1000 credits (vjeux on Discord 2026-09-15 05:36 PT: "the list of all the
        // people that played all the maps … keep that list around on whitestick").
        "leaderboards" => {
            let uids_file = f("--uids-file").ok_or("leaderboards needs --uids-file (one uid per line, optional \tname)")?;
            let out = PathBuf::from(f("--out").ok_or("leaderboards needs --out records.tsv")?);
            let text = std::fs::read_to_string(&uids_file).map_err(|e| format!("{uids_file}: {e}"))?;
            let uids: Vec<(String, String)> = text.lines().filter(|l| !l.trim().is_empty()).map(|l| { let mut it = l.split('\t'); (it.next().unwrap_or("").trim().to_string(), it.next().unwrap_or("").trim().to_string()) }).collect();
            // resume: uids already in the output are skipped
            // …and the ones with NO records too: they leave no row in the output, so a
            // rerun walked every empty board again (2026-10-18: 789 of 1,574 redone after a
            // token expiry). `OUT.done` lists every uid finished, records or not.
            let done_path = format!("{}.done", out.display());
            let mut done: std::collections::HashSet<String> = std::fs::read_to_string(&out).unwrap_or_default().lines().skip(1).filter_map(|l| l.split('\t').next().map(String::from)).collect();
            done.extend(std::fs::read_to_string(&done_path).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()));
            let mut done_w = std::fs::OpenOptions::new().create(true).append(true).open(&done_path).map_err(|e| format!("{done_path}: {e}"))?;
            let mut w = std::fs::OpenOptions::new().create(true).append(true).open(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            use std::io::Write;
            if std::fs::metadata(&out).map(|m| m.len() == 0).unwrap_or(true) {
                writeln!(w, "mapUid\tmapName\taccountId\tposition\tscore").map_err(|e| e.to_string())?;
            }
            let mut accounts: std::collections::HashSet<String> = std::fs::read_to_string(&out).unwrap_or_default().lines().skip(1).filter_map(|l| l.split('\t').nth(2).map(String::from)).collect();
            let mut n_maps = 0usize;
            let mut n_rows = 0usize;
            let mut live_auth = auth_live.clone();
            // --cap N: stop a board after N records (U10S_01 has 45k+; the regulars are
            // found across the hundreds of small boards, not at the bottom of the big ones)
            let cap: usize = f("--cap").and_then(|c| c.parse().ok()).unwrap_or(usize::MAX);
            for (i, (uid, name)) in uids.iter().enumerate() {
                if done.contains(uid) {
                    continue;
                }
                let mut offset = 0usize;
                let mut rows: Vec<String> = Vec::new();
                let mut transient = 0u32;
                loop {
                    let url = format!("{LIVE}/api/token/leaderboard/group/Personal_Best/map/{uid}/top?length=100&onlyWorld=true&offset={offset}");
                    let v = match get_json(&live_auth, &url) {
                        Ok(v) => v,
                        Err(e) if e.contains("HTTP 401") => {
                            // the live token lasts about an hour: mint again (no render lock)
                            eprintln!("token expired — minting again");
                            let (_, live2) = tokens_opt(&shootctl, &owner, false)?;
                            live_auth = format!("Authorization: {live2}");
                            continue;
                        }
                        Err(e) if e.contains("HTTP 429") => {
                            eprintln!("rate limited — 30 s pause");
                            std::thread::sleep(Duration::from_secs(30));
                            continue;
                        }
                        Err(e) => {
                            // a 5xx HTML page, a dropped connection: transient — up to 5 retries
                            transient += 1;
                            if transient > 5 {
                                return Err(e);
                            }
                            eprintln!("transient error ({transient}/5): {} — 15 s pause", e.chars().take(120).collect::<String>());
                            std::thread::sleep(Duration::from_secs(15));
                            continue;
                        }
                    };
                    transient = 0;
                    let top = v.get("tops").and_then(|t| t.as_array()).and_then(|a| a.first()).and_then(|z| z.get("top")).and_then(|t| t.as_array()).cloned().unwrap_or_default();
                    for r in &top {
                        rows.push(format!("{uid}\t{name}\t{}\t{}\t{}", s(r, "accountId"), s(r, "position"), s(r, "score")));
                        accounts.insert(s(r, "accountId"));
                    }
                    if top.len() < 100 || offset + 100 >= cap {
                        break;
                    }
                    offset += 100;
                    std::thread::sleep(Duration::from_millis(250));
                }
                for r in &rows {
                    writeln!(w, "{r}").map_err(|e| e.to_string())?;
                }
                w.flush().map_err(|e| e.to_string())?;
                writeln!(done_w, "{uid}").map_err(|e| e.to_string())?;
                n_maps += 1;
                n_rows += rows.len();
                eprintln!("[{:>4}/{}] {name} {uid}: {} records", i + 1, uids.len(), rows.len());
                std::thread::sleep(Duration::from_millis(300));
            }
            eprintln!("{n_maps} maps, {n_rows} records, {} distinct accounts", accounts.len());
            if let Some(names_out) = f("--names") {
                let mut known: std::collections::HashMap<String, String> = std::fs::read_to_string(&names_out).unwrap_or_default().lines().skip(1).filter_map(|l| { let mut it = l.split('\t'); Some((it.next()?.to_string(), it.next()?.to_string())) }).collect();
                let todo: Vec<String> = accounts.iter().filter(|a| !known.contains_key(*a) && a.len() > 30).cloned().collect();
                eprintln!("resolving {} display names ({} known)", todo.len(), known.len());
                for chunk in todo.chunks(50) {
                    let url = format!("{CORE}/accounts/displayNames?accountIdList={}", chunk.join(","));
                    match get_json(&auth_core, &url) {
                        Ok(Value::Array(arr)) => {
                            for e in arr {
                                known.insert(s(&e, "accountId"), s(&e, "displayName"));
                            }
                        }
                        Ok(other) => eprintln!("displayNames: unexpected {}", other.to_string().chars().take(200).collect::<String>()),
                        Err(e) => {
                            eprintln!("displayNames: {e}");
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(300));
                }
                let mut t = String::from("accountId\tdisplayName\n");
                let mut ks: Vec<_> = known.iter().collect();
                ks.sort();
                for (a, n) in ks {
                    t.push_str(&format!("{a}\t{n}\n"));
                }
                std::fs::write(&names_out, t).map_err(|e| format!("{names_out}: {e}"))?;
                eprintln!("wrote {names_out}: {} names", known.len());
            }
            Ok(())
        }
        // tinyctl nadeo-here reuid-swap --club C --pairs pairs.tsv [--dry-run] [--no-lock]
        // pairs.tsv: `oldUid\tnewUid` per line. Every campaign playlist and every room map
        // list of the club that contains an old uid is rewritten with the new one (one
        // write per touched campaign / room). The records reset, option (b): the new uid
        // starts an empty leaderboard; the old maps simply drop out of the club.
        // tinyctl nadeo-here room-drop-map --club C --room ID --uid U [--no-lock]: the room's
        // playlist without U (Everios96 2026-11-04 20:49: Giant 300's 9 MB file blocks every
        // edit of room "Giant V3 201-300" — "we need to remove this map for now")
        "room-drop-map" => {
            let club = f("--club").ok_or("room-drop-map needs --club ID")?;
            let id = f("--room").ok_or("room-drop-map needs --room ID")?;
            let uid = f("--uid").ok_or("room-drop-map needs --uid U")?;
            let d = get_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/room/{id}"))?;
            let room = d.get("room").cloned().unwrap_or(Value::Null);
            let maps: Vec<String> = room.get("maps").and_then(|m| m.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
            if !maps.iter().any(|m| m == &uid) {
                return Err(format!("room {id}: {uid} is not in its playlist ({} maps)", maps.len()));
            }
            let new_maps: Vec<String> = maps.iter().filter(|m| *m != &uid).cloned().collect();
            let name = s(&d, "name");
            let settings: Vec<Value> = room.get("scriptSettings").and_then(|o| o.as_object()).map(|o| o.values().map(|v| serde_json::json!({"key": s(v, "key"), "value": s(v, "value"), "type": s(v, "type")})).collect()).unwrap_or_default();
            let body = serde_json::json!({
                "name": name, "region": s(&room, "region"), "maxPlayersPerServer": room.get("maxPlayers").and_then(|x| x.as_u64()).unwrap_or(100),
                "script": s(&room, "script"), "settings": settings,
                "maps": new_maps, "scalable": 0, "password": 0, "shufflePlaylist": 0
            });
            let r = post_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/room/{id}/edit"), &body.to_string())?;
            let n = r["room"]["maps"].as_array().map(|a| a.len()).unwrap_or(0);
            println!("room\t{id}\t{name}\tdropped {uid}\t{} → {n} maps", maps.len());
            Ok(())
        }
        "reuid-swap" => {
            let club = f("--club").ok_or("reuid-swap needs --club ID")?;
            let pairs_p = f("--pairs").ok_or("reuid-swap needs --pairs pairs.tsv")?;
            let dry = tmmaps::cli::has(args, "--dry-run");
            // --name-filter S: only campaigns whose LIST entry name contains S (case-insensitive)
            // get their playlist fetched — the club has ~390 campaigns and one set's maps sit in
            // ~40 of them; the per-campaign GET was ~5 of a 16-minute wipe chunk (2026-11-04)
            let name_filter = f("--name-filter").map(|v| v.to_lowercase());
            let pairs: std::collections::HashMap<String, String> = std::fs::read_to_string(&pairs_p).map_err(|e| format!("{pairs_p}: {e}"))?.lines().filter_map(|l| { let mut it = l.split('\t'); Some((it.next()?.trim().to_string(), it.next()?.trim().to_string())) }).filter(|(a, b)| a.len() > 20 && b.len() > 20).collect();
            eprintln!("{} uid pairs", pairs.len());
            // campaigns
            let mut touched_c = 0;
            let mut offset = 0;
            loop {
                let v = get_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/campaign?length=100&offset={offset}"))?;
                let list = v.get("clubCampaignList").and_then(|l| l.as_array()).cloned().unwrap_or_default();
                if list.is_empty() {
                    break;
                }
                for c in &list {
                    let camp = c.get("campaignId").and_then(|x| x.as_u64()).or_else(|| c.get("id").and_then(|x| x.as_u64())).unwrap_or(0).to_string();
                    if let Some(nf) = &name_filter {
                        let list_name = s(c, "name").to_lowercase();
                        if !list_name.contains(nf.as_str()) {
                            continue;
                        }
                    }
                    let d = get_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/campaign/{camp}"))?;
                    let inner = d.get("campaign").cloned().unwrap_or(Value::Null);
                    let name = s(&d, "name");
                    let mut uids: Vec<(u64, String)> = inner.get("playlist").and_then(|l| l.as_array()).map(|a| a.iter().map(|e| (e.get("position").and_then(|p| p.as_u64()).unwrap_or(0), s(e, "mapUid"))).collect()).unwrap_or_default();
                    uids.sort();
                    let mut hits = 0;
                    let new_uids: Vec<String> = uids.iter().map(|(_, u)| match pairs.get(u) { Some(n) => { hits += 1; n.clone() } None => u.clone() }).collect();
                    if hits > 0 {
                        touched_c += 1;
                        if dry {
                            println!("campaign\t{camp}\t{name}\t{hits} uids to swap (dry run)");
                        } else {
                            let r = campaign_set(&auth_live, &club, &camp, &name, &new_uids)?;
                            let n = serde_json::to_string(&r).unwrap_or_default().matches("\"mapUid\"").count();
                            println!("campaign\t{camp}\t{name}\t{hits} swapped\t{n} maps in the playlist{}", if n == new_uids.len() { "" } else { "\tCOUNT MISMATCH" });
                        }
                        std::thread::sleep(Duration::from_millis(400));
                    }
                }
                if list.len() < 100 {
                    break;
                }
                offset += 100;
            }
            // rooms (activities of type room)
            let mut touched_r = 0;
            let mut offset = 0;
            loop {
                let v = get_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/activity?length=100&offset={offset}&active=true"))?;
                let list = v.get("activityList").and_then(|l| l.as_array()).cloned().unwrap_or_default();
                if list.is_empty() {
                    break;
                }
                for a in &list {
                    if s(a, "activityType") != "room" {
                        continue;
                    }
                    let id = a.get("id").and_then(|x| x.as_u64()).unwrap_or(0);
                    let d = get_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/room/{id}"))?;
                    let room = d.get("room").cloned().unwrap_or(Value::Null);
                    let maps: Vec<String> = room.get("maps").and_then(|m| m.as_array()).map(|a| a.iter().map(|m| m.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
                    let mut hits = 0;
                    let new_maps: Vec<String> = maps.iter().map(|u| match pairs.get(u) { Some(n) => { hits += 1; n.clone() } None => u.clone() }).collect();
                    if hits == 0 {
                        continue;
                    }
                    touched_r += 1;
                    let name = s(&d, "name");
                    if dry {
                        println!("room\t{id}\t{name}\t{hits} uids to swap (dry run)");
                        continue;
                    }
                    // the GET gives scriptSettings as an object keyed by setting; the edit wants the array form
                    let settings: Vec<Value> = room.get("scriptSettings").and_then(|o| o.as_object()).map(|o| o.values().map(|v| serde_json::json!({"key": s(v, "key"), "value": s(v, "value"), "type": s(v, "type")})).collect()).unwrap_or_default();
                    let body = serde_json::json!({
                        "name": name, "region": s(&room, "region"), "maxPlayersPerServer": room.get("maxPlayers").and_then(|x| x.as_u64()).unwrap_or(100),
                        "script": s(&room, "script"), "settings": settings,
                        "maps": new_maps, "scalable": 0, "password": 0, "shufflePlaylist": 0
                    });
                    let r = post_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/room/{id}/edit"), &body.to_string())?;
                    let n = r["room"]["maps"].as_array().map(|a| a.len()).unwrap_or(0);
                    println!("room\t{id}\t{name}\t{hits} swapped\t{n} maps{}", if n == new_maps.len() { "" } else { "\tCOUNT MISMATCH" });
                    std::thread::sleep(Duration::from_millis(400));
                }
                if list.len() < 100 {
                    break;
                }
                offset += 100;
            }
            println!("{touched_c} campaigns, {touched_r} rooms {}", if dry { "would be rewritten" } else { "rewritten" });
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
        "ghosts" => {
            // tinyctl nadeo-here ghosts --uids-file F --out-dir D [--top N] [--no-lock]
            // — the top-N records' ghost files of each map (Live top → Core mapRecords url →
            // download), as D/<uid>-<rank>.Ghost.Gbx. Everios96 2026-09-19 06:00: "check the
            // records on the original maps" to see which stage decks are driven.
            let uids_file = f("--uids-file").ok_or("ghosts needs --uids-file (one uid per line, optional \tname)")?;
            let out_dir = PathBuf::from(f("--out-dir").ok_or("ghosts needs --out-dir D")?);
            std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
            let top: usize = f("--top").and_then(|v| v.parse().ok()).unwrap_or(3);
            let text = std::fs::read_to_string(&uids_file).map_err(|e| format!("{uids_file}: {e}"))?;
            let uids: Vec<String> = text.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split('\t').next().unwrap_or("").trim().to_string()).collect();
            let mut got = 0usize;
            for uid in &uids {
                // the map's id (Core) and its top accounts (Live)
                let mi = get_json(&auth_core, &format!("{CORE}/maps/?mapUidList={uid}"))?;
                let map_id = mi.as_array().and_then(|a| a.first()).map(|m| s(m, "mapId")).unwrap_or_default();
                if map_id.is_empty() {
                    eprintln!("{uid}: no mapId");
                    continue;
                }
                let lb = get_json(&auth_live, &format!("{LIVE}/api/token/leaderboard/group/Personal_Best/map/{uid}/top?length={top}&onlyWorld=true&offset=0"))?;
                let accounts: Vec<String> = lb.get("tops").and_then(|t| t.as_array()).and_then(|t| t.first()).and_then(|z| z.get("top")).and_then(|t| t.as_array()).map(|a| a.iter().map(|r| s(r, "accountId")).collect()).unwrap_or_default();
                if accounts.is_empty() {
                    eprintln!("{uid}: no records");
                    continue;
                }
                let recs = get_json(&auth_core, &format!("{CORE}/v2/mapRecords/?accountIdList={}&mapId={map_id}", accounts.join(",")))?;
                for (rank, acc) in accounts.iter().enumerate() {
                    let url = recs.as_array().map(|a| a.iter().find(|r| s(r, "accountId") == *acc).map(|r| s(r, "url")).unwrap_or_default()).unwrap_or_default();
                    if url.is_empty() {
                        continue;
                    }
                    let dest = out_dir.join(format!("{uid}-{}.Ghost.Gbx", rank + 1));
                    let st = std::process::Command::new("curl").args(["-s", "--max-time", "120", "-L", "-o"]).arg(&dest).arg(&url).status().map_err(|e| e.to_string())?;
                    if st.success() && std::fs::metadata(&dest).map(|m| m.len() > 1000).unwrap_or(false) {
                        got += 1;
                        println!("{uid}\t{}\t{acc}\t{}", rank + 1, dest.display());
                    } else {
                        eprintln!("{uid} #{}: download failed", rank + 1);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(150));
                }
            }
            println!("{got} ghosts for {} maps → {}", uids.len(), out_dir.display());
            Ok(())
        }
        "activities-visibility" => {
            // tinyctl nadeo-here activities-visibility --club ID --public 0|1 [--type campaign|room] [--dry-run]
            // — every ACTIVE activity of the club (all pages) set to the asked visibility;
            // rows already there are skipped. Everios96 2026-09-18: "the campaigns are not
            // set to private, so people can play them … 351 campaigns to set to private".
            let club = f("--club").ok_or("activities-visibility needs --club ID")?;
            let public = f("--public").map(|x| x != "0").ok_or("activities-visibility needs --public 0|1")?;
            let want_type = f("--type");
            let dry = tmmaps::cli::has(args, "--dry-run");
            let mut offset = 0usize;
            let (mut seen, mut changed, mut skipped, mut failed) = (0usize, 0usize, 0usize, 0usize);
            loop {
                let v = get_json(&auth_live, &format!("{LIVE}/api/token/club/{club}/activity?length=100&offset={offset}&active=true"))?;
                let list = v.get("activityList").and_then(|l| l.as_array()).cloned().unwrap_or_default();
                if list.is_empty() {
                    break;
                }
                for a in &list {
                    seen += 1;
                    let ty = s(a, "activityType");
                    if want_type.as_deref().map(|t| t != ty).unwrap_or(false) {
                        continue;
                    }
                    let id = s(a, "id");
                    let is_public = a.get("public").and_then(|b| b.as_bool()).unwrap_or(false);
                    if is_public == public {
                        skipped += 1;
                        continue;
                    }
                    if dry {
                        println!("would set	{id}	{ty}	{}	public {is_public} -> {public}", s(a, "name"));
                        changed += 1;
                        continue;
                    }
                    match activity_edit(&auth_live, &club, &id, true, public) {
                        Ok(r) => {
                            changed += 1;
                            println!("set	{id}	{ty}	{}	public {}", s(a, "name"), s(&r, "public"));
                        }
                        Err(e) => {
                            failed += 1;
                            eprintln!("FAILED	{id}	{ty}	{}	{e}", s(a, "name"));
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(150));
                }
                offset += list.len();
                if list.len() < 100 {
                    break;
                }
            }
            println!("{seen} activities seen, {changed} {}, {skipped} already public={public}, {failed} failed", if dry { "would change" } else { "changed" });
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

/// Every campaign of a club: (campaignId, name), paged 100 at a time.
pub fn club_campaigns(auth_live: &str, club: &str) -> Result<Vec<(String, String)>, String> {
    let mut camps: Vec<(String, String)> = Vec::new();
    let mut offset = 0usize;
    loop {
        let v = get_json(auth_live, &format!("{LIVE}/api/token/club/{club}/campaign?length=100&offset={offset}"))?;
        let list = v.get("clubCampaignList").and_then(|l| l.as_array()).cloned().unwrap_or_default();
        for c in &list {
            camps.push((s(c, "campaignId"), s(c, "name")));
        }
        let total: usize = s(&v, "itemCount").parse().unwrap_or(0);
        offset += list.len();
        if list.is_empty() || offset >= total {
            break;
        }
    }
    Ok(camps)
}

/// A campaign's playlist uids, in position order.
pub fn campaign_playlist(auth_live: &str, club: &str, campaign: &str) -> Result<Vec<String>, String> {
    let v = get_json(auth_live, &format!("{LIVE}/api/token/club/{club}/campaign/{campaign}"))?;
    let mut rows: Vec<(u64, String)> = v
        .get("campaign")
        .and_then(|c| c.get("playlist"))
        .and_then(|p| p.as_array())
        .map(|a| a.iter().map(|m| (m.get("position").and_then(|x| x.as_u64()).unwrap_or(0), s(m, "mapUid"))).collect())
        .unwrap_or_default();
    rows.sort();
    Ok(rows.into_iter().map(|(_, u)| u).collect())
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

/// `tinyctl nadeo-here club-layout --club C --folder "Tiny U10S" --word Tiny --parts-dir D
/// [--room-size 100] [--outdir O] [--keep-part-rooms]` — Everios96's second club
/// layout (2026-09-14 08:35): inside the alteration's folder, ROOMS of 100 maps
/// ("Tiny V1 01-100" … "Tiny V10 901-975"; Nadeo caps a room name at 20 chars)
/// AND one CAMPAIGN per part ("Tiny Part 01" … "Tiny Part 39", 25 maps each). The
/// uids come from `D/results-pNN.tsv` (IDENTICAL rows) in part order; no map
/// bytes move. The old per-part rooms of the folder ("<X> - Part NN" /
/// "<X> U10S - Part NN") are deleted unless --keep-part-rooms. Idempotent through
/// `O/club-<C>-layout-<Folder>.tsv` (campaign + room ids). Runs ON THE BOX.
/// Every active activity of the club (optionally inside one folder), across the
/// 100-per-page listing — the hunt club holds 360+ activities and the folders
/// sit past the first page (club-layout, 2026-09-14).
fn all_activities(auth: &str, club: &str, parent: Option<u64>) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    loop {
        let parent_q = parent.map(|p| format!("&parentActivityId={p}")).unwrap_or_default();
        let v = get_json(auth, &format!("{LIVE}/api/token/club/{club}/activity?length=100&offset={offset}&active=true{parent_q}"))?;
        let page = v["activityList"].as_array().cloned().unwrap_or_default();
        let n = page.len();
        out.extend(page);
        if n < 100 {
            break;
        }
        offset += 100;
    }
    Ok(out)
}

pub fn club_layout(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let club = f("--club").ok_or("club-layout needs --club ID")?;
    let folder_name = f("--folder").ok_or("club-layout needs --folder NAME")?;
    let word = f("--word").ok_or("club-layout needs --word X (the alteration word: Tiny, Wet Wood…)")?;
    let parts_dir = std::path::PathBuf::from(f("--parts-dir").ok_or("club-layout needs --parts-dir D")?);
    let outdir = std::path::PathBuf::from(f("--outdir").unwrap_or_else(|| "/home/vjeux/shoot/u10s".into()));
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let room_size: usize = f("--room-size").and_then(|s| s.parse().ok()).unwrap_or(100);
    let region = f("--region").unwrap_or_else(|| "eu-west".into());
    let max_players: u32 = f("--max-players").and_then(|s| s.parse().ok()).unwrap_or(100);
    let keep_part_rooms = tmmaps::cli::has(args, "--keep-part-rooms");
    let shootctl = f("--shootctl").unwrap_or_else(|| "/home/vjeux/trackmania-tas/tools/target/release/shootctl".into());
    let (_core, live) = tokens_opt(&shootctl, "club-layout", false)?;
    let auth = format!("Authorization: {live}");
    // the folder
    let acts = all_activities(&auth, &club, None)?;
    let found = acts
        .iter()
        .find(|a| a["activityType"] == "folder" && a["name"] == folder_name.as_str())
        .and_then(|a| a["id"].as_u64());
    // `--create-folder`: a new alteration's folder (Tarmac, 2026-10-25 — Everios96: "You didnt
    // put the tarmac campaigns and rooms into a folder") is made here, the way club-rooms makes it
    let fid = match found {
        Some(id) => id,
        None if tmmaps::cli::has(args, "--create-folder") => {
            let body = serde_json::json!({"name": folder_name, "folderId": 0}).to_string();
            let v = post_json(&auth, &format!("{LIVE}/api/token/club/{club}/folder/create"), &body)?;
            let id = v["id"].as_u64().or_else(|| v["targetActivityId"].as_u64()).ok_or_else(|| format!("folder create: no id in {v}"))?;
            println!("folder {folder_name:?}: created activity {id}");
            id
        }
        None => return Err(format!("folder {folder_name:?} not found in club {club} ({} activities); --create-folder makes it", acts.len())),
    };
    println!("folder {folder_name:?}: activity {fid}");
    // state
    let state_path = outdir.join(format!("club-{club}-layout-{}.tsv", folder_name.replace(' ', "_")));
    let mut state: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new(); // "camp:NN" -> "campId\tactId", "room:K" -> id
    for l in std::fs::read_to_string(&state_path).unwrap_or_default().lines() {
        if let Some((k, v)) = l.split_once('\t') {
            state.insert(k.to_string(), v.to_string());
        }
    }
    let save = |state: &std::collections::BTreeMap<String, String>| {
        let s: String = state.iter().map(|(k, v)| format!("{k}\t{v}\n")).collect();
        let _ = std::fs::write(&state_path, s);
    };
    // the uids, part by part, in order
    let mut parts: Vec<(String, Vec<String>)> = Vec::new();
    let mut names: Vec<String> = std::fs::read_dir(&parts_dir).map_err(|e| format!("{}: {e}", parts_dir.display()))?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("results-p") && n.ends_with(".tsv")).collect();
    names.sort();
    for n in &names {
        let part = n.trim_start_matches("results-p").trim_end_matches(".tsv").to_string();
        let uids: Vec<String> = std::fs::read_to_string(parts_dir.join(n))
            .unwrap_or_default()
            .lines()
            .skip(1)
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                (c.len() >= 8 && c[7] == "IDENTICAL" && c[2] != "-").then(|| c[2].to_string())
            })
            .collect();
        if !uids.is_empty() {
            parts.push((part, uids));
        }
    }
    let all: Vec<String> = parts.iter().flat_map(|(_, u)| u.iter().cloned()).collect();
    println!("{} parts, {} maps", parts.len(), all.len());
    let mut failed = 0usize;
    // 1. campaigns: one per part, inside the folder
    println!("campaign	id	activity	maps	verdict");
    for (part, uids) in &parts {
        let name = format!("{word} Part {part}");
        let key = format!("camp:{part}");
        let (camp, act) = match state.get(&key).and_then(|v| v.split_once('\t').map(|(a, b)| (a.to_string(), b.to_string()))) {
            Some(x) => x,
            None => match campaign_create(&auth, &club, &name) {
                Ok((c, a)) => {
                    state.insert(key.clone(), format!("{c}\t{a}"));
                    save(&state);
                    (c, a)
                }
                Err(e) => {
                    println!("{name}\t-\t-\t{}\tcreate FAILED: {}", uids.len(), e.lines().next().unwrap_or("").chars().take(160).collect::<String>());
                    failed += 1;
                    continue;
                }
            },
        };
        let n = match campaign_set(&auth, &club, &camp, &name, uids) {
            Ok(v) => serde_json::to_string(&v).unwrap_or_default().matches("\"mapUid\"").count(),
            Err(e) => {
                println!("{name}\t{camp}\t{act}\t{}\tplaylist FAILED: {}", uids.len(), e.lines().next().unwrap_or("").chars().take(160).collect::<String>());
                failed += 1;
                continue;
            }
        };
        // into the folder (position: part order)
        if act != "-" {
            let body = serde_json::json!({"folderId": fid}).to_string();
            if let Err(e) = post_json(&auth, &format!("{LIVE}/api/token/club/{club}/activity/{act}/edit"), &body) {
                println!("{name}\t{camp}\t{act}\t{n}\tinto folder FAILED: {}", e.lines().next().unwrap_or("").chars().take(160).collect::<String>());
                failed += 1;
                continue;
            }
        }
        println!("{name}\t{camp}\t{act}\t{n}\t{}", if n == uids.len() { "OK" } else { "COUNT MISMATCH" });
        if n != uids.len() {
            failed += 1;
        }
    }
    // 2. rooms of `room_size` maps
    let settings = serde_json::json!([
        {"key": "S_TimeLimit", "value": "", "type": "integer"},
        {"key": "S_DecoImageUrl_WhoAmIUrl", "value": format!("/api/club/{club}"), "type": "text"},
        {"key": "S_ForceLapsNb", "value": "-1", "type": "integer"}
    ]);
    println!("room\tid\tmaps\tverdict");
    for (k, chunk) in all.chunks(room_size).enumerate() {
        let a = k * room_size + 1;
        let b = k * room_size + chunk.len();
        let name = format!("{word} V{} {a:02}-{b}", k + 1);
        let key = format!("room:{}", k + 1);
        let mut body = serde_json::json!({
            "name": name, "region": region, "maxPlayersPerServer": max_players,
            "script": "TrackMania/TM_TimeAttack_Online.Script.txt", "settings": settings,
            "maps": chunk, "scalable": 0, "password": 0, "shufflePlaylist": 0, "folderId": fid
        });
        let r = match state.get(&key).and_then(|v| v.parse::<u64>().ok()) {
            Some(id) => {
                body.as_object_mut().unwrap().remove("folderId");
                post_json(&auth, &format!("{LIVE}/api/token/club/{club}/room/{id}/edit"), &body.to_string()).map(|v| (id, v))
            }
            None => post_json(&auth, &format!("{LIVE}/api/token/club/{club}/room/create"), &body.to_string()).map(|v| (v["activityId"].as_u64().or_else(|| v["id"].as_u64()).unwrap_or(0), v)),
        };
        match r {
            Ok((id, v)) => {
                let n = v["room"]["maps"].as_array().map(|a| a.len()).unwrap_or(0);
                state.insert(key, id.to_string());
                save(&state);
                // a new room is private, like Everios wants them
                let _ = post_json(&auth, &format!("{LIVE}/api/token/club/{club}/activity/{id}/edit"), &serde_json::json!({"public": false}).to_string());
                println!("{name}\t{id}\t{n}\t{}", if n == chunk.len() { "OK" } else { "COUNT MISMATCH" });
                if n != chunk.len() {
                    failed += 1;
                }
            }
            Err(e) => {
                println!("{name}\t-\t{}\tFAILED: {}", chunk.len(), e.lines().next().unwrap_or("").chars().take(200).collect::<String>());
                failed += 1;
            }
        }
    }
    // 3. the old per-part rooms of the folder
    if !keep_part_rooms && failed == 0 {
        // THIS folder's rooms only: the listing's parentActivityId filter is not
        // honoured past the first page (2026-09-14 16:24Z: the Tiny run deleted
        // 39 Magnet + 17 Dirt rooms) — the activity's own folderId decides, and
        // the room's name must start with the alteration word
        let acts = all_activities(&auth, &club, None)?;
        let mut deleted = 0usize;
        for a in acts {
            if a["activityType"] != "room" || a["folderId"].as_u64() != Some(fid) {
                continue;
            }
            let n = a["name"].as_str().unwrap_or("");
            if n.contains(" - Part ") && n.starts_with(word.as_str()) {
                if let Some(id) = a["id"].as_u64() {
                    // the delete answers an empty body: judge by the HTTP code only
                    match curl(&["-X", "POST", "-H", &auth, "-H", "Content-Type: application/json", "-d", "{}", &format!("{LIVE}/api/token/club/{club}/activity/{id}/delete")]) {
                        Ok((_, code)) if code.starts_with('2') => deleted += 1,
                        Ok((body, code)) => println!("delete {id} {n:?}: HTTP {code} {}", body.chars().take(120).collect::<String>()),
                        Err(e) => println!("delete {id} {n:?}: {}", e.lines().next().unwrap_or("").chars().take(120).collect::<String>()),
                    }
                }
            }
        }
        println!("old per-part rooms deleted: {deleted}");
    }
    if failed > 0 {
        return Err(format!("{failed} step(s) failed"));
    }
    Ok(())
}

/// `tinyctl nadeo-here club-rooms --club C --folder "Giant U10S" --room-prefix "Giant U10S - Part "
/// --parts-dir D [--only 01,02] [--region eu-west] [--max-players 100] [--outdir O]` —
/// Everios' hunt-club layout: ONE folder per alteration holding one Nadeo-hosted
/// TimeAttack ROOM per part ("Giant U10S - Part 05", 25 maps in campaign order).
/// Reads the part uids from `D/results-pNN.tsv` (publish-batch results, IDENTICAL
/// rows), creates the folder once (id remembered in `O/club-<C>-rooms.tsv`),
/// then creates or edits the room per part. No map bytes move.
pub fn club_rooms(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let club = f("--club").ok_or("club-rooms needs --club ID")?;
    let folder_name = f("--folder").ok_or("club-rooms needs --folder NAME")?;
    let prefix = f("--room-prefix").ok_or("club-rooms needs --room-prefix \"X U10S - Part \"")?;
    let parts_dir = std::path::PathBuf::from(f("--parts-dir").ok_or("club-rooms needs --parts-dir D")?);
    let outdir = std::path::PathBuf::from(f("--outdir").unwrap_or_else(|| "/home/vjeux/shoot/u10s".into()));
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let region = f("--region").unwrap_or_else(|| "eu-west".into());
    let max_players: u32 = f("--max-players").and_then(|s| s.parse().ok()).unwrap_or(100);
    let only: Option<Vec<String>> = f("--only").map(|s| s.split(',').map(|x| format!("{:02}", x.trim().parse::<usize>().unwrap_or(0))).collect());
    let shootctl = f("--shootctl").unwrap_or_else(|| "/home/vjeux/trackmania-tas/tools/target/release/shootctl".into());
    let (_core, live) = tokens_opt(&shootctl, "club-rooms", false)?;
    let auth = format!("Authorization: {live}");
    // state: folder id + part -> room activity id
    let state_path = outdir.join(format!("club-{club}-rooms-{}.tsv", folder_name.replace(' ', "_")));
    let mut folder_id: Option<u64> = None;
    let mut rooms: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    for l in std::fs::read_to_string(&state_path).unwrap_or_default().lines() {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() >= 2 {
            if c[0] == "folder" {
                folder_id = c[1].parse().ok();
            } else if let Ok(id) = c[1].parse() {
                rooms.insert(c[0].to_string(), id);
            }
        }
    }
    let save = |folder_id: Option<u64>, rooms: &std::collections::BTreeMap<String, u64>| {
        let mut s = String::new();
        if let Some(fid) = folder_id {
            s.push_str(&format!("folder\t{fid}\n"));
        }
        for (p, id) in rooms {
            s.push_str(&format!("{p}\t{id}\n"));
        }
        let _ = std::fs::write(&state_path, s);
    };
    // the folder: reuse an existing one of that name (Everios may have made it), else create
    if folder_id.is_none() {
        let acts = get_json(&auth, &format!("{LIVE}/api/token/club/{club}/activity?length=100&offset=0&active=true"))?;
        if let Some(a) = acts["activityList"].as_array().and_then(|l| l.iter().find(|a| a["activityType"] == "folder" && a["name"] == folder_name.as_str())) {
            folder_id = a["id"].as_u64();
            println!("folder {folder_name:?}: existing activity {}", folder_id.unwrap_or(0));
        }
    }
    if folder_id.is_none() {
        let body = serde_json::json!({"name": folder_name, "folderId": 0}).to_string();
        let v = post_json(&auth, &format!("{LIVE}/api/token/club/{club}/folder/create"), &body)?;
        folder_id = v["id"].as_u64().or_else(|| v["targetActivityId"].as_u64());
        println!("folder {folder_name:?}: created activity {}", folder_id.unwrap_or(0));
    }
    let fid = folder_id.ok_or("no folder id")?;
    save(folder_id, &rooms);
    // existing rooms in the folder by name (idempotent re-runs, or Everios' own)
    {
        let acts = get_json(&auth, &format!("{LIVE}/api/token/club/{club}/activity?length=100&offset=0&active=true&parentActivityId={fid}"))?;
        for a in acts["activityList"].as_array().cloned().unwrap_or_default() {
            if a["activityType"] == "room" {
                if let (Some(n), Some(id)) = (a["name"].as_str(), a["id"].as_u64()) {
                    if let Some(p) = n.strip_prefix(prefix.as_str()) {
                        rooms.entry(format!("{:02}", p.trim().parse::<usize>().unwrap_or(0))).or_insert(id);
                    }
                }
            }
        }
    }
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
    let settings = serde_json::json!([
        {"key": "S_TimeLimit", "value": "", "type": "integer"},
        {"key": "S_DecoImageUrl_WhoAmIUrl", "value": format!("/api/club/{club}"), "type": "text"},
        {"key": "S_ForceLapsNb", "value": "-1", "type": "integer"}
    ]);
    println!("part\troom\tmaps\tverdict");
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
            println!("{part}\t-\t0\tno IDENTICAL rows");
            failed += 1;
            continue;
        }
        let name = format!("{prefix}{part}");
        let mut body = serde_json::json!({
            "name": name, "region": region, "maxPlayersPerServer": max_players,
            "script": "TrackMania/TM_TimeAttack_Online.Script.txt", "settings": settings,
            "maps": uids, "scalable": 0, "password": 0, "shufflePlaylist": 0, "folderId": fid
        });
        let r = match rooms.get(part) {
            Some(id) => {
                body.as_object_mut().unwrap().remove("folderId");
                post_json(&auth, &format!("{LIVE}/api/token/club/{club}/room/{id}/edit"), &body.to_string()).map(|v| (*id, v))
            }
            None => post_json(&auth, &format!("{LIVE}/api/token/club/{club}/room/create"), &body.to_string()).map(|v| (v["activityId"].as_u64().or_else(|| v["id"].as_u64()).unwrap_or(0), v)),
        };
        match r {
            Ok((id, v)) => {
                let n = v["room"]["maps"].as_array().map(|a| a.len()).unwrap_or(0);
                rooms.insert(part.clone(), id);
                save(folder_id, &rooms);
                println!("{part}\t{id}\t{n}\t{}", if n == uids.len() { "OK" } else { "COUNT MISMATCH" });
                if n != uids.len() {
                    failed += 1;
                }
            }
            Err(e) => {
                println!("{part}\t-\t{}\tFAILED: {}", uids.len(), e.lines().next().unwrap_or("").chars().take(200).collect::<String>());
                failed += 1;
            }
        }
    }
    if failed > 0 {
        return Err(format!("{failed} of {} parts failed", parts.len()));
    }
    Ok(())
}
