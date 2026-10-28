//! tmlb -- the leaderboards of Nadeo's seasonal campaigns, from trackmania.io.
//!
//!   tmlb campaigns [--kind seasonal|totd|weekly]
//!                                          the campaigns (id, name, maps, published)
//!   tmlb maps --out maps.tsv [--kind seasonal|totd|weekly] [--campaign "Summer 2021"]...
//!                                          every map of every campaign of that kind:
//!                                          seasonal = the 26 Nadeo seasons (650 maps),
//!                                          totd = every Track of the Day by month,
//!                                          weekly = every Weekly Shorts week (5 maps each)
//!   tmlb top --maps maps.tsv --out DIR [--length 500] [--pace-ms 1200] [--campaign NAME]...
//!                                          the top N of each map: raw pages of 100 in
//!                                          DIR/<uid>/<offset>.json; a page already on disk is
//!                                          never fetched again, so a stopped sweep resumes
//!   tmlb flatten DIR --maps maps.tsv --out records.tsv
//!                                          one row a record: campaign, map, uid, position,
//!                                          time_ms, account_id, player, zone chain, set_utc,
//!                                          ghost_url
//!   tmlb sweep --out DIR [--kind seasonal|totd|weekly] [--length 500] [--pace-ms 1200] [--campaign NAME]...
//!                                          campaigns -> maps -> top -> flatten, in one go
//!   tmlb report records.tsv [--top 10] [--late-years 2] [--players 40]
//!                                          what the table says: records per campaign and
//!                                          year, the players with the most top-N records,
//!                                          the #1s with the largest lead over #2, and the
//!                                          LATE records -- top-N positions taken years after
//!                                          the campaign was published (a 2026 record on a
//!                                          2021 map at #1 is where this started)
//!
//! Pace: never two requests closer than --pace-ms (default 1200 ms -- the same
//! courtesy the investigations used); a 429 waits 60 s, a 5xx 10 s, five
//! failures on one page and the page is skipped and named at the end. Every
//! request carries a user agent that says who is asking and why.
//!
//! trackmania.io's leaderboard endpoint serves 100 rows a page and reaches at
//! least offset 900; `--length` rounds up to whole pages.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::Value;

const UA: &str = "trackmania-tas-research/1.0 (fair-play analysis; contact vjeux@meta.com)";
const BASE: &str = "https://trackmania.io/api";

fn die(msg: impl AsRef<str>) -> ! {
    eprintln!("tmlb: {}", msg.as_ref());
    std::process::exit(2);
}

fn flag<'a>(a: &'a [String], k: &str) -> Option<&'a str> {
    a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).map(String::as_str)
}

fn flags<'a>(a: &'a [String], k: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < a.len() {
        if a[i] == k {
            if let Some(v) = a.get(i + 1) {
                out.push(v.as_str());
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

/// A TSV cell: tabs, carriage returns and newlines become spaces. Map and
/// player names carry all three (a Track of the Day of 2023-01 has a newline
/// inside its name), and one of them splits a row.
fn cell(s: &str) -> String {
    s.replace(['\t', '\r', '\n'], " ")
}

/// Rows of a TSV whose cells were NOT sanitised when written (the first
/// sweeps): a physical line with fewer than `n` fields is joined to the next
/// line until the row is whole. A rewritten table never needs this.
fn tsv_rows(text: &str, n: usize) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let mut pending = String::new();
    for l in text.lines() {
        if pending.is_empty() {
            pending = l.to_string();
        } else {
            pending.push(' ');
            pending.push_str(l);
        }
        if pending.split('\t').count() >= n {
            rows.push(std::mem::take(&mut pending));
        }
    }
    if !pending.trim().is_empty() {
        rows.push(pending);
    }
    rows
}

fn need<'a>(a: &'a [String], k: &str) -> &'a str {
    flag(a, k).unwrap_or_else(|| die(format!("missing {k} VALUE")))
}

/// One HTTP client that keeps the pace.
struct Client {
    agent: ureq::Agent,
    pace: Duration,
    last: Option<Instant>,
    requests: usize,
}

