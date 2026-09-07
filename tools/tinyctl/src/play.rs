//! `tinyctl play` — a map in PLAY mode on the render box, N timed frames from
//! the moment the playground opens, pulled back as ONE contact sheet.
//!
//! ```text
//! tinyctl play --map MAP --tag T [--shots 4] [--every-ms 200] [--first-ms 300]
//!              [--timeout 600] [--outdir /tmp/tiny3] [--wsx P] [-v]
//! ```
//!
//! The MediaTracker intro plays when a map opens in play (a 10 s camera
//! flight over the start on the Summer maps), so the frames of the first ten
//! seconds ARE the intro: shot k of the original and shot k of the tiny build
//! are the same moment of the same clip (a screenshot costs ~2.7 s, so four
//! shots at `--every-ms 200` sample t ≈ 0.3, 3.2, 6.1, 9.0 s). The frames stay
//! on the box (4K PNGs, ~10 MB each; the bridge moves ~1.4 MB/s); ffmpeg
//! there scales them to 960 wide and stacks them into
//! `play-<T>-sheet.jpg`, which is what crosses.

use std::path::PathBuf;
use std::time::Duration;

use crate::wsx::{to_win, Wsx};

const STAGE: &str = "/home/vjeux/shoot/_stage";
const SHOTS: &str = "/mnt/c/Users/vjeux/tinyshots";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";
const BOX_FFMPEG: &str = "/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin/ffmpeg.exe";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let map = PathBuf::from(f("--map").ok_or("play needs --map MAP.Map.Gbx")?);
    let tag = f("--tag").ok_or("play needs --tag T (frames and sheet are named after it)")?;
    if !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("--tag {tag}: letters, digits, - and _ only (it becomes a file name on the box)"));
    }
    let shots: usize = f("--shots").map(|s| s.parse().map_err(|_| "--shots wants a number")).transpose()?.unwrap_or(4);
    let every_ms: u64 = f("--every-ms").map(|s| s.parse().map_err(|_| "--every-ms wants a number")).transpose()?.unwrap_or(200);
    let first_ms: u64 = f("--first-ms").map(|s| s.parse().map_err(|_| "--first-ms wants a number")).transpose()?.unwrap_or(300);
    let timeout: u64 = f("--timeout").map(|s| s.parse().map_err(|_| "--timeout wants seconds")).transpose()?.unwrap_or(600);
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| "/tmp/tiny3".into()));
    let shootctl = f("--box-shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    if !map.exists() {
        return Err(format!("{}: no such file", map.display()));
    }
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let wsx = Wsx::new(args);
    let remote_map = format!("{STAGE}/{tag}Play.Map.Gbx");
    let remote_dir = format!("{SHOTS}/play-{tag}");
    eprintln!("pushing {} to the box …", map.display());
    wsx.push(&map, &remote_map)?;
    let cmd = format!("{shootctl} playshots --detach --map {remote_map} --outdir {remote_dir} --tag {tag} --shots {shots} --every-ms {every_ms} --first-ms {first_ms} --timeout {timeout}");
    eprintln!("playing {tag} on the box ({shots} frames) — waits for the render lock if another thread holds the game …");
    let started = wsx.sh(&cmd)?;
    if wsx.verbose {
        eprintln!("{}", started.trim());
    }
    // the lock wait (600 s) + the load + the frames
    let done = wsx.wait_done(&format!("{remote_dir}/done-play.txt"), &format!("{remote_dir}/playshots.log"), Duration::from_secs(timeout + 900), &format!("playshots {tag}"))?;
    for l in done.lines() {
        eprintln!("  {l}");
    }
    // one sheet: every frame scaled to 960 wide, stacked top to bottom
    let mut inputs = String::new();
    let mut scales = String::new();
    let mut labels = String::new();
    for k in 0..shots {
        inputs.push_str(&format!(" -i {}", to_win(&format!("{remote_dir}/play-{tag}-{k}.png"))));
        scales.push_str(&format!("[{k}:v]scale=960:-1[s{k}];"));
        labels.push_str(&format!("[s{k}]"));
    }
    let sheet_remote = format!("{remote_dir}/play-{tag}-sheet.jpg");
    let filter = if shots > 1 { format!("{scales}{labels}vstack=inputs={shots}") } else { "[0:v]scale=960:-1".to_string() };
    let ff = format!("\"{BOX_FFMPEG}\" -nostdin -y -loglevel error{inputs} -filter_complex \"{filter}\" -q:v 4 {}", to_win(&sheet_remote));
    wsx.sh(&ff).map_err(|e| format!("contact sheet: {e}"))?;
    let local = outdir.join(format!("play-{tag}-sheet.jpg"));
    let n = wsx.pull(&sheet_remote, &local)?;
    println!("{} ({n} B): {shots} frames top to bottom; full frames in {remote_dir} on the box", local.display());
    Ok(())
}
