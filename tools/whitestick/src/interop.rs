//! The WSL interop guard: keeps Windows executables launchable from inside WSL.
//!
//! On the WhiteStick box (WSL2 Ubuntu with systemd), every new `wsl.exe`
//! session races systemd-binfmt over its drop-in and the binfmt_misc entry
//! `WSLInterop` ends up unregistered (measured 2026-09-29). From then on no
//! `.exe` can be exec'd from WSL: `cmd.exe`, `wsl.exe`, `wslpath`, the game
//! launchers -- everything the agent's commands lean on -- fail with
//! "Exec format error". The interop SOCKET keeps working, only the binfmt
//! hook is gone, so the fix can be applied from inside WSL without binfmt:
//! `/init` is the interpreter binfmt would have used, and invoked directly as
//! `/init <exe> <argv0> <args...>` it launches a Windows program the same way.
//! That opens a root shell in the same distro (`wsl.exe -u root`), which
//! writes the registration line back into `/proc/sys/fs/binfmt_misc/register`.
//!
//! This module is that fix as a background task of `whitestick agent`: every
//! [`EVERY`] it looks for the entry and re-registers it when missing, with a
//! [`FIX_TIMEOUT`] around the interop call (an interop call that hangs must
//! never hang the agent), a counter in the log, and nothing else. It replaced
//! the `~/bin/whitestick-agent-loop.sh` guard subshell + `fix-interop.sh`.
//!
//! The decision logic (are we under WSL? does the config want the guard? is the
//! entry there? what exactly do we exec?) is pure and unit-tested; only the
//! spawn and the sleep touch the world.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

/// How often the entry is checked.
pub const EVERY: Duration = Duration::from_secs(20);
/// Ceiling on one re-registration attempt (the interop call included).
pub const FIX_TIMEOUT: Duration = Duration::from_secs(30);

pub const BINFMT_DIR: &str = "/proc/sys/fs/binfmt_misc";
pub const ENTRY: &str = "WSLInterop";
/// What WSL's own drop-in (`/usr/lib/binfmt.d/WSLInterop.conf`) registers:
/// magic `MZ`, interpreter `/init`, P = preserve argv[0], F = open the
/// interpreter at registration time.
pub const REGISTER_LINE: &str = ":WSLInterop:M::MZ::/init:PF";

const INIT: &str = "/init";
const WSL_EXE: &str = "/mnt/c/Windows/System32/wsl.exe";
const DEFAULT_DISTRO: &str = "Ubuntu";

/// Directories the box-side commands expect on PATH under WSL. A session
/// started by the Startup-folder `.vbs` (`wsl.exe ... /bin/sh -lc`) gets them
/// from WSL's own PATH injection, but not every way of starting the agent does,
/// and without them `cmd.exe`, `wslpath` and `powershell.exe` are not found.
pub const WINDOWS_PATH_DIRS: &[&str] = &[
    "/mnt/c/Windows/System32",
    "/mnt/c/Windows",
    "/mnt/c/Windows/System32/WindowsPowerShell/v1.0",
];

/// Is this process running inside WSL? Either the kernel says so (WSL1 says
/// "Microsoft", WSL2 "microsoft-standard-WSL2") or the interop socket is in
/// the environment.
pub fn is_wsl(proc_version: &str, wsl_interop_env: Option<&str>) -> bool {
    proc_version.to_ascii_lowercase().contains("microsoft")
        || wsl_interop_env.is_some_and(|v| !v.trim().is_empty())
}

pub fn running_under_wsl() -> bool {
    let version = std::fs::read_to_string("/proc/version").unwrap_or_default();
    let interop = std::env::var("WSL_INTEROP").ok();
    is_wsl(&version, interop.as_deref())
}

/// Whether the guard runs, and if not, why -- for the one startup log line.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Run,
    Off(&'static str),
}

/// `flag` is `[agent] interop_guard` from the config (unset = default).
/// The default is on under WSL; off anywhere else, because `/init` and
/// binfmt_misc's `WSLInterop` only exist there -- and an explicit `true`
/// cannot override that.
pub fn decide(flag: Option<bool>, on_wsl: bool) -> Decision {
    match (flag, on_wsl) {
        (Some(false), _) => Decision::Off("disabled by config ([agent] interop_guard = false)"),
        (_, false) => Decision::Off("not running under WSL"),
        (_, true) => Decision::Run,
    }
}

