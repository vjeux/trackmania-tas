//! wslwatch: restarts WhiteStick's WSL when it freezes behind an agent that
//! still looks online. It never restarts Windows.
//!
//! Oct 21 2026, 2:00 AM: the box's WSL stopped completing disk I/O. The
//! whitestick agent kept its relay connection (that needs no disk), so the box
//! read "online" for 18 hours while every command hung. Nothing inside WSL can
//! recover from that, so the watcher runs on the Windows side:
//!
//!   wslwatch watch      (Windows) every --interval s, run `wslwatch probe` in
//!                       the distro through wsl.exe. When probes have gone
//!                       unanswered for --hang-after s: `wsl.exe --shutdown`
//!                       (then `--force`), and check that WSL and the agent come
//!                       back. Limits: one restart per --min-gap, --max-per-day
//!                       per 24 h; a restart that does not bring WSL + the agent
//!                       back within --recover-within, or a cap hit, turns the
//!                       watcher OFF (log-only) and reports.
//!   wslwatch test-cycle (Windows) simulated hang -> real WSL restart -> WSL and
//!                       agent back. A pass removes the off switch: enforcing.
//!   wslwatch probe      (Linux, inside WSL) write + fsync + read back one file;
//!                       with --agent-since, also "the agent reconnected since".
//!   wslwatch off | on   the off switch, DIR\wslwatch.off: log-only while it exists.
//!   wslwatch install    (Windows) copy to %USERPROFILE%\.whitestick, HKCU Run
//!                       entry, start it LOG-ONLY (off switch set).
//!   wslwatch uninstall | status
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const USAGE: &str = "usage:
  wslwatch watch [--distro Ubuntu] [--user U] [--probe-bin PATH] [--interval 60]
                 [--probe-timeout 60] [--hang-after 300] [--cooldown 600]
                 [--min-gap 3600] [--max-per-day 3] [--recover-within 600]
                 [--dir DIR] [--dry-run] [--max-restarts N] [-- PROBE ARGV...]
  wslwatch test-cycle [--detach] [watch flags...]
  wslwatch probe [--file PATH] [--agent-since ISO] [--agent-log PATH]
  wslwatch off [--reason TEXT] [--dir DIR]
  wslwatch on [--dir DIR]
  wslwatch install [watch flags...]
  wslwatch uninstall
  wslwatch status [--dir DIR] [-n 20]
";
#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const RUN_VALUE: &str = "WhiteStick wslwatch";
/// probe exit code: WSL answered, but the agent has not reconnected yet
const AGENT_NOT_BACK: i32 = 5;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    let code = match args.first().map(String::as_str) {
        Some("watch") => WatchOpts::parse(rest).map_or_else(|e| fail(&e), watch),
        Some("test-cycle") if rest.iter().any(|a| a == "--detach") => detach_self("test-cycle", rest),
        Some("test-cycle") => WatchOpts::parse(rest).map_or_else(|e| fail(&e), test_cycle),
        Some("probe") => probe(rest),
        Some("off") => off_cmd(rest),
        Some("on") => on_cmd(rest),
        Some("install") => install(rest),
        Some("uninstall") => uninstall(),
        Some("status") => status(rest),
        Some("--version") | Some("-V") => {
            println!("wslwatch {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => fail(USAGE),
    };
    std::process::exit(code);
}

fn fail(msg: &str) -> i32 {
    eprintln!("{}", msg.trim_end());
    2
}

fn state_dir() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join(".whitestick")
}

fn wsl_exe() -> String {
    std::env::var("SystemRoot").map(|r| format!(r"{r}\System32\wsl.exe")).unwrap_or_else(|_| "wsl.exe".into())
}

/// `--dir DIR` from a flag list, else the state dir.
fn dir_flag(a: &[String]) -> Result<PathBuf, String> {
    match a.iter().position(|x| x == "--dir") {
        Some(i) => a.get(i + 1).map(PathBuf::from).ok_or_else(|| "--dir needs a value".into()),
        None => Ok(state_dir()),
    }
}

// ---------------------------------------------------------------- state files

struct Paths {
    dir: PathBuf,
}

impl Paths {
    fn log(&self) -> PathBuf {
        self.dir.join("wslwatch.log")
    }
    fn lock(&self) -> PathBuf {
        self.dir.join("wslwatch.lock")
    }
    fn test_lock(&self) -> PathBuf {
        self.dir.join("wslwatch-test.lock")
    }
    fn off(&self) -> PathBuf {
        self.dir.join("wslwatch.off")
    }
    fn restarts(&self) -> PathBuf {
        self.dir.join("wslwatch.restarts")
    }
}

/// Some(reason) while the off switch exists.
fn off_reason(p: &Paths) -> Option<String> {
    fs::read_to_string(p.off()).ok().map(|s| s.trim().to_string())
}

fn set_off(p: &Paths, reason: &str) -> std::io::Result<()> {
    fs::create_dir_all(&p.dir)?;
    fs::write(p.off(), format!("{} {reason}\n", utc_now()))
}

/// Ok(true) when an off switch was removed.
fn set_on(p: &Paths) -> std::io::Result<bool> {
    match fs::remove_file(p.off()) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Every WSL restart wslwatch made (watch and test-cycle alike), unix seconds.
fn history(p: &Paths) -> Vec<i64> {
    fs::read_to_string(p.restarts())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_whitespace().next()?.parse().ok())
        .collect()
}

fn record_restart(p: &Paths, at: i64, why: &str) {
    let _ = fs::create_dir_all(&p.dir);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p.restarts()) {
        let _ = writeln!(f, "{at} {} {why}", utc(at));
    }
}

