//! `tinyctl tmio` — trackmania.io lookups (no auth; 40 requests/minute, honoured via the
//! `x-ratelimit-*` headers). Nadeo closed the core `accounts/displayNames` endpoint to game
//! tokens (401 "Invalid credentials", 2026-09-15), so display names come from here.
//!
//! ```text
//! tinyctl tmio names --ids-file ids.txt --out names.tsv          accountId → displayName (+ zone)
//! tinyctl tmio find  --people discord-people.tsv --out found.tsv  Discord person → accountId by exact name
//! ```
//! Both are resumable (rows already in --out are skipped) and pace themselves at ~1.6 s.

use std::collections::HashSet;
use std::io::Write;
use std::process::Command;
use std::time::Duration;

const UA: &str = "u10s-credits/1.0 (vjeux; Everios96 U10S map-1000 credits; discord vjeux)";

fn flag<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

/// GET → (body, http code, ratelimit-remaining, ratelimit-reset seconds)
fn get(url: &str) -> Result<(String, String, Option<u32>, Option<u32>), String> {
    let out = Command::new("curl").args(["-s", "--max-time", "40", "-A", UA, "-D", "-", url]).output().map_err(|e| format!("curl: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    // headers … blank line … body (curl -D - puts headers on stdout first)
    let (head, body) = match text.find("\r\n\r\n") {
        Some(i) => (&text[..i], &text[i + 4..]),
        None => match text.find("\n\n") {
            Some(i) => (&text[..i], &text[i + 2..]),
            None => ("", text.as_str()),
        },
    };
    let code = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("").to_string();
    let hdr = |k: &str| head.lines().find(|l| l.to_lowercase().starts_with(k)).and_then(|l| l.split(':').nth(1)).and_then(|v| v.trim().parse::<u32>().ok());
    Ok((body.to_string(), code, hdr("x-ratelimit-remaining"), hdr("x-ratelimit-reset")))
}

/// One paced request with 429 handling; returns the JSON body.
fn get_json(url: &str) -> Result<serde_json::Value, String> {
    for _ in 0..6 {
        let (body, code, remaining, reset) = get(url)?;
        if code == "429" || remaining == Some(0) {
            let wait = reset.unwrap_or(30) + 2;
            eprintln!("  rate limit — {wait} s");
            std::thread::sleep(Duration::from_secs(wait as u64));
            if code == "429" {
                continue;
            }
        }
        if code != "200" {
            return Err(format!("HTTP {code} for {url}: {}", body.chars().take(200).collect::<String>()));
        }
        if let Some(r) = remaining {
            // pace to the window: never burn the last few slots
            if r < 5 {
                std::thread::sleep(Duration::from_secs(reset.unwrap_or(10) as u64 + 1));
            }
        }
        return serde_json::from_str(&body).map_err(|e| format!("{url}: not JSON ({e}): {}", body.chars().take(120).collect::<String>()));
    }
    Err(format!("{url}: rate limited six times"))
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    match args.first().map(|s| s.as_str()) {
        Some("names") => names(&args[1..]),
        Some("find") => find(&args[1..]),
        _ => Err("tmio needs `names` or `find`".into()),
    }
}

fn names(args: &[String]) -> Result<(), String> {
    let ids_file = flag(args, "--ids-file").ok_or("--ids-file")?;
    let out = flag(args, "--out").ok_or("--out")?;
    let ids: Vec<String> = std::fs::read_to_string(ids_file).map_err(|e| format!("{ids_file}: {e}"))?.lines().map(|l| l.split('\t').next().unwrap_or("").trim().to_string()).filter(|s| s.len() > 30).collect();
    let done: HashSet<String> = std::fs::read_to_string(out).unwrap_or_default().lines().skip(1).filter_map(|l| l.split('\t').next().map(String::from)).collect();
    let mut w = std::fs::OpenOptions::new().create(true).append(true).open(out).map_err(|e| format!("{out}: {e}"))?;
    if done.is_empty() {
        writeln!(w, "accountId\tdisplayName\tzone\tclubTag").map_err(|e| e.to_string())?;
    }
    let todo: Vec<&String> = ids.iter().filter(|i| !done.contains(*i)).collect();
    eprintln!("{} ids, {} done, {} to resolve", ids.len(), done.len(), todo.len());
    for (k, id) in todo.iter().enumerate() {
        let v = match get_json(&format!("https://trackmania.io/api/player/{id}")) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("  {id}: {e}");
                writeln!(w, "{id}\t\t\t").map_err(|e| e.to_string())?;
                continue;
            }
        };
        let name = v["displayname"].as_str().unwrap_or("");
        let zone = v["trophies"]["zone"]["name"].as_str().or_else(|| v["zone"]["name"].as_str()).unwrap_or("");
        let tag = v["clubtag"].as_str().unwrap_or("");
        writeln!(w, "{id}\t{name}\t{zone}\t{}", tag.replace('\t', " ")).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        if k % 50 == 0 {
            eprintln!("[{:>5}/{}] {name}", k + 1, todo.len());
        }
        std::thread::sleep(Duration::from_millis(1550));
    }
    println!("{} names in {out}", done.len() + todo.len());
    Ok(())
}

