//! Talking to the render box: `~/bin/wsx` (the WhiteStick bridge client, a
//! Rust binary in `tools/wsx`) run as a subprocess — `push`, `pull`, `sh` —
//! plus the one pattern every long job needs: start it detached over there,
//! poll for its done file here.
//!
//! The bridge cuts a command at ~90 s and forwards no stdin, which is why the
//! long-running side (`shootctl shootset`, `tinyctl publish-here`) is a
//! program that detaches itself and writes a done file, and this side only
//! ever sends short commands.

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
    pub fn wait_done(&self, done: &str, log: &str, timeout: Duration, what: &str) -> Result<String, String> {
        let t0 = Instant::now();
        let mut last_note = Instant::now();
        loop {
            if let Some(text) = self.cat(done) {
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
            if last_note.elapsed() > Duration::from_secs(60) {
                last_note = Instant::now();
                let tail = self.cat(log).unwrap_or_default();
                if let Some(l) = tail.lines().last() {
                    eprintln!("  [{:>4.0}s] {what}: {l}", t0.elapsed().as_secs_f64());
                }
            }
            std::thread::sleep(Duration::from_secs(12));
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