impl Client {
    fn new(pace_ms: u64) -> Client {
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_global(Some(Duration::from_secs(60)))
                .build(),
        );
        Client { agent, pace: Duration::from_millis(pace_ms), last: None, requests: 0 }
    }

    fn wait_turn(&mut self) {
        if let Some(t) = self.last {
            let since = t.elapsed();
            if since < self.pace {
                sleep(self.pace - since);
            }
        }
        self.last = Some(Instant::now());
        self.requests += 1;
    }

    /// GET a JSON document, with the pace and the retry policy above.
    fn get_json(&mut self, url: &str) -> Result<Value, String> {
        let mut failures = 0;
        loop {
            self.wait_turn();
            let resp = self.agent.get(url).header("user-agent", UA).header("accept", "application/json").call();
            match resp {
                Ok(mut r) => {
                    let status = r.status().as_u16();
                    let body = r
                        .body_mut()
                        .with_config()
                        .limit(32 << 20)
                        .read_to_string()
                        .map_err(|e| format!("{url}: reading the body: {e}"))?;
                    match status {
                        200 => {
                            return serde_json::from_str::<Value>(&body)
                                .map_err(|e| format!("{url}: not JSON ({e}): {}", &body[..body.len().min(120)]))
                        }
                        429 => {
                            failures += 1;
                            eprintln!("  429 on {url}; waiting 60 s ({failures}/5)");
                            sleep(Duration::from_secs(60));
                        }
                        500..=599 => {
                            failures += 1;
                            eprintln!("  {status} on {url}; waiting 10 s ({failures}/5)");
                            sleep(Duration::from_secs(10));
                        }
                        _ => return Err(format!("{url}: http {status}: {}", &body[..body.len().min(160)])),
                    }
                }
                Err(e) => {
                    failures += 1;
                    eprintln!("  transport error on {url}: {e}; waiting 10 s ({failures}/5)");
                    sleep(Duration::from_secs(10));
                }
            }
            if failures >= 5 {
                return Err(format!("{url}: gave up after 5 failures"));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Seasonal,
    Totd,
    Weekly,
}

impl Kind {
    fn parse(s: Option<&str>) -> Kind {
        match s.unwrap_or("seasonal") {
            "seasonal" => Kind::Seasonal,
            "totd" => Kind::Totd,
            "weekly" | "weeklyshorts" | "shorts" => Kind::Weekly,
            other => die(format!("--kind {other}: seasonal, totd or weekly")),
        }
    }
}

#[derive(Clone, Debug)]
struct Campaign {
    id: i64,
    name: String,
    mapcount: i64,
    published: i64,
}

/// The seasonal campaigns, or the Weekly Shorts weeks: the same paged listing
/// under two paths. Track of the Day has no campaign listing -- its "campaigns"
/// are the months, produced by `totd_maps` directly.
fn campaigns(c: &mut Client, kind: Kind) -> Vec<Campaign> {
    let (path, prefix) = match kind {
        Kind::Seasonal => ("seasonal", ""),
        Kind::Weekly => ("weeklyshorts", "Weekly Shorts "),
        Kind::Totd => die("Track of the Day has months, not campaigns: use `maps --kind totd`"),
    };
    let mut out = Vec::new();
    let mut page = 0;
    loop {
        let v = c.get_json(&format!("{BASE}/campaigns/{path}/{page}")).unwrap_or_else(|e| die(e));
        for k in v["campaigns"].as_array().cloned().unwrap_or_default() {
            let ok = match kind {
                Kind::Seasonal => k["seasonal"].as_bool() == Some(true),
                _ => k["weekly"].as_bool() == Some(true),
            };
            if !ok {
                continue;
            }
            out.push(Campaign {
                id: k["id"].as_i64().unwrap_or(0),
                name: format!("{prefix}{}", k["name"].as_str().unwrap_or("")),
                mapcount: k["mapcount"].as_i64().unwrap_or(0),
                published: k["timestamp"].as_i64().unwrap_or(0),
            });
        }
        let pages = v["pageCount"].as_i64().unwrap_or(1);
        page += 1;
        if page >= pages {
            break;
        }
    }
    out
}

/// Every Track of the Day, month by month (`/api/totd/<monthoffset>`, 0 = the
/// current month). The campaign is the month ("TOTD 2024-03"), the index the
/// day of the month, `published` the month's first day.
fn totd_maps(c: &mut Client) -> Vec<MapRow> {
    let mut out = Vec::new();
    let mut offset = 0;
    let mut months = 1;
    while offset < months {
        let v = c.get_json(&format!("{BASE}/totd/{offset}")).unwrap_or_else(|e| die(e));
        months = v["monthcount"].as_i64().unwrap_or(1) as usize;
        let year = v["year"].as_i64().unwrap_or(0);
        let month = v["month"].as_i64().unwrap_or(0);
        let published = days_from_civil(year, month, 1) * 86400;
        for d in v["days"].as_array().cloned().unwrap_or_default() {
            let m = &d["map"];
            if m["mapUid"].as_str().is_none() {
                continue;
            }
            out.push(MapRow {
                campaign: format!("TOTD {year}-{month:02}"),
                campaign_id: d["campaignid"].as_i64().unwrap_or(0),
                index: d["monthday"].as_i64().unwrap_or(0) as usize,
                name: cell(m["name"].as_str().unwrap_or("")),
                uid: m["mapUid"].as_str().unwrap_or("").to_string(),
                map_id: m["mapId"].as_str().unwrap_or("").to_string(),
                author_ms: m["authorScore"].as_i64().unwrap_or(0),
                published,
            });
        }
        if offset % 12 == 0 {
            eprintln!("  totd month {offset}/{months}: {year}-{month:02}, {} maps so far", out.len());
        }
        offset += 1;
    }
    out
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn all_maps(c: &mut Client, kind: Kind, only: &[&str]) -> Vec<MapRow> {
    match kind {
        Kind::Totd => totd_maps(c),
        _ => {
            let mut rows = Vec::new();
            for k in campaigns(c, kind).iter().filter(|k| only.is_empty() || only.iter().any(|o| o.eq_ignore_ascii_case(&k.name))) {
                rows.extend(maps_of(c, k, kind));
            }
            rows
        }
    }
}

#[derive(Clone, Debug)]
struct MapRow {
    campaign: String,
    campaign_id: i64,
    index: usize,
    name: String,
    uid: String,
    map_id: String,
    author_ms: i64,
    /// The campaign's publish time, Unix seconds (0 when the table predates the column).
    published: i64,
}

const MAPS_HEADER: &str = "campaign\tcampaign_id\tindex\tname\tuid\tmap_id\tauthor_ms\tpublished";

fn maps_of(c: &mut Client, k: &Campaign, kind: Kind) -> Vec<MapRow> {
    let path = match kind {
        Kind::Seasonal => "seasonal",
        Kind::Weekly => "weeklyshorts",
        Kind::Totd => unreachable!("totd has no campaign detail"),
    };
    let v = c.get_json(&format!("{BASE}/campaign/{path}/{}", k.id)).unwrap_or_else(|e| die(e));
    v["playlist"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(i, m)| MapRow {
            campaign: k.name.clone(),
            campaign_id: k.id,
            index: i + 1,
            name: cell(m["name"].as_str().unwrap_or("")),
            uid: m["mapUid"].as_str().unwrap_or("").to_string(),
            map_id: m["mapId"].as_str().unwrap_or("").to_string(),
            author_ms: m["authorScore"].as_i64().unwrap_or(0),
            published: k.published,
        })
        .collect()
}

fn write_maps(path: &str, rows: &[MapRow]) {
    let mut s = String::from(MAPS_HEADER);
    s.push('\n');
    for r in rows {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            cell(&r.campaign), r.campaign_id, r.index, cell(&r.name), r.uid, r.map_id, r.author_ms, r.published
        ));
    }
    fs::write(path, s).unwrap_or_else(|e| die(format!("{path}: {e}")));
}

fn read_maps(path: &str) -> Vec<MapRow> {
    let s = fs::read_to_string(path).unwrap_or_else(|e| die(format!("{path}: {e}")));
    let n = s.lines().next().map(|h| h.split('\t').count()).unwrap_or(7);
    tsv_rows(&s, n)
        .iter()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 7 {
                die(format!("{path}: short row: {l}"));
            }
            MapRow {
                campaign: f[0].to_string(),
                campaign_id: f[1].parse().unwrap_or(0),
                index: f[2].parse().unwrap_or(0),
                name: f[3].to_string(),
                uid: f[4].to_string(),
                map_id: f[5].to_string(),
                author_ms: f[6].parse().unwrap_or(0),
                published: f.get(7).and_then(|x| x.parse().ok()).unwrap_or(0),
            }
        })
        .collect()
}

