//! `tmsite refresh` / `tmsite records` — the live human leaderboards, beside
//! what this repo publishes about them.
//!
//! Two commands, deliberately separate so the table can be rebuilt without
//! touching the network again:
//!
//! * `refresh` fetches and **banks raw responses**, one file per request, plus
//!   a log line per request with its HTTP status and byte count. Nothing is
//!   interpreted at fetch time.
//! * `records` reads the bank, reads the pages, and writes the table. It never
//!   reaches the network, so a table can always be re-derived from what was
//!   banked.
//!
//! ## Where the numbers come from
//!
//! Nadeo's own live services want a Ubisoft session ticket, which this project
//! does not hold. The two public mirrors of the same board are used instead,
//! and they are *not* collapsed into one number:
//!
//! | source | endpoint | gives |
//! |---|---|---|
//! | trackmania.io | `/api/map/<uid>`, `/api/leaderboard/map/<uid>` | author score, top 15, holder, timestamp, total record count |
//! | trackmania.exchange | `/api/maps?id=…&fields=…` | TMX id → uid, author medal, its own WR mirror and record count |
//!
//! Disagreement between the two is reported, never averaged or silently
//! preferred.
//!
//! ## An empty board is not an answer on its own
//!
//! Several maps here genuinely have zero human records, and a broken fetch
//! looks exactly like one. So every empty board is only called empty when
//! **other maps in the same pass came back populated** — the positive control
//! is counted and printed with the result, and a map whose fetch did not
//! return 200 is `UNKNOWN`, never "no records".
//!
//! Nothing here writes to a leaderboard. Every request is a GET.

use crate::json::parse;
use crate::tick::secs;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Named absolutely, and run with a cleared environment: a `curl` off PATH
/// could be a wrapper carrying a cookie jar, and these requests are meant to
/// be exactly what a logged-out visitor gets.
const CURL: &str = "/usr/bin/curl";

pub const DEFAULT_UA: &str =
    "tmtas-research/1.0 (vjeux TAS research / github.com/vjeux/trackmania-tas)";

// ---------------------------------------------------------------------------
// the corpus: <id>-<slug>/ directories at the repo root
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct MapDir {
    /// The leading digits of the directory name: a trackmania.exchange map id.
    pub tmx_id: i64,
    pub dir: String,
}

/// Every `<digits>-<slug>` directory under `root`, ordered by id.
pub fn map_dirs(root: &str) -> Result<Vec<MapDir>, String> {
    let rd = std::fs::read_dir(root).map_err(|e| format!("read {}: {}", root, e))?;
    let mut out = Vec::new();
    for ent in rd {
        let ent = ent.map_err(|e| format!("read {}: {}", root, e))?;
        if !ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = ent.file_name().to_string_lossy().into_owned();
        let Some((head, _)) = name.split_once('-') else { continue };
        let Ok(id) = head.parse::<i64>() else { continue };
        out.push(MapDir { tmx_id: id, dir: name });
    }
    out.sort_by_key(|m| m.tmx_id);
    Ok(out)
}

// ---------------------------------------------------------------------------
// fetching
// ---------------------------------------------------------------------------

pub struct Fetch {
    pub root: String,
    pub bank: String,
    /// Egress proxy, passed to curl explicitly because the environment is
    /// cleared. Empty string means a direct connection.
    pub proxy: String,
    pub sleep_ms: u64,
    pub ua: String,
}

/// One banked request.
struct Hit {
    id: i64,
    kind: &'static str,
    url: String,
    http: String,
    bytes: u64,
}

