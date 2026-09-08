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
    /// `--camlog-ms MS`: the live CAMERA (and the car) per frame from the
    /// moment the playground opens, for MS milliseconds, into
    /// `OUTDIR/cam-<tag>.tsv` — on its own thread, alongside the shots and
    /// the drive: the intro's camera path (the tiny build's must be the
    /// original's through the transform) and the in-game clip's jump.
    pub camlog_ms: u64,
    /// `--wheels-ms MS`: the surface under each WHEEL per frame (`/wheels`,
    /// the VehicleState readout: the four `*GroundContactMaterial` ids, the
    /// speed, the pedals) for MS milliseconds into `OUTDIR/wheels-<tag>.tsv`,
    /// on its own thread like the camera log — what the PHYSICS resolved
    /// under the car on the original vs the tiny build (2026-09-07).
    pub wheels_ms: u64,
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
        camlog_ms: num("--camlog-ms", 0)?,
        wheels_ms: num("--wheels-ms", 0)?,
        detach: args.iter().any(|a| a == "--detach"),
    })
}

pub fn run(args: &[String]) -> i32 {
    let opts = match parse_opts(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("usage: shootctl playshots --map MAP --outdir /mnt/c/... [--tag T] [--shots N] [--every-ms MS] [--first-ms MS] [--carlog-ms MS] [--drive-ms MS [--drive-at-ms MS]] [--camlog-ms MS] [--wheels-ms MS] [--timeout S] [--detach]");
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
        // THE PLAYGROUND, not merely "not the menu": /ctx is 3 with a
        // CurrentPlayground and no editor, 1/2/9 inside an editor, 0 at the
        // menu. `n != 0` took a transient editor context 0.3 s after /playmap
        // for the playground (2026-09-08: startcheck shot the main menu, read
        // "no vehicle", and failed a map that opened fine seconds later).
        if super::ctx() == Some(3) {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let opened = load0.elapsed();
    println!("{} playground after {:.1}s (ctx {})", el(), opened.as_secs_f64(), super::http_get("/ctx", 10).unwrap_or_default().trim());
    let mut lines = Vec::new();
    let mut driver: Option<std::thread::JoinHandle<Result<String, String>>> = None;
    let mut camlog: Option<std::thread::JoinHandle<Result<String, String>>> = None;
    let mut wheels: Option<std::thread::JoinHandle<Result<String, String>>> = None;
    if opts.wheels_ms > 0 {
        // the wheel log, sliced like the camera log, on its own thread
        let (file, total) = (opts.outdir.join(format!("wheels-{}.tsv", opts.tag)), opts.wheels_ms);
        wheels = Some(std::thread::spawn(move || {
            let mut tsv = String::new();
            let mut left = total;
            let t0 = Instant::now();
            while left > 0 {
                let chunk = left.min(5_000);
                let body = super::http_get(&format!("/wheels?ms={chunk}"), chunk / 1000 + 20)?;
                for (i, row) in body.lines().enumerate() {
                    if i == 0 && !tsv.is_empty() {
                        continue;
                    }
                    tsv.push_str(row);
                    tsv.push('\n');
                }
                left -= chunk;
            }
            std::fs::write(&file, &tsv).map_err(|e| format!("{}: {e}", file.display()))?;
            Ok(format!("wheels\t{} rows over {:.1} s\t{}", tsv.lines().count().saturating_sub(1), t0.elapsed().as_secs_f64(), file.display()))
        }));
    }
    if opts.camlog_ms > 0 {
        let (file, total) = (opts.outdir.join(format!("cam-{}.tsv", opts.tag)), opts.camlog_ms);
        camlog = Some(std::thread::spawn(move || {
            // 5 s slices like the car log: one long request never comes back
            let mut tsv = String::new();
            let mut left = total;
            let t0 = Instant::now();
            while left > 0 {
                let chunk = left.min(5_000);
                let body = super::http_get(&format!("/camlog?ms={chunk}"), chunk / 1000 + 20)?;
                for (i, row) in body.lines().enumerate() {
                    if i == 0 && !tsv.is_empty() {
                        continue;
                    }
                    tsv.push_str(row);
                    tsv.push('\n');
                }
                left -= chunk;
            }
            std::fs::write(&file, &tsv).map_err(|e| format!("{}: {e}", file.display()))?;
            Ok(format!("camlog\t{} rows over {:.1} s\t{}", tsv.lines().count().saturating_sub(1), t0.elapsed().as_secs_f64(), file.display()))
        }));
    }
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
    if let Some(c) = camlog {
        match c.join() {
            Ok(Ok(text)) => lines.push(text),
            Ok(Err(e)) => lines.push(format!("camlog\tFAILED: {e}")),
            Err(_) => lines.push("camlog\tFAILED: the camera thread panicked".to_string()),
        }
    }
    if let Some(w) = wheels {
        match w.join() {
            Ok(Ok(text)) => lines.push(text),
            Ok(Err(e)) => lines.push(format!("wheels\tFAILED: {e}")),
            Err(_) => lines.push("wheels\tFAILED: the wheel thread panicked".to_string()),
        }
    }
    if let Some(d) = driver {
        match d.join() {
            Ok(Ok(text)) => lines.push(format!("drive\t{text}")),
            Ok(Err(e)) => lines.push(format!("drive\tFAILED: {e}")),
            Err(_) => lines.push("drive\tFAILED: the key thread panicked".to_string()),
        }
    }
    let _ = super::to_menu();
    Ok(lines)
}

/// Hold the Up arrow for `ms` in the game window: PowerShell + `keybd_event`
/// (an extended key: scan 0x48, flags 1 down / 3 up). The game is brought to
/// the FOREGROUND first — measured 2026-09-07: it was not (the foreground was
/// an untitled window and a 7 s hold moved nothing), and a background
/// process may only call `SetForegroundWindow` after it has itself generated
/// input, so a bare ALT tap precedes the call (the standard dance;
/// `SwitchToThisWindow` as the second attempt). The foreground window's
/// title after that is what the result reports: `fg=[Trackmania]` is the
/// proof the keys went to the game.
fn hold_accelerator(ms: u64) -> Result<String, String> {
    let script = format!(
        "$sig = '[DllImport(\"user32.dll\")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, System.UIntPtr dwExtraInfo); \
         [DllImport(\"user32.dll\")] public static extern bool SetForegroundWindow(System.IntPtr hWnd); \
         [DllImport(\"user32.dll\")] public static extern void SwitchToThisWindow(System.IntPtr hWnd, bool fAltTab); \
         [DllImport(\"user32.dll\")] public static extern System.IntPtr GetForegroundWindow(); \
         [DllImport(\"user32.dll\")] public static extern int GetWindowText(System.IntPtr hWnd, System.Text.StringBuilder text, int count);'; \
         $k = Add-Type -MemberDefinition $sig -Name Keys -Namespace Drive -PassThru; \
         $p = Get-Process Trackmania -ErrorAction SilentlyContinue | Select-Object -First 1; \
         if (-not $p) {{ 'no Trackmania process'; exit 1 }}; \
         $h = $p.MainWindowHandle; \
         $k::keybd_event(0x12, 0x38, 0, [System.UIntPtr]::Zero); [void]$k::SetForegroundWindow($h); $k::keybd_event(0x12, 0x38, 2, [System.UIntPtr]::Zero); Start-Sleep -Milliseconds 200; \
         if ($k::GetForegroundWindow() -ne $h) {{ $k::SwitchToThisWindow($h, $true); Start-Sleep -Milliseconds 300 }}; \
         $sb = New-Object System.Text.StringBuilder 256; [void]$k::GetWindowText($k::GetForegroundWindow(), $sb, 256); \
         $k::keybd_event(0x26, 0x48, 1, [System.UIntPtr]::Zero); Start-Sleep -Milliseconds {ms}; $k::keybd_event(0x26, 0x48, 3, [System.UIntPtr]::Zero); \
         'held fg=[' + $sb.ToString() + ']'"
    );
    let out = std::process::Command::new(super::shootset::POWERSHELL)
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("powershell: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || !text.contains("held") {
        return Err(format!("keybd_event script: {text} {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(text)
}

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

/// The physics name of an `EPlugSurfaceMaterialId` (the pack's table; the
/// same list `mapgeom` prints — kept short here, the rest print as numbers).
fn surface_name(id: i64) -> String {
    match id {
        0 => "Concrete".into(),
        1 => "Pavement".into(),
        2 => "Grass".into(),
        3 => "Ice".into(),
        4 => "Metal".into(),
        5 => "Sand".into(),
        6 => "Dirt".into(),
        8 => "DirtRoad".into(),
        9 => "Rubber".into(),
        10 => "SlidingRubber".into(),
        12 => "Rock".into(),
        13 => "Water".into(),
        14 => "Wood".into(),
        16 => "Asphalt".into(),
        21 => "Snow".into(),
        28 => "NotCollidable".into(),
        33 => "Stone".into(),
        74 => "RoadIce".into(),
        75 => "RoadSynthetic".into(),
        76 => "Green".into(),
        77 => "Plastic".into(),
        n => format!("id{n}"),
    }
}

/// `shootctl wheels FILE.tsv [--step MS]`: a wheel log read as the surface
/// census it is — per wheel, how many frames on which material (ground-contact
/// frames only) — and the acceleration trace from the first frame the gas
/// pedal is down: speed and distance at 0.5 s steps, with the surface under
/// the wheels at each step. Two such tables, original and tiny, side by side,
/// ARE the physics comparison.
pub fn summarize_wheels(args: &[String]) -> i32 {
    let step: u64 = args.iter().position(|a| a == "--step").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(500);
    let Some(file) = args.iter().find(|a| !a.starts_with("--") && a.ends_with(".tsv")) else {
        eprintln!("usage: shootctl wheels FILE.tsv [--step MS]");
        return 2;
    };
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{file}: {e}");
            return 2;
        }
    };
    // wall_ms t_ms x y z vx vy vz frontspeed gas brake steer ground fl fr rl rr …
    struct Row {
        wall: u64,
        race: i64,
        pos: [f64; 3],
        speed: f64,
        gas: f64,
        ground: bool,
        mats: [i64; 4],
    }
    let mut rows: Vec<Row> = Vec::new();
    let mut comments = 0usize;
    for line in text.lines().skip(1) {
        if line.starts_with('#') {
            comments += 1;
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 17 {
            continue;
        }
        let num = |i: usize| f[i].parse::<f64>().unwrap_or(0.0);
        let int = |i: usize| f[i].parse::<f64>().map(|x| x as i64).unwrap_or(-1);
        rows.push(Row {
            wall: num(0) as u64,
            race: if f[1].is_empty() { i64::MIN } else { int(1) },
            pos: [num(2), num(3), num(4)],
            speed: num(8),
            gas: num(9),
            ground: int(12) != 0,
            mats: [int(13), int(14), int(15), int(16)],
        });
    }
    if rows.is_empty() {
        println!("{file}: no vehicle rows ({comments} frames without a vehicle)");
        return 1;
    }
    let span = (rows.last().unwrap().wall - rows[0].wall) as f64 / 1000.0;
    println!("{file}: {} vehicle rows over {span:.1} s ({comments} frames without a vehicle)", rows.len());
    // per-wheel census over ground-contact frames
    let names = ["FL", "FR", "RL", "RR"];
    for (w, name) in names.iter().enumerate() {
        let mut counts: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
        let mut n = 0usize;
        for r in rows.iter().filter(|r| r.ground) {
            *counts.entry(r.mats[w]).or_insert(0) += 1;
            n += 1;
        }
        let mut v: Vec<(i64, usize)> = counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let desc: Vec<String> = v.iter().map(|(id, c)| format!("{} {c} ({:.0}%)", surface_name(*id), 100.0 * *c as f64 / n.max(1) as f64)).collect();
        println!("  {name}: {} ground frames: {}", n, desc.join(", "));
    }
    // the acceleration trace from the first gas frame, stepped on RACE time
    // when the log has it (the game's clock: two logs of two loads align
    // exactly), else on the wall clock
    let Some(g0) = rows.iter().position(|r| r.gas > 0.5) else {
        println!("  no frame with the gas pedal down");
        return 0;
    };
    // (CurrentRaceTime stays 0 in a /playmap playground — measured 2026-09-07 — so the clock must be seen to advance)
    let use_race = rows[g0].race != i64::MIN && rows[g0].race >= 0 && rows.last().map(|r| r.race > rows[g0].race + 1000).unwrap_or(false);
    let clock = |r: &Row| -> i64 { if use_race { r.race } else { r.wall as i64 } };
    let t0 = clock(&rows[g0]);
    let p0 = rows[g0].pos;
    println!(
        "  gas from wall {} (race time {}), at ({:.2}, {:.2}, {:.2}); speed and distance every {step} ms of {} time:",
        rows[g0].wall,
        if rows[g0].race == i64::MIN { "-".to_string() } else { rows[g0].race.to_string() },
        p0[0],
        p0[1],
        p0[2],
        if use_race { "RACE" } else { "wall" }
    );
    println!("    t(s)   speed   dist(m)   wheels");
    let mut next = 0i64;
    let mut last_gas = t0;
    for r in rows[g0..].iter() {
        if r.gas > 0.5 {
            last_gas = clock(r);
        }
        let dt = clock(r) - t0;
        if dt >= next {
            let d = ((r.pos[0] - p0[0]).powi(2) + (r.pos[1] - p0[1]).powi(2) + (r.pos[2] - p0[2]).powi(2)).sqrt();
            let mats: Vec<String> = r.mats.iter().map(|m| surface_name(*m)).collect();
            println!("    {:5.2}  {:6.1}  {:8.2}   {}{}", dt as f64 / 1000.0, r.speed, d, mats.join("/"), if r.ground { "" } else { " (airborne)" });
            next += step as i64;
        }
        if dt > 30_000 {
            break;
        }
    }
    println!("  gas pedal last down at +{:.1} s", (last_gas - t0) as f64 / 1000.0);
    0
}

/// Tap one virtual key in the game window (foregrounded first, the same
/// dance as `hold_accelerator`): `shootctl key ESC|ENTER|UP|... [--hold-ms N]`.
/// The menu-level popups the title raises (a PASSWORD prompt sat on the main
/// menu for an hour on 2026-09-07, `IsReady` false, every driver on the box
/// waiting on `ready`, `/dismiss` blind to it — it is not a CGameDialogs
/// frame) go away with Escape, and nothing else in the pipeline can press it.
pub fn tap_key(name: &str, hold_ms: u64) -> Result<String, String> {
    let (vk, scan, ext): (u8, u8, bool) = match name.to_ascii_uppercase().as_str() {
        "ESC" | "ESCAPE" => (0x1B, 0x01, false),
        "ENTER" | "RETURN" => (0x0D, 0x1C, false),
        "SPACE" => (0x20, 0x39, false),
        "TAB" => (0x09, 0x0F, false),
        "UP" => (0x26, 0x48, true),
        "DOWN" => (0x28, 0x50, true),
        "LEFT" => (0x25, 0x4B, true),
        "RIGHT" => (0x27, 0x4D, true),
        "DELETE" | "DEL" => (0x2E, 0x53, true),
        "BACKSPACE" => (0x08, 0x0E, false),
        other => {
            let s = other.strip_prefix("VK").or_else(|| other.strip_prefix("0X")).ok_or_else(|| format!("key {other}: not a known name (ESC ENTER SPACE TAB UP DOWN LEFT RIGHT DEL BACKSPACE, or VK<hex>)"))?;
            (u8::from_str_radix(s, 16).map_err(|e| format!("key {other}: {e}"))?, 0, false)
        }
    };
    let (down, up) = if ext { (1u32, 3u32) } else { (0u32, 2u32) };
    let script = format!(
        "$sig = '[DllImport(\"user32.dll\")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, System.UIntPtr dwExtraInfo); \
         [DllImport(\"user32.dll\")] public static extern bool SetForegroundWindow(System.IntPtr hWnd); \
         [DllImport(\"user32.dll\")] public static extern void SwitchToThisWindow(System.IntPtr hWnd, bool fAltTab); \
         [DllImport(\"user32.dll\")] public static extern System.IntPtr GetForegroundWindow(); \
         [DllImport(\"user32.dll\")] public static extern int GetWindowText(System.IntPtr hWnd, System.Text.StringBuilder text, int count);'; \
         $k = Add-Type -MemberDefinition $sig -Name Keys2 -Namespace Drive -PassThru; \
         $p = Get-Process Trackmania -ErrorAction SilentlyContinue | Select-Object -First 1; \
         if (-not $p) {{ 'no Trackmania process'; exit 1 }}; \
         $h = $p.MainWindowHandle; \
         $k::keybd_event(0x12, 0x38, 0, [System.UIntPtr]::Zero); [void]$k::SetForegroundWindow($h); $k::keybd_event(0x12, 0x38, 2, [System.UIntPtr]::Zero); Start-Sleep -Milliseconds 200; \
         if ($k::GetForegroundWindow() -ne $h) {{ $k::SwitchToThisWindow($h, $true); Start-Sleep -Milliseconds 300 }}; \
         $sb = New-Object System.Text.StringBuilder 256; [void]$k::GetWindowText($k::GetForegroundWindow(), $sb, 256); \
         $k::keybd_event({vk}, {scan}, {down}, [System.UIntPtr]::Zero); Start-Sleep -Milliseconds {hold_ms}; $k::keybd_event({vk}, {scan}, {up}, [System.UIntPtr]::Zero); \
         'tapped fg=[' + $sb.ToString() + ']'"
    );
    let out = std::process::Command::new(super::shootset::POWERSHELL)
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
