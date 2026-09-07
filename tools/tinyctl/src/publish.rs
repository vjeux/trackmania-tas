//! Publishing a tiny map to Nadeo Services and the club campaign — the
//! `nadeo-publish.sh` / `nadeo-update.sh` / `nadeo-playcheck.sh` chain as one
//! program, in two halves:
//!
//! - `tinyctl publish-here …` runs ON THE RENDER BOX (WSL): it gets the game's
//!   own Nadeo tokens through the GhostShooter `/nadeotoken` route (Ubisoft's
//!   password login answers 403 for this account), uploads the map — an UPDATE
//!   of the existing record when the uid is already known, a create otherwise
//!   (a second create with the same uid keeps the old collectionName) — puts
//!   the uid into the campaign playlist at the wanted position, reads the
//!   record back, downloads the stored file and compares its md5 with the
//!   local bytes, and optionally plays the stored copy (`--playcheck`) and
//!   screenshots it. HTTP is `curl` (present on the box) run as a subprocess.
//! - `tinyctl publish-map NN --map TINY.Map.Gbx …` runs on the devserver: the
//!   gates (item-check over the items directory when given; a sane header),
//!   the push, `publish-here --detach` over the bridge, the wait, the report.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::wsx::Wsx;

const CORE: &str = "https://prod.trackmania.core.nadeo.online";
const LIVE: &str = "https://live-services.trackmania.nadeo.live";
const STORE: &str = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";
const STAGE: &str = "/home/vjeux/shoot/_stage";
const POWERSHELL: &str = "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe";
pub const DEFAULT_CLUB: &str = "43788";
pub const DEFAULT_CAMPAIGN: &str = "155555";
pub const DEFAULT_CAMPAIGN_NAME: &str = "Tiny Campaign";

/// First `"key":"value"` in a JSON text — the records here are flat enough.
pub fn json_str(body: &str, key: &str) -> Option<String> {
    let k = format!("\"{key}\":\"");
    let i = body.find(&k)? + k.len();
    let rest = &body[i..];
    let e = rest.find('"')?;
    Some(rest[..e].to_string())
}
pub fn json_strs(body: &str, key: &str) -> Vec<String> {
    let k = format!("\"{key}\":\"");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = body[from..].find(&k) {
        let s = from + i + k.len();
        match body[s..].find('"') {
            Some(e) => {
                out.push(body[s..s + e].to_string());
                from = s + e;
            }
            None => break,
        }
    }
    out
}

