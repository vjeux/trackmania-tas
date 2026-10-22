//! wslwatch: restarts WhiteStick's WSL when it freezes behind an agent that
//! still looks online.
//!
//! Oct 21 2026, 2:00 AM: the box's WSL stopped completing disk I/O. The
//! whitestick agent kept its relay connection (that needs no disk), so the box
//! read "online" for 18 hours while every command hung. Nothing inside WSL can
//! recover from that, so the watcher runs on the Windows side:
//!
//!   wslwatch watch     (Windows) every --interval s, run `wslwatch probe` in
//!                      the distro through wsl.exe. When probes have failed for
//!                      --hang-after s: `wsl.exe --shutdown`, then `--force`.
//!                      The Startup-folder .vbs loop brings the agent back.
//!   wslwatch probe     (Linux, inside WSL) write + fsync + read back one file.
//!   wslwatch install   (Windows) copy to %USERPROFILE%\.whitestick, register in
//!                      HKCU\...\Run, stop an older copy, start the new one.
//!   wslwatch uninstall (Windows) remove the Run entry, stop it.
//!   wslwatch status    is one running? + the log tail.
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
                 [--log FILE] [--lock FILE] [--dry-run] [--max-restarts N]
                 [-- PROBE ARGV...]
  wslwatch probe [--file PATH]
  wslwatch install [watch flags...]
  wslwatch uninstall
  wslwatch status [--log FILE] [--lock FILE] [-n 20]
";
#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const RUN_VALUE: &str = "WhiteStick wslwatch";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    let code = match args.first().map(String::as_str) {
        Some("watch") => match WatchOpts::parse(rest) {
            Ok(o) => watch(o),
            Err(e) => fail(&e),
        },
        Some("probe") => probe(rest),
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

// ---------------------------------------------------------------- log + lock

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
    utc(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64))
}

struct Log {
    path: PathBuf,
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
            let _ = writeln!(f, "[{}] {msg}", utc_now());
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
    log: PathBuf,
    lock: PathBuf,
    dry_run: bool,
    /// Exit after this many restarts (dry-run ones count): bounds a test
    /// instance, which otherwise outlives the WSL it just shut down.
    max_restarts: Option<u32>,
    probe_argv: Vec<String>,
}

impl WatchOpts {
    fn parse(a: &[String]) -> Result<Self, String> {
        let dir = state_dir();
        let mut o = WatchOpts {
            distro: "Ubuntu".into(),
            user: std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "vjeux".into()).to_lowercase(),
            probe_bin: None,
            interval: 60,
            probe_timeout: 60,
            hang_after: 300,
            cooldown: 600,
            log: dir.join("wslwatch.log"),
            lock: dir.join("wslwatch.lock"),
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
                "--max-restarts" => o.max_restarts = Some(num(&v)?.clamp(1, u64::from(u32::MAX)) as u32),
                "--log" => o.log = v.into(),
                "--lock" => o.lock = v.into(),
                _ => return Err(format!("unknown flag {f}\n{USAGE}")),
            }
            i += 2;
        }
        Ok(o)
    }

    fn probe_cmd(&self) -> Vec<String> {
        if !self.probe_argv.is_empty() {
            return self.probe_argv.clone();
        }
        let bin = self.probe_bin.clone().unwrap_or_else(|| format!("/home/{}/bin/wslwatch", self.user));
        vec![wsl_exe(), "-d".into(), self.distro.clone(), "-u".into(), self.user.clone(), "--exec".into(), bin, "probe".into()]
    }
}