#[derive(Debug, PartialEq)]
enum Gate {
    Go,
    /// seconds since the last restart, inside --min-gap
    Wait(i64),
    /// restarts in the last 24 h, at --max-per-day
    Cap(usize),
}

fn gate(hist: &[i64], now: i64, min_gap: i64, max_per_day: usize) -> Gate {
    let recent = hist.iter().filter(|&&t| now - t < 86_400).count();
    if recent >= max_per_day {
        return Gate::Cap(recent);
    }
    match hist.iter().filter(|&&t| now - t < min_gap).max() {
        Some(&last) => Gate::Wait(now - last),
        None => Gate::Go,
    }
}

// ---------------------------------------------------------------- decision

#[derive(Debug, PartialEq)]
enum Verdict {
    Ok,
    Failing,
    Restart,
}

/// Failures must span `hang_after` (at least two of them) before a restart;
/// after a restart, none for `cooldown` while the agent comes back.
struct Hang {
    hang_after: Duration,
    cooldown: Duration,
    first_fail: Option<Instant>,
    fails: u32,
    quiet_until: Option<Instant>,
}

impl Hang {
    fn new(hang_after: Duration, cooldown: Duration) -> Self {
        Hang { hang_after, cooldown, first_fail: None, fails: 0, quiet_until: None }
    }

    /// `at` = when the probe started.
    fn observe(&mut self, ok: bool, at: Instant) -> Verdict {
        if ok {
            self.first_fail = None;
            self.fails = 0;
            return Verdict::Ok;
        }
        self.fails += 1;
        let first = *self.first_fail.get_or_insert(at);
        if self.quiet_until.is_some_and(|q| at < q) {
            return Verdict::Failing;
        }
        if self.fails >= 2 && at.duration_since(first) >= self.hang_after {
            self.first_fail = None;
            self.fails = 0;
            self.quiet_until = Some(at + self.cooldown);
            return Verdict::Restart;
        }
        Verdict::Failing
    }
}

// ---------------------------------------------------------------- running things

/// Only silence is evidence of a freeze. A probe that fails FAST means WSL
/// answered (a missing or broken probe binary, say): restarting WSL would not
/// fix that, and doing it every 10 minutes would kill every job on the box.
/// A failure that took half the timeout or more (wsl.exe giving up on a
/// wedged VM) counts like a timeout.
fn is_hang(o: &Outcome, took: Duration, timeout: Duration) -> bool {
    match o {
        Outcome::TimedOut => true,
        Outcome::Exited { ok: false, .. } => took >= timeout / 2,
        Outcome::Exited { ok: true, .. } | Outcome::SpawnFailed(_) => false,
    }
}

enum Outcome {
    Exited { ok: bool, code: Option<i32>, out: String },
    TimedOut,
    SpawnFailed(String),
}

fn describe(o: &Outcome) -> String {
    match o {
        Outcome::Exited { ok: true, out, .. } => format!("ok {out}"),
        Outcome::Exited { code, out, .. } => format!("exit {}: {out}", code.map_or("?".into(), |c| c.to_string())),
        Outcome::TimedOut => "no answer, killed".into(),
        Outcome::SpawnFailed(e) => format!("could not start: {e}"),
    }
}

#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}
#[cfg(not(windows))]
fn no_window(_: &mut Command) {}

/// wsl.exe prints its own messages in UTF-16: drop the NULs, squeeze, cap.
fn clean(b: &[u8]) -> String {
    let s: String = String::from_utf8_lossy(b).chars().filter(|&c| c != '\0').collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    s.chars().take(300).collect()
}

fn run(argv: &[String], timeout: Duration) -> Outcome {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    no_window(&mut cmd);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Outcome::SpawnFailed(e.to_string()),
    };
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let pipes: Vec<Box<dyn Read + Send>> = [
        child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>),
        child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .collect();
    for mut pipe in pipes {
        let tx = tx.clone();
        thread::spawn(move || {
            let (mut keep, mut buf) = (Vec::new(), [0u8; 4096]);
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if keep.len() < 8192 {
                    keep.extend_from_slice(&buf[..n]);
                }
            }
            let _ = tx.send(keep);
        });
    }
    drop(tx);
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let mut out = Vec::new();
                let deadline = Instant::now() + Duration::from_secs(2);
                while let Ok(b) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    out.extend(b);
                }
                return Outcome::Exited { ok: st.success(), code: st.code(), out: clean(&out) };
            }
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Outcome::TimedOut;
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => return Outcome::SpawnFailed(e.to_string()),
        }
    }
}

// ---------------------------------------------------------------- log, lock, report

fn utc(secs: i64) -> String {
    let (days, sod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", sod / 3600, sod % 3600 / 60, sod % 60)
}

fn utc_now() -> String {
    utc(now_secs())
}

struct Log {
    path: PathBuf,
    prefix: &'static str,
}

impl Log {
    fn line(&self, msg: &str) {
        if let Some(d) = self.path.parent() {
            let _ = fs::create_dir_all(d);
        }
        if fs::metadata(&self.path).is_ok_and(|m| m.len() > 5_000_000) {
            let _ = fs::rename(&self.path, self.path.with_extension("log.1"));
        }
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = writeln!(f, "[{}] {}{msg}", utc_now(), self.prefix);
        }
    }
}

/// Windows: an exclusive open IS the single-instance lock, released when the
/// process dies however it dies. Elsewhere (tests) there is no exclusivity.
fn take_lock(p: &Path) -> std::io::Result<File> {
    if let Some(d) = p.parent() {
        let _ = fs::create_dir_all(d);
    }
    let mut o = OpenOptions::new();
    o.create(true).truncate(false).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.share_mode(0);
    }
    o.open(p)
}

