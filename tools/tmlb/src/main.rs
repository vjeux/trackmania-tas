//! tmlb -- the leaderboards of Nadeo's seasonal campaigns, from trackmania.io.
//!
//!   tmlb campaigns                         the seasonal campaigns (id, name, maps, published)
//!   tmlb maps --out maps.tsv [--campaign "Summer 2021"]...
//!                                          every map of every seasonal campaign
//!   tmlb top --maps maps.tsv --out DIR [--length 500] [--pace-ms 1200] [--campaign NAME]...
//!                                          the top N of each map: raw pages of 100 in
//!                                          DIR/<uid>/<offset>.json; a page already on disk is
//!                                          never fetched again, so a stopped sweep resumes
//!   tmlb flatten DIR --maps maps.tsv --out records.tsv
//!                                          one row a record: campaign, map, uid, position,
//!                                          time_ms, account_id, player, zone chain, set_utc,
//!                                          ghost_url
//!   tmlb sweep --out DIR [--length 500] [--pace-ms 1200] [--campaign NAME]...
//!                                          campaigns -> maps -> top -> flatten, in one go
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

#[derive(Clone, Debug)]
struct Campaign {
    id: i64,
    name: String,
    mapcount: i64,
    published: i64,
}

fn campaigns(c: &mut Client) -> Vec<Campaign> {
    let mut out = Vec::new();
    let mut page = 0;
    loop {
        let v = c.get_json(&format!("{BASE}/campaigns/seasonal/{page}")).unwrap_or_else(|e| die(e));
        for k in v["campaigns"].as_array().cloned().unwrap_or_default() {
            if k["seasonal"].as_bool() != Some(true) {
                continue;
            }
            out.push(Campaign {
                id: k["id"].as_i64().unwrap_or(0),
                name: k["name"].as_str().unwrap_or("").to_string(),
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

#[derive(Clone, Debug)]
struct MapRow {
    campaign: String,
    campaign_id: i64,
    index: usize,
    name: String,
    uid: String,
    map_id: String,
    author_ms: i64,
}

const MAPS_HEADER: &str = "campaign\tcampaign_id\tindex\tname\tuid\tmap_id\tauthor_ms";

fn maps_of(c: &mut Client, k: &Campaign) -> Vec<MapRow> {
    let v = c.get_json(&format!("{BASE}/campaign/seasonal/{}", k.id)).unwrap_or_else(|e| die(e));
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
            name: m["name"].as_str().unwrap_or("").to_string(),
            uid: m["mapUid"].as_str().unwrap_or("").to_string(),
            map_id: m["mapId"].as_str().unwrap_or("").to_string(),
            author_ms: m["authorScore"].as_i64().unwrap_or(0),
        })
        .collect()
}

fn write_maps(path: &str, rows: &[MapRow]) {
    let mut s = String::from(MAPS_HEADER);
    s.push('\n');
    for r in rows {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.campaign, r.campaign_id, r.index, r.name, r.uid, r.map_id, r.author_ms
        ));
    }
    fs::write(path, s).unwrap_or_else(|e| die(format!("{path}: {e}")));
}

fn read_maps(path: &str) -> Vec<MapRow> {
    let s = fs::read_to_string(path).unwrap_or_else(|e| die(format!("{path}: {e}")));
    s.lines()
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
    "campaign\tmap_index\tmap\tuid\tmap_id\tauthor_ms\tposition\ttime_ms\taccount_id\tplayer\tzone\tset_utc\tghost_url";

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
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    mr.campaign,
                    mr.index,
                    mr.name,
                    mr.uid,
                    mr.map_id,
                    mr.author_ms,
                    t["position"].as_i64().unwrap_or(0),
                    t["time"].as_i64().unwrap_or(0),
                    t["player"]["id"].as_str().unwrap_or(""),
                    t["player"]["name"].as_str().unwrap_or("").replace('\t', " "),
                    zone_chain(&t["player"]["zone"]),
                    t["timestamp"].as_str().unwrap_or(""),
                    t["url"].as_str().map(|u| format!("https://trackmania.io{u}")).unwrap_or_default()
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let what = args.first().map(String::as_str).unwrap_or("");
    let rest = &args[1.min(args.len())..];
    let pace: u64 = flag(rest, "--pace-ms").map(|v| v.parse().unwrap_or_else(|_| die("--pace-ms wants milliseconds"))).unwrap_or(1200);
    let length: usize = flag(rest, "--length").map(|v| v.parse().unwrap_or_else(|_| die("--length wants a number"))).unwrap_or(500);
    let only = flags(rest, "--campaign");
    match what {
        "campaigns" => {
            let mut c = Client::new(pace);
            println!("id\tname\tmaps\tpublished_utc");
            for k in campaigns(&mut c) {
                println!("{}\t{}\t{}\t{}", k.id, k.name, k.mapcount, k.published);
            }
        }
        "maps" => {
            let out = need(rest, "--out");
            let mut c = Client::new(pace);
            let mut rows = Vec::new();
            for k in campaigns(&mut c).iter().filter(|k| only.is_empty() || only.iter().any(|o| o.eq_ignore_ascii_case(&k.name))) {
                rows.extend(maps_of(&mut c, k));
            }
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
                let mut rows = Vec::new();
                for k in campaigns(&mut c) {
                    rows.extend(maps_of(&mut c, &k));
                }
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
            "tmlb campaigns\n\
             tmlb maps --out maps.tsv [--campaign NAME]...\n\
             tmlb top --maps maps.tsv --out DIR [--length 500] [--pace-ms 1200] [--campaign NAME]...\n\
             tmlb flatten DIR --maps maps.tsv --out records.tsv\n\
             tmlb sweep --out DIR [--length 500] [--pace-ms 1200] [--campaign NAME]...",
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
    fn repeated_flags_collect() {
        let a: Vec<String> = ["--campaign", "Summer 2021", "--campaign", "Fall 2020", "--length", "500"].iter().map(|s| s.to_string()).collect();
        assert_eq!(flags(&a, "--campaign"), vec!["Summer 2021", "Fall 2020"]);
        assert_eq!(flag(&a, "--length"), Some("500"));
    }
}