fn watch(o: WatchOpts) -> i32 {
    let log = Log { path: o.log.clone() };
    let _lock = match take_lock(&o.lock) {
        Ok(f) => f,
        Err(e) => {
            log.line(&format!("not starting: {} is held ({e}); another wslwatch is running", o.lock.display()));
            return 0;
        }
    };
    let probe = o.probe_cmd();
    log.line(&format!(
        "wslwatch {} pid {}: probe every {} s (timeout {} s), restart WSL after {} s of failures, then quiet {} s{}; probe = {}",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        o.interval,
        o.probe_timeout,
        o.hang_after,
        o.cooldown,
        if o.dry_run { "; DRY RUN" } else { "" },
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
                if o.dry_run {
                    log.line(&format!("WSL has not answered for {} s: DRY RUN, would run wsl.exe --shutdown", o.hang_after));
                } else {
                    log.line(&format!("WSL has not answered for {} s: restarting it", o.hang_after));
                    restart_wsl(&log);
                }
                restarts += 1;
                if o.max_restarts.is_some_and(|m| restarts >= m) {
                    log.line(&format!("exiting after {restarts} restart(s) (--max-restarts)"));
                    return 0;
                }
            }
        }
        if beat.elapsed() >= Duration::from_secs(3600) {
            log.line(&format!("alive: {n_ok} probes ok in the last hour, slowest {:.1} s", slowest.as_secs_f64()));
            (n_ok, slowest, beat) = (0, Duration::ZERO, Instant::now());
        }
        thread::sleep(Duration::from_secs(o.interval).saturating_sub(started.elapsed()));
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

// ---------------------------------------------------------------- probe

fn probe(a: &[String]) -> i32 {
    let path = match a {
        [] => state_dir().join("wslwatch-probe"),
        [f, p] if f == "--file" => PathBuf::from(p),
        _ => return fail(USAGE),
    };
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
    match r {
        Ok(()) => {
            println!("ok {:.3} s {}", t0.elapsed().as_secs_f64(), path.display());
            0
        }
        Err(e) => {
            eprintln!("probe failed: {e} ({})", path.display());
            1
        }
    }
}

// ---------------------------------------------------------------- status

fn status(a: &[String]) -> i32 {
    let dir = state_dir();
    let (mut log, mut lock, mut n) = (dir.join("wslwatch.log"), dir.join("wslwatch.lock"), 20usize);
    let mut i = 0;
    while i + 1 < a.len() {
        match a[i].as_str() {
            "--log" => log = a[i + 1].clone().into(),
            "--lock" => lock = a[i + 1].clone().into(),
            "-n" => n = a[i + 1].parse().unwrap_or(20),
            _ => return fail(USAGE),
        }
        i += 2;
    }
    if i != a.len() {
        return fail(USAGE);
    }
    let running = if cfg!(windows) { take_lock(&lock).is_err() } else { false };
    if cfg!(windows) {
        println!("wslwatch: {}", if running { "running" } else { "NOT running" });
    } else {
        println!("wslwatch: (the running check is Windows-only)");
    }
    let text = fs::read_to_string(&log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    for l in &lines[lines.len().saturating_sub(n)..] {
        println!("{l}");
    }
    if running || !cfg!(windows) {
        0
    } else {
        3
    }
}

// ---------------------------------------------------------------- install

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

#[cfg(windows)]
fn install(a: &[String]) -> i32 {
    use std::os::windows::process::CommandExt;
    if let Err(e) = WatchOpts::parse(a) {
        return fail(&e);
    }
    let dir = state_dir();
    let _ = fs::create_dir_all(&dir);
    let dest = dir.join("wslwatch.exe");
    let me = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return fail(&format!("current_exe: {e}")),
    };
    stop_others();
    let same = fs::canonicalize(&me).ok() == fs::canonicalize(&dest).ok();
    if !same {
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
    let _ = std::io::stdout().flush();
    // CreateProcess here inherits every inheritable handle, and the std handles
    // WSL interop gave us are inheritable pipes: a watcher holding them keeps
    // the caller's `wslwatch.exe install` from ever seeing EOF (it hung until
    // `timeout` killed it). Unmark them first.
    stop_inheriting_stdio();
    const DETACHED: u32 = 0x0000_0008 | 0x0000_0200; // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    const BREAKAWAY: u32 = 0x0100_0000; // CREATE_BREAKAWAY_FROM_JOB
    let mk = |flags: u32| {
        let mut c = Command::new(&dest);
        c.arg("watch").args(a).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(flags);
        c
    };
    match mk(DETACHED | BREAKAWAY).spawn().or_else(|_| mk(DETACHED).spawn()) {
        Ok(c) => {
            println!("started pid {}", c.id());
            0
        }
        Err(e) => fail(&format!("installed, but could not start it: {e}")),
    }
}

#[cfg(windows)]
fn stop_inheriting_stdio() {
    extern "system" {
        fn GetStdHandle(which: u32) -> isize;
        fn SetHandleInformation(handle: isize, mask: u32, flags: u32) -> i32;
    }
    const HANDLE_FLAG_INHERIT: u32 = 1;
    // STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE = (DWORD)-10, -11, -12
    for which in [-10i32, -11, -12] {
        // SAFETY: plain kernel32 calls on our own std handles; a null or
        // INVALID_HANDLE_VALUE result is skipped.
        unsafe {
            let h = GetStdHandle(which as u32);
            if h != 0 && h != -1 {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
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
        // quiet ends at 900; the failures began at 360, so the next one restarts
        assert_eq!(h.observe(false, t + s(900)), Verdict::Restart);
    }

    #[test]
    fn utc_formats() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_792_680_177), "2026-10-22T14:42:57Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn probe_writes_and_reads_back() {
        let p = std::env::temp_dir().join(format!("wslwatch-test-{}", std::process::id())).join("probe");
        assert_eq!(probe(&["--file".into(), p.display().to_string()]), 0);
        assert!(fs::read_to_string(&p).unwrap().ends_with(&format!(" {}\n", std::process::id())));
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn a_hung_command_is_killed_at_the_timeout() {
        let t0 = Instant::now();
        let o = run(&["sleep".into(), "30".into()], s(1));
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
        let o = WatchOpts::parse(&["--interval".into(), "5".into(), "--dry-run".into(), "--".into(), "sleep".into(), "9".into()]).unwrap();
        assert_eq!((o.interval, o.dry_run, o.max_restarts), (5, true, None));
        assert_eq!(o.probe_cmd(), vec!["sleep".to_string(), "9".to_string()]);
        assert!(WatchOpts::parse(&["--bogus".into(), "1".into()]).is_err());
        assert_eq!(WatchOpts::parse(&["--max-restarts".into(), "1".into()]).unwrap().max_restarts, Some(1));
    }

    #[test]
    fn max_restarts_ends_a_test_instance() {
        let dir = std::env::temp_dir().join(format!("wslwatch-max-{}", std::process::id()));
        let a = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut args = a(&["--dry-run", "--interval", "1", "--probe-timeout", "1", "--hang-after", "1", "--max-restarts", "1"]);
        args.extend(a(&["--log", &dir.join("w.log").display().to_string(), "--lock", &dir.join("w.lock").display().to_string()]));
        args.extend(a(&["--", "sleep", "5"]));
        let t0 = Instant::now();
        assert_eq!(watch(WatchOpts::parse(&args).unwrap()), 0);
        assert!(t0.elapsed() < s(10), "took {:?}", t0.elapsed());
        let log = fs::read_to_string(dir.join("w.log")).unwrap();
        assert!(log.contains("DRY RUN, would run wsl.exe --shutdown"), "{log}");
        assert!(log.contains("exiting after 1 restart(s)"), "{log}");
        let _ = fs::remove_dir_all(&dir);
    }
}