/// The watcher stops acting: off switch with the reason, a log line, and a
/// message box on the box's screen (not topmost: it never steals a game's focus).
fn disarm(p: &Paths, log: &Log, reason: &str) {
    if let Err(e) = set_off(p, reason) {
        log.line(&format!("could not write the off switch {}: {e}", p.off().display()));
    }
    log.line(&format!("OFF, log-only from now on: {reason}"));
    notify(&format!("wslwatch stopped restarting WSL:\n\n{reason}\n\nLog: {}", p.log().display()));
}

#[cfg(windows)]
fn notify(text: &str) {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(owner: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let caption: Vec<u16> = "WhiteStick wslwatch".encode_utf16().chain(Some(0)).collect();
    // SAFETY: both buffers are NUL-terminated UTF-16 and live in the thread.
    thread::spawn(move || unsafe { MessageBoxW(0, text.as_ptr(), caption.as_ptr(), 0x30) }); // MB_OK | MB_ICONWARNING
}
#[cfg(not(windows))]
fn notify(_: &str) {}

// ---------------------------------------------------------------- watch

#[derive(Clone, Debug)]
struct WatchOpts {
    distro: String,
    user: String,
    probe_bin: Option<String>,
    interval: u64,
    probe_timeout: u64,
    hang_after: u64,
    cooldown: u64,
    min_gap: u64,
    max_per_day: usize,
    recover_within: u64,
    dir: PathBuf,
    dry_run: bool,
    /// Exit after this many restart decisions (dry-run ones count): bounds a
    /// test instance.
    max_restarts: Option<u32>,
    probe_argv: Vec<String>,
}

impl WatchOpts {
    fn parse(a: &[String]) -> Result<Self, String> {
        let mut o = WatchOpts {
            distro: "Ubuntu".into(),
            user: std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "vjeux".into()).to_lowercase(),
            probe_bin: None,
            interval: 60,
            probe_timeout: 60,
            hang_after: 300,
            cooldown: 600,
            min_gap: 3600,
            max_per_day: 3,
            recover_within: 600,
            dir: state_dir(),
            dry_run: false,
            max_restarts: None,
            probe_argv: vec![],
        };
        let mut i = 0;
        while i < a.len() {
            let f = a[i].as_str();
            if f == "--" {
                o.probe_argv = a[i + 1..].to_vec();
                break;
            }
            if f == "--dry-run" {
                o.dry_run = true;
                i += 1;
                continue;
            }
            let v = a.get(i + 1).ok_or_else(|| format!("{f} needs a value\n{USAGE}"))?.clone();
            let num = |v: &str| v.parse::<u64>().map_err(|_| format!("{f}: not a number: {v}"));
            match f {
                "--distro" => o.distro = v,
                "--user" => o.user = v,
                "--probe-bin" => o.probe_bin = Some(v),
                "--interval" => o.interval = num(&v)?.max(1),
                "--probe-timeout" => o.probe_timeout = num(&v)?.max(1),
                "--hang-after" => o.hang_after = num(&v)?,
                "--cooldown" => o.cooldown = num(&v)?,
                "--min-gap" => o.min_gap = num(&v)?,
                "--max-per-day" => o.max_per_day = num(&v)?.max(1) as usize,
                "--recover-within" => o.recover_within = num(&v)?.max(1),
                "--dir" => o.dir = v.into(),
                "--max-restarts" => o.max_restarts = Some(num(&v)?.clamp(1, u64::from(u32::MAX)) as u32),
                _ => return Err(format!("unknown flag {f}\n{USAGE}")),
            }
            i += 2;
        }
        Ok(o)
    }

    /// The real probe: ignores any `-- PROBE ARGV` override.
    fn real_probe(&self) -> Vec<String> {
        let bin = self.probe_bin.clone().unwrap_or_else(|| format!("/home/{}/bin/wslwatch", self.user));
        vec![wsl_exe(), "-d".into(), self.distro.clone(), "-u".into(), self.user.clone(), "--exec".into(), bin, "probe".into()]
    }

    fn probe_cmd(&self) -> Vec<String> {
        if self.probe_argv.is_empty() {
            self.real_probe()
        } else {
            self.probe_argv.clone()
        }
    }

    /// The probe that also requires the agent to have reconnected since `since`.
    fn agent_probe(&self, since: &str) -> Vec<String> {
        let mut v = self.real_probe();
        v.extend(["--agent-since".to_string(), since.to_string()]);
        v
    }

    fn paths(&self) -> Paths {
        Paths { dir: self.dir.clone() }
    }
}