/// Is the entry registered? binfmt_misc exposes one file per registered
/// name; unregistering removes the file.
pub fn registered(binfmt_dir: &Path) -> bool {
    binfmt_dir.join(ENTRY).exists()
}

/// The shell script the root session runs. Re-registers only when the entry
/// is (still) missing -- a second registration of an existing name fails with
/// EEXIST -- and prints the entry back so the log shows what the kernel has.
pub fn fix_script() -> String {
    format!(
        "if [ ! -e {dir}/{entry} ]; then echo '{line}' > {dir}/register; fi; cat {dir}/{entry}",
        dir = BINFMT_DIR,
        entry = ENTRY,
        line = REGISTER_LINE,
    )
}

/// The argv that applies the fix without binfmt: `/init` as the interop
/// launcher, the exe path, then -- explicitly, /init does not add it -- the
/// argv[0] the Windows program sees, then wsl.exe's own arguments.
pub fn fix_argv(distro: &str) -> Vec<String> {
    vec![
        INIT.to_string(),
        WSL_EXE.to_string(),
        "wsl.exe".to_string(),
        "-d".to_string(),
        distro.to_string(),
        "-u".to_string(),
        "root".to_string(),
        "--".to_string(),
        "/bin/sh".to_string(),
        "-c".to_string(),
        fix_script(),
    ]
}

/// The distro to open the root session in: this one.
pub fn distro_name(wsl_distro_env: Option<&str>) -> String {
    match wsl_distro_env.map(str::trim) {
        Some(d) if !d.is_empty() => d.to_string(),
        _ => DEFAULT_DISTRO.to_string(),
    }
}

/// PATH for the agent's child commands: `path` with every missing
/// [`WINDOWS_PATH_DIRS`] entry appended, or `None` when nothing is missing.
/// Comparison is on whole components, so `/mnt/c/Windows` does not count as
/// `/mnt/c/Windows/System32`.
pub fn path_with_windows_dirs(path: Option<&str>) -> Option<String> {
    let current = path.unwrap_or("");
    let have: Vec<&str> = current
        .split(':')
        .filter(|c| !c.is_empty())
        .map(|c| c.trim_end_matches('/'))
        .collect();
    let missing: Vec<&str> = WINDOWS_PATH_DIRS
        .iter()
        .copied()
        .filter(|d| !have.contains(d))
        .collect();
    if missing.is_empty() {
        return None;
    }
    let mut out = current.trim_end_matches(':').to_string();
    for d in missing {
        if !out.is_empty() {
            out.push(':');
        }
        out.push_str(d);
    }
    Some(out)
}

/// What one attempt did.
#[derive(Debug)]
pub enum FixOutcome {
    /// The command ran; `output` is its stdout+stderr, trimmed.
    Ran { status: String, output: String },
    /// Killed at the deadline.
    TimedOut,
    /// Could not even start (no `/init`? not WSL?).
    SpawnFailed(std::io::Error),
}