fn curl(args: &[&str]) -> Result<(String, String), String> {
    let out = Command::new("curl").arg("-s").arg("-w").arg("\n%{http_code}").args(args).output().map_err(|e| format!("curl: {e}"))?;
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

fn token(shootctl: &str, aud: &str) -> Result<String, String> {
    let st = Command::new(shootctl).arg("get").arg(format!("/nadeotoken?aud={aud}")).output().map_err(|e| format!("{shootctl}: {e}"))?;
    if !st.status.success() {
        return Err(format!("/nadeotoken?aud={aud}: {}", String::from_utf8_lossy(&st.stderr).trim()));
    }
    // the plugin writes the Authorization header value to a file
    let p = format!("{STORE}/token-{aud}.txt");
    let t0 = Instant::now();
    loop {
        if let Ok(t) = std::fs::read_to_string(&p) {
            let t = t.trim().to_string();
            if t.len() > 20 {
                return Ok(t);
            }
        }
        if t0.elapsed() > Duration::from_secs(20) {
            return Err(format!("{p}: no token written by the plugin — is the game up and logged in?"));
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}

pub struct PublishOpts {
    pub map: PathBuf,
    pub name: String,
    pub club: String,
    pub campaign: String,
    pub campaign_name: String,
    pub position: Option<usize>,
    pub playcheck: bool,
    pub outdir: PathBuf,
    pub shootctl: String,
}

pub fn publish_here_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let opts = PublishOpts {
        map: PathBuf::from(f("--map").ok_or("publish-here needs --map MAP.Map.Gbx")?),
        name: f("--name").ok_or("publish-here needs --name \"Tiny Summer 2026 - NN\"")?,
        club: f("--club").unwrap_or_else(|| DEFAULT_CLUB.into()),
        campaign: f("--campaign").unwrap_or_else(|| DEFAULT_CAMPAIGN.into()),
        campaign_name: f("--campaign-name").unwrap_or_else(|| DEFAULT_CAMPAIGN_NAME.into()),
        position: f("--position").map(|s| s.parse::<usize>().map_err(|_| "--position number")).transpose()?,
        playcheck: tmmaps::cli::has(args, "--playcheck"),
        outdir: PathBuf::from(f("--outdir").unwrap_or_else(|| "/mnt/c/Users/vjeux/tinyshots/publish".into())),
        shootctl: f("--shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl")),
    };
    std::fs::create_dir_all(&opts.outdir).map_err(|e| format!("{}: {e}", opts.outdir.display()))?;
    let done = opts.outdir.join("done-publish.txt");
    let _ = std::fs::remove_file(&done);
    if tmmaps::cli::has(args, "--detach") {
        return detach(&opts.outdir);
    }
    let t0 = Instant::now();
    // One game, several drivers: the token route and the playcheck talk to
    // the running game, which answers nothing while another thread's shootset
    // or playshots has it loading a map (Summer 19's first publish died on
    // `/nadeotoken: Resource temporarily unavailable`; 17's and 18's
    // playchecks waited out their 600 s). Hold the same render lock those
    // take — the upload itself is Nadeo-side, but it is short next to a wait.
    let owner = format!("publish-{}", o_stem(&opts.map));
    let res = match render_lock(&opts.shootctl, &owner, "acquire", &["--wait", "900"]) {
        Ok(()) => {
            let r = publish_here(&opts);
            let _ = render_lock(&opts.shootctl, &owner, "release", &[]);
            r
        }
        Err(e) => Err(format!("render lock: {e}")),
    };
    let summary = match &res {
        Ok(lines) => format!("OK in {:.0}s\n{}\n", t0.elapsed().as_secs_f64(), lines.join("\n")),
        Err(e) => format!("FAILED after {:.0}s: {e}\n", t0.elapsed().as_secs_f64()),
    };
    print!("{summary}");
    let tmp = opts.outdir.join("done-publish.tmp");
    std::fs::write(&tmp, &summary).and_then(|_| std::fs::rename(&tmp, &done)).map_err(|e| format!("{}: {e}", done.display()))?;
    res.map(|_| ())
}

/// `shootctl lock acquire|release --owner WHO [--wait S]` — the render box's
/// one-driver lock (a directory beside the game; shootset and playshots take
/// the same one). Through the CLI rather than a crate link: the box builds
/// shootctl and tinyctl side by side, and the lock's home is shootctl's.
fn render_lock(shootctl: &str, owner: &str, verb: &str, extra: &[&str]) -> Result<(), String> {
    let out = Command::new(shootctl).arg("lock").arg(verb).arg("--owner").arg(owner).args(extra).output().map_err(|e| format!("{shootctl}: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if let Some(l) = text.lines().find(|l| l.contains("render lock")) {
        println!("{l}");
    }
    if out.status.success() {
        Ok(())
    } else {
        Err(text.trim().to_string())
    }
}

/// `Tiny19` for `…/Tiny19.Map.Gbx` — the lock owner name.
fn o_stem(map: &Path) -> String {
    map.file_name().and_then(|s| s.to_str()).map(|s| s.split('.').next().unwrap_or(s).to_string()).unwrap_or_else(|| "map".into())
}

fn detach(outdir: &Path) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--detach").collect();
    let log = outdir.join("publish.log");
    let out = std::fs::File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let child = Command::new(exe).args(&args).stdin(std::process::Stdio::null()).stdout(out).stderr(err).process_group(0).spawn().map_err(|e| format!("spawn: {e}"))?;
    println!("detached pid {} — log {} — done file {}", child.id(), log.display(), outdir.join("done-publish.txt").display());
    Ok(())
}

fn publish_here(o: &PublishOpts) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let map = o.map.to_str().ok_or("map path is not utf-8")?;
    let hdr = tmmaps::header::read(map)?;
    let uid = hdr.uid.clone();
    if uid == "-" || uid.is_empty() {
        return Err("the map header has no uid".into());
    }
    let num = |s: &str, what: &str| s.parse::<i64>().map_err(|_| format!("header {what} `{s}` is not a number"));
    let (at, gold, silver, bronze) = (num(&hdr.authortime, "authortime")?, num(&hdr.gold, "gold")?, num(&hdr.silver, "silver")?, num(&hdr.bronze, "bronze")?);
    let envir = if hdr.envir == "-" { "Stadium".to_string() } else { hdr.envir.clone() };
    let md5_local = md5_hex(&std::fs::read(&o.map).map_err(|e| format!("{map}: {e}"))?);
    lines.push(format!("map\t{map}\tuid {uid}\tenvir {envir}\tAT {at}\tmd5 {md5_local}"));

    let core = token(&o.shootctl, "NadeoServices")?;
    let live = token(&o.shootctl, "NadeoLiveServices")?;
    let auth_core = format!("Authorization: {core}");
    let auth_live = format!("Authorization: {live}");

    // the existing record, if any
    let (rec, code) = curl(&["-H", &auth_core, &format!("{CORE}/maps/?mapUidList={uid}")])?;
    if code != "200" {
        return Err(format!("GET /maps/?mapUidList: HTTP {code} {}", &rec[..rec.len().min(300)]));
    }
    let existing = json_str(&rec, "mapId");
    let me = match json_str(&rec, "author") {
        Some(a) => a,
        None => {
            let (mine, c) = curl(&["-H", &auth_live, &format!("{LIVE}/api/token/club/mine?length=1&offset=0")])?;
            json_str(&mine, "authorAccountId").ok_or_else(|| format!("club/mine: HTTP {c} without authorAccountId: {}", &mine[..mine.len().min(200)]))?
        }
    };
    let params = format!(
        "{{\"isPlayable\":true,\"author\":\"{me}\",\"authorScore\":{at},\"bronzeScore\":{bronze},\"silverScore\":{silver},\"goldScore\":{gold},\"collectionName\":\"{envir}\",\"mapStyle\":\"\",\"mapType\":\"TrackMania\\\\TM_Race\",\"name\":\"{}\",\"mapUid\":\"{uid}\"}}",
        o.name.replace('"', "\\\"")
    );
    let route = match &existing {
        Some(id) => format!("{CORE}/maps/{id}"),
        None => format!("{CORE}/maps/"),
    };
    let (up, code) = curl(&[
        "-H",
        &auth_core,
        "-F",
        &format!("nadeoservices-core-parameters={params};type=application/json"),
        "-F",
        &format!("data=@{map};type=application/octet-stream;filename={}.Map.Gbx", o.name),
        &route,
    ])?;
    if !code.starts_with('2') {
        return Err(format!("upload {}: HTTP {code} {}", if existing.is_some() { "(update)" } else { "(create)" }, &up[..up.len().min(400)]));
    }
    let map_id = json_str(&up, "mapId").or(existing.clone()).unwrap_or_default();
    lines.push(format!("upload\t{}\tmapId {map_id}\tas {me}\tHTTP {code}", if existing.is_some() { "update" } else { "create" }));

    // campaign playlist
    let (camp, code) = curl(&["-H", &auth_live, &format!("{LIVE}/api/token/club/{}/campaign/{}", o.club, o.campaign)])?;
    if code != "200" {
        return Err(format!("GET campaign: HTTP {code} {}", &camp[..camp.len().min(300)]));
    }
    let mut uids: Vec<String> = json_strs(&camp, "mapUid").into_iter().filter(|u| *u != uid).collect();
    let pos = o.position.unwrap_or(uids.len()).min(uids.len());
    uids.insert(pos, uid.clone());
    let playlist: Vec<String> = uids.iter().enumerate().map(|(i, u)| format!("{{\"mapUid\":\"{u}\",\"position\":{i}}}")).collect();
    let body = format!("{{\"name\":\"{}\",\"playlist\":[{}]}}", o.campaign_name.replace('"', "\\\""), playlist.join(","));
    let (ed, code) = curl(&["-X", "POST", "-H", &auth_live, "-H", "Content-Type: application/json", "-d", &body, &format!("{LIVE}/api/token/club/{}/campaign/{}/edit", o.club, o.campaign)])?;
    if !code.starts_with('2') {
        return Err(format!("campaign edit: HTTP {code} {}", &ed[..ed.len().min(400)]));
    }
    let in_list = json_strs(&ed, "mapUid").iter().position(|u| *u == uid);
    lines.push(format!("campaign\t{}\t{} maps\tours at position {:?}", o.campaign, json_strs(&ed, "mapUid").len(), in_list));

    // read back: the stored bytes must be ours
    let (rec2, _) = curl(&["-H", &auth_core, &format!("{CORE}/maps/?mapUidList={uid}")])?;
    let file_url = json_str(&rec2, "fileUrl").ok_or("record has no fileUrl after upload")?;
    let back = o.outdir.join(format!("readback-{uid}.Map.Gbx"));
    let (_, code) = curl(&["-L", "-H", &auth_core, "-o", back.to_str().unwrap(), &file_url])?;
    let stored = std::fs::read(&back).map_err(|e| format!("{}: {e}", back.display()))?;
    let md5_stored = md5_hex(&stored);
    lines.push(format!("stored\tHTTP {code}\t{} bytes\tmd5 {md5_stored}\tcollection {}\t{}", stored.len(), json_str(&rec2, "collectionName").unwrap_or_default(), if md5_stored == md5_local { "IDENTICAL" } else { "⚠ DIFFERS FROM LOCAL" }));
    if md5_stored != md5_local {
        return Err(format!("the stored file's md5 {md5_stored} is not the local {md5_local}"));
    }

    if o.playcheck {
        // The upload is done and verified by now: a playcheck that fails
        // (Summer 05 takes ~7 min to load, longer than the wait was) must not
        // hide the upload/campaign/stored lines — it becomes a line of its own.
        match playcheck(o, &auth_core, &file_url, &uid) {
            Ok(line) => lines.push(line),
            Err(e) => lines.push(format!("playcheck\tFAILED\t{e}")),
        }
    }
    Ok(lines)
}

