//! `tinyctl shoot` — both sides of a map's comparison views through the render
//! box, then the diff, in one command from the devserver.
//!
//! ```text
//! tinyctl shoot --orig SRC.Map.Gbx --tiny TINY.Map.Gbx --views VIEWS.tsv --tag sNN
//!               --anchor sx,sy,sz:tx,ty,tz [--outdir /tmp/tiny3] [--only o|t]
//!               [--pull-full] [--box-shootctl P] [--box-tinyctl P] [--wsx P] [-v]
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
    for side in &sides {
        let map = if *side == "o" { &r_orig } else { &r_tiny };
        let anchor_arg = if *side == "t" { format!(" --anchor {anchor}") } else { String::new() };
        let cmd = format!("{shootctl} shootset --detach --map {map} --views {r_views} --side {side} --tag {tag} --outdir {remote_dir}{anchor_arg}");
        eprintln!("shooting side {side} …");
        let started = wsx.sh(&cmd)?;
        if wsx.verbose {
            eprintln!("{}", started.trim());
        }
        let done = wsx.wait_done(&format!("{remote_dir}/done-{side}.txt"), &format!("{remote_dir}/shootset-{side}.log"), Duration::from_secs(1800), &format!("shootset {side}"))?;
        for l in done.lines() {
            eprintln!("  {l}");
        }
    }
    if sides.len() < 2 {
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
        Err(e) => {
            eprintln!("compare on the box failed ({e}); pulling the frames and comparing here");
            pull_frames(&wsx, &remote_dir, &tag, &names, &outdir)?;
            let mut cargs: Vec<String> = vec!["--views".into(), views.to_string_lossy().into_owned(), "--dir".into(), outdir.to_string_lossy().into_owned(), "--tag".into(), tag.clone()];
            if Path::new(&ffmpeg_local).exists() {
                cargs.push("--hstack-ffmpeg".into());
                cargs.push(ffmpeg_local.clone());
            }
            crate::compare::cmd(&cargs)?;
        }
    }
    if tmmaps::cli::has(args, "--pull-full") {
        pull_frames(&wsx, &remote_dir, &tag, &names, &outdir)?;
    }
    eprintln!("look at {}/cmpdiff-{tag}-crops.png, then -overview.png; full frames stay in {remote_dir} on the box", outdir.display());
    Ok(())
}

fn pull_frames(wsx: &Wsx, remote_dir: &str, tag: &str, names: &[String], outdir: &Path) -> Result<(), String> {
    for name in names {
        for side in ["o", "t"] {
            let f = format!("cmp-{tag}{name}-{side}.png");
            let n = wsx.pull(&format!("{remote_dir}/{f}"), &outdir.join(&f))?;
            eprintln!("  pulled {f} ({n} B)");
        }
    }
    Ok(())
}
