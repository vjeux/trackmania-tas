//! The log file: one writer thread, size-based rotation, and `tail`.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::clock;

#[derive(Clone)]
pub struct Logger {
    tx: Sender<String>,
    echo: bool,
}

impl Logger {
    /// Start the writer thread. `echo` also prints every line to stdout.
    pub fn start(path: PathBuf, max_bytes: u64, keep: usize, echo: bool) -> io::Result<Logger> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let (tx, rx) = mpsc::channel::<String>();
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        thread::Builder::new().name("log-writer".into()).spawn(move || writer(rx, &mut file, &path, max_bytes, keep, echo))?;
        Ok(Logger { tx, echo })
    }

    /// A line from the tool itself.
    pub fn note(&self, msg: &str) {
        self.raw(format!("[{}] [tmserverctl] {msg}", clock::now_utc()));
    }

    /// A line the server printed.
    pub fn server(&self, line: &str) {
        self.raw(format!("[{}] {line}", clock::now_utc()));
    }

    fn raw(&self, line: String) {
        if self.tx.send(line.clone()).is_err() && self.echo {
            println!("{line}");
        }
    }
}

fn writer(rx: Receiver<String>, file: &mut File, path: &Path, max_bytes: u64, keep: usize, echo: bool) {
    let mut size = file.metadata().map(|m| m.len()).unwrap_or(0);
    for line in rx {
        if echo {
            println!("{line}");
        }
        let _ = writeln!(file, "{line}");
        size += line.len() as u64 + 1;
        if size > max_bytes {
            let _ = file.flush();
            rotate(path, keep);
            match OpenOptions::new().create(true).append(true).open(path) {
                Ok(f) => {
                    *file = f;
                    size = 0;
                }
                Err(e) => {
                    eprintln!("tmserverctl: cannot reopen {}: {e}", path.display());
                }
            }
        }
    }
}

/// server.log -> server.log.1 -> ... -> server.log.<keep>; the oldest falls off.
fn rotate(path: &Path, keep: usize) {
    if keep == 0 {
        let _ = fs::remove_file(path);
        return;
    }
    let numbered = |n: usize| PathBuf::from(format!("{}.{n}", path.display()));
    let _ = fs::remove_file(numbered(keep));
    for n in (1..keep).rev() {
        let _ = fs::rename(numbered(n), numbered(n + 1));
    }
    let _ = fs::rename(path, numbered(1));
}

/// Print the last `lines` lines; with `follow`, keep printing new ones until
/// `stop` is set (Ctrl-C) or the file is rotated away and recreated.
pub fn tail(path: &Path, lines: usize, follow: bool, stop: Arc<AtomicBool>) -> io::Result<()> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            println!("(no log yet: {})", path.display());
            if !follow {
                return Ok(());
            }
            wait_for_file(path, &stop);
            File::open(path)?
        }
        Err(e) => return Err(e),
    };
    let len = file.metadata()?.len();
    // Read the tail in one chunk (enough for a few hundred lines), then split.
    let window = (lines as u64 * 400).min(len).max(1);
    file.seek(SeekFrom::Start(len - window))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    let all: Vec<&str> = buf.lines().collect();
    let skip_first = window < len && !all.is_empty(); // a partial first line
    let start = all.len().saturating_sub(lines + skip_first as usize);
    for l in &all[start + skip_first as usize..] {
        println!("{l}");
    }
    if !follow {
        return Ok(());
    }
    let mut pos = len;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    while !stop.load(Ordering::Relaxed) {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // Nothing new: rotated? A shorter file means a new one.
                match fs::metadata(path) {
                    Ok(m) if m.len() < pos => {
                        if let Ok(f) = File::open(path) {
                            reader = BufReader::new(f);
                            pos = 0;
                        }
                    }
                    _ => {}
                }
                thread::sleep(Duration::from_millis(250));
            }
            Ok(n) => {
                pos += n as u64;
                print!("{line}");
                let _ = io::stdout().flush();
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn wait_for_file(path: &Path, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) && !path.exists() {
        thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_and_keeps_n() {
        let dir = std::env::temp_dir().join(format!("tmserverctl-log-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("server.log");
        let log = Logger::start(path.clone(), 200, 2, false).unwrap();
        for i in 0..40 {
            log.server(&format!("line {i} ................................................"));
        }
        drop(log);
        // Let the writer drain.
        for _ in 0..50 {
            if dir.join("server.log.2").exists() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(path.exists());
        assert!(dir.join("server.log.1").exists());
        assert!(dir.join("server.log.2").exists());
        assert!(!dir.join("server.log.3").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
