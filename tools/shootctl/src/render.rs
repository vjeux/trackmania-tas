//! `shootctl render` — ONE ghost video on a SHARED game, detachable.
//!
//! ```text
//! shootctl render --map MAP --name NAME --outdir /mnt/c/DIR [--cam 2] [--load-timeout 120]
//!                 [--quit] [--detach] GHOST...
//! ```
//!
//! `run` was written for a box with one driver: it starts the game, renders and
//! closes the game. Today eight sessions share the one Trackmania instance,
//! and what each of them needs is what this does:
//!
//! 1. the render lock, taken HERE by the process that lives as long as the
//!    render (a `sh -c 'acquire; job; release'` wrapper records the shell as
//!    the holder and a bridge timeout leaves it held; a lock taken by the
//!    driver itself dies with the driver and the next acquirer breaks it);
//! 2. launch (only if the game is down), `setup` (map, ghosts, camera),
//!    `shoot` — the game is left UP for the next driver unless `--quit`;
//! 3. the lock released the moment the game is no longer needed, so the
//!    contact sheets below never hold anybody up;
//! 4. `DIR/done-render.txt` — `OK <webm> <bytes> <seconds>` or `FAILED …` —
//!    so a caller on another machine (`tinyctl video`, over the WhiteStick
//!    bridge, which cuts a command at ~90 s) can poll instead of hold a
//!    connection open. `--detach` re-runs in the background with the log in
//!    `DIR/render.log`, exactly like `shootset`/`playshots`.
//!
//! After the lock: ffmpeg (a Windows build, no game needed) writes two contact
//! sheets beside the done file — `DIR/NAME-sheet.png`, 16 tiles spread over
//! the whole clip (0.5 fps up to a 32 s lap; the way to LOOK at a lap in one
//! image), and `DIR/NAME-dense.png`, 2 fps in rows of six, for when the coarse
//! sheet shows something worth a closer look. The clip itself stays where the
//! game wrote it (`ScreenShots/NAME.webm`); it is the caller's to copy.
//! `render --sheets-only WEBM --outdir DIR --name N` writes just the two sheets
//! for a clip that already exists — no game, no lock.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

const SCREENSHOTS: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/ScreenShots";
const MAPS_SHOOT: &str = "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot";
const FFMPEG: &str = "/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin/ffmpeg.exe";
const FFPROBE: &str = "/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin/ffprobe.exe";

struct Opts {
    map: String,
    name: String,
    outdir: PathBuf,
    cam: u8,
    load_timeout_s: u64,
    quit: bool,
    detach: bool,
    ghosts: Vec<String>,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts { map: String::new(), name: String::new(), outdir: PathBuf::new(), cam: 2, load_timeout_s: 120, quit: false, detach: false, ghosts: Vec::new() };
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let val = |i: usize| args.get(i + 1).cloned().ok_or_else(|| format!("{a} needs a value"));
        match a {
            "--map" => { o.map = val(i)?; i += 2; }
            "--name" => { o.name = val(i)?; i += 2; }
            "--outdir" => { o.outdir = PathBuf::from(val(i)?); i += 2; }
            "--cam" => {
                o.cam = val(i)?.parse::<u8>().ok().filter(|c| *c <= 6).ok_or("--cam takes 0..6 (2 External, 1 Internal, 6 Ext2, 3 Helico)")?;
                i += 2;
            }
            "--load-timeout" => { o.load_timeout_s = val(i)?.parse().map_err(|_| "--load-timeout wants seconds")?; i += 2; }
            "--quit" => { o.quit = true; i += 1; }
            "--detach" => { o.detach = true; i += 1; }
            other if other.starts_with("--") => return Err(format!("unknown option: {other}")),
            _ => { o.ghosts.push(args[i].clone()); i += 1; }
        }
    }
    if o.map.is_empty() || o.name.is_empty() || o.outdir.as_os_str().is_empty() || o.ghosts.is_empty() {
        return Err("render --map MAP --name NAME --outdir /mnt/c/DIR [--cam N] [--load-timeout S] [--quit] [--detach] GHOST...".into());
    }
    if !o.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("--name {}: letters, digits, - and _ only (it becomes the clip's file name)", o.name));
    }
    for g in &o.ghosts {
        if !Path::new(g).is_file() {
            return Err(format!("{g}: no such ghost file"));
        }
    }
    Ok(o)
}