fn selected<'a>(rows: &'a [MapRow], only: &[&str]) -> Vec<&'a MapRow> {
    rows.iter().filter(|r| only.is_empty() || only.iter().any(|o| o.eq_ignore_ascii_case(&r.campaign))).collect()
}

/// The top N of every selected map, page by page, each page written only when
/// it arrived whole. Returns the pages that failed.
fn top(c: &mut Client, maps: &[&MapRow], out: &Path, length: usize) -> Vec<String> {
    let pages = (length + 99) / 100;
    let mut failed = Vec::new();
    let total = maps.len() * pages;
    let mut done = 0;
    let started = Instant::now();
    for m in maps {
        let dir = out.join(&m.uid);
        fs::create_dir_all(&dir).unwrap_or_else(|e| die(format!("{}: {e}", dir.display())));
        for p in 0..pages {
            let offset = p * 100;
            let file = dir.join(format!("{offset}.json"));
            done += 1;
            if file.exists() {
                continue;
            }
            let url = format!("{BASE}/leaderboard/map/{}?offset={offset}&length=100", m.uid);
            match c.get_json(&url) {
                Ok(v) => {
                    let n = v["tops"].as_array().map(|a| a.len()).unwrap_or(0);
                    fs::write(&file, serde_json::to_string(&v).unwrap_or_default())
                        .unwrap_or_else(|e| die(format!("{}: {e}", file.display())));
                    if done % 25 == 0 || n < 100 {
                        let el = started.elapsed().as_secs();
                        eprintln!(
                            "[{done}/{total}] {} / {} offset {offset}: {n} rows  ({} requests, {}m{:02}s)",
                            m.campaign,
                            m.name,
                            c.requests,
                            el / 60,
                            el % 60
                        );
                    }
                    if n < 100 {
                        // the board ends here; the later pages would be empty
                        for q in (p + 1)..pages {
                            let f2 = dir.join(format!("{}.json", q * 100));
                            if !f2.exists() {
                                fs::write(&f2, "{\"tops\":[],\"short\":true}").ok();
                            }
                        }
                        done += pages - p - 1;
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("  FAILED {e}");
                    failed.push(format!("{}\t{}\t{offset}", m.uid, m.name));
                }
            }
        }
    }
    failed
}

fn zone_chain(z: &Value) -> String {
    let mut parts = Vec::new();
    let mut cur = z;
    while let Some(name) = cur["name"].as_str() {
        parts.push(name.to_string());
        cur = &cur["parent"];
    }
    parts.join("|")
}

const RECORDS_HEADER: &str =
    "campaign\tmap_index\tmap\tuid\tmap_id\tauthor_ms\tposition\ttime_ms\taccount_id\tplayer\tzone\tset_utc\tghost_url\tcampaign_published";

/// Unix seconds -> "YYYY-MM-DD" (empty for 0).
fn ymd(secs: i64) -> String {
    if secs <= 0 {
        return String::new();
    }
    let z = secs.div_euclid(86400) + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn flatten(dir: &Path, maps: &[MapRow], out: &str) -> (usize, usize) {
    let by_uid: BTreeMap<&str, &MapRow> = maps.iter().map(|m| (m.uid.as_str(), m)).collect();
    let mut s = String::from(RECORDS_HEADER);
    s.push('\n');
    let mut n_maps = 0;
    let mut n_rows = 0;
    for m in maps {
        let d = dir.join(&m.uid);
        if !d.is_dir() {
            continue;
        }
        let mut pages: Vec<PathBuf> = fs::read_dir(&d)
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "json")).collect())
            .unwrap_or_default();
        pages.sort_by_key(|p| p.file_stem().and_then(|x| x.to_str()).and_then(|x| x.parse::<usize>().ok()).unwrap_or(0));
        let mut any = false;
        for p in pages {
            let txt = fs::read_to_string(&p).unwrap_or_default();
            let v: Value = serde_json::from_str(&txt).unwrap_or(Value::Null);
            for t in v["tops"].as_array().cloned().unwrap_or_default() {
                any = true;
                n_rows += 1;
                let mr = by_uid.get(m.uid.as_str()).copied().unwrap_or(m);
                s.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    cell(&mr.campaign),
                    mr.index,
                    cell(&mr.name),
                    mr.uid,
                    mr.map_id,
                    mr.author_ms,
                    t["position"].as_i64().unwrap_or(0),
                    t["time"].as_i64().unwrap_or(0),
                    t["player"]["id"].as_str().unwrap_or(""),
                    cell(t["player"]["name"].as_str().unwrap_or("")),
                    cell(&zone_chain(&t["player"]["zone"])),
                    t["timestamp"].as_str().unwrap_or(""),
                    t["url"].as_str().map(|u| format!("https://trackmania.io{u}")).unwrap_or_default(),
                    ymd(mr.published)
                ));
            }
        }
        if any {
            n_maps += 1;
        }
    }
    fs::write(out, s).unwrap_or_else(|e| die(format!("{out}: {e}")));
    (n_maps, n_rows)
}

