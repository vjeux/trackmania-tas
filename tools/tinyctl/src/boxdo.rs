//! `tinyctl box-do --label L -- CMD…` — run a command that touches the render box's SCREEN
//! (a Chrome launch, a window move, a CDP click, a game helper) with its run REGISTERED in
//! the box run log first, so the human-idle gate attributes the input it causes to this
//! run and not to a person.
//!
//! Why a wrapper and not a convention: on 2026-11-07 (box-close-game) and 11-09 (a hand-
//! written "manual.N" row for a Chrome launch) registration was a step the lane had to
//! remember; vjeux: "please fix the bug where your own inputs are treated as me using it".
//! Registration here happens before the child starts and carries the gate's CURRENT idle
//! reading (never 0: an idle-0 row invents a person — 2026-11-07). The row's heartbeat runs
//! for the child's lifetime; the wrapper exits with the child's status.
//!
//!   tinyctl box-do --label "tmx login" -- whitestick "…"
//!   tinyctl tmx-login            — renew TMX's session through the chrome-tmx profile
//!                                  (nav /login → /mapupload must show the form), registered

use std::process::Command;

/// The gate's current keyboard/mouse idle, read once; 0 only when the reading itself fails
/// (the attribution then falls back to "input right at the start", the conservative side).
fn idle_now(tinyctl: &str) -> f64 {
    let out = Command::new(tinyctl).args(["box-idle", "--window-s", "60"]).output().ok();
    out.map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .and_then(|s| s.split_whitespace().find_map(|w| w.strip_prefix("kbm_idle_s=").and_then(|v| v.parse::<f64>().ok())))
        .unwrap_or(0.0)
}

pub fn cmd(args: &[String]) -> Result<i32, String> {
    let label = tmmaps::cli::flag(args, "--label").map(String::from).unwrap_or_else(|| "box-do".into());
    let sep = args.iter().position(|a| a == "--").ok_or("box-do needs `-- CMD…`")?;
    let child_args: Vec<&String> = args[sep + 1..].iter().collect();
    if child_args.is_empty() {
        return Err("box-do: nothing to run after --".into());
    }
    let me = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "tinyctl".into());
    crate::inputidle::runlog::start(&format!("box-do {label}"), idle_now(&me));
    let st = Command::new(child_args[0]).args(&child_args[1..]).status().map_err(|e| format!("{}: {e}", child_args[0]))?;
    Ok(st.code().unwrap_or(1))
}

/// `tinyctl tmx-login [--tmxup WINPATH]`: TMX's own session lasts about a week; the Ubisoft
/// OAuth session behind it lasts far longer. Opening /login in the chrome-tmx profile makes
/// TMX re-issue its session silently (proven 2026-11-09 19:25: /login → ?token=… → the
/// "request_refresh" page; /mapupload then serves the form). Registered: Chrome's window
/// comes to the foreground on navigation.
pub fn tmx_login(args: &[String]) -> Result<(), String> {
    let tmxup = tmmaps::cli::flag(args, "--tmxup").map(String::from).unwrap_or_else(|| "/mnt/c/Users/vjeux/tmxup.exe".into());
    let whitestick = std::env::var("HOME").map(|h| format!("{h}/bin/whitestick")).unwrap_or_else(|_| "whitestick".into());
    let me = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "tinyctl".into());
    crate::inputidle::runlog::start("tmx-login (chrome-tmx /login drive)", idle_now(&me));
    let sh = |s: &str| -> Result<String, String> {
        let o = Command::new(&whitestick).arg(s).output().map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&o.stdout).to_string())
    };
    // Chrome up with the debugging port? Else launch the profile, off-screen.
    let up = sh("curl -s -m 5 http://127.0.0.1:9222/json | head -c 10")?;
    if up.trim().is_empty() {
        let _ = sh("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -NoProfile -Command \"Start-Process chrome.exe -ArgumentList '--user-data-dir=C:\\\\Users\\\\vjeux\\\\chrome-tmx','--remote-debugging-port=9222','--window-position=2000,2000','https://trackmania.exchange/'\"; sleep 10")?;
    }
    let _ = sh(&format!("{tmxup} nav --url https://trackmania.exchange/login --wait-s 10 2>&1 | tail -1"))?;
    std::thread::sleep(std::time::Duration::from_secs(3));
    let _ = sh(&format!("{tmxup} nav --url https://trackmania.exchange/mapupload --wait-s 8 2>&1 | tail -1"))?;
    let check = sh(&format!("{tmxup} eval --expr 'JSON.stringify({{form:!!document.querySelector(\"#mapUploadForm\"), title:document.title}})' 2>&1 | tail -1"))?;
    if check.contains("\\\"form\\\":true") || check.contains("\"form\":true") {
        println!("tmx-login: session renewed, /mapupload serves the form");
        Ok(())
    } else {
        Err(format!("tmx-login: /mapupload has no upload form after /login — Ubisoft may be asking for a password or 2FA (only then does a person need the box): {}", check.trim()))
    }
}

#[cfg(test)]
mod tests {
    /// An unregistered box action is the bug: with no run row covering the instant of an
    /// input, the gate books it as a person's. `box-do` registers before the child runs, so a
    /// row covers the child's whole lifetime (plus the 120 s grace the gate already gives).
    #[test]
    fn box_do_registers_before_the_child_runs() {
        let dir = std::env::temp_dir().join(format!("tinyctl-boxdo-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("HOME", &dir);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
        // before: nothing covers "now" — the bug state
        assert!(crate::inputidle::runlog::covering(now).is_none(), "fresh log must not cover the instant");
        // `box-do -- true`: registers, then runs a no-op child
        let args: Vec<String> = ["--label", "unit test", "--", "true"].iter().map(|s| s.to_string()).collect();
        let code = super::cmd(&args).expect("box-do runs");
        assert_eq!(code, 0);
        let row = crate::inputidle::runlog::covering(now + 0.5).expect("the run row covers the child's run");
        assert!(row.3.contains("box-do unit test"), "row names the run: {}", row.3);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