fn curl(o: &Fetch, url: &str, out: &Path) -> Result<String, String> {
    let mut args: Vec<String> = vec![
        "-sS".into(),
        "-L".into(),
        "--max-time".into(),
        "60".into(),
        "-A".into(),
        o.ua.clone(),
        "-o".into(),
        out.to_string_lossy().into_owned(),
        "-w".into(),
        "%{http_code}".into(),
    ];
    if !o.proxy.is_empty() {
        args.push("--proxy".into());
        args.push(o.proxy.clone());
    }
    args.push(url.into());
    let r = Command::new(CURL)
        .env_clear()
        .args(&args)
        .output()
        .map_err(|e| format!("cannot run {}: {}", CURL, e))?;
    if !r.status.success() && r.stdout.is_empty() {
        return Err(format!(
            "curl failed for {}: {}",
            url,
            String::from_utf8_lossy(&r.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&r.stdout).trim().to_string())
}

fn filesize(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

/// A GET that backs off once on 429, the way the public APIs ask to be used.
fn get(o: &Fetch, id: i64, kind: &'static str, url: &str, out: &Path) -> Result<Hit, String> {
    let mut http = curl(o, url, out)?;
    if http == "429" {
        eprintln!("refresh: 429 on {} — backing off 30 s", url);
        std::thread::sleep(std::time::Duration::from_secs(30));
        http = curl(o, url, out)?;
    }
    Ok(Hit { id, kind, url: url.to_string(), http, bytes: filesize(out) })
}

fn bank_path(bank: &str, rel: &str) -> PathBuf {
    Path::new(bank).join(rel)
}

fn mkdir(p: &Path) -> Result<(), String> {
    std::fs::create_dir_all(p).map_err(|e| format!("mkdir {}: {}", p.display(), e))
}

/// Fetch every map's board and bank the raw bytes. Returns a one-line summary.
pub fn refresh(o: &Fetch) -> Result<String, String> {
    let maps = map_dirs(&o.root)?;
    if maps.is_empty() {
        return Err(format!("no <id>-<slug> map directories under {}", o.root));
    }
    mkdir(&bank_path(&o.bank, "tmio_map"))?;
    mkdir(&bank_path(&o.bank, "tmio_lb"))?;
    let mut log: Vec<Hit> = Vec::new();

    // 1. TMX, in batches: the id -> uid resolution, and TMX's own WR mirror.
    let mut uid: BTreeMap<i64, String> = BTreeMap::new();
    for (n, chunk) in maps.chunks(20).enumerate() {
        let ids: Vec<String> = chunk.iter().map(|m| m.tmx_id.to_string()).collect();
        let url = format!(
            "https://trackmania.exchange/api/maps?id={}&fields={}",
            ids.join(","),
            "MapId,MapUid,Name,Medals.Author,OnlineWR,OnlineRecordCount,UpdatedAt"
        );
        let f = bank_path(&o.bank, &format!("tmx_{}.json", n));
        let hit = get(o, -1, "tmx", &url, &f)?;
        println!("tmx batch {} http {} {} bytes", n, hit.http, hit.bytes);
        log.push(hit);
        if let Ok(text) = std::fs::read_to_string(&f) {
            for (id, u) in tmx_uids(&text) {
                uid.insert(id, u);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(o.sleep_ms));
    }

    // 2. trackmania.io, one map at a time.
    for m in &maps {
        let Some(u) = uid.get(&m.tmx_id) else {
            println!("{} NO UID from TMX — no live board can be fetched", m.tmx_id);
            continue;
        };
        for (kind, url, rel) in [
            (
                "tmio_map",
                format!("https://trackmania.io/api/map/{}", u),
                format!("tmio_map/{}.json", m.tmx_id),
            ),
            (
                "tmio_lb",
                format!("https://trackmania.io/api/leaderboard/map/{}", u),
                format!("tmio_lb/{}.json", m.tmx_id),
            ),
        ] {
            let f = bank_path(&o.bank, &rel);
            let hit = get(o, m.tmx_id, if kind == "tmio_map" { "tmio_map" } else { "tmio_lb" }, &url, &f)?;
            println!("{} {} http {} {} bytes", m.tmx_id, kind, hit.http, hit.bytes);
            log.push(hit);
            std::thread::sleep(std::time::Duration::from_millis(o.sleep_ms));
        }
    }

    let mut text = String::from("mapid\tkind\thttp\tbytes\turl\n");
    for h in &log {
        text.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", h.id, h.kind, h.http, h.bytes, h.url));
    }
    let logf = bank_path(&o.bank, "fetch_log.tsv");
    std::fs::write(&logf, &text).map_err(|e| format!("write {}: {}", logf.display(), e))?;

    let bad = log.iter().filter(|h| h.http != "200").count();
    Ok(format!(
        "{} requests banked under {} ({} not 200), {} maps, {} uids resolved",
        log.len(),
        o.bank,
        bad,
        maps.len(),
        uid.len()
    ))
}

// ---------------------------------------------------------------------------
// reading the banked JSON
// ---------------------------------------------------------------------------

/// `MapId` -> `MapUid` out of one TMX batch response.
pub fn tmx_uids(text: &str) -> Vec<(i64, String)> {
    let Ok(v) = parse(text) else { return Vec::new() };
    let Some(rows) = v.get("Results").and_then(|r| r.as_arr()) else { return Vec::new() };
    let mut out = Vec::new();
    for r in rows {
        if let (Some(id), Some(u)) = (
            r.get("MapId").and_then(|x| x.as_i64()),
            r.get("MapUid").and_then(|x| x.as_str()),
        ) {
            out.push((id, u.to_string()));
        }
    }
    out
}

/// What TMX says about one map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tmx {
    pub uid: String,
    pub name: String,
    pub author_ms: Option<i64>,
    pub wr_ms: Option<i64>,
    pub wr_holder: Option<String>,
    pub records: Option<i64>,
}

pub fn tmx_rows(text: &str) -> Vec<(i64, Tmx)> {
    let Ok(v) = parse(text) else { return Vec::new() };
    let Some(rows) = v.get("Results").and_then(|r| r.as_arr()) else { return Vec::new() };
    let mut out = Vec::new();
    for r in rows {
        let Some(id) = r.get("MapId").and_then(|x| x.as_i64()) else { continue };
        let wr = r.get("OnlineWR");
        out.push((
            id,
            Tmx {
                uid: r.get("MapUid").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                name: r.get("Name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                author_ms: r.get("Medals").and_then(|m| m.get("Author")).and_then(|x| x.as_i64()),
                // TMX spells "no record" as a WR object of nulls with
                // RecordTime 0, which is an absence, not a 0.000 s run.
                wr_ms: wr.and_then(|w| w.get("RecordTime")).and_then(|x| x.as_i64()).filter(|v| *v > 0),
                wr_holder: wr
                    .and_then(|w| w.get("DisplayName"))
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                records: r.get("OnlineRecordCount").and_then(|x| x.as_i64()),
            },
        ));
    }
    out
}

/// One row of a live board.
#[derive(Clone, Debug, PartialEq)]
pub struct Top {
    pub position: i64,
    pub time_ms: i64,
    pub player: String,
    pub when: String,
    /// trackmania.io's own path to the stored replay, when it offers one.
    /// The one way to ask a surprising record what map it ran on.
    pub ghost: String,
}

/// What trackmania.io's leaderboard endpoint says.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    pub tops: Vec<Top>,
    /// Total number of records on the map, not the length of `tops`.
    pub records: Option<i64>,
}

pub fn board(text: &str) -> Result<Board, String> {
    let v = parse(text)?;
    let mut tops = Vec::new();
    if let Some(arr) = v.get("tops").and_then(|t| t.as_arr()) {
        for t in arr {
            let (Some(pos), Some(ms)) = (
                t.get("position").and_then(|x| x.as_i64()),
                t.get("time").and_then(|x| x.as_i64()),
            ) else {
                continue;
            };
            tops.push(Top {
                position: pos,
                time_ms: ms,
                player: t
                    .get("player")
                    .and_then(|p| p.get("name"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                when: t.get("timestamp").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                ghost: t.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            });
        }
    }
    tops.sort_by_key(|t| t.position);
    Ok(Board { tops, records: v.get("playercount").and_then(|x| x.as_i64()) })
}

/// The author score trackmania.io holds for the map, and the map's own name.
pub fn map_info(text: &str) -> Option<(String, Option<i64>)> {
    let v = parse(text).ok()?;
    Some((
        v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        v.get("authorScore").and_then(|x| x.as_i64()),
    ))
}

// ---------------------------------------------------------------------------
// reading the pages: what this repo currently claims
// ---------------------------------------------------------------------------

/// `12.345` (or `**12.345**`, or `` `12.345` ``) as milliseconds. A signed
/// token is a delta, not a time, and is refused.
pub fn page_ms(tok: &str) -> Option<i64> {
    let t = tok.trim().trim_matches(|c| c == '*' || c == '`' || c == ' ');
    if t.starts_with('-') || t.starts_with('+') || t.starts_with('\u{2212}') || t.starts_with('\u{b1}') {
        return None;
    }
    let t: String = t.chars().filter(|c| *c != ',').collect();
    let (a, f) = t.split_once('.')?;
    if f.len() != 3 || a.is_empty() || !a.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(a.parse::<i64>().ok()? * 1000 + f.parse::<i64>().ok()?)
}

/// `1,052` / `**1**` as a count. `—`, `-` and `*none*` are absence.
pub fn page_count(tok: &str) -> Option<i64> {
    let t: String = tok
        .trim()
        .trim_matches(|c| c == '*' || c == '`' || c == ' ')
        .chars()
        .filter(|c| *c != ',')
        .collect();
    t.parse::<i64>().ok()
}

/// A world-record claim as a page states it.
#[derive(Clone, Debug, PartialEq)]
pub struct WrClaim {
    pub wr_ms: Option<i64>,
    pub holder: Option<String>,
    /// The whole caption fragment, for quoting back at a reader.
    pub text: String,
}

/// The caption line every published clip carries:
/// `**Map** — TAS **22.072** (−1.253) | AT 23.325 | WR 23.298 by Lukrecja666`
pub fn caption_wr(line: &str) -> Option<WrClaim> {
    let at = line.find("| AT ")?;
    let wr_at = line[at..].find("| WR ")? + at;
    let rest = line[wr_at + 5..].trim();
    let frag = rest.split(" |").next().unwrap_or(rest).trim().to_string();
    let mut wr_ms = None;
    let mut holder = None;
    let first = frag.split_whitespace().next().unwrap_or("");
    if first != "—" && first != "-" {
        wr_ms = page_ms(first);
    }
    if let Some(b) = frag.find(" by ") {
        let h = frag[b + 4..].trim();
        // "by nobody (0 online records)" is an absence, not a holder.
        let h = h.split(" (").next().unwrap_or(h).trim().trim_matches('`');
        if !h.is_empty() && h != "nobody" {
            holder = Some(h.to_string());
        }
    }
    Some(WrClaim { wr_ms, holder, text: frag })
}

/// The author time a caption states.
pub fn caption_at(line: &str) -> Option<i64> {
    let at = line.find("| AT ")?;
    let frag = line[at + 5..].split('|').next()?;
    page_ms(frag.split_whitespace().next()?)
}

/// One map's row in the root README's two result tables:
/// `| [map](dir) | records | author time | best human | this TAS | …`
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RootRow {
    pub name: String,
    pub records: Option<i64>,
    pub at_ms: Option<i64>,
    pub human_ms: Option<i64>,
    pub tas_ms: Option<i64>,
}

pub fn root_rows(readme: &str) -> BTreeMap<String, RootRow> {
    let mut out = BTreeMap::new();
    for line in readme.lines() {
        let l = line.trim();
        if !l.starts_with("| [") {
            continue;
        }
        let cells: Vec<&str> = l.trim_matches('|').split('|').collect();
        if cells.len() < 5 {
            continue;
        }
        let first = cells[0].trim();
        let Some(link) = first.find("](") else { continue };
        let Some(close) = first[link + 2..].find(')') else { continue };
        let dir = first[link + 2..link + 2 + close].to_string();
        let name = first[first.find('[').map(|i| i + 1).unwrap_or(0)..link].to_string();
        out.insert(
            dir,
            RootRow {
                name,
                records: page_count(cells[1]),
                at_ms: page_ms(cells[2]),
                human_ms: page_ms(cells[3]),
                tas_ms: page_ms(cells[4]),
            },
        );
    }
    out
}

// ---------------------------------------------------------------------------
// the join
// ---------------------------------------------------------------------------

/// Everything known about one map, page side and live side.
pub struct Row {
    pub tmx_id: i64,
    pub dir: String,
    pub name: String,
    pub uid: String,
    // what we publish
    pub page_at_ms: Option<i64>,
    pub page_wr_ms: Option<i64>,
    pub page_holder: Option<String>,
    pub page_records: Option<i64>,
    pub page_tas_ms: Option<i64>,
    /// Distinct WR figures the map's own page states, when it states more than one.
    pub page_wr_variants: Vec<String>,
    // live
    pub live_at_ms: Option<i64>,
    pub live_wr_ms: Option<i64>,
    pub live_holder: Option<String>,
    pub live_when: String,
    pub live_records: Option<i64>,
    pub tied: usize,
    pub top_under_at: usize,
    pub tops_seen: usize,
    pub tmx: Option<Tmx>,
    /// `""` when the map's board was fetched cleanly.
    pub fetch_problem: String,
    /// Set when page and live name the same player differently — `keby` vs
    /// `keby..`, `Beagle.3` vs `beagle.3`. A display-name difference is not a
    /// change of holder and must not be reported as one.
    pub name_drift: Option<String>,
    pub verdict: Vec<String>,
}

/// Two spellings of one player. Nadeo display names differ from our captions
/// in case and in trailing dots; anything past that is a different player.
fn same_player(a: &str, b: &str) -> bool {
    let n = |s: &str| s.trim().trim_end_matches(['.', ' ']).to_ascii_lowercase();
    n(a) == n(b)
}

fn read(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

/// HTTP status per (mapid, kind) out of the bank's fetch log.
fn fetch_log(bank: &str) -> BTreeMap<(i64, String), String> {
    let mut out = BTreeMap::new();
    let Some(text) = read(&bank_path(bank, "fetch_log.tsv")) else { return out };
    for line in text.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 3 {
            continue;
        }
        if let Ok(id) = c[0].parse::<i64>() {
            out.insert((id, c[1].to_string()), c[2].to_string());
        }
    }
    out
}

pub struct Table {
    pub root: String,
    pub bank: String,
    /// A previous capture in this tool's TSV format (or the 2026-08-21 one:
    /// the columns read are `mapid`, `wr_ms`, `holder`, `records`).
    pub prev: Option<String>,
    pub out: Option<String>,
    pub tsv: Option<String>,
    pub fetched: String,
    /// Print one map's banked board row by row instead of writing a table.
    /// A surprising verdict is checked here before it is believed.
    pub detail: Option<i64>,
}

struct Prev {
    wr_ms: Option<i64>,
    holder: String,
    records: Option<i64>,
}

fn read_prev(path: &str) -> Result<BTreeMap<i64, Prev>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path, e))?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let col = |n: &str| head.iter().position(|h| *h == n);
    let (Some(ci), Some(cw), Some(ch), Some(cr)) =
        (col("mapid"), col("wr_ms"), col("holder"), col("records"))
    else {
        return Err(format!("{}: needs columns mapid, wr_ms, holder, records", path));
    };
    let mut out = BTreeMap::new();
    for l in lines {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() <= ci.max(cw).max(ch).max(cr) {
            continue;
        }
        let Ok(id) = c[ci].parse::<i64>() else { continue };
        let wr = c[cw].parse::<i64>().ok().filter(|v| *v > 0);
        out.insert(
            id,
            Prev { wr_ms: wr, holder: c[ch].to_string(), records: c[cr].parse::<i64>().ok() },
        );
    }
    Ok(out)
}

fn opt_secs(v: Option<i64>) -> String {
    v.map(secs).unwrap_or_else(|| "—".into())
}

fn opt_count(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "—".into())
}

/// Build every row from the bank and the pages. No network.
pub fn rows(o: &Table) -> Result<Vec<Row>, String> {
    let maps = map_dirs(&o.root)?;
    let root_readme = std::fs::read_to_string(Path::new(&o.root).join("README.md"))
        .map_err(|e| format!("read root README.md: {}", e))?;
    let roots = root_rows(&root_readme);
    let log = fetch_log(&o.bank);

    // Every TMX batch in the bank, joined by id.
    let mut tmx: BTreeMap<i64, Tmx> = BTreeMap::new();
    let rd = std::fs::read_dir(&o.bank).map_err(|e| format!("read {}: {}", o.bank, e))?;
    let mut batches: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("tmx_"))
                .unwrap_or(false)
        })
        .collect();
    batches.sort();
    for b in &batches {
        if let Some(text) = read(b) {
            for (id, t) in tmx_rows(&text) {
                tmx.insert(id, t);
            }
        }
    }

    let mut out = Vec::new();
    for m in &maps {
        let page = read(&Path::new(&o.root).join(&m.dir).join("README.md")).unwrap_or_default();
        let mut claims: Vec<WrClaim> = Vec::new();
        let mut page_at = None;
        for line in page.lines() {
            if line.contains("| AT ") && line.contains("| WR ") {
                if let Some(c) = caption_wr(line) {
                    claims.push(c);
                }
                page_at = page_at.or_else(|| caption_at(line));
            }
        }
        let rr = roots.get(&m.dir).cloned().unwrap_or_default();
        let mut variants: Vec<String> = Vec::new();
        for c in &claims {
            // Distinct WR *times*: two captions naming the same time with
            // different words ("by AffiTM" / "six players tied") are one claim.
            let key = opt_secs(c.wr_ms);
            if !variants.contains(&key) {
                variants.push(key);
            }
        }
        let page_wr = claims.iter().find_map(|c| c.wr_ms).or(rr.human_ms);
        let page_holder = claims.iter().find_map(|c| c.holder.clone());

        let t = tmx.get(&m.tmx_id).cloned();
        let uid = t.as_ref().map(|x| x.uid.clone()).unwrap_or_default();
        let info = read(&bank_path(&o.bank, &format!("tmio_map/{}.json", m.tmx_id)))
            .and_then(|s| map_info(&s));
        let lb_text = read(&bank_path(&o.bank, &format!("tmio_lb/{}.json", m.tmx_id)));
        let lb_http = log.get(&(m.tmx_id, "tmio_lb".to_string())).cloned().unwrap_or_default();

        let mut problem = String::new();
        let mut b = Board::default();
        match (&lb_text, lb_http.as_str()) {
            (_, "") => problem = "no fetch logged".into(),
            (None, _) => problem = "no banked response".into(),
            (Some(text), "200") => match board(text) {
                Ok(x) => b = x,
                Err(e) => problem = format!("unparseable board: {}", e),
            },
            (Some(_), code) => problem = format!("HTTP {}", code),
        }

        let live_at = info
            .as_ref()
            .and_then(|x| x.1)
            .or_else(|| t.as_ref().and_then(|x| x.author_ms))
            // An author score of 0 is "this map declares none", not a 0.000 s
            // author medal.
            .filter(|v| *v > 0);
        let live_wr = b.tops.first().map(|t| t.time_ms);
        let at_for_field = live_at.or(page_at).or(rr.at_ms);
        let top_under_at = match at_for_field {
            Some(a) => b.tops.iter().filter(|t| t.time_ms <= a).count(),
            None => 0,
        };
        let tied = match live_wr {
            Some(w) => b.tops.iter().filter(|t| t.time_ms == w).count(),
            None => 0,
        };

        let name = if !rr.name.is_empty() {
            rr.name.clone()
        } else {
            t.as_ref().map(|x| x.name.clone()).unwrap_or_else(|| m.dir.clone())
        };

        out.push(Row {
            tmx_id: m.tmx_id,
            dir: m.dir.clone(),
            name,
            uid,
            page_at_ms: page_at.or(rr.at_ms),
            page_wr_ms: page_wr,
            page_holder,
            page_records: rr.records,
            page_tas_ms: rr.tas_ms,
            page_wr_variants: variants,
            live_at_ms: live_at,
            live_wr_ms: live_wr,
            live_holder: b.tops.first().map(|t| t.player.clone()),
            live_when: b.tops.first().map(|t| t.when.clone()).unwrap_or_default(),
            live_records: b.records,
            tied,
            top_under_at,
            tops_seen: b.tops.len(),
            tmx: t,
            fetch_problem: problem,
            name_drift: None,
            verdict: Vec::new(),
        });
    }
    Ok(out)
}

