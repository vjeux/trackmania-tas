//! `shootctl loadprof` — PROFILE one map load on the render box instead of
//! guessing at it.
//!
//! ```text
//! shootctl loadprof --map MAP --outdir /mnt/c/DIR [--tag T] [--how edit|play]
//!                   [--timeout S] [--dump-every S] [--wpr PROFILE|off]
//!                   [--restart] [--back] [--tracerpt] [--detach]
//! ```
//!
//! What it records, all of it stamped against ONE clock (the moment the load
//! is asked for = t0):
//!
//! * the game's own timeline: every change of `/ctx` (menu → loading → editor
//!   or playground) with its offset, into `timeline-<tag>.txt`;
//! * per-second counters from `typeperf` into `perf-<tag>.csv`: the game's
//!   CPU (process and EVERY thread), I/O bytes, working set, page faults, the
//!   GPU engines of its pid, the disk, the whole machine — CPU-bound or
//!   waiting, one thread or many, reading or computing;
//! * a Windows Performance Recorder trace around the load (`--wpr`, default
//!   the `Light` profile of `loadprof.wprp` beside the output: sampled
//!   profile with stacks, no context switches — the stock `CPU` profile
//!   writes ~40 MB/s and drowns a five-minute load) into `trace-<tag>.etl`,
//!   with `--tracerpt` dumped to `trace-<tag>.csv` for `mapgeom etlsum`;
//! * a minidump of the game every `--dump-every` seconds (default 10; 0
//!   off) into `dump-<tag>-<k>.dmp` — `rundll32 comsvcs.dll, MiniDump …
//!   mini`, ~0.5 MB each — a poor man's sampler that needs no ETW at all,
//!   read by `mapgeom threads`.
//!
//! `--restart` closes and relaunches the game first (a COLD load: nothing of
//! the map or its items in the game's caches); without it the load is WARM
//! when the same map was open before. `--back` returns to the menu after the
//! measurement so the next run starts from the same place.
//!
//! The whole run holds the render lock (one game, many sessions), and the
//! trace is stopped / cancelled on every exit path: a WPR session left
//! recording fills the disk of a box that has 24 GB free.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct Opts {
    pub map: String,
    pub outdir: PathBuf,
    pub tag: String,
    pub how: String,
    pub timeout_s: u64,
    pub dump_every_s: u64,
    /// `off`, a built-in WPR profile name (`CPU`, `GeneralProfile`), or
    /// `C:\path\file.wprp!Profile`; default: the bundled light profile.
    pub wpr: String,
    pub restart: bool,
    pub back: bool,
    pub tracerpt: bool,
    /// seconds to keep recording after the map has opened (the editor keeps
    /// working for a moment after the loading screen goes)
    pub settle_s: u64,
    pub detach: bool,
}

pub fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let val = |k: &str| -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() };
    let num = |k: &str, d: u64| -> Result<u64, String> { val(k).map(|s| s.parse::<u64>().map_err(|_| format!("{k} wants a number"))).transpose().map(|o| o.unwrap_or(d)) };
    let outdir = PathBuf::from(val("--outdir").ok_or("loadprof needs --outdir <dir under /mnt/c>")?);
    if !outdir.starts_with("/mnt/") {
        return Err(format!("--outdir {} must live under /mnt/<drive>/ — typeperf, wpr and the minidumps are Windows programs", outdir.display()));
    }
    let how = val("--how").unwrap_or_else(|| "edit".into());
    if how != "edit" && how != "play" {
        return Err(format!("--how {how}: edit or play"));
    }
    let tag = val("--tag").unwrap_or_else(|| "load".into());
    if !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("--tag {tag}: letters, digits, - and _ only"));
    }
    Ok(Opts {
        map: val("--map").ok_or("loadprof needs --map <path>")?,
        outdir,
        tag,
        how,
        timeout_s: num("--timeout", 900)?,
        dump_every_s: num("--dump-every", 10)?,
        wpr: val("--wpr").unwrap_or_else(|| "light".into()),
        restart: args.iter().any(|a| a == "--restart"),
        back: args.iter().any(|a| a == "--back"),
        tracerpt: args.iter().any(|a| a == "--tracerpt"),
        settle_s: num("--settle", 5)?,
        detach: args.iter().any(|a| a == "--detach"),
    })
}