fn watch(o: WatchOpts) -> i32 {
    let p = o.paths();
    let log = Log { path: p.log(), prefix: "" };
    let _lock = match take_lock(&p.lock()) {
        Ok(f) => f,
        Err(e) => {
            log.line(&format!("not starting: {} is held ({e}); another wslwatch is running", p.lock().display()));
            return 0;
        }
    };
    let probe = o.probe_cmd();
    log.line(&format!(
        "wslwatch {} pid {}: probe every {} s (timeout {} s); after {} s unanswered, restart WSL (never Windows): at most one per {} min, {} per 24 h, and WSL + agent must be back within {} s or it stops acting{}{}; probe = {}",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        o.interval,
        o.probe_timeout,
        o.hang_after,
        o.min_gap / 60,
        o.max_per_day,
        o.recover_within,
        if o.dry_run { "; DRY RUN" } else { "" },
        off_reason(&p).map(|r| format!("; OFF (log-only): {r}")).unwrap_or_default(),
        probe.join(" ")
    ));
    let mut hang = Hang::new(Duration::from_secs(o.hang_after), Duration::from_secs(o.cooldown));
    let (mut n_ok, mut slowest, mut beat, mut failing, mut was_broken) = (0u64, Duration::ZERO, Instant::now(), false, false);
    let mut restarts = 0u32;
    loop {
        let started = Instant::now();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&probe, Duration::from_secs(o.probe_timeout))))
            .unwrap_or_else(|_| Outcome::SpawnFailed("probe panicked".into()));
        let ok = matches!(outcome, Outcome::Exited { ok: true, .. });
        let took = started.elapsed();
        let hung = is_hang(&outcome, took, Duration::from_secs(o.probe_timeout));
        let broken = !ok && !hung;
        match hang.observe(!hung, started) {
            Verdict::Ok if broken => {
                if !was_broken {
                    log.line(&format!(
                        "probe broken, but WSL answered in {:.1} s, so this is no freeze and no restart: {}",
                        took.as_secs_f64(),
                        describe(&outcome)
                    ));
                }
                (was_broken, failing) = (true, false);
            }
            Verdict::Ok => {
                if failing || was_broken {
                    log.line(&format!("probe ok again ({:.1} s)", took.as_secs_f64()));
                }
                (was_broken, failing) = (false, false);
                n_ok += 1;
                slowest = slowest.max(took);
            }
            Verdict::Failing => {
                failing = true;
                log.line(&format!("probe FAILED ({} in a row, {:.1} s): {}", hang.fails, took.as_secs_f64(), describe(&outcome)));
            }
            Verdict::Restart => {
                failing = true;
                log.line(&format!("probe FAILED ({:.1} s): {}", took.as_secs_f64(), describe(&outcome)));
                act(&o, &p, &log);
                restarts += 1;
                if o.max_restarts.is_some_and(|m| restarts >= m) {
                    log.line(&format!("exiting after {restarts} restart decision(s) (--max-restarts)"));
                    return 0;
                }
            }
        }
        if beat.elapsed() >= Duration::from_secs(3600) {
            let state = off_reason(&p).map_or("enforcing".to_string(), |r| format!("OFF (log-only): {r}"));
            log.line(&format!("alive: {n_ok} probes ok in the last hour, slowest {:.1} s; {state}", slowest.as_secs_f64()));
            (n_ok, slowest, beat) = (0, Duration::ZERO, Instant::now());
        }
        thread::sleep(Duration::from_secs(o.interval).saturating_sub(started.elapsed()));
    }
}

/// A hang was confirmed: decide whether to restart WSL, and do it.
fn act(o: &WatchOpts, p: &Paths, log: &Log) {
    let why = format!("WSL has not answered for {} s", o.hang_after);
    if let Some(r) = off_reason(p) {
        log.line(&format!("{why}: OFF, not restarting (off switch {}: {r})", p.off().display()));
        return;
    }
    if o.dry_run {
        log.line(&format!("{why}: DRY RUN, would run wsl.exe --shutdown"));
        return;
    }
    match gate(&history(p), now_secs(), o.min_gap as i64, o.max_per_day) {
        Gate::Cap(n) => disarm(p, log, &format!("{why}, but wslwatch already restarted WSL {n} times in 24 h; `wslwatch.exe on` re-arms it")),
        Gate::Wait(ago) => log.line(&format!("{why}: not restarting, the last restart was {} min ago (at most one per {} min)", ago / 60, o.min_gap / 60)),
        Gate::Go => restart_and_verify(o, p, log, &why),
    }
}

fn restart_and_verify(o: &WatchOpts, p: &Paths, log: &Log, why: &str) {
    log.line(&format!("{why}: restarting WSL"));
    let (since, t0) = (utc_now(), Instant::now());
    record_restart(p, now_secs(), why);
    restart_wsl(log);
    match recover(&o.agent_probe(&since), o, t0) {
        Ok((wsl, agent)) => log.line(&format!("back: WSL answered {wsl:.0} s after the restart, the agent was connected at {agent:.0} s")),
        Err(e) => disarm(p, log, &format!("WSL or the agent was not back {} s after the restart at {since} ({e}), so it stopped acting instead of looping; `wslwatch.exe on` re-arms it", o.recover_within)),
    }
}

/// Poll `argv` (the agent probe) until it passes or --recover-within runs out.
/// Ok((s until WSL answered, s until the agent was connected)).
fn recover(argv: &[String], o: &WatchOpts, t0: Instant) -> Result<(f64, f64), String> {
    let deadline = t0 + Duration::from_secs(o.recover_within);
    let poll = Duration::from_secs(o.interval.min(5));
    let mut wsl_back = None::<f64>;
    loop {
        let t = Instant::now();
        let out = run(argv, Duration::from_secs(o.probe_timeout.min(o.recover_within)));
        let at = t0.elapsed().as_secs_f64();
        match &out {
            Outcome::Exited { ok: true, .. } => return Ok((wsl_back.unwrap_or(at), at)),
            Outcome::Exited { code: Some(AGENT_NOT_BACK), .. } => {
                wsl_back.get_or_insert(at);
            }
            _ => {}
        }
        let last = describe(&out);
        if Instant::now() >= deadline {
            return Err(match wsl_back {
                Some(w) => format!("WSL answered at {w:.0} s, the agent never reconnected: {last}"),
                None => format!("WSL never answered: {last}"),
            });
        }
        thread::sleep(poll.saturating_sub(t.elapsed()));
    }
}

