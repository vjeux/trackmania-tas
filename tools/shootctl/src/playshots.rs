//! `shootctl playshots --map MAP --outdir /mnt/c/DIR [--tag T] [--shots N]
//! [--every-ms MS] [--first-ms MS] [--timeout S] [--detach]`: open the map in
//! PLAY mode (a playground, the car at the start) under the render lock and
//! take N timed screenshots — the test of whether a moving item's collision
//! moves with it (four half-size pushers around the spawn, 2026-09-07): if
//! their pistons shove the car, it is not where it spawned by the third shot.
//!
//! Like `shootset`, `--detach` re-runs in the background with the log in
//! `OUTDIR/playshots.log` and `OUTDIR/done-play.txt` written last.

use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct Opts {
    pub map: String,
    pub outdir: PathBuf,
    pub tag: String,
    pub shots: usize,
    pub every_ms: u64,
    pub first_ms: u64,
    pub timeout_s: u64,
    /// `--carlog-ms MS`: after the playground opens, record the live car
    /// (`/carlog`, one row per frame: race time, position, velocity, speed)
    /// for MS milliseconds into `OUTDIR/car-<tag>.tsv` before the shots —
    /// the spawn point is the first row, a shove is a velocity that appears.
    pub carlog_ms: u64,
    /// `--drive-ms MS [--drive-at-ms MS]`: hold the accelerator (the Up arrow,
    /// a synthetic key held for MS through the Windows input stream) from
    /// `drive_at_ms` after the playground opens — the race starts ~14.7 s in
    /// (10 s intro + countdown) — so the car rolls off the start straight
    /// ahead: the one way this pipeline has of putting the car INTO an
    /// in-game MediaTracker trigger (a car spawned inside one does not fire
    /// it, measured on Summer 15).
    pub drive_ms: u64,
    pub drive_at_ms: u64,
    pub detach: bool,
}

pub fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let val = |k: &str| -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() };
    let num = |k: &str, d: u64| -> Result<u64, String> { val(k).map(|s| s.parse::<u64>().map_err(|_| format!("{k} wants a number"))).transpose().map(|o| o.unwrap_or(d)) };
    let outdir = PathBuf::from(val("--outdir").ok_or("playshots needs --outdir <dir under /mnt/c>")?);
    if !outdir.starts_with("/mnt/") {
        return Err(format!("--outdir {} must live under /mnt/<drive>/ — the screenshot is taken by a Windows program", outdir.display()));
    }
    Ok(Opts {
        map: val("--map").ok_or("playshots needs --map <path>")?,
        outdir,
        tag: val("--tag").unwrap_or_else(|| "play".into()),
        shots: num("--shots", 4)? as usize,
        every_ms: num("--every-ms", 3000)?,
        first_ms: num("--first-ms", 4000)?,
        timeout_s: num("--timeout", 300)?,
        carlog_ms: num("--carlog-ms", 0)?,
        drive_ms: num("--drive-ms", 0)?,
        drive_at_ms: num("--drive-at-ms", 13500)?,
        detach: args.iter().any(|a| a == "--detach"),
    })
}

pub fn run(args: &[String]) -> i32 {
    let opts = match parse_opts(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("usage: shootctl playshots --map MAP --outdir /mnt/c/... [--tag T] [--shots N] [--every-ms MS] [--first-ms MS] [--carlog-ms MS] [--drive-ms MS [--drive-at-ms MS]] [--timeout S] [--detach]");
            return 2;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&opts.outdir) {
        eprintln!("{}: {e}", opts.outdir.display());
        return 2;
    }
    let done = opts.outdir.join("done-play.txt");
    let _ = std::fs::remove_file(&done);
    if opts.detach {
        return super::shootset::detach_as(&opts.outdir.join("playshots.log"), &done);
    }
    let t0 = Instant::now();
    let result = run_shots(&opts, t0);
    let summary = match &result {
        Ok(lines) => format!("OK {} shots in {:.0}s\n{}\n", lines.len(), t0.elapsed().as_secs_f64(), lines.join("\n")),
        Err(e) => format!("FAILED after {:.0}s: {e}\n", t0.elapsed().as_secs_f64()),
    };
    print!("{summary}");
    let tmp = opts.outdir.join("done-play.tmp");
    if std::fs::write(&tmp, &summary).and_then(|_| std::fs::rename(&tmp, &done)).is_err() {
        eprintln!("could not write {}", done.display());
        return 1;
    }
    if result.is_ok() { 0 } else { 1 }
}