pub fn run(args: &[String]) -> i32 {
    let opts = match parse_opts(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("usage: shootctl loadprof --map MAP --outdir /mnt/c/... [--tag T] [--how edit|play] [--timeout S] [--dump-every S] [--wpr light|off|CPU|FILE!Profile] [--settle S] [--restart] [--back] [--tracerpt] [--detach]");
            return 2;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&opts.outdir) {
        eprintln!("{}: {e}", opts.outdir.display());
        return 2;
    }
    let done = opts.outdir.join(format!("done-loadprof-{}.txt", opts.tag));
    let _ = std::fs::remove_file(&done);
    if opts.detach {
        return super::shootset::detach_as(&opts.outdir.join(format!("loadprof-{}.log", opts.tag)), &done);
    }
    let t0 = Instant::now();
    let result = run_prof(&opts, t0);
    let summary = match &result {
        Ok(lines) => format!("OK loadprof {} in {:.0}s\n{}\n", opts.tag, t0.elapsed().as_secs_f64(), lines.join("\n")),
        Err(e) => format!("FAILED after {:.0}s: {e}\n", t0.elapsed().as_secs_f64()),
    };
    print!("{summary}");
    let tmp = opts.outdir.join(format!("done-loadprof-{}.tmp", opts.tag));
    if std::fs::write(&tmp, &summary).and_then(|_| std::fs::rename(&tmp, &done)).is_err() {
        eprintln!("could not write {}", done.display());
        return 1;
    }
    if result.is_ok() { 0 } else { 1 }
}

/// The WSL path of a file under /mnt/c as the Windows programs want it.
fn win(p: &Path) -> String {
    let s = p.to_string_lossy();
    match s.strip_prefix("/mnt/c/") {
        Some(rest) => format!("C:\\{}", rest.replace('/', "\\")),
        None => s.into_owned(),
    }
}

fn unix_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

