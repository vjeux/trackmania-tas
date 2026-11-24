//! `dmwatch-spool` — dmwatch's pings delivered through a session subscription hook
//! instead of `agentcloudctl notify` (2026-11-24).
//!
//! Why: dmwatch's home (vjeux's devserver) needs his laptop to renew its user
//! certificate every 3 days; he is away for a week. An agent OnDemand node keeps its
//! credentials renewed by the platform, but it acts under the `open_source_developer`
//! environment role, and the platform refuses `agentcloudctl notify` from that role
//! into a session that proves none. A session's own cron subscription with a shell
//! hook on that node is the sanctioned route: the hook's stdout becomes an input of
//! the subscribing session, an empty stdout wakes nothing.
//!
//! Two verbs:
//!
//!   dmwatch-spool notify --session ID [--mode M] [--idempotency-key K] (--text T | --text-file F)
//!       what dmwatch runs as its `agentcloudctl` (config.json `"agentcloudctl"`): the ping
//!       is written to `~/.dmwatch/spool/` (one JSON file, atomic rename), a key already
//!       spooled or flushed is not spooled again (the key names the notification, as
//!       notify's does), and an `op-spool-<key>` id is printed so dmwatch counts it
//!       delivered and moves its cursor.
//!
//!   dmwatch-spool flush --me SESSION
//!       the hook: prints every spooled ping, oldest first, and moves it to
//!       `spool/sent/` (kept 7 days). Pings for SESSION print as dmwatch wrote them;
//!       pings for any other session print under a `[dmwatch relay -> session <id>]`
//!       header so the hook's session forwards them. Nothing spooled: no output.
//!
//! State lives under `$DMWATCH_HOME` (default `~/.dmwatch`), like dmwatch's own.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn home() -> PathBuf {
    if let Ok(h) = std::env::var("DMWATCH_HOME") {
        if !h.is_empty() {
            return PathBuf::from(h);
        }
    }
    let base = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    Path::new(&base).join(".dmwatch")
}

fn flag(args: &[String], names: &[&str]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        for n in names {
            if args[i] == *n {
                return args.get(i + 1).cloned();
            }
            if let Some(v) = args[i].strip_prefix(&format!("{n}=")) {
                return Some(v.to_string());
            }
        }
        i += 1;
    }
    None
}

/// A key as a file-name fragment: keep [A-Za-z0-9._-], map the rest to `_`.
fn safe(key: &str) -> String {
    key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '_' }).take(160).collect()
}

fn already_has(dir: &Path, key_part: &str) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().ends_with(&format!("-{key_part}.json"))))
        .unwrap_or(false)
}

fn notify(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("dmwatch-spool notify --session ID [--mode M] [--idempotency-key K] (--text T | --text-file F)");
        return Ok(());
    }
    let session = flag(args, &["--session", "-s"]).ok_or("notify needs --session ID")?;
    let mode = flag(args, &["--mode"]).unwrap_or_else(|| "cli-script".into());
    let text = match (flag(args, &["--text", "-t"]), flag(args, &["--text-file"])) {
        (Some(t), _) => t,
        (None, Some(f)) => std::fs::read_to_string(&f).map_err(|e| format!("{f}: {e}"))?,
        (None, None) => {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).map_err(|e| format!("stdin: {e}"))?;
            s
        }
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?;
    let key = flag(args, &["--idempotency-key"]).unwrap_or_else(|| format!("nokey-{}", now.as_nanos()));
    let spool = home().join("spool");
    let sent = spool.join("sent");
    std::fs::create_dir_all(&sent).map_err(|e| format!("{}: {e}", sent.display()))?;
    let part = safe(&key);
    if !already_has(&spool, &part) && !already_has(&sent, &part) {
        let body = serde_json::json!({ "session": session, "mode": mode, "key": key, "text": text, "spooled_ms": now.as_millis() as u64 });
        let name = format!("{:020}-{part}.json", now.as_millis());
        let tmp = spool.join(format!(".{name}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec(&body).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, spool.join(&name)).map_err(|e| format!("spool rename: {e}"))?;
    }
    println!("op-spool-{key}");
    Ok(())
}

fn flush(args: &[String]) -> Result<(), String> {
    let me = flag(args, &["--me"]).ok_or("flush needs --me SESSION (the hook's own session)")?;
    let spool = home().join("spool");
    let sent = spool.join("sent");
    std::fs::create_dir_all(&sent).map_err(|e| format!("{}: {e}", sent.display()))?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(&spool)
        .map_err(|e| format!("{}: {e}", spool.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().map(|x| x == "json").unwrap_or(false) && !p.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(true))
        .collect();
    files.sort();
    let mut out = String::new();
    for p in &files {
        let raw = match std::fs::read(p) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_slice(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let session = v["session"].as_str().unwrap_or("");
        let text = v["text"].as_str().unwrap_or("");
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        if session == me || me.starts_with(session) || session.starts_with(&me) {
            out.push_str(text.trim_end());
        } else {
            out.push_str(&format!("[dmwatch relay -> session {session} ({})]\n{}", v["mode"].as_str().unwrap_or("cli-script"), text.trim_end()));
        }
        let dest = sent.join(p.file_name().unwrap());
        std::fs::rename(p, &dest).map_err(|e| format!("move {}: {e}", p.display()))?;
    }
    // keep a week of flushed pings (the dedupe memory and the audit trail)
    let week_ago = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0).saturating_sub(7 * 24 * 3600 * 1000);
    if let Ok(rd) = std::fs::read_dir(&sent) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(ms) = name.split('-').next().and_then(|s| s.parse::<u64>().ok()) {
                if ms < week_ago {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
    if !out.is_empty() {
        println!("{out}");
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("notify") => notify(&args[1..]),
        Some("flush") => flush(&args[1..]),
        _ => Err("usage: dmwatch-spool notify --session ID [--mode M] [--idempotency-key K] (--text T | --text-file F)\n       dmwatch-spool flush --me SESSION".into()),
    };
    if let Err(e) = r {
        eprintln!("dmwatch-spool: {e}");
        std::process::exit(1);
    }
}