fn run_shots(opts: &Opts, t0: Instant) -> Result<Vec<String>, String> {
    let el = || format!("[{:6.1}s]", t0.elapsed().as_secs_f64());
    let d = super::lock::lock_dir();
    let owner = format!("playshots-{}", opts.tag);
    super::lock::acquire(&d, &owner, 1500, 0).map_err(|e| format!("lock: {e}"))?; // eight threads queue on one game (see shootset.rs)
    let _guard = super::shootset::LockGuard::new(d, owner);
    let staged = super::shootset::stage_map(&opts.map)?;
    let game_map = super::game_path(&staged)?;
    println!("{} map {}", el(), game_map);
    if super::launch(180, false) != 0 {
        return Err("the game did not come up".into());
    }
    super::to_menu()?;
    super::await_cond("ready", 60)?;
    let store = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
    let _ = std::fs::create_dir_all(store);
    std::fs::write(format!("{store}/editmap.txt"), &game_map).map_err(|e| format!("editmap.txt: {e}"))?;
    println!("{} /playmap: {}", el(), super::http_get("/playmap?mode=", 30).unwrap_or_default().trim());
    let load0 = Instant::now();
    loop {
        if load0.elapsed().as_secs() > opts.timeout_s {
            return Err(format!("no playground in {} s; last ctx {}", opts.timeout_s, super::http_get("/ctx", 10).unwrap_or_default().trim()));
        }
        if !super::tm_running() {
            return Err("the game process is gone — the map crashed the client".into());
        }
        let c = super::http_get("/ctx", 10).unwrap_or_default();
        if c.contains("FrameAskYesNo") {
            let text = super::http_get("/dlgtext", 10).unwrap_or_default().trim().to_string();
            println!("{} DIALOG {}", el(), text);
            let _ = super::http_get("/yes", 10);
        }
        // anywhere but the menu: the playground is up (ctx 2 in play)
        if matches!(super::ctx(), Some(n) if n != 0) {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let opened = load0.elapsed();
    println!("{} playground after {:.1}s (ctx {})", el(), opened.as_secs_f64(), super::http_get("/ctx", 10).unwrap_or_default().trim());
    let mut lines = Vec::new();
    let mut driver: Option<std::thread::JoinHandle<Result<(), String>>> = None;
    if opts.drive_ms > 0 {
        // the key is held on its own thread so the shots keep their cadence
        let (at, hold) = (opts.drive_at_ms, opts.drive_ms);
        let opened_at = load0;
        driver = Some(std::thread::spawn(move || {
            let wait = Duration::from_millis(at).saturating_sub(opened_at.elapsed());
            std::thread::sleep(wait);
            hold_accelerator(hold)
        }));
        lines.push(format!("drive\taccelerator held {} ms from {} ms after the playground opened", opts.drive_ms, opts.drive_at_ms));
    }
    if opts.carlog_ms > 0 {
        // one call per 5 s slice: a 24 s request never came back (the server
        // serves a handler that yields, but not for that long), and the
        // handler samples at a few Hz anyway (one row per server turn)
        let file = opts.outdir.join(format!("car-{}.tsv", opts.tag));
        let mut tsv = String::new();
        let mut left = opts.carlog_ms;
        while left > 0 {
            let chunk = left.min(5_000);
            let body = match super::http_get(&format!("/carlog?ms={chunk}"), chunk / 1000 + 20) {
                Ok(b) => b,
                Err(e) => {
                    println!("{} carlog slice failed: {e}", el());
                    break;
                }
            };
            for (i, row) in body.lines().enumerate() {
                if i == 0 && !tsv.is_empty() {
                    continue; // one header
                }
                tsv.push_str(row);
                tsv.push('\n');
            }
            left -= chunk;
        }
        std::fs::write(&file, &tsv).map_err(|e| format!("{}: {e}", file.display()))?;
        let rows = tsv.lines().count().saturating_sub(1);
        let line = format!("carlog\t{rows} rows over {} ms\t{}", opts.carlog_ms, file.display());
        println!("{} {line}", el());
        lines.push(line);
    }
    std::thread::sleep(Duration::from_millis(opts.first_ms));
    for k in 0..opts.shots {
        if k > 0 {
            std::thread::sleep(Duration::from_millis(opts.every_ms));
        }
        let file = opts.outdir.join(format!("play-{}-{k}.png", opts.tag));
        let _ = std::fs::remove_file(&file);
        let at = load0.elapsed().as_secs_f64();
        super::shootset::screenshot(&file)?;
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        let line = format!("shot {k}\t{at:.1}s after the playground opened\t{}\t{size}", file.display());
        println!("{} {line}", el());
        lines.push(line);
    }
    if let Some(d) = driver {
        match d.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => lines.push(format!("drive\tFAILED: {e}")),
            Err(_) => lines.push("drive\tFAILED: the key thread panicked".to_string()),
        }
    }
    let _ = super::to_menu();
    Ok(lines)
}

/// Hold the Up arrow for `ms` in the foreground game window: PowerShell +
/// `keybd_event` (an extended key: scan 0x48, flags 1 down / 3 up), the game
/// brought to the foreground first. Synthetic input reaches the game the way
/// AutoHotkey's does.
fn hold_accelerator(ms: u64) -> Result<(), String> {
    let script = format!(
        "$sig = '[DllImport(\"user32.dll\")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, System.UIntPtr dwExtraInfo); [DllImport(\"user32.dll\")] public static extern bool SetForegroundWindow(System.IntPtr hWnd);'; \
         $k = Add-Type -MemberDefinition $sig -Name Keys -Namespace Drive -PassThru; \
         $p = Get-Process Trackmania -ErrorAction SilentlyContinue | Select-Object -First 1; \
         if ($p) {{ [void]$k::SetForegroundWindow($p.MainWindowHandle) }}; Start-Sleep -Milliseconds 150; \
         $k::keybd_event(0x26, 0x48, 1, [System.UIntPtr]::Zero); Start-Sleep -Milliseconds {ms}; $k::keybd_event(0x26, 0x48, 3, [System.UIntPtr]::Zero); 'held'"
    );
    let out = std::process::Command::new(super::shootset::POWERSHELL)
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("powershell: {e}"))?;
    if !out.status.success() || !String::from_utf8_lossy(&out.stdout).contains("held") {
        return Err(format!("keybd_event script: {} {}", String::from_utf8_lossy(&out.stdout).trim(), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

/// `shootctl carlog FILE`: what a `car-<tag>.tsv` says in five lines — rows
/// and wall span, where the car started and ended, how far it got from the
/// start, its top speed, and the first moment it had left the spawn by more
/// than half a metre (a shove) — instead of reading 3000 rows by eye.
pub fn summarize(args: &[String]) -> i32 {
    let Some(file) = args.first() else {
        eprintln!("usage: shootctl carlog FILE.tsv");
        return 2;
    };
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{file}: {e}");
            return 2;
        }
    };
    let mut rows: Vec<(u64, [f64; 3], f64)> = Vec::new(); // wall_ms, pos, speed
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 9 || line.starts_with('#') {
            continue;
        }
        let num = |i: usize| f[i].parse::<f64>().unwrap_or(0.0);
        let pos = [num(2), num(3), num(4)];
        if pos == [0.0; 3] {
            continue; // the frame before the player exists
        }
        rows.push((num(0) as u64, pos, num(8)));
    }
    let Some(first) = rows.first().copied() else {
        println!("{file}: no car rows");
        return 1;
    };
    let last = *rows.last().unwrap();
    let dist = |a: [f64; 3], b: [f64; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let far = rows.iter().map(|r| dist(r.1, first.1)).fold(0.0, f64::max);
    let top = rows.iter().map(|r| r.2).fold(0.0, f64::max);
    let shove = rows.iter().find(|r| dist(r.1, first.1) > 0.5);
    println!("{}: {} rows over {:.1} s", file, rows.len(), (last.0 - first.0) as f64 / 1000.0);
    println!("  start ({:.2}, {:.2}, {:.2})  end ({:.2}, {:.2}, {:.2})", first.1[0], first.1[1], first.1[2], last.1[0], last.1[1], last.1[2]);
    println!("  farthest from the start {far:.2} m, top speed {top:.1}");
    match shove {
        Some(r) => println!("  left the spawn (> 0.5 m) at +{:.1} s: ({:.2}, {:.2}, {:.2})", (r.0 - first.0) as f64 / 1000.0, r.1[0], r.1[1], r.1[2]),
        None => println!("  never left the spawn"),
    }
    0
}
