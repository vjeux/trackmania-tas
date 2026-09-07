//! `tinyctl box-build` — build the toolchain ON the render box (WSL has cargo),
//! instead of pushing 4 MB binaries across a 1.4 MB/s bridge after every fix.
//!
//! Two paths:
//! - the box already has `tinyctl`: `tinyctl selfbuild --detach` runs there
//!   (a program: `git pull --ff-only`, then `cargo build --release` for the
//!   named crates, done file at the end) and this side polls;
//! - bootstrap (no `tinyctl` on the box yet): the same three steps as one
//!   detached `sh -c` line, once. After that the program path is used.
//!
//! `tinyctl selfbuild` is the box-side half; it also works on the devserver
//! for the same reason (the checkout there is a git clone too).

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::wsx::Wsx;

const BOX_REPO: &str = "/home/vjeux/trackmania-tas";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";
const BOX_CARGO: &str = "/home/vjeux/.cargo/bin/cargo";
const DEFAULT_CRATES: &str = "shootctl,tinyctl,mapgeom,tmmaps";
const LOG_DIR: &str = "/home/vjeux/shoot";

pub fn box_build_cmd(args: &[String]) -> Result<(), String> {
    let crates = tmmaps::cli::flag(args, "--crates").unwrap_or(DEFAULT_CRATES);
    let wsx = Wsx::new(args);
    let done = format!("{LOG_DIR}/box-build-done.txt");
    let log = format!("{LOG_DIR}/box-build.log");
    let have = wsx.sh(&format!("test -x {BOX_TOOLS}/tinyctl")).is_ok();
    let _ = wsx.sh(&format!("rm -f {done}"));
    if have && !tmmaps::cli::has(args, "--bootstrap") {
        eprintln!("box has tinyctl: running selfbuild there …");
        let out = wsx.sh(&format!("{BOX_TOOLS}/tinyctl selfbuild --detach --repo {BOX_REPO} --cargo {BOX_CARGO} --crates {crates} --done {done} --log {log}"))?;
        if wsx.verbose {
            eprintln!("{}", out.trim());
        }
    } else {
        eprintln!("bootstrapping: no tinyctl on the box yet (or --bootstrap); one detached shell line, once …");
        let ps: String = crates.split(',').map(|c| format!("-p {c}")).collect::<Vec<_>>().join(" ");
        let line = format!("cd {BOX_REPO} && git pull --ff-only && {BOX_CARGO} build --release {ps}; echo \"BUILD rc=$?\"");
        let cmd = format!("nohup sh -c '{line}; if grep -q \"BUILD rc=0\" {log}; then echo OK > {done}; else echo FAILED > {done}; fi' > {log} 2>&1 < /dev/null &");
        wsx.sh(&cmd)?;
    }
    let res = wsx.wait_done(&done, &log, Duration::from_secs(2400), "box build")?;
    println!("{}", res.trim());
    let tail = wsx.cat(&log).unwrap_or_default();
    for l in tail.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev() {
        println!("  {l}");
    }
    let vers = wsx.sh(&format!("{BOX_TOOLS}/tinyctl --version; {BOX_TOOLS}/shootctl --version")).unwrap_or_default();
    println!("{}", vers.trim());
    Ok(())
}

/// The box-side half (also usable on any git checkout): pull, build, done file.
pub fn selfbuild_cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let repo = f("--repo").unwrap_or_else(|| BOX_REPO.into());
    let cargo = f("--cargo").unwrap_or_else(|| BOX_CARGO.into());
    let crates = f("--crates").unwrap_or_else(|| DEFAULT_CRATES.into());
    let done = f("--done").unwrap_or_else(|| format!("{LOG_DIR}/box-build-done.txt"));
    let log = f("--log").unwrap_or_else(|| format!("{LOG_DIR}/box-build.log"));
    let _ = std::fs::remove_file(&done);
    if tmmaps::cli::has(args, "--detach") {
        use std::os::unix::process::CommandExt;
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--detach").collect();
        if let Some(d) = Path::new(&log).parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let out = std::fs::File::create(&log).map_err(|e| format!("{log}: {e}"))?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let child = Command::new(exe).args(&args).stdin(std::process::Stdio::null()).stdout(out).stderr(err).process_group(0).spawn().map_err(|e| format!("spawn: {e}"))?;
        println!("detached pid {} — log {log} — done file {done}", child.id());
        return Ok(());
    }
    let t0 = std::time::Instant::now();
    let result = (|| -> Result<String, String> {
        let pull = Command::new("git").args(["pull", "--ff-only"]).current_dir(&repo).output().map_err(|e| format!("git: {e}"))?;
        println!("git pull: {}{}", String::from_utf8_lossy(&pull.stdout).trim(), String::from_utf8_lossy(&pull.stderr).trim());
        if !pull.status.success() {
            return Err(format!("git pull failed ({})", pull.status));
        }
        let head = Command::new("git").args(["rev-parse", "--short", "HEAD"]).current_dir(&repo).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let mut c = Command::new(&cargo);
        c.arg("build").arg("--release").current_dir(format!("{repo}/tools"));
        for k in crates.split(',') {
            c.arg("-p").arg(k);
        }
        let out = c.output().map_err(|e| format!("{cargo}: {e}"))?;
        print!("{}", String::from_utf8_lossy(&out.stderr));
        if !out.status.success() {
            return Err(format!("cargo build failed ({})", out.status));
        }
        Ok(format!("built {crates} at {head} in {:.0}s", t0.elapsed().as_secs_f64()))
    })();
    let summary = match &result {
        Ok(s) => format!("OK {s}\n"),
        Err(e) => format!("FAILED {e}\n"),
    };
    print!("{summary}");
    std::fs::write(&done, &summary).map_err(|e| format!("{done}: {e}"))?;
    result.map(|_| ())
}
