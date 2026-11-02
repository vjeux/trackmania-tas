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
    // ONE GAME, ONE DRIVER (tmdrive): the plugin refuses a mutating route without the
    // driver's lock token; `tmdrive run … -- tinyctl thumbs …` hands it down as
    // TM_LOCK_TOKEN (the plugin ignores it on reads) — 2026-11-02
    let path: String = match std::env::var("TM_LOCK_TOKEN").ok().filter(|t| !t.is_empty()) {
        Some(t) => format!("{path}{}token={t}", if path.contains('?') { '&' } else { '?' }),
        None => path.to_string(),
    };
    let path = path.as_str();
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

/// Is the plugin answering? (The game crashed on U10S_606 [Plastic] mid-load,
/// 2026-09-15 05:08Z — the run must survive that.)
fn plugin_alive() -> bool {
    http_get("/ctx").map(|c| c.contains("\"ctx\"")).unwrap_or(false)
}

/// Bring the game back: `shootctl launch` (Steam/Ubisoft launcher, waits for the
/// plugin and the title), then the capture server is re-checked by the caller.
fn relaunch_game() -> Result<(), String> {
    let out = std::process::Command::new("/home/vjeux/trackmania-tas/tools/target/release/shootctl").args(["launch", "240", "--force"]).output().map_err(|e| format!("shootctl launch: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    if !text.contains("title ready") && !plugin_alive() {
        return Err(format!("relaunch failed: {}", text.lines().last().unwrap_or("")));
    }
    // the plugin's state machine starts IDLE after a launch
    std::thread::sleep(std::time::Duration::from_millis(1500));
    Ok(())
}

/// One map through the editor: stage → /editmap → editor ready → /thumbedit?save=1 →
/// saved file present and stable → its header JPEG spliced into the live bytes →
/// /thumbedit?quit=1 → menu. Returns (detail, jpeg bytes).
fn editor_shot(k: usize, src: &Path, out_map: &Path, tmmaps_bin: &str) -> Result<(String, usize), String> {
    let t0 = Instant::now();
    let staged = format!("{SHOOT_DIR}/thumb-{k}.Map.Gbx");
    let saved = format!("{SHOOT_DIR}/thumb-{k}-saved.Map.Gbx");
    let _ = std::fs::remove_file(&saved);
    std::fs::copy(src, &staged).map_err(|e| format!("stage {}: {e}", src.display()))?;
    // Maps-RELATIVE paths for the editor (an absolute path opened fine but left the editor
    // in a state /back could not leave — measured by hand 2026-09-15: relative = 6.3 s cycle)
    std::fs::write(format!("{STORE}/editmap.txt"), format!("_shoot\\thumb-{k}.Map.Gbx")).map_err(|e| e.to_string())?;
    std::fs::write(format!("{STORE}/savemap.txt"), format!("_shoot\\thumb-{k}-saved.Map.Gbx")).map_err(|e| e.to_string())?;
    let a = http_get("/editmap")?;
    if !a.starts_with("ok") {
        return Err(format!("editmap refused: {a}"));
    }
    // editor up and its loading dialog gone
    let te = Instant::now();
    loop {
        if te.elapsed().as_secs() > 60 {
            return Err(format!("editor did not come up in 60 s (last ctx {})", http_get("/ctx").unwrap_or_default().chars().take(80).collect::<String>()));
        }
        // ready = the editor is up AND the plugin-map API answers (the editor keeps a
        // hidden progress bar alive, so the dialog flag never clears here)
        let ctx = http_get("/ctx").unwrap_or_default();
        if ctx.contains("\"editor\":\"trackeditor\"") {
            let st = http_get("/thumbedit").unwrap_or_default();
            if st.starts_with("idle") || st.starts_with("POSING") {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    let t_editor = t0.elapsed().as_millis();
    std::thread::sleep(std::time::Duration::from_millis(1200));
    let sv = http_get("/thumbedit?save=1")?;
    if !sv.starts_with("saved") {
        return Err(format!("save refused: {sv}"));
    }
    // the saved file: present, non-empty, size stable for 300 ms, header thumbnail inside
    let ts = Instant::now();
    let mut last = 0u64;
    let jpeg = loop {
        if ts.elapsed().as_secs() > 30 {
            return Err("saved file did not appear in 30 s".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        let n = std::fs::metadata(&saved).map(|m| m.len()).unwrap_or(0);
        if n > 0 && n == last {
            let bytes = std::fs::read(&saved).map_err(|e| e.to_string())?;
            if let Some(s0) = bytes.windows(15).position(|w| w == b"<Thumbnail.jpg>") {
                let s0 = s0 + 15;
                if let Some(e0) = bytes[s0..].windows(16).position(|w| w == b"</Thumbnail.jpg>") {
                    break bytes[s0..s0 + e0].to_vec();
                }
            }
        }
        last = n;
    };
    let t_saved = t0.elapsed().as_millis();
    if jpeg.len() < 20_000 || !jpeg.starts_with(&[0xFF, 0xD8]) {
        return Err(format!("saved thumbnail too small or not a JPEG ({} bytes)", jpeg.len()));
    }
    let jpg_wsl = format!("{CAP_DIR}/thumb-{k}.jpg");
    std::fs::write(&jpg_wsl, &jpeg).map_err(|e| e.to_string())?;
    if let Some(parent) = out_map.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let sp = std::process::Command::new(tmmaps_bin).arg("set-thumbnail").arg(src).arg("--jpeg").arg(&jpg_wsl).arg("--out").arg(out_map).output().map_err(|e| e.to_string())?;
    if !sp.status.success() {
        return Err(format!("set-thumbnail: {}", String::from_utf8_lossy(&sp.stderr).trim()));
    }
    let _ = http_get("/back");
    let tq = Instant::now();
    loop {
        if tq.elapsed().as_secs() > 30 {
            return Err(format!("editor did not quit in 30 s (editor {t_editor}ms, saved {t_saved}ms)"));
        }
        let ctx = http_get("/ctx").unwrap_or_default();
        if ctx.contains("\"editor\":\"none\"") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    let _ = std::fs::remove_file(&staged);
    // THUMBS_KEEP_SAVED=1: the editor's own save stays (what the editor made of the file —
    // which records it kept or regenerated — is readable there, 2026-11-02)
    if std::env::var("THUMBS_KEEP_SAVED").map(|v| v == "1").unwrap_or(false) {
        let keep = out_map.with_extension("editor-saved.Map.Gbx");
        let _ = std::fs::copy(&saved, &keep);
    }
    let _ = std::fs::remove_file(&saved);
    let _ = std::fs::remove_file(&jpg_wsl);
    Ok((format!("editor {t_editor}ms saved {t_saved}ms quit {}ms", t0.elapsed().as_millis()), jpeg.len()))
}

fn poll_until(prefixes: &[&str], timeout_ms: u64) -> Result<String, String> {
    let t0 = Instant::now();
    let mut last = String::new();
    let mut misses = 0;
    while t0.elapsed().as_millis() < timeout_ms as u128 {
        match http_get("/thumbshot") {
            Ok(l) => {
                misses = 0;
                last = l;
            }
            Err(e) => {
                // the plugin not answering 6 times in a row (~2 s) = the game is gone;
                // do not sit out the whole timeout (a crash cost 140 s before this)
                misses += 1;
                last = e;
                if misses >= 6 {
                    return Err(format!("game down: {last}"));
                }
                std::thread::sleep(std::time::Duration::from_millis(300));
                continue;
            }
        }
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

/// One key tap into the game window (keybd_event through PowerShell; the game is brought
/// to the foreground first) — shootctl's `playshots::tap_key`, the same bytes.
fn tap_key(name: &str, hold_ms: u64) -> Result<String, String> {
    let (vk, scan, ext): (u8, u8, bool) = match name.to_ascii_uppercase().as_str() {
        "ESC" | "ESCAPE" => (0x1B, 0x01, false),
        "ENTER" | "RETURN" => (0x0D, 0x1C, false),
        "SPACE" => (0x20, 0x39, false),
        other => return Err(format!("key {other}: not a known name (ESC ENTER SPACE)")),
    };
    let (down, up) = if ext { (1u32, 3u32) } else { (0u32, 2u32) };
    let script = format!(
        "$sig = '[DllImport(\"user32.dll\")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, System.UIntPtr dwExtraInfo); \
         [DllImport(\"user32.dll\")] public static extern bool SetForegroundWindow(System.IntPtr hWnd); \
         [DllImport(\"user32.dll\")] public static extern void SwitchToThisWindow(System.IntPtr hWnd, bool fAltTab); \
         [DllImport(\"user32.dll\")] public static extern System.IntPtr GetForegroundWindow();'; \
         $k = Add-Type -MemberDefinition $sig -Name Keys3 -Namespace Thumbs -PassThru; \
         $p = Get-Process Trackmania -ErrorAction SilentlyContinue | Select-Object -First 1; \
         if (-not $p) {{ 'no Trackmania process'; exit 1 }}; \
         $h = $p.MainWindowHandle; \
         $k::keybd_event(0x12, 0x38, 0, [System.UIntPtr]::Zero); [void]$k::SetForegroundWindow($h); $k::keybd_event(0x12, 0x38, 2, [System.UIntPtr]::Zero); Start-Sleep -Milliseconds 150; \
         if ($k::GetForegroundWindow() -ne $h) {{ $k::SwitchToThisWindow($h, $true); Start-Sleep -Milliseconds 300 }}; \
         $k::keybd_event({vk}, {scan}, {down}, [System.UIntPtr]::Zero); Start-Sleep -Milliseconds {hold_ms}; $k::keybd_event({vk}, {scan}, {up}, [System.UIntPtr]::Zero); 'tapped'"
    );
    let out = std::process::Command::new("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("powershell: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || !text.contains("tapped") {
        return Err(format!("keybd_event script: {text} {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(text)
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
    // the race started before the shot (see the play route); --start-wait-ms: intro skip → race
    let start_race = !tmmaps::cli::has(args, "--no-start");
    let start_wait_ms: u64 = f("--start-wait-ms").and_then(|s| s.parse().ok()).unwrap_or(4500);
    let limit: usize = f("--limit").and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let tmmaps_bin = f("--tmmaps").unwrap_or_else(|| "/home/vjeux/shoot/fix4/target/release/tmmaps".into());
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    std::fs::create_dir_all(CAP_DIR).map_err(|e| format!("{CAP_DIR}: {e}"))?;
    std::fs::create_dir_all(SHOOT_DIR).map_err(|e| format!("{SHOOT_DIR}: {e}"))?;
    let log_path = outdir.join("thumbs.tsv");
    let mut done: std::collections::HashSet<String> = std::collections::HashSet::new();
    if resume {
        if let Ok(t) = std::fs::read_to_string(&log_path) {
            let mut fails: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
            for l in t.lines().skip(1) {
                if let Some(p) = l.split('\t').next() {
                    match l.split('\t').nth(4) {
                        Some("ok") => { done.insert(p.to_string()); }
                        // a map that failed twice already (a game-crashing map) is not tried again
                        Some("FAIL") => { *fails.entry(p.to_string()).or_default() += 1; }
                        _ => {}
                    }
                }
            }
            for (p, n) in fails {
                if n >= 2 && !done.contains(&p) {
                    eprintln!("skipping {p}: {n} earlier failures");
                    done.insert(p);
                }
            }
            // the log may spell paths differently from the list (relative vs absolute):
            // match by file name too
            let names: Vec<String> = done.iter().filter_map(|p| Path::new(p).file_name().map(|n| n.to_string_lossy().to_string())).collect();
            done.extend(names);
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
    // --editor: the EDITOR-SAVE route (2026-09-15). The play-mode camera override never
    // took effect (the playground rewrites the camera every frame: every shot of the
    // 14/15 September run was the spawn camera). TM2020 renders the thumbnail from the
    // map's stored thumbnail camera on every editor save — square, stored fov, no UI:
    // open the map in the editor, save a scratch copy, lift its JPEG, splice it in.
    let editor_mode = tmmaps::cli::has(args, "--editor") || std::env::var("THUMBS_EDITOR").map(|v| v == "1").unwrap_or(false);
    // the capture server (`\\.\pipe\tmcap`) serves the play-mode route only; the editor
    // route lifts the JPEG the game itself wrote, so it needs no capture server (a box
    // without one failed the probe before the first map, 2026-11-02)
    let mut cap_client = if editor_mode { None } else { Some(CapClient::start()?) };
    if let Some(c) = cap_client.as_mut() {
        c.capture(&format!("{CAP_WIN}\\_probe.jpg"))?;
    }
    let t_run = Instant::now();
    let mut n_ok = 0usize;
    let mut n_fail = 0usize;
    let mut attempts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut retry_queue: Vec<usize> = Vec::new();
    let order: Vec<usize> = (0..maps.len()).collect();
    let mut oi = 0usize;
    loop {
        let k = if let Some(r) = retry_queue.pop() { r } else if oi < order.len() { let k = order[oi]; oi += 1; k } else { break };
        let m = &maps[k];
        if n_ok + n_fail >= limit {
            break;
        }
        if done.contains(m) || Path::new(m).file_name().map(|n| done.contains(&n.to_string_lossy().to_string())).unwrap_or(false) {
            continue;
        }
        let t0 = Instant::now();
        let src = match &root { Some(r) => r.join(m), None => PathBuf::from(m) };
        let rel = match &root { Some(r) => src.strip_prefix(r).unwrap_or(&src).to_path_buf(), None => PathBuf::from(Path::new(m).file_name().unwrap_or_default()) };
        let out_map = outdir.join(&rel);
        let res = if editor_mode {
            let r = editor_shot(k, &src, &out_map, &tmmaps_bin);
            if r.is_err() && !plugin_alive() {
                // the game died under this map (icy 130, 2026-09-15 19:05 PT: the editor
                // path had no relaunch and hung the whole set): bring it back, then retry
                eprintln!("game DOWN after {} — relaunching", src.display());
                relaunch_game();
            } else if r.is_err() {
                let _ = http_get("/back");
                let tq = Instant::now();
                while tq.elapsed().as_secs() < 20 && !http_get("/ctx").unwrap_or_default().contains("\"editor\":\"none\"") {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
            r
        } else { (|| -> Result<(String, usize), String> {
            let name = src.file_name().and_then(|n| n.to_str()).ok_or("no file name")?.to_string();
            // stage under a SHORT ascii name: the game path goes through editmap.txt verbatim
            let staged = format!("{SHOOT_DIR}/thumb-{k}.Map.Gbx");
            std::fs::copy(&src, &staged).map_err(|e| format!("stage {}: {e}", src.display()))?;
            std::fs::write(format!("{STORE}/editmap.txt"), format!("{SHOOT_WIN}\\thumb-{k}.Map.Gbx")).map_err(|e| e.to_string())?;
            // THUMBS_EXTRA_QUERY: camera knobs for the plugin (fovmul, pitchsign, aspect) — the
            // single-map iteration against a map's original thumbnail (2026-09-15)
            let extra = std::env::var("THUMBS_EXTRA_QUERY").unwrap_or_default();
            let a = http_get(&format!("/thumbshot?map=1&frames={frames}&ms={ready_ms}{}{extra}", if extra.is_empty() || extra.starts_with('&') { "" } else { "&" }))?;
            if !a.starts_with("LOADING") {
                return Err(format!("load refused: {a}"));
            }
            let ready = poll_until(&["READY"], 25_000)?;
            // THE RACE MUST BE RUNNING for the playground camera to yield to the pose:
            // during the intro / countdown the game's own camera wins every frame (the
            // September run: every shot was the spawn camera, 2026-09-14). The MK64 skin QA
            // found the way (/lookat, 2026-09-24): skip the intro with ENTER, let the 3-2-1
            // run, re-arm the pose (+ HUD off) once the race clock runs. `--no-start` keeps
            // the old behaviour.
            if start_race {
                tap_key("ENTER", 60)?;
                std::thread::sleep(std::time::Duration::from_millis(start_wait_ms));
                let arm = http_get("/thumbcam?on=1")?;
                if !arm.starts_with("armed") {
                    return Err(format!("re-arm after the start refused: {arm}"));
                }
                std::thread::sleep(std::time::Duration::from_millis(1500));
            }
            let jpg_win = format!("{CAP_WIN}\\thumb-{k}.jpg");
            let jpg_wsl = format!("{CAP_DIR}/thumb-{k}.jpg");
            // the map-load FADE: the first posed frames are under a dark veil that
            // lifts over ~1 s. The capture reports the frame's mean luminance; shoot
            // again while it is still rising (two captures 250 ms apart agreeing
            // within 2 = stable), at most 8 times. Measured 2026-09-14: the veiled
            // frame reads ~45, the clear one ~110 on the same map.
            let cap_client = cap_client.as_mut().ok_or("play-mode capture without a capture server")?;
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
        })() };
        let ms = t0.elapsed().as_millis();
        match res {
            Ok((detail, bytes)) => {
                n_ok += 1;
                let uid = tmmaps::header::read(out_map.to_str().unwrap_or_default()).map(|h| h.uid).unwrap_or_default();
                writeln!(log, "{m}\t{uid}\t{bytes}\t{ms}\tok\t{}", detail.replace('\t', " ")).map_err(|e| e.to_string())?;
                println!("[{:>4}/{}] ok {ms:5} ms  {}", k + 1, maps.len(), rel.display());
            }
            Err(e) => {
                let dead = !plugin_alive();
                *attempts.entry(m.clone()).or_default() += 1;
                let tries = attempts[m];
                eprintln!("[{:>4}/{}] FAIL {ms:5} ms  {} (attempt {tries}{}): {e}", k + 1, maps.len(), rel.display(), if dead { ", game DOWN" } else { "" });
                if dead {
                    // the game crashed: bring it back (the capture server survives, it
                    // is a separate process) and retry this map once
                    match relaunch_game() {
                        Ok(()) => eprintln!("      game relaunched"),
                        Err(re) => {
                            writeln!(log, "{m}\t\t0\t{ms}\tFAIL\t{}", format!("{e}; {re}").replace('\t', " ")).map_err(|e| e.to_string())?;
                            return Err(format!("game relaunch failed: {re}"));
                        }
                    }
                } else {
                    let _ = http_get("/thumbshot?done=1");
                    let _ = poll_until(&["MENU", "IDLE"], 30_000);
                }
                if tries < 2 {
                    // retry the same map once (a crash on load is not always the map's fault)
                    retry_queue.push(k);
                } else {
                    n_fail += 1;
                    writeln!(log, "{m}\t\t0\t{ms}\tFAIL\t{}", e.replace('\t', " ")).map_err(|e| e.to_string())?;
                }
            }
        }
        let _ = log.flush();
    }
    println!("{n_ok} ok, {n_fail} failed in {:.0} s ({:.2} s/map)", t_run.elapsed().as_secs_f64(), t_run.elapsed().as_secs_f64() / (n_ok + n_fail).max(1) as f64);
    Ok(())
}
