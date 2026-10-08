//! `tinyctl play` — a map in PLAY mode on the render box, N timed frames from
//! the moment the playground opens, pulled back as ONE contact sheet.
//!
//! ```text
//! tinyctl play --map MAP --tag T [--shots 4] [--every-ms 200] [--first-ms 300]
//!              [--drive-ms MS [--drive-at-ms 13500]] [--camlog-ms MS] [--wheels-ms MS] [--hold-up [EXE]] [--timeout 600] [--outdir /tmp/tiny3] [--wsx P] [-v]
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

/// The box's Windows SendInput driver that holds the accelerator for 20 s
/// (`--mode collision-ram-straight`), from the MK64 kart lane.
const HOLD_UP_EXE: &str = "/mnt/c/Users/vjeux/inputidle-hold.exe";

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
    crate::inputidle::gate(&wsx, args, "tinyctl play")?;
    let remote_map = format!("{STAGE}/{tag}Play.Map.Gbx");
    let remote_dir = format!("{SHOTS}/play-{tag}");
    eprintln!("pushing {} to the box …", map.display());
    wsx.push(&map, &remote_map)?;
    // --drive-ms MS [--drive-at-ms MS]: hold the accelerator on the box (the
    // car rolls off the start into the in-game triggers ahead of it)
    let drive = match (f("--drive-ms"), f("--drive-at-ms")) {
        (Some(ms), at) => format!(" --drive-ms {ms}{}", at.map(|a| format!(" --drive-at-ms {a}")).unwrap_or_default()),
        (None, _) => String::new(),
    };
    let camlog = f("--camlog-ms").map(|ms| format!(" --camlog-ms {ms}")).unwrap_or_default();
    // --wheels-ms MS: the surface under each wheel per frame (wheels-<tag>.tsv, pulled back)
    let wheels = f("--wheels-ms").map(|ms| format!(" --wheels-ms {ms}")).unwrap_or_default();
    let env = crate::remoteshots::ENV;
    let cmd = format!("{env}; {shootctl} playshots --detach --map {remote_map} --outdir {remote_dir} --tag {tag} --shots {shots} --every-ms {every_ms} --first-ms {first_ms} --timeout {timeout}{drive}{camlog}{wheels}");
    eprintln!("playing {tag} on the box ({shots} frames) — waits for the render lock if another thread holds the game …");
    let started = wsx.sh(&cmd)?;
    if wsx.verbose {
        eprintln!("{}", started.trim());
    }
    // --hold-up [EXE] [--hold-ms 20000]: the moment the playground opens, bring the game
    // to the front and hold the accelerator (`inputidle.exe hold`, one SendInput key
    // down/up by scan code — the MK64 kart driver refuses its setup guard on this box
    // whenever the game owns more than one visible window), so the wheel log
    // (`--wheels-ms`) reads a MOVING car — the surface it drives on, not the spawn
    // (2026-10-06: the Everios96 water-physics question needed a driven measurement;
    // every earlier wheel log was a parked car). The driver needs the game window
    // in the foreground and the Trackmania pid; both are read on the box. A setup
    // refusal (exit 3: the window was not in the foreground at that instant) is
    // retried for a few seconds — the foreground flickers right after a map load.
    if let Some(exe) = f("--hold-up").map(|v| if v.is_empty() || v.starts_with("--") { HOLD_UP_EXE.to_string() } else { v.to_string() }) {
        let hold_ms: u64 = f("--hold-ms").map(|s| s.parse().map_err(|_| "--hold-ms wants a number")).transpose()?.unwrap_or(20000);
        let log = format!("{remote_dir}/playshots.log");
        let deadline = std::time::Instant::now() + Duration::from_secs(timeout + 600);
        loop {
            if std::time::Instant::now() > deadline {
                return Err("hold-up: the playground never opened".into());
            }
            let seen = wsx.sh(&format!("grep -c 'playground after' {log} 2>/dev/null; grep -c -E '^(OK|FAILED)' {log} 2>/dev/null; true"))?;
            let mut it = seen.lines().map(|l| l.trim().parse::<u32>().unwrap_or(0));
            let (pg, ended) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
            if ended > 0 {
                return Err("hold-up: the run ended before the playground opened".into());
            }
            if pg > 0 {
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        let launch = format!(
            "PID=$(/mnt/c/Windows/System32/tasklist.exe /FI 'IMAGENAME eq Trackmania.exe' /NH /FO CSV 2>/dev/null | grep -i trackmania | head -1 | cut -d, -f2 | tr -d '\" \r'); \
             if [ -z \"$PID\" ]; then echo no-pid; else /mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -NoProfile -Command \"(New-Object -ComObject WScript.Shell).AppActivate($PID)\" >/dev/null 2>&1; (nohup {exe} hold --scan 0x48 --ms {hold_ms} > {remote_dir}/hold-up-{tag}.txt 2>&1 &); echo \"holding Up for {hold_ms} ms (game pid $PID in front)\"; fi"
        );
        let out = wsx.sh(&launch)?;
        eprintln!("  hold-up: {}", out.trim());
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
    if !camlog.is_empty() {
        let tsv = format!("cam-{tag}.tsv");
        match wsx.pull(&format!("{remote_dir}/{tsv}"), &outdir.join(&tsv)) {
            Ok(n) => eprintln!("  pulled {tsv} ({n} B)"),
            Err(e) => eprintln!("  {tsv}: {e}"),
        }
    }
    if !wheels.is_empty() {
        let tsv = format!("wheels-{tag}.tsv");
        match wsx.pull(&format!("{remote_dir}/{tsv}"), &outdir.join(&tsv)) {
            Ok(n) => eprintln!("  pulled {tsv} ({n} B)"),
            Err(e) => eprintln!("  {tsv}: {e}"),
        }
    }
    let local = outdir.join(format!("play-{tag}-sheet.jpg"));
    let n = wsx.pull(&sheet_remote, &local)?;
    println!("{} ({n} B): {shots} frames top to bottom; full frames in {remote_dir} on the box", local.display());
    Ok(())
}