/// The verdict column, in the order the reader cares about. `populated` is the
/// positive control: how many maps in this pass came back with a board. `prev`
/// is the previous live capture, which separates *the page is stale* from
/// *the board moved*: a live figure the previous capture already carried is a
/// page/live divergence, not a new record.
fn verdicts(r: &mut Row, populated: usize, prev: Option<&Prev>) {
    if !r.fetch_problem.is_empty() {
        r.verdict.push(format!("**UNKNOWN** — {}", r.fetch_problem));
        return;
    }
    if r.live_wr_ms.is_none() {
        // An empty board is only an absence when the same pass proved the
        // fetch works elsewhere.
        r.verdict.push(if populated > 0 {
            format!("no human record (empty board; {} populated boards in the same pass)", populated)
        } else {
            "**UNKNOWN** — empty board and no positive control in this pass".into()
        });
        return;
    }
    let live = r.live_wr_ms.unwrap();
    // Did this figure arrive since the last capture, or has the page simply
    // never quoted the board's own number?
    let already_there = prev.map(|p| p.wr_ms == Some(live)).unwrap_or(false);
    let aside = if already_there {
        " — already on the previous capture, so the page quotes a different figure rather than the board's"
    } else {
        ""
    };
    match r.page_wr_ms {
        Some(p) if live < p => r.verdict.push(format!(
            "{} {} (page cites {}){}",
            if already_there { "page is stale:" } else { "**NEW WR**" },
            secs(live),
            secs(p),
            aside
        )),
        Some(p) if live > p => r.verdict.push(format!(
            "live WR {} is SLOWER than the {} the page cites{}",
            secs(live),
            secs(p),
            aside
        )),
        Some(_) => {}
        None => r.verdict.push(format!("page cites no WR; live {}", secs(live))),
    }
    if let (Some(ph), Some(lh)) = (&r.page_holder, &r.live_holder) {
        if !same_player(ph, lh) {
            r.verdict.push(format!("holder {} → **{}**", ph, lh));
        } else if ph != lh {
            r.name_drift = Some(format!("{} → {}", ph, lh));
        }
    }
    // Author time: an AT scalp only counts if the page still calls it unbeaten.
    if let Some(at) = r.live_at_ms {
        if live <= at {
            let page_said_unbeaten = r.page_wr_ms.map(|p| p > at).unwrap_or(r.page_wr_ms.is_none());
            let field = format!("{} of the top {} at or under the AT", r.top_under_at, r.tops_seen);
            if page_said_unbeaten && !already_there {
                r.verdict.push(format!(
                    "**AT NO LONGER UNBEATEN** — {} vs AT {} ({})",
                    secs(live),
                    secs(at),
                    field
                ));
            } else {
                r.verdict.push(format!("AT already beaten ({})", field));
            }
        }
    }
    if let Some(tas) = r.page_tas_ms {
        if live < tas {
            r.verdict.push(format!(
                "{} {} < {}",
                if already_there { "human ahead of our TAS:" } else { "**HUMAN PASSED OUR TAS**" },
                secs(live),
                secs(tas)
            ));
        }
    }
    if let (Some(pr), Some(lr)) = (r.page_records, r.live_records) {
        if lr != pr {
            r.verdict.push(format!("records {} → {}", pr, lr));
        }
    }
    if r.verdict.is_empty() {
        r.verdict.push("unchanged".into());
    }
}