/// One flattened record, as `report` reads it back.
#[derive(Clone, Debug)]
struct Rec {
    campaign: String,
    map: String,
    uid: String,
    author_ms: i64,
    position: i64,
    time_ms: i64,
    account_id: String,
    player: String,
    zone: String,
    set_utc: String,
    ghost_url: String,
    campaign_published: String,
}

fn read_records(path: &str) -> Vec<Rec> {
    let s = fs::read_to_string(path).unwrap_or_else(|e| die(format!("{path}: {e}")));
    let n = s.lines().next().map(|h| h.split('\t').count()).unwrap_or(13);
    let mut out = Vec::new();
    for l in tsv_rows(&s, n).iter().skip(1).filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 13 {
            die(format!("{path}: short row: {l}"));
        }
        out.push(Rec {
            campaign: f[0].to_string(),
            map: f[2].to_string(),
            uid: f[3].to_string(),
            author_ms: f[5].parse().unwrap_or(0),
            position: f[6].parse().unwrap_or(0),
            time_ms: f[7].parse().unwrap_or(0),
            account_id: f[8].to_string(),
            player: f[9].to_string(),
            zone: f[10].to_string(),
            set_utc: f[11].to_string(),
            ghost_url: f[12].to_string(),
            campaign_published: f.get(13).unwrap_or(&"").to_string(),
        });
    }
    out
}