pub fn run(args: &[String]) -> i32 {
    // `render --sheets-only WEBM --outdir DIR --name N`: the two contact sheets
    // for a clip that already exists (rendered by `run`/vid.sh before this
    // command did), no game, no lock.
    if args.iter().any(|a| a == "--sheets-only") {
        return sheets_only(args);
    }
    let opts = match parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&opts.outdir) {
        eprintln!("{}: {e}", opts.outdir.display());
        return 2;
    }
    let done = opts.outdir.join("done-render.txt");
    let _ = std::fs::remove_file(&done);
    if opts.detach {
        return super::shootset::detach_as(&opts.outdir.join("render.log"), &done);
    }
    let t0 = Instant::now();
    let result = render(&opts, t0);
    let summary = match &result {
        Ok((webm, bytes, secs)) => format!("OK {webm} {bytes} {secs:.3}\n"),
        Err(e) => format!("FAILED after {:.0}s: {e}\n", t0.elapsed().as_secs_f64()),
    };
    print!("{summary}");
    let tmp = opts.outdir.join("done-render.tmp");
    if std::fs::write(&tmp, &summary).and_then(|_| std::fs::rename(&tmp, &done)).is_err() {
        eprintln!("could not write {}", done.display());
        return 1;
    }
    if result.is_ok() { 0 } else { 1 }
}

/// The clip: path, bytes, duration in seconds.
fn render(opts: &Opts, t0: Instant) -> Result<(String, u64, f64), String> {
    let el = || format!("[{:6.1}s]", t0.elapsed().as_secs_f64());
    let webm = format!("{SCREENSHOTS}/{}.webm", opts.name);
    {
        // THE GAME-DRIVING PART, and only that, under the lock.
        let d = super::lock::lock_dir();
        let owner = format!("render-{}", opts.name);
        super::lock::acquire(&d, &owner, 1500, 0).map_err(|e| format!("lock: {e}"))?;
        let _guard = super::shootset::LockGuard::new(d, owner);
        super::LOAD_TIMEOUT_S.store(opts.load_timeout_s, Ordering::Relaxed);
        let staged = super::shootset::stage_map(&opts.map)?;
        println!("{} map {}", el(), staged);
        if super::launch(180, false) != 0 {
            return Err("the game did not come up".into());
        }
        let rc = super::setup(&staged, &opts.ghosts, opts.cam);
        if rc != 0 {
            return Err(format!("setup failed (rc {rc}) — see the lines above"));
        }
        println!("{} scene ready, shooting {}", el(), opts.name);
        let before = super::webm_snapshot();
        let rc = super::shoot(3600, &opts.name);
        if rc != 0 {
            return Err(format!("shoot failed (rc {rc}) — see the lines above"));
        }
        // DISK HYGIENE. `shoot` copies the game's VideoNN.webm to NAME.webm and
        // leaves the original: two 30 MB copies per lap on a C: drive that
        // was 99 % full the morning this was written. The original goes.
        for p in super::webms_changed(&before) {
            if !super::same_file(&p, &webm) && p != webm {
                match std::fs::remove_file(&p) {
                    Ok(()) => println!("{} removed the game's own copy {}", el(), p),
                    Err(e) => println!("{} could not remove {p}: {e}", el()),
                }
            }
        }
        // The Maps/_shoot copy `stage_map` made is a copy of the staged file;
        // the next render stages it again from `_stage` in a second.
        if staged != opts.map && !opts.map.starts_with(MAPS_SHOOT) {
            match std::fs::remove_file(&staged) {
                Ok(()) => println!("{} removed the staged copy {}", el(), staged),
                Err(e) => println!("{} could not remove {staged}: {e}", el()),
            }
        }
        if opts.quit {
            super::quit_game();
        }
        // _guard drops here: the lock is released before the sheets.
    }
    let bytes = std::fs::metadata(&webm).map(|m| m.len()).map_err(|e| format!("{webm}: {e}"))?;
    if bytes == 0 {
        return Err(format!("{webm} is empty"));
    }
    let secs = duration_s(&webm)?;
    println!("{} clip {} ({bytes} bytes, {secs:.3} s)", el(), webm);
    let (_, _sheet, _dense) = sheets(&webm, &opts.outdir, &opts.name)?;
    Ok((webm, bytes, secs))
}

