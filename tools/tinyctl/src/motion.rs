//! `tinyctl motion` — side-by-side VIDEO of the moving blocks: original LEFT,
//! tiny RIGHT, same relative camera (the tiny at half the distance through the
//! anchor, exactly as `tinyctl shoot` frames a still), same duration, so a
//! tiny at 2x should read frame for frame like the original — stroke, period,
//! phase (vjeux, 2026-09-08: "verification must be a side-by-side video, not
//! stills").
//!
//! Capture = the track editor (the game's kinematic clock runs there), the
//! orbital camera of `shootctl shootset`, and `--video S` on it: ffmpeg's
//! gdigrab reads the screen at a fixed rate for S seconds. Both worlds are
//! captured the same way, the same settle after the editor opens; the seconds
//! from "editor open" to "capture start" are in each side's log line, so a
//! phase offset between the two can be read off — and `--shift-ms` slides the
//! tiny clip against the original when it is.
//!
//! Then on the box: original | tiny stitched into `motion-<tag><view>.webm`
//! (1920x540: two 960x540 panes) and a contact sheet of the first 8 seconds,
//! one frame per second (`motion-<tag><view>-sheet.png`, 2x4 tiles). The webm
//! also goes to `Maps\Tiny\videos\` on the box; the sheets come here.
//!
//!   tinyctl motion --orig SRC --tiny TINY --views V.tsv --anchor A --tag T
//!                  [--seconds 8] [--fps 20] [--outdir /tmp/tinyvid/motion]
//!                  [--settle-ms 5000] [--shift-ms N] [--only o|t] [-v]

use crate::wsx::Wsx;
use std::path::{Path, PathBuf};
use std::time::Duration;