/// Play the copy Nadeo stores (PlayMap on the signed CDN url the core /file
/// route redirects to), wait for the playground, screenshot it.
fn playcheck(o: &PublishOpts, auth_core: &str, file_url: &str, uid: &str) -> Result<String, String> {
    let (cdn, _) = curl(&["-o", "/dev/null", "-w", "%{redirect_url}", "-H", auth_core, file_url])?;
    let cdn = cdn.lines().next().unwrap_or("").trim().to_string();
    if !cdn.starts_with("http") {
        return Err(format!("no CDN redirect for {file_url}: `{cdn}`"));
    }
    let get = |route: &str| -> String {
        Command::new(&o.shootctl).arg("get").arg(route).output().map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string()).unwrap_or_default()
    };
    // back to the menu
    for _ in 0..10 {
        let c = get("/ctx");
        if c.contains("\"ctx\":0") && c.contains("\"dialog\":null") {
            break;
        }
        if c.contains("\"dialog\":null") {
            get("/back");
        } else {
            get("/dismiss");
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    std::fs::write(format!("{STORE}/editmap.txt"), &cdn).map_err(|e| format!("editmap.txt: {e}"))?;
    let pm = get("/playmap?mode=");
    let t0 = Instant::now();
    let mut ctx = String::new();
    // the tiny 05 needs ~7 min from PlayMap to the playground; the others < 1 min
    while t0.elapsed() < Duration::from_secs(600) {
        std::thread::sleep(Duration::from_secs(3));
        ctx = get("/ctx");
        if ctx.contains("\"playground\":true") {
            break;
        }
    }
    if !ctx.contains("\"playground\":true") {
        return Err(format!("playcheck: no playground after {:.0}s; playmap said `{pm}`; ctx {ctx}", t0.elapsed().as_secs_f64()));
    }
    std::thread::sleep(Duration::from_secs(8));
    let shot = o.outdir.join(format!("playcheck-{uid}.png"));
    let win = crate::wsx::to_win(shot.to_str().unwrap()).replace('/', "\\");
    let st = Command::new(POWERSHELL).args(["-ExecutionPolicy", "Bypass", "-File", "C:\\Users\\vjeux\\shotdpi.ps1", &win]).output().map_err(|e| format!("powershell: {e}"))?;
    let size = std::fs::metadata(&shot).map(|m| m.len()).unwrap_or(0);
    get("/back");
    Ok(format!("playcheck\tplayground after {:.0}s\tscreenshot {} ({size} B, powershell {})", t0.elapsed().as_secs_f64(), shot.display(), st.status))
}

// ---------------------------------------------------------------- devserver

pub fn publish_map_cmd(args: &[String]) -> Result<(), String> {
    let nn = args.get(0).ok_or("publish-map needs NN (the map number, 01..25)")?;
    let n: usize = nn.parse().map_err(|_| format!("`{nn}` is not a map number"))?;
    if !(1..=25).contains(&n) {
        return Err(format!("map number {n} is not in 01..25"));
    }
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let map = PathBuf::from(f("--map").ok_or("publish-map needs --map TINY.Map.Gbx")?);
    let name = f("--name").unwrap_or_else(|| format!("Tiny Summer 2026 - {n:02}"));
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| "/tmp/tiny3".into()));
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let hdr = tmmaps::header::read(map.to_str().ok_or("map path")?)?;
    println!("map {}: name {} uid {} envir {} AT {} validated {} ({} items, {} zip bytes)", map.display(), hdr.name, hdr.uid, hdr.envir, hdr.authortime, hdr.validated, hdr.items, hdr.zip_bytes);
    if hdr.uid == "-" || hdr.authortime == "-" || hdr.envir == "-" {
        return Err("the header lacks uid / authortime / envir — not publishable".into());
    }
    if !hdr.uid.starts_with("Tin") && !tmmaps::cli::has(args, "--any-uid") {
        return Err(format!("uid {} does not start with `Tin` — is this the tiny map? (--any-uid to publish anyway)", hdr.uid));
    }
    // gate: item-check over the library items when given (in-process: the
    // same code `mapgeom item-check` runs; prints one line per item)
    if let Some(dir) = f("--items-dir") {
        let mut items: Vec<String> = std::fs::read_dir(&dir).map_err(|e| format!("{dir}: {e}"))?.filter_map(|e| e.ok()).map(|e| e.path().to_string_lossy().into_owned()).filter(|p| p.ends_with(".Item.Gbx")).collect();
        items.sort();
        if items.is_empty() {
            return Err(format!("{dir}: no .Item.Gbx files"));
        }
        let mut rest: Vec<String> = vec!["item-check".into()];
        rest.extend(items.iter().cloned());
        // the material-link rule needs the packs: `--paks "--pak F:KEY …"`
        // like probe (an empty store fails every link as "no .Material.Gbx")
        let paks = f("--paks").ok_or("publish-map --items-dir needs --paks \"--pak FILE:KEY …\" so item-check can resolve the material links (or drop --items-dir)")?;
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
        match mapgeom::static_item::check::run(&rest, &mut open) {
            Ok(()) => println!("item-check: {} items ok", items.len()),
            Err(e) => return Err(format!("item-check refused the library ({e}) — not publishing")),
        }
    } else {
        println!("item-check: skipped (pass --items-dir DIR with the map's baked items to gate on the format rules)");
    }
    let wsx = Wsx::new(args);
    let remote = format!("{STAGE}/Tiny{n:02}.Map.Gbx");
    eprintln!("pushing {} → box {remote} …", map.display());
    wsx.push(&map, &remote)?;
    let tinyctl = f("--box-tinyctl").unwrap_or_else(|| format!("{BOX_TOOLS}/tinyctl"));
    let remote_out = format!("/mnt/c/Users/vjeux/tinyshots/publish-{n:02}");
    let mut cmd = format!("{tinyctl} publish-here --detach --map {remote} --name '{}' --club {} --campaign {} --campaign-name '{}' --outdir {remote_out} --position {}", name.replace('\'', ""), f("--club").unwrap_or_else(|| DEFAULT_CLUB.into()), f("--campaign").unwrap_or_else(|| DEFAULT_CAMPAIGN.into()), f("--campaign-name").unwrap_or_else(|| DEFAULT_CAMPAIGN_NAME.into()).replace('\'', ""), f("--position").unwrap_or_else(|| (n - 1).to_string()));
    if tmmaps::cli::has(args, "--playcheck") {
        cmd.push_str(" --playcheck");
    }
    let started = wsx.sh(&cmd)?;
    if wsx.verbose {
        eprintln!("{}", started.trim());
    }
    let done = wsx.wait_done(&format!("{remote_out}/done-publish.txt"), &format!("{remote_out}/publish.log"), Duration::from_secs(1800), "publish")?;
    println!("{}", done.trim());
    if tmmaps::cli::has(args, "--playcheck") {
        let shot = format!("playcheck-{}.png", hdr.uid);
        match wsx.pull(&format!("{remote_out}/{shot}"), &outdir.join(&shot)) {
            Ok(n) => println!("playcheck screenshot: {} ({n} B)", outdir.join(&shot).display()),
            Err(e) => eprintln!("playcheck screenshot: {e}"),
        }
    }
    Ok(())
}

/// Lowercase hex md5 — what `md5sum` prints, so the numbers here can be
/// checked against a shell's by eye.
fn md5_hex(data: &[u8]) -> String {
    mapgeom::md5::md5(data).iter().map(|b| format!("{b:02x}")).collect()
}