/// The game's pid from tasklist (CSV, no header).
fn game_pid() -> Option<u32> {
    let out = std::process::Command::new("/mnt/c/Windows/System32/tasklist.exe")
        .args(["/FI", "IMAGENAME eq Trackmania.exe", "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.to_lowercase().contains("trackmania.exe"))?;
    let pid = line.split(',').nth(1)?.trim().trim_matches('"');
    pid.parse().ok()
}

/// The bundled light WPR profile: sampled profile + stacks, process/thread/
/// image rundown, nothing else. The stock `CPU` profile also records every
/// context switch and ready-thread with stacks: 240 MB for five seconds on
/// this box (measured 2026-09-07), which no five-minute load survives.
const WPRP: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<WindowsPerformanceRecorder Version="1.0" Author="shootctl loadprof" Comments="sampled CPU profile with stacks, no context switches">
  <Profiles>
    <SystemCollector Id="SystemCollector_Light" Name="NT Kernel Logger">
      <BufferSize Value="1024" />
      <Buffers Value="256" />
    </SystemCollector>
    <SystemProvider Id="SystemProvider_Light">
      <Keywords>
        <Keyword Value="ProcessThread" />
        <Keyword Value="Loader" />
        <Keyword Value="SampledProfile" />
      </Keywords>
      <Stacks>
        <Stack Value="SampledProfile" />
      </Stacks>
    </SystemProvider>
    <SystemProvider Id="SystemProvider_Waits">
      <Keywords>
        <Keyword Value="ProcessThread" />
        <Keyword Value="Loader" />
        <Keyword Value="CSwitch" />
        <Keyword Value="ReadyThread" />
      </Keywords>
      <Stacks>
        <Stack Value="CSwitch" />
        <Stack Value="ReadyThread" />
      </Stacks>
    </SystemProvider>
    <Profile Id="Light.Verbose.File" Name="Light" Description="sampled profile with stacks" LoggingMode="File" DetailLevel="Verbose">
      <Collectors>
        <SystemCollectorId Value="SystemCollector_Light">
          <SystemProviderId Value="SystemProvider_Light" />
        </SystemCollectorId>
      </Collectors>
    </Profile>
    <Profile Id="Waits.Verbose.File" Name="Waits" Description="context switches and ready threads with stacks" LoggingMode="File" DetailLevel="Verbose">
      <Collectors>
        <SystemCollectorId Value="SystemCollector_Light">
          <SystemProviderId Value="SystemProvider_Waits" />
        </SystemCollectorId>
      </Collectors>
    </Profile>
  </Profiles>
</WindowsPerformanceRecorder>
"#;

struct Wpr {
    recording: bool,
}
impl Wpr {
    fn start(profile: &str, outdir: &Path) -> Result<Wpr, String> {
        let prof = match profile {
            "light" | "waits" => {
                let f = outdir.join("loadprof.wprp");
                std::fs::write(&f, WPRP).map_err(|e| format!("{}: {e}", f.display()))?;
                format!("{}!{}", win(&f), if profile == "light" { "Light" } else { "Waits" })
            }
            other => other.to_string(),
        };
        let out = std::process::Command::new("/mnt/c/Windows/System32/wpr.exe")
            .args(["-start", &prof, "-filemode"])
            .output()
            .map_err(|e| format!("wpr: {e}"))?;
        if !out.status.success() {
            return Err(format!("wpr -start {prof}: {}{}", String::from_utf8_lossy(&out.stdout).trim(), String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(Wpr { recording: true })
    }
    fn stop(&mut self, etl: &Path) -> Result<String, String> {
        if !self.recording {
            return Err("not recording".into());
        }
        self.recording = false;
        let out = std::process::Command::new("/mnt/c/Windows/System32/wpr.exe")
            .args(["-stop", &win(etl)])
            .output()
            .map_err(|e| format!("wpr: {e}"))?;
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout).trim(), String::from_utf8_lossy(&out.stderr).trim());
        if !out.status.success() {
            return Err(format!("wpr -stop: {text}"));
        }
        Ok(text)
    }
}
impl Drop for Wpr {
    fn drop(&mut self) {
        if self.recording {
            let _ = std::process::Command::new("/mnt/c/Windows/System32/wpr.exe").arg("-cancel").output();
        }
    }
}

struct Typeperf {
    child: std::process::Child,
}
impl Typeperf {
    fn start(outdir: &Path, tag: &str, pid: u32) -> Result<Typeperf, String> {
        let counters = [
            "\\Process(Trackmania)\\% Processor Time".to_string(),
            "\\Process(Trackmania)\\% User Time".into(),
            "\\Process(Trackmania)\\% Privileged Time".into(),
            "\\Process(Trackmania)\\IO Read Bytes/sec".into(),
            "\\Process(Trackmania)\\IO Read Operations/sec".into(),
            "\\Process(Trackmania)\\IO Write Bytes/sec".into(),
            "\\Process(Trackmania)\\IO Other Operations/sec".into(),
            "\\Process(Trackmania)\\Working Set".into(),
            "\\Process(Trackmania)\\Private Bytes".into(),
            "\\Process(Trackmania)\\Thread Count".into(),
            "\\Process(Trackmania)\\Page Faults/sec".into(),
            "\\Thread(Trackmania/*)\\% Processor Time".into(),
            "\\Thread(Trackmania/*)\\ID Thread".into(),
            "\\Thread(Trackmania/*)\\Thread State".into(),
            "\\Thread(Trackmania/*)\\Thread Wait Reason".into(),
            format!("\\GPU Engine(pid_{pid}*)\\Utilization Percentage"),
            "\\GPU Adapter Memory(*)\\Dedicated Usage".into(),
            "\\PhysicalDisk(_Total)\\Disk Read Bytes/sec".into(),
            "\\PhysicalDisk(_Total)\\Avg. Disk sec/Read".into(),
            "\\Processor(_Total)\\% Processor Time".into(),
            "\\Memory\\Available MBytes".into(),
        ];
        let cf = outdir.join(format!("counters-{tag}.txt"));
        std::fs::write(&cf, counters.join("\r\n") + "\r\n").map_err(|e| format!("{}: {e}", cf.display()))?;
        let csv = outdir.join(format!("perf-{tag}.csv"));
        let _ = std::fs::remove_file(&csv);
        let log = std::fs::File::create(outdir.join(format!("typeperf-{tag}.log"))).map_err(|e| format!("typeperf log: {e}"))?;
        let child = std::process::Command::new("/mnt/c/Windows/System32/typeperf.exe")
            .args(["-cf", &win(&cf), "-si", "1", "-y", "-o", &win(&csv)])
            .stdin(std::process::Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log)
            .spawn()
            .map_err(|e| format!("typeperf: {e}"))?;
        Ok(Typeperf { child })
    }
}
impl Drop for Typeperf {
    fn drop(&mut self) {
        // the WSL-side handle is a stub around the Windows process; kill both
        let _ = std::process::Command::new("/mnt/c/Windows/System32/taskkill.exe")
            .args(["/IM", "typeperf.exe", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One minidump of the game (`mini`: stacks + contexts + module list, no
/// heap — half a megabyte). The dumper suspends the process while it writes,
/// a few hundred milliseconds here.
fn minidump(pid: u32, file: &Path) -> Result<Duration, String> {
    let t = Instant::now();
    let out = std::process::Command::new("/mnt/c/Windows/System32/rundll32.exe")
        .args(["comsvcs.dll,", "MiniDump", &pid.to_string(), &win(file), "mini"])
        .output()
        .map_err(|e| format!("rundll32: {e}"))?;
    if !out.status.success() {
        return Err(format!("MiniDump {}: {}", file.display(), String::from_utf8_lossy(&out.stderr).trim()));
    }
    if !file.exists() {
        return Err(format!("MiniDump wrote nothing at {}", file.display()));
    }
    Ok(t.elapsed())
}

fn run_prof(opts: &Opts, t0: Instant) -> Result<Vec<String>, String> {
    let el = || format!("[{:6.1}s]", t0.elapsed().as_secs_f64());
    let d = super::lock::lock_dir();
    let owner = format!("loadprof-{}", opts.tag);
    super::lock::acquire(&d, &owner, 1500, 0).map_err(|e| format!("lock: {e}"))?;
    let _guard = super::shootset::LockGuard::new(d, owner);
    let mut lines = Vec::new();
    let mut timeline = String::new();
    let tl = |timeline: &mut String, s: String| {
        println!("{s}");
        timeline.push_str(&s);
        timeline.push('\n');
    };
    let staged = super::shootset::stage_map(&opts.map)?;
    let game_map = super::game_path(&staged)?;
    tl(&mut timeline, format!("{} map {} ({})", el(), game_map, opts.how));
    if opts.restart {
        tl(&mut timeline, format!("{} restarting the game for a cold load", el()));
        super::quit_game();
        std::thread::sleep(Duration::from_secs(2));
    }
    if super::launch(180, false) != 0 {
        return Err("the game did not come up".into());
    }
    super::to_menu()?;
    super::await_cond("ready", 60)?;
    let pid = game_pid().ok_or("no Trackmania.exe pid in tasklist")?;
    tl(&mut timeline, format!("{} game pid {pid}; restart={} (cold={})", el(), opts.restart, opts.restart));
    let store = "/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter";
    let _ = std::fs::create_dir_all(store);
    std::fs::write(format!("{store}/editmap.txt"), &game_map).map_err(|e| format!("editmap.txt: {e}"))?;

    // recorders up BEFORE the load is asked for
    let perf = Typeperf::start(&opts.outdir, &opts.tag, pid)?;
    let mut wpr = if opts.wpr == "off" {
        None
    } else {
        let t = Instant::now();
        let w = Wpr::start(&opts.wpr, &opts.outdir)?;
        tl(&mut timeline, format!("{} wpr -start {} took {:.1}s", el(), opts.wpr, t.elapsed().as_secs_f64()));
        Some(w)
    };
    // typeperf needs a moment for its first sample
    std::thread::sleep(Duration::from_millis(1500));

    let door = match opts.how.as_str() {
        "play" => "/playmap?mode=".to_string(),
        _ => "/editmap".to_string(),
    };
    let load0 = Instant::now();
    let load0_unix = unix_ms();
    tl(&mut timeline, format!("{} T0 unix_ms {load0_unix} {door}: {}", el(), super::http_get(&door, 30).unwrap_or_default().trim()));

    // the dumper, on its own thread, until told to stop
    let stop = Arc::new(AtomicBool::new(false));
    let dumper = if opts.dump_every_s > 0 {
        let (stop2, outdir, tag, every) = (stop.clone(), opts.outdir.clone(), opts.tag.clone(), opts.dump_every_s);
        Some(std::thread::spawn(move || {
            let mut rows = Vec::new();
            let mut k = 0;
            let mut next = load0 + Duration::from_secs(every);
            while !stop2.load(Ordering::Relaxed) {
                if Instant::now() < next {
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                }
                let at = load0.elapsed().as_secs_f64();
                let f = outdir.join(format!("dump-{tag}-{k:03}.dmp"));
                match minidump(pid, &f) {
                    Ok(took) => rows.push(format!("dump\t{k:03}\t{at:.1}\t{:.2}\t{}", took.as_secs_f64(), f.display())),
                    Err(e) => rows.push(format!("dump\t{k:03}\t{at:.1}\tFAILED\t{e}")),
                }
                k += 1;
                next += Duration::from_secs(every);
            }
            rows
        }))
    } else {
        None
    };

    let mut last = String::new();
    let mut opened: Option<Duration> = None;
    let mut gone = false;
    loop {
        if load0.elapsed().as_secs() > opts.timeout_s {
            tl(&mut timeline, format!("{} TIMEOUT: no {} in {} s; last ctx {}", el(), opts.how, opts.timeout_s, last));
            break;
        }
        if !super::tm_running() {
            tl(&mut timeline, format!("{} the game process is gone (crash) at +{:.1}s", el(), load0.elapsed().as_secs_f64()));
            gone = true;
            break;
        }
        let c = super::http_get("/ctx", 10).unwrap_or_default();
        let ready = super::http_get("/ready", 10).unwrap_or_default();
        let line = format!("{} | {}", c.trim(), ready.trim());
        if line != last {
            tl(&mut timeline, format!("{} +{:7.1}s {line}", el(), load0.elapsed().as_secs_f64()));
            last = line;
        }
        if c.contains("FrameAskYesNo") {
            let text = super::http_get("/dlgtext", 10).unwrap_or_default().trim().to_string();
            tl(&mut timeline, format!("{} DIALOG {text}", el()));
            let _ = super::http_get("/yes", 10);
        }
        if matches!(super::ctx(), Some(n) if n != 0) {
            let o = load0.elapsed();
            tl(&mut timeline, format!("{} OPENED after {:.1}s (ctx {})", el(), o.as_secs_f64(), c.trim()));
            opened = Some(o);
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    if opened.is_some() && opts.settle_s > 0 && !gone {
        std::thread::sleep(Duration::from_secs(opts.settle_s));
    }
    stop.store(true, Ordering::Relaxed);
    let t_end = load0.elapsed();
    // stop the recorders: wpr first (its stop is the slow one and the trace
    // must end before anything else happens to the game)
    if let Some(w) = wpr.as_mut() {
        let etl = opts.outdir.join(format!("trace-{}.etl", opts.tag));
        let t = Instant::now();
        match w.stop(&etl) {
            Ok(_) => {
                let size = std::fs::metadata(&etl).map(|m| m.len()).unwrap_or(0);
                tl(&mut timeline, format!("{} wpr -stop → {} ({} MB) took {:.1}s; trace covers T0-1.5s .. T0+{:.1}s", el(), etl.display(), size >> 20, t.elapsed().as_secs_f64(), t_end.as_secs_f64()));
                lines.push(format!("etl\t{}\t{}", etl.display(), size));
            }
            Err(e) => tl(&mut timeline, format!("{} wpr -stop FAILED: {e}", el())),
        }
    }
    drop(perf);
    if let Some(h) = dumper {
        if let Ok(rows) = h.join() {
            for r in &rows {
                timeline.push_str(r);
                timeline.push('\n');
            }
            lines.push(format!("dumps\t{}", rows.len()));
        }
    }
    let uid = super::loaded_uid().unwrap_or_else(|| "none".into());
    tl(&mut timeline, format!("{} loaded uid {uid}", el()));
    match opened {
        Some(o) => lines.push(format!("opened\t{:.1}\t{}\t{}", o.as_secs_f64(), opts.how, game_map)),
        None => lines.push(format!("opened\tNEVER\t{}\t{}", opts.how, game_map)),
    }
    lines.push(format!("perf\t{}", opts.outdir.join(format!("perf-{}.csv", opts.tag)).display()));
    let tlf = opts.outdir.join(format!("timeline-{}.txt", opts.tag));
    std::fs::write(&tlf, &timeline).map_err(|e| format!("{}: {e}", tlf.display()))?;
    lines.push(format!("timeline\t{}", tlf.display()));

    if opts.tracerpt && opts.wpr != "off" {
        let etl = opts.outdir.join(format!("trace-{}.etl", opts.tag));
        let csv = opts.outdir.join(format!("trace-{}.csv", opts.tag));
        let t = Instant::now();
        let out = std::process::Command::new("/mnt/c/Windows/System32/tracerpt.exe")
            .args([&win(&etl), "-o", &win(&csv), "-of", "CSV", "-y"])
            .output()
            .map_err(|e| format!("tracerpt: {e}"))?;
        let size = std::fs::metadata(&csv).map(|m| m.len()).unwrap_or(0);
        println!("{} tracerpt → {} ({} MB) in {:.0}s: {}", el(), csv.display(), size >> 20, t.elapsed().as_secs_f64(), String::from_utf8_lossy(&out.stdout).trim());
        lines.push(format!("csv\t{}\t{}", csv.display(), size));
    }
    if opts.back && !gone {
        super::to_menu()?;
        println!("{} back in the menu", el());
    }
    if gone {
        return Err("the game crashed during the load".into());
    }
    Ok(lines)
}
