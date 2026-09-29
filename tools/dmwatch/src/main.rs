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
    if args.len() >= 3 && args[1] == "tick" {
        tick(&args[2..]);
        return;
    }
    if args.len() < 4 || args[1] != "read" {
        eprintln!("usage: dmwatch read CHAN SINCE-ISO-UTC [LIMIT]\n       dmwatch tick LABEL=CHAN:SINCE[:LIMIT] …   (all channels in one call; one failure never hides the others)");
        std::process::exit(2);
    }
    let chan = &args[2];
    let since = normalize(&args[3]);
    let limit: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(8);
    std::process::exit(read_one(chan, &since, limit, ""));
}

/// The cron tick as ONE call: every channel is read even when an earlier one fails
/// (a transient Discord 503 on the first DM used to end the whole shell loop and leave
/// eight channels unread, 2026-09-29 12:20). Each channel's lines carry its LABEL as a
/// prefix (empty label = none); a failed channel prints one `LABEL !! reason` line and
/// the tick goes on. Exit 0 if every channel read, 2 if any failed (the cron then knows
/// to retry that tick rather than call it "nothing new").
fn tick(specs: &[String]) {
    let mut failed = 0;
    for spec in specs {
        // LABEL=CHAN:SINCE[:LIMIT] — SINCE itself contains colons (`2026-09-24T10:10`), so
        // the spec is split as: CHAN = up to the first ':'; LIMIT = a trailing ':<digits>'
        // only if what follows the last ':' is all digits AND that segment is not part of
        // the time (a time segment is exactly 2 digits and the LIMIT field is optional);
        // everything between is SINCE. Bug found 2026-09-29 14:20: splitn(3, ':') cut
        // `2026-09-24T10:10` to `2026-09-24T10`, which normalized to nothing useful and
        // let a 10:06 message through a 10:10 anchor on every tick.
        let (label, rest) = spec.split_once('=').unwrap_or(("", spec.as_str()));
        let (chan, tail) = rest.split_once(':').unwrap_or((rest, ""));
        let (since_raw, limit) = match tail.rsplit_once(':') {
            // a trailing field that is 1–3 digits and NOT a 2-digit time segment → LIMIT
            Some((head, last)) if !last.is_empty() && last.len() != 2 && last.chars().all(|c| c.is_ascii_digit()) => {
                (head, last.parse().unwrap_or(8))
            }
            // `…T10:10:6` → head `…T10:10`, last `6`  (handled above); `…T10:10:00` → time
            _ => (tail, 8usize),
        };
        let since = normalize(if since_raw.is_empty() { "1970-01-01T00:00" } else { since_raw });
        let prefix = if label.is_empty() { String::new() } else { format!("{label} ") };
        if read_one(chan, &since, limit, &prefix) != 0 {
            failed += 1;
        }
    }
    std::process::exit(if failed == 0 { 0 } else { 2 });
}

/// Read one channel; prints its new messages (prefixed) and returns 0, or prints one
/// `PREFIX!! reason` line to stdout AND stderr and returns 2. Never panics on a bad
/// channel — the tick must survive one dead channel.
fn read_one(chan: &str, since: &str, limit: usize, prefix: &str) -> i32 {
    let discordctl = std::env::var("DISCORDCTL").unwrap_or_else(|_| "/home/vjeux/bin/discordctl".into());
    let fail = |why: String| -> i32 {
        println!("{prefix}!! {why}");
        eprintln!("dmwatch: {chan}: {why}");
        2
    };
    let out = match Command::new(&discordctl).args(["read", chan, "--limit", &limit.to_string(), "--json"]).output() {
        Ok(o) => o,
        Err(e) => return fail(format!("cannot run {discordctl}: {e}")),
    };
    if !out.status.success() {
        return fail(format!("discordctl read failed: {}", String::from_utf8_lossy(&out.stderr).trim().chars().take(160).collect::<String>()));
    }
    let mut msgs: Vec<serde_json::Value> = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => return fail(format!("discordctl output is not a JSON array: {e}")),
    };
    // discordctl gives newest-first or oldest-first depending on version; sort by timestamp
    msgs.sort_by(|a, b| ts(a).cmp(&ts(b)));
    for m in &msgs {
        let t = ts(m);
        if t.as_str() <= since {
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
        println!("{prefix}{stamp} {author}: {}", lines.next().unwrap_or(""));
        for l in lines {
            println!("{prefix}    {l}");
        }
        if let Some(atts) = m.get("attachments").and_then(|a| a.as_array()) {
            for a in atts {
                if let Some(u) = a.get("url").and_then(|u| u.as_str()) {
                    println!("{prefix}    ATT {u}");
                }
            }
        }
    }
    0
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