/// Run `argv` with a deadline, killing it (and its process group) at the
/// deadline. Split out from [`reregister`] so the timeout path has a test
/// that needs no WSL.
pub async fn run_with_deadline(argv: &[String], deadline: Duration) -> FixOutcome {
    let mut cmd = tokio::process::Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return FixOutcome::SpawnFailed(e),
    };
    let pid = child.id().unwrap_or(0) as i32;
    // Readers of their own, so a chatty command cannot block on a full pipe
    // and a hung one can be killed while its pipes are still open.
    let out_task = tokio::spawn(slurp(child.stdout.take()));
    let err_task = tokio::spawn(slurp(child.stderr.take()));
    match tokio::time::timeout(deadline, child.wait()).await {
        Ok(Ok(status)) => {
            // The pipes close when the last writer exits; a helper that
            // outlives the command must not hold the outcome hostage.
            let grace = Duration::from_secs(2);
            let stdout = tokio::time::timeout(grace, out_task)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default();
            let stderr = tokio::time::timeout(grace, err_task)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default();
            let mut text = stdout.trim().to_string();
            let err = stderr.trim();
            if !err.is_empty() {
                if !text.is_empty() {
                    text.push_str(" | ");
                }
                text.push_str("stderr: ");
                text.push_str(err);
            }
            FixOutcome::Ran {
                status: status.to_string(),
                output: text.split_whitespace().collect::<Vec<_>>().join(" "),
            }
        }
        Ok(Err(e)) => FixOutcome::SpawnFailed(e),
        Err(_elapsed) => {
            // The whole group: wsl.exe's helpers sit under /init.
            if pid > 0 {
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
            }
            let _ = child.kill().await;
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
            out_task.abort();
            err_task.abort();
            FixOutcome::TimedOut
        }
    }
}

async fn slurp(pipe: Option<impl tokio::io::AsyncRead + Unpin>) -> String {
    use tokio::io::AsyncReadExt;
    let Some(mut pipe) = pipe else {
        return String::new();
    };
    let mut buf = Vec::new();
    let _ = pipe.read_to_end(&mut buf).await;
    String::from_utf8_lossy(&buf).into_owned()
}

/// One re-registration through the interop launcher.
pub async fn reregister(distro: &str) -> FixOutcome {
    run_with_deadline(&fix_argv(distro), FIX_TIMEOUT).await
}

pub struct Guard {
    pub binfmt_dir: PathBuf,
    pub distro: String,
    pub every: Duration,
}

impl Guard {
    pub fn from_env() -> Self {
        let distro_env = std::env::var("WSL_DISTRO_NAME").ok();
        Guard {
            binfmt_dir: PathBuf::from(BINFMT_DIR),
            distro: distro_name(distro_env.as_deref()),
            every: EVERY,
        }
    }