fn restart_wsl(log: &Log) {
    let wsl = wsl_exe();
    let first = run(&[wsl.clone(), "--shutdown".into()], Duration::from_secs(120));
    if let Outcome::Exited { ok: true, .. } = first {
        log.line("wsl.exe --shutdown done; the Startup-folder .vbs loop restarts the agent");
        return;
    }
    log.line(&format!("wsl.exe --shutdown: {}; forcing", describe(&first)));
    let forced = run(&[wsl, "--shutdown".into(), "--force".into()], Duration::from_secs(120));
    log.line(&format!("wsl.exe --shutdown --force: {}", describe(&forced)));
}

// ---------------------------------------------------------------- test cycle

/// The controlled cycle that must pass before the watcher enforces: a
/// simulated hang (a probe that never answers) is detected, WSL is really
/// restarted, and WSL + the agent come back. Pass: the off switch goes.
fn test_cycle(o: WatchOpts) -> i32 {
    let p = o.paths();
    let log = Log { path: p.log(), prefix: "test-cycle: " };
    let _lock = match take_lock(&p.test_lock()) {
        Ok(f) => f,
        Err(e) => {
            log.line(&format!("not starting: another test cycle holds {} ({e})", p.test_lock().display()));
            return 1;
        }
    };
    let fake = if o.probe_argv.is_empty() {
        vec![wsl_exe(), "-d".into(), o.distro.clone(), "-u".into(), o.user.clone(), "--exec".into(), "/bin/sleep".into(), "30".into()]
    } else {
        o.probe_argv.clone()
    };
    let (step, window) = (Duration::from_secs(5), Duration::from_secs(20));
    log.line(&format!("wslwatch {} pid {}: 1/3 simulated hang, probe = {} (killed after 5 s), window 20 s", env!("CARGO_PKG_VERSION"), std::process::id(), fake.join(" ")));
    let (mut hang, t_start) = (Hang::new(window, Duration::from_secs(600)), Instant::now());
    loop {
        let t = Instant::now();
        let out = run(&fake, step);
        let hung = is_hang(&out, t.elapsed(), step);
        match hang.observe(!hung, t) {
            Verdict::Restart => break,
            Verdict::Ok => {
                log.line(&format!("FAIL: the simulated hang answered ({}); nothing restarted", describe(&out)));
                return 1;
            }
            Verdict::Failing => log.line(&format!("simulated probe unanswered ({} in a row)", hang.fails)),
        }
        if t_start.elapsed() > Duration::from_secs(120) {
            log.line("FAIL: no restart decision within 120 s; nothing restarted");
            return 1;
        }
        thread::sleep(step.saturating_sub(t.elapsed()));
    }
    log.line(&format!("hang detected after {:.0} s", t_start.elapsed().as_secs_f64()));
    if o.dry_run {
        log.line("2/3 DRY RUN: would run wsl.exe --shutdown; the off switch is unchanged");
        return 0;
    }
    if let Gate::Cap(n) = gate(&history(&p), now_secs(), 0, o.max_per_day) {
        log.line(&format!("FAIL: wslwatch already restarted WSL {n} times in 24 h; not restarting"));
        return 1;
    }
    log.line("2/3 restarting WSL for real");
    let (since, t0) = (utc_now(), Instant::now());
    record_restart(&p, now_secs(), "test-cycle");
    restart_wsl(&log);
    match recover(&o.agent_probe(&since), &o, t0) {
        Ok((wsl, agent)) => {
            log.line(&format!("3/3 PASS: WSL answered {wsl:.0} s after the restart, the agent was connected at {agent:.0} s"));
            match set_on(&p) {
                Ok(true) => log.line("off switch removed: ENFORCING"),
                Ok(false) => log.line("there was no off switch: already enforcing"),
                Err(e) => log.line(&format!("could not remove the off switch {}: {e}; still log-only", p.off().display())),
            }
            0
        }
        Err(e) => {
            disarm(&p, &log, &format!("test cycle FAILED: {e}"));
            1
        }
    }
}

// ---------------------------------------------------------------- probe

/// The agent's last connection event in its log: (timestamp, connected?).
fn agent_state(text: &str) -> Option<(String, bool)> {
    text.lines().rev().find_map(|l| {
        let (ts, msg) = l.strip_prefix('[')?.split_once("] ")?;
        if msg.starts_with("connected to relay") {
            Some((ts.to_string(), true))
        } else if msg.starts_with("disconnected") {
            Some((ts.to_string(), false))
        } else {
            None
        }
    })
}

