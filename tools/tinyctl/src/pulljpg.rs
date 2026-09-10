//! `tinyctl pulljpg --tag T [--outdir D] [--quality 2] [--wsx P]` — the frames
//! of a shoot as JPEGs, in ONE bridge transfer.
//!
//! A 4K capture is ~10 MB as PNG and the bridge moves ~1.4 MB/s, so pulling a
//! 78-view lineup as PNGs is nine minutes; as JPEGs (q 2, ~1 MB each) in one
//! tar it is under a minute. The conversion runs ON THE BOX with its ffmpeg,
//! detached (a bridge command is capped at 90 s), the done file is polled
//! (one call per 30 s), the tar is pulled once and unpacked into `--outdir`
//! (`cmp-<tag><view>-<side>.jpg`).

use std::path::PathBuf;
use std::time::Duration;

use crate::wsx::Wsx;

const SHOTS: &str = "/mnt/c/Users/vjeux/tinyshots";
const BOX_FFMPEG: &str = "/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin/ffmpeg.exe";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let tag = f("--tag").ok_or("pulljpg needs --tag T")?;
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| format!("/tmp/lin/shots{tag}")));
    let quality = f("--quality").unwrap_or_else(|| "2".into());
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let wsx = Wsx::new(args);
    let dir = format!("{SHOTS}/{tag}");
    let done = format!("{dir}/jpg-done.txt");
    let log = format!("{dir}/jpg.log");
    let tar = format!("{dir}/{tag}-jpg.tar");
    let job = format!("cd {dir} && for f in cmp-{tag}*.png; do \"{BOX_FFMPEG}\" -v error -y -i \"$f\" -q:v {quality} \"${{f%.png}}.jpg\"; done; tar cf {tag}-jpg.tar cmp-{tag}*.jpg && echo OK $(ls cmp-{tag}*.jpg | wc -l) > {done} || echo FAILED > {done}");
    wsx.sh(&format!("rm -f {done} {tar}; nohup setsid sh -c '{job}' > {log} 2>&1 < /dev/null &"))?;
    let text = wsx.wait_done(&done, &log, Duration::from_secs(1200), "jpeg conversion")?;
    if !text.starts_with("OK") {
        return Err(format!("the box's conversion failed: {}", text.trim()));
    }
    let local_tar = outdir.join(format!("{tag}-jpg.tar"));
    let n = wsx.pull(&tar, &local_tar)?;
    let st = std::process::Command::new("tar").arg("xf").arg(&local_tar).current_dir(&outdir).status().map_err(|e| format!("tar: {e}"))?;
    if !st.success() {
        return Err(format!("tar xf {} failed", local_tar.display()));
    }
    let count = std::fs::read_dir(&outdir).map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".jpg")).count()).unwrap_or(0);
    println!("{}: {count} JPEG frames of {tag} ({n} B in one transfer; {})", outdir.display(), text.trim());
    Ok(())
}