    /// Check forever. Never returns; run it as its own task.
    pub async fn run(self) {
        let entry = self.binfmt_dir.join(ENTRY);
        crate::log(&format!(
            "interop guard: watching {} every {} s ({} now); missing -> re-register via {} {} -d {} -u root ({} s timeout)",
            entry.display(),
            self.every.as_secs(),
            if registered(&self.binfmt_dir) { "present" } else { "MISSING" },
            INIT,
            WSL_EXE,
            self.distro,
            FIX_TIMEOUT.as_secs()
        ));
        let mut fixes: u64 = 0;
        let mut failures: u64 = 0;
        loop {
            if !registered(&self.binfmt_dir) {
                fixes += 1;
                crate::log(&format!(
                    "interop guard: {ENTRY} missing (#{fixes}); re-registering"
                ));
                let started = Instant::now();
                let outcome = reregister(&self.distro).await;
                let took = started.elapsed().as_secs_f64();
                let fixed = registered(&self.binfmt_dir);
                if !fixed {
                    failures += 1;
                }
                match outcome {
                    FixOutcome::Ran { status, output } => crate::log(&format!(
                        "interop guard: fix #{fixes} {} in {took:.3} s ({status}; {output}); failures so far: {failures}",
                        if fixed { "re-registered" } else { "ran but the entry is still missing" },
                    )),
                    FixOutcome::TimedOut => crate::log(&format!(
                        "interop guard: fix #{fixes} timed out after {} s and was killed; entry {}; failures so far: {failures}",
                        FIX_TIMEOUT.as_secs(),
                        if fixed { "present anyway" } else { "still missing" }
                    )),
                    FixOutcome::SpawnFailed(e) => crate::log(&format!(
                        "interop guard: fix #{fixes} could not run {INIT}: {e}; entry {}; failures so far: {failures}",
                        if fixed { "present anyway" } else { "still missing" }
                    )),
                }
            }
            tokio::time::sleep(self.every).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WSL2: &str = "Linux version 6.18.33.2-microsoft-standard-WSL2 (root@f1bbfb02316b) (gcc (GCC) 13.2.0, GNU ld (GNU Binutils) 2.41) #1 SMP PREEMPT_DYNAMIC Thu Jun 18 21:54:43 UTC 2026";
    const WSL1: &str = "Linux version 4.4.0-19041-Microsoft (Microsoft@Microsoft.com) (gcc version 5.4.0 (GCC) ) #1237-Microsoft Sat Sep 11 14:32:00 PST 2021";
    const CENTOS: &str = "Linux version 5.19.0-0_fbk12_zion_11583_g0bef9520ca2b (kernel@build) (gcc (GCC) 11.x) #1 SMP";

    #[test]
    fn detects_wsl_from_proc_version() {
        assert!(is_wsl(WSL2, None));
        assert!(is_wsl(WSL1, None), "WSL1 capitalises Microsoft");
        assert!(!is_wsl(CENTOS, None));
        assert!(!is_wsl("", None));
    }

    #[test]
    fn detects_wsl_from_interop_socket_env() {
        assert!(is_wsl(CENTOS, Some("/run/WSL/394_interop")));
        assert!(
            !is_wsl(CENTOS, Some("")),
            "an empty WSL_INTEROP is not a socket"
        );
        assert!(!is_wsl(CENTOS, Some("   ")));
    }

    #[test]
    fn guard_defaults_on_under_wsl_and_off_elsewhere() {
        assert_eq!(decide(None, true), Decision::Run);
        assert_eq!(decide(None, false), Decision::Off("not running under WSL"));
    }

    #[test]
    fn config_false_turns_it_off_even_under_wsl() {
        assert_eq!(
            decide(Some(false), true),
            Decision::Off("disabled by config ([agent] interop_guard = false)")
        );
        assert_eq!(
            decide(Some(false), false),
            Decision::Off("disabled by config ([agent] interop_guard = false)")
        );
    }

    #[test]
    fn config_true_cannot_force_it_outside_wsl() {
        assert_eq!(decide(Some(true), true), Decision::Run);
        assert_eq!(
            decide(Some(true), false),
            Decision::Off("not running under WSL")
        );
    }

    #[test]
    fn registered_means_the_entry_file_exists() {
        let dir = std::env::temp_dir().join(format!(
            "whitestick-interop-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!registered(&dir), "empty binfmt dir: nothing registered");
        std::fs::write(dir.join("register"), b"").unwrap();
        std::fs::write(dir.join("status"), b"enabled\n").unwrap();
        assert!(!registered(&dir), "register/status are not the entry");
        std::fs::write(
            dir.join(ENTRY),
            b"enabled\ninterpreter /init\nflags: PF\noffset 0\nmagic 4d5a\n",
        )
        .unwrap();
        assert!(registered(&dir));
        std::fs::remove_file(dir.join(ENTRY)).unwrap();
        assert!(!registered(&dir), "unregistering removes the file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fix_argv_goes_through_init_with_explicit_argv0() {
        let argv = fix_argv("Ubuntu");
        assert_eq!(argv[0], "/init", "the interop launcher, not binfmt");
        assert_eq!(argv[1], "/mnt/c/Windows/System32/wsl.exe");
        assert_eq!(
            argv[2], "wsl.exe",
            "argv[0] for the Windows side must be passed explicitly"
        );
        assert_eq!(&argv[3..7], &["-d", "Ubuntu", "-u", "root"]);
        assert_eq!(&argv[7..10], &["--", "/bin/sh", "-c"]);
        assert_eq!(argv.len(), 11);
        let script = &argv[10];
        assert!(script
            .contains("echo ':WSLInterop:M::MZ::/init:PF' > /proc/sys/fs/binfmt_misc/register"));
        assert!(
            script.starts_with("if [ ! -e /proc/sys/fs/binfmt_misc/WSLInterop ]; then"),
            "registering an existing name is EEXIST, so the root shell checks first: {script}"
        );
        assert!(script.ends_with("cat /proc/sys/fs/binfmt_misc/WSLInterop"));
    }

    #[test]
    fn fix_argv_uses_the_given_distro() {
        let argv = fix_argv("Ubuntu-24.04");
        assert_eq!(&argv[3..5], &["-d", "Ubuntu-24.04"]);
    }

    #[test]
    fn distro_comes_from_the_environment_with_a_fallback() {
        assert_eq!(distro_name(Some("Ubuntu-22.04")), "Ubuntu-22.04");
        assert_eq!(distro_name(Some(" Debian ")), "Debian");
        assert_eq!(distro_name(Some("")), "Ubuntu");
        assert_eq!(distro_name(None), "Ubuntu");
    }

    #[test]
    fn path_gains_the_windows_dirs_it_lacks() {
        assert_eq!(
            path_with_windows_dirs(Some("/home/vjeux/bin:/usr/bin")),
            Some(
                "/home/vjeux/bin:/usr/bin:/mnt/c/Windows/System32:/mnt/c/Windows:/mnt/c/Windows/System32/WindowsPowerShell/v1.0"
                    .to_string()
            )
        );
        assert_eq!(
            path_with_windows_dirs(None),
            Some("/mnt/c/Windows/System32:/mnt/c/Windows:/mnt/c/Windows/System32/WindowsPowerShell/v1.0".to_string())
        );
        assert_eq!(
            path_with_windows_dirs(Some("")),
            Some("/mnt/c/Windows/System32:/mnt/c/Windows:/mnt/c/Windows/System32/WindowsPowerShell/v1.0".to_string())
        );
    }

    #[test]
    fn path_is_left_alone_when_it_already_has_them() {
        let full = "/home/vjeux/bin:/usr/bin:/mnt/c/Windows/System32:/mnt/c/Windows:/mnt/c/Windows/System32/WindowsPowerShell/v1.0";
        assert_eq!(path_with_windows_dirs(Some(full)), None);
        // Order and trailing slashes do not matter for "present".
        let shuffled = "/mnt/c/Windows/System32/WindowsPowerShell/v1.0:/mnt/c/Windows/:/usr/bin:/mnt/c/Windows/System32/";
        assert_eq!(path_with_windows_dirs(Some(shuffled)), None);
    }

    #[test]
    fn path_only_adds_what_is_missing_and_matches_whole_components() {
        // /mnt/c/Windows is there; System32 and PowerShell are not.
        assert_eq!(
            path_with_windows_dirs(Some("/usr/bin:/mnt/c/Windows:/usr/local/bin")),
            Some("/usr/bin:/mnt/c/Windows:/usr/local/bin:/mnt/c/Windows/System32:/mnt/c/Windows/System32/WindowsPowerShell/v1.0".to_string())
        );
        // A component that merely CONTAINS the dir is not the dir.
        assert_eq!(
            path_with_windows_dirs(Some("/opt/mnt/c/Windows/System32:/mnt/c/Windows:/mnt/c/Windows/System32/WindowsPowerShell/v1.0")),
            Some("/opt/mnt/c/Windows/System32:/mnt/c/Windows:/mnt/c/Windows/System32/WindowsPowerShell/v1.0:/mnt/c/Windows/System32".to_string())
        );
    }

    fn sh(script: &str) -> Vec<String> {
        vec!["/bin/sh".to_string(), "-c".to_string(), script.to_string()]
    }

    #[tokio::test]
    async fn deadline_runs_a_quick_command_to_completion() {
        match run_with_deadline(
            &sh("echo enabled; echo oops >&2; exit 3"),
            Duration::from_secs(10),
        )
        .await
        {
            FixOutcome::Ran { status, output } => {
                assert!(status.contains('3'), "exit status is reported: {status}");
                assert_eq!(output, "enabled | stderr: oops");
            }
            other => panic!("expected Ran, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn deadline_kills_a_hung_command() {
        let started = Instant::now();
        let outcome = run_with_deadline(&sh("sleep 30"), Duration::from_millis(300)).await;
        assert!(matches!(outcome, FixOutcome::TimedOut), "got {outcome:?}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the deadline must not wait for the command: {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn deadline_reports_a_command_that_cannot_start() {
        let argv = vec!["/nonexistent/whitestick-init".to_string()];
        assert!(matches!(
            run_with_deadline(&argv, Duration::from_secs(1)).await,
            FixOutcome::SpawnFailed(_)
        ));
    }
}