fn probe(a: &[String]) -> i32 {
    let (mut path, mut since, mut agent_log) = (state_dir().join("wslwatch-probe"), None::<String>, state_dir().join("agent.log"));
    let mut i = 0;
    while i < a.len() {
        let v = match a.get(i + 1) {
            Some(v) => v.clone(),
            None => return fail(USAGE),
        };
        match a[i].as_str() {
            "--file" => path = v.into(),
            "--agent-since" => since = Some(v),
            "--agent-log" => agent_log = v.into(),
            _ => return fail(USAGE),
        }
        i += 2;
    }
    let t0 = Instant::now();
    let stamp = format!("{} {}\n", utc_now(), std::process::id());
    let tmp = path.with_extension("tmp");
    let r = (|| -> std::io::Result<()> {
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        fs::create_dir_all(&dir)?;
        {
            let mut f = File::create(&tmp)?;
            f.write_all(stamp.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &path)?;
        #[cfg(unix)]
        File::open(&dir)?.sync_all()?;
        if fs::read_to_string(&path)? != stamp {
            return Err(std::io::Error::other("read back something else"));
        }
        Ok(())
    })();
    if let Err(e) = r {
        eprintln!("probe failed: {e} ({})", path.display());
        return 1;
    }
    let disk = t0.elapsed().as_secs_f64();
    let Some(since) = since else {
        println!("ok {disk:.3} s {}", path.display());
        return 0;
    };
    match agent_state(&fs::read_to_string(&agent_log).unwrap_or_default()) {
        Some((ts, true)) if ts.as_str() >= since.as_str() => {
            println!("ok {disk:.3} s; agent connected at {ts}");
            0
        }
        state => {
            println!("disk ok {disk:.3} s; agent not reconnected since {since} (last event: {state:?})");
            AGENT_NOT_BACK
        }
    }
}

// ---------------------------------------------------------------- off / on / status

fn off_cmd(a: &[String]) -> i32 {
    let p = match dir_flag(a) {
        Ok(d) => Paths { dir: d },
        Err(e) => return fail(&e),
    };
    let reason = a.iter().position(|x| x == "--reason").and_then(|i| a.get(i + 1)).cloned().unwrap_or_else(|| "turned off by hand".into());
    match set_off(&p, &reason) {
        Ok(()) => {
            Log { path: p.log(), prefix: "" }.line(&format!("off switch set: {reason}"));
            println!("OFF: {} (log-only until `wslwatch on`)", p.off().display());
            0
        }
        Err(e) => fail(&format!("{}: {e}", p.off().display())),
    }
}

fn on_cmd(a: &[String]) -> i32 {
    let p = match dir_flag(a) {
        Ok(d) => Paths { dir: d },
        Err(e) => return fail(&e),
    };
    match set_on(&p) {
        Ok(removed) => {
            Log { path: p.log(), prefix: "" }.line(if removed { "off switch removed by hand: enforcing" } else { "on: there was no off switch" });
            println!("enforcing{}", if removed { "" } else { " (there was no off switch)" });
            0
        }
        Err(e) => fail(&format!("{}: {e}", p.off().display())),
    }
}

fn status(a: &[String]) -> i32 {
    let p = match dir_flag(a) {
        Ok(d) => Paths { dir: d },
        Err(e) => return fail(&e),
    };
    let n = a.iter().position(|x| x == "-n").and_then(|i| a.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(20usize);
    let running = cfg!(windows) && take_lock(&p.lock()).is_err();
    if cfg!(windows) {
        println!("wslwatch: {}", if running { "running" } else { "NOT running" });
    }
    let off = off_reason(&p);
    match &off {
        Some(r) => println!("OFF (log-only): {r}"),
        None => println!("enforcing (no off switch)"),
    }
    let now = now_secs();
    let day: Vec<String> = history(&p).into_iter().filter(|&t| now - t < 86_400).map(utc).collect();
    println!("WSL restarts in the last 24 h: {}{}", day.len(), if day.is_empty() { String::new() } else { format!(" ({})", day.join(", ")) });
    let text = fs::read_to_string(p.log()).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    for l in &lines[lines.len().saturating_sub(n)..] {
        println!("{l}");
    }
    match (cfg!(windows), running, off.is_some()) {
        (true, false, _) => 3,
        (_, _, true) => 4,
        _ => 0,
    }
}

// ---------------------------------------------------------------- install, detach

#[cfg(windows)]
fn reg(args: &[&str]) -> bool {
    let mut c = Command::new("reg.exe");
    c.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    no_window(&mut c);
    c.status().is_ok_and(|s| s.success())
}

#[cfg(windows)]
fn stop_others() {
    let mut c = Command::new("taskkill.exe");
    c.args(["/F", "/FI", "IMAGENAME eq wslwatch.exe", "/FI", &format!("PID ne {}", std::process::id())]);
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    no_window(&mut c);
    let _ = c.status();
    thread::sleep(Duration::from_millis(500));
}

/// CreateProcess inherits every inheritable handle, and the std handles WSL
/// interop gives us are inheritable pipes: a child holding them keeps the
/// caller (a whitestick command) from ever seeing EOF. Unmark them first.
#[cfg(windows)]
fn stop_inheriting_stdio() {
    extern "system" {
        fn GetStdHandle(which: u32) -> isize;
        fn SetHandleInformation(handle: isize, mask: u32, flags: u32) -> i32;
    }
    for which in [-10i32, -11, -12] {
        // SAFETY: plain kernel32 calls on our own std handles; null or
        // INVALID_HANDLE_VALUE is skipped.
        unsafe {
            let h = GetStdHandle(which as u32);
            if h != 0 && h != -1 {
                SetHandleInformation(h, 1, 0); // HANDLE_FLAG_INHERIT off
            }
        }
    }
}

#[cfg(windows)]
fn spawn_detached(exe: &Path, args: &[String]) -> std::io::Result<u32> {
    use std::os::windows::process::CommandExt;
    let _ = std::io::stdout().flush();
    stop_inheriting_stdio();
    const DETACHED: u32 = 0x0000_0008 | 0x0000_0200; // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    const BREAKAWAY: u32 = 0x0100_0000; // CREATE_BREAKAWAY_FROM_JOB
    let mk = |flags: u32| {
        let mut c = Command::new(exe);
        c.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(flags);
        c
    };
    mk(DETACHED | BREAKAWAY).spawn().or_else(|_| mk(DETACHED).spawn()).map(|c| c.id())
}

/// `test-cycle --detach ...`: the cycle shuts WSL down, which would kill a
/// caller running inside WSL, so it continues as its own Windows process.
#[cfg(windows)]
fn detach_self(sub: &str, rest: &[String]) -> i32 {
    let mut args = vec![sub.to_string()];
    args.extend(rest.iter().filter(|a| *a != "--detach").cloned());
    if let Err(e) = WatchOpts::parse(&args[1..]) {
        return fail(&e);
    }
    let me = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return fail(&format!("current_exe: {e}")),
    };
    match spawn_detached(&me, &args) {
        Ok(pid) => {
            println!("{sub} running detached as pid {pid}; follow it with `wslwatch.exe status`");
            0
        }
        Err(e) => fail(&format!("could not start {sub}: {e}")),
    }
}
#[cfg(not(windows))]
fn detach_self(_: &str, _: &[String]) -> i32 {
    fail("--detach is Windows-only")
}

#[cfg(windows)]
fn install(a: &[String]) -> i32 {
    let o = match WatchOpts::parse(a) {
        Ok(o) => o,
        Err(e) => return fail(&e),
    };
    let dir = state_dir();
    let _ = fs::create_dir_all(&dir);
    let dest = dir.join("wslwatch.exe");
    let me = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return fail(&format!("current_exe: {e}")),
    };
    stop_others();
    if fs::canonicalize(&me).ok() != fs::canonicalize(&dest).ok() {
        if dest.exists() {
            let old = dir.join("wslwatch.old.exe");
            let _ = fs::remove_file(&old);
            if let Err(e) = fs::rename(&dest, &old) {
                return fail(&format!("cannot move the old {} aside: {e}", dest.display()));
            }
        }
        if let Err(e) = fs::copy(&me, &dest) {
            return fail(&format!("copy to {}: {e}", dest.display()));
        }
    }
    let p = o.paths();
    let reason = format!("installed wslwatch {} log-only; `wslwatch.exe test-cycle --detach` must pass before it acts", env!("CARGO_PKG_VERSION"));
    if let Err(e) = set_off(&p, &reason) {
        return fail(&format!("could not set the off switch {}: {e}; not installing", p.off().display()));
    }
    println!("OFF (log-only): {}", p.off().display());
    let exe = dest.display().to_string();
    let mut cmdline = if exe.contains(' ') { format!("\"{exe}\" watch") } else { format!("{exe} watch") };
    for x in a {
        cmdline.push(' ');
        cmdline.push_str(x);
    }
    if !reg(&["add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &cmdline, "/f"]) {
        return fail("reg.exe add failed");
    }
    println!("{RUN_KEY}\\{RUN_VALUE} = {cmdline}");
    let mut args = vec!["watch".to_string()];
    args.extend(a.iter().cloned());
    match spawn_detached(&dest, &args) {
        Ok(pid) => {
            println!("started pid {pid}");
            0
        }
        Err(e) => fail(&format!("installed, but could not start it: {e}")),
    }
}

#[cfg(windows)]
fn uninstall() -> i32 {
    let removed = reg(&["delete", RUN_KEY, "/v", RUN_VALUE, "/f"]);
    stop_others();
    println!("Run entry {}; running copies stopped", if removed { "removed" } else { "was not there" });
    0
}

#[cfg(not(windows))]
fn install(_: &[String]) -> i32 {
    fail("install is Windows-only: run wslwatch.exe install on the box")
}

#[cfg(not(windows))]
fn uninstall() -> i32 {
    fail("uninstall is Windows-only")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wslwatch-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn strs(x: &[&str]) -> Vec<String> {
        x.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_success_resets_the_window() {
        let t = Instant::now();
        let mut h = Hang::new(s(300), s(600));
        assert_eq!(h.observe(false, t), Verdict::Failing);
        assert_eq!(h.observe(false, t + s(240)), Verdict::Failing);
        assert_eq!(h.observe(true, t + s(280)), Verdict::Ok);
        assert_eq!(h.observe(false, t + s(320)), Verdict::Failing);
        assert_eq!(h.observe(false, t + s(560)), Verdict::Failing);
        assert_eq!(h.observe(false, t + s(620)), Verdict::Restart);
    }

    #[test]
    fn five_minutes_of_timeouts_restart_once() {
        let t = Instant::now();
        let mut h = Hang::new(s(300), s(600));
        for k in 0..5 {
            assert_eq!(h.observe(false, t + s(60 * k)), Verdict::Failing, "probe {k}");
        }
        assert_eq!(h.observe(false, t + s(300)), Verdict::Restart);
    }

    #[test]
    fn one_failure_alone_never_restarts() {
        let t = Instant::now();
        let mut h = Hang::new(s(0), s(600));
        assert_eq!(h.observe(false, t), Verdict::Failing);
        assert_eq!(h.observe(true, t + s(60)), Verdict::Ok);
    }

    #[test]
    fn quiet_after_a_restart() {
        let t = Instant::now();
        let mut h = Hang::new(s(300), s(600));
        assert_eq!(h.observe(false, t), Verdict::Failing);
        assert_eq!(h.observe(false, t + s(300)), Verdict::Restart);
        for k in 1..10 {
            assert_eq!(h.observe(false, t + s(300 + 60 * k)), Verdict::Failing, "quiet probe {k}");
        }
        assert_eq!(h.observe(false, t + s(900)), Verdict::Restart);
    }

    #[test]
    fn rate_limits() {
        let now = 1_000_000;
        assert_eq!(gate(&[], now, 3600, 3), Gate::Go);
        assert_eq!(gate(&[now - 600], now, 3600, 3), Gate::Wait(600));
        assert_eq!(gate(&[now - 7200], now, 3600, 3), Gate::Go);
        assert_eq!(gate(&[now - 80_000, now - 40_000, now - 7200], now, 3600, 3), Gate::Cap(3));
        assert_eq!(gate(&[now - 90_000, now - 40_000, now - 7200], now, 3600, 3), Gate::Go, "a restart over 24 h old does not count");
    }

    #[test]
    fn history_and_off_switch_round_trip() {
        let p = Paths { dir: tmpdir("state") };
        assert!(off_reason(&p).is_none());
        set_off(&p, "maintenance").unwrap();
        assert!(off_reason(&p).unwrap().ends_with("maintenance"));
        assert!(set_on(&p).unwrap());
        assert!(!set_on(&p).unwrap());
        record_restart(&p, 1_000, "a");
        record_restart(&p, 2_000, "b");
        assert_eq!(history(&p), vec![1_000, 2_000]);
        let _ = fs::remove_dir_all(&p.dir);
    }

    #[test]
    fn the_off_switch_wins_over_a_hang() {
        let dir = tmpdir("off");
        let p = Paths { dir: dir.clone() };
        set_off(&p, "test").unwrap();
        let mut args = strs(&["--interval", "1", "--probe-timeout", "1", "--hang-after", "1", "--max-restarts", "1", "--dir"]);
        args.push(dir.display().to_string());
        args.extend(strs(&["--", "sleep", "5"]));
        assert_eq!(watch(WatchOpts::parse(&args).unwrap()), 0);
        let log = fs::read_to_string(p.log()).unwrap();
        assert!(log.contains("OFF, not restarting"), "{log}");
        assert!(history(&p).is_empty(), "nothing was restarted");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn max_restarts_ends_a_dry_run_instance() {
        let dir = tmpdir("max");
        let mut args = strs(&["--dry-run", "--interval", "1", "--probe-timeout", "1", "--hang-after", "1", "--max-restarts", "1", "--dir"]);
        args.push(dir.display().to_string());
        args.extend(strs(&["--", "sleep", "5"]));
        let t0 = Instant::now();
        assert_eq!(watch(WatchOpts::parse(&args).unwrap()), 0);
        assert!(t0.elapsed() < s(10), "took {:?}", t0.elapsed());
        let log = fs::read_to_string(dir.join("wslwatch.log")).unwrap();
        assert!(log.contains("DRY RUN, would run wsl.exe --shutdown"), "{log}");
        assert!(log.contains("exiting after 1 restart decision(s)"), "{log}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn agent_log_last_event() {
        let text = "[2026-10-22T16:08:01Z] whitestick agent 1.0.0\n[2026-10-22T16:08:02Z] connected to relay\n[2026-10-22T16:08:05Z] [0005d] pid 1: ls\n";
        assert_eq!(agent_state(text), Some(("2026-10-22T16:08:02Z".into(), true)));
        let later = format!("{text}[2026-10-22T16:09:00Z] disconnected after 58 s: nothing heard\n");
        assert_eq!(agent_state(&later), Some(("2026-10-22T16:09:00Z".into(), false)));
        assert_eq!(agent_state("[x] nothing\n"), None);
    }

    #[test]
    fn probe_checks_the_agent_since() {
        let dir = tmpdir("agent");
        let (file, log) = (dir.join("probe"), dir.join("agent.log"));
        fs::write(&log, "[2026-10-22T16:08:02Z] connected to relay\n").unwrap();
        let run_probe = |since: &str| probe(&strs(&["--file", &file.display().to_string(), "--agent-log", &log.display().to_string(), "--agent-since", since]));
        assert_eq!(run_probe("2026-10-22T16:07:44Z"), 0);
        assert_eq!(run_probe("2026-10-22T16:08:03Z"), AGENT_NOT_BACK);
        assert_eq!(probe(&strs(&["--file", &file.display().to_string()])), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn recovery_passes_or_gives_up() {
        let mut o = WatchOpts::parse(&strs(&["--interval", "1", "--probe-timeout", "1", "--recover-within", "2"])).unwrap();
        assert!(recover(&strs(&["true"]), &o, Instant::now()).is_ok());
        let t0 = Instant::now();
        let e = recover(&strs(&["false"]), &o, t0).unwrap_err();
        assert!(e.contains("WSL never answered"), "{e}");
        assert!(t0.elapsed() < s(6));
        o.recover_within = 1;
        assert!(recover(&strs(&["sleep", "5"]), &o, Instant::now()).is_err());
    }

    #[test]
    fn utc_formats() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_792_680_177), "2026-10-22T14:42:57Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn a_hung_command_is_killed_at_the_timeout() {
        let t0 = Instant::now();
        let o = run(&strs(&["sleep", "30"]), s(1));
        assert!(matches!(o, Outcome::TimedOut));
        assert!(t0.elapsed() < s(5));
    }

    #[test]
    fn only_silence_counts_as_a_freeze() {
        let failed = |secs| (Outcome::Exited { ok: false, code: Some(1), out: String::new() }, s(secs));
        let t = s(60);
        assert!(is_hang(&Outcome::TimedOut, s(60), t));
        let (o, took) = failed(0);
        assert!(!is_hang(&o, took, t), "a fast failure means WSL answered");
        let (o, took) = failed(45);
        assert!(is_hang(&o, took, t), "wsl.exe giving up after 45 s is a freeze");
        assert!(!is_hang(&Outcome::SpawnFailed("x".into()), s(0), t));
        assert!(!is_hang(&Outcome::Exited { ok: true, code: Some(0), out: String::new() }, s(59), t));
    }

    #[test]
    fn flags_parse() {
        let o = WatchOpts::parse(&strs(&["--interval", "5", "--dry-run", "--", "sleep", "9"])).unwrap();
        assert_eq!((o.interval, o.dry_run, o.max_restarts, o.min_gap, o.max_per_day, o.recover_within), (5, true, None, 3600, 3, 600));
        assert_eq!(o.probe_cmd(), strs(&["sleep", "9"]));
        assert_eq!(o.agent_probe("T").last().unwrap(), "T");
        assert!(o.agent_probe("T").contains(&"probe".to_string()), "the agent probe is always the real one");
        assert!(WatchOpts::parse(&strs(&["--bogus", "1"])).is_err());
    }
}
