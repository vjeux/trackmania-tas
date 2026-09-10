//! Talking to the render box: `~/bin/wsx` (the WhiteStick bridge client, a
//! Rust binary in `tools/wsx`) run as a subprocess — `push`, `pull`, `sh` —
//! plus the one pattern every long job needs: start it detached over there,
//! poll for its done file here.
//!
//! The long-running side (`shootctl shootset`, `tinyctl publish-here`) is a
//! program that detaches itself and writes a done file, and this side only
//! ever sends short commands — and FEW of them: the bridge's calls are the
//! cost (the navi bridge metered them against a daily quota; ours does not,
//! but the fleet's ceiling of two calls a minute per loop stands), so a poll
//! is one call per 30 s and a file is one streamed call.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

pub struct Wsx {
    pub bin: String,
    pub verbose: bool,
}

impl Wsx {
    pub fn new(args: &[String]) -> Wsx {
        let bin = tmmaps::cli::flag(args, "--wsx").map(String::from).unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into());
            format!("{home}/bin/wsx")
        });
        Wsx { bin, verbose: tmmaps::cli::has(args, "-v") }
    }

    fn run(&self, args: &[&str], timeout: Duration) -> Result<String, String> {
        if self.verbose {
            eprintln!("+ wsx {}", args.join(" "));
        }
        let mut child = Command::new(&self.bin)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("{}: {e}", self.bin))?;
        // Drain stdout/stderr on threads: a pull's answer can be megabytes and
        // a child left with a full pipe looks exactly like a stalled bridge.
        let mut so = child.stdout.take().unwrap();
        let mut se = child.stderr.take().unwrap();
        let ho = std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = std::io::Read::read_to_end(&mut so, &mut v);
            v
        });
        let he = std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = std::io::Read::read_to_end(&mut se, &mut v);
            v
        });
        let t0 = Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
                break s;
            }
            if t0.elapsed() > timeout {
                let _ = child.kill();
                return Err(format!("wsx {} timed out after {}s", args.first().unwrap_or(&""), timeout.as_secs()));
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let out = String::from_utf8_lossy(&ho.join().unwrap_or_default()).into_owned();
        let err = String::from_utf8_lossy(&he.join().unwrap_or_default()).into_owned();
        if !status.success() {
            return Err(format!("wsx {} failed ({status}): {}{}", args.first().unwrap_or(&""), out.trim(), err.trim()));
        }
        Ok(out)
    }

    /// Run one command line on the box (`/bin/sh -c` there — keep it to ONE
    /// program invocation; logic belongs in the program).
    pub fn sh(&self, cmd: &str) -> Result<String, String> {
        self.run(&["sh", cmd], Duration::from_secs(150))
    }

    pub fn push(&self, local: &Path, remote: &str) -> Result<(), String> {
        let l = local.to_str().ok_or("path is not utf-8")?;
        let out = self.run(&["push", l, remote], Duration::from_secs(900))?;
        if self.verbose {
            eprintln!("{}", out.trim());
        }
        Ok(())
    }

    pub fn pull(&self, remote: &str, local: &Path) -> Result<u64, String> {
        let l = local.to_str().ok_or("path is not utf-8")?;
        self.run(&["pull", remote, l], Duration::from_secs(900))?;
        std::fs::metadata(local).map(|m| m.len()).map_err(|e| format!("{}: {e}", local.display()))
    }

    /// Read a small remote text file; `None` when it is not there yet.
    pub fn cat(&self, remote: &str) -> Option<String> {
        // `cat` exits 1 on a missing file and wsx reports the failure — that
        // is the "not yet" signal.
        self.run(&["sh", &format!("cat '{remote}'")], Duration::from_secs(60)).ok()
    }

    /// Poll for a done file written by a detached job. Prints the log's tail
    /// on failure.
    ///
    /// ONE bridge call every 30 s, carrying the done file (if it exists) AND
    /// the log's last line together — two calls a minute, the ceiling the fleet
    /// agreed on after a watcher polling every 12 s (plus a separate log read
    /// each minute) burned the bridge's daily quota for every session on
    /// 2026-09-09. A render is 4–9 minutes; 30 s of latency on its end is
    /// nothing next to that.
    pub fn wait_done(&self, done: &str, log: &str, timeout: Duration, what: &str) -> Result<String, String> {
        let t0 = Instant::now();
        let mut last_note = Instant::now();
        loop {
            let probe = format!("if [ -f '{done}' ]; then printf 'DONE:'; tr '\\n' ' ' < '{done}'; fi; printf '\\nLOG:'; tail -n 1 '{log}' 2>/dev/null");
            let out = self.run(&["sh", &probe], Duration::from_secs(60)).unwrap_or_default();
            let mut done_text: Option<String> = None;
            let mut log_line = String::new();
            for l in out.lines() {
                if let Some(d) = l.strip_prefix("DONE:") {
                    done_text = Some(d.trim().to_string());
                } else if let Some(g) = l.strip_prefix("LOG:") {
                    log_line = g.to_string();
                }
            }
            if let Some(text) = done_text {
                if text.starts_with("OK") {
                    return Ok(text);
                }
                let tail = self.cat(log).unwrap_or_default();
                let tail: String = tail.lines().rev().take(25).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
                return Err(format!("{what}: {}\n--- log tail ---\n{tail}", text.trim()));
            }
            if t0.elapsed() > timeout {
                let tail = self.cat(log).unwrap_or_default();
                let tail: String = tail.lines().rev().take(25).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
                return Err(format!("{what}: no done file after {}s\n--- log tail ---\n{tail}", timeout.as_secs()));
            }
            if last_note.elapsed() > Duration::from_secs(60) && !log_line.is_empty() {
                last_note = Instant::now();
                eprintln!("  [{:>4.0}s] {what}: {log_line}", t0.elapsed().as_secs_f64());
            }
            std::thread::sleep(Duration::from_secs(30));
        }
    }
}

/// The remote's WSL path for a Windows-side file and back.
pub fn to_win(p: &str) -> String {
    match p.strip_prefix("/mnt/c/") {
        Some(rest) => format!("C:/{rest}"),
        None => p.to_string(),
    }
}