const STAGE: &str = "/home/vjeux/shoot/_stage";
const SHOTS: &str = "/mnt/c/Users/vjeux/tinyshots";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";
const BOX_FFMPEG: &str = "/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin/ffmpeg.exe";
const BOX_VIDEOS: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/Tiny/videos";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let orig = PathBuf::from(f("--orig").ok_or("motion needs --orig SRC.Map.Gbx")?);
    let tiny = PathBuf::from(f("--tiny").ok_or("motion needs --tiny TINY.Map.Gbx")?);
    let views = PathBuf::from(f("--views").ok_or("motion needs --views VIEWS.tsv")?);
    let tag = f("--tag").ok_or("motion needs --tag mNN")?;
    let anchor = f("--anchor").ok_or("motion needs --anchor sx,sy,sz:tx,ty,tz (tinyctl views prints it)")?;
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| "/tmp/tinyvid/motion".into()));
    let seconds: f64 = f("--seconds").map(|s| s.parse().map_err(|_| "--seconds wants a number")).transpose()?.unwrap_or(8.0);
    let fps: u64 = f("--fps").map(|s| s.parse().map_err(|_| "--fps wants a number")).transpose()?.unwrap_or(20);
    let shift_ms: i64 = f("--shift-ms").map(|s| s.parse().map_err(|_| "--shift-ms wants milliseconds (tiny later = positive)")).transpose()?.unwrap_or(0);
    let only = f("--only");
    let settle_arg = f("--settle-ms").map(|ms| format!(" --settle-ms {ms}")).unwrap_or_default();
    for p in [&orig, &tiny, &views] {
        if !p.exists() {
            return Err(format!("{}: no such file", p.display()));
        }
    }
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let names = crate::compare_view_names(&views)?;
    if names.is_empty() {
        return Err(format!("{}: no view rows", views.display()));
    }
    let wsx = Wsx::new(args);
    let remote_dir = format!("{SHOTS}/{tag}");
    let r_orig = format!("{STAGE}/{tag}Orig.Map.Gbx");
    let r_tiny = format!("{STAGE}/{tag}Tiny.Map.Gbx");
    let r_views = format!("{STAGE}/{tag}-views.tsv");

    // ONE push at a time, and none for bytes the box already has (fleet rule).
    wsx.push(&views, &r_views)?;
    if only.as_deref() != Some("t") {
        push_if_changed(&wsx, &orig, &r_orig)?;
    }
    if only.as_deref() != Some("o") {
        push_if_changed(&wsx, &tiny, &r_tiny)?;
    }

    let sides: Vec<&str> = match only.as_deref() {
        Some("o") => vec!["o"],
        Some("t") => vec!["t"],
        _ => vec!["o", "t"],
    };
    let shootctl = format!("{BOX_TOOLS}/shootctl");
    let mut opened_to_capture: Vec<(String, String)> = Vec::new();
    for side in &sides {
        let map = if *side == "o" { &r_orig } else { &r_tiny };
        let anchor_arg = if *side == "t" { format!(" --anchor {anchor}") } else { String::new() };
        let cmd = format!("{shootctl} shootset --detach --map {map} --views {r_views} --side {side} --tag {tag} --outdir {remote_dir}{anchor_arg}{settle_arg} --video {seconds:.1} --video-fps {fps}");
        eprintln!("capturing side {side}: {} view(s) x {seconds:.0} s at {fps} fps …", names.len());
        let started = wsx.sh(&cmd)?;
        if wsx.verbose {
            eprintln!("{}", started.trim());
        }
        // a view costs settle + capture + the aim; the editor load up to 7 min
        let budget = Duration::from_secs(1800 + (names.len() as u64) * (seconds as u64 + 20));
        let done = wsx.wait_done(&format!("{remote_dir}/done-{side}.txt"), &format!("{remote_dir}/shootset-{side}.log"), budget, &format!("shootset {side}"))?;
        for l in done.lines() {
            eprintln!("  {l}");
            if let Some(p) = l.find("video ") {
                opened_to_capture.push((format!("{side} {}", l.split('\t').next().unwrap_or("")), l[p..].to_string()));
            }
        }
    }
    if sides.len() < 2 {
        eprintln!("one side only — clips are in {remote_dir} on the box; run both sides for the stitch");
        return Ok(());
    }

    // --- stitch + sheet on the box, one call
    let mut script = String::new();
    script.push_str(&format!("mkdir -p '{BOX_VIDEOS}'; cd '{remote_dir}' || exit 1; F='{BOX_FFMPEG}'; "));
    for n in &names {
        let o = win(&format!("{remote_dir}/cmp-{tag}{n}-o.mp4"));
        let t = win(&format!("{remote_dir}/cmp-{tag}{n}-t.mp4"));
        let webm = win(&format!("{remote_dir}/motion-{tag}{n}.webm"));
        let sheet = win(&format!("{remote_dir}/motion-{tag}{n}-sheet.png"));
        // --shift-ms: the tiny pane starts N ms later (positive) or earlier
        // (negative) — `setpts` on one input, trimmed to the common span
        let (tiny_filter, orig_filter) = if shift_ms > 0 {
            (format!("trim=start={:.3},setpts=PTS-STARTPTS,", shift_ms as f64 / 1000.0), String::new())
        } else if shift_ms < 0 {
            (String::new(), format!("trim=start={:.3},setpts=PTS-STARTPTS,", (-shift_ms) as f64 / 1000.0))
        } else {
            (String::new(), String::new())
        };
        script.push_str(&format!(
            "\"$F\" -y -loglevel error -i '{o}' -i '{t}' -filter_complex \"[0:v]{orig_filter}scale=960:540[a];[1:v]{tiny_filter}scale=960:540[b];[a][b]hstack=shortest=1\" -c:v libvpx-vp9 -b:v 0 -crf 30 -row-mt 1 -r {fps} '{webm}' && "
        ));
        script.push_str(&format!(
            "\"$F\" -y -loglevel error -i '{webm}' -vf fps=1,tile=2x4 -frames:v 1 '{sheet}' && "
        ));
        script.push_str(&format!("cp -f 'motion-{tag}{n}.webm' '{BOX_VIDEOS}/motion-{tag}{n}.webm' && echo 'stitched motion-{tag}{n}.webm' $(stat -c %s 'motion-{tag}{n}.webm') B; "));
    }
    eprintln!("stitching original | tiny on the box …");
    let out = wsx.sh(&script)?;
    for l in out.lines() {
        eprintln!("  {}", l.trim());
    }
    for n in &names {
        let sheet = format!("motion-{tag}{n}-sheet.png");
        let bytes = wsx.pull(&format!("{remote_dir}/{sheet}"), &outdir.join(&sheet))?;
        println!("sheet: {} ({bytes} B)", outdir.join(&sheet).display());
        println!("webm:  {BOX_VIDEOS}/motion-{tag}{n}.webm (box) — also {remote_dir}/motion-{tag}{n}.webm");
    }
    if !opened_to_capture.is_empty() {
        println!("phase reference (capture start after the editor opened):");
        for (k, v) in &opened_to_capture {
            println!("  {k}: {v}");
        }
        println!("  a stroke seen later on one side by D s = that side's kinematic clock started later; re-run with --shift-ms to slide the tiny pane (tiny later = positive)");
    }
    Ok(())
}

/// A WSL `/mnt/c/...` path as `C:/...` for ffmpeg.exe (forward slashes: the
/// nested sh eats backslashes).
fn win(p: &str) -> String {
    match p.strip_prefix("/mnt/c/") {
        Some(rest) => format!("C:/{rest}"),
        None => p.to_string(),
    }
}

/// Push `local` to `remote` on the box unless the box already holds these bytes.
fn push_if_changed(wsx: &Wsx, local: &Path, remote: &str) -> Result<(), String> {
    let want = crate::video::md5_of(local)?;
    let have = wsx.sh(&format!("md5sum '{remote}' 2>/dev/null | cut -c1-32")).unwrap_or_default().trim().to_string();
    if have == want {
        eprintln!("{} already on the box ({want}) — not pushing", local.file_name().and_then(|s| s.to_str()).unwrap_or("?"));
        return Ok(());
    }
    eprintln!("pushing {} ({} MB) …", local.display(), std::fs::metadata(local).map(|m| m.len() / 1_000_000).unwrap_or(0));
    wsx.push(local, remote)?;
    let now = wsx.sh(&format!("md5sum '{remote}' | cut -c1-32")).unwrap_or_default().trim().to_string();
    if now != want {
        return Err(format!("{remote} on the box reads md5 {now}, pushed {want}"));
    }
    Ok(())
}