/// The campaign's publish time as a decimal year: the table's own column when
/// it has one, else read off a seasonal name.
fn rec_campaign_year(r: &Rec) -> Option<f64> {
    stamp_year(&r.campaign_published).or_else(|| campaign_year(&r.campaign))
}

/// "Summer 2021" -> 2021.5 (the season's rough publish time as a decimal year).
fn campaign_year(name: &str) -> Option<f64> {
    let mut it = name.split_whitespace();
    let season = it.next()?;
    let year: f64 = it.next()?.parse().ok()?;
    let frac = match season {
        "Winter" => 0.0,
        "Spring" => 0.25,
        "Summer" => 0.5,
        "Fall" => 0.75,
        _ => return None,
    };
    Some(year + frac)
}

/// "2026-10-22T18:10:41+00:00" -> 2026.81
fn stamp_year(s: &str) -> Option<f64> {
    let y: f64 = s.get(0..4)?.parse().ok()?;
    let m: f64 = s.get(5..7)?.parse().ok()?;
    let d: f64 = s.get(8..10)?.parse().ok()?;
    Some(y + (m - 1.0) / 12.0 + (d - 1.0) / 365.0)
}

fn secs(ms: i64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

fn report(recs: &[Rec], top: i64, late_years: f64, players: usize) {
    // trackmania.io hides the times of the live Weekly Shorts week (and the odd
    // old record) as -1: those rows keep their position but carry no time, so
    // nothing below measures them.
    let hidden = recs.iter().filter(|r| r.time_ms <= 0).count();
    if hidden > 0 {
        println!("({hidden} records carry a hidden time of -1 -- trackmania.io masks the live Weekly Shorts week; positions kept, times excluded below)");
    }
    println!("RECORDS TABLE: {} records, {} maps, {} campaigns, {} distinct accounts",
        recs.len(),
        recs.iter().map(|r| r.uid.as_str()).collect::<std::collections::BTreeSet<_>>().len(),
        recs.iter().map(|r| r.campaign.as_str()).collect::<std::collections::BTreeSet<_>>().len(),
        recs.iter().map(|r| r.account_id.as_str()).collect::<std::collections::BTreeSet<_>>().len());

    // per campaign: maps, records, depth reached, how many of the top-N were set in the campaign's own year
    println!("\nBY CAMPAIGN (records / maps / deepest position / top-{top} set within a year of release / top-{top} set {late_years}+ years later)");
    let mut camp_year: BTreeMap<&str, f64> = BTreeMap::new();
    for r in recs {
        camp_year.entry(r.campaign.as_str()).or_insert_with(|| rec_campaign_year(r).unwrap_or(0.0));
    }
    let mut camps: Vec<&str> = camp_year.keys().copied().collect();
    camps.sort_by(|a, b| camp_year[a].partial_cmp(&camp_year[b]).unwrap_or(std::cmp::Ordering::Equal));
    for c in &camps {
        let rs: Vec<&Rec> = recs.iter().filter(|r| r.campaign == *c).collect();
        let maps = rs.iter().map(|r| r.uid.as_str()).collect::<std::collections::BTreeSet<_>>().len();
        let deepest = rs.iter().map(|r| r.position).max().unwrap_or(0);
        let cy = camp_year[c];
        let topn: Vec<&&Rec> = rs.iter().filter(|r| r.position <= top).collect();
        let fresh = topn.iter().filter(|r| stamp_year(&r.set_utc).map_or(false, |y| y - cy < 1.0)).count();
        let late = topn.iter().filter(|r| stamp_year(&r.set_utc).map_or(false, |y| y - cy >= late_years)).count();
        println!("  {:<12} {:>6} {:>4} {:>5}   {:>4}/{:<4} {:>4}", c, rs.len(), maps, deepest, fresh, topn.len(), late);
    }

    // by year the record was set
    println!("\nBY YEAR THE RECORD WAS SET (all positions / top-{top})");
    let mut by_year: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for r in recs {
        let y = r.set_utc.get(0..4).unwrap_or("?").to_string();
        let e = by_year.entry(y).or_insert((0, 0));
        e.0 += 1;
        if r.position <= top {
            e.1 += 1;
        }
    }
    for (y, (n, t)) in &by_year {
        println!("  {y}  {n:>7}  {t:>6}");
    }

    // players
    println!("\nPLAYERS with the most top-{top} records (top-{top} / #1s / all in table / campaigns spanned / first..last set)");
    let mut per: BTreeMap<&str, (String, usize, usize, usize, std::collections::BTreeSet<&str>, String, String)> = BTreeMap::new();
    for r in recs {
        let e = per.entry(r.account_id.as_str()).or_insert((r.player.clone(), 0, 0, 0, Default::default(), r.set_utc.clone(), r.set_utc.clone()));
        e.3 += 1;
        if r.position <= top {
            e.1 += 1;
        }
        if r.position == 1 {
            e.2 += 1;
        }
        e.4.insert(r.campaign.as_str());
        if r.set_utc < e.5 {
            e.5 = r.set_utc.clone();
        }
        if r.set_utc > e.6 {
            e.6 = r.set_utc.clone();
        }
    }
    let mut pv: Vec<_> = per.iter().collect();
    pv.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(b.1 .2.cmp(&a.1 .2)).then(b.1 .3.cmp(&a.1 .3)));
    for (id, (name, t, ones, all, camps, first, last)) in pv.iter().take(players) {
        println!("  {:<20} {:>4} {:>4} {:>6} {:>3}   {}..{}   {}", name, t, ones, all, camps.len(), first.get(0..10).unwrap_or(""), last.get(0..10).unwrap_or(""), id);
    }

    // #1 leads over #2
    println!("\n#1s WITH THE LARGEST LEAD OVER #2 (lead / lead % of the time / set / player / map)");
    let mut leads: Vec<(i64, f64, &Rec)> = Vec::new();
    let mut by_map: BTreeMap<&str, Vec<&Rec>> = BTreeMap::new();
    for r in recs {
        by_map.entry(r.uid.as_str()).or_default().push(r);
    }
    for (_, v) in by_map.iter_mut() {
        v.sort_by_key(|r| r.position);
        if v.len() >= 2 && v[0].position == 1 && v[1].position == 2 && v[0].time_ms > 0 && v[1].time_ms > 0 {
            let lead = v[1].time_ms - v[0].time_ms;
            leads.push((lead, 100.0 * lead as f64 / v[0].time_ms.max(1) as f64, v[0]));
        }
    }
    leads.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (lead, pct, r) in leads.iter().take(25) {
        println!("  {:>7} {:>5.2}%  {}  {:<20} {}", secs(*lead), pct, r.set_utc.get(0..10).unwrap_or(""), r.player, r.map);
    }
    let med = if leads.is_empty() { 0.0 } else { let mut p: Vec<f64> = leads.iter().map(|l| l.1).collect(); p.sort_by(|a, b| a.partial_cmp(b).unwrap()); p[p.len() / 2] };
    println!("  ({} maps with a #1 and #2; median lead {:.2}%)", leads.len(), med);

    // late records: top-N positions taken late_years+ after the campaign
    println!("\nLATE RECORDS: top-{top} positions taken {late_years}+ years after the campaign was published, newest first (set / pos / time / vs #2 or #1 / player / map)");
    let mut late: Vec<&Rec> = recs
        .iter()
        .filter(|r| r.position <= top && r.time_ms > 0)
        .filter(|r| match (rec_campaign_year(r), stamp_year(&r.set_utc)) {
            (Some(c), Some(y)) => y - c >= late_years,
            _ => false,
        })
        .collect();
    late.sort_by(|a, b| b.set_utc.cmp(&a.set_utc));
    println!("  {} such records by {} accounts", late.len(), late.iter().map(|r| r.account_id.as_str()).collect::<std::collections::BTreeSet<_>>().len());
    for r in late.iter().take(60) {
        let others = by_map.get(r.uid.as_str()).cloned().unwrap_or_default();
        let neighbour = if r.position == 1 {
            others.iter().find(|o| o.position == 2).map(|o| format!("{} ahead of #2", secs(o.time_ms - r.time_ms)))
        } else {
            others.iter().find(|o| o.position == 1).map(|o| format!("{} behind #1", secs(r.time_ms - o.time_ms)))
        }
        .unwrap_or_default();
        println!("  {}  #{:<3} {:>8}  {:<22} {:<20} {}", r.set_utc.get(0..10).unwrap_or(""), r.position, secs(r.time_ms), neighbour, r.player, r.map);
    }
    // the accounts with the most late top-N records
    let mut late_per: BTreeMap<&str, (String, usize, usize)> = BTreeMap::new();
    for r in &late {
        let e = late_per.entry(r.account_id.as_str()).or_insert((r.player.clone(), 0, 0));
        e.1 += 1;
        if r.position == 1 {
            e.2 += 1;
        }
    }
    let mut lp: Vec<_> = late_per.iter().collect();
    lp.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
    println!("\n  accounts with the most late top-{top} records (late top-{top} / late #1s):");
    for (id, (name, n, ones)) in lp.iter().take(30) {
        println!("    {:<20} {:>4} {:>4}   {}", name, n, ones, id);
    }
    let _ = (&recs[0].zone, &recs[0].ghost_url, &recs[0].author_ms);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let what = args.first().map(String::as_str).unwrap_or("");
    let rest = &args[1.min(args.len())..];
    let pace: u64 = flag(rest, "--pace-ms").map(|v| v.parse().unwrap_or_else(|_| die("--pace-ms wants milliseconds"))).unwrap_or(1200);
    let length: usize = flag(rest, "--length").map(|v| v.parse().unwrap_or_else(|_| die("--length wants a number"))).unwrap_or(500);
    let only = flags(rest, "--campaign");
    let kind = Kind::parse(flag(rest, "--kind"));
    match what {
        "campaigns" => {
            let mut c = Client::new(pace);
            println!("id\tname\tmaps\tpublished_utc");
            for k in campaigns(&mut c, kind) {
                println!("{}\t{}\t{}\t{}", k.id, k.name, k.mapcount, k.published);
            }
        }
        "maps" => {
            let out = need(rest, "--out");
            let mut c = Client::new(pace);
            let rows = all_maps(&mut c, kind, &only);
            write_maps(out, &rows);
            eprintln!("wrote {out}: {} maps ({} requests)", rows.len(), c.requests);
        }
        "top" => {
            let maps = read_maps(need(rest, "--maps"));
            let out = PathBuf::from(need(rest, "--out"));
            let mut c = Client::new(pace);
            let sel = selected(&maps, &only);
            let failed = top(&mut c, &sel, &out, length);
            eprintln!("top {length} of {} maps -> {} ({} requests); {} page(s) failed", sel.len(), out.display(), c.requests, failed.len());
            for f in &failed {
                eprintln!("  failed: {f}");
            }
            if !failed.is_empty() {
                std::process::exit(1);
            }
        }
        "report" => {
            let path = rest.first().unwrap_or_else(|| die("tmlb report records.tsv [--top 10] [--late-years 2] [--players 40]"));
            let recs = read_records(path);
            if recs.is_empty() {
                die("no records");
            }
            let top: i64 = flag(rest, "--top").map(|v| v.parse().unwrap_or_else(|_| die("--top wants a number"))).unwrap_or(10);
            let late: f64 = flag(rest, "--late-years").map(|v| v.parse().unwrap_or_else(|_| die("--late-years wants years"))).unwrap_or(2.0);
            let players: usize = flag(rest, "--players").map(|v| v.parse().unwrap_or_else(|_| die("--players wants a number"))).unwrap_or(40);
            report(&recs, top, late, players);
        }
        "flatten" => {
            let dir = PathBuf::from(rest.first().unwrap_or_else(|| die("tmlb flatten DIR --maps maps.tsv --out records.tsv")));
            let maps = read_maps(need(rest, "--maps"));
            let out = need(rest, "--out");
            let (nm, nr) = flatten(&dir, &maps, out);
            eprintln!("wrote {out}: {nr} records from {nm} maps");
        }
        "sweep" => {
            let out = PathBuf::from(need(rest, "--out"));
            fs::create_dir_all(&out).unwrap_or_else(|e| die(format!("{}: {e}", out.display())));
            let maps_path = out.join("maps.tsv");
            let mut c = Client::new(pace);
            let rows = if maps_path.exists() {
                read_maps(maps_path.to_str().unwrap())
            } else {
                let rows = all_maps(&mut c, kind, &[]);
                write_maps(maps_path.to_str().unwrap(), &rows);
                eprintln!("{} maps in {} campaigns", rows.len(), rows.iter().map(|r| r.campaign_id).collect::<std::collections::BTreeSet<_>>().len());
                rows
            };
            let sel = selected(&rows, &only);
            let pages_dir = out.join("pages");
            let failed = top(&mut c, &sel, &pages_dir, length);
            let records = out.join("records.tsv");
            let (nm, nr) = flatten(&pages_dir, &rows, records.to_str().unwrap());
            eprintln!(
                "sweep done: {} requests; {nr} records from {nm} maps -> {}; {} page(s) failed",
                c.requests,
                records.display(),
                failed.len()
            );
            for f in &failed {
                eprintln!("  failed: {f}");
            }
            if !failed.is_empty() {
                std::process::exit(1);
            }
        }
        _ => die(
            "tmlb campaigns [--kind seasonal|totd|weekly]\n\
             tmlb maps --out maps.tsv [--kind seasonal|totd|weekly] [--campaign NAME]...\n\
             tmlb top --maps maps.tsv --out DIR [--length 500] [--pace-ms 1200] [--campaign NAME]...\n\
             tmlb flatten DIR --maps maps.tsv --out records.tsv\n\
             tmlb sweep --out DIR [--kind seasonal|totd|weekly] [--length 500] [--pace-ms 1200] [--campaign NAME]...\n\
             tmlb report records.tsv [--top 10] [--late-years 2] [--players 40]",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_chain_walks_to_world() {
        let z: Value = serde_json::from_str(
            r#"{"name":"South West England","parent":{"name":"England","parent":{"name":"United Kingdom","parent":{"name":"Europe","parent":{"name":"World"}}}}}"#,
        )
        .unwrap();
        assert_eq!(zone_chain(&z), "South West England|England|United Kingdom|Europe|World");
        assert_eq!(zone_chain(&Value::Null), "");
    }

    #[test]
    fn years_read_as_decimals() {
        assert_eq!(campaign_year("Summer 2021"), Some(2021.5));
        assert_eq!(campaign_year("Winter 2026"), Some(2026.0));
        assert_eq!(campaign_year("Weekly Shorts"), None);
        let y = stamp_year("2026-10-22T18:10:41+00:00").unwrap();
        assert!((y - 2026.807).abs() < 0.01, "{y}");
        assert_eq!(secs(43961), "43.961");
    }

    #[test]
    fn cells_never_break_a_row() {
        assert_eq!(cell("a\tb\r\nc"), "a b  c");
        let legacy = "h1\th2\th3\nx\tbroken\nname\t3\ny\twhole\t4\n";
        let rows = tsv_rows(legacy, 3);
        assert_eq!(rows, vec!["h1\th2\th3", "x\tbroken name\t3", "y\twhole\t4"]);
    }

    #[test]
    fn civil_dates_round_trip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 10, 1) * 86400, 1790812800);
        assert_eq!(ymd(1790812800), "2026-10-01");
        assert_eq!(ymd(1625151600), "2021-07-01"); // Summer 2021's publish stamp (15:00 UTC)
        assert_eq!(ymd(0), "");
    }

    #[test]
    fn repeated_flags_collect() {
        let a: Vec<String> = ["--campaign", "Summer 2021", "--campaign", "Fall 2020", "--length", "500"].iter().map(|s| s.to_string()).collect();
        assert_eq!(flags(&a, "--campaign"), vec!["Summer 2021", "Fall 2020"]);
        assert_eq!(flag(&a, "--length"), Some("500"));
    }
}