// ---------------------------------------------------------------------------
// output
// ---------------------------------------------------------------------------

fn tsv(rows: &[Row], fetched: &str) -> String {
    let mut s = String::from(
        "mapid\tdir\tname\tuid\tat_ms\ttas_ms\twr_ms\tholder\twhen\trecords\ttied\ttmx_wr_ms\ttmx_holder\ttmx_records\tpage_wr_ms\tpage_holder\tpage_records\tfetched\tproblem\n",
    );
    for r in rows {
        let t = r.tmx.clone().unwrap_or_default();
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.tmx_id,
            r.dir,
            r.name,
            r.uid,
            r.live_at_ms.unwrap_or(0),
            r.page_tas_ms.unwrap_or(0),
            r.live_wr_ms.unwrap_or(0),
            r.live_holder.clone().unwrap_or_default(),
            r.live_when,
            r.live_records.unwrap_or(0),
            r.tied,
            t.wr_ms.unwrap_or(0),
            t.wr_holder.unwrap_or_default(),
            t.records.unwrap_or(0),
            r.page_wr_ms.unwrap_or(0),
            r.page_holder.clone().unwrap_or_default(),
            r.page_records.unwrap_or(0),
            fetched,
            r.fetch_problem,
        ));
    }
    s
}

fn markdown(o: &Table, rows: &[Row], prev: &BTreeMap<i64, Prev>) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "# Live leaderboards — refresh of {}\n\n\
         Every map in this repo, fetched from the public mirrors of the Nadeo board and \
         set beside what this repo publishes. Times are seconds. **Read only: nothing here \
         was submitted to any leaderboard.**\n\n\
         Sources: trackmania.io (`/api/map/<uid>`, `/api/leaderboard/map/<uid>`) for the live \
         board, holder, timestamp and total record count; trackmania.exchange \
         (`/api/maps?id=…&fields=…`) for the TMX id → uid resolution and its own mirror of the \
         same record, kept as a separate column rather than merged. Nadeo's own live service \
         needs a Ubisoft session ticket this project does not hold, so these two mirrors are \
         the board of record here. Rebuild this file from the banked responses with \
         `tmsite records` — it does not touch the network.\n\n\
         \"WR then\" is what **this repo currently says** — the caption line on the map's own \
         page (`| WR … by …`), falling back to the *best human* column of the root README. \
         \"records then\" is that page's recorded-run count.\n\n",
        o.fetched
    ));

    // The headline, in the order a reader cares: a record we quote that has
    // been beaten, an author time that is no longer unbeaten, a human past one
    // of our runs. Each list is derived from the verdicts, not written by hand.
    let hits = |needle: &str| -> Vec<String> {
        rows.iter()
            .filter(|r| r.verdict.iter().any(|v| v.contains(needle)))
            .map(|r| r.name.clone())
            .collect()
    };
    for (title, needle) in [
        ("New world record", "**NEW WR**"),
        ("Author time no longer unbeaten", "**AT NO LONGER UNBEATEN**"),
        ("A human is now past our TAS", "**HUMAN PASSED OUR TAS**"),
    ] {
        let h = hits(needle);
        s.push_str(&format!(
            "**{}**: {}\n\n",
            title,
            if h.is_empty() { "none.".to_string() } else { format!("{}.", h.join(", ")) }
        ));
    }

    s.push_str("| map | our TAS | AT | WR then | WR now | holder now | records then | records now | verdict |\n");
    s.push_str("|---|---|---|---|---|---|---|---|---|\n");
    for r in rows {
        s.push_str(&format!(
            "| [{}]({}) | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.name,
            r.dir,
            opt_secs(r.page_tas_ms),
            opt_secs(r.live_at_ms.or(r.page_at_ms)),
            opt_secs(r.page_wr_ms),
            opt_secs(r.live_wr_ms),
            r.live_holder.clone().filter(|h| !h.is_empty()).unwrap_or_else(|| "—".into()),
            opt_count(r.page_records),
            opt_count(r.live_records),
            r.verdict.join("; "),
        ));
    }

    // What actually changed.
    let changed: Vec<&Row> = rows
        .iter()
        .filter(|r| !(r.verdict.len() == 1 && r.verdict[0] == "unchanged"))
        .collect();
    s.push_str(&format!("\n## What changed ({} of {} maps)\n\n", changed.len(), rows.len()));
    for r in &changed {
        s.push_str(&format!("- **{}** — {}\n", r.name, r.verdict.join("; ")));
    }

    // Movement since the previous live capture, which is a different question
    // from "is the page stale".
    if !prev.is_empty() {
        s.push_str("\n## Since the previous live capture\n\n");
        let mut any = false;
        for r in rows {
            let Some(p) = prev.get(&r.tmx_id) else { continue };
            let mut bits = Vec::new();
            match (p.wr_ms, r.live_wr_ms) {
                (Some(a), Some(b)) if a != b => bits.push(format!("WR {} → **{}**", secs(a), secs(b))),
                (None, Some(b)) => bits.push(format!("first record **{}**", secs(b))),
                (Some(a), None) => bits.push(format!("WR {} → nothing on the board", secs(a))),
                _ => {}
            }
            if let Some(lh) = &r.live_holder {
                if !p.holder.is_empty() && p.holder != *lh {
                    bits.push(format!("holder {} → **{}**", p.holder, lh));
                }
            }
            if let (Some(a), Some(b)) = (p.records, r.live_records) {
                if a != b {
                    bits.push(format!("records {} → {}", a, b));
                }
            }
            if !bits.is_empty() {
                any = true;
                s.push_str(&format!("- **{}** — {}\n", r.name, bits.join(", ")));
            }
        }
        if !any {
            s.push_str("- nothing moved.\n");
        }
    }

    // Controls.
    let with_board = rows.iter().filter(|r| r.tops_seen > 0).count();
    let empty: Vec<&Row> = rows.iter().filter(|r| r.fetch_problem.is_empty() && r.tops_seen == 0).collect();
    let failed: Vec<&Row> = rows.iter().filter(|r| !r.fetch_problem.is_empty()).collect();
    let mut disagree = Vec::new();
    let mut agree = 0usize;
    for r in rows {
        let Some(t) = &r.tmx else { continue };
        match (t.wr_ms, r.live_wr_ms) {
            (Some(a), Some(b)) if a == b => agree += 1,
            (Some(a), Some(b)) => disagree.push(format!("{}: TMX {} vs live {}", r.name, secs(a), secs(b))),
            (None, None) => agree += 1,
            (a, b) => disagree.push(format!(
                "{}: TMX {} vs live {}",
                r.name,
                opt_secs(a),
                opt_secs(b)
            )),
        }
    }
    let mut count_gap = Vec::new();
    for r in rows {
        if let (Some(t), Some(l)) = (r.tmx.as_ref().and_then(|t| t.records), r.live_records) {
            if t != l {
                count_gap.push(format!("{}: TMX {} vs live {}", r.name, t, l));
            }
        }
    }
    let mut at_moved = Vec::new();
    for r in rows {
        if let (Some(p), Some(l)) = (r.page_at_ms, r.live_at_ms) {
            if p != l {
                at_moved.push(format!("{}: page {} vs live {}", r.name, secs(p), secs(l)));
            }
        }
    }
    s.push_str("\n## Controls\n\n");
    s.push_str(&format!(
        "- **Positive control for every empty board**: {} of {} maps returned a populated board in this same pass, through the same code path, so an empty board is an absence of records and not a failed fetch.\n",
        with_board,
        rows.len()
    ));
    if empty.is_empty() {
        s.push_str("- No map returned an empty board.\n");
    } else {
        s.push_str(&format!(
            "- Empty boards ({}): {}.\n",
            empty.len(),
            empty.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    if failed.is_empty() {
        s.push_str("- Every request returned HTTP 200; no rate limit and no auth failure was hit.\n");
    } else {
        s.push_str(&format!(
            "- **UNKNOWN, fetch did not succeed** ({}): {}.\n",
            failed.len(),
            failed
                .iter()
                .map(|r| format!("{} ({})", r.name, r.fetch_problem))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    s.push_str(&format!(
        "- **Second source**: TMX's own mirror agrees with the live board's WR on {} of {} maps.{}\n",
        agree,
        rows.len(),
        if disagree.is_empty() {
            String::new()
        } else {
            format!(" Disagreements: {}.", disagree.join("; "))
        }
    ));
    s.push_str(&format!(
        "- **Record counts across the two sources**: {}\n",
        if count_gap.is_empty() {
            "identical on every map.".to_string()
        } else {
            format!(
                "TMX's count lags the live board on {} map(s) — {}. The live count is the one tabulated.",
                count_gap.len(),
                count_gap.join("; ")
            )
        }
    ));
    let drift: Vec<&Row> = rows.iter().filter(|r| r.name_drift.is_some()).collect();
    if !drift.is_empty() {
        s.push_str(&format!(
            "- Display-name spelling only, **not** a change of holder: {}.\n",
            drift
                .iter()
                .map(|r| format!("{} ({})", r.name, r.name_drift.clone().unwrap_or_default()))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    s.push_str(&format!(
        "- **Author times**: {}\n",
        if at_moved.is_empty() {
            "every page's author time matches the live one.".to_string()
        } else {
            format!("moved on {} map(s): {}.", at_moved.len(), at_moved.join("; "))
        }
    ));
    let multi: Vec<&Row> = rows.iter().filter(|r| r.page_wr_variants.len() > 1).collect();
    if !multi.is_empty() {
        s.push_str(&format!(
            "- Pages stating more than one WR figure: {}.\n",
            multi
                .iter()
                .map(|r| format!("{} ({})", r.name, r.page_wr_variants.join(" / ")))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    s
}

/// Join the bank with the pages and write the table.
pub fn records(o: &Table) -> Result<String, String> {
    let mut rs = rows(o)?;
    if let Some(id) = o.detail {
        let r = rs
            .iter()
            .find(|r| r.tmx_id == id)
            .ok_or_else(|| format!("no map {} in {}", id, o.root))?;
        let text = read(&bank_path(&o.bank, &format!("tmio_lb/{}.json", id)))
            .ok_or_else(|| format!("no banked board for {}", id))?;
        let b = board(&text)?;
        println!(
            "{} {}\n  uid            {}\n  page           TAS {} | AT {} | WR {} by {}\n  live author    {}\n  records        {}\n  board rows     {}",
            r.tmx_id,
            r.name,
            r.uid,
            opt_secs(r.page_tas_ms),
            opt_secs(r.page_at_ms),
            opt_secs(r.page_wr_ms),
            r.page_holder.clone().unwrap_or_else(|| "—".into()),
            opt_secs(r.live_at_ms),
            opt_count(r.live_records),
            b.tops.len()
        );
        for t in &b.tops {
            println!(
                "  {:>3}  {:>12}  {:<24} {}  {}",
                t.position,
                secs(t.time_ms),
                t.player,
                t.when,
                if t.ghost.is_empty() {
                    "no stored replay".to_string()
                } else {
                    format!("https://trackmania.io{}", t.ghost)
                }
            );
        }
        return Ok(format!("{} board rows printed", b.tops.len()));
    }
    let populated = rs.iter().filter(|r| r.tops_seen > 0).count();
    let prev = match &o.prev {
        Some(p) => read_prev(p)?,
        None => BTreeMap::new(),
    };
    for r in rs.iter_mut() {
        let p = prev.get(&r.tmx_id);
        verdicts(r, populated, p);
    }
    let md = markdown(o, &rs, &prev);
    match &o.out {
        Some(f) => std::fs::write(f, &md).map_err(|e| format!("write {}: {}", f, e))?,
        None => print!("{}", md),
    }
    if let Some(f) = &o.tsv {
        std::fs::write(f, tsv(&rs, &o.fetched)).map_err(|e| format!("write {}: {}", f, e))?;
    }
    let changed = rs
        .iter()
        .filter(|r| !(r.verdict.len() == 1 && r.verdict[0] == "unchanged"))
        .count();
    Ok(format!(
        "{} maps, {} with a live board, {} changed against what the pages claim",
        rs.len(),
        populated,
        changed
    ))
}
