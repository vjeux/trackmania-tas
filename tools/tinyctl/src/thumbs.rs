//! `tinyctl thumbs --list MAPS.tsv --outdir DIR [--frames 70] [--resume]` — ON THE BOX:
//! re-shoot the thumbnail of every map in the list from its own stored thumbnail
//! camera, in the game, as fast as the game loads maps.
//!
//! One map = `/thumbshot?map=1` (the plugin's state machine: PlayMap, wait for the
//! playground, arm the camera override + hide the HUD, count posed frames) →
//! poll `/thumbshot` until READY → one line to the resident capture server
//! (`\\.\pipe\tmcap`: game window client rect, square, 1024², flipped, JPEG) →
//! `/thumbshot?done=1` → poll until MENU. ~3.2 s per map measured 2026-09-14
//! (load 1.9–2.4 s, capture 0.1 s + 0.3 s pipe, back 0.4 s); nothing sleeps.
//!
//! Then `tmmaps set-thumbnail` splices the JPEG into the map's header; the
//! output map goes to `DIR/<same relative path>`; `DIR/thumbs.tsv` records each
//! map (path, uid, jpeg bytes, ms) and `--resume` skips the ones already done.
//! The list: `path` per line (a `.Map.Gbx` under the run's map root), `#` comments.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const PLUGIN: &str = "127.0.0.1:29800";
const STORE: &str = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
const SHOOT_DIR: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot";
const SHOOT_WIN: &str = "C:\\Users\\vjeux\\OneDrive\\Documents\\Trackmania\\Maps\\_shoot";
const CAP_DIR: &str = "/mnt/c/Users/vjeux/tinyshots/thumbs";
const CAP_WIN: &str = "C:\\Users\\vjeux\\tinyshots\\thumbs";

/// The plugin's address from WSL: SHOOT_HOST, then 127.0.0.1, then the Windows
/// host as WSL sees it (resolv.conf's nameserver) — the same ladder shootctl
/// climbs; the first that accepts a connection is kept for the run.
fn plugin_addr() -> &'static str {
    use std::sync::OnceLock;
    static ADDR: OnceLock<String> = OnceLock::new();
    ADDR.get_or_init(|| {
        let mut cands: Vec<String> = Vec::new();
        if let Ok(h) = std::env::var("SHOOT_HOST") { if !h.trim().is_empty() { cands.push(format!("{}:29800", h.trim())); } }
        cands.push(PLUGIN.to_string());
        if let Ok(rc) = std::fs::read_to_string("/etc/resolv.conf") {
            for line in rc.lines() { if let Some(ip) = line.strip_prefix("nameserver ") { cands.push(format!("{}:29800", ip.trim())); } }
        }
        // the default gateway (WSL2 NAT: the Windows host)
        if let Ok(out) = std::process::Command::new("ip").args(["route"]).output() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if let Some(rest) = line.strip_prefix("default via ") { if let Some(ip) = rest.split_whitespace().next() { cands.push(format!("{ip}:29800")); } }
            }
        }
        for c in &cands {
            if let Ok(sa) = c.parse::<std::net::SocketAddr>() {
                if std::net::TcpStream::connect_timeout(&sa, std::time::Duration::from_millis(500)).is_ok() { return c.clone(); }
            }
        }
        PLUGIN.to_string()
    })
}

fn http_get(path: &str) -> Result<String, String> {
    let addr = plugin_addr();
    let mut s = std::net::TcpStream::connect(addr).map_err(|e| format!("connect {addr}: {e}"))?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(10))).ok();
    s.set_write_timeout(Some(std::time::Duration::from_secs(10))).ok();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").as_bytes()).map_err(|e| format!("write: {e}"))?;
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).map_err(|e| format!("read: {e}"))?;
    let text = String::from_utf8_lossy(&buf).to_string();
    Ok(match text.find("\r\n\r\n") { Some(i) => text[i + 4..].trim().to_string(), None => text.trim().to_string() })
}

/// The capture client, RESIDENT: one PowerShell (`capclient2.ps1`) kept alive for the
/// run, a path in on stdin, the server's answer out on stdout — the one-shot client
/// cost ~600 ms of process start per capture (2026-09-14); this costs the pipe hop.
struct CapClient {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: std::io::BufReader<std::process::ChildStdout>,
}