fn find(args: &[String]) -> Result<(), String> {
    let people = flag(args, "--people").ok_or("--people discord-people.tsv")?;
    let out = flag(args, "--out").ok_or("--out")?;
    let text = std::fs::read_to_string(people).map_err(|e| format!("{people}: {e}"))?;
    let done: HashSet<String> = std::fs::read_to_string(out).unwrap_or_default().lines().skip(1).filter_map(|l| l.split('\t').next().map(String::from)).collect();
    let mut w = std::fs::OpenOptions::new().create(true).append(true).open(out).map_err(|e| format!("{out}: {e}"))?;
    if done.is_empty() {
        writeln!(w, "userId\tusername\tdiscordName\tmatchedName\taccountId\tcandidates").map_err(|e| e.to_string())?;
    }
    let rows: Vec<Vec<&str>> = text.lines().skip(1).map(|l| l.split('\t').collect::<Vec<&str>>()).filter(|c: &Vec<&str>| c.len() >= 3 && !done.contains(c[0])).collect();
    eprintln!("{} discord people to look up ({} done)", rows.len(), done.len());
    for (k, c) in rows.iter().enumerate() {
        let (uid, user, disp) = (c[0], c[1], c[2]);
        let mut matched = (String::new(), String::new());
        let mut cands: Vec<String> = Vec::new(); // the first names trackmania.io offered (for hand-mapping the misses)
        // exact (case-insensitive) hit on the display name first, then the username
        'outer: for q in [disp, user] {
            let q: &str = q.trim();
            if q.len() < 2 {
                continue;
            }
            let url = format!("https://trackmania.io/api/players/find?search={}", urlencode(q));
            let v = match get_json(&url) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("  {user}: {e}");
                    continue;
                }
            };
            let arr = v.as_array().cloned().unwrap_or_default();
            for r in arr.iter().take(4) {
                if let Some(n) = r["player"]["name"].as_str() {
                    if cands.len() < 6 && !cands.iter().any(|c| c == n) {
                        cands.push(n.to_string());
                    }
                }
            }
            for r in &arr {
                let n = r["player"]["name"].as_str().unwrap_or("");
                if n.eq_ignore_ascii_case(q) || n.to_lowercase() == q.to_lowercase() {
                    matched = (n.to_string(), r["player"]["id"].as_str().unwrap_or("").to_string());
                    break 'outer;
                }
            }
            std::thread::sleep(Duration::from_millis(1550));
        }
        writeln!(w, "{uid}\t{user}\t{disp}\t{}\t{}\t{}", matched.0, matched.1, cands.join("|")).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        if k % 25 == 0 {
            eprintln!("[{:>4}/{}] {user} -> {}", k + 1, rows.len(), if matched.1.is_empty() { "-" } else { matched.0.as_str() });
        }
        std::thread::sleep(Duration::from_millis(1550));
    }
    println!("done -> {out}");
    Ok(())
}

fn urlencode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => o.push(b as char),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}