fn win(p: &str) -> String {
    p.strip_prefix("/mnt/c/").map(|r| format!("C:/{r}")).unwrap_or_else(|| p.to_string())
}

fn duration_s(webm: &str) -> Result<f64, String> {
    let out = std::process::Command::new(FFPROBE)
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"])
        .arg(win(webm))
        .output()
        .map_err(|e| format!("ffprobe: {e}"))?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    s.parse::<f64>().map_err(|_| format!("ffprobe gave no duration for {webm}: `{s}` {}", String::from_utf8_lossy(&out.stderr).trim()))
}

fn contact_sheet(webm: &str, out: &Path, fps: f64, tile_w: u32, cols: u32, rows: u32) -> Result<(), String> {
    let out_s = out.to_str().ok_or("sheet path is not utf-8")?;
    let filter = format!("fps={fps:.4},scale={tile_w}:-1,tile={cols}x{rows}");
    let st = std::process::Command::new(FFMPEG)
        .args(["-nostdin", "-y", "-loglevel", "error", "-i"])
        .arg(win(webm))
        .args(["-vf", &filter, "-frames:v", "1"])
        .arg(win(out_s))
        .status()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if !st.success() {
        return Err(format!("ffmpeg failed ({st}) writing {}", out.display()));
    }
    let n = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    if n == 0 {
        return Err(format!("{}: ffmpeg wrote nothing", out.display()));
    }
    Ok(())
}

fn sheets_only(args: &[String]) -> i32 {
    let val = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let webm = args.iter().enumerate().find(|(i, a)| !a.starts_with("--") && (*i == 0 || !matches!(args[i - 1].as_str(), "--outdir" | "--name"))).map(|(_, a)| a.clone());
    let (Some(webm), Some(outdir), Some(name)) = (webm, val("--outdir"), val("--name")) else {
        eprintln!("render --sheets-only WEBM --outdir /mnt/c/DIR --name N");
        return 2;
    };
    if let Err(e) = std::fs::create_dir_all(&outdir) {
        eprintln!("{outdir}: {e}");
        return 2;
    }
    match sheets(&webm, Path::new(&outdir), &name) {
        Ok((secs, sheet, dense)) => {
            println!("OK {webm} {secs:.3} {} {}", sheet.display(), dense.display());
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// The two contact sheets of a clip: (duration, coarse sheet, dense sheet).
fn sheets(webm: &str, outdir: &Path, name: &str) -> Result<(f64, PathBuf, PathBuf), String> {
    let secs = duration_s(webm)?;
    let sheet = outdir.join(format!("{name}-sheet.png"));
    let dense = outdir.join(format!("{name}-dense.png"));
    // 16 tiles over the whole clip: 0.5 fps until the lap outgrows 32 s.
    let fps = (15.9 / secs).min(0.5);
    contact_sheet(webm, &sheet, fps, 480, 4, 4)?;
    println!("sheet {} ({fps:.3} fps, 4x4)", sheet.display());
    // 2 fps in rows of six; slower for a lap that would need more than 12 rows.
    let dfps = (71.9 / secs).min(2.0);
    let frames = (secs * dfps).ceil() as u32;
    let rows = frames.div_ceil(6).max(1);
    contact_sheet(webm, &dense, dfps, 320, 6, rows)?;
    println!("dense {} ({dfps:.3} fps, 6x{rows})", dense.display());
    Ok((secs, sheet, dense))
}
