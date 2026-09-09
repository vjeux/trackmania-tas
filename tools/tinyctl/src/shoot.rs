//! `tinyctl shoot` — both sides of a map's comparison views through the render
//! box, then the diff, in one command from the devserver.
//!
//! ```text
//! tinyctl shoot --orig SRC.Map.Gbx --tiny TINY.Map.Gbx --views VIEWS.tsv --tag sNN
//!               --anchor sx,sy,sz:tx,ty,tz [--outdir /tmp/tiny3] [--only o|t]
//!               [--pull-full] [--ab] [--fresh] [--box-shootctl P] [--box-tinyctl P] [--wsx P] [-v]
//! ```
//!
//! What happens, and where:
//!
//! 1. the two maps and the views file are pushed to the box's staging dir;
//! 2. `shootctl shootset --detach` runs there for the original, then for the
//!    tiny map: ONE editor load per side, one screenshot per view, into
//!    `/mnt/c/Users/vjeux/tinyshots/<tag>/`; this side polls the done files;
//! 3. `tinyctl compare` runs ON THE BOX (the full-resolution frames never
//!    cross the bridge: a 3840x2160 capture is ~10 MB and the bridge moves
//!    ~1.4 MB/s) and writes the overview + crops sheets and the plain
//!    side-by-side JPGs; when the box has no `tinyctl` yet the frames are
//!    pulled and compared here instead;
//! 4. the sheets, the report and the JPGs are pulled into `--outdir`.
//!
//! The summary printed at the end is the compare's: which views have flagged
//! cells and how many. Look at `cmpdiff-<tag>-crops.png` first.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::wsx::Wsx;

