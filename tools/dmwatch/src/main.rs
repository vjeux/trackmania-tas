//! `dmwatch read CHAN SINCE [LIMIT]` — the read half of the Altered U10S Discord watch.
//!
//! Prints every message in channel CHAN newer than SINCE (ISO-8601 UTC, `2026-09-29T14:00`
//! or `…T14:00:00`), oldest first, as `MM-DD HH:MM author: text` with continuation lines
//! indented (whole multi-line messages, never truncated) and one `    ATT <url>` line per
//! attachment. Messages whose last line is our own sign-off (`-- ai`) are skipped, so a
//! tick sees only what OTHERS wrote. LIMIT (default 8) is how many messages to fetch
//! before the time filter — raise it when a window is busy.
//!
//! History: the watch ran as `shoot/hunt/dmread.py` on the box until 2026-09-29 07:34,
//! when a disk cleanup deleted the whole `shoot/hunt` tree (189 GB of old shoots — plus
//! the only copy of that script). This is its replacement, in Rust per the standing rule,
//! installed OUTSIDE any scratch tree (`/home/vjeux/tools/dmwatch/`) so a cleanup never
//! takes it again. It shells out to `discordctl read --json` (the Rust Discord client
//! already on the box; token in `~/.discordctl/token`), so it needs no token of its own.
//!
//! Exit code: 0 with output, 0 with no output when nothing is new, 2 on a discordctl
//! failure (printed to stderr — the cron treats that as "box/Discord unreachable").

use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 || args[1] != "read" {
        eprintln!("usage: dmwatch read CHAN SINCE-ISO-UTC [LIMIT]");
        std::process::exit(2);
    }
    let chan = &args[2];
    let since = normalize(&args[3]);
    let limit: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(8);
    let discordctl = std::env::var("DISCORDCTL").unwrap_or_else(|_| "/home/vjeux/bin/discordctl".into());
    let out = match Command::new(&discordctl).args(["read", chan, "--limit", &limit.to_string(), "--json"]).output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("dmwatch: cannot run {discordctl}: {e}");
            std::process::exit(2);
        }
    };
    if !out.status.success() {
        eprintln!("dmwatch: discordctl read {chan} failed: {}", String::from_utf8_lossy(&out.stderr).trim());
        std::process::exit(2);
    }
    let msgs: Vec<serde_json::Value> = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("dmwatch: discordctl output is not a JSON array: {e}");
            std::process::exit(2);
        }
    };
    // discordctl gives newest-first or oldest-first depending on version; sort by timestamp
    let mut msgs = msgs;
    msgs.sort_by(|a, b| ts(a).cmp(&ts(b)));
    for m in &msgs {
        let t = ts(m);
        if t.as_str() <= since.as_str() {
            continue;
        }
        let content = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
        if content.trim_end().lines().last().map(|l| l.trim() == "-- ai").unwrap_or(false) {
            continue; // our own post
        }
        let author = m
            .get("author")
            .and_then(|a| a.get("global_name").and_then(|g| g.as_str()).or_else(|| a.get("username").and_then(|u| u.as_str())))
            .unwrap_or("?");
        // 2026-09-29T14:30:14.841000+00:00 → 09-29 14:30
        let stamp = format!("{} {}", &t.get(5..10).unwrap_or("??-??"), &t.get(11..16).unwrap_or("??:??"));
        let mut lines = content.lines();
        println!("{stamp} {author}: {}", lines.next().unwrap_or(""));
        for l in lines {
            println!("    {l}");
        }
        if let Some(atts) = m.get("attachments").and_then(|a| a.as_array()) {
            for a in atts {
                if let Some(u) = a.get("url").and_then(|u| u.as_str()) {
                    println!("    ATT {u}");
                }
            }
        }
    }
}

fn ts(m: &serde_json::Value) -> String {
    m.get("timestamp").and_then(|t| t.as_str()).unwrap_or("").to_string()
}

/// `2026-09-29T14:00` → `2026-09-29T14:00:00` so a plain string compare against the
/// API's `2026-09-29T14:30:14.841000+00:00` works (same prefix shape up to the seconds).
fn normalize(s: &str) -> String {
    let s = s.trim();
    match s.len() {
        16 => format!("{s}:00"),
        10 => format!("{s}T00:00:00"),
        _ => s.to_string(),
    }
}