impl CapClient {
    fn start() -> Result<CapClient, String> {
        let mut child = std::process::Command::new("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "C:\\Users\\vjeux\\capclient2.ps1"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("capclient2: {e}"))?;
        let stdin = child.stdin.take().ok_or("capclient2: no stdin")?;
        let stdout = std::io::BufReader::new(child.stdout.take().ok_or("capclient2: no stdout")?);
        Ok(CapClient { child, stdin, stdout })
    }
    fn capture(&mut self, win_path: &str) -> Result<String, String> {
        use std::io::BufRead;
        writeln!(self.stdin, "{win_path}").map_err(|e| format!("capclient2 write: {e}"))?;
        self.stdin.flush().ok();
        let mut line = String::new();
        self.stdout.read_line(&mut line).map_err(|e| format!("capclient2 read: {e}"))?;
        let text = line.trim().to_string();
        if !text.starts_with("ok") {
            return Err(format!("capture: {text}"));
        }
        Ok(text)
    }
}

impl Drop for CapClient {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.child.wait();
    }
}

fn poll_until(prefixes: &[&str], timeout_ms: u64) -> Result<String, String> {
    let t0 = Instant::now();
    let mut last = String::new();
    while t0.elapsed().as_millis() < timeout_ms as u128 {
        last = http_get("/thumbshot").unwrap_or_default();
        if prefixes.iter().any(|p| last.starts_with(p)) {
            return Ok(last);
        }
        if last.starts_with("ERR") {
            return Err(last);
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    Err(format!("timeout after {timeout_ms} ms; last {last}"))
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let list = PathBuf::from(f("--list").ok_or("thumbs needs --list MAPS.tsv (one map path per line)")?);
    let outdir = PathBuf::from(f("--outdir").ok_or("thumbs needs --outdir DIR")?);
    let root = f("--root").map(PathBuf::from);
    let frames: u32 = f("--frames").and_then(|s| s.parse().ok()).unwrap_or(30);
    // wall time the pose must hold before the shot (the map-load fade): 2.4 s measured
    let ready_ms: u32 = f("--ready-ms").and_then(|s| s.parse().ok()).unwrap_or(2400);
    let resume = tmmaps::cli::has(args, "--resume");
    let limit: usize = f("--limit").and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let tmmaps_bin = f("--tmmaps").unwrap_or_else(|| "/home/vjeux/shoot/fix4/target/release/tmmaps".into());
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    std::fs::create_dir_all(CAP_DIR).map_err(|e| format!("{CAP_DIR}: {e}"))?;
    std::fs::create_dir_all(SHOOT_DIR).map_err(|e| format!("{SHOOT_DIR}: {e}"))?;
    let log_path = outdir.join("thumbs.tsv");
    let mut done: std::collections::HashSet<String> = std::collections::HashSet::new();
    if resume {
        if let Ok(t) = std::fs::read_to_string(&log_path) {
            for l in t.lines().skip(1) {
                if let Some(p) = l.split('\t').next() {
                    if l.split('\t').nth(4).map(|s| s == "ok").unwrap_or(false) {
                        done.insert(p.to_string());
                    }
                }
            }
        }
    }
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
    if log.metadata().map(|m| m.len() == 0).unwrap_or(true) {
        writeln!(log, "path\tuid\tjpeg_bytes\tms\tstatus\tdetail").map_err(|e| e.to_string())?;
    }
    let maps: Vec<String> = std::fs::read_to_string(&list).map_err(|e| format!("{}: {e}", list.display()))?.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    println!("{} maps listed, {} already done", maps.len(), done.len());
    // sanity: the plugin answers and the capture server is up
    let st = http_get("/thumbshot")?;
    if !(st.starts_with("IDLE") || st.starts_with("MENU")) {
        let _ = http_get("/thumbshot?done=1");
        poll_until(&["MENU", "IDLE"], 20_000)?;
    }
    let mut cap_client = CapClient::start()?;
    cap_client.capture(&format!("{CAP_WIN}\\_probe.jpg"))?;
    let t_run = Instant::now();
    let mut n_ok = 0usize;
    let mut n_fail = 0usize;
    for (k, m) in maps.iter().enumerate() {
        if n_ok + n_fail >= limit {
            break;
        }
        if done.contains(m) {
            continue;
        }
        let t0 = Instant::now();
        let src = match &root { Some(r) => r.join(m), None => PathBuf::from(m) };
        let rel = match &root { Some(r) => src.strip_prefix(r).unwrap_or(&src).to_path_buf(), None => PathBuf::from(Path::new(m).file_name().unwrap_or_default()) };
        let out_map = outdir.join(&rel);
        let res = (|| -> Result<(String, usize), String> {
            let name = src.file_name().and_then(|n| n.to_str()).ok_or("no file name")?.to_string();
            // stage under a SHORT ascii name: the game path goes through editmap.txt verbatim
            let staged = format!("{SHOOT_DIR}/thumb-{k}.Map.Gbx");
            std::fs::copy(&src, &staged).map_err(|e| format!("stage {}: {e}", src.display()))?;
            std::fs::write(format!("{STORE}/editmap.txt"), format!("{SHOOT_WIN}\\thumb-{k}.Map.Gbx")).map_err(|e| e.to_string())?;
            let a = http_get(&format!("/thumbshot?map=1&frames={frames}&ms={ready_ms}"))?;
            if !a.starts_with("LOADING") {
                return Err(format!("load refused: {a}"));
            }
            let ready = poll_until(&["READY"], 60_000)?;
            let jpg_win = format!("{CAP_WIN}\\thumb-{k}.jpg");
            let jpg_wsl = format!("{CAP_DIR}/thumb-{k}.jpg");
            // the map-load FADE: the first posed frames are under a dark veil that
            // lifts over ~1 s. The capture reports the frame's mean luminance; shoot
            // again while it is still rising (two captures 250 ms apart agreeing
            // within 2 = stable), at most 8 times. Measured 2026-09-14: the veiled
            // frame reads ~45, the clear one ~110 on the same map.
            let mut cap = cap_client.capture(&jpg_win)?;
            let lum = |c: &str| c.split("lum=").nth(1).and_then(|r| r.split(' ').next()).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
            let mut retries = 0;
            // ONE confirmation only when the frame could still be under the fade
            // (the plugin's READY already waited `ready_ms` after the pose): a second
            // capture 150 ms later that is brighter by > 3 means the veil was still
            // lifting — take a third; a stable pair is final.
            let mut prev = lum(&cap);
            for _ in 0..3 {
                if prev >= 140 && retries == 0 {
                    break; // a bright frame is never a veiled one (the veil caps ~120)
                }
                std::thread::sleep(std::time::Duration::from_millis(150));
                let c2 = cap_client.capture(&jpg_win)?;
                let l2 = lum(&c2);
                cap = c2;
                retries += 1;
                if (l2 - prev).abs() <= 3 {
                    break;
                }
                prev = l2;
            }
            let cap = format!("{cap} retries={retries}");
            let _ = http_get("/thumbshot?done=1");
            let jpeg = std::fs::read(&jpg_wsl).map_err(|e| format!("{jpg_wsl}: {e}"))?;
            if jpeg.len() < 20_000 || !jpeg.starts_with(&[0xFF, 0xD8]) {
                return Err(format!("capture too small or not a JPEG ({} bytes)", jpeg.len()));
            }
            // splice while the game goes back to the menu
            if let Some(parent) = out_map.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let sp = std::process::Command::new(&tmmaps_bin).arg("set-thumbnail").arg(&src).arg("--jpeg").arg(&jpg_wsl).arg("--out").arg(&out_map).output().map_err(|e| format!("tmmaps: {e}"))?;
            if !sp.status.success() {
                return Err(format!("set-thumbnail: {}", String::from_utf8_lossy(&sp.stderr).trim()));
            }
            poll_until(&["MENU", "IDLE"], 30_000)?;
            let _ = std::fs::remove_file(&staged);
            let _ = std::fs::remove_file(&jpg_wsl);
            let _ = name;
            Ok((format!("{ready} | {cap}"), jpeg.len()))
        })();
        let ms = t0.elapsed().as_millis();
        match res {
            Ok((detail, bytes)) => {
                n_ok += 1;
                let uid = tmmaps::header::read(out_map.to_str().unwrap_or_default()).map(|h| h.uid).unwrap_or_default();
                writeln!(log, "{m}\t{uid}\t{bytes}\t{ms}\tok\t{}", detail.replace('\t', " ")).map_err(|e| e.to_string())?;
                println!("[{:>4}/{}] ok {ms:5} ms  {}", k + 1, maps.len(), rel.display());
            }
            Err(e) => {
                n_fail += 1;
                writeln!(log, "{m}\t\t0\t{ms}\tFAIL\t{}", e.replace('\t', " ")).map_err(|e| e.to_string())?;
                eprintln!("[{:>4}/{}] FAIL {ms:5} ms  {}: {e}", k + 1, maps.len(), rel.display());
                // recover the game state before the next map
                let _ = http_get("/thumbshot?done=1");
                let _ = poll_until(&["MENU", "IDLE"], 30_000);
            }
        }
        let _ = log.flush();
    }
    println!("{n_ok} ok, {n_fail} failed in {:.0} s ({:.2} s/map)", t_run.elapsed().as_secs_f64(), t_run.elapsed().as_secs_f64() / (n_ok + n_fail).max(1) as f64);
    Ok(())
}