const STAGE: &str = "/home/vjeux/shoot/_stage";
const SHOTS: &str = "/mnt/c/Users/vjeux/tinyshots";
const BOX_TOOLS: &str = "/home/vjeux/trackmania-tas/tools/target/release";
const BOX_FFMPEG: &str = "/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin/ffmpeg.exe";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let orig = PathBuf::from(f("--orig").ok_or("shoot needs --orig SRC.Map.Gbx")?);
    let tiny = PathBuf::from(f("--tiny").ok_or("shoot needs --tiny TINY.Map.Gbx")?);
    let views = PathBuf::from(f("--views").ok_or("shoot needs --views VIEWS.tsv")?);
    let tag = f("--tag").ok_or("shoot needs --tag sNN")?;
    let anchor = f("--anchor").ok_or("shoot needs --anchor sx,sy,sz:tx,ty,tz (tinyctl views prints it)")?;
    let outdir = PathBuf::from(f("--outdir").unwrap_or_else(|| "/tmp/tiny3".into()));
    let only = f("--only");
    // --ab: BOTH maps are tiny builds (an A/B of two `tmmaps tiny` outputs,
    // e.g. parked vs deleted blocks), so the orig side is shot through the
    // anchor as well and the views file stays in source coordinates.
    let ab = tmmaps::cli::has(args, "--ab");
    let shootctl = f("--box-shootctl").unwrap_or_else(|| format!("{BOX_TOOLS}/shootctl"));
    let tinyctl = f("--box-tinyctl").unwrap_or_else(|| format!("{BOX_TOOLS}/tinyctl"));
    let ffmpeg_local = f("--ffmpeg").unwrap_or_else(|| format!("{}/bin/ffmpeg", std::env::var("HOME").unwrap_or_else(|_| "/home/vjeux".into())));
    for p in [&orig, &tiny, &views] {
        if !p.exists() {
            return Err(format!("{}: no such file", p.display()));
        }
    }
    std::fs::create_dir_all(&outdir).map_err(|e| format!("{}: {e}", outdir.display()))?;
    let wsx = Wsx::new(args);
    let remote_dir = format!("{SHOTS}/{tag}");
    let r_orig = format!("{STAGE}/{tag}Orig.Map.Gbx");
    let r_tiny = format!("{STAGE}/{tag}Tiny.Map.Gbx");
    let r_views = format!("{STAGE}/{tag}-views.tsv");

    eprintln!("pushing maps + views to the box …");
    wsx.push(&views, &r_views)?;
    if only.as_deref() != Some("t") {
        wsx.push(&orig, &r_orig)?;
    }
    if only.as_deref() != Some("o") {
        wsx.push(&tiny, &r_tiny)?;
    }

    let sides: Vec<&str> = match only.as_deref() {
        Some("o") => vec!["o"],
        Some("t") => vec!["t"],
        _ => vec!["o", "t"],
    };
    // --get [N:]/route (repeatable) goes through to shootset: a plugin route
    // fired once the map is open (N: before view N), its answer in the log
    let mut get_args = String::new();
    for (i, a) in args.iter().enumerate() {
        if a == "--get" {
            if let Some(route) = args.get(i + 1) {
                get_args.push_str(&format!(" --get '{route}'"));
            }
        }
    }
    // --settle-ms MS goes through to shootset (the pause before each shot; the
    // in-game advertisements rotate, so a skin probe repeats one view with a
    // longer pause to see every state)
    let settle_arg = f("--settle-ms").map(|ms| format!(" --settle-ms {ms}")).unwrap_or_default();
    // --fresh: restart the game before the first load, under the render lock.
    // The client caches an embedded item model by FILE NAME for the whole
    // game session (coordinator, 2026-09-09 06:16Z): two builds of one map
    // reuse AC/AV names for different pieces, so a frame of build B shot after
    // build A can show A's pieces under B's names. A before/after pair is
    // evidence only when its first side is the first load of a session (or
    // every side carries names never loaded before — TINY_ALIAS_BASE). The
    // restart runs detached (a launch can take minutes; a bridge command is
    // capped at 90 s) and hands the lock back before the shootsets take it.
    if tmmaps::cli::has(args, "--fresh") {
        eprintln!("restarting the game first (--fresh) …");
        let done = format!("{STAGE}/{tag}-fresh-done.txt");
        let log = format!("{STAGE}/{tag}-fresh.log");
        let owner = format!("{tag}-fresh");
        let job = format!("{shootctl} lock acquire --owner {owner} --wait 900 || exit 3; {shootctl} quit; {shootctl} launch 300 --force; rc=$?; {shootctl} lock release --owner {owner}; if [ $rc = 0 ]; then echo OK fresh game > {done}; else echo FAILED launch rc=$rc > {done}; fi");
        wsx.sh(&format!("rm -f {done}; nohup setsid sh -c '{job}' > {log} 2>&1 < /dev/null &"))?;
        let text = wsx.wait_done(&done, &log, Duration::from_secs(1200), "fresh game")?;
        eprintln!("  {}", text.trim());
    }
    for side in &sides {
        let map = if *side == "o" { &r_orig } else { &r_tiny };
        // shootset maps the camera through the anchor for --side t only, so
        // an --ab orig is shot AS side t (own tag, own dir) and its frames
        // are then moved under the compare's -o names.
        let as_t = *side == "o" && ab;
        let (shoot_side, shoot_tag, shoot_dir) = if as_t { ("t", format!("{tag}A"), format!("{remote_dir}A")) } else { (*side, tag.clone(), remote_dir.clone()) };
        // --scale S: the tiny side's camera scale (default 0.5); `--scale 1 --anchor
        // 0,0,0:0,0,0` shoots a FULL-SIZE variant of the original (a map with
        // records removed) from the very same cameras — the "original minus X"
        // protocol that tells what a record contributes to the picture
        let scale_arg = f("--scale").map(|s| format!(" --scale {s}")).unwrap_or_default();
        let anchor_arg = if shoot_side == "t" { format!(" --anchor {anchor}{scale_arg}") } else { String::new() };
        // --shadows Q: compute the lightmap on both sides before shooting
        let shadows_arg = f("--shadows").map(|q| format!(" --shadows {q}")).unwrap_or_default();
        let cmd = format!("{shootctl} shootset --detach --map {map} --views {r_views} --side {shoot_side} --tag {shoot_tag} --outdir {shoot_dir}{anchor_arg}{shadows_arg}{settle_arg}{get_args}");
        eprintln!("shooting side {side}{} …", if as_t { " (a tiny build: through the anchor)" } else { "" });
        let started = wsx.sh(&cmd)?;
        if wsx.verbose {
            eprintln!("{}", started.trim());
        }
        let done = wsx.wait_done(&format!("{shoot_dir}/done-{shoot_side}.txt"), &format!("{shoot_dir}/shootset-{shoot_side}.log"), Duration::from_secs(1800), &format!("shootset {side}"))?;
        for l in done.lines() {
            eprintln!("  {l}");
        }
        if as_t {
            let names = crate::compare_view_names(&views)?;
            let mvs: Vec<String> = names.iter().map(|n| format!("mv -f {shoot_dir}/cmp-{shoot_tag}{n}-t.png {remote_dir}/cmp-{tag}{n}-o.png")).collect();
            wsx.sh(&format!("mkdir -p {remote_dir}; {}; cp -f {shoot_dir}/shootset-t.log {remote_dir}/shootset-o.log", mvs.join("; ")))?;
        }
    }
    // One side re-shot (a fix on the tiny side): the other side's frames are
    // still on the box from the first shoot, so the comparison runs the same
    // way — it needs the frames, not the shoot. `--no-compare` skips it.
    let one_side = sides.len() < 2;
    if one_side && tmmaps::cli::has(args, "--no-compare") {
        if tmmaps::cli::has(args, "--pull-full") {
            let names = crate::compare_view_names(&views)?;
            pull_frames(&wsx, &remote_dir, &tag, &names, &sides, &outdir)?;
        }
        eprintln!("one side only — no comparison; frames are in {remote_dir} on the box");
        return Ok(());
    }

    // --- compare on the box, or here
    let names = crate::compare_view_names(&views)?;
    let compared_remote = wsx.sh(&format!("{tinyctl} compare --views {r_views} --dir {remote_dir} --tag {tag} --hstack-ffmpeg {BOX_FFMPEG}"));
    match compared_remote {
        Ok(summary) => {
            println!("{}", summary.trim());
            for suffix in ["-overview.png", "-crops.png", ".tsv"] {
                let name = format!("cmpdiff-{tag}{suffix}");
                let n = wsx.pull(&format!("{remote_dir}/{name}"), &outdir.join(&name))?;
                eprintln!("  pulled {name} ({n} B)");
            }
            for name in &names {
                let jpg = format!("cmp-{tag}{name}.jpg");
                if let Err(e) = wsx.pull(&format!("{remote_dir}/{jpg}"), &outdir.join(&jpg)) {
                    eprintln!("  {jpg}: {e}");
                }
            }
        }
        Err(e) if one_side => {
            eprintln!("compare on the box failed ({e}) — the other side's frames are probably not in {remote_dir}; shoot both sides once");
            return Ok(());
        }
        Err(e) => {
            eprintln!("compare on the box failed ({e}); pulling the frames and comparing here");
            pull_frames(&wsx, &remote_dir, &tag, &names, &["o", "t"], &outdir)?;
            let mut cargs: Vec<String> = vec!["--views".into(), views.to_string_lossy().into_owned(), "--dir".into(), outdir.to_string_lossy().into_owned(), "--tag".into(), tag.clone()];
            if Path::new(&ffmpeg_local).exists() {
                cargs.push("--hstack-ffmpeg".into());
                cargs.push(ffmpeg_local.clone());
            }
            crate::compare::cmd(&cargs)?;
        }
    }
    if tmmaps::cli::has(args, "--pull-full") {
        pull_frames(&wsx, &remote_dir, &tag, &names, &["o", "t"], &outdir)?;
    }
    eprintln!("look at {}/cmpdiff-{tag}-crops.png, then -overview.png; full frames stay in {remote_dir} on the box", outdir.display());
    Ok(())
}

/// The full frames of `sides` (`o`/`t`), one per view, into `outdir`.
fn pull_frames(wsx: &Wsx, remote_dir: &str, tag: &str, names: &[String], sides: &[&str], outdir: &Path) -> Result<(), String> {
    for name in names {
        for side in sides {
            let f = format!("cmp-{tag}{name}-{side}.png");
            let n = wsx.pull(&format!("{remote_dir}/{f}"), &outdir.join(&f))?;
            eprintln!("  pulled {f} ({n} B)");
        }
    }
    Ok(())
}
